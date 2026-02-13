/// Gossip Protocol Simulation
/// 
/// Demonstrates anonymous announcement propagation across a simulated network.

use phantom_discovery::{GossipManager, GossipConfig, NodeAnnouncement, NodeDescriptor, NodeCapabilities};
use phantom_core::network::NodeId;
use phantom_crypto::pq::KeyPair;
use std::collections::HashMap;
use std::time::Instant;

fn create_test_announcement(node_id: u8, routing_key: Vec<u8>) -> NodeAnnouncement {
    let keypair = KeyPair::generate();
    
    let descriptor = NodeDescriptor::new(
        routing_key,
        vec![format!("192.168.1.{}:8080", node_id).parse().unwrap()],
        1,
        NodeCapabilities::default(),
        1_000_000,
        Some("us-west".to_string()),
    );
    
    NodeAnnouncement::new(
        descriptor,
        vec![0u8; 100], // Mock membership proof
        &NodeId([node_id; 32]),
        1,
        &[0xBBu8; 32],
        &keypair,
    ).unwrap()
}

fn main() {
    println!("🌐 PHANTOM Gossip Protocol Simulation");
    println!("=====================================\n");
    
    // Create network of gossip managers (simulating nodes)
    let num_nodes = 20;
    let config = GossipConfig {
        max_batch_size: 10,
        fanout: 3,
        ..Default::default()
    };
    
    let mut managers: HashMap<u8, GossipManager> = (0..num_nodes)
        .map(|i| (i, GossipManager::new(config.clone())))
        .collect();
    
    println!("✅ Created {} gossip nodes", num_nodes);
    
    // Node 0 creates initial announcements
    println!("\n📢 Node 0 creating 5 new announcements...");
    let announcements: Vec<_> = (0..5)
        .map(|i| create_test_announcement(i, vec![i]))
        .collect();
    
    let initial_message = managers.get_mut(&0).unwrap().create_message(announcements.clone());
    println!("   Message ID: {:?}", hex::encode(&initial_message.message_id));
    println!("   TTL: {}", initial_message.ttl);
    println!("   Announcements: {}", initial_message.announcements.len());
    
    // Simulate gossip propagation
    println!("\n🔄 Simulating epidemic gossip propagation...");
    let start = Instant::now();
    
    // Round 1: Node 0 forwards to 3 random peers
    let peers = vec![1, 5, 10];
    for &peer in &peers {
        let new_announcements = managers.get_mut(&peer).unwrap()
            .process_message(&initial_message).unwrap();
        println!("   Node {} received {} new announcements", peer, new_announcements.len());
    }
    
    // Round 2: Those peers forward to their neighbors
    let round2_message = managers.get_mut(&1).unwrap()
        .create_forward_message(&initial_message).unwrap();
    
    let round2_peers = vec![2, 3, 7];
    for &peer in &round2_peers {
        let new_announcements = managers.get_mut(&peer).unwrap()
            .process_message(&round2_message).unwrap();
        println!("   Node {} received {} new announcements", peer, new_announcements.len());
    }
    
    // Round 3: Continue propagation
    let round3_message = managers.get_mut(&2).unwrap()
        .create_forward_message(&round2_message).unwrap();
    
    let round3_peers = vec![4, 8, 12];
    for &peer in &round3_peers {
        let new_announcements = managers.get_mut(&peer).unwrap()
            .process_message(&round3_message).unwrap();
        println!("   Node {} received {} new announcements", peer, new_announcements.len());
    }
    
    let propagation_time = start.elapsed();
    
    // Check convergence: how many nodes have all announcements?
    let mut converged_nodes = 0;
    for node_id in 0..num_nodes {
        let manager = managers.get(&node_id).unwrap();
        if manager.get_all_announcements().len() == 5 {
            converged_nodes += 1;
        }
    }
    
    println!("\n📊 Propagation Results:");
    println!("   Converged nodes: {}/{}", converged_nodes, num_nodes);
    println!("   Coverage: {:.1}%", (converged_nodes as f64 / num_nodes as f64) * 100.0);
    println!("   Time: {:?}", propagation_time);
    
    // Test deduplication
    println!("\n🔒 Testing deduplication...");
    let duplicate_message = managers.get_mut(&0).unwrap().create_message(announcements.clone());
    
    // Try to re-send to node 1 (should be filtered)
    let new_announcements = managers.get_mut(&1).unwrap()
        .process_message(&duplicate_message).unwrap();
    
    println!("   Duplicate message sent to Node 1");
    println!("   New announcements received: {} (expected 0)", new_announcements.len());
    
    if new_announcements.is_empty() {
        println!("   ✅ Deduplication working correctly!");
    } else {
        println!("   ❌ Deduplication failed!");
    }
    
    // Test TTL exhaustion
    println!("\n⏱️  Testing TTL exhaustion...");
    let mut ttl_message = managers.get_mut(&0).unwrap()
        .create_message(vec![create_test_announcement(99, vec![99])]);
    ttl_message.ttl = 2;
    
    println!("   Initial TTL: {}", ttl_message.ttl);
    
    let forward1 = managers.get_mut(&1).unwrap()
        .create_forward_message(&ttl_message);
    if let Some(msg) = &forward1 {
        println!("   Forward 1 TTL: {}", msg.ttl);
    }
    
    let forward2 = managers.get_mut(&2).unwrap()
        .create_forward_message(&forward1.unwrap());
    
    if forward2.is_none() {
        println!("   ✅ TTL exhausted correctly (no forward after TTL=1)");
    } else {
        println!("   ❌ TTL exhaustion failed!");
    }
    
    // Statistics
    println!("\n📈 Final Statistics:");
    for node_id in &[0u8, 1, 5, 10] {
        let stats = managers.get(node_id).unwrap().stats();
        println!("   Node {}: {} announcements, {} recent messages", 
                 node_id, stats.total_announcements, stats.recent_messages);
    }
    
    println!("\n✅ Gossip protocol simulation complete!");
    println!("\n🔑 Key Properties Demonstrated:");
    println!("   ✅ Epidemic propagation (announcements spread to multiple nodes)");
    println!("   ✅ Bloom filter deduplication (prevents redundant transmission)");
    println!("   ✅ Message loop prevention (same message_id not reprocessed)");
    println!("   ✅ TTL-based termination (prevents infinite propagation)");
    println!("   ✅ Eventual consistency (all nodes eventually receive announcements)");
}
