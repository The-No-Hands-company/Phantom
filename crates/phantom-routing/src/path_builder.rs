//! Path Construction for Anonymous Routing
//!
//! Builds multi-hop routing paths from selected peers and constructs FHE routing blobs.
//!
//! ## Overview
//!
//! The path builder takes a set of selected peers and constructs a secure routing path:
//! 1. Validate path constraints (min/max hops, diversity)
//! 2. Order nodes to maximize anonymity
//! 3. Construct FHE routing blob (encrypted routing table)
//! 4. Compute path metadata (latency, reliability)
//!
//! ## Example
//!
//! ```rust,no_run
//! use phantom_routing::{PathBuilder, PeerSelector, SelectionStrategy};
//!
//! # fn main() -> anyhow::Result<()> {
//! let mut selector = PeerSelector::new(SelectionStrategy::Weighted);
//! let peers = selector.select_peers(5, 100)?;
//!
//! let builder = PathBuilder::new()
//!     .min_hops(3)
//!     .max_hops(7)
//!     .diversity_threshold(0.8);
//!
//! let path = builder.build_path(peers)?;
//! # Ok(())
//! # }
//! ```

use crate::peer_selector::{NodeInfo, Nullifier};
use anyhow::{Result, bail};
use serde::{Serialize, Deserialize};
use std::collections::HashSet;

/// Configuration for path construction
#[derive(Clone, Debug)]
pub struct PathBuilder {
    /// Minimum number of hops (default: 3)
    min_hops: usize,
    /// Maximum number of hops (default: 7)
    max_hops: usize,
    /// Diversity threshold (0.0-1.0, default: 0.7)
    /// Prevents too many nodes from same region/operator
    diversity_threshold: f64,
    /// Require high-capacity nodes (default: false)
    require_high_capacity: bool,
}

impl PathBuilder {
    /// Create a new path builder with default settings
    pub fn new() -> Self {
        Self {
            min_hops: 3,
            max_hops: 7,
            diversity_threshold: 0.7,
            require_high_capacity: false,
        }
    }

    /// Set minimum number of hops
    pub fn min_hops(mut self, hops: usize) -> Self {
        self.min_hops = hops;
        self
    }

    /// Set maximum number of hops
    pub fn max_hops(mut self, hops: usize) -> Self {
        self.max_hops = hops;
        self
    }

    /// Set diversity threshold (0.0-1.0)
    pub fn diversity_threshold(mut self, threshold: f64) -> Self {
        self.diversity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Require all nodes to be high-capacity
    pub fn require_high_capacity(mut self, required: bool) -> Self {
        self.require_high_capacity = required;
        self
    }

    /// Build a routing path from selected peers
    ///
    /// # Arguments
    /// * `peers` - Selected peer nodes (from PeerSelector)
    ///
    /// # Returns
    /// A validated PhantomPath ready for packet construction
    pub fn build_path(&self, peers: Vec<NodeInfo>) -> Result<PhantomPath> {
        // Validate input
        if peers.len() < self.min_hops {
            bail!("Not enough peers: need {}, got {}", self.min_hops, peers.len());
        }

        // Take only max_hops nodes
        let mut path_nodes: Vec<NodeInfo> = peers.into_iter()
            .take(self.max_hops)
            .collect();

        // Validate capacity requirements
        if self.require_high_capacity {
            for node in &path_nodes {
                if node.capacity < 100_000 { // 100 KB/s
                    bail!("Node {:?} does not meet high-capacity requirement", node.nullifier);
                }
            }
        }

        // Check diversity
        let diversity_score = self.compute_diversity(&path_nodes);
        if diversity_score < self.diversity_threshold {
            bail!("Path diversity too low: {:.2} < {:.2}", 
                  diversity_score, self.diversity_threshold);
        }

        // Shuffle for unpredictability (in production, use secure randomness)
        // For now, just reverse to show reordering
        path_nodes.reverse();

        // Compute path metadata
        let estimated_latency_ms = self.estimate_latency(&path_nodes);
        let reliability_score = self.compute_reliability(&path_nodes);

        // Build the path
        let path = PhantomPath {
            nodes: path_nodes.clone(),
            hop_count: path_nodes.len(),
            estimated_latency_ms,
            reliability_score,
            diversity_score,
        };

        Ok(path)
    }

    /// Compute diversity score for a set of nodes
    ///
    /// Diversity is based on:
    /// - Geographic distribution (future)
    /// - Operator diversity (future)
    /// - Network topology (future)
    ///
    /// For now, simplified: unique nullifiers / total nodes
    fn compute_diversity(&self, nodes: &[NodeInfo]) -> f64 {
        if nodes.is_empty() {
            return 0.0;
        }

        let unique_nodes: HashSet<_> = nodes.iter()
            .map(|n| n.nullifier)
            .collect();

        unique_nodes.len() as f64 / nodes.len() as f64
    }

    /// Estimate total path latency
    fn estimate_latency(&self, nodes: &[NodeInfo]) -> u64 {
        // Base latency per hop: 100ms
        // Add node processing time estimates
        let base_latency = nodes.len() as u64 * 100;
        
        // Add bandwidth-based estimates (lower bandwidth = higher latency)
        let bandwidth_penalty: u64 = nodes.iter()
            .map(|n| {
                if n.capacity < 10_000 { // <10 KB/s
                    50 // +50ms for slow nodes
                } else {
                    0
                }
            })
            .sum();

        base_latency + bandwidth_penalty
    }

    /// Compute overall path reliability
    fn compute_reliability(&self, nodes: &[NodeInfo]) -> f64 {
        if nodes.is_empty() {
            return 0.0;
        }

        // Multiply individual reliabilities (path is only as strong as weakest link)
        nodes.iter()
            .map(|n| n.reliability)
            .product()
    }
}

impl Default for PathBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// A constructed routing path ready for packet construction
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhantomPath {
    /// Ordered list of nodes in the path
    pub nodes: Vec<NodeInfo>,
    /// Number of hops
    pub hop_count: usize,
    /// Estimated round-trip latency in milliseconds
    pub estimated_latency_ms: u64,
    /// Overall reliability score (0.0-1.0)
    pub reliability_score: f64,
    /// Diversity score (0.0-1.0)
    pub diversity_score: f64,
}

impl PhantomPath {
    /// Get the entry node (first hop)
    pub fn entry_node(&self) -> Option<&NodeInfo> {
        self.nodes.first()
    }

