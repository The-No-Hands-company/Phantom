//! Integration Tests for Anonymous Node Discovery

use phantom_discovery::{NetworkState, Announcer, Announcement, VerificationResult};
use std::collections::HashSet;

#[test]
fn test_100_node_announcement_flow() {
    println!("\n=== 100-Node Announcement Flow Test ===\n");
    
    let mut network_state = NetworkState::new();
    network_state.update_merkle_root([0xAB; 32]);
    network_state.set_node_count(100);
    
    let mut announcer = Announcer::new(10_000);
    let current_epoch = NetworkState::current_epoch();
    
    let mut accepted = 0;
    for i in 0..100 {
        let nullifier = create_nullifier(i);
        let announcement = create_announcement(
            nullifier,
            current_epoch,
            network_state.merkle_root,
        );
        
        let result = announcer.verify_and_register(&announcement, &network_state)
            .expect("Verification failed");
        
        if result == VerificationResult::Valid {
            accepted += 1;
        }
    }
    
    println!("✓ Accepted {}/100 announcements", accepted);
    assert_eq!(accepted, 100);
    assert_eq!(announcer.nullifier_count(), 100);
}

#[test]
fn test_spam_attack_resistance() {
    println!("\n=== Spam Attack Resistance Test ===\n");
    
    let mut network_state = NetworkState::new();
    network_state.update_merkle_root([0xAB; 32]);
    
    let mut announcer = Announcer::new(10_000);
    let current_epoch = NetworkState::current_epoch();
    
    for i in 0..100 {
        let nullifier = create_nullifier(i);
        let announcement = create_announcement(nullifier, current_epoch, network_state.merkle_root);
        announcer.verify_and_register(&announcement, &network_state).unwrap();
    }
    
    let mut blocked = 0;
    for i in 0..50 {
        let nullifier = create_nullifier(i);
        let announcement = create_announcement(nullifier, current_epoch, network_state.merkle_root);
        
        let result = announcer.verify_and_register(&announcement, &network_state).unwrap();
        if result == VerificationResult::Duplicate {
            blocked += 1;
        }
    }
    
    println!("✓ Spam attack: {}/50 duplicates blocked", blocked);
    assert_eq!(blocked, 50);
    assert_eq!(announcer.nullifier_count(), 100);
}

#[test]
fn test_performance_1000_announcements() {
    println!("\n=== Performance: 1000 Announcements ===\n");
    
    let mut network_state = NetworkState::new();
    network_state.update_merkle_root([0xAB; 32]);
    
    let mut announcer = Announcer::new(10_000);
    let current_epoch = NetworkState::current_epoch();
    
    let start = std::time::Instant::now();
    
    for i in 0..1000 {
        let nullifier = create_nullifier(i);
        let announcement = create_announcement(nullifier, current_epoch, network_state.merkle_root);
        announcer.verify_and_register(&announcement, &network_state).unwrap();
    }
    
    let elapsed = start.elapsed();
    
    println!("✓ Processed 1000 announcements in {:?}", elapsed);
    println!("  Throughput: {:.0} announcements/sec", 1000.0 / elapsed.as_secs_f64());
    
    assert_eq!(announcer.nullifier_count(), 1000);
}

fn create_nullifier(id: u64) -> [u8; 32] {
    let mut nullifier = [0u8; 32];
    nullifier[0..8].copy_from_slice(&id.to_le_bytes());
    nullifier
}

fn create_announcement(nullifier: [u8; 32], epoch: u64, merkle_root: [u8; 32]) -> Announcement {
    Announcement::new(vec![0xDE, 0xAD, 0xBE, 0xEF], nullifier, epoch, merkle_root)
}
