use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use phantom_crypto::FheEngine;
use std::time::Duration;

/// Benchmark FHE oblivious routing lookup with different routing table sizes
/// 
/// This benchmark measures the CRITICAL BOTTLENECK: FHE-based routing table lookup.
/// 
/// CPU Baseline (Week 7):
/// - 5 entries: ~2500ms per lookup
/// - 10 entries: ~5000ms per lookup
/// - 20 entries: ~10000ms per lookup
/// 
/// GPU Targets (Week 8) - varies by hardware:
/// 
/// Low-spec GPU (GTX 1060, RTX 2060):
/// - 5 entries: ~1000-1250ms (2-3x speedup)
/// - 10 entries: ~2000-2500ms
/// - 20 entries: ~4000-5000ms
/// 
/// Mid-range GPU (RTX 3060, 4060):
/// - 5 entries: ~400-600ms (5-7x speedup)
/// - 10 entries: ~800-1200ms
/// - 20 entries: ~1600-2400ms
/// 
/// High-end GPU (RTX 4090, A100):
/// - 5 entries: ~200-300ms (8-12x speedup)
/// - 10 entries: ~400-600ms
/// - 20 entries: ~800-1200ms
fn benchmark_oblivious_routing_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("fhe_oblivious_routing");
    
    // Configure for SLOW FHE operations
    group.sample_size(10);  // Only 10 samples (FHE is VERY slow)
    group.measurement_time(Duration::from_secs(120));  // 2 minutes per test
    
    // Generate FHE keys once (expensive setup)
    println!("\nGenerating FHE keys...");
    let fhe_engine = FheEngine::generate_keys();
    println!("Keys generated. Starting benchmarks...\n");
    
    // Test different routing table sizes
    for &num_entries in &[5, 10, 20] {
        // Build routing table with specified number of entries
        let path: Vec<u32> = (0..num_entries).collect();
        let routing_table = fhe_engine.build_routing_table(&path);
        let routing_blob = bincode::serialize(&routing_table).unwrap();
        
        // Benchmark lookup for middle node (worst case - linear scan)
        let my_node_id = num_entries / 2;
        
        group.bench_with_input(
            BenchmarkId::new("lookup", num_entries),
            &routing_blob,
            |b, blob| {
                b.iter(|| {
                    fhe_engine.oblivious_routing_lookup(
                        black_box(my_node_id),
                        black_box(blob)
                    )
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark FHE batch encryption (already optimized in Week 7)
/// 
/// This should show good speedup from Rayon parallelization.
fn benchmark_batch_encryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("fhe_batch_encryption");
    
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    
    let fhe_engine = FheEngine::generate_keys();
    
    for &num_values in &[5, 10, 20] {
        let values: Vec<u32> = (0..num_values).collect();
        
        group.bench_with_input(
            BenchmarkId::new("batch_encrypt", num_values),
            &values,
            |b, vals| {
                b.iter(|| {
                    fhe_engine.encrypt_u32_batch(black_box(vals))
                })
            },
        );
    }
    
    group.finish();
}

/// Benchmark routing table construction (combines batch encryption + pairing)
fn benchmark_routing_table_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("fhe_routing_table_build");
    
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(60));
    
    let fhe_engine = FheEngine::generate_keys();
    
    for &path_length in &[5, 10, 20] {
        let path: Vec<u32> = (0..path_length).collect();
        
        group.bench_with_input(
            BenchmarkId::new("build_table", path_length),
            &path,
            |b, p| {
                b.iter(|| {
                    fhe_engine.build_routing_table(black_box(p))
                })
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    benchmark_oblivious_routing_lookup,
    benchmark_batch_encryption,
    benchmark_routing_table_construction
);
criterion_main!(benches);
