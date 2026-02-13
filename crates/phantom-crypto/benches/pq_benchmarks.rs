//! Benchmarks for Post-Quantum Cryptography
//!
//! Run with: cargo bench --bench pq_benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use phantom_crypto::pq::{KeyPair, SigningKeyPair};

fn bench_kyber_keygen(c: &mut Criterion) {
    c.bench_function("kyber_1024_keygen", |b| {
        b.iter(|| {
            KeyPair::generate()
        })
    });
}

fn bench_kyber_encapsulation(c: &mut Criterion) {
    let keypair = KeyPair::generate();
    
    c.bench_function("kyber_1024_encapsulate", |b| {
        b.iter(|| {
            KeyPair::encapsulate(black_box(&keypair.public))
        })
    });
}

fn bench_kyber_decapsulation(c: &mut Criterion) {
    let keypair = KeyPair::generate();
    let (ciphertext, _) = KeyPair::encapsulate(&keypair.public).unwrap();
    
    c.bench_function("kyber_1024_decapsulate", |b| {
        b.iter(|| {
            keypair.decapsulate(black_box(&ciphertext))
        })
    });
}

fn bench_dilithium_keygen(c: &mut Criterion) {
    c.bench_function("dilithium_5_keygen", |b| {
        b.iter(|| {
            SigningKeyPair::generate()
        })
    });
}

fn bench_dilithium_sign(c: &mut Criterion) {
    let keypair = SigningKeyPair::generate();
    let message = b"PHANTOM protocol - quantum-resistant anonymous networking";
    
    c.bench_function("dilithium_5_sign", |b| {
        b.iter(|| {
            keypair.sign(black_box(message))
        })
    });
}

fn bench_dilithium_verify(c: &mut Criterion) {
    let keypair = SigningKeyPair::generate();
    let message = b"PHANTOM protocol - quantum-resistant anonymous networking";
    let signature = keypair.sign(message).unwrap();
    
    c.bench_function("dilithium_5_verify", |b| {
        b.iter(|| {
            SigningKeyPair::verify(
                black_box(&keypair.public),
                black_box(message),
                black_box(&signature)
            )
        })
    });
}

fn bench_full_key_exchange(c: &mut Criterion) {
    c.bench_function("kyber_full_exchange", |b| {
        b.iter(|| {
            let keypair = KeyPair::generate();
            let (ciphertext, ss1) = KeyPair::encapsulate(&keypair.public).unwrap();
            let ss2 = keypair.decapsulate(&ciphertext).unwrap();
            assert_eq!(ss1.0, ss2.0);
        })
    });
}

fn bench_message_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("dilithium_sign_varying_size");
    
    for size in [64, 256, 1024, 4096, 16384].iter() {
        let keypair = SigningKeyPair::generate();
        let message = vec![0u8; *size];
        
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _| {
            b.iter(|| {
                keypair.sign(black_box(&message))
            })
        });
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_kyber_keygen,
    bench_kyber_encapsulation,
    bench_kyber_decapsulation,
    bench_dilithium_keygen,
    bench_dilithium_sign,
    bench_dilithium_verify,
    bench_full_key_exchange,
    bench_message_sizes,
);

criterion_main!(benches);
