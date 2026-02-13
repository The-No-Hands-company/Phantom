//! Performance benchmarks for complete PHANTOM routing proofs (Merkle + Path validation)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use phantom_circuit::{
    merkle::{MerkleCircuit, MerkleData},
    path::{PathData},
    routing::{RoutingProofSystem, RoutingProofData},
};
use plonky2::hash::poseidon::PoseidonHash;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

fn build_test_network(node_count: usize) -> (Vec<F>, Vec<Vec<F>>) {
    use plonky2::field::types::Field;
    
    // Generate nodes (leaf values in Merkle tree)
    let nodes: Vec<F> = (0..node_count)
        .map(|i| F::from_canonical_u64(i as u64))
        .collect();
    
    // Build Merkle tree
    let mut current_level = nodes.clone();
    let mut merkle_tree = vec![current_level.clone()];
    
    while current_level.len() > 1 {
        let mut next_level = Vec::new();
        for i in (0..current_level.len()).step_by(2) {
            let left = current_level[i];
            let right = if i + 1 < current_level.len() {
                current_level[i + 1]
            } else {
                left // Duplicate for odd number of nodes
            };
            
            // Hash (left || right) using Poseidon
            let hash = PoseidonHash::hash_no_pad(&[left, right]);
            next_level.push(hash.elements[0]);
        }
        merkle_tree.push(next_level.clone());
        current_level = next_level;
    }
    
    let root = current_level[0];
    
    // Generate Merkle proofs for each level
    let mut proofs = Vec::new();
    for level_idx in 0..merkle_tree.len() - 1 {
        let level = &merkle_tree[level_idx];
        let mut level_proofs = Vec::new();
        
        for node_idx in 0..level.len() {
            let sibling_idx = if node_idx % 2 == 0 {
                node_idx + 1
            } else {
                node_idx - 1
            };
            
            let sibling = if sibling_idx < level.len() {
                level[sibling_idx]
            } else {
                level[node_idx] // Self if no sibling
            };
            
            level_proofs.push(sibling);
        }
        
        proofs.push(level_proofs);
    }
    
    (vec![root], proofs)
}

fn bench_routing_proof_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_proof_generation");
    group.sample_size(10); // Proof generation is expensive
    
    // Build circuits once (reused across benchmarks)
    let merkle_circuit = MerkleCircuit::<F, C, D>::new();
    let path_circuit = phantom_circuit::path::PathValidationCircuit::<F, C, D>::new();
    let routing_system = RoutingProofSystem::new(merkle_circuit, path_circuit);
    
    // Network sizes: 16, 256, 4096, 65536 nodes (2^4 to 2^16)
    for tree_depth in [4, 8, 12, 16] {
        let node_count = 1 << tree_depth;
        let (roots, _proofs) = build_test_network(node_count);
        let root = roots[0];
        
        group.bench_with_input(
            BenchmarkId::new("nodes", node_count),
            &(tree_depth, root),
            |b, &(depth, merkle_root)| {
                // Test path: first 4 nodes
                let path_nodes = vec![0, 1, 2, 3];
                
                // Generate Merkle proofs for each path node
                let mut merkle_proofs = Vec::new();
                for &node_id in &path_nodes {
                    let mut proof = Vec::new();
                    let mut index = node_id;
                    
                    // Build proof path from leaf to root
                    for level in 0..depth {
                        let sibling_idx = if index % 2 == 0 { index + 1 } else { index - 1 };
                        let sibling = F::from_canonical_u64((sibling_idx as u64) % (1 << (depth - level)));
                        proof.push(sibling);
                        index /= 2;
                    }
                    
                    merkle_proofs.push(MerkleData {
                        leaf: F::from_canonical_u64(node_id as u64),
                        proof,
                        root: merkle_root,
                    });
                }
                
                let routing_data = RoutingProofData {
                    path: PathData {
                        node_ids: path_nodes.iter().map(|&id| id as u64).collect(),
                        path_length: path_nodes.len(),
                    },
                    merkle_proofs,
                };
                
                b.iter(|| {
                    let (_path_proof, _merkle_proofs) = routing_system
                        .prove_routing(black_box(&routing_data), black_box(merkle_root))
                        .expect("proof generation failed");
                });
            },
        );
    }
    
    group.finish();
}

fn bench_routing_proof_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_proof_verification");
    
    // Build circuits and generate proof once
    let merkle_circuit = MerkleCircuit::<F, C, D>::new();
    let path_circuit = phantom_circuit::path::PathValidationCircuit::<F, C, D>::new();
    let routing_system = RoutingProofSystem::new(merkle_circuit, path_circuit);
    
    // Use 16-node network for fast proof generation
    let (roots, _proofs) = build_test_network(16);
    let root = roots[0];
    
    // Generate test path
    let path_nodes = vec![0, 1, 2, 3];
    let mut merkle_proofs = Vec::new();
    
    for &node_id in &path_nodes {
        let mut proof = Vec::new();
        let mut index = node_id;
        
        for level in 0..4 {
            let sibling_idx = if index % 2 == 0 { index + 1 } else { index - 1 };
            let sibling = F::from_canonical_u64((sibling_idx as u64) % (1 << (4 - level)));
            proof.push(sibling);
            index /= 2;
        }
        
        merkle_proofs.push(MerkleData {
            leaf: F::from_canonical_u64(node_id as u64),
            proof,
            root,
        });
    }
    
    let routing_data = RoutingProofData {
        path: PathData {
            node_ids: path_nodes.iter().map(|&id| id as u64).collect(),
            path_length: path_nodes.len(),
        },
        merkle_proofs,
    };
    
    // Generate proof to verify
    let (path_proof, merkle_proof_list) = routing_system
        .prove_routing(&routing_data, root)
        .expect("proof generation failed");
    
    group.bench_function("verify_routing_proof", |b| {
        b.iter(|| {
            routing_system
                .verify_routing(
                    black_box(&path_proof),
                    black_box(&merkle_proof_list),
                    black_box(root),
                )
                .expect("verification failed");
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_routing_proof_generation, bench_routing_proof_verification);
criterion_main!(benches);
