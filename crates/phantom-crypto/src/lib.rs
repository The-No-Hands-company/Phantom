//! PHANTOM Cryptographic Primitives
//!
//! This crate provides the core cryptographic building blocks for the PHANTOM protocol:
//! - Post-quantum key exchange (Kyber)
//! - Post-quantum signatures (Dilithium)
//! - Fully Homomorphic Encryption (TFHE)
//! - Zero-knowledge proof systems (Halo2)

pub mod pq;
pub mod fhe;
pub mod zk;
pub mod primitives;

pub use pq::{KeyPair, PublicKey, SecretKey, SharedSecret};
pub use fhe::{FheEngine, EncryptedValue, ServerKey, ClientKey};
pub use zk::{ProofSystem, Proof, Circuit};

/// Re-export common types
pub use blake3::Hash;
pub use rand_core::{RngCore, CryptoRng};

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Post-quantum key exchange failed: {0}")]
    KeyExchangeError(String),
    
    #[error("FHE operation failed: {0}")]
    FheError(String),
    
    #[error("Zero-knowledge proof generation failed: {0}")]
    ProofGenerationError(String),
    
    #[error("Proof verification failed")]
    ProofVerificationError,
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

pub type Result<T> = std::result::Result<T, CryptoError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_loads() {
        // Basic smoke test
        assert!(true);
    }
}
