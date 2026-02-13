/// Anonymous Node Announcement Protocol
/// 
/// Nodes announce their presence without revealing identity through:
/// - zk-Membership proof (I'm in the network Merkle tree)
/// - Nullifier (prevents double-announcements)
/// - Signed node descriptor (proves control of routing key)

use serde::{Deserialize, Serialize};
use blake3::Hash;
use phantom_core::network::NodeId;
use phantom_crypto::pq::{KeyPair, PublicKey, Signature};
use std::net::SocketAddr;

/// Anonymous node announcement message
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeAnnouncement {
    /// Node descriptor (public routing information)
    pub descriptor: NodeDescriptor,
    
    /// zk-SNARK proof of Merkle tree membership
    /// Proves: "I'm node at leaf index i in the network tree"
    /// Without revealing: which index i
    pub membership_proof: Vec<u8>,
    
    /// Nullifier to prevent double-announcements
    /// nullifier = H(node_id || epoch || network_commitment)
    pub nullifier: [u8; 32],
    
    /// Signature over (descriptor || membership_proof || nullifier)
    /// Proves control of routing key without revealing node_id
    pub signature: Vec<u8>,
    
    /// Timestamp (for freshness, not binding)
    pub timestamp: u64,
}

/// Public node descriptor - routing information
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeDescriptor {
    /// Public routing key (for PHANTOM packet forwarding)
    /// NOT the same as node_id (which stays secret)
    pub routing_key: Vec<u8>,
    
    /// Network addresses for incoming connections
    pub addresses: Vec<SocketAddr>,
    
    /// Supported protocol version
    pub protocol_version: u32,
    
    /// Node capabilities flags
    pub capabilities: NodeCapabilities,
    
    /// Relay bandwidth capacity (bytes/sec, self-reported)
    pub bandwidth_capacity: u64,
    
    /// Geographic region hint (for latency optimization, optional)
    pub region_hint: Option<String>,
}

/// Node capability flags
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeCapabilities {
    /// Can forward PHANTOM packets
    pub routing: bool,
    
    /// Can act as directory/bootstrap node
    pub directory: bool,
    
    /// Supports FHE-based oblivious routing
    pub fhe_routing: bool,
    
    /// Supports zkVM proof verification
    pub zkvm_verification: bool,
}

impl NodeAnnouncement {
    /// Create new node announcement
    pub fn new(
        descriptor: NodeDescriptor,
        membership_proof: Vec<u8>,
        node_id: &NodeId,
        epoch: u64,
        network_commitment: &[u8; 32],
        signing_key: &KeyPair,
    ) -> Result<Self, AnnouncementError> {
        // Compute nullifier: H(node_id || epoch || network_commitment)
        let nullifier = Self::compute_nullifier(node_id, epoch, network_commitment);
        
        // Serialize announcement data for signing
        let announcement_data = bincode::serialize(&(
            &descriptor,
            &membership_proof,
            &nullifier,
        ))?;
        
        // Sign with Dilithium-5 (post-quantum signature)
        let signature_bytes = signing_key.sign(&announcement_data);
        let signature = bincode::serialize(&signature_bytes)?;
        
        Ok(Self {
            descriptor,
            membership_proof,
            nullifier,
            signature,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| AnnouncementError::InvalidTimestamp)?
                .as_secs(),
        })
    }
    
    /// Verify announcement signature and membership proof
    pub fn verify(
        &self,
        verification_key: &PublicKey,
        network_commitment: &[u8; 32],
    ) -> Result<(), AnnouncementError> {
        // Verify signature
        let announcement_data = bincode::serialize(&(
            &self.descriptor,
            &self.membership_proof,
            &self.nullifier,
        ))?;
        
        let signature: Signature = bincode::deserialize(&self.signature)
            .map_err(|_| AnnouncementError::InvalidSignature)?;
        
        if !verification_key.verify(&announcement_data, &signature) {
            return Err(AnnouncementError::InvalidSignature);
        }
        
        // TODO: Verify membership proof using Plonky2
        // This requires the Merkle tree root (network_commitment)
        // For now, accept all proofs (will integrate with phantom-circuit)
        
        Ok(())
    }
    
    /// Compute nullifier for rate limiting
    pub fn compute_nullifier(
        node_id: &NodeId,
        epoch: u64,
        network_commitment: &[u8; 32],
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&node_id.0);
        hasher.update(&epoch.to_le_bytes());
        hasher.update(network_commitment);
        *hasher.finalize().as_bytes()
    }
    
    /// Get unique identifier for this announcement
    pub fn announcement_id(&self) -> Hash {
        blake3::hash(&bincode::serialize(self).unwrap())
    }
    
    /// Check if announcement is fresh (within 5 minutes)
    pub fn is_fresh(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // Announcement valid for 5 minutes
        now.saturating_sub(self.timestamp) < 300
    }
}

