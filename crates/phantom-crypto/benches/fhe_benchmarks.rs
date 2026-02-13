//! Benchmarks for Fully Homomorphic Encryption
//!
//! Run with: cargo bench --bench fhe_benchmarks
//! 
//! WARNING: These benchmarks are SLOW (FHE operations take seconds)
//! Run individual benchmarks with: cargo bench --bench fhe_benchmarks -- <bench_name>

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use phantom_crypto::fhe::FheEngine;

fn bench_fhe_keygen(c: &mut Criterion) {
    let mut group = c.benchmark_group("fhe_operations");
    group.sample_size(10); // FHE is slow, fewer samples
    
    group.bench_function("fhe_keygen", |b| {
        b.iter(|| {
            FheEngine::generate_keys()
        })
    });
    
    group.finish();
}

fn bench_fhe_encrypt_decrypt(c: &mut Criterion) {
    let mut group = c.benchmark_group("fhe_encryption");
    group.sample_size(30);
    
    let engine = FheEngine::generate_keys();
    
    group.bench_function("encrypt_u8", |b| {
        b.iter(|| {
            engine.encrypt_u8(black_box(42))
        })
    });
    
    let encrypted = engine.encrypt_u8(42);
    group.bench_function("decrypt_u8", |b| {
        b.iter(|| {
            engine.decrypt_u8(black_box(&encrypted))
        })
    });
    
    group.bench_function("encrypt_u32", |b| {
        b.iter(|| {
            engine.encrypt_u32(black_box(12345))
        })
    });
    
    let encrypted_u32 = engine.encrypt_u32(12345);
    group.bench_function("decrypt_u32", |b| {
        b.iter(|| {
            engine.decrypt_u32(black_box(&encrypted_u32))
        })
    });
    
    group.finish();
}

fn bench_routing_table_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("fhe_routing");
    group.sample_size(10); // Very slow operation
    
    let engine = FheEngine::generate_keys();
    
    // Benchmark different routing table sizes
    for table_size in [3, 5, 10].iter() {
        let table: Vec<_> = (0..*table_size)
            .map(|i| {
                (
                    engine.encrypt_u32(100 + i),
                    engine.encrypt_u32(200 + i),
                )
            })
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("routing_lookup", table_size),
            table_size,
            |b, _| {
                b.iter(|| {
                    engine.lookup_routing_table(
                        black_box(101),
                        black_box(&table)
                    )
                })
            }
        );
    }
    
    group.finish();
}

fn bench_end_to_end_routing(c: &mut Criterion) {
    let mut group = c.benchmark_group("fhe_end_to_end");
    group.sample_size(10);
    
    group.bench_function("full_routing_operation", |b| {
        b.iter(|| {
            // Simulate full routing: keygen + encrypt table + lookup + decrypt
            let engine = FheEngine::generate_keys();
            
            let table = vec![
                (engine.encrypt_u32(100), engine.encrypt_u32(200)),
                (engine.encrypt_u32(101), engine.encrypt_u32(201)),
                (engine.encrypt_u32(102), engine.encrypt_u32(202)),
            ];
            
            let result = engine.lookup_routing_table(101, &table).unwrap();
            let next_hop = engine.decrypt_u32(&result).unwrap();
            
            assert_eq!(next_hop, 201);
        })
    });
    
    group.finish();
}

// Separate the very slow benchmarks
fn bench_large_routing_tables(c: &mut Criterion) {
    let mut group = c.benchmark_group("fhe_large_tables");
    group.sample_size(10); // Minimum for criterion
    
    let engine = FheEngine::generate_keys();
    
    for table_size in [20, 50].iter() {
        let table: Vec<_> = (0..*table_size)
            .map(|i| {
                (
                    engine.encrypt_u32(1000 + i),
                    engine.encrypt_u32(2000 + i),
                )
            })
            .collect();
        
        group.bench_with_input(
            BenchmarkId::new("large_routing_lookup", table_size),
            table_size,
            |b, _| {
                b.iter(|| {
                    engine.lookup_routing_table(
                        black_box(1010),
                        black_box(&table)
                    )
                })
            }
        );
    }
    
    group.finish();
}

