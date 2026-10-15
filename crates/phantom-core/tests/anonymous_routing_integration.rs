//! Week 3 Day 3: Packet Construction Integration Tests
//!
//! Tests the integration of anonymous packet construction with membership proofs

use phantom_core::{
    AnonymousPacketBuilder, SenderCredentials, MembershipProofData,
    NetworkGraph,
};
use phantom_core::network::NodeInfo;
use phantom_crypto::FheEngine;
use std::sync::Arc;

#[test]
fn test_end_to_end_anonymous_packet_flow() {
    // Setup network
    let network = create_test_network(20); // 20 nodes
    let network_arc = Arc::new(network);

    // Initialize FHE engine
    let fhe_engine = Arc::new(FheEngine::generate_keys());

    let epoch = 12345;
    let credentials = SenderCredentials {
        node_id: [99u8; 32],
        merkle_proof: MembershipProofData {
            leaf_index: 0,
            merkle_root: *network_arc.commitment(),
            path_siblings: vec![],
            path_directions: vec![],
        },
        epoch,
    };

    let recipient = phantom_crypto::KeyPair::generate();
    let builder = AnonymousPacketBuilder::new(fhe_engine, network_arc, epoch);
    let packet = builder.build_packet(
        &credentials,
        105,
        b"Anonymous message".to_vec(),
        recipient.public.to_bytes(),
    );

    assert!(packet.is_ok(), "Packet construction should succeed");
    let packet = packet.unwrap();

    assert!(!packet.routing_blob.is_empty());
    // Payload must be encrypted — not stored in the clear.
    assert_ne!(packet.payload, b"Anonymous message");
    // Recipient can decrypt it.
    let recovered = packet.decrypt_payload(recipient.secret.to_bytes()).unwrap();
    assert_eq!(recovered, b"Anonymous message");
    assert_ne!(packet.nullifier, [0u8; 32]);
    assert_ne!(packet.packet_id, [0u8; 32]);
}

#[test]
fn test_credential_epoch_validation() {
    let network = Arc::new(create_test_network(10));
    let fhe_engine = Arc::new(FheEngine::generate_keys());

    let correct_epoch = 100;
    let builder = AnonymousPacketBuilder::new(fhe_engine, network.clone(), correct_epoch);

    let bad_credentials = SenderCredentials {
        node_id: [1u8; 32],
        merkle_proof: MembershipProofData {
            leaf_index: 0,
            merkle_root: *network.commitment(),
            path_siblings: vec![],
            path_directions: vec![],
        },
        epoch: 99, // Wrong epoch
    };

    let recipient = phantom_crypto::KeyPair::generate();
    let result = builder.build_packet(&bad_credentials, 105, vec![], recipient.public.to_bytes());
    assert!(result.is_err());
}

#[test]
fn test_credential_merkle_root_validation() {
    let network = Arc::new(create_test_network(10));
    let fhe_engine = Arc::new(FheEngine::generate_keys());
    let epoch = 100;

    let builder = AnonymousPacketBuilder::new(fhe_engine, network.clone(), epoch);

    let bad_credentials = SenderCredentials {
        node_id: [1u8; 32],
        merkle_proof: MembershipProofData {
            leaf_index: 0,
            merkle_root: [0u8; 32], // Wrong root
            path_siblings: vec![],
            path_directions: vec![],
        },
        epoch,
    };

    let recipient = phantom_crypto::KeyPair::generate();
    let result = builder.build_packet(&bad_credentials, 105, vec![], recipient.public.to_bytes());
    assert!(result.is_err());
}

#[test]
fn test_nullifier_uniqueness() {
    let network = Arc::new(create_test_network(10));
    let fhe_engine = Arc::new(FheEngine::generate_keys());
    let epoch = 100;

    let builder = AnonymousPacketBuilder::new(fhe_engine, network.clone(), epoch);

    let credentials1 = create_credentials([1u8; 32], &network, epoch);
    let credentials2 = create_credentials([2u8; 32], &network, epoch);
    let credentials3 = create_credentials([1u8; 32], &network, epoch + 1);

    let recipient = phantom_crypto::KeyPair::generate();
    let packet1 = builder.build_packet(&credentials1, 105, vec![], recipient.public.to_bytes()).unwrap();
    let packet2 = builder.build_packet(&credentials2, 105, vec![], recipient.public.to_bytes()).unwrap();

    // Different nodes → different nullifiers
    assert_ne!(packet1.nullifier, packet2.nullifier);

    // Same node, different epoch → different nullifiers
    let builder2 = AnonymousPacketBuilder::new(
        Arc::new(FheEngine::generate_keys()),
        network.clone(),
        epoch + 1,
    );
    let packet3 = builder2.build_packet(&credentials3, 105, vec![], recipient.public.to_bytes()).unwrap();
    assert_ne!(packet1.nullifier, packet3.nullifier);
}

#[test]
fn test_path_randomization() {
    let network = Arc::new(create_test_network(20));
    let fhe_engine = Arc::new(FheEngine::generate_keys());
    let epoch = 100;

    let builder = AnonymousPacketBuilder::new(fhe_engine, network.clone(), epoch);
    let credentials = create_credentials([42u8; 32], &network, epoch);

    let destination = 105;
    let mut successful_packets = Vec::new();
    let recipient = phantom_crypto::KeyPair::generate();

    for _ in 0..10 {
        if let Ok(packet) = builder.build_packet(&credentials, destination, vec![], recipient.public.to_bytes()) {
            successful_packets.push(packet);
        }
    }

    assert!(!successful_packets.is_empty(), "Should build at least one packet");

    for packet in &successful_packets {
        assert!(!packet.routing_blob.is_empty());
        assert_ne!(packet.nullifier, [0u8; 32]);
    }
}

// Helper functions

fn create_test_network(size: usize) -> NetworkGraph {
    let mut network = NetworkGraph::new();

    for id in 100..(100 + size as u32) {
        network.add_node(NodeInfo {
            id,
            bandwidth: 1_000_000,
            latency_ms: 50,
            uptime_hours: 24,
            reputation: 0.9,
        });
    }

    for id1 in 100..(100 + size as u32) {
        for id2 in (id1 + 1).min(100 + size as u32)..(100 + size as u32).min(id1 + 4) {
            if id2 < 100 + size as u32 {
                network.add_edge(id1, id2);
            }
        }
    }

    network
}

fn create_credentials(
    node_id: [u8; 32],
    network: &NetworkGraph,
    epoch: u64,
) -> SenderCredentials {
    SenderCredentials {
        node_id,
        merkle_proof: MembershipProofData {
            leaf_index: 0,
            merkle_root: *network.commitment(),
            path_siblings: vec![],
            path_directions: vec![],
        },
        epoch,
    }
}
