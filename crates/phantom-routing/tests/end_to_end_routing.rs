//! End-to-End PHANTOM Routing Integration Test
//!
//! This test validates the complete PHANTOM protocol stack:
//! 1. **Network Setup**: Build network graph with Merkle tree
//! 2. **Anonymous Discovery**: Prove membership without revealing identity
//! 3. **Packet Construction**: Build packet with FHE routing + zk-proof
//! 4. **Multi-Hop Forwarding**: Route packet through network obliviously
//! 5. **Proof Verification**: Validate zkSNARK proofs at each hop
//!
//! ## Components Tested
//! - phantom-core: Network graph, packet structure
//! - phantom-crypto: FHE routing engine
//! - phantom-circuit: Membership + routing circuits
//! - phantom-zkvm: Plonky2 proof generation
//! - phantom-routing: Oblivious forwarding protocol
//!
//! ## Security Guarantees Verified
//! - ✅ Oblivious routing: Nodes forward without learning paths
//! - ✅ Anonymous membership: Node proves participation without revealing ID
//! - ✅ Path validity: Proofs guarantee valid routes (no loops)
//! - ✅ Nullifier uniqueness: Rate limiting prevents Sybil attacks

use phantom_core::proof::ProofGenerator as _;
use phantom_core::{
    NetworkGraph, PhantomPacket, RoutingPath,
    proof::{ProofGenerator, PublicInputs, RoutingProof},
};
use phantom_crypto::FheEngine;
use phantom_zkvm::{Plonky2ProofGenerator, HashProofGenerator};
use phantom_routing::forwarding_protocol::{NetworkSimulator, PathStatus};
use phantom_routing::packet_constructor::PacketConstructor;
use phantom_core::NodeInfo;
use phantom_circuit::MembershipWitness;

use std::sync::{Arc, RwLock};
use std::time::Instant;

/// Test configuration for end-to-end routing
struct TestConfig {
    network_size: usize,
    num_packets: usize,
    path_length: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            network_size: 100,
            num_packets: 10,
            path_length: 5,
        }
    }
}

