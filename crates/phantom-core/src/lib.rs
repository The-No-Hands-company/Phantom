//! PHANTOM Core Protocol
//!
//! Core data structures and protocol logic for PHANTOM.

pub mod error;
pub mod packet;
pub mod network;
pub mod merkle;
pub mod proof;
pub mod anonymous_routing;

pub use error::{ProtocolError, Result};
pub use packet::{PhantomPacket, RoutingPath};
pub use network::NetworkGraph;
pub use merkle::{MerkleTree, Hash};
pub use proof::{RoutingProof, PublicInputs, MerkleProof as ProofMerkleProof, ProofGenerator};
pub use anonymous_routing::{AnonymousPacketBuilder, SenderCredentials, MembershipProofData};

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_test() {
        assert!(true);
    }
}
