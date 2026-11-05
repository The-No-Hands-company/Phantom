//! Network graph and topology management

use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::cmp::Reverse;
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

/// Wrapper so `BinaryHeap<Reverse<PathByLen>>` is ordered by hop count.
#[derive(Eq, PartialEq)]
struct PathByLen(Vec<NodeId>);

impl Ord for PathByLen {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.len().cmp(&other.0.len())
            .then_with(|| self.0.cmp(&other.0))
    }
}
impl PartialOrd for PathByLen {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
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
    
    /// Find up to k shortest (fewest-hop) paths from `source` to `dest`.
    ///
    /// Uses Yen's k-shortest simple-paths algorithm. All edges are unit weight
    /// (the graph has no latency or cost annotations at this layer), so BFS
    /// yields optimal paths and Yen's loop finds k diverse alternatives.
    ///
    /// Returns fewer than k paths when the graph is too sparse.
    pub fn k_shortest_paths(
        &self,
        source: NodeId,
        dest: NodeId,
        k: usize,
    ) -> Vec<Vec<NodeId>> {
        if source == dest || k == 0 {
            return vec![];
        }

        // ── Seed: shortest path with unrestricted BFS ──
        let first = match self.bfs_path(source, dest, &HashSet::new(), &HashSet::new()) {
            Some(p) => p,
            None => return vec![],
        };

        let mut result: Vec<Vec<NodeId>> = vec![first];
        // Min-heap ordered by (path_length, path_lex) so the shortest candidate
        // is always next.
        let mut candidates: BinaryHeap<Reverse<PathByLen>> = BinaryHeap::new();

        for i in 1..k {
            let prev = result[i - 1].clone();

            for spur_idx in 0..prev.len().saturating_sub(1) {
                let spur_node = prev[spur_idx];
                let root_path = &prev[..=spur_idx];

                // Remove edges that any already-found path shares with root_path
                // at the spur position, so each new path diverges here.
                let mut removed_edges: HashSet<(NodeId, NodeId)> = HashSet::new();
                for p in &result {
                    if p.len() > spur_idx && p[..=spur_idx] == *root_path {
                        if let (Some(&u), Some(&v)) = (p.get(spur_idx), p.get(spur_idx + 1)) {
                            removed_edges.insert((u, v));
                            removed_edges.insert((v, u));
                        }
                    }
                }

                // Remove root nodes (except spur itself) to keep path simple.
                let removed_nodes: HashSet<NodeId> =
                    root_path[..spur_idx].iter().copied().collect();

                if let Some(spur) =
                    self.bfs_path(spur_node, dest, &removed_edges, &removed_nodes)
                {
                    let mut total: Vec<NodeId> = root_path[..spur_idx].to_vec();
                    total.extend_from_slice(&spur);
                    if !result.contains(&total) {
                        candidates.push(Reverse(PathByLen(total)));
                    }
                }
            }

            match candidates.pop() {
                Some(Reverse(PathByLen(path))) => result.push(path),
                None => break,
            }
        }

        result
    }

    /// BFS shortest path respecting removed edges and nodes.
    fn bfs_path(
        &self,
        source: NodeId,
        dest: NodeId,
        removed_edges: &HashSet<(NodeId, NodeId)>,
        removed_nodes: &HashSet<NodeId>,
    ) -> Option<Vec<NodeId>> {
        let mut queue: VecDeque<NodeId> = VecDeque::new();
        let mut parent: HashMap<NodeId, NodeId> = HashMap::new();

        queue.push_back(source);
        parent.insert(source, source); // sentinel: source is its own parent

        while let Some(node) = queue.pop_front() {
            if node == dest {
                // Reconstruct path
                let mut path = vec![dest];
                let mut cur = dest;
                loop {
                    let p = parent[&cur];
                    if p == cur { break; } // back at source sentinel
                    path.push(p);
                    cur = p;
                }
                path.reverse();
                return Some(path);
            }
            if let Some(neighbors) = self.edges.get(&node) {
                let mut sorted: Vec<NodeId> = neighbors.iter().copied().collect();
                sorted.sort_unstable(); // deterministic traversal
                for next in sorted {
                    if !parent.contains_key(&next)
                        && !removed_nodes.contains(&next)
                        && !removed_edges.contains(&(node, next))
                    {
                        parent.insert(next, node);
                        queue.push_back(next);
                    }
                }
            }
        }
        None
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

    fn diamond_graph() -> NetworkGraph {
        // Diamond: A — B — D
        //                \—C—/
        let mut g = NetworkGraph::new();
        let mk = |id| NodeInfo { id, bandwidth: 1_000_000, latency_ms: 10, uptime_hours: 0, reputation: 1.0 };
        for id in [1u32, 2, 3, 4] { g.add_node(mk(id)); }
        // A=1, B=2, C=3, D=4
        g.add_edge(1, 2); // A-B
        g.add_edge(1, 3); // A-C
        g.add_edge(2, 4); // B-D
        g.add_edge(3, 4); // C-D
        g
    }

    #[test]
    fn test_k_shortest_paths_direct() {
        let mut g = NetworkGraph::new();
        let mk = |id| NodeInfo { id, bandwidth: 1_000_000, latency_ms: 10, uptime_hours: 0, reputation: 1.0 };
        for id in [1u32, 2, 3] { g.add_node(mk(id)); }
        g.add_edge(1, 2);
        g.add_edge(2, 3);

        let paths = g.k_shortest_paths(1, 3, 1);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0], vec![1, 2, 3]);
    }

    #[test]
    fn test_k_shortest_paths_diamond_two_paths() {
        let g = diamond_graph();
        // A→D has two equal-length paths: A-B-D and A-C-D
        let paths = g.k_shortest_paths(1, 4, 2);
        assert_eq!(paths.len(), 2);
        // Both paths must be length 3
        for p in &paths { assert_eq!(p.len(), 3); }
        // Both start at 1, end at 4
        for p in &paths {
            assert_eq!(*p.first().unwrap(), 1);
            assert_eq!(*p.last().unwrap(), 4);
        }
        // The two paths must be distinct
        assert_ne!(paths[0], paths[1]);
    }

    #[test]
    fn test_k_shortest_paths_no_route() {
        let g = diamond_graph();
        // Node 99 doesn't exist
        let paths = g.k_shortest_paths(1, 99, 3);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_k_shortest_paths_same_source_dest() {
        let g = diamond_graph();
        let paths = g.k_shortest_paths(1, 1, 3);
        assert!(paths.is_empty());
    }

    #[test]
    fn test_k_less_than_available() {
        // Ask for 1 path when 2 exist — must return exactly 1
        let g = diamond_graph();
        let paths = g.k_shortest_paths(1, 4, 1);
        assert_eq!(paths.len(), 1);
    }
}