impl NodeDescriptor {
    /// Create new node descriptor
    pub fn new(
        routing_key: Vec<u8>,
        addresses: Vec<SocketAddr>,
        protocol_version: u32,
        capabilities: NodeCapabilities,
        bandwidth_capacity: u64,
        region_hint: Option<String>,
    ) -> Self {
        Self {
            routing_key,
            addresses,
            protocol_version,
            capabilities,
            bandwidth_capacity,
            region_hint,
        }
    }
    
    /// Get descriptor hash (for duplicate detection)
    pub fn descriptor_hash(&self) -> Hash {
        blake3::hash(&bincode::serialize(self).unwrap())
    }
}

impl Default for NodeCapabilities {
    fn default() -> Self {
        Self {
            routing: true,
            directory: false,
            fhe_routing: true,
            zkvm_verification: true,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AnnouncementError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Invalid signature")]
    InvalidSignature,
    
    #[error("Invalid membership proof")]
    InvalidMembershipProof,
    
    #[error("Invalid timestamp")]
    InvalidTimestamp,
    
    #[error("Duplicate nullifier (already announced)")]
    DuplicateNullifier,
    
    #[error("Announcement expired")]
    Expired,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_nullifier_computation() {
        let node_id = NodeId([42u8; 32]);
        let epoch = 12345u64;
        let network_commitment = [0xAAu8; 32];
        
        let nullifier1 = NodeAnnouncement::compute_nullifier(&node_id, epoch, &network_commitment);
        let nullifier2 = NodeAnnouncement::compute_nullifier(&node_id, epoch, &network_commitment);
        
        // Deterministic
        assert_eq!(nullifier1, nullifier2);
        
        // Different epoch = different nullifier
        let nullifier3 = NodeAnnouncement::compute_nullifier(&node_id, epoch + 1, &network_commitment);
        assert_ne!(nullifier1, nullifier3);
    }
    
    #[test]
    fn test_node_descriptor_creation() {
        let descriptor = NodeDescriptor::new(
            vec![1, 2, 3, 4],
            vec!["127.0.0.1:8080".parse().unwrap()],
            1,
            NodeCapabilities::default(),
            1_000_000,
            Some("us-west".to_string()),
        );
        
        assert_eq!(descriptor.routing_key, vec![1, 2, 3, 4]);
        assert_eq!(descriptor.addresses.len(), 1);
        assert!(descriptor.capabilities.routing);
    }
    
    #[test]
    fn test_announcement_signature() {
        let keypair = KeyPair::generate();
        
        let descriptor = NodeDescriptor::new(
            vec![1, 2, 3, 4],
            vec!["127.0.0.1:8080".parse().unwrap()],
            1,
            NodeCapabilities::default(),
            1_000_000,
            None,
        );
        
        let node_id = NodeId([42u8; 32]);
        let epoch = 1;
        let network_commitment = [0xBBu8; 32];
        let membership_proof = vec![0u8; 100]; // Mock proof
        
        let announcement = NodeAnnouncement::new(
            descriptor,
            membership_proof,
            &node_id,
            epoch,
            &network_commitment,
            &keypair,
        ).unwrap();
        
        // Verify signature
        assert!(announcement.verify(&keypair.public, &network_commitment).is_ok());
        
        // Wrong key fails
        let wrong_keypair = KeyPair::generate();
        assert!(announcement.verify(&wrong_keypair.public, &network_commitment).is_err());
    }
    
    #[test]
    fn test_announcement_freshness() {
        let keypair = KeyPair::generate();
        
        let descriptor = NodeDescriptor::new(
            vec![1, 2, 3, 4],
            vec!["127.0.0.1:8080".parse().unwrap()],
            1,
            NodeCapabilities::default(),
            1_000_000,
            None,
        );
        
        let mut announcement = NodeAnnouncement::new(
            descriptor,
            vec![0u8; 100],
            &NodeId([42u8; 32]),
            1,
            &[0xBBu8; 32],
            &keypair,
        ).unwrap();
        
        // Fresh announcement
        assert!(announcement.is_fresh());
        
        // Expired announcement (6 minutes old)
        announcement.timestamp -= 360;
        assert!(!announcement.is_fresh());
    }
}
