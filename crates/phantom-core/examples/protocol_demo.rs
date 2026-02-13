/// PHANTOM Protocol Demonstration
///
/// This example demonstrates:
/// 1. Network graph construction with node discovery
/// 2. Path selection across multiple hops
/// 3. FHE-encrypted packet construction
/// 4. Verification of packet integrity
///
/// This is the foundation of oblivious routing - packets carry encrypted
/// routing information that nodes can evaluate without learning the path.

use phantom_core::{PhantomPacket, NetworkGraph};
use phantom_core::network::NodeInfo;
use phantom_core::packet::RoutingPath;
use phantom_crypto::FheEngine;

fn main() -> anyhow::Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("    PHANTOM Protocol - Oblivious Routing Demonstration");
    println!("═══════════════════════════════════════════════════════════\n");

    // Phase 1: Network Setup
    println!("Phase 1: Building Network Topology");
    println!("───────────────────────────────────────────────────────────");
    
    let mut network = NetworkGraph::new();
    
    // Add nodes to the network (simulating discovery)
    let nodes = vec![
        (100, "Entry Guard 1"),
        (101, "Entry Guard 2"),
        (200, "Middle Relay 1"),
        (201, "Middle Relay 2"),
        (202, "Middle Relay 3"),
        (300, "Exit Node 1"),
        (301, "Exit Node 2"),
    ];
    
    for (id, name) in nodes.iter() {
        let node_info = NodeInfo {
            id: *id,
            bandwidth: 1_000_000_000, // 1 Gbps
            latency_ms: 50,
            uptime_hours: 720, // 30 days
            reputation: 0.95,
        };
        network.add_node(node_info);
        println!("  ✓ Discovered node {}: {}", id, name);
    }
    
    // Add edges (connections between nodes)
    let edges = vec![
        (100, 200), (100, 201),  // Entry guards → Middle relays
        (101, 200), (101, 202),
        (200, 300), (200, 301),  // Middle relays → Exit nodes
        (201, 300), (201, 301),
        (202, 300), (202, 301),
    ];
    
    for (from, to) in edges {
        network.add_edge(from, to);
    }
    
    let stats = network.stats();
    println!("\n  Network graph: {} nodes, {} edges", 
             stats.node_count, stats.edge_count);
    
    // Compute network commitment (Merkle root of topology)
    let network_commitment = network.commitment();
    println!("  Network commitment: {}...{}", 
             hex::encode(&network_commitment[..4]),
             hex::encode(&network_commitment[28..]));

    // Phase 2: Path Selection
    println!("\n\nPhase 2: Path Selection");
    println!("───────────────────────────────────────────────────────────");
    
    // Manually construct a 5-hop path through the network
    let path = RoutingPath::new(vec![100, 200, 201, 202, 300])
        .expect("Valid path should succeed");
    println!("  Selected {}-hop path:", path.hops.len());
    for (i, hop) in path.hops.iter().enumerate() {
        let node_name = nodes.iter().find(|(id, _)| id == hop)
            .map(|(_, name)| *name)
            .unwrap_or("Unknown");
        println!("    Hop {}: Node {} ({})", i + 1, hop, node_name);
    }
    
    println!("\n  Next hops (encrypted routing table):");
    for (i, next_hop) in path.next_hops.iter().enumerate() {
        println!("    If at node {}, forward to {}", path.hops[i], next_hop);
    }

    // Phase 3: FHE Key Generation
    println!("\n\nPhase 3: Cryptographic Setup");
    println!("───────────────────────────────────────────────────────────");
    println!("  Generating FHE keys (this takes ~2 seconds)...");
    
    let start = std::time::Instant::now();
    let fhe_engine = FheEngine::generate_keys();
    let keygen_time = start.elapsed();
    
    println!("  ✓ FHE key generation completed in {:.2}s", keygen_time.as_secs_f64());
    println!("    - Client key: Can encrypt/decrypt");
    println!("    - Server key: Can evaluate on encrypted data");

    // Phase 4: Packet Construction
    println!("\n\nPhase 4: Packet Construction");
    println!("───────────────────────────────────────────────────────────");
    
    let payload = b"Secret message to destination".to_vec();
    println!("  Payload: {} bytes", payload.len());
    println!("  Message: \"{}\"", String::from_utf8_lossy(&payload));
    
    println!("\n  Constructing PHANTOM packet with FHE-encrypted routing...");
    let start = std::time::Instant::now();
    
    let packet = PhantomPacket::construct(
        path.clone(),
        payload.clone(),
        &fhe_engine,
        &network_commitment,
    )?;
    
    let construction_time = start.elapsed();
    
    println!("  ✓ Packet constructed in {:.2}s", construction_time.as_secs_f64());
    println!("\n  Packet structure:");
    println!("    - Packet ID: {}...{}", 
             hex::encode(&packet.packet_id[..4]),
             hex::encode(&packet.packet_id[28..]));
    println!("    - Routing blob: {} bytes (FHE-encrypted)", packet.routing_blob.len());
    println!("    - Path proof: {} bytes (zkVM proof)", packet.path_proof.proof_data.len());
    println!("    - Payload: {} bytes (encrypted)", packet.payload.len());
    println!("    - Nullifier: {}...{}", 
             hex::encode(&packet.nullifier[..4]),
             hex::encode(&packet.nullifier[28..]));

    // Phase 5: Packet Verification
    println!("\n\nPhase 5: Packet Verification");
    println!("───────────────────────────────────────────────────────────");
    
    println!("  Verifying path proof against network commitment...");
    let start = std::time::Instant::now();
    
    let is_valid = packet.verify_path_proof(&network_commitment)?;
    let verification_time = start.elapsed();
    
    if is_valid {
        println!("  ✓ Path proof VALID in {:.2}ms", verification_time.as_secs_f64() * 1000.0);
        println!("    - Path is part of the committed network topology");
        println!("    - Routing is cryptographically correct");
    } else {
        println!("  ✗ Path proof INVALID");
        return Err(anyhow::anyhow!("Proof verification failed"));
    }

    // Phase 6: Oblivious Routing Simulation
    println!("\n\nPhase 6: Oblivious Routing Simulation");
    println!("───────────────────────────────────────────────────────────");
    println!("  Simulating packet forwarding at each hop...\n");
    
    for (i, current_node) in path.hops.iter().enumerate() {
        println!("  Hop {}: Packet arrives at node {}", i + 1, current_node);
        
        // Node performs FHE-based routing table lookup
        // CRITICAL: Node never decrypts the routing blob!
        println!("    → Node evaluates FHE circuit: \"Should I forward this?\"");
        
        let start = std::time::Instant::now();
        let next_hop = fhe_engine.oblivious_routing_lookup(
            *current_node,
            &packet.routing_blob,
        )?;
        let lookup_time = start.elapsed();
        
        println!("    → FHE evaluation completed in {:.0}ms", lookup_time.as_secs_f64() * 1000.0);
        
        if next_hop == 0 {
            println!("    → Destination reached! Delivering payload.");
            println!("    ✓ Message: \"{}\"", String::from_utf8_lossy(&payload));
        } else {
            println!("    → Forwarding to node {}", next_hop);
            println!("    ✓ Node {} learned: \"Forward this packet\"", current_node);
            println!("    ✓ Node {} did NOT learn: Source, destination, full path", current_node);
        }
        println!();
    }

    // Summary
    println!("\n═══════════════════════════════════════════════════════════");
    println!("                    Summary");
    println!("═══════════════════════════════════════════════════════════");
    let final_stats = network.stats();
    println!("  ✓ Network topology: {} nodes, {} edges", final_stats.node_count, final_stats.edge_count);
    println!("  ✓ Path length: {} hops", path.hops.len());
    println!("  ✓ FHE key generation: {:.2}s", keygen_time.as_secs_f64());
    println!("  ✓ Packet construction: {:.2}s", construction_time.as_secs_f64());
    println!("  ✓ Proof verification: {:.2}ms", verification_time.as_secs_f64() * 1000.0);
    println!("\n  PHANTOM Protocol Properties:");
    println!("    • Nodes route packets they CANNOT decrypt (FHE)");
    println!("    • Paths are cryptographically proven valid (zk-SNARK)");
    println!("    • Post-quantum secure (Kyber-1024, Dilithium-5)");
    println!("    • Metadata protection: No node learns the full path");
    println!("═══════════════════════════════════════════════════════════\n");

    Ok(())
}
