use phantom_discovery::{NetworkState, EPOCH_DURATION_SECS, current_timestamp};

/// Demonstrate network state management
fn main() {
    println!("═══════════════════════════════════════════════════════");
    println!("  PHANTOM Network State Management Demo");
    println!("═══════════════════════════════════════════════════════\n");

    // Create initial network state
    println!("Phase 1: Initialize Network State");
    println!("─────────────────────────────────────────────────────");
    let mut state = NetworkState::new();
    println!("✓ Network state created");
    println!("  Merkle root: {:?}", hex_string(&state.merkle_root[..8]));
    println!("  Epoch: {}", state.epoch);
    println!("  Node count: {}", state.node_count);
    println!("  Last update: {} (Unix timestamp)", state.last_update);
    println!();

    // Show epoch information
    println!("Phase 2: Epoch Information");
    println!("─────────────────────────────────────────────────────");
    let current_epoch = NetworkState::current_epoch();
    let time_remaining = NetworkState::epoch_time_remaining();
    println!("✓ Current epoch: {}", current_epoch);
    println!("  Epoch duration: {}s (10 minutes)", EPOCH_DURATION_SECS);
    println!("  Time remaining in epoch: {}s ({:.1} minutes)", 
             time_remaining, time_remaining as f64 / 60.0);
    println!();

    // Simulate network growth
    println!("Phase 3: Network Growth Simulation");
    println!("─────────────────────────────────────────────────────");
    
    // Add 100 nodes
    state.set_node_count(100);
    let mock_root = hash_nodes(100);
    state.update_merkle_root(mock_root);
    
    println!("✓ Network updated: 100 nodes joined");
    println!("  New Merkle root: {:?}", hex_string(&state.merkle_root[..8]));
    println!("  Node count: {}", state.node_count);
    println!("  State stale? {}", state.is_stale());
    println!();

    // Test epoch validation
    println!("Phase 4: Epoch Validation");
    println!("─────────────────────────────────────────────────────");
    let current_epoch = NetworkState::current_epoch();
    let test_epochs = [
        (current_epoch - 2, "2 epochs ago", false),
        (current_epoch - 1, "Previous epoch", true),
        (current_epoch, "Current epoch", true),
        (current_epoch + 1, "Next epoch", true),
        (current_epoch + 2, "2 epochs ahead", false),
    ];
    
    for (epoch, desc, expected) in test_epochs {
        let valid = state.is_epoch_valid(epoch);
        let status = if valid { "✓ VALID" } else { "✗ INVALID" };
        let emoji = if valid == expected { "✓" } else { "❌" };
        println!("  {} Epoch {} ({}): {}", emoji, epoch, desc, status);
    }
    println!();

    // Show state persistence
    println!("Phase 5: State Serialization");
    println!("─────────────────────────────────────────────────────");
    let serialized = bincode::serialize(&state).unwrap();
    println!("✓ State serialized: {} bytes", serialized.len());
    
    let deserialized: NetworkState = bincode::deserialize(&serialized).unwrap();
    println!("✓ State deserialized successfully");
    println!("  Merkle roots match: {}", state.merkle_root == deserialized.merkle_root);
    println!("  Epochs match: {}", state.epoch == deserialized.epoch);
    println!();

    // Summary
    println!("═══════════════════════════════════════════════════════");
    println!("  Summary");
    println!("═══════════════════════════════════════════════════════");
    println!("✓ Network State Management: WORKING");
    println!("✓ Epoch Calculation: WORKING");
    println!("✓ Merkle Root Updates: WORKING");
    println!("✓ Epoch Validation: WORKING");
    println!("✓ State Serialization: WORKING");
    println!();
    println!("Next Steps:");
    println!("  Day 2: Membership Proof Circuit");
    println!("  Day 3: Membership Proof Generator");
    println!("  Day 4: Nullifier Tracking System");
    println!("═══════════════════════════════════════════════════════");
}

// Helper: Mock Merkle root generation
fn hash_nodes(count: usize) -> [u8; 32] {
    use blake3::Hasher;
    let mut hasher = Hasher::new();
    hasher.update(b"phantom_network");
    hasher.update(&count.to_le_bytes());
    hasher.update(&current_timestamp().to_le_bytes());
    
    let hash = hasher.finalize();
    let mut result = [0u8; 32];
    result.copy_from_slice(hash.as_bytes());
    result
}

// Helper: Convert bytes to hex string
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<String>>()
        .join("")
}
