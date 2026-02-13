//! Recursive Proof Aggregation for PHANTOM
//!
//! Aggregates multiple Merkle proofs into a single recursive proof using Plonky2's
//! recursive verification capabilities. This dramatically reduces proof size and
//! verification time for multi-hop routing.
//!
//! ## Performance Target
//! - **Without aggregation**: 4 Merkle proofs × 10ms = 40ms verification
//! - **With aggregation**: 1 aggregated proof = ~5ms verification
//! - **Proof size**: 4 × 70KB = 280KB → ~90KB (3.1x reduction)
//!
//! ## Approach
//! 1. Build recursive circuit that verifies N inner proofs
//! 2. Use Plonky2's `verify_proof` inside circuit
//! 3. Aggregate public inputs (all Merkle roots must match)
//! 4. Generate single proof that attests to all N verifications

use plonky2::field::types::Field;
use plonky2::hash::hash_types::HashOut;
use plonky2::iop::witness::{PartialWitness, WitnessWrite};
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::{CircuitConfig, CircuitData, VerifierCircuitTarget};
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};
use plonky2::plonk::proof::{ProofWithPublicInputs, ProofWithPublicInputsTarget};
use anyhow::Result;

const D: usize = 2;
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

/// Aggregation circuit that verifies multiple inner proofs recursively
pub struct AggregationCircuit {
    pub num_proofs: usize,
}

/// Targets for aggregation circuit
pub struct AggregationTargets {
    pub inner_proof_targets: Vec<ProofWithPublicInputsTarget<D>>,
    pub verifier_data_target: VerifierCircuitTarget,
}

impl AggregationCircuit {
    pub fn new(num_proofs: usize) -> Self {
        Self { num_proofs }
    }

    /// Build aggregation circuit that verifies N inner proofs
    /// 
    /// **Circuit Logic**:
    /// 1. For each inner proof, add recursive verification gadget
    /// 2. Assert all proofs verify against same verifier data
    /// 3. Extract and check public inputs (Merkle roots must match)
    /// 4. Output aggregated result as public input
    pub fn build_circuit(
        &self,
        inner_circuit_data: &CircuitData<F, C, D>,
    ) -> Result<(CircuitData<F, C, D>, AggregationTargets)> {
        let config = CircuitConfig::standard_recursion_config();
        let mut builder = CircuitBuilder::<F, D>::new(config);

        // Add verifier data as circuit target (constant)
        let verifier_data_target = builder.add_virtual_verifier_data(inner_circuit_data.common.config.fri_config.cap_height);

        // For each inner proof, add recursive verification
        let mut inner_proof_targets = Vec::new();
        
        for i in 0..self.num_proofs {
            // Add proof target (witness will be provided during proving)
            let proof_target = builder.add_virtual_proof_with_pis(&inner_circuit_data.common);
            
            // Verify proof recursively inside this circuit
            builder.verify_proof::<C>(
                &proof_target,
                &verifier_data_target,
                &inner_circuit_data.common,
            );
            
            inner_proof_targets.push(proof_target);
            
            println!("Added recursive verification for proof {}/{}", i + 1, self.num_proofs);
        }

        // Assert all Merkle roots match (first 4 public inputs of each proof)
        if self.num_proofs > 1 {
            let first_root = &inner_proof_targets[0].public_inputs[0..4];
            
            for i in 1..self.num_proofs {
                let current_root = &inner_proof_targets[i].public_inputs[0..4];
                
                for j in 0..4 {
                    builder.connect(first_root[j], current_root[j]);
                }
            }
        }

        // Register first Merkle root as public output (all roots are equal)
        if !inner_proof_targets.is_empty() {
            for j in 0..4 {
                builder.register_public_input(inner_proof_targets[0].public_inputs[j]);
            }
        }

        let targets = AggregationTargets {
            inner_proof_targets,
            verifier_data_target,
        };

        let circuit_data = builder.build::<C>();
        
        println!("Aggregation circuit built:");
        println!("  Proofs aggregated: {}", self.num_proofs);
        println!("  Gates: {}", circuit_data.common.gates.len());
        println!("  Degree: {}", circuit_data.common.degree_bits());

        Ok((circuit_data, targets))
    }

    /// Generate aggregated proof from N inner proofs
    /// 
    /// **Inputs**:
    /// - `inner_proofs`: The N Merkle proofs to aggregate
    /// - `inner_circuit_data`: CircuitData of the Merkle circuit
    /// - `agg_circuit_data`: CircuitData of this aggregation circuit
    /// - `agg_targets`: Targets for witness assignment
    /// 
    /// **Output**: Single proof that verifies all N inner proofs
    pub fn prove(
        &self,
        inner_proofs: &[ProofWithPublicInputs<F, C, D>],
        inner_circuit_data: &CircuitData<F, C, D>,
        agg_circuit_data: &CircuitData<F, C, D>,
        agg_targets: &AggregationTargets,
    ) -> Result<ProofWithPublicInputs<F, C, D>> {
        if inner_proofs.len() != self.num_proofs {
            anyhow::bail!(
                "Expected {} inner proofs, got {}",
                self.num_proofs,
                inner_proofs.len()
            );
        }

        let mut pw = PartialWitness::new();

        // Set verifier data (constant from inner circuit)
        pw.set_verifier_data_target(
            &agg_targets.verifier_data_target,
            &inner_circuit_data.verifier_only,
        );

        // Set each inner proof as witness
        for (i, proof) in inner_proofs.iter().enumerate() {
            pw.set_proof_with_pis_target(&agg_targets.inner_proof_targets[i], proof);
        }

        // Generate aggregated proof
        let proof = agg_circuit_data.prove(pw)?;

        Ok(proof)
    }

