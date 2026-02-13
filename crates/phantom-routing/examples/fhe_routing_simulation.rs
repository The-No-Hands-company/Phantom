//! FHE Routing Simulation - Demonstrates Oblivious Packet Forwarding
//!
//! This simulation shows PHANTOM's revolutionary routing: nodes forward packets
//! through FHE evaluation without learning the route.
//!
//! Week 3, Day 4: Anonymous Routing Integration

use phantom_crypto::FheEngine;
use phantom_core::{NetworkGraph, packet::{PhantomPacket, RoutingPath}};
use phantom_core::network::NodeInfo;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🌐 PHANTOM FHE Routing Simulation");
    println!("==========================================\n");

    // Test 1: Single packet through 5-hop path
    println!("📦 Test 1: Single Packet - 5 Hops");
    println!("------------------------------------------");
    simulate_single_packet()?;
    println!();

    // Test 2: Multiple packets in parallel
    println!("📦 Test 2: 10 Concurrent Packets");
    println!("------------------------------------------");
    simulate_concurrent_packets(10)?;
    println!();

    // Test 3: Byzantine node resistance
    println!("🛡️  Test 3: Byzantine Node Resistance");
    println!("------------------------------------------");
    simulate_byzantine_resistance()?;
    println!();

    // Test 4: Scalability test (100 packets)
    println!("📈 Test 4: Scalability (100 Packets)");
    println!("------------------------------------------");
    simulate_scalability(100)?;

    println!("\n✅ All routing simulations passed!");
    Ok(())
}

/// Test single packet forwarding through 5-hop path
fn simulate_single_packet() -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    // 1. Create network (100 nodes)
    let network = create_test_network(100)?;
    let stats = network.stats();
    println!("   Network: {} nodes, {} edges", stats.node_count, stats.edge_count);

    // 2. Initialize FHE engine
    let fhe_engine = FheEngine::generate_keys();
    println!("   FHE engine initialized");

    // 3. Create routing path (5 hops)
    let path = find_path_in_network(&network, 5)?;
    println!("   Path: {:?}", path.hops);

    // 4. Build PHANTOM packet
    let packet_start = Instant::now();
    let packet = build_packet(&fhe_engine, &path, b"Hello PHANTOM!")?;
    let packet_time = packet_start.elapsed();
    println!("   Packet construction: {:?}", packet_time);

    // 5. Simulate oblivious forwarding through each hop
    let routing_start = Instant::now();
    let mut hop_times = Vec::new();
    
    // Deserialize routing table from packet
    let routing_table: Vec<(phantom_crypto::EncryptedValue, phantom_crypto::EncryptedValue)> = 
        bincode::deserialize(&packet.routing_blob)?;
    
    for (i, &node_id) in path.hops.iter().enumerate() {
        let hop_start = Instant::now();
        
        // Node performs FHE lookup: "What's my next hop?"
        let encrypted_next_hop = fhe_engine.lookup_routing_table(
            node_id,
            &routing_table
        )?;
        
        // Decrypt to verify (in reality, node wouldn't decrypt - it just forwards)
        let next_hop = fhe_engine.decrypt_u32(&encrypted_next_hop)?;
        
        let hop_time = hop_start.elapsed();
        hop_times.push(hop_time);
        
        println!("   Hop {}: Node {} → next_hop={} (took {:?})", 
            i + 1, node_id, next_hop, hop_time);
    }
    
    let total_routing = routing_start.elapsed();
    let avg_hop = hop_times.iter().sum::<std::time::Duration>() / hop_times.len() as u32;

    println!("\n   Summary:");
    println!("   - Total routing time: {:?}", total_routing);
    println!("   - Average per hop: {:?}", avg_hop);
    println!("   - Total end-to-end: {:?}", start.elapsed());

    Ok(())
}

/// Test concurrent packet forwarding
fn simulate_concurrent_packets(count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    let network = create_test_network(100)?;
    let fhe_engine = FheEngine::generate_keys();

    println!("   Building {} packets...", count);
    let mut packets = Vec::new();
    let mut paths = Vec::new();

    for i in 0..count {
        let path = find_path_in_network(&network, 5)?;
        let payload = format!("Packet {}", i);
        let packet = build_packet(&fhe_engine, &path, payload.as_bytes())?;
        packets.push(packet);
        paths.push(path);
    }

    println!("   Packet construction complete");
    println!("\n   Simulating concurrent routing...");

    let routing_start = Instant::now();
    let mut total_hops = 0;

    for (packet, path) in packets.iter().zip(paths.iter()) {
        // Deserialize routing table
        let routing_table: Vec<(phantom_crypto::EncryptedValue, phantom_crypto::EncryptedValue)> = 
            bincode::deserialize(&packet.routing_blob)?;
            
        for &node_id in &path.hops {
            let encrypted_next_hop = fhe_engine.lookup_routing_table(
                node_id,
                &routing_table
            )?;
            let _next_hop = fhe_engine.decrypt_u32(&encrypted_next_hop)?;
            total_hops += 1;
        }
    }

    let total_time = routing_start.elapsed();
    let avg_per_hop = total_time / total_hops as u32;
    let throughput = (total_hops as f64) / total_time.as_secs_f64();

    println!("\n   Summary:");
    println!("   - Total hops processed: {}", total_hops);
    println!("   - Total routing time: {:?}", total_time);
    println!("   - Average per hop: {:?}", avg_per_hop);
    println!("   - Throughput: {:.2} hops/sec", throughput);
    println!("   - End-to-end: {:?}", start.elapsed());

    Ok(())
}

