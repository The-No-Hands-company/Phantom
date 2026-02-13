//! Error types for PHANTOM protocol
//!
//! Cloudflare-proof error handling: graceful failures instead of panics

use std::fmt;

/// Protocol-level errors
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolError {
    /// Invalid routing path configuration
    InvalidPath(String),
    
    /// Packet construction failed
    PacketConstruction(String),
    
    /// Packet verification failed
    PacketVerification(String),
    
    /// Network topology error
    NetworkError(String),
    
    /// Serialization/deserialization error
    SerializationError(String),
    
    /// Cryptographic operation failed
    CryptoError(String),
    
    /// Invalid sender credentials (membership proof)
    InvalidCredentials(String),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ProtocolError::InvalidPath(msg) => write!(f, "Invalid path: {}", msg),
            ProtocolError::PacketConstruction(msg) => write!(f, "Packet construction failed: {}", msg),
            ProtocolError::PacketVerification(msg) => write!(f, "Packet verification failed: {}", msg),
            ProtocolError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ProtocolError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ProtocolError::CryptoError(msg) => write!(f, "Crypto error: {}", msg),
            ProtocolError::InvalidCredentials(msg) => write!(f, "Invalid credentials: {}", msg),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// Convert from phantom_crypto::CryptoError
impl From<phantom_crypto::CryptoError> for ProtocolError {
    fn from(err: phantom_crypto::CryptoError) -> Self {
        ProtocolError::CryptoError(err.to_string())
    }
}

/// Convert from bincode errors
impl From<bincode::Error> for ProtocolError {
    fn from(err: bincode::Error) -> Self {
        ProtocolError::SerializationError(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