    /// Verify aggregated proof
    pub fn verify(
        &self,
        proof: &ProofWithPublicInputs<F, C, D>,
        agg_circuit_data: &CircuitData<F, C, D>,
    ) -> Result<()> {
        agg_circuit_data.verify(proof.clone())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle::{MerkleCircuit, MerkleProof};
    use plonky2::hash::hash_types::HashOut;

    #[test]
    fn test_aggregation_circuit_builds() {
        println!("\n=== Aggregation Circuit Build Test ===\n");

        // Build Merkle circuit (this will be the inner circuit)
        let merkle_circuit = MerkleCircuit::new(4); // 4-level tree
        let (merkle_circuit_data, _merkle_targets) = merkle_circuit
            .build_circuit()
            .expect("Failed to build Merkle circuit");

        println!("Inner Merkle circuit:");
        println!("  Gates: {}", merkle_circuit_data.common.gates.len());
        println!("  Degree: {}", merkle_circuit_data.common.degree_bits());

        // Build aggregation circuit for 4 proofs
        let agg_circuit = AggregationCircuit::new(4);
        let (agg_circuit_data, _agg_targets) = agg_circuit
            .build_circuit(&merkle_circuit_data)
            .expect("Failed to build aggregation circuit");

        println!("\nAggregation circuit:");
        println!("  Gates: {}", agg_circuit_data.common.gates.len());
        println!("  Degree: {}", agg_circuit_data.common.degree_bits());
        
        assert!(agg_circuit_data.common.gates.len() > 0);
    }

    #[test]
    fn test_aggregation_proof_generation() {
        println!("\n=== Aggregation Proof Generation Test ===\n");

        // Build Merkle tree
        let leaves: Vec<HashOut<F>> = (0..16)
            .map(|i| {
                let val = F::from_canonical_u64(i);
                HashOut {
                    elements: [val, F::ZERO, F::ZERO, F::ZERO],
                }
            })
            .collect();

        let merkle_circuit = MerkleCircuit::new(4);
        let (root, proofs_map) = MerkleCircuit::build_tree(&leaves);
        
        println!("Built Merkle tree:");
        println!("  Leaves: {}", leaves.len());
        println!("  Root: {:?}", root.elements[0]);

        // Build circuits
        let (merkle_circuit_data, merkle_targets) = merkle_circuit
            .build_circuit()
            .expect("Failed to build Merkle circuit");

        // Generate 4 Merkle proofs for different leaves
        let mut inner_proofs = Vec::new();
        for i in 0..4 {
            let proof_data = proofs_map.get(&i).expect("Proof not found");
            let merkle_proof = merkle_circuit
                .prove(&merkle_circuit_data, &merkle_targets, proof_data)
                .expect("Failed to generate Merkle proof");
            
            inner_proofs.push(merkle_proof);
            println!("Generated Merkle proof {}/4", i + 1);
        }

        // Build aggregation circuit
        let agg_circuit = AggregationCircuit::new(4);
        let (agg_circuit_data, agg_targets) = agg_circuit
            .build_circuit(&merkle_circuit_data)
            .expect("Failed to build aggregation circuit");

        // Generate aggregated proof
        let start = std::time::Instant::now();
        let aggregated_proof = agg_circuit
            .prove(
                &inner_proofs,
                &merkle_circuit_data,
                &agg_circuit_data,
                &agg_targets,
            )
            .expect("Failed to generate aggregated proof");
        let prove_time = start.elapsed();

        println!("\n✅ Aggregated proof generated in {:?}", prove_time);
        println!("  Proof size: {} bytes", bincode::serialize(&aggregated_proof).unwrap().len());

        // Verify aggregated proof
        let start = std::time::Instant::now();
        agg_circuit
            .verify(&aggregated_proof, &agg_circuit_data)
            .expect("Aggregated proof verification failed");
        let verify_time = start.elapsed();

        println!("✅ Aggregated proof verified in {:?}", verify_time);
        
        // Compare to sequential verification
        let start = std::time::Instant::now();
        for proof in &inner_proofs {
            merkle_circuit_data.verify(proof.clone()).expect("Merkle proof verification failed");
        }
        let sequential_verify_time = start.elapsed();

        println!("\nPerformance comparison:");
        println!("  Sequential verification (4 proofs): {:?}", sequential_verify_time);
        println!("  Aggregated verification (1 proof): {:?}", verify_time);
        println!("  Speedup: {:.2}x", sequential_verify_time.as_secs_f64() / verify_time.as_secs_f64());
    }
}
