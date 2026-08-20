//! Node identity.
//!
//! PHANTOM has two different things that both want to be called a "node id",
//! and conflating them breaks the protocol's central promise.
//!
//! [`crate::network::NodeId`] is a `u32`. It is a routing-table index: the key
//! of the network graph, the leaf index of the Merkle tree, the value a packet
//! carries so a relay knows where to send it next. It is public by
//! construction — that is its whole job — and `u32` is the right size for it.
//!
//! [`NodeIdentity`] is a 32-byte secret. It is what a node proves control of
//! without revealing, and it is the preimage of the announcement nullifier:
//!
//! ```text
//! nullifier = H(node_identity || epoch || network_commitment)
//! ```
//!
//! Both `epoch` and `network_commitment` are public values. If the identity in
//! that hash were a `u32`, anyone could enumerate all 2^32 candidates against a
//! published nullifier and recover the node behind it — a few minutes of GPU
//! time for a complete deanonymisation of the network. The nullifier exists to
//! stop double-announcements *without* revealing who announced; a guessable
//! preimage removes the second half and leaves only the cost.
//!
//! 32 bytes of entropy puts that search out of reach, which is the property the
//! discovery protocol assumes it has.

use serde::{Deserialize, Serialize};

/// A node's secret identity: 32 bytes, never transmitted in the clear.
///
/// Construct one with [`NodeIdentity::generate`] and keep it alongside the
/// node's signing key. What goes on the wire is the nullifier and the
/// membership proof, neither of which reveals this value.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeIdentity(pub [u8; 32]);

impl NodeIdentity {
    /// Generate a fresh identity from the OS random source.
    pub fn generate() -> Self {
        use rand::RngCore;
        let mut bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Borrow the raw bytes, for feeding into a hash.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Deliberately does not print the secret.
///
/// An identity that leaks through a stray `{:?}` in a log line is as
/// compromised as one that leaks over the wire, and debug formatting is
/// exactly the kind of thing that gets added in a hurry while chasing
/// something else. The prefix is enough to tell two identities apart in a
/// trace without being enough to use.
impl std::fmt::Debug for NodeIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NodeIdentity({:02x}{:02x}…)",
            self.0[0], self.0[1]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_identities_differ() {
        // If this ever fails the RNG is broken, and every nullifier in the
        // network collides.
        let a = NodeIdentity::generate();
        let b = NodeIdentity::generate();
        assert_ne!(a, b);
    }

    #[test]
    fn generated_identity_is_not_trivial() {
        // Catches a zeroed buffer being returned as though it were random.
        let id = NodeIdentity::generate();
        assert_ne!(id.0, [0u8; 32]);
    }

    #[test]
    fn debug_does_not_print_the_secret() {
        // The property that matters: the full secret must not appear in a
        // formatted string. Asserting on the prefix alone would still pass if
        // someone appended the rest.
        let id = NodeIdentity([0xAB; 32]);
        let shown = format!("{:?}", id);
        let full_hex: String = id.0.iter().map(|b| format!("{:02x}", b)).collect();
        assert!(!shown.contains(&full_hex));
        assert!(shown.contains("abab"));
    }

    #[test]
    fn round_trips_through_serde() {
        let id = NodeIdentity::generate();
        let bytes = bincode::serialize(&id).unwrap();
        let back: NodeIdentity = bincode::deserialize(&bytes).unwrap();
        assert_eq!(id, back);
    }
}