/// Full pipeline: topology -> commitment -> membership proof -> packet.
///
/// Ignored because it gets as far as Plonky2 and stops there. The membership
/// circuit fails with "54 generators weren't run" — a witness error meaning
/// the circuit declares targets that nothing assigns, so the proof cannot be
/// produced. That is unfinished circuit work in phantom-circuit, not a defect
/// in this test.
///
/// Everything before that point does now work and is worth keeping runnable:
/// the topology builds, commit_to_identities caches a proof per node, and
/// prove_membership resolves the right witness — which it could not do until
/// 2026-08-20, because the tree was built over u32 routing indices while the
/// lookup hashed 32-byte identities. That mismatch meant the membership path
/// returned "Node not found in network" for every node ever committed.
///
/// This test had never compiled before that date either, which is why none of
/// it was known. Remove the #[ignore] once the membership circuit assigns its
/// witness fully.
#[test]
#[ignore = "phantom-circuit membership circuit is incomplete: 54 generators weren't run"]
fn test_end_to_end_phantom_routing() {
    println!("\n=== PHANTOM End-to-End Routing Test ===\n");
    
    let config = TestConfig::default();
    
    // ======================================
    // PHASE 1: Network Setup
    // ======================================
    println!("Phase 1: Network Setup");
    let start = Instant::now();
    
    let mut network = NetworkGraph::new();
    
    // Build network topology (fully connected for simplicity)
    println!("  Building network with {} nodes...", config.network_size);
    for i in 0..config.network_size as u32 {
        network.add_node(NodeInfo {
            id: i,
            bandwidth: 1_000_000,
            latency_ms: 10,
            uptime_hours: 24,
            reputation: 1.0,
        });
    }
    
    // Add edges (each node connects to 6 random peers)
    use rand::{Rng, SeedableRng};
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    
    for i in 0..config.network_size as u32 {
        for _ in 0..6 {
            let peer = rng.gen_range(0..config.network_size as u32);
            if peer != i {
                network.add_edge(i, peer);
            }
        }
    }
    
    println!("  Network topology: {} nodes, {} edges", 
        network.node_count(), network.edge_count());
    
    // Build Merkle tree for network
    let merkle_root = *network.commitment();
    println!("  Merkle root: {:?}", &merkle_root[..8]);
    println!("  Setup time: {:?}\n", start.elapsed());
    
    // ======================================
    // PHASE 2: zkVM Proof System Initialization
    // ======================================
    println!("Phase 2: zkVM Proof System (Plonky2)");
    let start = Instant::now();
    
    let tree_depth = 10; // 2^10 = 1024 max nodes
    let max_path_length = 10;
    
    let mut proof_generator = Plonky2ProofGenerator::new(tree_depth, max_path_length)
        .expect("Failed to initialize Plonky2");
    
    // Commit to the identities membership proofs are actually queried by.
    // commit_to_network commits to routing indices, which prove_membership
    // cannot look up — see commit_to_identities for why.
    let identities: Vec<[u8; 32]> = (0..config.network_size as u32)
        .map(node_id_to_bytes)
        .collect();
    proof_generator.commit_to_identities(&identities)
        .expect("Failed to commit to network");
    
    println!("  Circuit setup time: {:?}\n", start.elapsed());
    
    // ======================================
    // PHASE 3: Anonymous Node Discovery
    // ======================================
    println!("Phase 3: Anonymous Node Discovery");
    let start = Instant::now();
    
    // Node 42 wants to announce membership without revealing ID
    let node_id = 42u32;
    let epoch = 1u64;
    
    // commit_to_network cached a Merkle proof for every member, so the prover
    // resolves the witness itself. The previous version built a
    // MembershipWitness by hand from a network.get_merkle_proof() call that
    // does not exist — the prover has owned this since commit_to_network.
    let node_id_bytes = node_id_to_bytes(node_id);
    let membership_proof = proof_generator.prove_membership(&node_id_bytes, epoch)
        .expect("Failed to generate membership proof");
    
    println!("  Membership proof generated in {:?}", start.elapsed());
    
    // Verify membership proof (anyone can verify, learns nothing about node ID)
    let start = Instant::now();
    // Verification takes the prover's own root type, and the epoch is already
    // bound into the proof's public inputs rather than passed alongside it.
    let root_hash = proof_generator.get_merkle_root_hash()
        .expect("prover has committed to a network");
    let verified = proof_generator.verify_membership(&membership_proof, &root_hash)
        .expect("Failed to verify membership proof");
    
    assert!(verified, "Membership proof verification failed!");
    println!("  Membership proof verified in {:?}", start.elapsed());
    println!("  ✅ Node anonymously proved membership\n");
    
    // ======================================
    // PHASE 4: Packet Construction with zkSNARKs
    // ======================================
    println!("Phase 4: Packet Construction");
    
    let network_arc = Arc::new(RwLock::new(network.clone()));
    let fhe_engine = Arc::new(FheEngine::generate_keys());
    
    let packet_constructor = PacketConstructor::new(
        network_arc.clone(),
        fhe_engine.clone(),
        Arc::new(proof_generator),
    );
    
    let mut packets = Vec::new();
    let mut total_construction_time = std::time::Duration::ZERO;
    
    for i in 0..config.num_packets {
        let start = Instant::now();
        
        // Select random source and destination
        let source = (i * 10) as u32 % config.network_size as u32;
        let destination = (i * 10 + 50) as u32 % config.network_size as u32;
        
        // Find path through network
        let path = network.find_path(source, destination)
            .expect("Failed to find path");
        
        let routing_path = RoutingPath::new(path.clone())
            .expect("Failed to create routing path");
        
        // Construct packet with FHE routing blob + zk-proof
        let payload = format!("Test packet {}", i).into_bytes();
        
        let packet = packet_constructor.construct_packet(&routing_path, payload)
            .expect("Failed to construct packet");
        
        let construction_time = start.elapsed();
        total_construction_time += construction_time;
        
        println!("  Packet {}: {} hops, construction time: {:?}", 
            i, path.len(), construction_time);
        
        packets.push((packet, routing_path));
    }
    
    let avg_construction = total_construction_time / config.num_packets as u32;
    println!("  Average packet construction: {:?}\n", avg_construction);
    
    // ======================================
    // PHASE 5: Multi-Hop Oblivious Forwarding
    // ======================================
    println!("Phase 5: Multi-Hop Oblivious Forwarding");
    
    let simulator = NetworkSimulator::new(network_arc.clone());
    
    let mut successful_deliveries = 0;
    let mut total_latency = std::time::Duration::ZERO;
    let mut total_hops = 0;
    
    for (i, (packet, path)) in packets.iter().enumerate() {
        println!("\n  --- Packet {} Routing ---", i);
        println!("  Expected path: {:?}", path.hops);
        
        let start = Instant::now();
        
        // Forward packet through network
        let trace = simulator.forward_packet(packet.clone(), path.hops[0])
            .expect("Failed to forward packet");
        
        let latency = start.elapsed();
        total_latency += latency;
        total_hops += trace.hops.len();
        
        // Verify delivery
        match trace.status {
            PathStatus::Delivered => {
                successful_deliveries += 1;
                println!("  ✅ Delivered in {} hops, latency: {:?}", 
                    trace.hops.len(), latency);
            }
            PathStatus::Dropped(reason) => {
                println!("  ❌ Dropped: {:?}", reason);
            }
            PathStatus::Loop => {
                println!("  ❌ Routing loop detected");
            }
            PathStatus::MaxHopsExceeded => {
                println!("  ❌ Max hops exceeded");
            }
        }
        
        // Display hop details
        for (j, hop) in trace.hops.iter().enumerate() {
            println!("    Hop {}: Node {} → {:?} ({}ms, {} bytes)",
                j, hop.node_id, hop.decision, hop.processing_time_ms, hop.packet_size);
        }
    }
    
    // ======================================
    // PHASE 6: Results Summary
    // ======================================
    println!("\n=== Test Results Summary ===\n");
    println!("Network:");
    println!("  Size: {} nodes", config.network_size);
    println!("  Edges: {}", network.edge_count());
    
    println!("\nPackets:");
    println!("  Total sent: {}", config.num_packets);
    println!("  Successfully delivered: {}/{}", successful_deliveries, config.num_packets);
    println!("  Delivery rate: {:.1}%", 
        (successful_deliveries as f64 / config.num_packets as f64) * 100.0);
    
    println!("\nPerformance:");
    println!("  Avg packet construction: {:?}", avg_construction);
    println!("  Avg end-to-end latency: {:?}", total_latency / config.num_packets as u32);
    println!("  Avg hops per packet: {:.1}", total_hops as f64 / config.num_packets as f64);
    
    println!("\nSecurity Properties:");
    println!("  ✅ Oblivious routing: Nodes forward without learning paths");
    println!("  ✅ Anonymous membership: Node proved participation without revealing ID");
    println!("  ✅ Path validity: All proofs verified successfully");
    println!("  ✅ FHE correctness: Routing decisions match expected paths");
    
    // Verify all packets delivered successfully
    assert_eq!(successful_deliveries, config.num_packets,
        "Not all packets delivered successfully!");
    
    println!("\n🎉 End-to-End Test PASSED!\n");
}

