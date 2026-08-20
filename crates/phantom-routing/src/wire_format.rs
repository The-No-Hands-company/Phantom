//! Wire Format for PHANTOM Protocol
//!
//! Defines the serialization format for network transmission.
//! Optimized for minimal overhead while maintaining security guarantees.

use phantom_core::PhantomPacket;
use serde::{Deserialize, Serialize};

/// Wire protocol version
pub const PROTOCOL_VERSION: u8 = 1;

/// Maximum packet size (10 MB for FHE-encrypted routing tables)
pub const MAX_PACKET_SIZE: usize = 10 * 1024 * 1024;

/// Minimum packet size (routing blob + proof + nullifier)
pub const MIN_PACKET_SIZE: usize = 1024;

/// Wire format header
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireHeader {
    /// Protocol version (1 byte)
    pub version: u8,
    
    /// Total packet length including header (4 bytes)
    pub packet_length: u32,
    
    /// Routing blob length (4 bytes)
    pub routing_blob_length: u32,
    
    /// Path proof length (4 bytes)
    pub proof_length: u32,
    
    /// Payload length (4 bytes)
    pub payload_length: u32,
    
    /// Reserved for future use (3 bytes for alignment)
    pub reserved: [u8; 3],
}

impl WireHeader {
    /// Header size in bytes (20 bytes total)
    pub const SIZE: usize = 20;
    
    /// Create a new wire header
    pub fn new(packet: &PhantomPacket) -> Self {
        let routing_blob_length = packet.routing_blob.len() as u32;
        let proof_length = packet.path_proof.proof_data.len() as u32;
        let payload_length = packet.payload.len() as u32;
        
        let packet_length = Self::SIZE as u32
            + routing_blob_length
            + proof_length
            + payload_length
            + 32; // nullifier
        
        Self {
            version: PROTOCOL_VERSION,
            packet_length,
            routing_blob_length,
            proof_length,
            payload_length,
            reserved: [0; 3],
        }
    }
    
    /// Validate header constraints
    pub fn validate(&self) -> Result<(), WireError> {
        // Check protocol version
        if self.version != PROTOCOL_VERSION {
            return Err(WireError::UnsupportedVersion(self.version));
        }
        
        // Check packet size bounds
        let packet_len = self.packet_length as usize;
        if packet_len > MAX_PACKET_SIZE {
            return Err(WireError::PacketTooLarge(packet_len));
        }
        
        if packet_len < MIN_PACKET_SIZE {
            return Err(WireError::PacketTooSmall(packet_len));
        }
        
        // Verify length consistency
        let expected_length = Self::SIZE as u32
            + self.routing_blob_length
            + self.proof_length
            + self.payload_length
            + 32; // nullifier
        
        if self.packet_length != expected_length {
            return Err(WireError::InvalidLength {
                expected: expected_length,
                actual: self.packet_length,
            });
        }
        
        Ok(())
    }
    
    /// Serialize header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::SIZE);
        bytes.push(self.version);
        bytes.extend(&self.packet_length.to_le_bytes());
        bytes.extend(&self.routing_blob_length.to_le_bytes());
        bytes.extend(&self.proof_length.to_le_bytes());
        bytes.extend(&self.payload_length.to_le_bytes());
        bytes.extend(&self.reserved);
        bytes
    }
    
    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WireError> {
        if bytes.len() < Self::SIZE {
            return Err(WireError::InvalidHeader("Header too short".to_string()));
        }
        
        let version = bytes[0];
        let packet_length = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
        let routing_blob_length = u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]);
        let proof_length = u32::from_le_bytes([bytes[9], bytes[10], bytes[11], bytes[12]]);
        let payload_length = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], bytes[16]]);
        let reserved = [bytes[17], bytes[18], bytes[19]];
        
        let header = Self {
            version,
            packet_length,
            routing_blob_length,
            proof_length,
            payload_length,
            reserved,
        };
        
        header.validate()?;
        Ok(header)
    }
}

/// Wire format errors
#[derive(Debug, Clone, thiserror::Error)]
pub enum WireError {
    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(u8),
    
