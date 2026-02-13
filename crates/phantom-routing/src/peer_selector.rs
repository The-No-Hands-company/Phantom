//! Peer Selection System for Anonymous Routing
//!
//! Selects routing peers from discovered nodes using various strategies.
//!
//! ## Overview
//!
//! The peer selector chooses which nodes to use for routing paths based on:
//! - Reliability (uptime, successful forwards)
//! - Capacity (bandwidth, current load)
//! - Diversity (avoid centralization)
//! - Freshness (recent announcements)
//!
//! ## Example
//!
//! ```rust,no_run
//! use phantom_routing::{PeerSelector, SelectionStrategy, NodeInfo};
//!
//! # fn main() -> anyhow::Result<()> {
//! let mut selector = PeerSelector::new(SelectionStrategy::Weighted);
//!
//! // Add discovered nodes
//! # let node_info = unimplemented!();
//! selector.add_node(node_info);
//!
//! // Select peers for routing
//! let peers = selector.select_peers(5, 100)?; // Select 5 peers at epoch 100
//! # Ok(())
//! # }
//! ```

use serde::{Serialize, Deserialize};
use anyhow::{Result, bail};
use std::collections::HashMap;

/// Node nullifier (unique identifier)
pub type Nullifier = [u8; 32];

/// Information about a discovered peer node
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Unique nullifier from announcement
    pub nullifier: Nullifier,
    
    /// Last epoch this node was seen
    pub last_seen_epoch: u64,
    
    /// Reliability score (0.0 - 1.0)
    /// Based on uptime and successful packet forwards
    pub reliability: f64,
    
    /// Estimated capacity (bytes/sec)
    pub capacity: u64,
    
    /// Number of times selected for routing
    pub selection_count: u64,
    
    /// Node's announced capabilities
    pub capabilities: NodeCapabilities,
}

impl NodeInfo {
    /// Create new node info from announcement
    pub fn new(nullifier: Nullifier, epoch: u64) -> Self {
        Self {
            nullifier,
            last_seen_epoch: epoch,
            reliability: 1.0, // Start with perfect score
            capacity: 1_000_000, // 1 MB/s default
            selection_count: 0,
            capabilities: NodeCapabilities::default(),
        }
    }
    
    /// Check if node is considered active
    pub fn is_active(&self, current_epoch: u64, max_age: u64) -> bool {
        current_epoch.saturating_sub(self.last_seen_epoch) < max_age
    }
    
    /// Update reliability based on performance
    pub fn update_reliability(&mut self, success: bool) {
        const ALPHA: f64 = 0.1; // Learning rate
        let reward = if success { 1.0 } else { 0.0 };
        self.reliability = (1.0 - ALPHA) * self.reliability + ALPHA * reward;
    }
    
    /// Record that this node was selected
    pub fn record_selection(&mut self) {
        self.selection_count += 1;
    }
}

/// Node capabilities
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Supports FHE routing
    pub fhe_routing: bool,
    
    /// Supports zkVM proof verification
    pub zkvm_verification: bool,
    
    /// Maximum packet size (bytes)
    pub max_packet_size: usize,
}

impl Default for NodeCapabilities {
    fn default() -> Self {
        Self {
            fhe_routing: true,
            zkvm_verification: true,
            max_packet_size: 65536, // 64 KB
        }
    }
}

/// Strategy for selecting peers
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// Pure random selection
    Random,
    
    /// Weighted by reliability
    Weighted,
    
    /// Prioritize high-capacity nodes
    HighCapacity,
    
    /// Balance load across all nodes
    LoadBalanced,
}

/// Peer selector - chooses routing peers from discovered nodes
pub struct PeerSelector {
    /// All known nodes
    nodes: HashMap<Nullifier, NodeInfo>,
    
    /// Selection strategy
    strategy: SelectionStrategy,
    
    /// Maximum age for considering nodes active (epochs)
    max_node_age: u64,
}

impl PeerSelector {
    /// Create new peer selector
    pub fn new(strategy: SelectionStrategy) -> Self {
        Self {
            nodes: HashMap::new(),
            strategy,
            max_node_age: 3, // 30 minutes default
        }
    }
    
    /// Add or update a discovered node
    pub fn add_node(&mut self, node: NodeInfo) {
        self.nodes.insert(node.nullifier, node);
    }
    
