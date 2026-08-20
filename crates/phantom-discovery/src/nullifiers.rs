//! Nullifier Tracking System
//!
//! Prevents spam and duplicate announcements by tracking unique nullifiers.
//!
//! ## How It Works
//!
//! Each (node, epoch) pair produces a unique nullifier via:
//! ```text
//! nullifier = hash(node_id || epoch)
//! ```
//!
//! The registry tracks all seen nullifiers and rejects duplicates.
//!
//! ## Spam Prevention
//!
//! - **Duplicate detection**: Same node can't announce twice per epoch
//! - **TTL**: Old nullifiers auto-expire (default: 3 epochs)
//! - **Memory bound**: LRU eviction when capacity reached
//!
//! ## Example
//!
//! ```rust
//! use phantom_discovery::NullifierRegistry;
//!
//! let mut registry = NullifierRegistry::new(100_000); // Max 100K nullifiers
//!
//! // First announcement: accepted
//! let nullifier = [1u8; 32];
//! assert!(registry.register(nullifier, 100));
//!
//! // Duplicate: rejected
//! assert!(!registry.register(nullifier, 100));
//!
//! // A new epoch means a genuinely different nullifier, because the epoch is
//! // hashed into it: nullifier = H(identity || epoch || commitment). Passing
//! // the *same* nullifier with a later epoch is still a duplicate — has_seen
//! // keys on the nullifier alone — and this example used to claim otherwise.
//! let next_epoch_nullifier = [2u8; 32];
//! assert!(registry.register(next_epoch_nullifier, 101));
//! ```

use std::collections::HashMap;
use serde::{Serialize, Deserialize};

/// Nullifier value (32-byte hash)
pub type Nullifier = [u8; 32];

/// Entry in nullifier registry with metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
struct NullifierEntry {
    /// Epoch when this nullifier was first seen
    epoch: u64,
    
    /// Timestamp when registered (Unix seconds)
    timestamp: u64,
}

/// Nullifier registry for spam prevention
///
/// Tracks seen nullifiers to prevent duplicate announcements.
/// Automatically evicts old entries to bound memory usage.
pub struct NullifierRegistry {
    /// Map of nullifier → metadata
    nullifiers: HashMap<Nullifier, NullifierEntry>,
    
    /// Maximum number of nullifiers to store
    max_capacity: usize,
    
    /// Number of epochs before nullifiers expire (default: 3)
    ttl_epochs: u64,
}

impl NullifierRegistry {
    /// Create new nullifier registry
    ///
    /// **Parameters**:
    /// - `max_capacity`: Maximum nullifiers to store (evicts oldest when full)
    pub fn new(max_capacity: usize) -> Self {
        Self {
            nullifiers: HashMap::with_capacity(max_capacity),
            max_capacity,
            ttl_epochs: 3, // Keep nullifiers for 3 epochs (30 minutes default)
        }
    }
    
    /// Register a nullifier from a membership proof
    ///
    /// **Returns** `true` if the nullifier was accepted (first time seen),
    /// `false` if it was rejected as a duplicate.
    ///
    /// This is infallible. It previously returned `Result<bool>` with no `Err`
    /// arm anywhere in the body, which made every caller handle a failure that
    /// could not occur — and, worse, made a real failure indistinguishable
    /// from the ceremony if one were ever added.
    pub fn register(&mut self, nullifier: Nullifier, epoch: u64) -> bool {
        // Check if already seen
        if self.has_seen(&nullifier) {
            return false; // Duplicate!
        }
        
        // Check capacity and evict if needed
        if self.nullifiers.len() >= self.max_capacity {
            self.evict_oldest();
        }
        
        // Register nullifier
        let entry = NullifierEntry {
            epoch,
            timestamp: current_timestamp(),
        };
        
        self.nullifiers.insert(nullifier, entry);
        true
    }
    
    /// Check if nullifier has been seen before
    pub fn has_seen(&self, nullifier: &Nullifier) -> bool {
        self.nullifiers.contains_key(nullifier)
    }
    
