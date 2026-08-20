//! Week 5 Day 5: Multi-Node Discovery Integration Tests
//! 
//! Comprehensive tests simulating real network scenarios:
//! - Multi-node bootstrap and announcement
//! - Gossip protocol propagation across network
//! - Peer discovery with query filters
//! - Byzantine attack resistance
//! - Network churn (nodes joining/leaving)

use phantom_discovery::{
    NetworkState, Announcer, Announcement, VerificationResult,
    GossipMessage, BloomFilter,
    DiscoveryService, DiscoveryConfig, DiscoveryQuery,
    NodeAnnouncement, NodeDescriptor, NodeCapabilities,
};
use phantom_core::identity::NodeIdentity;
use phantom_crypto::pq::SigningKeyPair;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

/// Simulated network node with full discovery stack
struct SimulatedNode {
    id: u64,
    announcer: Announcer,
    network_state: NetworkState,
}

impl SimulatedNode {
    fn new(id: u64) -> Self {
        let network_state = NetworkState::new();
        Self {
            id,
            announcer: Announcer::new(10_000),
            network_state,
        }
    }

    fn create_announcement(&self, epoch: u64) -> Announcement {
        let nullifier = self.create_nullifier(epoch);
        Announcement::new(
            vec![0xDE, 0xAD, 0xBE, 0xEF],
            nullifier,
            epoch,
            self.network_state.merkle_root,
        )
    }

    fn create_nullifier(&self, epoch: u64) -> [u8; 32] {
        let mut nullifier = [0u8; 32];
        nullifier[0..8].copy_from_slice(&self.id.to_le_bytes());
        nullifier[8..16].copy_from_slice(&epoch.to_le_bytes());
        nullifier
    }
}

/// Test 1: 100-Node Network Bootstrap
#[test]
fn test_100_node_bootstrap() {
    println!("\n=== Test 1: 100-Node Bootstrap ===");
    
    let start = Instant::now();
    let mut nodes: Vec<SimulatedNode> = (0..100)
        .map(|id| SimulatedNode::new(id))
        .collect();
    
    // Setup shared network state
    let merkle_root = [0xAB; 32];
    for node in &mut nodes {
        node.network_state.update_merkle_root(merkle_root);
        node.network_state.set_node_count(100);
    }
    
    let current_epoch = NetworkState::current_epoch();
    
    // Each node announces itself
    let mut total_accepted = 0;
    for node in &mut nodes {
        let announcement = node.create_announcement(current_epoch);
        let result = node.announcer.verify_and_register(&announcement, &node.network_state)
            .expect("Verification failed");
        
        if result == VerificationResult::Valid {
            total_accepted += 1;
        }
    }
    
    let elapsed = start.elapsed();
    
    println!("✓ Bootstrap complete in {:?}", elapsed);
    println!("  Accepted: {}/100 announcements", total_accepted);
    println!("  Throughput: {:.0} nodes/sec", 100.0 / elapsed.as_secs_f64());
    
    assert_eq!(total_accepted, 100);
    assert!(elapsed.as_secs() < 5, "Bootstrap should complete in <5 seconds");
}

/// Test 2: Gossip Propagation Simulation
#[test]
fn test_gossip_propagation() {
    println!("\n=== Test 2: Gossip Propagation ===");
    
    let mut nodes: Vec<SimulatedNode> = (0..50)
        .map(|id| SimulatedNode::new(id))
        .collect();
    
    // Setup network topology: each node connected to 5 random peers
    let topology = build_random_topology(50, 5);
    
    // Node 0 creates a message
    // GossipMessage carries a batch of announcements and a bloom filter of
    // what the sender has already seen. This test measures propagation
    // topology rather than payload handling, so the batch is empty and the id
    // is fixed — what matters is that the same message reaches every node.
    let original_message = GossipMessage {
        announcements: Vec::new(),
        bloom_filter: BloomFilter::new(),
        message_id: [0xCA; 16],
        ttl: 8,
    };
    
    let mut seen_by: HashSet<u64> = HashSet::new();
    seen_by.insert(0);
    
    // Track message propagation
    let mut message_queue: Vec<(u64, GossipMessage)> = vec![(0, original_message)];
    let mut rounds = 0;
    let max_rounds = 10;
    
    while !message_queue.is_empty() && rounds < max_rounds {
        rounds += 1;
        let mut next_queue = Vec::new();
        
        for (sender_id, message) in message_queue {
            // Get neighbors of sender
            if let Some(neighbors) = topology.get(&sender_id) {
                for &neighbor_id in neighbors {
                    if !seen_by.contains(&neighbor_id) {
                        seen_by.insert(neighbor_id);
                        next_queue.push((neighbor_id, message.clone()));
                    }
                }
            }
        }
        
        message_queue = next_queue;
    }
    
    let coverage = (seen_by.len() as f64 / 50.0) * 100.0;
    
    println!("✓ Gossip propagation complete");
    println!("  Rounds: {}", rounds);
    println!("  Coverage: {:.1}% ({}/50 nodes)", coverage, seen_by.len());
    println!("  Avg hops: {:.2}", rounds as f64);
    
    assert!(coverage >= 90.0, "Gossip should reach >90% of network");
    assert!(rounds <= 8, "Gossip should complete in <8 rounds for 50 nodes");
}