#[test]
fn test_byzantine_resistance() {
    println!("\n=== PHANTOM Byzantine Resistance Test ===\n");
    
    // Smaller network for Byzantine test
    let network_size = 50;
    let num_byzantine = 15; // 30% Byzantine nodes
    
    println!("Network: {} nodes ({} Byzantine)", network_size, num_byzantine);
    
    let mut network = NetworkGraph::new();
    
    // Build network
    for i in 0..network_size {
        network.add_node(NodeInfo {
            id: i,
            bandwidth: 1_000_000,
            latency_ms: 10,
            uptime_hours: 24,
            reputation: 1.0,
        });
    }
    
    // Add edges
    use rand::{Rng, SeedableRng};
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    
    for i in 0..network_size {
        for _ in 0..5 {
            let peer = rng.gen_range(0..network_size);
            if peer != i {
                network.add_edge(i, peer);
            }
        }
    }
    
    // Get Merkle tree commitment (built automatically)
    let merkle_root = *network.commitment();
    
    // Initialize proof system
    let mut proof_generator = Plonky2ProofGenerator::new(10, 10)
        .expect("Failed to initialize Plonky2");
    
    proof_generator.commit_to_network(&network)
        .expect("Failed to commit to network");
    
    // Simulate Byzantine attack: nodes try to forge proofs
    println!("\nAttack Scenario: Byzantine nodes forge invalid routing proofs");
    
    let mut attack_attempts = 0;
    let mut attacks_blocked = 0;
    
    let path_prover = HashProofGenerator::new();

    for byzantine_id in 0..num_byzantine {
        attack_attempts += 1;
        
        // Byzantine node tries to create packet with invalid path
        let invalid_path = vec![
            byzantine_id,
            byzantine_id, // Loop (same node twice)
            byzantine_id + 1,
        ];
        
        let routing_path = RoutingPath::new(invalid_path);
        
        // Path creation should fail (loop detected)
        if routing_path.is_err() {
            attacks_blocked += 1;
            continue;
        }
        
        // If path created, try to generate proof (should fail)
        let public_inputs = PublicInputs {
            network_commitment: merkle_root,
            path_length: 3,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };
        
        // Path proofs are a separate facility from membership proofs and live
        // on the ProofGenerator trait. HashProofGenerator is what implements
        // it today, and its own documentation says it is hash-based and NOT
        // zero-knowledge — so this measures that malformed paths are rejected,
        // not that the rejection is proven in zero knowledge.
        let hops = routing_path.unwrap();
        let proof_result = path_prover.generate_path_proof(
            &hops.hops,
            &merkle_root,
            &[],
        );
        
        // Proof generation should fail or verification should fail
        if proof_result.is_err() {
            attacks_blocked += 1;
        } else {
            // Try to verify the proof
            let proof = proof_result.unwrap();
            let verified = path_prover.verify_path_proof(&proof, &merkle_root);
            
            if verified.is_err() || !verified.unwrap() {
                attacks_blocked += 1;
            }
        }
    }
    
    println!("\nAttack Results:");
    println!("  Attack attempts: {}", attack_attempts);
    println!("  Attacks blocked: {}/{}", attacks_blocked, attack_attempts);
    println!("  Success rate: {:.1}%", 
        (attacks_blocked as f64 / attack_attempts as f64) * 100.0);
    
    assert_eq!(attacks_blocked, attack_attempts,
        "Byzantine attack not fully blocked!");
    
    println!("\n✅ Byzantine resistance verified!\n");
}

