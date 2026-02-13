use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use phantom_crypto::fhe::FheEngine;
use std::time::Instant;

/// Detailed profiling of FHE operations to identify bottlenecks
/// 
/// This benchmark measures each FHE operation individually to find
/// where we can optimize for CPU performance.

fn profile_key_generation(c: &mut Criterion) {
    c.bench_function("fhe_key_generation", |b| {
        b.iter(|| {
            black_box(FheEngine::generate_keys())
        });
    });
}

fn profile_encryption_operations(c: &mut Criterion) {
    let engine = FheEngine::generate_keys();
    
    // Single u32 encryption
    c.bench_function("fhe_encrypt_u32_single", |b| {
        b.iter(|| {
            black_box(engine.encrypt_u32(42))
        });
    });
    
    // Batch encryption (various sizes)
    for size in [5, 10, 20, 50].iter() {
        c.bench_with_input(
            BenchmarkId::new("fhe_encrypt_u32_batch", size),
            size,
            |b, &size| {
                let values: Vec<u32> = (0..size).collect();
                b.iter(|| {
                    black_box(engine.encrypt_u32_batch(&values))
                });
            },
        );
    }
}

fn profile_routing_table_operations(c: &mut Criterion) {
    let engine = FheEngine::generate_keys();
    
    // Routing table construction (different path lengths)
    for path_len in [3, 5, 7, 10].iter() {
        c.bench_with_input(
            BenchmarkId::new("fhe_build_routing_table", path_len),
            path_len,
            |b, &path_len| {
                let path: Vec<u32> = (0..path_len).collect();
                b.iter(|| {
                    black_box(engine.build_routing_table(&path))
                });
            },
        );
    }
}

fn profile_lookup_operations(c: &mut Criterion) {
    let engine = FheEngine::generate_keys();
    
    // Build a 5-hop routing table once
    let path = vec![1, 2, 3, 4, 5];
    let routing_table = engine.build_routing_table(&path);
    
    // Serialize for realistic test
    let routing_blob = bincode::serialize(&routing_table)
        .expect("Serialization should work");
    
    // Profile oblivious lookup (THE BOTTLENECK)
    c.bench_function("fhe_oblivious_lookup_5hop", |b| {
        b.iter(|| {
            black_box(engine.oblivious_routing_lookup(2, &routing_blob))
        });
    });
    
    // Profile direct table lookup (without serialization)
    c.bench_function("fhe_table_lookup_direct", |b| {
        b.iter(|| {
            black_box(engine.lookup_routing_table(2, &routing_table))
        });
    });
}

fn profile_homomorphic_operations(c: &mut Criterion) {
    let engine = FheEngine::generate_keys();
    
    println!("\n═══════════════════════════════════════════════════════");
    println!("  Detailed FHE Operation Profiling");
    println!("  Goal: Identify CPU optimization opportunities");
    println!("═══════════════════════════════════════════════════════\n");
    
    // Individual FHE operations that happen in lookup
    let val1 = engine.encrypt_u32(42);
    let val2 = engine.encrypt_u32(42);
    let val3 = engine.encrypt_u32(100);
    
    // Profile equality check (used in every table entry)
    println!("Profiling: Homomorphic Equality (eq)");
    let start = Instant::now();
    for _ in 0..10 {
        use tfhe::prelude::*;
        let fhe1: tfhe::FheUint32 = bincode::deserialize(&val1.data).unwrap();
        let fhe2: tfhe::FheUint32 = bincode::deserialize(&val2.data).unwrap();
        tfhe::set_server_key(engine.server_key().inner.clone());
        let _ = black_box(fhe1.eq(&fhe2));
    }
    let avg_eq = start.elapsed() / 10;
    println!("  Average: {:?}", avg_eq);
    
    // Profile conditional select (used in every table entry)
    println!("\nProfiling: Homomorphic Conditional (if_then_else)");
    let start = Instant::now();
    for _ in 0..10 {
        use tfhe::prelude::*;
        let fhe1: tfhe::FheUint32 = bincode::deserialize(&val1.data).unwrap();
        let fhe2: tfhe::FheUint32 = bincode::deserialize(&val2.data).unwrap();
        let fhe3: tfhe::FheUint32 = bincode::deserialize(&val3.data).unwrap();
        tfhe::set_server_key(engine.server_key().inner.clone());
        let matches = fhe1.eq(&fhe2);
        let _ = black_box(matches.if_then_else(&fhe2, &fhe3));
    }
    let avg_select = start.elapsed() / 10;
    println!("  Average: {:?}", avg_select);
    
    // Profile addition (used for accumulation)
    println!("\nProfiling: Homomorphic Addition (+)");
    let start = Instant::now();
    for _ in 0..10 {
        use tfhe::prelude::*;
        let fhe1: tfhe::FheUint32 = bincode::deserialize(&val1.data).unwrap();
        let fhe2: tfhe::FheUint32 = bincode::deserialize(&val2.data).unwrap();
        tfhe::set_server_key(engine.server_key().inner.clone());
        let _ = black_box(&fhe1 + &fhe2);
    }
    let avg_add = start.elapsed() / 10;
    println!("  Average: {:?}", avg_add);
    
    // Estimate total lookup cost for 5-hop path
    println!("\n─────────────────────────────────────────────────────");
    println!("Estimated 5-hop lookup breakdown:");
    println!("  Per entry: eq ({:?}) + select ({:?}) + add ({:?})", avg_eq, avg_select, avg_add);
    let per_entry = avg_eq + avg_select + avg_add;
    println!("  Per entry total: {:?}", per_entry);
    println!("  5 entries × {:?} = {:?}", per_entry, per_entry * 5);
    println!("  Plus deserialization overhead: ~50-100ms");
    println!("─────────────────────────────────────────────────────\n");
}

fn profile_serialization(c: &mut Criterion) {
    let engine = FheEngine::generate_keys();
    let path = vec![1, 2, 3, 4, 5];
    let routing_table = engine.build_routing_table(&path);
    
    // Profile serialization cost
    c.bench_function("fhe_serialize_routing_table", |b| {
        b.iter(|| {
            black_box(bincode::serialize(&routing_table))
        });
    });
    
    let routing_blob = bincode::serialize(&routing_table).unwrap();
    
    // Profile deserialization cost
    c.bench_function("fhe_deserialize_routing_table", |b| {
        b.iter(|| {
            let _: Vec<(phantom_crypto::fhe::EncryptedValue, phantom_crypto::fhe::EncryptedValue)> = 
                black_box(bincode::deserialize(&routing_blob).unwrap());
        });
    });
    
    println!("\nRouting blob size: {} bytes ({:.2} MB)", 
             routing_blob.len(), 
             routing_blob.len() as f64 / 1_048_576.0);
}

criterion_group! {
    name = fhe_profiling;
    config = Criterion::default()
        .sample_size(10)  // Reduce sample size (FHE is slow)
        .measurement_time(std::time::Duration::from_secs(30));  // 30s per benchmark
    targets = 
        profile_key_generation,
        profile_encryption_operations,
        profile_routing_table_operations,
        profile_homomorphic_operations,
        profile_serialization,
        profile_lookup_operations
}

criterion_main!(fhe_profiling);
