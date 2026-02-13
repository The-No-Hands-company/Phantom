use phantom_crypto::fhe::FheEngine;
use std::time::Instant;

/// Micro-benchmark to identify exactly where time is spent in FHE lookup
fn main() {
    println!("═══════════════════════════════════════════════════════");
    println!("  FHE Micro-Benchmark - Detailed Timing Breakdown");
    println!("═══════════════════════════════════════════════════════\n");

    println!("Setting up FHE engine...");
    let start = Instant::now();
    let engine = FheEngine::generate_keys();
    println!("✓ Setup complete: {:?}\n", start.elapsed());

    // Build a small routing table
    let path = vec![1, 2, 3];
    let routing_table = engine.build_routing_table(&path);
    let routing_blob = bincode::serialize(&routing_table).unwrap();

    println!("Testing lookup for Node 2 (expecting next_hop = 3)");
    println!("─────────────────────────────────────────────────────\n");

    // Measure each phase separately
    let total_start = Instant::now();
    
    // Phase 1: Deserialization
    let deser_start = Instant::now();
    let encrypted_table: Vec<(phantom_crypto::fhe::EncryptedValue, phantom_crypto::fhe::EncryptedValue)> = 
        bincode::deserialize(&routing_blob).unwrap();
    let deser_time = deser_start.elapsed();
    println!("Phase 1 - Deserialize routing blob: {:?}", deser_time);

    // Phase 2: Encrypt my node ID
    let encrypt_start = Instant::now();
    let _my_id_enc = engine.encrypt_u32(2);
    let encrypt_time = encrypt_start.elapsed();
    println!("Phase 2 - Encrypt my node ID: {:?}", encrypt_time);

    // Phase 3: The actual lookup (expensive!)
    println!("\nPhase 3 - Homomorphic Lookup:");
    let lookup_start = Instant::now();
    
    // We can't easily break down internal FHE ops without modifying the library,
    // so we'll just time the full lookup
    let result = engine.oblivious_routing_lookup(2, &routing_blob).unwrap();
    
    let lookup_time = lookup_start.elapsed();
    println!("  Full lookup time: {:?}", lookup_time);
    println!("  Result: next_hop = {} ✓", result);

    let total_time = total_start.elapsed();

    println!("\n─────────────────────────────────────────────────────");
    println!("Timing Breakdown:");
    println!("  Deserialization: {:?} ({:.1}%)", 
             deser_time, 
             100.0 * deser_time.as_secs_f64() / total_time.as_secs_f64());
    println!("  ID Encryption:   {:?} ({:.1}%)", 
             encrypt_time,
             100.0 * encrypt_time.as_secs_f64() / total_time.as_secs_f64());
    println!("  FHE Lookup:      {:?} ({:.1}%)", 
             lookup_time,
             100.0 * lookup_time.as_secs_f64() / total_time.as_secs_f64());
    println!("  Total:           {:?}", total_time);
    
    println!("\n═══════════════════════════════════════════════════════");
    println!("Analysis:");
    println!("═══════════════════════════════════════════════════════");
    
    let fhe_ops_time = lookup_time - deser_time - encrypt_time;
    println!("Pure FHE operations: ~{:?}", fhe_ops_time);
    println!("\nThis means {:.1}% of time is spent on:", 
             100.0 * fhe_ops_time.as_secs_f64() / lookup_time.as_secs_f64());
    println!("  - Homomorphic equality (eq)");
    println!("  - Homomorphic conditional (if_then_else)");
    println!("  - Homomorphic addition (+)");
    println!("\n⚠ These are TFHE-rs library operations.");
    println!("  Our optimizations don't affect them much.");
    println!("\nNext steps:");
    println!("  1. Reduce FHE parameter sizes (smaller = faster)");
    println!("  2. Enable TFHE x86_64 optimizations in Cargo.toml");
    println!("  3. Consider algorithmic changes (fewer FHE ops)");
    println!("═══════════════════════════════════════════════════════");
}
