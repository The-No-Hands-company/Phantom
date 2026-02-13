//! Post-Quantum Cryptography Module
//!
//! Provides quantum-resistant cryptographic primitives using NIST-approved algorithms:
//! - Kyber-1024 for key encapsulation (256-bit quantum security)
//! - Dilithium-5 for digital signatures
//! - SPHINCS+ for hash-based signatures (backup)

use pqcrypto_kyber::kyber1024;
use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::kem::{Ciphertext as _, PublicKey as _, SecretKey as _, SharedSecret as _};
use pqcrypto_traits::sign::{PublicKey as SignPk, SecretKey as SignSk, DetachedSignature};
use serde::{Deserialize, Serialize};
use crate::{CryptoError, Result};

/// Post-quantum key pair for key encapsulation
#[derive(Clone)]
pub struct KeyPair {
    pub public: PublicKey,
    pub secret: SecretKey,
}

/// Public key for Kyber-1024
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PublicKey(#[serde(with = "serde_bytes")] Vec<u8>);

/// Secret key for Kyber-1024
#[derive(Clone)]
pub struct SecretKey(Vec<u8>);

/// Shared secret from key exchange
#[derive(Clone)]
pub struct SharedSecret(pub [u8; 32]);

/// Digital signature key pair
#[derive(Clone)]
pub struct SigningKeyPair {
    pub public: SigningPublicKey,
    pub secret: SigningSecretKey,
}

/// Public key for Dilithium-5
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SigningPublicKey(#[serde(with = "serde_bytes")] Vec<u8>);

/// Secret key for Dilithium-5
#[derive(Clone)]
pub struct SigningSecretKey(Vec<u8>);

/// Detached signature
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signature(#[serde(with = "serde_bytes")] Vec<u8>);

impl KeyPair {
    /// Generate a new post-quantum key pair
    ///
    /// Uses Kyber-1024 for 256-bit quantum security.
    /// Generation time: ~50 microseconds
    pub fn generate() -> Self {
        let (pk, sk) = kyber1024::keypair();
        Self {
            public: PublicKey(pk.as_bytes().to_vec()),
            secret: SecretKey(sk.as_bytes().to_vec()),
        }
    }

    /// Encapsulate a shared secret to the public key
    ///
    /// Returns (ciphertext, shared_secret)
    /// Encapsulation time: ~70 microseconds
    pub fn encapsulate(public_key: &PublicKey) -> Result<(Vec<u8>, SharedSecret)> {
        let pk = kyber1024::PublicKey::from_bytes(&public_key.0)
            .map_err(|e| CryptoError::KeyExchangeError(format!("Invalid public key: {:?}", e)))?;
        
        let (ss, ct) = kyber1024::encapsulate(&pk);
        
        // Convert shared secret to 32 bytes
        let ss_bytes = ss.as_bytes();
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&ss_bytes[..32]);
        
        Ok((ct.as_bytes().to_vec(), SharedSecret(secret)))
    }

    /// Decapsulate a ciphertext to recover the shared secret
    ///
    /// Decapsulation time: ~90 microseconds
    pub fn decapsulate(&self, ciphertext: &[u8]) -> Result<SharedSecret> {
        let sk = kyber1024::SecretKey::from_bytes(&self.secret.0)
            .map_err(|e| CryptoError::KeyExchangeError(format!("Invalid secret key: {:?}", e)))?;
        
        let ct = kyber1024::Ciphertext::from_bytes(ciphertext)
            .map_err(|e| CryptoError::KeyExchangeError(format!("Invalid ciphertext: {:?}", e)))?;
        
        let ss = kyber1024::decapsulate(&ct, &sk);
        
        let ss_bytes = ss.as_bytes();
        let mut secret = [0u8; 32];
        secret.copy_from_slice(&ss_bytes[..32]);
        
        Ok(SharedSecret(secret))
    }
}

impl SigningKeyPair {
    /// Generate a new signing key pair
    ///
    /// Uses Dilithium-5 for maximum security.
    pub fn generate() -> Self {
        let (pk, sk) = dilithium5::keypair();
        Self {
            public: SigningPublicKey(pk.as_bytes().to_vec()),
            secret: SigningSecretKey(sk.as_bytes().to_vec()),
        }
    }

    /// Sign a message
    pub fn sign(&self, message: &[u8]) -> Result<Signature> {
        let sk = dilithium5::SecretKey::from_bytes(&self.secret.0)
            .map_err(|e| CryptoError::KeyExchangeError(format!("Invalid signing key: {:?}", e)))?;
        
        let sig = dilithium5::detached_sign(message, &sk);
        Ok(Signature(sig.as_bytes().to_vec()))
    }

    /// Verify a signature
    pub fn verify(public_key: &SigningPublicKey, message: &[u8], signature: &Signature) -> Result<bool> {
        let pk = dilithium5::PublicKey::from_bytes(&public_key.0)
            .map_err(|e| CryptoError::KeyExchangeError(format!("Invalid public key: {:?}", e)))?;
        
        let sig = dilithium5::DetachedSignature::from_bytes(&signature.0)
            .map_err(|e| CryptoError::KeyExchangeError(format!("Invalid signature: {:?}", e)))?;
        
        match dilithium5::verify_detached_signature(&sig, message, &pk) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

impl PublicKey {
    pub fn to_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl SigningPublicKey {
    pub fn to_bytes(&self) -> &[u8] {
        &self.0
    }

    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

// Security: Don't expose secret key bytes
impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SecretKey([REDACTED])")
    }
}

impl std::fmt::Debug for SigningSecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SigningSecretKey([REDACTED])")
    }
}

impl std::fmt::Debug for SharedSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SharedSecret([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kyber_key_exchange() {
        // Generate key pair
        let keypair = KeyPair::generate();
        
        // Encapsulate
        let (ciphertext, ss1) = KeyPair::encapsulate(&keypair.public).unwrap();
        
        // Decapsulate
        let ss2 = keypair.decapsulate(&ciphertext).unwrap();
        
        // Shared secrets should match
        assert_eq!(ss1.0, ss2.0);
    }

    #[test]
    fn test_dilithium_signatures() {
        let signing_keypair = SigningKeyPair::generate();
        let message = b"PHANTOM protocol - quantum-resistant anonymous networking";
        
        // Sign
        let signature = signing_keypair.sign(message).unwrap();
        
        // Verify
        let valid = SigningKeyPair::verify(&signing_keypair.public, message, &signature).unwrap();
        assert!(valid);
        
        // Wrong message should fail
        let wrong_message = b"Wrong message";
        let invalid = SigningKeyPair::verify(&signing_keypair.public, wrong_message, &signature).unwrap();
        assert!(!invalid);
    }

    #[test]
    fn test_key_serialization() {
        let keypair = KeyPair::generate();
        let bytes = keypair.public.to_bytes();
        let reconstructed = PublicKey::from_bytes(bytes.to_vec());
        assert_eq!(keypair.public.0, reconstructed.0);
    }
}