/// Test 3: Peer Discovery Query Performance
///
/// Rewritten against the real DiscoveryService. The previous version called
/// `PeerDiscoveryService::register_peer` and `query_random_peers`, neither of
/// which exists — no type by that name was ever written, so this test had
/// never compiled, let alone measured anything.
///
/// The real service stores signed NodeAnnouncements and answers filtered
/// queries, so that is what is measured here: population is excluded from the
/// timing, and each query comes from a distinct requester because the service
/// rate-limits to one query per IP per 10 seconds.
#[test]
fn test_peer_discovery_queries() {
    println!("\n=== Test 3: Peer Discovery Queries ===");

    let mut discovery = DiscoveryService::new(DiscoveryConfig::default());

    // Populate with 200 announcements. Each one costs a Dilithium-5
    // signature, so this is deliberately smaller than the 1000 the old test
    // claimed to register without ever signing anything.
    let population = 200usize;
    for i in 0..population {
        discovery
            .add_announcement(build_announcement(i as u64))
            .expect("announcement should be accepted");
    }
    assert_eq!(discovery.active_announcements(), population);

    let query = || DiscoveryQuery {
        requires_routing: Some(true),
        requires_directory: None,
        requires_fhe: None,
        requires_zkvm: None,
        min_bandwidth: None,
        region: None,
        limit: 10,
        fresh_only: true,
    };

    let start = Instant::now();

    let queries = 100;
    let mut total_peers = 0;
    for i in 0..queries {
        // A distinct requester per query: one IP would be rate-limited after
        // the first, and every subsequent result would be an error rather
        // than a measurement.
        let ip = format!("10.0.{}.{}", i / 256, i % 256);
        let result = discovery.query(query(), &ip).expect("query should succeed");
        total_peers += result.nodes.len();
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed.as_micros() / queries as u128;

    println!("✓ Query performance");
    println!("  Total queries: {}", queries);
    println!("  Total peers returned: {}", total_peers);
    println!("  Avg latency: {}µs per query", avg_latency);

    // Every query asked for 10 and the pool holds 200 matching nodes, so a
    // short result means the filter or the sampling is wrong, not that the
    // network is small.
    assert_eq!(
        total_peers,
        queries * 10,
        "each query should return its full limit from a pool of {}",
        population
    );
    assert!(
        avg_latency < 10_000,
        "queries should stay well under 10ms; measured {}µs",
        avg_latency
    );
}

/// Build a signed announcement for a synthetic node.
///
/// Each node gets its own signing key and its own secret identity, which is
/// what the real protocol does — the routing index is public, the identity
/// behind the nullifier is not.
fn build_announcement(index: u64) -> NodeAnnouncement {
    let keypair = SigningKeyPair::generate();

    let descriptor = NodeDescriptor::new(
        keypair.public.0.clone(),
        vec![format!("10.1.{}.{}:8080", index / 256, index % 256)
            .parse()
            .unwrap()],
        1,
        NodeCapabilities::default(),
        1_000_000,
        Some("us-west".to_string()),
    );

    let mut identity_bytes = [0u8; 32];
    identity_bytes[..8].copy_from_slice(&index.to_le_bytes());
    let identity = NodeIdentity(identity_bytes);

    NodeAnnouncement::new(
        descriptor,
        vec![0u8; 100], // Mock membership proof
        &identity,
        1,
        &[0xBBu8; 32],
        &keypair,
    )
    .expect("announcement construction should succeed")
}

/// Test 4: Byzantine Attack Resistance
#[test]
fn test_byzantine_attack_resistance() {
    println!("\n=== Test 4: Byzantine Attack Resistance ===");
    
    let mut nodes: Vec<SimulatedNode> = (0..100)
        .map(|id| SimulatedNode::new(id))
        .collect();
    
    // Setup network
    let merkle_root = [0xAB; 32];
    for node in &mut nodes {
        node.network_state.update_merkle_root(merkle_root);
    }
    
    let current_epoch = NetworkState::current_epoch();
    
    // Honest nodes announce once
    for i in 0..70 {
        let announcement = nodes[i].create_announcement(current_epoch);
        let state = nodes[i].network_state.clone();
        nodes[i].announcer.verify_and_register(&announcement, &state).unwrap();
    }
    
    // Byzantine nodes (30%) attempt spam attack
    let mut spam_blocked = 0;
    for i in 70..100 {
        // First valid announcement
        let announcement = nodes[i].create_announcement(current_epoch);
        let state = nodes[i].network_state.clone();
        nodes[i].announcer.verify_and_register(&announcement, &state).unwrap();
        
        // Attempt 10 spam announcements
        for _ in 0..10 {
            let announcement = nodes[i].create_announcement(current_epoch);
            let result = nodes[i].announcer.verify_and_register(&announcement, &state).unwrap();
            if result == VerificationResult::Duplicate {
                spam_blocked += 1;
            }
        }
    }
    
    println!("✓ Byzantine resistance");
    println!("  Honest nodes: 70");
    println!("  Byzantine nodes: 30");
    println!("  Spam attempts: 300");
    println!("  Spam blocked: {}", spam_blocked);
    println!("  Block rate: {:.1}%", (spam_blocked as f64 / 300.0) * 100.0);
    
    assert_eq!(spam_blocked, 300, "All spam should be blocked");
}

/// Test 5: Network Churn (Nodes Joining/Leaving)
#[test]
fn test_network_churn() {
    println!("\n=== Test 5: Network Churn ===");
    
    let mut network_state = NetworkState::new();
    network_state.update_merkle_root([0xAB; 32]);
    
    let mut announcer = Announcer::new(10_000);
    
    // Epochs are wall-clock derived (unix_time / EPOCH_DURATION_SECS), and
    // verify_and_register evicts nullifiers older than the *current* epoch.
    // Announcing at epochs 0..10 meant every nullifier was ~2.9 million epochs
    // stale the moment it was written, so the registry evicted all 200 and the
    // final count was 0. The eviction was correct; the epoch numbers were toy.
    let base_epoch = NetworkState::current_epoch();

    // Initial network: 100 nodes
    for epoch in base_epoch..base_epoch + 10 {
        network_state.epoch = epoch;
        
        // 20% of nodes churn each epoch
        let churned_nodes = 20;
        
        for i in 0..churned_nodes {
            let nullifier = create_nullifier(epoch * 100 + i);
            let announcement = Announcement::new(
                vec![0xDE, 0xAD, 0xBE, 0xEF],
                nullifier,
                epoch,
                network_state.merkle_root,
            );
            announcer.verify_and_register(&announcement, &network_state).unwrap();
        }
    }
    
    println!("✓ Network churn simulation");
    println!("  Epochs: 10");
    println!("  Churn per epoch: 20 nodes");
    println!("  Total announcements: 200");
    println!("  Nullifier set size: {}", announcer.nullifier_count());
    
    assert_eq!(announcer.nullifier_count(), 200);
}

/// Test 6: Large-Scale Network (1000 Nodes)
#[test]
fn test_1000_node_network() {
    println!("\n=== Test 6: 1000-Node Network ===");
    
    let start = Instant::now();
    
    let mut network_state = NetworkState::new();
    network_state.update_merkle_root([0xAB; 32]);
    network_state.set_node_count(1000);
    
    let mut announcer = Announcer::new(10_000);
    let current_epoch = NetworkState::current_epoch();
    
    // All nodes announce
    for i in 0..1000 {
        let nullifier = create_nullifier(i);
        let announcement = Announcement::new(
            vec![0xDE, 0xAD, 0xBE, 0xEF],
            nullifier,
            current_epoch,
            network_state.merkle_root,
        );
        announcer.verify_and_register(&announcement, &network_state).unwrap();
    }
    
    let elapsed = start.elapsed();
    
    println!("✓ 1000-node network initialized in {:?}", elapsed);
    println!("  Throughput: {:.0} announcements/sec", 1000.0 / elapsed.as_secs_f64());
    println!("  Avg latency: {:.2}ms per announcement", elapsed.as_secs_f64() * 1000.0 / 1000.0);
    
    assert_eq!(announcer.nullifier_count(), 1000);
    assert!(elapsed.as_secs() < 10, "Should complete in <10 seconds");
}

/// Test 7: Epoch Transition
#[test]
fn test_epoch_transition() {
    println!("\n=== Test 7: Epoch Transition ===");
    
    let mut network_state = NetworkState::new();
    network_state.update_merkle_root([0xAB; 32]);
    
    let mut announcer = Announcer::new(10_000);
    
    // As in the churn test: epoch 0 is ~2.9 million epochs in the past, so an
    // announcement stamped with it is expired on arrival and the transition
    // being tested never gets a chance to happen.
    let base_epoch = NetworkState::current_epoch();
    network_state.epoch = base_epoch;

    // Nodes announce in the current epoch
    for i in 0..100 {
        let nullifier = create_nullifier_with_epoch(i, base_epoch);
        let announcement = Announcement::new(
            vec![0xDE, 0xAD, 0xBE, 0xEF],
            nullifier,
            base_epoch,
            network_state.merkle_root,
        );
        announcer.verify_and_register(&announcement, &network_state).unwrap();
    }
    
    // Epoch transition: the same nodes announce in the next epoch
    for i in 0..100 {
        let nullifier = create_nullifier_with_epoch(i, base_epoch + 1);
        let announcement = Announcement::new(
            vec![0xDE, 0xAD, 0xBE, 0xEF],
            nullifier,
            base_epoch + 1,
            network_state.merkle_root,
        );
        let result = announcer.verify_and_register(&announcement, &network_state).unwrap();
        assert_eq!(result, VerificationResult::Valid, "Epoch transition should allow re-announcement");
    }
    
    println!("✓ Epoch transition successful");
    println!("  Epoch 0 announcements: 100");
    println!("  Epoch 1 announcements: 100");
    println!("  Total nullifiers: {}", announcer.nullifier_count());
    
    assert_eq!(announcer.nullifier_count(), 200);
}

// Helper Functions

fn create_nullifier(id: u64) -> [u8; 32] {
    let mut nullifier = [0u8; 32];
    nullifier[0..8].copy_from_slice(&id.to_le_bytes());
    nullifier
}

fn create_nullifier_with_epoch(id: u64, epoch: u64) -> [u8; 32] {
    let mut nullifier = [0u8; 32];
    nullifier[0..8].copy_from_slice(&id.to_le_bytes());
    nullifier[8..16].copy_from_slice(&epoch.to_le_bytes());
    nullifier
}

/// Build random network topology
/// A random peer graph — which this did not previously build.
///
/// The old version connected node `i` to `i+1 ..= i+k` (mod n): a directed
/// ring lattice, entirely deterministic despite the name. Every edge points
/// forward by at most `k`, so a broadcast advances `k` positions per round and
/// needs ceil((n-1)/k) rounds — 10 for 50 nodes with 5 peers, which is exactly
/// what the propagation test measured and exactly why its `rounds <= 8`
/// assertion failed. That assertion was written for the graph the name
/// describes; the graph was a worst case that gossip would never see.
///
/// Seeded, so the test is reproducible: an unseeded random topology would make
/// the round count vary run to run and the assertion flaky.
fn build_random_topology(num_nodes: usize, connections_per_node: usize) -> HashMap<u64, Vec<u64>> {
    use rand::seq::SliceRandom;
    use rand::SeedableRng;

    let mut rng = rand::rngs::StdRng::seed_from_u64(0x9E3779B9);
    let mut topology = HashMap::new();

    for node_id in 0..num_nodes as u64 {
        let mut candidates: Vec<u64> = (0..num_nodes as u64)
            .filter(|&other| other != node_id)
            .collect();
        candidates.shuffle(&mut rng);
        candidates.truncate(connections_per_node);
        topology.insert(node_id, candidates);
    }

    topology
}
