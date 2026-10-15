//! Post-Quantum Cryptography Module
//!
//! Provides quantum-resistant cryptographic primitives using NIST-approved algorithms:
//! - Kyber-1024 for key encapsulation (256-bit quantum security)
//! - Dilithium-5 for digital signatures
//! - SPHINCS+ for hash-based signatures (backup)

use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use chacha20poly1305::aead::{Aead, KeyInit};
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
pub struct PublicKey(#[serde(with = "serde_bytes")] pub Vec<u8>);

/// Secret key for Kyber-1024
#[derive(Clone)]
pub struct SecretKey(pub Vec<u8>);

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
pub struct SigningPublicKey(#[serde(with = "serde_bytes")] pub Vec<u8>);

/// Secret key for Dilithium-5
#[derive(Clone)]
pub struct SigningSecretKey(pub Vec<u8>);

/// Detached signature
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Signature(#[serde(with = "serde_bytes")] pub Vec<u8>);

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

// Security: Don't expose secret key bytes in logs, but allow serialization
impl SecretKey {
    pub fn to_bytes(&self) -> &[u8] { &self.0 }
    pub fn from_bytes(bytes: Vec<u8>) -> Self { Self(bytes) }
}

impl SigningSecretKey {
    pub fn to_bytes(&self) -> &[u8] { &self.0 }
    pub fn from_bytes(bytes: Vec<u8>) -> Self { Self(bytes) }
}

impl Signature {
    pub fn to_bytes(&self) -> &[u8] { &self.0 }
    pub fn from_bytes(bytes: Vec<u8>) -> Self { Self(bytes) }
}
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

/// Hybrid-encrypted payload: ML-KEM-1024 encapsulation + ChaCha20-Poly1305.
///
/// The sender runs `EncryptedPayload::encrypt(plaintext, recipient_kem_public)`.
/// Each call generates a fresh KEM ciphertext (and therefore a fresh shared
/// secret), so the ChaCha20-Poly1305 key is unique per message; a 12-byte
/// random nonce then removes any remaining nonce-collision risk.
/// The recipient runs `encrypted.decrypt(kem_secret)` to recover the plaintext.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedPayload {
    /// ML-KEM-1024 ciphertext (~1568 bytes). Decapsulate to get the symmetric key.
    pub kem_ciphertext: Vec<u8>,
    /// Random ChaCha20-Poly1305 nonce (12 bytes).
    pub nonce: [u8; 12],
    /// ChaCha20-Poly1305 ciphertext + 16-byte authentication tag.
    pub ciphertext: Vec<u8>,
}

impl EncryptedPayload {
    /// Encrypt `plaintext` for the owner of `recipient_kem_public` (raw ML-KEM-1024 bytes).
    pub fn encrypt(plaintext: &[u8], recipient_kem_public: &[u8]) -> Result<Self> {
        let pk = PublicKey::from_bytes(recipient_kem_public.to_vec());
        let (kem_ciphertext, shared_secret) = KeyPair::encapsulate(&pk)?;

        let nonce_bytes: [u8; 12] = rand::random();
        let key = Key::from_slice(&shared_secret.0);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::KeyExchangeError(format!("payload encryption failed: {e}")))?;

        Ok(Self { kem_ciphertext, nonce: nonce_bytes, ciphertext })
    }

    /// Decrypt this payload using the recipient's raw ML-KEM-1024 secret key bytes.
    pub fn decrypt(&self, recipient_kem_secret: &[u8]) -> Result<Vec<u8>> {
        let sk = kyber1024::SecretKey::from_bytes(recipient_kem_secret)
            .map_err(|e| CryptoError::KeyExchangeError(format!("invalid secret key: {e:?}")))?;
        let ct = kyber1024::Ciphertext::from_bytes(&self.kem_ciphertext)
            .map_err(|e| CryptoError::KeyExchangeError(format!("invalid KEM ciphertext: {e:?}")))?;
        let ss = kyber1024::decapsulate(&ct, &sk);

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&ss.as_bytes()[..32]);

        let key = Key::from_slice(&key_bytes);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(&self.nonce);

        cipher
            .decrypt(nonce, self.ciphertext.as_slice())
            .map_err(|_| CryptoError::KeyExchangeError("payload decryption failed (bad key or tampered ciphertext)".to_string()))
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

    #[test]
    fn encrypted_payload_roundtrip() {
        let keypair = KeyPair::generate();
        let plaintext = b"PHANTOM end-to-end encrypted message";

        let encrypted = EncryptedPayload::encrypt(plaintext, keypair.public.to_bytes()).unwrap();

        // Wire form must not contain the plaintext.
        let wire = bincode::serialize(&encrypted).unwrap();
        assert!(
            wire.windows(plaintext.len()).all(|w| w != plaintext.as_slice()),
            "plaintext found in encrypted payload"
        );

        // Recipient decrypts successfully.
        let recovered = encrypted.decrypt(keypair.secret.to_bytes()).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn encrypted_payload_wrong_key_fails() {
        let sender_target = KeyPair::generate();
        let unrelated = KeyPair::generate();

        let encrypted = EncryptedPayload::encrypt(b"secret", sender_target.public.to_bytes()).unwrap();
        assert!(
            encrypted.decrypt(unrelated.secret.to_bytes()).is_err(),
            "decryption with wrong key must fail"
        );
    }
}
