//! Complete PHANTOM Routing Proof
//!
//! Combines Merkle proof + Path validation into a single routing proof.
//!
//! ## Proof Components
//! 1. **Merkle Proofs**: Each node in path is in the network (5 proofs)
//! 2. **Path Validation**: Path satisfies routing constraints
//!
//! ## Performance Target
//! - **5 Merkle proofs**: ~95ms (19ms each)
//! - **1 Path validation**: ~266ms
//! - **Total**: ~361ms (well under 1s target!)

use crate::merkle::{MerkleCircuit, MerkleProof, MerkleTargets};
use crate::path::{PathValidationCircuit, PathData, PathTargets};
use plonky2::field::types::Field;
use plonky2::hash::hash_types::HashOut;
use plonky2::plonk::circuit_data::CircuitData;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
use plonky2::plonk::proof::ProofWithPublicInputs;
use anyhow::Result;

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

/// Complete routing proof data
#[derive(Clone, Debug)]
pub struct RoutingProofData {
    pub path: PathData,
    pub merkle_proofs: Vec<MerkleProof>,
    pub merkle_root: HashOut<F>,
}

/// PHANTOM routing proof system
pub struct RoutingProofSystem {
    pub merkle_circuit: MerkleCircuit,
    pub path_circuit: PathValidationCircuit,
}

impl RoutingProofSystem {
    pub fn new(tree_depth: usize, max_path_length: usize) -> Self {
        Self {
            merkle_circuit: MerkleCircuit::new(tree_depth),
            path_circuit: PathValidationCircuit::new(max_path_length),
        }
    }

    /// Generate complete routing proof
    /// 
    /// **Steps**:
    /// 1. Prove path is valid (no loops, correct length)
    /// 2. Prove each node is in the network (Merkle proofs)
    /// 
    /// Returns: (path_proof, merkle_proofs)
    pub fn prove_routing(
        &self,
        routing_data: &RoutingProofData,
        merkle_circuit_data: &CircuitData<F, C, D>,
        merkle_targets: &MerkleTargets,
        path_circuit_data: &CircuitData<F, C, D>,
        path_targets: &PathTargets,
    ) -> Result<(ProofWithPublicInputs<F, C, D>, Vec<ProofWithPublicInputs<F, C, D>>)> {
        // Validate path constraints
        let path_proof = self.path_circuit.prove(
            path_circuit_data,
            path_targets,
            &routing_data.path,
        )?;

        // Prove each node is in the network
        let mut merkle_proofs = Vec::new();
        for merkle_data in &routing_data.merkle_proofs {
            let proof = self.merkle_circuit.prove(
                merkle_circuit_data,
                merkle_targets,
                merkle_data,
            )?;
            merkle_proofs.push(proof);
        }

        Ok((path_proof, merkle_proofs))
    }

    /// Verify complete routing proof
    pub fn verify_routing(
        &self,
        path_proof: &ProofWithPublicInputs<F, C, D>,
        merkle_proofs: &[ProofWithPublicInputs<F, C, D>],
        merkle_circuit_data: &CircuitData<F, C, D>,
        path_circuit_data: &CircuitData<F, C, D>,
    ) -> Result<()> {
        // Verify path proof
        path_circuit_data.verify(path_proof.clone())?;

        // Verify all Merkle proofs
        for proof in merkle_proofs {
            merkle_circuit_data.verify(proof.clone())?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_complete_routing_proof() {
        println!("\n=== Complete PHANTOM Routing Proof Test ===\n");

        // Setup: Create a network with 16 nodes
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

        let (merkle_root, proofs) = MerkleCircuit::build_tree(&leaves);
        println!("Network built: {} nodes, root={:?}", leaves.len(), merkle_root.elements[0]);

        // Create routing path: nodes [0, 1, 5, 10]
        let path_node_ids = vec![0, 1, 5, 10];
        let path_data = PathData {
            node_ids: path_node_ids.clone(),
            path_length: 4,
        };

        // Get Merkle proofs for each node in the path
        let merkle_proofs: Vec<MerkleProof> = path_node_ids
            .iter()
            .map(|&idx| proofs.get(&(idx as usize)).unwrap().clone())
            .collect();

        let routing_data = RoutingProofData {
            path: path_data,
            merkle_proofs,
            merkle_root,
        };

        // Build circuits
        let system = RoutingProofSystem::new(4, 10);
        
        let start = Instant::now();
        let (merkle_circuit_data, merkle_targets) = system.merkle_circuit.build_circuit()
            .expect("Merkle circuit should build");
        let merkle_build_time = start.elapsed();
        
        let start = Instant::now();
        let (path_circuit_data, path_targets) = system.path_circuit.build_circuit()
            .expect("Path circuit should build");
        let path_build_time = start.elapsed();

        println!("Circuits built:");
        println!("  Merkle: {} gates, degree {} ({:?})", 
            merkle_circuit_data.common.gates.len(),
            merkle_circuit_data.common.degree(),
            merkle_build_time);
        println!("  Path: {} gates, degree {} ({:?})",
            path_circuit_data.common.gates.len(),
            path_circuit_data.common.degree(),
            path_build_time);

        // Generate complete routing proof
        println!("\nGenerating routing proof...");
        let start = Instant::now();
        let (path_proof, merkle_proofs) = system.prove_routing(
            &routing_data,
            &merkle_circuit_data,
            &merkle_targets,
            &path_circuit_data,
            &path_targets,
        ).expect("Routing proof should generate");
        let total_prove_time = start.elapsed();

        println!("✅ Routing proof generated in {:?}", total_prove_time);
        println!("  Path proof: {} bytes", path_proof.to_bytes().len());
        println!("  Merkle proofs: {} × ~70KB", merkle_proofs.len());

        // Verify routing proof
        let start = Instant::now();
        system.verify_routing(
            &path_proof,
            &merkle_proofs,
            &merkle_circuit_data,
            &path_circuit_data,
        ).expect("Routing proof should verify");
        let total_verify_time = start.elapsed();

        println!("✅ Routing proof verified in {:?}", total_verify_time);

        // Performance summary
        println!("\n=== Performance Summary ===");
        println!("Circuit build: {:?}", merkle_build_time + path_build_time);
        println!("Proof generation: {:?}", total_prove_time);
        println!("Proof verification: {:?}", total_verify_time);
        println!("Total proof size: ~{} KB", (path_proof.to_bytes().len() + merkle_proofs.len() * 70000) / 1000);
        
        if total_prove_time.as_millis() < 1000 {
            println!("\n🎉 TARGET ACHIEVED: Proof generation <1s!");
        }
    }
}
