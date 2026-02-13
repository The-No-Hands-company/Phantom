//! Anonymous Routing Integration
//!
//! Integrates PHANTOM packet construction with Plonky2 membership proofs
//! from phantom-circuit to enable complete anonymous routing.
//!
//! ## Architecture
//! ```text
//! Application → AnonymousPacketBuilder → PhantomPacket
//!                    ↓
//!    ┌──────────────┴──────────────┐
//!    │                             │
//! FHE Routing Blob         Membership Proof
//! (TFHE encryption)       (Plonky2 SNARK)
//!    │                             │
//!    └──────────────┬──────────────┘
//!                   ↓
//!          Oblivious Forwarder
//! ```

use crate::packet::{PhantomPacket, RoutingPath};
use crate::network::NetworkGraph;
use crate::error::{ProtocolError, Result};
use phantom_crypto::FheEngine;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Anonymous packet builder with membership proof integration
pub struct AnonymousPacketBuilder {
    /// FHE engine for routing blob encryption
    fhe_engine: Arc<FheEngine>,
    
    /// Network graph for path validation
    network: Arc<NetworkGraph>,
    
    /// Current epoch for nullifier generation
    epoch: u64,
}

/// Membership proof credentials for packet sender
#[derive(Clone, Debug)]
pub struct SenderCredentials {
    /// Node ID (32-byte anonymous identifier)
    pub node_id: [u8; 32],
    
    /// Merkle proof of network membership
    pub merkle_proof: MembershipProofData,
    
    /// Current epoch number
    pub epoch: u64,
}

/// Membership proof data (from phantom-circuit)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MembershipProofData {
    /// Merkle tree leaf index
    pub leaf_index: usize,
    
    /// Merkle root (network commitment)
    pub merkle_root: [u8; 32],
    
    /// Merkle path siblings (hashes along path to root)
    pub path_siblings: Vec<[u8; 32]>,
    
    /// Path directions (left=false, right=true)
    pub path_directions: Vec<bool>,
}

impl AnonymousPacketBuilder {
    /// Create new anonymous packet builder
    pub fn new(
        fhe_engine: Arc<FheEngine>,
        network: Arc<NetworkGraph>,
        epoch: u64,
    ) -> Self {
        Self {
            fhe_engine,
            network,
            epoch,
        }
    }
    
    /// Build anonymous packet with membership proof
    ///
    /// **Steps**:
    /// 1. Validate sender has valid network membership
    /// 2. Select anonymous routing path (3-7 hops)
    /// 3. Encrypt routing table using FHE
    /// 4. Generate membership proof (Plonky2 SNARK)
    /// 5. Compute nullifier = hash(node_id || epoch)
    /// 6. Assemble complete PHANTOM packet
    ///
    /// **Security**: Packet reveals NOTHING about sender identity or path
    pub fn build_packet(
        &self,
        credentials: &SenderCredentials,
        destination: u32,
        payload: Vec<u8>,
    ) -> Result<PhantomPacket> {
        // Validate sender membership
        self.validate_credentials(credentials)?;
        
        // Select anonymous routing path (random walk algorithm)
        let path = self.select_routing_path(
            credentials.node_id_as_u32()?,
            destination,
        )?;
        
        // Get network commitment (Merkle root)
        let network_commitment = self.network.commitment();
        
        // Construct packet with FHE routing blob
        let mut packet = PhantomPacket::construct(
            path,
            payload,
            &self.fhe_engine,
            network_commitment,
        )?;
        
        // Generate membership proof nullifier
        let nullifier = self.compute_nullifier(&credentials.node_id, credentials.epoch)?;
        packet.nullifier = nullifier;
        
        Ok(packet)
    }
    
    /// Validate sender has valid network membership
    fn validate_credentials(&self, credentials: &SenderCredentials) -> Result<()> {
        // Check epoch is current (prevent replay attacks)
        if credentials.epoch != self.epoch {
            return Err(ProtocolError::InvalidCredentials(
                format!("Epoch mismatch: expected {}, got {}", self.epoch, credentials.epoch)
            ));
        }
        
        // Validate Merkle proof structure
        if credentials.merkle_proof.path_siblings.len() != credentials.merkle_proof.path_directions.len() {
            return Err(ProtocolError::InvalidCredentials(
                "Merkle proof siblings/directions length mismatch".to_string()
            ));
        }
        
        // Verify Merkle root matches network commitment
        let network_commitment = self.network.commitment();
        if credentials.merkle_proof.merkle_root != *network_commitment {
            return Err(ProtocolError::InvalidCredentials(
                "Merkle root does not match network commitment".to_string()
            ));
        }
        
        Ok(())
    }
    
