//! Path Validation Demo
//!
//! Demonstrates multi-hop path validation with quality metrics.
//!
//! Usage:
//!     cargo run --package phantom-routing --example path_validation_demo

use phantom_routing::{
    PeerSelector, PathBuilder, PathValidator, 
    SelectionStrategy, QualityWeights, NodeCapabilities,
};
use anyhow::Result;

fn main() -> Result<()> {
    println!("=== PHANTOM Path Validation Demo ===\n");

    // Simulate a network with diverse nodes
    println!("1. Simulating Network Nodes...");
    let mut selector = create_network_selector();
    println!("   ✓ Created network with 50 nodes\n");

    // Test different path scenarios
    demo_optimal_path(&mut selector)?;
    demo_fast_path(&mut selector)?;
    demo_reliable_path(&mut selector)?;
    demo_anonymous_path(&mut selector)?;
    demo_path_comparison(&mut selector)?;

    println!("\n=== Path Validation Demo Complete ===");
    Ok(())
}

/// Demo 1: Optimal balanced path
fn demo_optimal_path(selector: &mut PeerSelector) -> Result<()> {
    println!("2. Optimal Balanced Path");
    println!("   Goal: Best overall quality with default weights\n");

    let validator = PathValidator::new();
    
    // Select 5 high-quality peers
    let peers = selector.select_peers(5, 100)?;
    let path = PathBuilder::new()
        .min_hops(3)
        .max_hops(5)
        .build_path(peers)?;

    let result = validator.validate_path(&path)?;
    
    println!("   Path: {} hops", path.hop_count);
    println!("   Valid: {}", result.valid);
    println!("   Overall Score: {:.2}", result.overall_score);
    println!("\n   Quality Breakdown:");
    println!("     Latency:     {:.2} ({} ms)", 
        result.metrics.latency_score, result.metrics.total_latency_ms);
    println!("     Reliability: {:.2} ({:.2}%)", 
        result.metrics.reliability_score, result.metrics.path_reliability * 100.0);
    println!("     Capacity:    {:.2} ({} KB/s)", 
        result.metrics.capacity_score, result.metrics.min_capacity / 1000);
    println!("     Anonymity:   {:.2}", result.metrics.anonymity_score);
    println!("     Diversity:   {:.2}", result.metrics.diversity_score);
    
    if !result.errors.is_empty() {
        println!("\n   Validation Errors:");
        for err in &result.errors {
            println!("     ✗ {}", err);
        }
    }
    
    println!();
    Ok(())
}

/// Demo 2: Low-latency path
fn demo_fast_path(selector: &mut PeerSelector) -> Result<()> {
    println!("3. Low-Latency Path");
    println!("   Goal: Minimize latency (prioritize speed)\n");

    // Validator optimized for latency
    let weights = QualityWeights {
        latency_weight: 0.6,    // Prioritize latency
        reliability_weight: 0.2,
        capacity_weight: 0.1,
        anonymity_weight: 0.1,
    };
    
    let validator = PathValidator::new()
        .max_latency_ms(1500)   // Strict latency requirement
        .weights(weights);
    
    // Shorter path for lower latency
    let peers = selector.select_peers(3, 100)?;
    let path = PathBuilder::new()
        .min_hops(3)
        .max_hops(3)  // Fixed 3 hops for speed
        .build_path(peers)?;

    let result = validator.validate_path(&path)?;
    
    println!("   Path: {} hops", path.hop_count);
    println!("   Valid: {}", result.valid);
    println!("   Overall Score: {:.2}", result.overall_score);
    println!("   Latency: {} ms (target: <1500ms)", result.metrics.total_latency_ms);
    
    if result.valid {
        println!("   ✓ Meets low-latency requirements");
    } else {
        println!("   ✗ Failed latency requirements");
    }
    
    println!();
    Ok(())
}

/// Demo 3: High-reliability path
fn demo_reliable_path(selector: &mut PeerSelector) -> Result<()> {
    println!("4. High-Reliability Path");
    println!("   Goal: Maximum reliability (mission-critical)\n");

    // Validator optimized for reliability
    let weights = QualityWeights {
        latency_weight: 0.1,
        reliability_weight: 0.6,  // Prioritize reliability
        capacity_weight: 0.2,
        anonymity_weight: 0.1,
    };
    
    let validator = PathValidator::new()
        .min_reliability(0.95)   // Very strict reliability
        .weights(weights);
    
    let peers = selector.select_peers(5, 100)?;
    let path = PathBuilder::new()
        .min_hops(3)
        .max_hops(5)
        .build_path(peers)?;

    let result = validator.validate_path(&path)?;
    
    println!("   Path: {} hops", path.hop_count);
    println!("   Valid: {}", result.valid);
    println!("   Overall Score: {:.2}", result.overall_score);
    println!("   Reliability: {:.2}% (target: >95%)", 
        result.metrics.path_reliability * 100.0);
    
    if result.valid {
        println!("   ✓ Meets high-reliability requirements");
    } else {
        println!("   ✗ Failed reliability requirements");
    }
    
    println!();
    Ok(())
}