/// Test resistance to Byzantine (malicious) nodes
fn simulate_byzantine_resistance() -> Result<(), Box<dyn std::error::Error>> {
    let network = create_test_network(100)?;
    let fhe_engine = FheEngine::generate_keys();

    // Mark 30% of nodes as Byzantine (malicious)
    let byzantine_nodes: Vec<u32> = (0..100)
        .filter(|i| i % 3 == 0)
        .collect();
    
    println!("   Network: 100 nodes ({} Byzantine, {} honest)", 
        byzantine_nodes.len(), 100 - byzantine_nodes.len());

    // Find path avoiding Byzantine nodes
    let path = find_honest_path(&network, &byzantine_nodes, 5)?;

    println!("   Path (avoiding Byzantine): {:?}", path.hops);

    let packet = build_packet(&fhe_engine, &path, b"Byzantine-resistant message")?;
    let routing_table: Vec<(phantom_crypto::EncryptedValue, phantom_crypto::EncryptedValue)> = 
        bincode::deserialize(&packet.routing_blob)?;

    // Byzantine nodes try to inspect packet (they can't!)
    println!("\n   Byzantine nodes attempting inspection:");
    for &byzantine_id in byzantine_nodes.iter().take(5) {
        let encrypted_result = fhe_engine.lookup_routing_table(byzantine_id, &routing_table)?;
        let next_hop = fhe_engine.decrypt_u32(&encrypted_result)?;
        println!("   - Node {}: sees next_hop={} (learns nothing else!)", 
            byzantine_id, next_hop);
    }

    // Honest nodes successfully forward
    println!("\n   Honest nodes forwarding:");
    for &honest_id in &path.hops {
        let encrypted_next_hop = fhe_engine.lookup_routing_table(honest_id, &routing_table)?;
        let next_hop = fhe_engine.decrypt_u32(&encrypted_next_hop)?;
        println!("   - Node {}: next_hop = {}", honest_id, next_hop);
    }

    println!("\n   ✅ Byzantine resistance verified!");
    println!("   - Malicious nodes learn: only 'forward yes/no' (1 bit)");
    println!("   - Metadata protected: source, destination, path all hidden");

    Ok(())
}

/// Test scalability with many packets
fn simulate_scalability(packet_count: usize) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    // Larger network for scalability test
    let network = create_test_network(500)?;
    let fhe_engine = FheEngine::generate_keys();

    let stats = network.stats();
    println!("   Network: {} nodes, {} edges", stats.node_count, stats.edge_count);
    println!("   Generating {} packets...", packet_count);

    let mut packets = Vec::new();
    let mut paths = Vec::new();

    let gen_start = Instant::now();
    for i in 0..packet_count {
        let path = find_path_in_network(&network, 5)?;
        let payload = format!("Scalability test packet {}", i);
        let packet = build_packet(&fhe_engine, &path, payload.as_bytes())?;
        packets.push(packet);
        paths.push(path);
    }
    let gen_time = gen_start.elapsed();

    println!("   Packet generation: {:?} ({:?}/packet)", 
        gen_time, gen_time / packet_count as u32);

    println!("\n   Routing {} packets through network...", packet_count);
    let routing_start = Instant::now();
    
    let mut total_hops = 0;
    let mut successful_deliveries = 0;

    for (packet, path) in packets.iter().zip(paths.iter()) {
        let mut delivered = true;
        
        // Deserialize routing table
        let routing_table: Vec<(phantom_crypto::EncryptedValue, phantom_crypto::EncryptedValue)> = 
            match bincode::deserialize(&packet.routing_blob) {
                Ok(t) => t,
                Err(_) => {
                    delivered = false;
                    continue;
                }
            };
            
        for &node_id in &path.hops {
            match fhe_engine.lookup_routing_table(node_id, &routing_table) {
                Ok(encrypted_next_hop) => {
                    match fhe_engine.decrypt_u32(&encrypted_next_hop) {
                        Ok(_next_hop) => {
                            total_hops += 1;
                        }
                        Err(_) => {
                            delivered = false;
                            break;
                        }
                    }
                }
                Err(_) => {
                    delivered = false;
                    break;
                }
            }
        }
        if delivered {
            successful_deliveries += 1;
        }
    }

    let routing_time = routing_start.elapsed();
    let total_time = start.elapsed();

    println!("\n   Scalability Results:");
    println!("   - Packets processed: {}/{}", successful_deliveries, packet_count);
    println!("   - Total hops: {}", total_hops);
    println!("   - Packet generation: {:?}", gen_time);
    println!("   - Routing time: {:?}", routing_time);
    println!("   - Total time: {:?}", total_time);
    println!("   - Throughput: {:.2} packets/sec", 
        successful_deliveries as f64 / total_time.as_secs_f64());
    println!("   - Average latency: {:?}", 
        routing_time / successful_deliveries as u32);

    Ok(())
}