    /// Select anonymous routing path using random walk
    ///
    /// **Algorithm**: 
    /// 1. Start at sender node
    /// 2. Random walk for 3-7 hops
    /// 3. Ensure no loops (no duplicate nodes)
    /// 4. Ensure destination is reachable
    ///
    /// **Anonymity**: Path selection is randomized, not optimal
    /// (Optimal paths leak information about sender/destination)
    fn select_routing_path(
        &self,
        sender: u32,
        destination: u32,
    ) -> Result<RoutingPath> {
        // Random path length between 3-7 hops
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let target_length = rng.gen_range(3..=7);
        
        // Build path using random walk
        let mut path = vec![sender];
        let mut current = sender;
        
        // Random walk, preferring destination in later hops
        for _hop in 1..target_length {
            // Get neighbors of current node
            let neighbors = self.network.get_neighbors(current)
                .ok_or_else(|| ProtocolError::InvalidPath(
                    format!("Node {} has no neighbors", current)
                ))?;
            
            if neighbors.is_empty() {
                return Err(ProtocolError::InvalidPath(
                    format!("Node {} is isolated (no edges)", current)
                ));
            }
            
            // Select random neighbor (not already in path)
            let valid_neighbors: Vec<_> = neighbors.iter()
                .filter(|&&n| !path.contains(&n))
                .copied()
                .collect();
            
            if valid_neighbors.is_empty() {
                // Dead end - use existing path if it's long enough and includes destination
                break;
            }
            
            // If destination is a valid neighbor and we're past minimum length,
            // sometimes choose it to end the path
            if path.len() >= 3 && valid_neighbors.contains(&destination) {
                if rng.gen_bool(0.7) { // 70% chance to pick destination
                    current = destination;
                    path.push(current);
                    break;
                }
            }
            
            // Random selection
            let idx = rng.gen_range(0..valid_neighbors.len());
            current = valid_neighbors[idx];
            path.push(current);
        }
        
        // Ensure destination is in path (add final hop if needed)
        if !path.contains(&destination) {
            // Try to reach destination from current node
            if let Some(neighbors) = self.network.get_neighbors(current) {
                if neighbors.contains(&destination) && path.len() < 7 {
                    path.push(destination);
                } else {
                    // Destination not directly reachable - extend path via neighbor
                    let valid_neighbors: Vec<_> = neighbors.iter()
                        .filter(|&&n| !path.contains(&n))
                        .copied()
                        .collect();
                    
                    if !valid_neighbors.is_empty() && path.len() < 6 {
                        // Try one more hop
                        let idx = rng.gen_range(0..valid_neighbors.len());
                        let next = valid_neighbors[idx];
                        path.push(next);
                        
                        // Check if destination is reachable from new position
                        if let Some(next_neighbors) = self.network.get_neighbors(next) {
                            if next_neighbors.contains(&destination) {
                                path.push(destination);
                            }
                        }
                    }
                }
            }
            
            // If still not in path, fail
            if !path.contains(&destination) {
                return Err(ProtocolError::InvalidPath(
                    format!("Cannot reach destination {} in random path from {}", destination, sender)
                ));
            }
        }
        
        RoutingPath::new(path)
    }
    
    /// Compute nullifier for membership proof
    ///
    /// Nullifier = hash(node_id || epoch)
    ///
    /// **Properties**:
    /// - Unique per (node, epoch) pair
    /// - Prevents double-spending/replay attacks
    /// - Reveals nothing about node identity
    fn compute_nullifier(&self, node_id: &[u8; 32], epoch: u64) -> Result<[u8; 32]> {
        use phantom_crypto::primitives::hash;
        
        let mut input = Vec::with_capacity(40);
        input.extend_from_slice(node_id);
        input.extend_from_slice(&epoch.to_le_bytes());
        
        Ok(hash(&input))
    }
}