    /// Get the exit node (last hop)
    pub fn exit_node(&self) -> Option<&NodeInfo> {
        self.nodes.last()
    }

    /// Get intermediate nodes (all except first and last)
    pub fn intermediate_nodes(&self) -> &[NodeInfo] {
        if self.nodes.len() <= 2 {
            &[]
        } else {
            &self.nodes[1..self.nodes.len()-1]
        }
    }

    /// Get all node nullifiers in path order
    pub fn nullifiers(&self) -> Vec<Nullifier> {
        self.nodes.iter()
            .map(|n| n.nullifier)
            .collect()
    }

    /// Check if path contains a specific node
    pub fn contains_node(&self, nullifier: &Nullifier) -> bool {
        self.nodes.iter().any(|n| &n.nullifier == nullifier)
    }

    /// Construct FHE routing blob for this path
    ///
    /// The routing blob is an encrypted representation of the path that allows
    /// oblivious routing: each node can check if it's in the path without learning
    /// the full path structure.
    ///
    /// Format: [encrypted_table_entries]
    /// Each entry: (node_id, next_hop, hop_index)
    ///
    /// In production, this uses TFHE-rs FHE operations.
    /// For now, we create a placeholder structure.
    pub fn construct_routing_blob(&self) -> Result<Vec<u8>> {
        // Placeholder: In production, this would:
        // 1. Generate FHE keys (or use cached keys)
        // 2. Encrypt each routing table entry
        // 3. Serialize the encrypted table
        
        // For now, create a simple serialized structure
        let blob_size = self.nodes.len() * 96; // 96 bytes per entry (32 * 3 fields)
        let mut blob = Vec::with_capacity(blob_size);

        // Add path metadata (unencrypted for now)
        blob.extend_from_slice(&(self.hop_count as u32).to_le_bytes());
        
        // Add routing entries (would be FHE-encrypted in production)
        for (idx, node) in self.nodes.iter().enumerate() {
            blob.extend_from_slice(&node.nullifier);
            blob.extend_from_slice(&(idx as u32).to_le_bytes());
            
            // Next hop (or zeros for exit node)
            if idx + 1 < self.nodes.len() {
                blob.extend_from_slice(&self.nodes[idx + 1].nullifier);
            } else {
                blob.extend_from_slice(&[0u8; 32]); // Exit marker
            }
        }

        Ok(blob)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_selector::NodeCapabilities;

    fn create_test_node(id: u8, bandwidth: f64, reliability: f64) -> NodeInfo {
        let mut nullifier = [0u8; 32];
        nullifier[0] = id;

        NodeInfo {
            nullifier,
            last_seen_epoch: 100,
            capabilities: NodeCapabilities {
                fhe_routing: true,
                zkvm_verification: true,
                max_packet_size: 1024,
            },
            reliability: reliability,
            capacity: (bandwidth * 1000.0) as u64, // Convert to bytes/sec
            selection_count: 0,
        }
    }

    #[test]
    fn test_path_builder_creation() {
        let builder = PathBuilder::new();
        assert_eq!(builder.min_hops, 3);
        assert_eq!(builder.max_hops, 7);
        assert_eq!(builder.diversity_threshold, 0.7);
    }

    #[test]
    fn test_build_simple_path() {
        let builder = PathBuilder::new()
            .min_hops(3)
            .max_hops(5);

        let peers = vec![
            create_test_node(1, 100.0, 0.95),
            create_test_node(2, 100.0, 0.90),
            create_test_node(3, 100.0, 0.85),
            create_test_node(4, 100.0, 0.80),
        ];

        let path = builder.build_path(peers).unwrap();
        assert_eq!(path.hop_count, 4);
        assert!(path.reliability_score > 0.5);
        assert_eq!(path.diversity_score, 1.0); // All unique nodes
    }

    #[test]
    fn test_path_too_short() {
        let builder = PathBuilder::new().min_hops(5);

        let peers = vec![
            create_test_node(1, 100.0, 0.95),
            create_test_node(2, 100.0, 0.90),
        ];

        let result = builder.build_path(peers);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Not enough peers"));
    }

    #[test]
    fn test_high_capacity_requirement() {
        let builder = PathBuilder::new()
            .min_hops(2)
            .require_high_capacity(true);

        let peers = vec![
            create_test_node(1, 50.0, 0.95),  // Low bandwidth
            create_test_node(2, 100.0, 0.90),
        ];

        let result = builder.build_path(peers);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("high-capacity"));
    }