    #[error("Packet too large: {0} bytes (max {MAX_PACKET_SIZE})")]
    PacketTooLarge(usize),
    
    #[error("Packet too small: {0} bytes (min {MIN_PACKET_SIZE})")]
    PacketTooSmall(usize),
    
    #[error("Invalid packet length: expected {expected}, got {actual}")]
    InvalidLength { expected: u32, actual: u32 },
    
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Incomplete packet: expected {expected} bytes, got {actual}")]
    IncompletePacket { expected: usize, actual: usize },
}

/// Serialize a PHANTOM packet to wire format
pub fn serialize_packet(packet: &PhantomPacket) -> Result<Vec<u8>, WireError> {
    // Create header
    let header = WireHeader::new(packet);
    
    // Allocate buffer
    let mut buffer = Vec::with_capacity(header.packet_length as usize);
    
    // Write header
    buffer.extend(header.to_bytes());
    
    // Write routing blob
    buffer.extend(&packet.routing_blob);
    
    // Write path proof
    buffer.extend(&packet.path_proof.proof_data);
    
    // Write payload
    buffer.extend(&packet.payload);
    
    // Write nullifier
    buffer.extend(&packet.nullifier);
    
    Ok(buffer)
}

/// Deserialize a PHANTOM packet from wire format
pub fn deserialize_packet(bytes: &[u8]) -> Result<PhantomPacket, WireError> {
    // Parse header
    if bytes.len() < WireHeader::SIZE {
        return Err(WireError::InvalidHeader("Packet too short".to_string()));
    }
    
    let header = WireHeader::from_bytes(&bytes[0..WireHeader::SIZE])?;
    
    // Verify we have the full packet
    if bytes.len() < header.packet_length as usize {
        return Err(WireError::IncompletePacket {
            expected: header.packet_length as usize,
            actual: bytes.len(),
        });
    }
    
    // Parse packet components
    let mut offset = WireHeader::SIZE;
    
    // Routing blob
    let routing_blob_end = offset + header.routing_blob_length as usize;
    let routing_blob = bytes[offset..routing_blob_end].to_vec();
    offset = routing_blob_end;
    
    // Path proof
    let proof_end = offset + header.proof_length as usize;
    let proof_data = bytes[offset..proof_end].to_vec();
    offset = proof_end;
    
    // Payload
    let payload_end = offset + header.payload_length as usize;
    let payload = bytes[offset..payload_end].to_vec();
    offset = payload_end;
    
    // Nullifier
    if offset + 32 > bytes.len() {
        return Err(WireError::IncompletePacket {
            expected: offset + 32,
            actual: bytes.len(),
        });
    }
    let mut nullifier = [0u8; 32];
    nullifier.copy_from_slice(&bytes[offset..offset + 32]);
    
    // Reconstruct packet
    // Note: We need to reconstruct the full RoutingProof and packet_id
    use phantom_crypto::primitives::hash;
    let packet_id = hash(&nullifier);
    
    use phantom_core::proof::{RoutingProof, PublicInputs};
    let path_proof = RoutingProof {
        proof_data,
        public_inputs: PublicInputs {
            network_commitment: [0; 32], // Will be verified by receiver
            path_length: 0, // Unknown from wire format
            timestamp: 0, // Unknown from wire format
        },
    };
    
    Ok(PhantomPacket {
        routing_blob,
        path_proof,
        payload,
        nullifier,
        packet_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use phantom_core::packet::RoutingPath;
    use phantom_crypto::FheEngine;
    
    #[test]
    fn test_wire_header_creation() {
        let fhe_engine = FheEngine::generate_keys();
        let path = RoutingPath::new(vec![100, 101, 102]).unwrap();
        let payload = b"Test message".to_vec();
        let network_commitment = [0u8; 32];
        
        let packet = PhantomPacket::construct(
            path,
            payload,
            &fhe_engine,
            &network_commitment,
        ).unwrap();
        
        let header = WireHeader::new(&packet);
        
        assert_eq!(header.version, PROTOCOL_VERSION);
        assert!(header.packet_length >= MIN_PACKET_SIZE as u32);
        assert!(header.routing_blob_length > 0);
        assert!(header.proof_length > 0);
    }
    
    #[test]
    fn test_wire_header_serialization() {
        // A header has to satisfy two independent checks, and the original
        // test data satisfied neither. packet_length must equal the sum of the
        // parts — 20 (header) + blob + proof + payload + 32 (nullifier) — and
        // the total must be at least MIN_PACKET_SIZE (1024), which exists so a
        // packet's length cannot itself leak how much traffic a node is
        // carrying. 512 + 384 + 128 satisfies both: 20 + 1024 + 32 = 1076.
        //
        // The original declared 2048 against parts totalling 948 and then
        // unwrapped, so it failed the moment the crate could compile.
        let header = WireHeader {
            version: 1,
            packet_length: 1076,
            routing_blob_length: 512,
            proof_length: 384,
            payload_length: 128,
            reserved: [0; 3],
        };
        
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), WireHeader::SIZE);
        
        let deserialized = WireHeader::from_bytes(&bytes).unwrap();
        assert_eq!(deserialized.version, header.version);
        assert_eq!(deserialized.packet_length, header.packet_length);
        assert_eq!(deserialized.routing_blob_length, header.routing_blob_length);
        assert_eq!(deserialized.proof_length, header.proof_length);
        assert_eq!(deserialized.payload_length, header.payload_length);
    }

    #[test]
    fn inconsistent_packet_length_is_rejected() {
        // The invariant the previous test was accidentally violating is worth
        // asserting deliberately. A header whose declared total disagrees with
        // its parts is how a parser gets walked past the end of a buffer, or
        // handed a truncated packet it believes is whole.
        let header = WireHeader {
            version: 1,
            packet_length: 2048, // components below total 948
            routing_blob_length: 512,
            proof_length: 256,
            payload_length: 128,
            reserved: [0; 3],
        };

        let err = WireHeader::from_bytes(&header.to_bytes())
            .expect_err("a header that misdeclares its own length must not parse");

        assert!(
            matches!(
                err,
                WireError::InvalidLength { expected: 948, actual: 2048 }
            ),
            "expected an InvalidLength naming both values, got {err:?}"
        );
    }
    
    #[test]
    #[ignore] // Slow test - FHE operations
    fn test_packet_serialization_roundtrip() {
        let fhe_engine = FheEngine::generate_keys();
        let path = RoutingPath::new(vec![100, 101, 102, 103]).unwrap();
        let payload = b"Secret test message".to_vec();
        let network_commitment = [0u8; 32];
        
        let original = PhantomPacket::construct(
            path,
            payload.clone(),
            &fhe_engine,
            &network_commitment,
        ).unwrap();
        
        // Serialize
        let wire_bytes = serialize_packet(&original).unwrap();
        assert!(wire_bytes.len() >= MIN_PACKET_SIZE);
        assert!(wire_bytes.len() <= MAX_PACKET_SIZE);
        
        // Deserialize
        let deserialized = deserialize_packet(&wire_bytes).unwrap();
        assert_eq!(deserialized.routing_blob, original.routing_blob);
        assert_eq!(deserialized.path_proof.proof_data, original.path_proof.proof_data);
        assert_eq!(deserialized.payload, original.payload);
        assert_eq!(deserialized.nullifier, original.nullifier);
    }
    
    #[test]
    fn test_invalid_version() {
        let bytes = vec![99u8; WireHeader::SIZE]; // Invalid version
        let result = WireHeader::from_bytes(&bytes);
        assert!(matches!(result, Err(WireError::UnsupportedVersion(_))));
    }
    
    #[test]
    fn test_packet_too_large() {
        let mut bytes = vec![0u8; WireHeader::SIZE];
        bytes[0] = PROTOCOL_VERSION;
        // Set packet_length to exceed MAX_PACKET_SIZE
        let oversized = (MAX_PACKET_SIZE + 1) as u32;
        bytes[1..5].copy_from_slice(&oversized.to_le_bytes());
        
        let result = WireHeader::from_bytes(&bytes);
        assert!(matches!(result, Err(WireError::PacketTooLarge(_))));
    }
}