    /// Evict expired nullifiers (older than TTL)
    ///
    /// Call this periodically to free memory.
    pub fn evict_expired(&mut self, current_epoch: u64) {
        let ttl = self.ttl_epochs;
        self.nullifiers.retain(|_, entry| {
            current_epoch.saturating_sub(entry.epoch) < ttl
        });
    }
    
    /// Evict oldest nullifiers when capacity reached
    fn evict_oldest(&mut self) {
        // Find oldest entry by timestamp
        if let Some((&oldest_nullifier, _)) = self.nullifiers.iter()
            .min_by_key(|(_, entry)| entry.timestamp)
        {
            self.nullifiers.remove(&oldest_nullifier);
        }
    }
    
    /// Get current registry size
    pub fn len(&self) -> usize {
        self.nullifiers.len()
    }
    
    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.nullifiers.is_empty()
    }
    
    /// Clear all nullifiers (for testing)
    pub fn clear(&mut self) {
        self.nullifiers.clear();
    }
    
    /// Set TTL in epochs (default: 3)
    pub fn set_ttl(&mut self, ttl_epochs: u64) {
        self.ttl_epochs = ttl_epochs;
    }
}

/// Get current Unix timestamp in seconds
fn current_timestamp() -> u64 {
    use std::time::SystemTime;
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_register_and_duplicate_detection() {
        let mut registry = NullifierRegistry::new(100);
        
        let nullifier = [42u8; 32];
        let epoch = 100;
        
        // First registration: accepted
        assert!(registry.register(nullifier, epoch));
        assert_eq!(registry.len(), 1);
        
        // Duplicate: rejected
        assert!(!registry.register(nullifier, epoch));
        assert_eq!(registry.len(), 1); // Size unchanged
    }
    
    #[test]
    fn test_has_seen() {
        let mut registry = NullifierRegistry::new(100);
        
        let nullifier = [1u8; 32];
        assert!(!registry.has_seen(&nullifier));
        
        registry.register(nullifier, 100);
        assert!(registry.has_seen(&nullifier));
    }
    
    #[test]
    fn test_evict_expired() {
        let mut registry = NullifierRegistry::new(100);
        registry.set_ttl(2); // 2 epochs TTL
        
        // Register nullifiers in different epochs
        registry.register([1u8; 32], 100);
        registry.register([2u8; 32], 101);
        registry.register([3u8; 32], 103);
        
        assert_eq!(registry.len(), 3);
        
        // Evict at epoch 103 (TTL=2)
        // Epoch 100: 103-100=3 >= 2 → evicted
        // Epoch 101: 103-101=2 >= 2 → evicted  
        // Epoch 103: 103-103=0 < 2 → kept
        registry.evict_expired(103);
        
        assert_eq!(registry.len(), 1);
        assert!(!registry.has_seen(&[1u8; 32])); // Evicted
        assert!(!registry.has_seen(&[2u8; 32])); // Evicted
        assert!(registry.has_seen(&[3u8; 32]));  // Kept
    }
    
    #[test]
    fn test_capacity_limit() {
        let mut registry = NullifierRegistry::new(5);
        
        // Fill to capacity
        for i in 0..5 {
            let mut nullifier = [0u8; 32];
            nullifier[0] = i as u8;
            registry.register(nullifier, 100);
        }
        
        assert_eq!(registry.len(), 5);
        
        // Add one more → evicts oldest
        let new_nullifier = [99u8; 32];
        registry.register(new_nullifier, 100);
        
        assert_eq!(registry.len(), 5); // Still at capacity
        assert!(registry.has_seen(&new_nullifier)); // New one added
    }
    
    #[test]
    fn test_clear() {
        let mut registry = NullifierRegistry::new(100);
        
        registry.register([1u8; 32], 100);
        registry.register([2u8; 32], 100);
        
        assert_eq!(registry.len(), 2);
        
        registry.clear();
        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());
    }
}
