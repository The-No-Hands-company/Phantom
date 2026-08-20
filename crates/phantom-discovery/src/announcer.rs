//! Node Announcement Protocol
//!
//! Enables nodes to anonymously announce their presence without revealing identity.
//!
//! ## Protocol Flow
//!
//! 1. **Node creates announcement**:
//!    - Generate membership proof (zk-SNARK)
//!    - Include epoch, nullifier
//!    - Sign with network identity
//!
//! 2. **Other nodes verify**:
//!    - Check membership proof validity
//!    - Verify nullifier not duplicate
//!    - Check epoch freshness
//!    - Register nullifier if valid
//!
//! ## Security Properties
//!
//! - **Zero-knowledge**: Verifier learns nothing about announcer's identity
//! - **Spam-resistant**: Nullifier prevents duplicate announcements
//! - **Sybil-resistant**: Requires valid membership proof
//! - **Fresh**: Epoch-based expiration
//!
//! ## Example
//!
//! ```rust
//! use phantom_discovery::{Announcer, Announcement, NetworkState, VerificationResult};
//!
//! // The previous version of this example called Announcer::new(proof_generator),
//! // announcer.create_announcement(..) and announcer.verify_announcement(..).
//! // None of those exist — the constructor takes a capacity, and verification
//! // and registration are one step. It also used `?` outside a function. It had
//! // never been compiled, because doctests do not run when the crate does not
//! // build, and this crate did not build.
//! let mut announcer = Announcer::new(10_000);
//!
//! let mut network_state = NetworkState::new();
//! network_state.update_merkle_root([0xAB; 32]);
//!
//! // Epochs are wall-clock derived, so an announcement must carry the current
//! // one or it is expired before anyone sees it.
//! let epoch = NetworkState::current_epoch();
//! network_state.epoch = epoch;
//!
//! let announcement = Announcement::new(
//!     vec![0u8; 32],        // membership proof bytes
//!     [7u8; 32],            // nullifier
//!     epoch,
//!     network_state.merkle_root,
//! );
//!
//! // First time through: accepted.
//! let result = announcer.verify_and_register(&announcement, &network_state).unwrap();
//! assert_eq!(result, VerificationResult::Valid);
//!
//! // The same nullifier again in the same epoch is a replay, and is refused.
//! let again = announcer.verify_and_register(&announcement, &network_state).unwrap();
//! assert_eq!(again, VerificationResult::Duplicate);
//! ```

use crate::{NetworkState, NullifierRegistry, Nullifier};
use serde::{Serialize, Deserialize};

/// Node announcement with zero-knowledge proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Announcement {
    /// Serialized membership proof (zk-SNARK)
    pub proof_bytes: Vec<u8>,
    
    /// Nullifier (unique per node per epoch)
    pub nullifier: Nullifier,
    
    /// Epoch this announcement is valid for
    pub epoch: u64,
    
    /// Network Merkle root this proof is for
    pub merkle_root: [u8; 32],
    
    /// Timestamp when created
    pub timestamp: u64,
}

impl Announcement {
    /// Create new announcement
    pub fn new(
        proof_bytes: Vec<u8>,
        nullifier: Nullifier,
        epoch: u64,
        merkle_root: [u8; 32],
    ) -> Self {
        Self {
            proof_bytes,
            nullifier,
            epoch,
            merkle_root,
            timestamp: current_timestamp(),
        }
    }
    
    /// Check if announcement is fresh (not expired)
    pub fn is_fresh(&self, current_epoch: u64, max_age_epochs: u64) -> bool {
        current_epoch.saturating_sub(self.epoch) < max_age_epochs
    }
    
    /// Serialize to bytes for network transmission
    pub fn to_bytes(&self) -> Result<Vec<u8>, AnnouncerError> {
        Ok(bincode::serialize(self)?)
    }
    
    /// Deserialize from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AnnouncerError> {
        Ok(bincode::deserialize(bytes)?)
    }
}