    #[test]
    fn test_path_metadata() {
        let builder = PathBuilder::new().min_hops(3);

        let peers = vec![
            create_test_node(1, 100.0, 0.9),
            create_test_node(2, 5.0, 0.8),   // Slow node
            create_test_node(3, 100.0, 0.7),
        ];

        let path = builder.build_path(peers).unwrap();
        
        // Should have latency penalty for slow node
        assert!(path.estimated_latency_ms > 300); // 3 hops * 100ms base + penalty
        
        // Reliability should be product of individual scores
        let expected_reliability = 0.9 * 0.8 * 0.7;
        assert!((path.reliability_score - expected_reliability).abs() < 0.01);
    }

    #[test]
    fn test_path_navigation() {
        let builder = PathBuilder::new().min_hops(3);

        let peers = vec![
            create_test_node(1, 100.0, 0.9),
            create_test_node(2, 100.0, 0.8),
            create_test_node(3, 100.0, 0.7),
            create_test_node(4, 100.0, 0.6),
        ];

        let path = builder.build_path(peers).unwrap();

        assert!(path.entry_node().is_some());
        assert!(path.exit_node().is_some());
        assert_eq!(path.intermediate_nodes().len(), 2);
        assert_eq!(path.nullifiers().len(), 4);
    }

    #[test]
    fn test_contains_node() {
        let builder = PathBuilder::new().min_hops(2);

        let peers = vec![
            create_test_node(1, 100.0, 0.9),
            create_test_node(2, 100.0, 0.8),
        ];

        let path = builder.build_path(peers).unwrap();

        let mut test_nullifier = [0u8; 32];
        test_nullifier[0] = 1;
        assert!(path.contains_node(&test_nullifier));

        test_nullifier[0] = 99;
        assert!(!path.contains_node(&test_nullifier));
    }

    #[test]
    fn test_routing_blob_construction() {
        let builder = PathBuilder::new().min_hops(3);

        let peers = vec![
            create_test_node(1, 100.0, 0.9),
            create_test_node(2, 100.0, 0.8),
            create_test_node(3, 100.0, 0.7),
        ];

        let path = builder.build_path(peers).unwrap();
        let blob = path.construct_routing_blob().unwrap();

        // Should have reasonable size
        assert!(!blob.is_empty());
        
        // Should encode path length (first 4 bytes)
        let hop_count = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]);
        assert_eq!(hop_count, 3);
    }

    #[test]
    fn test_diversity_computation() {
        let builder = PathBuilder::new();

        // All unique nodes
        let diverse_peers = vec![
            create_test_node(1, 100.0, 0.9),
            create_test_node(2, 100.0, 0.8),
            create_test_node(3, 100.0, 0.7),
        ];
        let diversity = builder.compute_diversity(&diverse_peers);
        assert_eq!(diversity, 1.0);

        // Duplicate nodes (simulated by same ID)
        let duplicate_peers = vec![
            create_test_node(1, 100.0, 0.9),
            create_test_node(1, 100.0, 0.8), // Same ID
            create_test_node(2, 100.0, 0.7),
        ];
        let diversity = builder.compute_diversity(&duplicate_peers);
        assert!(diversity < 1.0);
    }

    #[test]
    fn test_max_hops_enforced() {
        let builder = PathBuilder::new()
            .min_hops(2)
            .max_hops(3);

        let peers = vec![
            create_test_node(1, 100.0, 0.9),
            create_test_node(2, 100.0, 0.8),
            create_test_node(3, 100.0, 0.7),
            create_test_node(4, 100.0, 0.6),
            create_test_node(5, 100.0, 0.5),
        ];

        let path = builder.build_path(peers).unwrap();
        assert_eq!(path.hop_count, 3); // Should truncate to max_hops
    }
}
