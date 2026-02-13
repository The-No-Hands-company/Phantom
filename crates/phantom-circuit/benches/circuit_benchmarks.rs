use criterion::{black_box, criterion_group, criterion_main, Criterion};
use phantom_circuit::{MerkleCircuit, MerkleProof};
use plonky2::hash::hash_types::HashOut;
use plonky2::field::types::Field;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

fn merkle_proof_generation_benchmark(c: &mut Criterion) {
    // Build a tree with 16 leaves (4 levels)
    let leaves: Vec<HashOut<F>> = (0..16)
        .map(|i| HashOut {
            elements: [
                F::from_canonical_u64(1000 + i),
                F::from_canonical_u64(2000 + i),
                F::from_canonical_u64(3000 + i),
                F::from_canonical_u64(4000 + i),
            ],
        })
        .collect();

    let (_root, proofs) = MerkleCircuit::build_tree(&leaves);
    let circuit = MerkleCircuit::new(4);
    let (circuit_data, targets) = circuit.build_circuit().expect("Circuit should build");

    c.bench_function("merkle_proof_4_levels", |b| {
        b.iter(|| {
            let proof_data = proofs.get(&black_box(0)).unwrap();
            circuit.prove(&circuit_data, &targets, proof_data).expect("Proof should work")
        });
    });
}

fn merkle_proof_verification_benchmark(c: &mut Criterion) {
    let leaves: Vec<HashOut<F>> = (0..16)
        .map(|i| HashOut {
            elements: [
                F::from_canonical_u64(1000 + i),
                F::from_canonical_u64(2000 + i),
                F::from_canonical_u64(3000 + i),
                F::from_canonical_u64(4000 + i),
            ],
        })
        .collect();

    let (_root, proofs) = MerkleCircuit::build_tree(&leaves);
    let circuit = MerkleCircuit::new(4);
    let (circuit_data, targets) = circuit.build_circuit().expect("Circuit should build");
    
    let proof_data = proofs.get(&0).unwrap();
    let proof = circuit.prove(&circuit_data, &targets, proof_data).expect("Proof should work");

    c.bench_function("merkle_verify_4_levels", |b| {
        b.iter(|| {
            circuit_data.verify(black_box(proof.clone())).expect("Verify should work")
        });
    });
}

fn merkle_circuit_build_benchmark(c: &mut Criterion) {
    c.bench_function("merkle_circuit_build_4_levels", |b| {
        b.iter(|| {
            let circuit = MerkleCircuit::new(black_box(4));
            circuit.build_circuit().expect("Circuit should build")
        });
    });
}

fn merkle_proof_20_levels_benchmark(c: &mut Criterion) {
    // Build a tree with 64 leaves (6 levels) - representative of larger trees
    let leaves: Vec<HashOut<F>> = (0..64)
        .map(|i| HashOut {
            elements: [
                F::from_canonical_u64(i),
                F::from_canonical_u64(i + 1000),
                F::from_canonical_u64(i + 2000),
                F::from_canonical_u64(i + 3000),
            ],
        })
        .collect();

    let (_root, proofs) = MerkleCircuit::build_tree(&leaves);
    let circuit = MerkleCircuit::new(6);
    let (circuit_data, targets) = circuit.build_circuit().expect("Circuit should build");

    c.bench_function("merkle_proof_6_levels", |b| {
        b.iter(|| {
            let proof_data = proofs.get(&black_box(0)).unwrap();
            circuit.prove(&circuit_data, &targets, proof_data).expect("Proof should work")
        });
    });
}

criterion_group!(
    benches,
    merkle_circuit_build_benchmark,
    merkle_proof_generation_benchmark,
    merkle_proof_verification_benchmark,
    merkle_proof_20_levels_benchmark
);
criterion_main!(benches);