criterion_group!(
    basic_fhe,
    bench_fhe_keygen,
    bench_fhe_encrypt_decrypt,
);

criterion_group!(
    routing_fhe,
    bench_routing_table_lookup,
    bench_end_to_end_routing,
);

criterion_group!(
    name = large_fhe;
    config = Criterion::default().sample_size(10);
    targets = bench_large_routing_tables
);

// Week 4 Phase 2: Sparse routing performance comparison
fn bench_sparse_vs_dense_routing(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_routing_comparison");
    group.sample_size(15); // Need at least 10 for criterion
    
    let engine = FheEngine::generate_keys();
    
    // Simulate realistic scenarios:
    // - 5-hop path through 1000-node network
    // - Dense: Encrypt all 1000 node pairs
    // - Sparse: Encrypt only 5 path pairs
    
    println!("\n=== Sparse Routing Benchmark ===");
    println!("Scenario: 5-hop path through 1000-node network");
    println!("Dense table: 1000 entries | Sparse table: 5 entries\n");
    
    // Sparse routing (realistic: only path nodes)
    let sparse_path = vec![100, 200, 300, 400, 500];
    let sparse_table = engine.build_routing_table(&sparse_path);
    
    group.bench_function("sparse_5hop_lookup", |b| {
        b.iter(|| {
            engine.lookup_routing_table(
                black_box(200), // Lookup node 200's next hop
                black_box(&sparse_table)
            )
        })
    });
    
    // Dense routing (unrealistic: all network nodes)
    // For benchmark purposes, use 100 nodes (not 1000 - too slow)
    let dense_network_size = 100;
    let dense_table: Vec<_> = (0..dense_network_size)
        .map(|i| {
            (
                engine.encrypt_u32(1000 + i),
                engine.encrypt_u32(2000 + i),
            )
        })
        .collect();
    
    group.bench_function("dense_100node_lookup", |b| {
        b.iter(|| {
            engine.lookup_routing_table(
                black_box(1010),
                black_box(&dense_table)
            )
        })
    });
    
    println!("\nExpected speedup: ~20x (100 entries vs 5 entries)");
    println!("For 1000-node network: sparse would be 200x faster!\n");
    
    group.finish();
}

// Week 4 Phase 2: Batch encryption performance
fn bench_batch_encryption(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_encryption");
    group.sample_size(15);
    
    let engine = FheEngine::generate_keys();
    
    // Compare sequential vs batch (parallel) encryption
    let values = vec![100u32, 200, 300, 400, 500];
    
    group.bench_function("sequential_5_values", |b| {
        b.iter(|| {
            let _encrypted: Vec<_> = values
                .iter()
                .map(|&v| engine.encrypt_u32(black_box(v)))
                .collect();
        })
    });
    
    group.bench_function("batch_5_values", |b| {
        b.iter(|| {
            engine.encrypt_u32_batch(black_box(&values))
        })
    });
    
    // Test larger batches
    let large_batch: Vec<u32> = (0..20).collect();
    
    group.bench_function("batch_20_values", |b| {
        b.iter(|| {
            engine.encrypt_u32_batch(black_box(&large_batch))
        })
    });
    
    group.finish();
}

// Week 4 Phase 2: Complete routing table construction
fn bench_routing_table_construction(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_table_construction");
    group.sample_size(15); // Need at least 10 for criterion
    
    let engine = FheEngine::generate_keys();
    
    // Benchmark realistic path sizes
    for path_length in [3, 5, 7, 10].iter() {
        let path: Vec<u32> = (0..*path_length).map(|i| 1000 + i).collect();
        
        group.bench_with_input(
            BenchmarkId::new("build_sparse_table", path_length),
            path_length,
            |b, _| {
                b.iter(|| {
                    engine.build_routing_table(black_box(&path))
                })
            }
        );
    }
    
    group.finish();
}

criterion_group!(
    name = sparse_routing;
    config = Criterion::default().sample_size(15);
    targets = bench_sparse_vs_dense_routing, bench_batch_encryption, bench_routing_table_construction
);

criterion_main!(basic_fhe, routing_fhe, large_fhe, sparse_routing);
