//! Merkle Tree for Network Commitment
//!
//! Provides cryptographic commitment to the network topology.
//! Used for zero-knowledge membership proofs in phantom-zkvm.

use blake3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::error::{ProtocolError, Result};

/// A Merkle tree for efficient set membership proofs
/// 
/// This is a sparse binary Merkle tree where:
/// - Leaves are node IDs (hashed)
/// - Internal nodes are hash(left || right)
/// - Root commits to entire network topology
#[derive(Clone, Debug)]
pub struct MerkleTree {
    /// Sparse storage: only store non-zero nodes
    /// Key: node index (level, position)
    /// Value: hash value
    nodes: HashMap<NodeIndex, Hash>,
    
    /// Tree depth (determines max capacity = 2^depth)
    depth: usize,
    
    /// Current root hash
    root: Hash,
    
    /// Number of leaves currently in tree
    leaf_count: usize,
}

/// Node index in the tree: (level, position)
/// Level 0 = leaves, level depth = root
type NodeIndex = (usize, u64);

/// 32-byte hash value (Blake3)
pub type Hash = [u8; 32];

/// Merkle proof for set membership
/// 
/// Proves: "leaf X is in the tree with root R"
/// Without revealing: position of X or other leaves
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MerkleProof {
    /// Index of the leaf in the tree
    pub leaf_index: u64,
    
    /// Sibling hashes along path from leaf to root
    /// Length = depth of tree
    pub siblings: Vec<Hash>,
    
    /// The root hash this proof is for
    pub root: Hash,
}

impl MerkleTree {
    /// Create a new Merkle tree with specified depth
    /// 
    /// Max capacity = 2^depth leaves
    /// Typical depths:
    /// - depth 10: 1,024 nodes
    /// - depth 16: 65,536 nodes  
    /// - depth 20: 1,048,576 nodes (1M nodes)
    pub fn new(depth: usize) -> Self {
        if depth == 0 || depth > 32 {
            panic!("Merkle tree depth must be between 1 and 32");
        }

        // Empty tree has zero hash as root
        let root = [0u8; 32];

        Self {
            nodes: HashMap::new(),
            depth,
            root,
            leaf_count: 0,
        }
    }

    /// Insert a node ID into the tree
    /// 
    /// This recomputes the path from leaf to root (~O(log n) operations)
    pub fn insert(&mut self, node_id: u32) -> Result<()> {
        let max_leaves = 1usize << self.depth;
        if self.leaf_count >= max_leaves {
            return Err(ProtocolError::InvalidPath(
                format!("Merkle tree full (max {} leaves)", max_leaves)
            ));
        }

        // Hash the node ID to get leaf value
        let leaf_hash = hash_node_id(node_id);
        
        // Insert at next available position
        let leaf_index = self.leaf_count as u64;
        self.nodes.insert((0, leaf_index), leaf_hash);
        
        // Recompute path to root
        self.recompute_path(leaf_index)?;
        
        self.leaf_count += 1;
        Ok(())
    }

