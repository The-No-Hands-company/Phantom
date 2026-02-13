//! Anonymous Node Discovery
//!
//! Zero-knowledge set membership for discovering nodes without leaking topology.
//! 
//! ## Architecture
//! 
//! PHANTOM uses a three-layer discovery protocol:
//! 
//! 1. **Network State**: Merkle root consensus + epoch-based updates
//! 2. **Membership Proofs**: zk-SNARKs proving "I'm in the network" without revealing identity
//! 3. **Nullifier System**: Prevent spam and Sybil attacks
//! 
//! ## Components
//! 
//! - `state`: Network state management (epochs, Merkle roots)
//! - `nullifiers`: Spam prevention via unique nullifiers
//! - `announcer`: Node announcement protocol (future)

pub mod state;
pub mod nullifiers;
pub mod announcer;
pub mod announcement;
pub mod gossip;
pub mod discovery;
pub mod bootstrap;

pub use state::{NetworkState, EPOCH_DURATION_SECS, current_timestamp};
pub use nullifiers::{NullifierRegistry, Nullifier};
pub use announcer::{Announcer, Announcement, VerificationResult};
pub use announcement::{NodeAnnouncement, NodeDescriptor, NodeCapabilities};
pub use gossip::{GossipManager, GossipMessage, GossipConfig, BloomFilter};
pub use discovery::{DiscoveryService, DiscoveryConfig, DiscoveryQuery, DiscoveryResult, DiscoveryError};
pub use bootstrap::{BootstrapClient, BootstrapConfig, BootstrapResult, BootstrapError};

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_test() {
        assert!(true);
    }
}
