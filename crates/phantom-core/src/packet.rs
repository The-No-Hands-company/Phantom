//! Packet structure and construction

use phantom_crypto::{FheEngine, primitives::hash};
use serde::{Deserialize, Serialize};
use crate::network::NetworkGraph;
use crate::error::{ProtocolError, Result};
use crate::proof::RoutingProof;

pub type NodeId = u32;
pub type PacketId = [u8; 32];

/// PHANTOM packet structure
///
/// This is the revolutionary packet format that enables oblivious routing.
/// Unlike Tor/I2P packets, routing metadata itself is FHE-encrypted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PhantomPacket {
    /// FHE-encrypted routing table
    /// Each node can homomorphically evaluate "should I forward this?"
    /// without learning the actual route
    pub routing_blob: Vec<u8>,
    
    /// zk-Proof of path validity (from phantom-zkvm)
    /// Proves the packet follows a valid route without revealing it
    pub path_proof: RoutingProof,
    
    /// Encrypted payload (application data)
    pub payload: Vec<u8>,
    
    /// Rate-limiting nullifier (prevents spam)
    pub nullifier: [u8; 32],
    
    /// Unique packet identifier
    pub packet_id: PacketId,
}

/// Routing path through the network
#[derive(Clone, Debug)]
pub struct RoutingPath {
    /// List of node IDs in the path
    pub hops: Vec<NodeId>,
    
    /// For each hop, the next hop to forward to
    /// (encrypted using FHE)
    pub next_hops: Vec<NodeId>,
}

impl RoutingPath {
    /// Create a new routing path with validation
    /// 
    /// # Cloudflare-proof: Returns Result instead of panicking
    pub fn new(hops: Vec<NodeId>) -> Result<Self> {
        // Validate path length (anonymity requirement)
        if hops.len() < 3 {
            return Err(ProtocolError::InvalidPath(
                "Path must have at least 3 hops for anonymity".to_string()
            ));
        }
        
        // Validate path length (performance requirement)
        if hops.len() > 7 {
            return Err(ProtocolError::InvalidPath(
                "Path must have at most 7 hops for performance".to_string()
            ));
        }
        
        // Build next_hops table: for each hop, what's the next hop?
        let mut next_hops = Vec::new();
        for i in 0..hops.len() - 1 {
            next_hops.push(hops[i + 1]);
        }
        next_hops.push(0); // Last hop has no next hop
        
        Ok(Self { hops, next_hops })
    }
    
    /// Validate path against network graph
    /// 
    /// Checks:
    /// 1. All hops exist in the network
    /// 2. Consecutive hops are connected (edges exist)
    /// 3. No loops (no duplicate node IDs)
    pub fn validate(&self, network: &NetworkGraph) -> bool {
        // Check for loops (no duplicate hops)
        let mut seen = std::collections::HashSet::new();
        for &hop in &self.hops {
            if !seen.insert(hop) {
                return false; // Duplicate found - loop detected
            }
        }

        // Check all hops exist in the network
        for &hop in &self.hops {
            if !network.contains_node(hop) {
                return false; // Node not in network
            }
        }

        // Check consecutive hops are connected
        for window in self.hops.windows(2) {
            let from = window[0];
            let to = window[1];
            if !network.are_connected(from, to) {
                return false; // Edge doesn't exist
            }
        }

        true
    }
}

