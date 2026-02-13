//! Zero-knowledge proof interface for PHANTOM routing
//!
//! This module defines the trait and types for generating and verifying
//! zero-knowledge proofs of routing path validity. Implementations live
//! in phantom-zkvm (RISC Zero, SP1, etc.)

use serde::{Deserialize, Serialize};

/// A zero-knowledge proof of routing path validity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingProof {
    /// The proof data (STARK/SNARK bytes)
    pub proof_data: Vec<u8>,
    
    /// Public inputs: network commitment, path length, timestamp
    pub public_inputs: PublicInputs,
}

/// Public inputs to the routing proof
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublicInputs {
    /// Commitment to the network topology (Merkle root)
    pub network_commitment: [u8; 32],
    
    /// Number of hops in the path
    pub path_length: usize,
    
    /// Timestamp of proof generation (for freshness)
    pub timestamp: u64,
}

/// Merkle proof for node membership in network
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MerkleProof {
    /// Node ID being proven
    pub node_id: u32,
    
    /// Merkle path (sibling hashes from leaf to root)
    pub path: Vec<[u8; 32]>,
    
    /// Position bits (0 = left, 1 = right)
    pub positions: Vec<bool>,
}

/// Trait for generating and verifying zero-knowledge proofs
///
/// Implementations:
/// - Hash-based (fast, for testing) - in phantom-zkvm
/// - RISC Zero (production STARKs) - in phantom-zkvm
/// - SP1 (alternative zkVM) - future
pub trait ProofGenerator: Send + Sync {
    /// Generate a proof that a path is valid within the network
    ///
    /// This proves:
    /// - All nodes exist in the network (Merkle membership)
    /// - Path length is valid (3-7 hops)
    /// - No loops in the path
    /// - Path is consistent with network commitment
    fn generate_path_proof(
        &self,
        path: &[u32],
        network_commitment: &[u8; 32],
        merkle_proofs: &[MerkleProof],
    ) -> anyhow::Result<RoutingProof>;
    
    /// Verify a routing proof
    ///
    /// Returns true if the proof is cryptographically valid and fresh
    fn verify_path_proof(
        &self,
        proof: &RoutingProof,
        network_commitment: &[u8; 32],
    ) -> anyhow::Result<bool>;
}