/// Demo 4: Maximum anonymity path
fn demo_anonymous_path(selector: &mut PeerSelector) -> Result<()> {
    println!("5. Maximum Anonymity Path");
    println!("   Goal: Strongest privacy guarantees\n");

    // Validator optimized for anonymity
    let weights = QualityWeights {
        latency_weight: 0.1,
        reliability_weight: 0.2,
        capacity_weight: 0.1,
        anonymity_weight: 0.6,  // Prioritize anonymity
    };
    
    let validator = PathValidator::new()
        .min_anonymity(0.85)    // High anonymity requirement
        .weights(weights);
    
    // Longer path with high diversity
    let peers = selector.select_peers(7, 100)?;
    let path = PathBuilder::new()
        .min_hops(5)
        .max_hops(7)  // Longer path for better anonymity
        .diversity_threshold(0.9)  // High diversity requirement
        .build_path(peers)?;

    let result = validator.validate_path(&path)?;
    
    println!("   Path: {} hops", path.hop_count);
    println!("   Valid: {}", result.valid);
    println!("   Overall Score: {:.2}", result.overall_score);
    println!("   Anonymity: {:.2}", result.metrics.anonymity_score);
    println!("   Diversity: {:.2}", result.metrics.diversity_score);
    
    if result.valid {
        println!("   ✓ Meets high-anonymity requirements");
    } else {
        println!("   ✗ Failed anonymity requirements");
    }
    
    println!();
    Ok(())
}

/// Demo 5: Path comparison and selection
fn demo_path_comparison(selector: &mut PeerSelector) -> Result<()> {
    println!("6. Path Comparison and Selection");
    println!("   Goal: Select best path from multiple candidates\n");

    let validator = PathValidator::new();
    
    // Generate multiple candidate paths
    println!("   Generating 3 candidate paths...");
    
    let path1 = PathBuilder::new()
        .min_hops(3)
        .max_hops(3)
        .build_path(selector.select_peers(3, 100)?)?;
    
    let path2 = PathBuilder::new()
        .min_hops(5)
        .max_hops(5)
        .build_path(selector.select_peers(5, 100)?)?;
    
    let path3 = PathBuilder::new()
        .min_hops(4)
        .max_hops(4)
        .build_path(selector.select_peers(4, 100)?)?;
    
    // Validate all paths
    let result1 = validator.validate_path(&path1)?;
    let result2 = validator.validate_path(&path2)?;
    let result3 = validator.validate_path(&path3)?;
    
    println!("\n   Candidate A: {} hops, score {:.2}, latency {} ms",
        path1.hop_count, result1.overall_score, result1.metrics.total_latency_ms);
    println!("   Candidate B: {} hops, score {:.2}, latency {} ms",
        path2.hop_count, result2.overall_score, result2.metrics.total_latency_ms);
    println!("   Candidate C: {} hops, score {:.2}, latency {} ms",
        path3.hop_count, result3.overall_score, result3.metrics.total_latency_ms);
    
    // Compare and select best
    let best_ab = validator.compare_paths(&path1, &path2)?;
    let best = validator.compare_paths(best_ab, &path3)?;
    
    let best_result = validator.validate_path(best)?;
    
    println!("\n   ✓ Selected best path: {} hops, score {:.2}",
        best.hop_count, best_result.overall_score);
    
    println!();
    Ok(())
}

/// Create a network selector with simulated diverse nodes
fn create_network_selector() -> PeerSelector {
    use phantom_routing::peer_selector::NodeInfo;

    let mut selector = PeerSelector::new(SelectionStrategy::Weighted);
    
    // Add 50 diverse nodes with varying characteristics
    for i in 0..50 {
        let mut nullifier = [0u8; 32];
        nullifier[0] = i;
        nullifier[1] = i / 10;  // Some diversity in nullifiers
        
        // Vary node characteristics
        let reliability = 0.85 + (i as f64 % 15.0) / 100.0; // 0.85-1.00
        let capacity = (50_000 + (i as u64 * 5000)) as u64;  // 50-300 KB/s
        
        let node = NodeInfo {
            nullifier,
            last_seen_epoch: 100 + (i as u64 % 10),
            capabilities: NodeCapabilities {
                fhe_routing: true,
                zkvm_verification: true,
                max_packet_size: 1024,
            },
            reliability,
            capacity,
            selection_count: 0,
        };
        
        selector.add_node(node);
    }
    
    selector
}
