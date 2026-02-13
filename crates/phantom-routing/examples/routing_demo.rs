/// PHANTOM Routing Engine Demonstration
///
/// Shows the oblivious packet forwarding engine in action:
/// - Multi-node network simulation
/// - Packet forwarding through the network
/// - Replay attack detection
/// - Performance statistics

use phantom_routing::{ObliviousForwarder, RoutingDecision, DropReason};
use phantom_core::{PhantomPacket, NetworkGraph, packet::RoutingPath};
use phantom_core::network::NodeInfo;
use phantom_crypto::FheEngine;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

fn main() -> anyhow::Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("       PHANTOM Routing Engine - Forwarding Demo");
    println!("═══════════════════════════════════════════════════════════\n");

    // Phase 1: Network Setup
    println!("Phase 1: Network Setup");
    println!("───────────────────────────────────────────────────────────");
    
    let network = Arc::new(RwLock::new(NetworkGraph::new()));
    
    // Add nodes
    let node_ids = vec![100, 200, 300, 400, 500];
    for &id in &node_ids {
        let mut net = network.write().unwrap();
        net.add_node(NodeInfo {
            id,
            bandwidth: 1_000_000_000,
            latency_ms: 50,
            uptime_hours: 720,
            reputation: 0.95,
        });
    }
    
    // Add edges (full mesh for simplicity)
    for &from in &node_ids {
        for &to in &node_ids {
            if from != to {
                network.write().unwrap().add_edge(from, to);
            }
        }
    }
    
    let stats = network.read().unwrap().stats();
    println!("  ✓ Network: {} nodes, {} edges", stats.node_count, stats.edge_count);

    // Phase 2: FHE Setup
    println!("\nPhase 2: Cryptographic Setup");
    println!("───────────────────────────────────────────────────────────");
    println!("  Generating FHE keys...");
    
    let fhe_engine = Arc::new(FheEngine::generate_keys());
    println!("  ✓ FHE keys generated");

    // Phase 3: Create Forwarders
    println!("\nPhase 3: Creating Oblivious Forwarders");
    println!("───────────────────────────────────────────────────────────");
    
    let mut forwarders = HashMap::new();
    for &node_id in &node_ids {
        let forwarder = ObliviousForwarder::new(
            node_id,
            fhe_engine.clone(),
            network.clone(),
        );
        forwarders.insert(node_id, forwarder);
        println!("  ✓ Forwarder created for node {}", node_id);
    }

    // Phase 4: Packet Construction
    println!("\nPhase 4: Packet Construction");
    println!("───────────────────────────────────────────────────────────");
    
    let path = RoutingPath::new(vec![100, 200, 300, 400, 500])?;
    let payload = b"Secret data traveling through the network".to_vec();
    let commitment = network.read().unwrap().commitment().clone();
    
    println!("  Path: {} → {} → {} → {} → {}", 
             path.hops[0], path.hops[1], path.hops[2], path.hops[3], path.hops[4]);
    println!("  Payload: {} bytes", payload.len());
    
    let packet = PhantomPacket::construct(
        path.clone(),
        payload.clone(),
        &fhe_engine,
        &commitment,
    )?;
    
    println!("  ✓ Packet constructed");
    println!("    - Routing blob: {} bytes", packet.routing_blob.len());
    println!("    - Proof: {} bytes", packet.path_proof.proof_data.len());

    // Phase 5: Multi-Hop Forwarding
    println!("\nPhase 5: Multi-Hop Packet Forwarding");
    println!("═══════════════════════════════════════════════════════════");
    
    let mut current_hop = 0;
    let mut total_fhe_time = 0u128;
    
    for &node_id in &path.hops {
        current_hop += 1;
        println!("\nHop {}: Node {}", current_hop, node_id);
        println!("───────────────────────────────────────────────────────────");
        
        let forwarder = forwarders.get(&node_id)
            .expect("Forwarder not found");
        
        println!("  Processing packet...");
        let start = std::time::Instant::now();
        let decision = forwarder.process_packet(&packet)?;
        let elapsed = start.elapsed();
        total_fhe_time += elapsed.as_millis();
        
        println!("  FHE evaluation: {:.0}ms", elapsed.as_millis());
        
        match decision {
            RoutingDecision::Forward(next_hop) => {
                println!("  ✓ FORWARD to node {}", next_hop);
                println!("    Node {} learned: \"Forward this packet\"", node_id);
                println!("    Node {} did NOT learn: Source, destination, full path", node_id);
            }
            RoutingDecision::Deliver => {
                println!("  ✓ DELIVER to application");
                println!("    Payload: \"{}\"", String::from_utf8_lossy(&payload));
                println!("    Destination reached successfully!");
            }
            RoutingDecision::Drop(reason) => {
                println!("  ✗ DROP: {:?}", reason);
                return Err(anyhow::anyhow!("Packet dropped: {:?}", reason));
            }
        }
        
        // Show forwarder stats
        let fwd_stats = forwarder.stats();
        println!("  Stats: Received={}, Forwarded={}, Delivered={}, Dropped={}", 
                 fwd_stats.packets_received, 
                 fwd_stats.packets_forwarded,
                 fwd_stats.packets_delivered,
                 fwd_stats.packets_dropped);
    }

    // Phase 6: Replay Attack Test
    println!("\n\nPhase 6: Replay Attack Detection");
    println!("═══════════════════════════════════════════════════════════");
    println!("  Attempting to replay the same packet...");
    
    let replay_forwarder = forwarders.get(&100).unwrap();
    let replay_decision = replay_forwarder.process_packet(&packet)?;
    
    match replay_decision {
        RoutingDecision::Drop(DropReason::ReplayAttack) => {
            println!("  ✓ Replay attack DETECTED and BLOCKED");
            println!("    Nullifier deduplication working correctly");
        }
        _ => {
            println!("  ✗ Replay attack NOT detected!");
            return Err(anyhow::anyhow!("Replay attack should have been blocked"));
        }
    }

    // Summary
    println!("\n\n═══════════════════════════════════════════════════════════");
    println!("                      Summary");
    println!("═══════════════════════════════════════════════════════════");
    println!("  ✓ Network: {} nodes, {} edges", stats.node_count, stats.edge_count);
    println!("  ✓ Path length: {} hops", path.hops.len());
    println!("  ✓ Total FHE time: {:.2}s", total_fhe_time as f64 / 1000.0);
    println!("  ✓ Avg per-hop latency: {:.0}ms", total_fhe_time as f64 / path.hops.len() as f64);
    println!("  ✓ Replay attack: Blocked");
    
    println!("\n  Oblivious Routing Properties:");
    println!("    • Each node evaluated FHE circuit on encrypted routing table");
    println!("    • No node learned source, destination, or full path");
    println!("    • Nullifier prevents replay attacks");
    println!("    • Path cryptographically proven valid");
    println!("═══════════════════════════════════════════════════════════\n");

    Ok(())
}