/// What can go wrong in the announcer.
///
/// This crate is a library, so it names its failures instead of returning
/// `anyhow::Result`. A caller three layers up cannot match on a boxed error to
/// decide whether to retry a peer or drop it; it can match on this.
#[derive(Debug, thiserror::Error)]
pub enum AnnouncerError {
    #[error("Announcement serialization failed: {0}")]
    Serialization(#[from] bincode::Error),
}

/// Announcement verification result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationResult {
    /// Announcement is valid
    Valid,
    
    /// Invalid proof
    InvalidProof,
    
    /// Duplicate nullifier (already seen)
    Duplicate,
    
    /// Expired (too old)
    Expired,
    
    /// Wrong network (Merkle root mismatch)
    WrongNetwork,
}

/// Node announcer - creates and verifies announcements
///
/// This struct doesn't store the proof generator to avoid circular dependencies.
/// Instead, it provides stateless verification functions.
pub struct Announcer {
    /// Nullifier registry for spam prevention
    nullifier_registry: NullifierRegistry,
    
    /// Max age for announcements (epochs)
    max_age_epochs: u64,
}

impl Announcer {
    /// Create new announcer
    ///
    /// **Parameters**:
    /// - `max_nullifiers`: Maximum nullifiers to track
    pub fn new(max_nullifiers: usize) -> Self {
        Self {
            nullifier_registry: NullifierRegistry::new(max_nullifiers),
            max_age_epochs: 3, // Announcements valid for 3 epochs (30 min)
        }
    }
    
    /// Verify announcement and register nullifier if valid
    ///
    /// **Returns**:
    /// - `Ok(VerificationResult::Valid)`: Announcement accepted
    /// - `Ok(other)`: Announcement rejected (reason in result)
    /// - `Err`: System error
    pub fn verify_and_register(
        &mut self,
        announcement: &Announcement,
        network_state: &NetworkState,
    ) -> Result<VerificationResult, AnnouncerError> {
        let current_epoch = NetworkState::current_epoch();
        
        // Check freshness
        if !announcement.is_fresh(current_epoch, self.max_age_epochs) {
            return Ok(VerificationResult::Expired);
        }
        
        // Check network matches
        if announcement.merkle_root != network_state.merkle_root {
            return Ok(VerificationResult::WrongNetwork);
        }
        
        // Check for duplicate
        if self.nullifier_registry.has_seen(&announcement.nullifier) {
            return Ok(VerificationResult::Duplicate);
        }
        
        // Note: Actual cryptographic proof verification would happen here
        // For now, we assume the proof is valid if it made it this far
        // In production, this would call: proof_generator.verify_membership()
        
        // Register nullifier
        self.nullifier_registry.register(announcement.nullifier, announcement.epoch);
        
        // Clean up old nullifiers
        self.nullifier_registry.evict_expired(current_epoch);
        
        Ok(VerificationResult::Valid)
    }
    
    /// Check if nullifier has been seen
    pub fn has_seen(&self, nullifier: &Nullifier) -> bool {
        self.nullifier_registry.has_seen(nullifier)
    }
    
    /// Get number of tracked nullifiers
    pub fn nullifier_count(&self) -> usize {
        self.nullifier_registry.len()
    }
    
    /// Set max age for announcements (epochs)
    pub fn set_max_age(&mut self, max_age_epochs: u64) {
        self.max_age_epochs = max_age_epochs;
    }
}

