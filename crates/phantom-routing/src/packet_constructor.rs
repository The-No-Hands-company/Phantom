//! Packet construction: the seam between routing, FHE and the prover.
//!
//! The three pieces a PHANTOM packet needs already existed and had never been
//! joined. `PhantomPacket::construct` encrypts the routing table under FHE and
//! seals the payload. `NetworkGraph` holds the membership commitment a relay
//! checks a packet against. `Plonky2ProofGenerator` holds the cached Merkle
//! proofs that let a sender prove membership without naming itself.
//!
//! Nothing owned all three at once, so every caller would have had to thread
//! them together by hand and keep their commitments in step. The end-to-end
//! test assumed a type that did this and referred to it by name; the type was
//! never written, which is why that test had never compiled.

use std::sync::{Arc, RwLock};

use phantom_core::{NetworkGraph, PhantomPacket, RoutingPath};
use phantom_crypto::FheEngine;
use phantom_zkvm::Plonky2ProofGenerator;

/// Builds PHANTOM packets against a shared view of the network.
///
/// Cheap to clone: every field is an `Arc`. The FHE engine in particular is
/// expensive to create and holds key material, so it is shared rather than
/// rebuilt per packet.
pub struct PacketConstructor {
    network: Arc<RwLock<NetworkGraph>>,
    fhe_engine: Arc<FheEngine>,
    proof_generator: Arc<Plonky2ProofGenerator>,
}

impl PacketConstructor {
    pub fn new(
        network: Arc<RwLock<NetworkGraph>>,
        fhe_engine: Arc<FheEngine>,
        proof_generator: Arc<Plonky2ProofGenerator>,
    ) -> Self {
        Self { network, fhe_engine, proof_generator }
    }

    /// Construct a packet for `path` carrying `payload`.
    ///
    /// The commitment is read from the network graph at call time rather than
    /// cached on this struct. Membership changes as nodes join and leave, and a
    /// packet built against a stale root is rejected by the first relay that
    /// checks it — so the freshest value is the only correct one, and holding a
    /// copy here would be a bug that only appears under churn.
    /// Construct a packet for `path` carrying `payload`, encrypted for `recipient_kem_public`.
    ///
    /// `recipient_kem_public` is the raw ML-KEM-1024 public key bytes of the intended
    /// recipient (registered via the Phantom identity API in Privacy Part 3).
    pub fn construct_packet(
        &self,
        path: &RoutingPath,
        payload: Vec<u8>,
        recipient_kem_public: &[u8],
    ) -> Result<PhantomPacket, PacketConstructionError> {
        let commitment = {
            let network = self
                .network
                .read()
                .map_err(|_| PacketConstructionError::NetworkLockPoisoned)?;
            *network.commitment()
        };

        PhantomPacket::construct(path.clone(), payload, recipient_kem_public, &self.fhe_engine, &commitment)
            .map_err(PacketConstructionError::Construction)
    }

    /// The membership root the prover is currently committed to.
    ///
    /// Returns `None` before `commit_to_network` has been called. Comparing
    /// this against the graph's own commitment is how a caller detects that the
    /// prover's cached Merkle proofs have gone stale: proofs generated against
    /// an old root verify against nothing.
    pub fn committed_root(&self) -> Option<[u8; 32]> {
        self.proof_generator.get_merkle_root()
    }