    /// Remove a node ID from the tree
    /// 
    /// Sets the leaf to zero hash and recomputes path
    pub fn remove(&mut self, node_id: u32) -> Result<bool> {
        // Find the leaf index for this node_id
        let target_hash = hash_node_id(node_id);
        
        let mut found_index = None;
        for (&(level, pos), &hash) in &self.nodes {
            if level == 0 && hash == target_hash {
                found_index = Some(pos);
                break;
            }
        }

        if let Some(index) = found_index {
            // Set leaf to zero (deletion)
            self.nodes.insert((0, index), [0u8; 32]);
            
            // Recompute path to root
            self.recompute_path(index)?;
            
            self.leaf_count -= 1;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get Merkle proof for a node ID
    /// 
    /// Returns None if node not in tree
    pub fn get_proof(&self, node_id: u32) -> Option<MerkleProof> {
        // Find the leaf index for this node_id
        let target_hash = hash_node_id(node_id);
        
        let mut leaf_index = None;
        for (&(level, pos), &hash) in &self.nodes {
            if level == 0 && hash == target_hash {
                leaf_index = Some(pos);
                break;
            }
        }

        let leaf_index = leaf_index?;

        // Collect sibling hashes along path to root
        let mut siblings = Vec::new();
        let mut current_index = leaf_index;

        for level in 0..self.depth {
            // Sibling is at same level, adjacent position
            let sibling_index = if current_index % 2 == 0 {
                current_index + 1
            } else {
                current_index - 1
            };

            let sibling_hash = self.nodes
                .get(&(level, sibling_index))
                .copied()
                .unwrap_or([0u8; 32]); // Missing nodes = zero hash

            siblings.push(sibling_hash);

            // Move up to parent
            current_index /= 2;
        }

        Some(MerkleProof {
            leaf_index,
            siblings,
            root: self.root,
        })
    }

    /// Verify a Merkle proof
    /// 
    /// Returns true if proof is valid for this tree's current root
    pub fn verify_proof(&self, node_id: u32, proof: &MerkleProof) -> bool {
        // Check root matches
        if proof.root != self.root {
            return false;
        }

        // Check proof depth matches tree depth
        if proof.siblings.len() != self.depth {
            return false;
        }

        // Recompute root from proof
        let leaf_hash = hash_node_id(node_id);
        let computed_root = compute_root_from_proof(leaf_hash, proof.leaf_index, &proof.siblings);

        computed_root == self.root
    }

    /// Get the current root hash
    pub fn root(&self) -> &Hash {
        &self.root
    }

    /// Get the number of leaves in the tree
    pub fn len(&self) -> usize {
        self.leaf_count
    }

    /// Check if tree is empty
    pub fn is_empty(&self) -> bool {
        self.leaf_count == 0
    }

    /// Recompute path from leaf to root after an update
    fn recompute_path(&mut self, leaf_index: u64) -> Result<()> {
        let mut current_index = leaf_index;

        // Walk up the tree, recomputing hashes
        for level in 0..self.depth {
            // Get left and right children (or current node and sibling)
            let (left_hash, right_hash) = if current_index % 2 == 0 {
                // Current node is left child
                let left = self.nodes.get(&(level, current_index)).copied().unwrap_or([0u8; 32]);
                let right = self.nodes.get(&(level, current_index + 1)).copied().unwrap_or([0u8; 32]);
                (left, right)
            } else {
                // Current node is right child
                let left = self.nodes.get(&(level, current_index - 1)).copied().unwrap_or([0u8; 32]);
                let right = self.nodes.get(&(level, current_index)).copied().unwrap_or([0u8; 32]);
                (left, right)
            };

            // Compute parent hash
            let parent_hash = hash_pair(left_hash, right_hash);
            
            // Store parent
            let parent_index = current_index / 2;
            self.nodes.insert((level + 1, parent_index), parent_hash);

            // Move up to parent
            current_index = parent_index;
        }

        // The root is at (depth, 0)
        self.root = self.nodes.get(&(self.depth, 0)).copied().unwrap_or([0u8; 32]);

        Ok(())
    }
}

/// Hash a node ID to a leaf value
fn hash_node_id(node_id: u32) -> Hash {
    let bytes = node_id.to_le_bytes();
    *blake3::hash(&bytes).as_bytes()
}

/// Hash a pair of values (for internal nodes)
fn hash_pair(left: Hash, right: Hash) -> Hash {
    let mut combined = Vec::with_capacity(64);
    combined.extend_from_slice(&left);
    combined.extend_from_slice(&right);
    *blake3::hash(&combined).as_bytes()
}

/// Compute root hash from a Merkle proof
fn compute_root_from_proof(leaf_hash: Hash, leaf_index: u64, siblings: &[Hash]) -> Hash {
    let mut current_hash = leaf_hash;
    let mut current_index = leaf_index;

    for sibling in siblings {
        // Determine if current node is left or right child
        current_hash = if current_index % 2 == 0 {
            // Current is left child
            hash_pair(current_hash, *sibling)
        } else {
            // Current is right child
            hash_pair(*sibling, current_hash)
        };

        current_index /= 2;
    }

    current_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_creation() {
        let tree = MerkleTree::new(10);
        assert_eq!(tree.len(), 0);
        assert!(tree.is_empty());
        assert_eq!(*tree.root(), [0u8; 32]);
    }

    #[test]
    fn test_insert_and_proof() {
        let mut tree = MerkleTree::new(10);
        
        // Insert some nodes
        tree.insert(100).unwrap();
        tree.insert(101).unwrap();
        tree.insert(102).unwrap();
        
        assert_eq!(tree.len(), 3);
        assert!(!tree.is_empty());
        
        // Get proof for node 101
        let proof = tree.get_proof(101).expect("Node should be in tree");
        
        // Verify proof
        assert!(tree.verify_proof(101, &proof));
        
        // Proof for non-existent node should fail
        assert!(!tree.verify_proof(999, &proof));
    }

    #[test]
    fn test_remove() {
        let mut tree = MerkleTree::new(10);
        
        tree.insert(100).unwrap();
        tree.insert(101).unwrap();
        
        assert_eq!(tree.len(), 2);
        
        // Remove node 100
        let removed = tree.remove(100).unwrap();
        assert!(removed);
        assert_eq!(tree.len(), 1);
        
        // Proof for removed node should fail
        let old_proof = tree.get_proof(100);
        assert!(old_proof.is_none());
        
        // Node 101 should still have valid proof
        let proof_101 = tree.get_proof(101).unwrap();
        assert!(tree.verify_proof(101, &proof_101));
    }

    #[test]
    fn test_large_tree() {
        let mut tree = MerkleTree::new(16); // 65K capacity
        
        // Insert 1000 nodes
        for i in 0..1000 {
            tree.insert(i).unwrap();
        }
        
        assert_eq!(tree.len(), 1000);
        
        // Verify random proofs
        for node_id in [0, 500, 999] {
            let proof = tree.get_proof(node_id).unwrap();
            assert!(tree.verify_proof(node_id, &proof));
        }
    }

    #[test]
    fn test_proof_invalid_after_update() {
        let mut tree = MerkleTree::new(10);
        
        tree.insert(100).unwrap();
        let proof = tree.get_proof(100).unwrap();
        
        // Add another node (changes root)
        tree.insert(101).unwrap();
        
        // Old proof should now be invalid (wrong root)
        assert!(!tree.verify_proof(100, &proof));
        
        // Get new proof - should be valid
        let new_proof = tree.get_proof(100).unwrap();
        assert!(tree.verify_proof(100, &new_proof));
    }
}
