//! RISC Zero zkVM Integration
//!
//! Production STARK proofs using RISC Zero zkVM
//!
//! This module replaces hash-based placeholders with real zero-knowledge proofs
//! that can be verified by anyone without revealing the routing path.

use risc0_zkvm::{default_prover, ExecutorEnv, Receipt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// Import the compiled guest program
use phantom_zkvm_methods::PHANTOM_ROUTE_VALIDATOR_ELF;

/// RISC Zero-based routing proof
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Risc0RoutingProof {
    /// The RISC Zero receipt (contains the STARK proof)
    #[serde(with = "serde_bytes")]
    pub receipt_data: Vec<u8>,
    
    /// Public inputs
    pub public_inputs: PublicInputs,
}

/// Public inputs to the circuit (same as hash-based version)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublicInputs {
    pub network_commitment: [u8; 32],
    pub path_length: usize,
    pub timestamp: u64,
}

/// Private inputs to the circuit (hidden from verifier)
#[derive(Serialize, Deserialize)]
struct RoutingPath {
    hops: Vec<u32>,
}

/// Merkle proof for node membership
#[derive(Serialize, Deserialize, Clone)]
pub struct MerkleProof {
    pub leaf_index: u64,
    pub siblings: Vec<[u8; 32]>,
}

/// RISC Zero proof generator
pub struct Risc0ProofGenerator {
    /// Cache for performance
    proof_cache: std::sync::RwLock<std::collections::HashMap<[u8; 32], Risc0RoutingProof>>,
}