    /// Remove a node
    pub fn remove_node(&mut self, nullifier: &Nullifier) {
        self.nodes.remove(nullifier);
    }
    
    /// Get node info
    pub fn get_node(&self, nullifier: &Nullifier) -> Option<&NodeInfo> {
        self.nodes.get(nullifier)
    }
    
    /// Get mutable node info
    pub fn get_node_mut(&mut self, nullifier: &Nullifier) -> Option<&mut NodeInfo> {
        self.nodes.get_mut(nullifier)
    }
    
    /// Select peers for routing
    ///
    /// **Parameters**:
    /// - `count`: Number of peers to select
    /// - `current_epoch`: Current epoch for freshness check
    ///
    /// **Returns**: Vec of selected NodeInfo (may be less than count if not enough active nodes)
    pub fn select_peers(&mut self, count: usize, current_epoch: u64) -> Result<Vec<NodeInfo>> {
        if count == 0 {
            bail!("Must select at least 1 peer");
        }
        
        // Get active nodes
        let active_nodes: Vec<&NodeInfo> = self.nodes.values()
            .filter(|n| n.is_active(current_epoch, self.max_node_age))
            .collect();
        
        if active_nodes.is_empty() {
            bail!("No active nodes available");
        }
        
        // Select based on strategy
        let selected = match self.strategy {
            SelectionStrategy::Random => self.select_random(&active_nodes, count),
            SelectionStrategy::Weighted => self.select_weighted(&active_nodes, count),
            SelectionStrategy::HighCapacity => self.select_high_capacity(&active_nodes, count),
            SelectionStrategy::LoadBalanced => self.select_load_balanced(&active_nodes, count),
        };
        
        // Record selections
        for node in &selected {
            if let Some(n) = self.nodes.get_mut(&node.nullifier) {
                n.record_selection();
            }
        }
        
        Ok(selected)
    }
    
    /// Random selection
    fn select_random(&self, nodes: &[&NodeInfo], count: usize) -> Vec<NodeInfo> {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        
        nodes.choose_multiple(&mut rng, count)
            .map(|n| (*n).clone())
            .collect()
    }
    
    /// Weighted selection by reliability
    fn select_weighted(&self, nodes: &[&NodeInfo], count: usize) -> Vec<NodeInfo> {
        use rand::distributions::WeightedIndex;
        use rand::prelude::*;
        
        let weights: Vec<f64> = nodes.iter().map(|n| n.reliability).collect();
        let dist = WeightedIndex::new(&weights).expect("Failed to create weighted distribution");
        
        let mut rng = rand::thread_rng();
        let mut selected = Vec::new();
        let mut used_indices = std::collections::HashSet::new();
        
        for _ in 0..count.min(nodes.len()) {
            // Find unused index
            let mut idx = dist.sample(&mut rng);
            while used_indices.contains(&idx) && used_indices.len() < nodes.len() {
                idx = dist.sample(&mut rng);
            }
            
            if !used_indices.contains(&idx) {
                used_indices.insert(idx);
                selected.push(nodes[idx].clone());
            }
        }
        
        selected
    }
    
    /// Select highest capacity nodes
    fn select_high_capacity(&self, nodes: &[&NodeInfo], count: usize) -> Vec<NodeInfo> {
        let mut sorted: Vec<&NodeInfo> = nodes.iter().copied().collect();
        sorted.sort_by(|a, b| b.capacity.cmp(&a.capacity));
        
        sorted.into_iter()
            .take(count)
            .map(|n| n.clone())
            .collect()
    }
    
    /// Load-balanced selection
    fn select_load_balanced(&self, nodes: &[&NodeInfo], count: usize) -> Vec<NodeInfo> {
        let mut sorted: Vec<&NodeInfo> = nodes.iter().copied().collect();
        sorted.sort_by(|a, b| a.selection_count.cmp(&b.selection_count));
        
        sorted.into_iter()
            .take(count)
            .map(|n| n.clone())
            .collect()
    }
    
    /// Get number of known nodes
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    
    /// Get number of active nodes
    pub fn active_node_count(&self, current_epoch: u64) -> usize {
        self.nodes.values()
            .filter(|n| n.is_active(current_epoch, self.max_node_age))
            .count()
    }
    
