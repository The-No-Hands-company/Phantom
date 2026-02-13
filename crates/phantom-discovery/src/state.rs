//! Network State Management
//!
//! Tracks the current state of the PHANTOM network including:
//! - Merkle root (cryptographic commitment to all nodes)
//! - Epoch (time-based updates every 10 minutes)
//! - Node count
//! - Last update timestamp

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Duration of one epoch in seconds (10 minutes)
pub const EPOCH_DURATION_SECS: u64 = 600;

/// Network state shared across all nodes
///
/// This represents the consensus view of the network at a given epoch.
/// All nodes must agree on the Merkle root for membership proofs to work.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NetworkState {
    /// Merkle root - cryptographic commitment to all nodes in the network
    pub merkle_root: [u8; 32],
    
    /// Current epoch number (increments every 10 minutes)
    pub epoch: u64,
    
    /// Total number of nodes in the network
    pub node_count: usize,
    
    /// Unix timestamp of last state update
    pub last_update: u64,
}

impl NetworkState {
    /// Create a new empty network state
    pub fn new() -> Self {
        Self {
            merkle_root: [0u8; 32],
            epoch: Self::current_epoch(),
            node_count: 0,
            last_update: current_timestamp(),
        }
    }
    
    /// Calculate the current epoch number
    ///
    /// Epoch 0 starts at Unix timestamp 0, each epoch is 10 minutes
    pub fn current_epoch() -> u64 {
        current_timestamp() / EPOCH_DURATION_SECS
    }
    
    /// Check if a given epoch is within valid range
    ///
    /// Valid epochs:
    /// - current_epoch - 1 (previous epoch, for transition period)
    /// - current_epoch (current epoch)
    /// - current_epoch + 1 (future epoch, for clock skew)
    pub fn is_epoch_valid(&self, epoch: u64) -> bool {
        let current = Self::current_epoch();
        
        // Allow previous, current, or next epoch (handles clock skew)
        epoch >= current.saturating_sub(1) && epoch <= current + 1
    }
    
    /// Update the Merkle root commitment
    ///
    /// This should be called when the network topology changes
    /// (nodes join or leave)
    pub fn update_merkle_root(&mut self, new_root: [u8; 32]) {
        self.merkle_root = new_root;
        self.last_update = current_timestamp();
        
        // Advance epoch if needed
        let current_epoch = Self::current_epoch();
        if current_epoch > self.epoch {
            self.epoch = current_epoch;
        }
    }
    
    /// Update node count
    pub fn set_node_count(&mut self, count: usize) {
        self.node_count = count;
        self.last_update = current_timestamp();
    }
    
    /// Check if the state is stale (hasn't been updated in 2+ epochs)
    pub fn is_stale(&self) -> bool {
        let current_epoch = Self::current_epoch();
        current_epoch > self.epoch + 2
    }
    
    /// Get time remaining in current epoch (seconds)
    pub fn epoch_time_remaining() -> u64 {
        let current_ts = current_timestamp();
        let epoch_start = (current_ts / EPOCH_DURATION_SECS) * EPOCH_DURATION_SECS;
        let epoch_end = epoch_start + EPOCH_DURATION_SECS;
        
        epoch_end.saturating_sub(current_ts)
    }
}

impl Default for NetworkState {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current Unix timestamp in seconds
pub fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before Unix epoch")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_network_state_creation() {
        let state = NetworkState::new();
        assert_eq!(state.merkle_root, [0u8; 32]);
        assert_eq!(state.node_count, 0);
        assert!(state.epoch > 0); // We're past epoch 0 (Unix timestamp 0)
    }
    
    #[test]
    fn test_epoch_calculation() {
        let epoch = NetworkState::current_epoch();
        
        // Epoch should be reasonable (after 2024)
        // Unix timestamp 1700000000 = Nov 2023
        // At 600s per epoch, that's ~2,833,333 epochs
        assert!(epoch > 2_800_000);
    }
    
    #[test]
    fn test_epoch_validation() {
        let mut state = NetworkState::new();
        let current = NetworkState::current_epoch();
        
        state.epoch = current;
        
        // Current epoch is valid
        assert!(state.is_epoch_valid(current));
        
        // Previous epoch is valid (transition period)
        assert!(state.is_epoch_valid(current - 1));
        
        // Next epoch is valid (clock skew)
        assert!(state.is_epoch_valid(current + 1));
        
        // 2 epochs ago is invalid
        assert!(!state.is_epoch_valid(current.saturating_sub(2)));
        
        // 2 epochs in future is invalid
        assert!(!state.is_epoch_valid(current + 2));
    }
    
    #[test]
    fn test_merkle_root_update() {
        let mut state = NetworkState::new();
        let initial_update = state.last_update;
        
        // Wait a bit to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));
        
        let new_root = [42u8; 32];
        state.update_merkle_root(new_root);
        
        assert_eq!(state.merkle_root, new_root);
        assert!(state.last_update > initial_update);
    }
    
    #[test]
    fn test_node_count_update() {
        let mut state = NetworkState::new();
        assert_eq!(state.node_count, 0);
        
        state.set_node_count(100);
        assert_eq!(state.node_count, 100);
    }
    
    #[test]
    fn test_staleness_check() {
        let mut state = NetworkState::new();
        
        // Current state is not stale
        assert!(!state.is_stale());
        
        // Set epoch to 3+ epochs ago (stale)
        let current = NetworkState::current_epoch();
        state.epoch = current.saturating_sub(3);
        assert!(state.is_stale());
    }
    
    #[test]
    fn test_epoch_time_remaining() {
        let remaining = NetworkState::epoch_time_remaining();
        
        // Should be between 0 and 600 seconds
        assert!(remaining <= EPOCH_DURATION_SECS);
        
        println!("Time remaining in current epoch: {}s", remaining);
    }
    
    #[test]
    fn test_serialization() {
        let state = NetworkState {
            merkle_root: [123u8; 32],
            epoch: 42,
            node_count: 100,
            last_update: 1234567890,
        };
        
        // Serialize
        let serialized = bincode::serialize(&state).unwrap();
        
        // Deserialize
        let deserialized: NetworkState = bincode::deserialize(&serialized).unwrap();
        
        assert_eq!(state, deserialized);
    }
}
