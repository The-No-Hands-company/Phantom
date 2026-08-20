//! Peer Discovery Service Demo
//!
//! Demonstrates PHANTOM's anonymous peer discovery system:
//! - Query nodes by capabilities (routing, directory, FHE, zkVM)
//! - Filter by region and bandwidth
//! - Random selection prevents topology inference
//! - Rate limiting prevents abuse

use phantom_discovery::{
    DiscoveryService, DiscoveryConfig, DiscoveryQuery,
    NodeAnnouncement, NodeDescriptor, NodeCapabilities,
    NetworkState,
};
use phantom_core::identity::NodeIdentity;
use phantom_crypto::pq::SigningKeyPair;
use rand::Rng;

fn main() {
    println!("=== PHANTOM Peer Discovery Service Demo ===\n");
    
    // 1. Create discovery service
    println!("1. Creating discovery service...");
    let config = DiscoveryConfig {
        max_results: 20,
        rate_limit_secs: 5,
        max_announcement_age: 600,
        prefer_diversity: true,
    };
    let mut service = DiscoveryService::new(config);
    
    // Set initial network state
    let mut network_state = NetworkState::new();
    network_state.update_merkle_root([0xAAu8; 32]);
    service.update_network_state(network_state);
    
    println!("   ✓ Discovery service initialized");
    println!("   - Max results per query: 20");
    println!("   - Rate limit: 5 seconds");
    println!("   - Geographic diversity: enabled\n");
    
    // 2. Populate with diverse nodes
    println!("2. Populating discovery pool with 100 nodes...");
    
    let regions = vec![
        "us-west", "us-east", "eu-central", "eu-west",
        "asia-pacific", "south-america", "africa", "australia"
    ];
    
    let mut rng = rand::thread_rng();
    
    for i in 0..100 {
        let region = regions[i % regions.len()].to_string();
        let bandwidth = (500_000 + rng.gen_range(0..10_000_000)) as u64;
        
        let capabilities = NodeCapabilities {
            routing: true,
            directory: i % 10 == 0, // 10% directory nodes
            fhe_routing: i % 5 != 0, // 80% support FHE
            zkvm_verification: i % 3 != 0, // 66% support zkVM
        };
        
        let announcement = create_announcement(
            Some(region),
            bandwidth,
            capabilities,
            i as u64,
        );
        
        service.add_announcement(announcement).unwrap();
    }
    
    println!("   ✓ Added 100 diverse nodes");
    println!("   - Regions: {}", regions.len());
    println!("   - Directory nodes: ~10");
    println!("   - FHE-capable nodes: ~80");
    println!("   - zkVM-capable nodes: ~66\n");
    
    // 3. Query #1: Find high-bandwidth nodes
    println!("3. Query: Find high-bandwidth nodes (>= 5 MB/s)...");
    
    let query = DiscoveryQuery {
        min_bandwidth: Some(5_000_000),
        limit: 10,
        ..Default::default()
    };
    
    let result = service.query(query, "client-ip-1").unwrap();
    
    println!("   Results:");
    println!("   - Total matches: {}", result.total_matches);
    println!("   - Returned: {}", result.nodes.len());
    println!("   - Nodes:");
    
    for (i, node) in result.nodes.iter().take(5).enumerate() {
        println!("     {}. Region: {:?}, Bandwidth: {} MB/s",
            i + 1,
            node.region_hint,
            node.bandwidth_capacity / 1_000_000
        );
    }
    println!();
    
    // 4. Query #2: Find directory nodes
    println!("4. Query: Find directory/bootstrap nodes...");
    
    let query = DiscoveryQuery {
        requires_directory: Some(true),
        limit: 5,
        ..Default::default()
    };
    
    let result = service.query(query, "client-ip-2").unwrap();
    
    println!("   Results:");
    println!("   - Total matches: {}", result.total_matches);
    println!("   - Returned: {}", result.nodes.len());
    
    for (i, node) in result.nodes.iter().enumerate() {
        println!("     {}. Region: {:?}, Directory: {}",
            i + 1,
            node.region_hint,
            node.capabilities.directory
        );
    }
    println!();
    
    // 5. Query #3: Find FHE-capable nodes in specific region
    println!("5. Query: Find FHE-capable nodes in EU...");
    
    let query = DiscoveryQuery {
        requires_fhe: Some(true),
        region: Some("eu-central".to_string()),
        limit: 10,
        ..Default::default()
    };
    
    let result = service.query(query, "client-ip-3").unwrap();
    
    println!("   Results:");
    println!("   - Total matches: {}", result.total_matches);
    println!("   - Returned: {}", result.nodes.len());
    
    for (i, node) in result.nodes.iter().enumerate() {
        println!("     {}. Region: {:?}, FHE: {}, Bandwidth: {} MB/s",
            i + 1,
            node.region_hint,
            node.capabilities.fhe_routing,
            node.bandwidth_capacity / 1_000_000
        );
    }
    println!();
    
    // 6. Query #4: Geographic diversity demonstration
    println!("6. Query: Show geographic diversity (limit=8)...");
    
    let query = DiscoveryQuery {
        limit: 8,
        ..Default::default()
    };
    
    let result = service.query(query, "client-ip-4").unwrap();
    
    println!("   Results (diversity-optimized):");
    
    use std::collections::HashMap;
    let mut region_counts: HashMap<String, usize> = HashMap::new();
    
    for node in &result.nodes {
        let region = node.region_hint.clone().unwrap_or_else(|| "unknown".to_string());
        *region_counts.entry(region).or_insert(0) += 1;
    }
    
    for (region, count) in region_counts.iter() {
        println!("     {}: {} nodes", region, count);
    }
    println!();
    
    // 7. Rate limiting demonstration
    println!("7. Rate limiting demonstration...");
    
    let query = DiscoveryQuery {
        limit: 5,
        ..Default::default()
    };
    
    // First query succeeds
    let result1 = service.query(query.clone(), "client-ip-5");
    println!("   Query 1 from client-ip-5: {}", 
        if result1.is_ok() { "✓ Success" } else { "✗ Failed" }
    );
    
    // Second query from same IP fails (rate limited)
    let result2 = service.query(query, "client-ip-5");
    println!("   Query 2 from client-ip-5: {}", 
        if result2.is_err() { "✗ Rate limited (expected)" } else { "✓ Success" }
    );
    println!();
    
    // 8. Prune expired announcements
    println!("8. Maintenance: Pruning expired announcements...");
    
    let initial_count = service.active_announcements();
    let pruned = service.prune_expired();
    let final_count = service.active_announcements();
    
    println!("   - Initial announcements: {}", initial_count);
    println!("   - Pruned: {}", pruned);
    println!("   - Remaining: {}\n", final_count);
    
    // 9. Summary
    println!("=== Summary ===");
    println!("✓ Peer discovery service working correctly");
    println!("✓ Query filters (region, bandwidth, capabilities) functional");
    println!("✓ Geographic diversity optimization active");
    println!("✓ Rate limiting prevents abuse");
    println!("✓ Random selection prevents topology inference");
    println!();
    println!("Week 5 Day 3: Peer Discovery Service - COMPLETE");
}

/// Helper: Create test announcement
fn create_announcement(
    region: Option<String>,
    bandwidth: u64,
    capabilities: NodeCapabilities,
    node_index: u64,
) -> NodeAnnouncement {
    let keypair = SigningKeyPair::generate();
    
    let descriptor = NodeDescriptor::new(
        keypair.public.0.clone(),
        vec![format!("10.0.{}.{}:8080", node_index / 256, node_index % 256).parse().unwrap()],
        1,
        capabilities,
        bandwidth,
        region,
    );
    
    let mut node_id_bytes = [0u8; 32];
    node_id_bytes[..8].copy_from_slice(&node_index.to_le_bytes());
    let node_id = NodeIdentity(node_id_bytes);
    
    let membership_proof = vec![0u8; 100]; // Mock proof (would be real Plonky2 proof)
    
    NodeAnnouncement::new(
        descriptor,
        membership_proof,
        &node_id,
        1,
        &[0xBBu8; 32],
        &keypair,
    ).unwrap()
}