impl Risc0ProofGenerator {
    /// Create a new RISC Zero proof generator
    pub fn new() -> Self {
        Self {
            proof_cache: std::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
    
    /// Generate a RISC Zero STARK proof of path validity
    ///
    /// This generates a real zero-knowledge proof that:
    /// - Path is 3-7 hops
    /// - No loops exist
    /// - All nodes are in the network (Merkle membership)
    ///
    /// The proof can be verified by anyone without revealing the path!
    pub fn generate_proof(
        &self,
        path: &[u32],
        merkle_proofs: &[MerkleProof],
        network_commitment: &[u8; 32],
    ) -> anyhow::Result<Risc0RoutingProof> {
        // Pre-validate inputs (fast fail before expensive zkVM execution)
        if path.len() < 3 {
            return Err(anyhow::anyhow!("Path too short: minimum 3 hops"));
        }
        if path.len() > 7 {
            return Err(anyhow::anyhow!("Path too long: maximum 7 hops"));
        }
        
        // Check for loops
        let unique_nodes: HashSet<_> = path.iter().collect();
        if unique_nodes.len() != path.len() {
            return Err(anyhow::anyhow!("Path contains loops"));
        }
        
        // Verify we have Merkle proofs for all nodes
        if merkle_proofs.len() != path.len() {
            return Err(anyhow::anyhow!(
                "Must provide Merkle proof for each hop ({} hops, {} proofs)",
                path.len(),
                merkle_proofs.len()
            ));
        }
        
        // Create public inputs
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let public_inputs = PublicInputs {
            network_commitment: *network_commitment,
            path_length: path.len(),
            timestamp,
        };
        
        // Prepare private witness
        let routing_path = RoutingPath {
            hops: path.to_vec(),
        };
        
        // ============================================================
        // RISC ZERO PROOF GENERATION
        // ============================================================
        
        // Create execution environment
        // RISC Zero requires Vec<T> for complex types, not &[T]
        let merkle_proofs_vec = merkle_proofs.to_vec();
        
        let env = ExecutorEnv::builder()
            .write(&public_inputs)?
            .write(&routing_path)?
            .write(&merkle_proofs_vec)?
            .build()?;
        
        // Run the guest program inside zkVM and generate proof
        println!("⏳ Generating RISC Zero proof (this may take 10-30 seconds)...");
        let start = std::time::Instant::now();
        
        let prover = default_prover();
        let prove_info = prover.prove(env, PHANTOM_ROUTE_VALIDATOR_ELF)?;
        
        let proof_time = start.elapsed();
        println!("✅ Proof generated in {:?}", proof_time);
        
        // RISC Zero 3.0 returns ProveInfo with receipt field
        let receipt = prove_info.receipt;
        let journal_bytes = receipt.journal.bytes.clone();
        println!("📊 Journal size: {} bytes", journal_bytes.len());
        
        // Serialize receipt for storage/transmission
        let receipt_data = bincode::serialize(&receipt)?;
        
        let proof = Risc0RoutingProof {
            receipt_data,
            public_inputs,
        };
        
        // Cache the proof
        let cache_key = blake3::hash(network_commitment).into();
        self.proof_cache.write().unwrap().insert(cache_key, proof.clone());
        
        Ok(proof)
    }
    
    /// Verify a RISC Zero proof
    ///
    /// Returns true if the STARK proof is valid
    /// This is FAST (~1-10ms) - only the prover is slow
    pub fn verify_proof(
        &self,
        proof: &Risc0RoutingProof,
        network_commitment: &[u8; 32],
    ) -> anyhow::Result<bool> {
        // Check network commitment matches
        if proof.public_inputs.network_commitment != *network_commitment {
            return Ok(false);
        }
        
        // Check path length constraints
        if proof.public_inputs.path_length < 3 || proof.public_inputs.path_length > 7 {
            return Ok(false);
        }
        
        // Check proof freshness (1 hour)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let proof_age = now.saturating_sub(proof.public_inputs.timestamp);
        if proof_age > 3600 {
            return Ok(false);
        }
        
        // ============================================================
        // RISC ZERO PROOF VERIFICATION
        // ============================================================
        
        let start = std::time::Instant::now();
        
        // Deserialize receipt
        let receipt: Receipt = bincode::deserialize(&proof.receipt_data)?;
        
        // Verify the STARK proof
        // RISC Zero 3.0 requires image_id (Digest) for verification
        // The image_id is computed from the ELF and embedded in methods module
        use phantom_zkvm_methods::PHANTOM_ROUTE_VALIDATOR_ID;
        receipt.verify(PHANTOM_ROUTE_VALIDATOR_ID)?;
        
        let verify_time = start.elapsed();
        println!("✅ Proof verified in {:?}", verify_time);
        
        Ok(true)
    }
}

impl Default for Risc0ProofGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    #[ignore] // Slow test - RISC Zero proof generation takes ~10-30s
    fn test_risc0_proof_generation_and_verification() {
        let generator = Risc0ProofGenerator::new();
        
        // Create a simple path
        let path = vec![100, 200, 300];
        let network_commitment = [1u8; 32];
        
        // Create dummy Merkle proofs (in production, these come from NetworkGraph)
        let merkle_proofs: Vec<MerkleProof> = (0..3)
            .map(|i| MerkleProof {
                leaf_index: i,
                siblings: vec![[0u8; 32]; 10], // depth-10 tree
            })
            .collect();
        
        // Generate proof
        let proof = generator
            .generate_proof(&path, &merkle_proofs, &network_commitment)
            .expect("Proof generation should succeed");
        
        // Verify proof
        let valid = generator
            .verify_proof(&proof, &network_commitment)
            .expect("Verification should succeed");
        
        assert!(valid, "Proof should be valid");
    }
    
    #[test]
    fn test_path_validation_before_zkvm() {
        let generator = Risc0ProofGenerator::new();
        
        // Path too short
        let result = generator.generate_proof(&[1, 2], &[], &[0u8; 32]);
        assert!(result.is_err());
        
        // Path too long
        let long_path = vec![1, 2, 3, 4, 5, 6, 7, 8];
        let result = generator.generate_proof(&long_path, &[], &[0u8; 32]);
        assert!(result.is_err());
        
        // Path with loop
        let loop_path = vec![1, 2, 3, 2, 5];
        let result = generator.generate_proof(&loop_path, &[], &[0u8; 32]);
        assert!(result.is_err());
    }
}
