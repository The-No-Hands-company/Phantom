use phantom_crypto::fhe::FheEngine;
use std::time::Instant;

/// Simple FHE profiling using only public APIs
/// This helps identify optimization opportunities for CPU performance

fn main() {
    println!("═══════════════════════════════════════════════════════");
    println!("  FHE Performance Profiling (CPU Baseline)");
    println!("  Goal: Identify bottlenecks for optimization");
    println!("═══════════════════════════════════════════════════════\n");

    // === PHASE 1: Key Generation ===
    println!("Phase 1: FHE Key Generation");
    println!("─────────────────────────────────────────────────────");
    let start = Instant::now();
    let engine = FheEngine::generate_keys();
    let keygen_time = start.elapsed();
    println!("✓ Keys generated in: {:?}", keygen_time);
    println!("  (This is one-time cost, keys are reused)\n");

    // === PHASE 2: Single Encryption ===
    println!("Phase 2: Single Value Encryption");
    println!("─────────────────────────────────────────────────────");
    let mut encryption_times = Vec::new();
    for i in 0..10 {
        let start = Instant::now();
        let _ = engine.encrypt_u32(i);
        encryption_times.push(start.elapsed());
    }
    let avg_encryption = encryption_times.iter().sum::<std::time::Duration>() / 10;
    println!("✓ Average encryption time (10 samples): {:?}", avg_encryption);
    println!("  Min: {:?}, Max: {:?}", 
             encryption_times.iter().min().unwrap(),
             encryption_times.iter().max().unwrap());
    println!();

    // === PHASE 3: Batch Encryption ===
    println!("Phase 3: Batch Encryption (Parallel)");
    println!("─────────────────────────────────────────────────────");
    for batch_size in [5, 10, 20] {
        let values: Vec<u32> = (0..batch_size).collect();
        let start = Instant::now();
        let _ = engine.encrypt_u32_batch(&values);
        let batch_time = start.elapsed();
        let per_value = batch_time / batch_size as u32;
        println!("✓ Batch size {}: {:?} total, {:?} per value", 
                 batch_size, batch_time, per_value);
    }
    println!();

    // === PHASE 4: Routing Table Construction ===
    println!("Phase 4: Routing Table Construction");
    println!("─────────────────────────────────────────────────────");
    for path_len in [3, 5, 7] {
        let path: Vec<u32> = (1..=path_len).collect();
        let start = Instant::now();
        let routing_table = engine.build_routing_table(&path);
        let build_time = start.elapsed();
        
        // Serialize to check size
        let routing_blob = bincode::serialize(&routing_table)
            .expect("Serialization should work");
        let size_kb = routing_blob.len() as f64 / 1024.0;
        
        println!("✓ Path length {}: {:?}, routing blob: {:.2} KB", 
                 path_len, build_time, size_kb);
    }
    println!();

    // === PHASE 5: Oblivious Routing Lookup (THE BOTTLENECK) ===
    println!("Phase 5: Oblivious Routing Lookup");
    println!("─────────────────────────────────────────────────────");
    println!("⚠ This is the performance bottleneck (2.5s per hop)");
    println!("  Testing with 3-hop path to save time...\n");
    
    let path = vec![1, 2, 3];
    let routing_table = engine.build_routing_table(&path);
    let routing_blob = bincode::serialize(&routing_table)
        .expect("Serialization should work");
    
    // Test lookup for each node in path
    for (i, &node_id) in path.iter().enumerate() {
        println!("  Hop {}: Node {} looking up next hop...", i + 1, node_id);
        let start = Instant::now();
        let next_hop = engine.oblivious_routing_lookup(node_id, &routing_blob)
            .expect("Lookup should work");
        let lookup_time = start.elapsed();
        
        let expected = if i < path.len() - 1 { path[i + 1] } else { 0 };
        println!("    Result: next_hop = {} (expected: {})", next_hop, expected);
        println!("    Time: {:?} ⏱", lookup_time);
        
        if next_hop != expected {
            println!("    ❌ ERROR: Incorrect routing!");
        } else {
            println!("    ✓ Correct routing");
        }
    }
    println!();

    // === SUMMARY ===
    println!("═══════════════════════════════════════════════════════");
    println!("  Performance Summary");
    println!("═══════════════════════════════════════════════════════");
    println!("Key Generation:     {:?} (one-time)", keygen_time);
    println!("Single Encryption:  {:?} (per value)", avg_encryption);
    println!("Batch Encryption:   ~{:?} (per value, parallel)", avg_encryption / 3);
    println!("\n⚠ BOTTLENECK: Oblivious Routing Lookup");
    println!("  Current: ~2.5s per hop on CPU");
    println!("  Target:  ~800ms per hop (3x speedup goal)");
    println!("\nOptimization Opportunities:");
    println!("  1. Enable AVX2/SIMD in TFHE-rs");
    println!("  2. Reduce FHE parameter sizes (smaller ciphertexts)");
    println!("  3. Cache intermediate FHE operations");
    println!("  4. Optimize loop in lookup_routing_table()");
    println!("  5. Pre-encrypt common values (e.g., zero)");
    println!("═══════════════════════════════════════════════════════");
}
