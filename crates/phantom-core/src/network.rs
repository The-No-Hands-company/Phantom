//! Network graph and topology management

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::merkle::MerkleTree;

pub type NodeId = u32;

/// Network graph representing the PHANTOM overlay network
#[derive(Clone, Debug)]
pub struct NetworkGraph {
    /// All nodes in the network
    nodes: HashMap<NodeId, NodeInfo>,
    
    /// Adjacency list: node -> set of neighbors
    edges: HashMap<NodeId, HashSet<NodeId>>,
    
    /// Merkle tree commitment of the network topology
    /// Used in zk-proofs to prove a node is in the network
    merkle_tree: MerkleTree,
}

/// Information about a node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: NodeId,
    pub bandwidth: u64,      // Advertised bandwidth in bytes/sec
    pub latency_ms: u32,     // Average latency in milliseconds
    pub uptime_hours: u32,   // Uptime in hours
    pub reputation: f64,     // Reputation score (0.0 to 1.0)
}

impl NetworkGraph {
    /// Create a new empty network graph
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            merkle_tree: MerkleTree::new(20), // 1M node capacity
        }
    }
    
    /// Add a node to the network
    pub fn add_node(&mut self, info: NodeInfo) {
        let id = info.id;
        self.nodes.insert(id, info);
        self.edges.entry(id).or_insert_with(HashSet::new);
        
        // Add node to Merkle tree
        self.merkle_tree.insert(id).expect("Merkle tree should not be full");
    }
    
    /// Add an edge between two nodes
    pub fn add_edge(&mut self, from: NodeId, to: NodeId) {
        self.edges.entry(from).or_default().insert(to);
        self.edges.entry(to).or_default().insert(from); // Bidirectional
        // Note: Edges don't change the Merkle tree (only nodes do)
    }
    
    /// Check if a node exists in the network
    pub fn contains_node(&self, id: NodeId) -> bool {
        self.nodes.contains_key(&id)
    }
    
    /// Check if two nodes are connected
    pub fn are_connected(&self, from: NodeId, to: NodeId) -> bool {
        self.edges.get(&from)
            .map(|neighbors| neighbors.contains(&to))
            .unwrap_or(false)
    }
    
    /// Get neighbors of a node
    pub fn get_neighbors(&self, id: NodeId) -> Option<Vec<NodeId>> {
        self.edges.get(&id)
            .map(|set| set.iter().copied().collect())
    }
    
    /// Get the Merkle root commitment of the network
    /// Number of nodes currently in the graph.
    ///
    /// `nodes` is private, so callers that only need the size had no way to
    /// ask without exposing the whole map.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Number of undirected edges in the graph.
    ///
    /// Adjacency is stored in both directions, so the raw entry count is
    /// double the number of edges: this halves it, matching
    /// [`NetworkStats::edge_count`]. A 10-node ring has 10 edges and a
    /// complete graph on 10 nodes has 45, which are the answers graph theory
    /// gives and the answers `stats()` already gave.
    ///
    /// Having this disagree with `stats().edge_count` would be worse than not
    /// having it — two methods with the same name and different answers is a
    /// bug waiting for whoever calls the wrong one.
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|peers| peers.len()).sum::<usize>() / 2
    }

    /// Every node id in the graph, ascending.
    ///
    /// Sorted rather than in HashMap order: callers that build a Merkle tree
    /// from this need the leaf order to be reproducible, or two nodes with
    /// identical membership compute different roots and never agree.
    pub fn node_ids(&self) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.nodes.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    /// A single path from `source` to `dest`, or None if they are not
    /// connected. Convenience over [`Self::k_shortest_paths`].
    pub fn find_path(&self, source: NodeId, dest: NodeId) -> Option<Vec<NodeId>> {
        self.k_shortest_paths(source, dest, 1).into_iter().next()
    }

    pub fn commitment(&self) -> &[u8; 32] {
        self.merkle_tree.root()
    }
    
    /// Get a Merkle proof that a node is in the network
    pub fn get_membership_proof(&self, node_id: NodeId) -> Option<crate::merkle::MerkleProof> {
        self.merkle_tree.get_proof(node_id)
    }
    
    /// Verify a node membership proof
    pub fn verify_membership(&self, node_id: NodeId, proof: &crate::merkle::MerkleProof) -> bool {
        self.merkle_tree.verify_proof(node_id, proof)
    }
    
    /// Find k-shortest paths between two nodes
    /// Returns up to k paths, each as a vector of node IDs
    pub fn k_shortest_paths(
        &self,
        source: NodeId,
        dest: NodeId,
        _k: usize,
    ) -> Vec<Vec<NodeId>> {
        // TODO: Implement Yen's k-shortest paths algorithm
        // For now, return a simple direct path if connected
        
        if source == dest {
            return vec![];
        }
        
        // Placeholder: just return direct connection if exists
        if self.are_connected(source, dest) {
            vec![vec![source, dest]]
        } else {
            vec![]
        }
    }
    
    /// Get network statistics
    pub fn stats(&self) -> NetworkStats {
        NetworkStats {
            node_count: self.nodes.len(),
            edge_count: self.edges.values().map(|set| set.len()).sum::<usize>() / 2,
            avg_degree: if self.nodes.is_empty() {
                0.0
            } else {
                self.edges.values().map(|set| set.len()).sum::<usize>() as f64 
                    / self.nodes.len() as f64
            },
        }
    }
}

impl Default for NetworkGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub node_count: usize,
    pub edge_count: usize,
    pub avg_degree: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_network_creation() {
        let mut network = NetworkGraph::new();
        
        network.add_node(NodeInfo {
            id: 1,
            bandwidth: 1_000_000,
            latency_ms: 10,
            uptime_hours: 100,
            reputation: 0.9,
        });
        
        network.add_node(NodeInfo {
            id: 2,
            bandwidth: 500_000,
            latency_ms: 20,
            uptime_hours: 50,
            reputation: 0.8,
        });
        
        assert!(network.contains_node(1));
        assert!(network.contains_node(2));
        assert!(!network.contains_node(3));
    }
    
    #[test]
    fn test_edges() {
        let mut network = NetworkGraph::new();
        
        network.add_node(NodeInfo {
            id: 1,
            bandwidth: 1_000_000,
            latency_ms: 10,
            uptime_hours: 100,
            reputation: 0.9,
        });
        
        network.add_node(NodeInfo {
            id: 2,
            bandwidth: 500_000,
            latency_ms: 20,
            uptime_hours: 50,
            reputation: 0.8,
        });
        
        network.add_edge(1, 2);
        
        assert!(network.are_connected(1, 2));
        assert!(network.are_connected(2, 1)); // Bidirectional
        
        let neighbors = network.get_neighbors(1);
        assert_eq!(neighbors, Some(vec![2]));
    }
    
    #[test]
    fn test_commitment_changes() {
        let mut network = NetworkGraph::new();
        let commitment1 = *network.commitment();
        
        network.add_node(NodeInfo {
            id: 1,
            bandwidth: 1_000_000,
            latency_ms: 10,
            uptime_hours: 100,
            reputation: 0.9,
        });
        
        let commitment2 = *network.commitment();
        assert_ne!(commitment1, commitment2);
    }
}
