//! Sparse vs Dense Routing Table Benchmark
//!
//! Measures performance improvement from Week 4 optimization.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use phantom_crypto::FheEngine;
use phantom_routing::SparseRoutingTable;
use std::time::Duration;

fn bench_sparse_routing_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("sparse_routing_lookup");
    group.measurement_time(Duration::from_secs(60)); // Long measurement (FHE is slow)
    group.sample_size(10); // Fewer samples (each iteration is expensive)
    
    // Generate FHE keys once (expensive)
    println!("Generating FHE keys (this takes ~2 seconds)...");
    let fhe_engine = FheEngine::generate_keys();
    println!("Keys generated. Starting benchmarks...");
    
    // Test different path lengths
    for path_length in [3, 5, 7, 10].iter() {
        // Create a path with specified length
        let path: Vec<u32> = (0..*path_length).map(|i| 100 + i * 100).collect();
        
        // Build sparse routing table
        let sparse_table = SparseRoutingTable::from_path(&path);
        
        // Encrypt the table
        let encrypted_table = sparse_table.encrypt(&fhe_engine)
            .expect("Encryption should succeed");
        
        // Pick a node in the middle of the path to lookup
        let lookup_node = path[path_length / 2];
        
        // Benchmark the lookup
        group.bench_with_input(
            BenchmarkId::new("sparse_lookup", path_length),
            &path_length,
            |b, _| {
                b.iter(|| {
                    let result = fhe_engine.oblivious_sparse_routing_lookup(
                        black_box(lookup_node),
                        black_box(&encrypted_table),
                    );
                    result.expect("Lookup should succeed")
                });
            },
        );
    }
    
    group.finish();
}

fn bench_dense_routing_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("dense_routing_lookup");
    group.measurement_time(Duration::from_secs(60));
    group.sample_size(10);
    
    println!("Generating FHE keys for dense table benchmark...");
    let fhe_engine = FheEngine::generate_keys();
    
    // Test different network sizes
    for network_size in [10, 50, 100].iter() {
        // Create a dense routing table (all nodes in network)
        let routing_table: Vec<(u32, u32)> = (0..*network_size)
            .map(|i| {
                let node_id = 100 + i * 10;
                let next_hop = if i + 1 < *network_size { 
                    100 + (i + 1) * 10 
                } else { 
                    0  // Last node is destination
                };
                (node_id, next_hop)
            })
            .collect();
        
        // Encrypt the dense table
        let encrypted_table = fhe_engine.encrypt_routing_table(&routing_table);
        
        // Pick a node in the middle to lookup
        let lookup_node = routing_table[network_size / 2].0;
        
        group.bench_with_input(
            BenchmarkId::new("dense_lookup", network_size),
            &network_size,
            |b, _| {
                b.iter(|| {
                    let result = fhe_engine.oblivious_routing_lookup(
                        black_box(lookup_node),
                        black_box(&encrypted_table),
                    );
                    result.expect("Lookup should succeed")
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(benches, bench_sparse_routing_lookup, bench_dense_routing_lookup);
criterion_main!(benches);