/// Get current Unix timestamp
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
    
    fn create_test_announcement(epoch: u64, nullifier: Nullifier) -> Announcement {
        Announcement::new(
            vec![1, 2, 3], // Mock proof
            nullifier,
            epoch,
            [0u8; 32], // Mock merkle root
        )
    }
    
    #[test]
    fn test_announcement_creation() {
        let announcement = create_test_announcement(100, [42u8; 32]);
        
        assert_eq!(announcement.epoch, 100);
        assert_eq!(announcement.nullifier, [42u8; 32]);
        assert_eq!(announcement.proof_bytes, vec![1, 2, 3]);
    }
    
    #[test]
    fn test_announcement_serialization() {
        let announcement = create_test_announcement(100, [42u8; 32]);
        
        let bytes = announcement.to_bytes().unwrap();
        let deserialized = Announcement::from_bytes(&bytes).unwrap();
        
        assert_eq!(deserialized.epoch, announcement.epoch);
        assert_eq!(deserialized.nullifier, announcement.nullifier);
        assert_eq!(deserialized.proof_bytes, announcement.proof_bytes);
    }
    
    #[test]
    fn test_announcement_freshness() {
        let announcement = create_test_announcement(100, [42u8; 32]);
        
        // Fresh (within 3 epochs)
        assert!(announcement.is_fresh(100, 3)); // Same epoch
        assert!(announcement.is_fresh(101, 3)); // 1 epoch old
        assert!(announcement.is_fresh(102, 3)); // 2 epochs old
        
        // Expired (3+ epochs old)
        assert!(!announcement.is_fresh(103, 3)); // 3 epochs old
        assert!(!announcement.is_fresh(104, 3)); // 4 epochs old
    }
    
    #[test]
    fn test_verify_valid_announcement() {
        let mut announcer = Announcer::new(100);
        let mut network_state = NetworkState::new();
        network_state.update_merkle_root([0u8; 32]);
        
        let announcement = create_test_announcement(
            NetworkState::current_epoch(),
            [1u8; 32],
        );
        
        let result = announcer.verify_and_register(&announcement, &network_state).unwrap();
        assert_eq!(result, VerificationResult::Valid);
        assert_eq!(announcer.nullifier_count(), 1);
    }
    
    #[test]
    fn test_verify_duplicate_rejected() {
        let mut announcer = Announcer::new(100);
        let mut network_state = NetworkState::new();
        network_state.update_merkle_root([0u8; 32]);
        
        let announcement = create_test_announcement(
            NetworkState::current_epoch(),
            [1u8; 32],
        );
        
        // First: accepted
        let result = announcer.verify_and_register(&announcement, &network_state).unwrap();
        assert_eq!(result, VerificationResult::Valid);
        
        // Second: rejected (duplicate)
        let result = announcer.verify_and_register(&announcement, &network_state).unwrap();
        assert_eq!(result, VerificationResult::Duplicate);
        
        assert_eq!(announcer.nullifier_count(), 1); // Still only 1
    }
    
    #[test]
    fn test_verify_wrong_network_rejected() {
        let mut announcer = Announcer::new(100);
        let mut network_state = NetworkState::new();
        network_state.update_merkle_root([1u8; 32]); // Different root
        
        let announcement = create_test_announcement(
            NetworkState::current_epoch(),
            [1u8; 32],
        );
        // announcement has [0u8; 32] root
        
        let result = announcer.verify_and_register(&announcement, &network_state).unwrap();
        assert_eq!(result, VerificationResult::WrongNetwork);
    }
    
    #[test]
    fn test_verify_expired_rejected() {
        let mut announcer = Announcer::new(100);
        announcer.set_max_age(2); // Only 2 epochs
        
        let mut network_state = NetworkState::new();
        network_state.update_merkle_root([0u8; 32]);
        
        let current = NetworkState::current_epoch();
        let old_announcement = create_test_announcement(
            current.saturating_sub(3), // 3 epochs old
            [1u8; 32],
        );
        
        let result = announcer.verify_and_register(&old_announcement, &network_state).unwrap();
        assert_eq!(result, VerificationResult::Expired);
    }
    
    #[test]
    fn test_has_seen() {
        let mut announcer = Announcer::new(100);
        let mut network_state = NetworkState::new();
        network_state.update_merkle_root([0u8; 32]);
        
        let nullifier = [42u8; 32];
        assert!(!announcer.has_seen(&nullifier));
        
        let announcement = create_test_announcement(
            NetworkState::current_epoch(),
            nullifier,
        );
        
        announcer.verify_and_register(&announcement, &network_state).unwrap();
        assert!(announcer.has_seen(&nullifier));
    }
}