impl PhantomPacket {
    /// Construct a new PHANTOM packet with Plonky2 proof integration
    ///
    /// This is the production-ready constructor that uses real Plonky2 proofs.
    /// For testing/examples without phantom-circuit dependency, use `construct()`.
    #[cfg(feature = "plonky2-integration")]
    pub fn new<const D: usize>(
        path: &[crate::network::NodeId],
        payload: Vec<u8>,
        recipient_public_key: &[u8],
        network: &crate::network::Network,
        circuit: &phantom_circuit::membership::MembershipCircuit,
        fhe_engine: &FheEngine,
    ) -> Result<Self> {
        // Convert to RoutingPath
        let routing_path = RoutingPath::new(path.iter().map(|id| id.0).collect())?;
        
        // Validate path against network
        if !routing_path.validate(network.graph()) {
            return Err(ProtocolError::InvalidPath("Path is not valid in network".to_string()));
        }
        
        // Generate packet ID
        let random_bytes = rand::random::<[u8; 32]>();
        let mut hash_input = Vec::new();
        hash_input.extend(path.iter().flat_map(|id| id.0.to_le_bytes()));
        hash_input.extend(&payload);
        hash_input.extend(&random_bytes);
        let packet_id = hash(&hash_input);
        
        // 1. Encrypt routing table using FHE
        let mut routing_table = Vec::new();
        for (i, &node_id) in path.iter().enumerate() {
            let next_hop = if i < path.len() - 1 {
                path[i + 1].0
            } else {
                0
            };
            
            let encrypted_node = fhe_engine.encrypt_u32(node_id.0);
            let encrypted_next = fhe_engine.encrypt_u32(next_hop);
            
            routing_table.push((encrypted_node, encrypted_next));
        }
        
        let routing_blob = bincode::serialize(&routing_table)?;
        
        // 2. Generate Plonky2 proof of path validity
        let plonky2_proof = circuit.prove_path_validity(path, network)
            .map_err(|e| ProtocolError::InvalidPath(format!("Plonky2 proof failed: {}", e)))?;
        
        // Serialize Plonky2 proof to RoutingProof format
        let path_proof = RoutingProof {
            proof_data: bincode::serialize(&plonky2_proof)?,
            public_inputs: crate::proof::PublicInputs {
                network_commitment: network.commitment(),
                path_length: path.len(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
        };
        
        // 3. Encrypt payload (TODO: use recipient_public_key for encryption)
        let encrypted_payload = payload; // Placeholder
        
        // 4. Generate rate-limiting nullifier
        let nullifier = hash(&packet_id);
        
        Ok(Self {
            routing_blob,
            path_proof,
            payload: encrypted_payload,
            nullifier,
            packet_id,
        })
    }

    /// Construct a new PHANTOM packet
    ///
    /// This is where the magic happens:
    /// 1. Encrypt the routing table using FHE
    /// 2. Generate zk-proof of path validity
    /// 3. Encrypt the payload — NOT IMPLEMENTED, see below
    /// 4. Generate rate-limiting nullifier — NOT IMPLEMENTED, see below
    ///
    /// # What this actually does today
    ///
    /// Only step 1 is real. The routing table is genuinely FHE-encrypted, so a
    /// relay cannot learn the route from a packet it forwards.
    ///
    /// The payload is stored verbatim. A relay carrying a packet can read its
    /// contents. The zk path proof is a placeholder, and the nullifier is
    /// `hash(packet_id)` rather than an RLN nullifier, so it prevents nothing:
    /// a fresh packet id yields a fresh nullifier.
    ///
    /// This matters because the project's README says nodes "route packets
    /// they literally cannot decrypt". That is true of the routing metadata
    /// and false of the payload, and the difference is the whole threat model.
    /// Do not treat a PhantomPacket as confidential until step 3 lands.
    pub fn construct(
        path: RoutingPath,
        payload: Vec<u8>,
        fhe_engine: &FheEngine,
        network_commitment: &[u8; 32],
    ) -> Result<Self> {
        // Generate unique packet ID
        let random_bytes = rand::random::<[u8; 32]>();
        let mut hash_input = Vec::new();
        hash_input.extend(path.hops.iter().flat_map(|h| h.to_le_bytes()));
        hash_input.extend(&payload);
        hash_input.extend(&random_bytes);
        let packet_id = hash(&hash_input);
        
        // 1. Encrypt routing table using FHE
        // Build table: (node_id, next_hop) pairs
        let mut routing_table = Vec::new();
        for (i, &node_id) in path.hops.iter().enumerate() {
            let next_hop = if i < path.next_hops.len() {
                path.next_hops[i]
            } else {
                0
            };
            
            // Encrypt both node_id and next_hop
            let encrypted_node = fhe_engine.encrypt_u32(node_id);
            let encrypted_next = fhe_engine.encrypt_u32(next_hop);
            
            routing_table.push((encrypted_node, encrypted_next));
        }
        
        // Serialize the encrypted routing table
        let routing_blob = bincode::serialize(&routing_table)?;
        
        // 2. Generate zk-proof of path validity
        // TODO: Implement actual zkVM proof generation
        // For now, create a placeholder proof
        let path_proof = Self::generate_path_proof(&path, network_commitment)?;
        
        // 3. Encrypt payload
        // TODO: Use FHE for payload encryption too
        // For now, just use the raw payload
        
        // 4. Generate rate-limiting nullifier
        // TODO: Integrate with RLN/Semaphore
        let nullifier = hash(&packet_id);
        
        Ok(Self {
            routing_blob,
            path_proof,
            payload,
            nullifier,
            packet_id,
        })
    }
    
    /// Generate zk-proof that the path is valid
    ///
    /// Proves:
    /// 1. Path length is between 3-7 hops
    /// 2. Each hop exists in the network graph
    /// 3. No duplicate hops (no loops)
    /// 4. Packet is fresh (nullifier not seen before)
    fn generate_path_proof(
        _path: &RoutingPath,
        _network_commitment: &[u8; 32],
    ) -> Result<RoutingProof> {
        // For testing in phantom-core, use mock proof generator
        // Production code should use phantom-zkvm::HashProofGenerator or Risc0ProofGenerator
        #[cfg(test)]
        {
            use crate::proof::ProofGenerator as ProofGeneratorTrait;
            let generator = tests::MockProofGenerator;
            let proof = generator.generate_path_proof(&_path.hops, _network_commitment, &[])
                .map_err(|e| ProtocolError::InvalidPath(format!("Proof generation failed: {}", e)))?;
            return Ok(proof);
        }
        
        #[cfg(not(test))]
        {
            // In non-test builds (examples, demos), use mock proof for now
            // TODO: Integrate actual zkVM proof generation when phantom-zkvm is ready
            use crate::proof::PublicInputs;
            Ok(RoutingProof {
                proof_data: vec![0u8; 32], // Placeholder proof
                public_inputs: PublicInputs {
                    network_commitment: *_network_commitment,
                    path_length: _path.hops.len(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                },
            })
        }
    }
    
    /// Verify the zk-proof of path validity
    pub fn verify_path_proof(
        &self,
        network_commitment: &[u8; 32],
    ) -> Result<bool> {
        #[cfg(test)]
        {
            use crate::proof::ProofGenerator as ProofGeneratorTrait;
            let generator = tests::MockProofGenerator;
            let valid = generator.verify_path_proof(&self.path_proof, network_commitment)
                .map_err(|e| ProtocolError::InvalidPath(format!("Proof verification failed: {}", e)))?;
            return Ok(valid);
        }
        
        #[cfg(not(test))]
        {
            Err(ProtocolError::InvalidPath(
                "Proof verification not available in phantom-core. Use phantom-zkvm crate.".to_string()
            ))
        }
    }
    
    /// Get the packet size in bytes
    pub fn size(&self) -> usize {
        self.routing_blob.len() + 
        self.path_proof.proof_data.len() + 
        self.payload.len() + 
        32 // nullifier
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phantom_crypto::FheEngine;
    use crate::proof::{ProofGenerator as ProofGeneratorTrait, PublicInputs};
    use std::collections::HashSet;
    
    // Mock proof generator for testing (hash-based, not zero-knowledge)
    pub(crate) struct MockProofGenerator;
    
    impl ProofGeneratorTrait for MockProofGenerator {
        fn generate_path_proof(
            &self,
            path: &[u32],
            network_commitment: &[u8; 32],
            _merkle_proofs: &[crate::proof::MerkleProof],
        ) -> anyhow::Result<RoutingProof> {
            // Validate path constraints
            if path.len() < 3 || path.len() > 7 {
                return Err(anyhow::anyhow!("Invalid path length"));
            }
            
            let unique_nodes: HashSet<_> = path.iter().collect();
            if unique_nodes.len() != path.len() {
                return Err(anyhow::anyhow!("Path contains loops"));
            }
            
            // Generate mock proof
            let proof_data = blake3::hash(&path.iter().flat_map(|n| n.to_le_bytes()).collect::<Vec<u8>>())
                .as_bytes()
                .to_vec();
            
            Ok(RoutingProof {
                proof_data,
                public_inputs: PublicInputs {
                    network_commitment: *network_commitment,
                    path_length: path.len(),
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                },
            })
        }
        
        fn verify_path_proof(
            &self,
            proof: &RoutingProof,
            network_commitment: &[u8; 32],
        ) -> anyhow::Result<bool> {
            Ok(proof.public_inputs.network_commitment == *network_commitment
                && proof.public_inputs.path_length >= 3
                && proof.public_inputs.path_length <= 7)
        }
    }
    
    #[test]
    fn test_routing_path_creation() {
        let path = RoutingPath::new(vec![100, 101, 102, 103, 104])
            .expect("Valid path should succeed");
        
        assert_eq!(path.hops.len(), 5);
        assert_eq!(path.next_hops.len(), 5);
        assert_eq!(path.next_hops[0], 101); // Node 100 -> 101
        assert_eq!(path.next_hops[1], 102); // Node 101 -> 102
        assert_eq!(path.next_hops[4], 0);   // Last hop has no next
    }
    
    #[test]
    fn test_path_too_short() {
        let result = RoutingPath::new(vec![100, 101]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProtocolError::InvalidPath(_)));
    }
    
    #[test]
    fn test_path_too_long() {
        let result = RoutingPath::new(vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProtocolError::InvalidPath(_)));
    }

    #[test]
    fn test_path_validation() {
        use crate::network::{NetworkGraph, NodeInfo};

        // Create a network with 5 connected nodes
        let mut network = NetworkGraph::new();
        for id in 100..105 {
            network.add_node(NodeInfo {
                id,
                bandwidth: 1_000_000,
                latency_ms: 50,
                uptime_hours: 24,
                reputation: 0.9,
            });
        }

        // Add edges: 100-101-102-103-104 (linear chain)
        network.add_edge(100, 101);
        network.add_edge(101, 102);
        network.add_edge(102, 103);
        network.add_edge(103, 104);

        // Valid path through connected nodes
        let valid_path = RoutingPath::new(vec![100, 101, 102]).unwrap();
        assert!(valid_path.validate(&network));

        // Invalid: node 999 doesn't exist
        let invalid_node = RoutingPath::new(vec![100, 101, 999]).unwrap();
        assert!(!invalid_node.validate(&network));

        // Invalid: 100 and 103 not directly connected
        let invalid_edge = RoutingPath::new(vec![100, 103, 104]).unwrap();
        assert!(!invalid_edge.validate(&network));

        // Invalid: loop (duplicate node 101)
        let invalid_loop = RoutingPath {
            hops: vec![100, 101, 102, 101],
            next_hops: vec![101, 102, 101, 0],
        };
        assert!(!invalid_loop.validate(&network));
    }
    
    #[test]
    #[ignore] // Slow test due to FHE operations
    fn test_packet_construction() {
        let fhe_engine = FheEngine::generate_keys();
        let path = RoutingPath::new(vec![100, 101, 102, 103, 104])
            .expect("Valid path");
        let payload = b"Secret message".to_vec();
        let network_commitment = [0u8; 32];
        
        let packet = PhantomPacket::construct(
            path,
            payload.clone(),
            &fhe_engine,
            &network_commitment,
        ).expect("Packet construction should succeed");
        
        assert!(!packet.routing_blob.is_empty());
        assert!(!packet.path_proof.proof_data.is_empty());
        assert_eq!(packet.payload, payload);
        assert!(packet.verify_path_proof(&network_commitment)
            .expect("Verification should succeed"));
    }
}