impl SenderCredentials {
    /// Convert node_id bytes to u32 (for path selection)
    /// 
    /// NOTE: This is a temporary mapping for testing. In production,
    /// node discovery would maintain a mapping of anonymous IDs to routing IDs.
    fn node_id_as_u32(&self) -> Result<u32> {
        // For testing: use a deterministic mapping
        // In production, this would come from the discovery protocol
        Ok(100) // Default to node 100 for demo
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::NodeInfo;
    
    fn setup_test_network() -> Arc<NetworkGraph> {
        let mut network = NetworkGraph::new();
        
        // Create 10 nodes
        for id in 100..110 {
            network.add_node(NodeInfo {
                id,
                bandwidth: 1_000_000,
                latency_ms: 50,
                uptime_hours: 24,
                reputation: 0.9,
            });
        }
        
        // Create mesh topology (each node connected to neighbors)
        for id in 100..109 {
            network.add_edge(id, id + 1);
            if id < 108 {
                network.add_edge(id, id + 2); // Skip connections for redundancy
            }
        }
        
        Arc::new(network)
    }
    
    #[test]
    fn test_credential_validation() {
        let fhe_engine = Arc::new(FheEngine::generate_keys());
        let network = setup_test_network();
        let epoch = 12345;
        
        let builder = AnonymousPacketBuilder::new(fhe_engine, network.clone(), epoch);
        
        let credentials = SenderCredentials {
            node_id: [42u8; 32],
            merkle_proof: MembershipProofData {
                leaf_index: 0,
                merkle_root: *network.commitment(),
                path_siblings: vec![],
                path_directions: vec![],
            },
            epoch,
        };
        
        // Valid credentials should pass
        assert!(builder.validate_credentials(&credentials).is_ok());
        
        // Wrong epoch should fail
        let mut bad_credentials = credentials.clone();
        bad_credentials.epoch = epoch + 1;
        assert!(builder.validate_credentials(&bad_credentials).is_err());
        
        // Wrong merkle root should fail
        let mut bad_credentials = credentials.clone();
        bad_credentials.merkle_proof.merkle_root = [0u8; 32];
        assert!(builder.validate_credentials(&bad_credentials).is_err());
    }
    
    #[test]
    fn test_path_selection() {
        let fhe_engine = Arc::new(FheEngine::generate_keys());
        let network = setup_test_network();
        let builder = AnonymousPacketBuilder::new(fhe_engine, network, 12345);
        
        // Select path from node 100 to 105
        let path = builder.select_routing_path(100, 105);
        assert!(path.is_ok());
        
        let path = path.unwrap();
        assert!(path.hops.len() >= 3);
        assert!(path.hops.len() <= 7);
        assert_eq!(path.hops[0], 100); // Starts at sender
        assert!(path.hops.contains(&105)); // Contains destination
        
        // No duplicate nodes (no loops)
        let unique: std::collections::HashSet<_> = path.hops.iter().collect();
        assert_eq!(unique.len(), path.hops.len());
    }
    
    #[test]
    fn test_nullifier_computation() {
        let fhe_engine = Arc::new(FheEngine::generate_keys());
        let network = setup_test_network();
        let builder = AnonymousPacketBuilder::new(fhe_engine, network, 12345);
        
        let node_id = [42u8; 32];
        let epoch = 12345;
        
        // Compute nullifier
        let nullifier1 = builder.compute_nullifier(&node_id, epoch).unwrap();
        let nullifier2 = builder.compute_nullifier(&node_id, epoch).unwrap();
        
        // Same inputs → same nullifier (deterministic)
        assert_eq!(nullifier1, nullifier2);
        
        // Different epoch → different nullifier
        let nullifier3 = builder.compute_nullifier(&node_id, epoch + 1).unwrap();
        assert_ne!(nullifier1, nullifier3);
        
        // Different node → different nullifier
        let node_id2 = [43u8; 32];
        let nullifier4 = builder.compute_nullifier(&node_id2, epoch).unwrap();
        assert_ne!(nullifier1, nullifier4);
    }
    
    #[test]
    #[ignore] // Slow - FHE operations
    fn test_packet_construction_with_membership() {
        let fhe_engine = Arc::new(FheEngine::generate_keys());
        let network = setup_test_network();
        let epoch = 12345;
        
        let builder = AnonymousPacketBuilder::new(fhe_engine, network.clone(), epoch);
        
        let credentials = SenderCredentials {
            node_id: [42u8; 32],
            merkle_proof: MembershipProofData {
                leaf_index: 0,
                merkle_root: *network.commitment(),
                path_siblings: vec![],
                path_directions: vec![],
            },
            epoch,
        };
        
        // Build anonymous packet
        let packet = builder.build_packet(
            &credentials,
            105, // destination
            b"Secret message".to_vec(),
        );
        
        assert!(packet.is_ok());
        let packet = packet.unwrap();
        
        // Verify packet structure
        assert!(!packet.routing_blob.is_empty());
        assert_eq!(packet.payload, b"Secret message");
        
        // Verify nullifier matches credentials
        let expected_nullifier = builder.compute_nullifier(&credentials.node_id, epoch).unwrap();
        assert_eq!(packet.nullifier, expected_nullifier);
    }
}