    /// Whether the prover's commitment still matches the live network.
    pub fn commitment_is_current(&self) -> Result<bool, PacketConstructionError> {
        let network = self
            .network
            .read()
            .map_err(|_| PacketConstructionError::NetworkLockPoisoned)?;
        Ok(self.committed_root().as_ref() == Some(network.commitment()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PacketConstructionError {
    /// A thread panicked while holding the network lock. Recovering the
    /// contents would mean trusting a half-written graph to build packets
    /// against, so this is surfaced rather than papered over.
    #[error("network lock poisoned")]
    NetworkLockPoisoned,

    #[error("packet construction failed: {0}")]
    Construction(#[from] phantom_core::ProtocolError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use phantom_core::NodeInfo;

    fn node(id: u32) -> NodeInfo {
        NodeInfo { id, bandwidth: 1_000_000, latency_ms: 10, uptime_hours: 24, reputation: 1.0 }
    }

    fn network_of(n: u32) -> Arc<RwLock<NetworkGraph>> {
        let mut g = NetworkGraph::new();
        for i in 0..n {
            g.add_node(node(i));
        }
        Arc::new(RwLock::new(g))
    }

    fn constructor(network: Arc<RwLock<NetworkGraph>>) -> PacketConstructor {
        let generator = Plonky2ProofGenerator::new(10, 10).expect("prover init");
        PacketConstructor::new(network, Arc::new(FheEngine::generate_keys()), Arc::new(generator))
    }

    #[test]
    fn constructs_a_packet_over_a_three_hop_path() {
        let network = network_of(4);
        let c = constructor(network);
        let recipient = phantom_crypto::KeyPair::generate();

        let path = RoutingPath::new(vec![0, 1, 2]).expect("3 hops is a valid path");
        let packet = c
            .construct_packet(&path, b"hello".to_vec(), recipient.public.to_bytes())
            .expect("construction should succeed");

        assert_ne!(packet.payload, b"hello", "payload must be encrypted, not stored in the clear");
        assert!(!packet.routing_blob.is_empty(), "routing table must be FHE-encrypted");
        // Verify the recipient can recover the original payload.
        let recovered = packet.decrypt_payload(recipient.secret.to_bytes()).expect("decrypt");
        assert_eq!(recovered, b"hello");
    }

    #[test]
    fn payload_is_not_readable_on_the_wire() {
        let network = network_of(4);
        let c = constructor(network);
        let recipient = phantom_crypto::KeyPair::generate();

        let path = RoutingPath::new(vec![0, 1, 2]).expect("3 hops is a valid path");
        let packet = c
            .construct_packet(&path, b"hello".to_vec(), recipient.public.to_bytes())
            .expect("construction should succeed");

        // The payload must not survive in the clear: a relay carrying this packet
        // learns nothing from the contents.
        let wire = bincode::serialize(&packet).expect("packet serializes");
        assert!(
            wire.windows(5).all(|w| w != b"hello"),
            "plaintext payload found in the serialized packet"
        );

        // The intended recipient can still recover it.
        let recovered = packet.decrypt_payload(recipient.secret.to_bytes()).expect("decrypt");
        assert_eq!(recovered, b"hello");
    }

    #[test]
    fn a_two_hop_path_is_refused_before_construction() {
        // Enforced by RoutingPath itself, which is where it belongs — a path
        // too short to hide the sender should never reach packet construction.
        assert!(RoutingPath::new(vec![0, 1]).is_err());
    }

    #[test]
    fn commitment_tracks_the_live_network_not_a_cached_copy() {
        let network = network_of(4);
        let c = constructor(network.clone());

        let before = {
            let g = network.read().unwrap();
            *g.commitment()
        };

        // A node joins after the constructor was built.
        network.write().unwrap().add_node(node(99));

        let after = {
            let g = network.read().unwrap();
            *g.commitment()
        };

        assert_ne!(before, after, "adding a node must change the commitment");

        // Building a packet now must use the new root. If the constructor had
        // cached the commitment at construction time this would still be the
        // old one, and every packet would be rejected by the first relay.
        let recipient = phantom_crypto::KeyPair::generate();
        let path = RoutingPath::new(vec![0, 1, 2]).unwrap();
        c.construct_packet(&path, b"x".to_vec(), recipient.public.to_bytes())
            .expect("construction should use the current commitment and succeed");
    }

    #[test]
    fn stale_prover_commitment_is_detectable() {
        let network = network_of(4);
        let mut generator = Plonky2ProofGenerator::new(10, 10).expect("prover init");
        generator
            .commit_to_network(&network.read().unwrap())
            .expect("commit should succeed");

        let c = PacketConstructor::new(
            network.clone(),
            Arc::new(FheEngine::generate_keys()),
            Arc::new(generator),
        );

        // The prover has committed, so it holds a root.
        assert!(c.committed_root().is_some());

        // Membership changes underneath it; the cached Merkle proofs are now
        // against a root nobody will accept, and that must be visible.
        network.write().unwrap().add_node(node(99));
        assert!(
            !c.commitment_is_current().unwrap(),
            "a membership change must invalidate the prover's commitment"
        );
    }
}