    /// Set max node age
    pub fn set_max_node_age(&mut self, max_age: u64) {
        self.max_node_age = max_age;
    }
    
    /// Evict inactive nodes
    pub fn evict_inactive(&mut self, current_epoch: u64) {
        self.nodes.retain(|_, node| node.is_active(current_epoch, self.max_node_age));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_node(id: u8, epoch: u64) -> NodeInfo {
        let mut nullifier = [0u8; 32];
        nullifier[0] = id;
        NodeInfo::new(nullifier, epoch)
    }
    
    #[test]
    fn test_node_info_creation() {
        let node = create_test_node(1, 100);
        
        assert_eq!(node.last_seen_epoch, 100);
        assert_eq!(node.reliability, 1.0);
        assert_eq!(node.selection_count, 0);
    }
    
    #[test]
    fn test_node_is_active() {
        let node = create_test_node(1, 100);
        
        assert!(node.is_active(100, 3)); // Same epoch
        assert!(node.is_active(102, 3)); // 2 epochs old
        assert!(!node.is_active(104, 3)); // 4 epochs old
    }
    
    #[test]
    fn test_reliability_update() {
        let mut node = create_test_node(1, 100);
        
        // Successful forward
        node.update_reliability(true);
        assert_eq!(node.reliability, 1.0); // Already perfect
        
        // Failed forward
        node.update_reliability(false);
        assert!(node.reliability < 1.0);
        assert!(node.reliability > 0.8);
    }
    
    #[test]
    fn test_peer_selector_random() {
        let mut selector = PeerSelector::new(SelectionStrategy::Random);
        
        // Add 10 nodes
        for i in 0..10 {
            selector.add_node(create_test_node(i, 100));
        }
        
        let selected = selector.select_peers(5, 100).unwrap();
        assert_eq!(selected.len(), 5);
        
        // Check uniqueness
        let nullifiers: std::collections::HashSet<_> = selected.iter()
            .map(|n| n.nullifier)
            .collect();
        assert_eq!(nullifiers.len(), 5);
    }
    
    #[test]
    fn test_peer_selector_weighted() {
        let mut selector = PeerSelector::new(SelectionStrategy::Weighted);
        
        // Add nodes with different reliability
        for i in 0..10 {
            let mut node = create_test_node(i, 100);
            node.reliability = (i as f64 + 1.0) / 10.0; // 0.1 to 1.0
            selector.add_node(node);
        }
        
        let selected = selector.select_peers(5, 100).unwrap();
        assert_eq!(selected.len(), 5);
        
        // Higher reliability nodes should be selected more often
        let avg_reliability: f64 = selected.iter()
            .map(|n| n.reliability)
            .sum::<f64>() / 5.0;
        
        assert!(avg_reliability > 0.5); // Should be above average
    }
    
    #[test]
    fn test_peer_selector_load_balanced() {
        let mut selector = PeerSelector::new(SelectionStrategy::LoadBalanced);
        
        // Add nodes with different selection counts
        for i in 0..10 {
            let mut node = create_test_node(i, 100);
            node.selection_count = i as u64 * 10;
            selector.add_node(node);
        }
        
        let selected = selector.select_peers(5, 100).unwrap();
        
        // Should select least-used nodes
        for node in &selected {
            assert!(node.selection_count < 50);
        }
    }
    
    #[test]
    fn test_evict_inactive() {
        let mut selector = PeerSelector::new(SelectionStrategy::Random);
        selector.set_max_node_age(3); // 3 epochs
        
        // Add nodes at different epochs
        selector.add_node(create_test_node(1, 100));
        selector.add_node(create_test_node(2, 98));
        selector.add_node(create_test_node(3, 95));
        
        assert_eq!(selector.node_count(), 3);
        
        // Evict old nodes (epoch 100, max_age 3 = keep nodes from epoch 97+)
        // Node 1: 100 (keep) - 0 epochs old
        // Node 2: 98  (keep) - 2 epochs old
        // Node 3: 95  (evict) - 5 epochs old
        selector.evict_inactive(100);
        
        assert_eq!(selector.node_count(), 2);
        assert_eq!(selector.active_node_count(100), 2);
    }
}
