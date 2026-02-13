//! Multi-Hop Forwarding Protocol Demo
//!
//! Demonstrates complete packet forwarding through a simulated PHANTOM network
//! with wire format serialization and multi-hop routing.

use phantom_routing::{NetworkSimulator, PathTrace, PathStatus};
use phantom_core::{PhantomPacket, NetworkGraph, packet::RoutingPath};
use phantom_core::network::NodeInfo;
use phantom_crypto::FheEngine;
use std::sync::{Arc, RwLock};

fn main() -> anyhow::Result<()> {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║   PHANTOM Multi-Hop Forwarding Protocol Demonstration    ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");
    
    // Step 1: Create a test network
    println!("Step 1: Building test network...");
    let network = create_star_network()?;
    
    // Step 2: Setup network simulator
    println!("Step 2: Initializing network simulator...");
    let mut simulator = NetworkSimulator::new(network.clone());
    
    // Add all nodes to simulator
    let node_ids: Vec<u32> = (1..=10).collect();
    for &node_id in &node_ids {
        simulator.add_node(node_id);
    }
    println!("  ✓ Added {} nodes to simulator\n", node_ids.len());
    
    // Step 3: Create and forward test packets
    println!("Step 3: Testing packet forwarding...\n");
    
    // Test 1: 3-hop path
    test_forwarding(
        &simulator,
        vec![1, 5, 9],
        "Short 3-hop path",
    )?;
    
    // Test 2: 5-hop path
    test_forwarding(
        &simulator,
        vec![1, 3, 5, 7, 9],
        "Medium 5-hop path",
    )?;
    
    // Test 3: Maximum 7-hop path
    test_forwarding(
        &simulator,
        vec![1, 2, 3, 4, 5, 6, 7],
        "Maximum 7-hop path",
    )?;
    
    // Step 4: Display network statistics
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                   Network Statistics                      ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");
    
    let stats = simulator.network_stats();
    let mut total_received = 0;
    let mut total_forwarded = 0;
    let mut total_delivered = 0;
    
    for node_id in 1..=10 {
        if let Some(node_stats) = stats.get(&node_id) {
            total_received += node_stats.packets_received;
            total_forwarded += node_stats.packets_forwarded;
            total_delivered += node_stats.packets_delivered;
            
            if node_stats.packets_received > 0 {
                println!("Node {}: received={}, forwarded={}, delivered={}",
                    node_id,
                    node_stats.packets_received,
                    node_stats.packets_forwarded,
                    node_stats.packets_delivered
                );
            }
        }
    }
    
    println!("\nTOTAL: received={}, forwarded={}, delivered={}",
        total_received, total_forwarded, total_delivered);
    
    println!("\n✅ Multi-hop forwarding protocol demonstration complete!");
    
    Ok(())
}

/// Create a star network topology (all nodes connect to central hub)
fn create_star_network() -> anyhow::Result<Arc<RwLock<NetworkGraph>>> {
    let network = Arc::new(RwLock::new(NetworkGraph::new()));
    
    // Add 10 nodes
    for id in 1..=10 {
        network.write().unwrap().add_node(NodeInfo {
            id,
            bandwidth: 1_000_000,
            latency_ms: 50,
            uptime_hours: 24,
            reputation: 0.9,
        });
    }
    
    // Create full mesh for flexibility - each node connects to all others
    for from in 1..=10 {
        for to in 1..=10 {
            if from < to {
                network.write().unwrap().add_edge(from, to);
            }
        }
    }
    
    Ok(network)
}

/// Test packet forwarding on a specific path
fn test_forwarding(
    simulator: &NetworkSimulator,
    path_nodes: Vec<u32>,
    description: &str,
) -> anyhow::Result<()> {
    println!("─────────────────────────────────────────────────────────────");
    println!("Test: {}", description);
    println!("Path: {:?}", path_nodes);
    println!("─────────────────────────────────────────────────────────────");
    
    // Create path
    let path = RoutingPath::new(path_nodes.clone())?;
    
    // Validate path against network
    let network = simulator.network().read().unwrap();
    if !path.validate(&network) {
        return Err(anyhow::anyhow!("Path validation failed"));
    }
    let commitment = network.commitment().clone();
    drop(network);
    
    // Create packet
    let fhe_engine = simulator.fhe_engine().clone();
    let payload = format!("Test message for {} hops", path_nodes.len()).into_bytes();
    
    let start = std::time::Instant::now();
    let packet = PhantomPacket::construct(
        path,
        payload.clone(),
        &fhe_engine,
        &commitment,
    )?;
    let construction_time = start.elapsed();
    
    println!("  Packet construction: {:?}", construction_time);
    println!("  Packet size: {} bytes", packet.size());
    println!("  Packet ID: {:02x}{:02x}...{:02x}{:02x}",
        packet.packet_id[0], packet.packet_id[1],
        packet.packet_id[30], packet.packet_id[31]);
    
    // Forward packet through network
    let entry_node = path_nodes[0];
    let start = std::time::Instant::now();
    let trace = simulator.forward_packet(packet.clone(), entry_node)?;
    let total_time = start.elapsed();
    
    // Display results
    println!("\n  Forwarding Results:");
    println!("  ├─ Hops traversed: {}", trace.hops.len());
    println!("  ├─ Total latency: {} ms", trace.total_latency_ms);
    println!("  ├─ Total bandwidth: {} bytes", trace.total_bytes);
    println!("  ├─ Wall clock time: {:?}", total_time);
    println!("  └─ Status: {:?}", trace.status);
    
    if trace.status == PathStatus::Delivered {
        println!("\n  ✓ Packet delivered successfully!");
        
        // Verify delivery
        if let Some(delivered_to) = simulator.was_delivered(&packet.packet_id) {
            println!("  ✓ Delivery confirmed at node {}", delivered_to);
        }
    } else {
        println!("\n  ✗ Packet not delivered: {:?}", trace.status);
    }
    
    // Show per-hop details
    println!("\n  Per-Hop Details:");
    for (i, hop) in trace.hops.iter().enumerate() {
        println!("    Hop {}: Node {} - {}ms - {} bytes - {:?}",
            i + 1,
            hop.node_id,
            hop.processing_time_ms,
            hop.packet_size,
            hop.decision
        );
    }
    
    println!();
    
    Ok(())
}