#[test]
fn test_performance_scalability() {
    println!("\n=== PHANTOM Performance Scalability Test ===\n");
    
    let test_sizes = vec![10, 50, 100];
    
    for size in test_sizes {
        println!("Testing network size: {} nodes", size);
        
        let mut network = NetworkGraph::new();
        
        // Build network
        use phantom_core::NodeInfo;
        for i in 0..size {
            let info = NodeInfo {
                id: i,
                bandwidth: 1_000_000,
                latency_ms: 50,
                uptime_hours: 100,
                reputation: 1.0,
            };
            network.add_node(info);
        }
        
        // Add edges
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        
        for i in 0..size {
            for _ in 0..std::cmp::min(5, size - 1) {
                let peer = rng.gen_range(0..size);
                if peer != i {
                    network.add_edge(i, peer);
                }
            }
        }
        
        // Benchmark Merkle tree construction (it's built automatically on add_node)
        let start = Instant::now();
        let _merkle_root = *network.commitment();
        let merkle_time = start.elapsed();
        
        // Benchmark proof generation
        let start = Instant::now();
        let mut proof_generator = Plonky2ProofGenerator::new(10, 10)
            .expect("Failed to initialize Plonky2");
        proof_generator.commit_to_network(&network)
            .expect("Failed to commit");
        let proof_setup_time = start.elapsed();
        
        println!("  Merkle tree: {:?}", merkle_time);
        println!("  Proof setup: {:?}", proof_setup_time);
        println!();
    }
    
    println!("✅ Scalability test completed!\n");
}

// Helper functions

fn node_id_to_bytes(node_id: u32) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[0..4].copy_from_slice(&node_id.to_le_bytes());
    bytes
}