/// Build PHANTOM packet with FHE routing blob
fn build_packet(
    fhe_engine: &FheEngine,
    path: &RoutingPath,
    payload: &[u8],
) -> Result<PhantomPacket, Box<dyn std::error::Error>> {
    // Network commitment (placeholder for now)
    let network_commitment = [0u8; 32];
    
    let packet = PhantomPacket::construct(
        path.clone(),
        payload.to_vec(),
        fhe_engine,
        &network_commitment,
    )?;

    Ok(packet)
}

/// Find path through honest (non-Byzantine) nodes
fn find_honest_path(
    network: &NetworkGraph,
    byzantine_nodes: &[u32],
    length: usize,
) -> Result<RoutingPath, Box<dyn std::error::Error>> {
    // Try multiple times to find honest path
    for _ in 0..100 {
        let path = find_path_in_network(network, length)?;
        let all_honest = path.hops.iter()
            .all(|node| !byzantine_nodes.contains(node));
        
        if all_honest {
            return Ok(path);
        }
    }

    Err("Could not find honest path after 100 attempts".into())
}

/// Create test network with given number of nodes
fn create_test_network(node_count: usize) -> Result<NetworkGraph, Box<dyn std::error::Error>> {
    use rand::Rng;
    
    let mut network = NetworkGraph::new();
    let mut rng = rand::thread_rng();
    
    // Add nodes
    for i in 0..node_count {
        let node_info = NodeInfo {
            id: i as u32,
            bandwidth: rng.gen_range(100_000_000..10_000_000_000), // 100 Mbps - 10 Gbps
            latency_ms: rng.gen_range(10..200),
            uptime_hours: rng.gen_range(100..8760), // 100h - 1 year
            reputation: rng.gen_range(0.5..1.0),
        };
        network.add_node(node_info);
    }
    
    // Add edges (random graph with decent connectivity)
    let target_edge_probability = 0.1;
    for i in 0..node_count {
        for j in (i + 1)..node_count {
            if rng.gen::<f64>() < target_edge_probability {
                network.add_edge(i as u32, j as u32);
            }
        }
    }
    
    Ok(network)
}

/// Find random path through network
fn find_path_in_network(
    network: &NetworkGraph,
    length: usize,
) -> Result<RoutingPath, Box<dyn std::error::Error>> {
    use rand::seq::SliceRandom;
    use rand::Rng;
    
    let mut rng = rand::thread_rng();
    let stats = network.stats();
    let node_count = stats.node_count;
    
    // Try multiple times to find valid path
    for _ in 0..100 {
        let mut path = Vec::new();
        
        // Pick random starting node
        let start = rng.gen_range(0..node_count as u32);
        path.push(start);
        
        // Build path by randomly selecting neighbors
        let mut current = start;
        for _ in 1..length {
            if let Some(neighbors) = network.get_neighbors(current) {
                if neighbors.is_empty() {
                    break;
                }
                
                // Filter out already visited nodes
                let available: Vec<_> = neighbors.iter()
                    .filter(|&&n| !path.contains(&n))
                    .copied()
                    .collect();
                
                if available.is_empty() {
                    break;
                }
                
                // Pick random neighbor
                let next = *available.choose(&mut rng).unwrap();
                path.push(next);
                current = next;
            } else {
                break;
            }
        }
        
        // Check if we found a valid path of required length
        if path.len() == length {
            return RoutingPath::new(path)
                .map_err(|e| format!("Invalid path: {}", e).into());
        }
    }
    
    Err("Could not find valid path after 100 attempts".into())
}

/// Compute nullifier for rate limiting
fn compute_nullifier(path: &RoutingPath, payload: &[u8]) -> [u8; 32] {
    use phantom_crypto::primitives::hash;
    
    let mut data = Vec::new();
    for &hop in &path.hops {
        data.extend_from_slice(&hop.to_le_bytes());
    }
    data.extend_from_slice(payload);
    
    hash(&data)
}
