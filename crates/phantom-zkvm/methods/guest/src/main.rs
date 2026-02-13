// Copyright 2025 PHANTOM Protocol
// Guest program for validating routing paths in zero-knowledge
//
// This program runs inside the RISC Zero zkVM and proves:
// 1. Path is valid length (3-7 hops)
// 2. Path has no loops (all nodes unique)
// 3. All nodes exist in network (Merkle membership)
// 4. Path matches network commitment

#![no_main]

use risc0_zkvm::guest::env;
use serde::{Deserialize, Serialize};

/// Public inputs to the circuit
#[derive(Serialize, Deserialize)]
struct PublicInputs {
    /// Merkle root of network topology
    network_commitment: [u8; 32],
    /// Number of hops in path
    path_length: usize,
    /// Proof timestamp (for freshness)
    timestamp: u64,
}

/// Private witness (the actual path)
#[derive(Serialize, Deserialize)]
struct RoutingPath {
    /// List of node IDs in the path
    hops: Vec<u32>,
}

/// Merkle proof for a single node
#[derive(Serialize, Deserialize)]
struct MerkleProof {
    leaf_index: u64,
    siblings: Vec<[u8; 32]>,
}

risc0_zkvm::guest::entry!(main);

fn main() {
    // Read public inputs
    let public_inputs: PublicInputs = env::read();
    
    // Read private witness (the actual routing path)
    let path: RoutingPath = env::read();
    
    // Read Merkle proofs for each node in path
    let merkle_proofs: Vec<MerkleProof> = env::read();
    
    // ============================================================
    // CONSTRAINT 1: Path length must be 3-7 hops for anonymity
    // ============================================================
    assert!(
        path.hops.len() >= 3,
        "Path too short: {} hops (minimum 3 required)",
        path.hops.len()
    );
    assert!(
        path.hops.len() <= 7,
        "Path too long: {} hops (maximum 7 allowed)",
        path.hops.len()
    );
    assert_eq!(
        path.hops.len(),
        public_inputs.path_length,
        "Path length mismatch"
    );
    
    // ============================================================
    // CONSTRAINT 2: No loops - all nodes must be unique
    // ============================================================
    for i in 0..path.hops.len() {
        for j in (i + 1)..path.hops.len() {
            assert!(
                path.hops[i] != path.hops[j],
                "Loop detected: node {} appears at positions {} and {}",
                path.hops[i],
                i,
                j
            );
        }
    }
    
    // ============================================================
    // CONSTRAINT 3: All nodes exist in network (Merkle membership)
    // ============================================================
    assert_eq!(
        path.hops.len(),
        merkle_proofs.len(),
        "Must provide Merkle proof for each hop"
    );
    
    for (i, (node_id, proof)) in path.hops.iter().zip(merkle_proofs.iter()).enumerate() {
        let valid = verify_merkle_proof(
            *node_id,
            proof,
            &public_inputs.network_commitment,
        );
        assert!(
            valid,
            "Node {} at position {} failed Merkle verification",
            node_id,
            i
        );
    }
    
    // ============================================================
    // CONSTRAINT 4: Timestamp freshness (prevent replay)
    // ============================================================
    // In production, would check timestamp is within acceptable window
    // For now, just ensure it's non-zero
    assert!(public_inputs.timestamp > 0, "Invalid timestamp");
    
    // ============================================================
    // Commit public outputs
    // ============================================================
    // The zkVM proof proves we satisfied all constraints
    // Commit the path length and timestamp as public outputs
    env::commit(&public_inputs.path_length);
    env::commit(&public_inputs.timestamp);
    env::commit(&true); // Proof succeeded
}

/// Verify a Merkle membership proof
///
/// Proves that a node with given ID is in the tree with given root
fn verify_merkle_proof(
    node_id: u32,
    proof: &MerkleProof,
    expected_root: &[u8; 32],
) -> bool {
    // Hash the node ID to get leaf value
    let leaf_hash = hash_node_id(node_id);
    
    // Recompute root from proof
    let computed_root = compute_root_from_proof(
        leaf_hash,
        proof.leaf_index,
        &proof.siblings,
    );
    
    // Check if computed root matches expected root
    computed_root == *expected_root
}

/// Hash a node ID to a leaf value (matches phantom-core implementation)
fn hash_node_id(node_id: u32) -> [u8; 32] {
    let bytes = node_id.to_le_bytes();
    *blake3::hash(&bytes).as_bytes()
}

/// Hash a pair of values for Merkle tree internal nodes
fn hash_pair(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut combined = [0u8; 64];
    combined[..32].copy_from_slice(&left);
    combined[32..].copy_from_slice(&right);
    *blake3::hash(&combined).as_bytes()
}

/// Compute Merkle root from a proof path
fn compute_root_from_proof(
    mut current_hash: [u8; 32],
    mut current_index: u64,
    siblings: &[[u8; 32]],
) -> [u8; 32] {
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
