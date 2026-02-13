//! Bootstrap Protocol
//!
//! Initial network join sequence for PHANTOM nodes.
//!
//! ## Bootstrap Process
//!
//! 1. Connect to hardcoded bootstrap nodes
//! 2. Query discovery service for current network state
//! 3. Download network Merkle tree
//! 4. Verify Merkle root matches network consensus
//! 5. Generate own membership proof
//! 6. Create and broadcast first announcement
//!
//! ## Security Properties
//!
//! - Bootstrap nodes cannot censor (multiple fallbacks)
//! - Merkle tree verification ensures integrity
//! - Anonymous announcement (no node_id disclosure)

use crate::discovery::{DiscoveryService, DiscoveryQuery};
use crate::announcement::{NodeAnnouncement, NodeDescriptor, NodeCapabilities};
use crate::state::NetworkState;
use phantom_core::network::{NodeId, Network};
use phantom_crypto::pq::KeyPair;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Bootstrap client for joining the PHANTOM network
pub struct BootstrapClient {
    /// Our node's identity
    node_id: NodeId,
    
    /// Signing keypair
    keypair: KeyPair,
    
    /// Our public descriptor
    descriptor: NodeDescriptor,
    
    /// Bootstrap configuration
    config: BootstrapConfig,
}

/// Bootstrap configuration
#[derive(Clone, Debug)]
pub struct BootstrapConfig {
    /// Hardcoded bootstrap node addresses
    pub bootstrap_nodes: Vec<SocketAddr>,
    
    /// Number of bootstrap nodes to query
    pub query_count: usize,
    
    /// Timeout for bootstrap queries (seconds)
    pub timeout_secs: u64,
    
    /// Retry attempts for failed bootstraps
    pub max_retries: usize,
}

impl Default for BootstrapConfig {
    fn default() -> Self {
        Self {
            bootstrap_nodes: vec![
                "bootstrap1.phantom.network:8080".parse().unwrap(),
                "bootstrap2.phantom.network:8080".parse().unwrap(),
                "bootstrap3.phantom.network:8080".parse().unwrap(),
            ],
            query_count: 3,
            timeout_secs: 10,
            max_retries: 3,
        }
    }
}

/// Bootstrap result
#[derive(Clone, Debug)]
pub struct BootstrapResult {
    /// Network state obtained from bootstrap
    pub network_state: NetworkState,
    
    /// Network Merkle tree
    pub network: Network,
    
    /// Our generated announcement
    pub announcement: NodeAnnouncement,
    
    /// Bootstrap nodes contacted
    pub bootstrap_nodes: Vec<SocketAddr>,
}

impl BootstrapClient {
    /// Create new bootstrap client
    pub fn new(
        node_id: NodeId,
        keypair: KeyPair,
        descriptor: NodeDescriptor,
        config: BootstrapConfig,
    ) -> Self {
        Self {
            node_id,
            keypair,
            descriptor,
            config,
        }
    }
    
    /// Execute bootstrap sequence
    ///
    /// This is a blocking operation that may take several seconds.
    pub fn bootstrap(&self) -> Result<BootstrapResult, BootstrapError> {
        println!("Starting bootstrap sequence...");
        
        // Step 1: Query bootstrap nodes for network state
        println!("  [1/5] Querying bootstrap nodes...");
        let (network_state, bootstrap_nodes) = self.query_bootstrap_nodes()?;
        
        println!("      ✓ Got network state: epoch={}, nodes={}",
            network_state.epoch, network_state.node_count);
        
        // Step 2: Download network Merkle tree
        println!("  [2/5] Downloading network Merkle tree...");
        let network = self.download_merkle_tree(&bootstrap_nodes, &network_state)?;
        
        println!("      ✓ Downloaded tree with {} nodes", network.nodes.len());
        
        // Step 3: Verify Merkle root
        println!("  [3/5] Verifying Merkle root...");
        self.verify_merkle_root(&network, &network_state)?;
        
        println!("      ✓ Merkle root verified");
        
        // Step 4: Generate membership proof
        println!("  [4/5] Generating membership proof...");
        let membership_proof = self.generate_membership_proof(&network)?;
        
        println!("      ✓ Generated proof ({} bytes)", membership_proof.len());
        
        // Step 5: Create announcement
        println!("  [5/5] Creating announcement...");
        let announcement = NodeAnnouncement::new(
            self.descriptor.clone(),
            membership_proof,
            &self.node_id,
            network_state.epoch,
            &network_state.merkle_root,
            &self.keypair,
        )?;
        
        println!("      ✓ Announcement created");
        
        println!("Bootstrap complete!");
        
        Ok(BootstrapResult {
            network_state,
            network,
            announcement,
            bootstrap_nodes,
        })
    }
    
    /// Query bootstrap nodes for network state
    fn query_bootstrap_nodes(&self) -> Result<(NetworkState, Vec<SocketAddr>), BootstrapError> {
        use rand::seq::SliceRandom;
        use rand::thread_rng;
        
        let mut rng = thread_rng();
        let mut nodes = self.config.bootstrap_nodes.clone();
        nodes.shuffle(&mut rng);
        
        let query_count = self.config.query_count.min(nodes.len());
        let to_query = &nodes[..query_count];
        
        // In production, this would make actual network requests
        // For now, simulate successful bootstrap
        
        for node_addr in to_query {
            // TODO: Actual HTTP/gRPC request to bootstrap node
            // let response = http_client.get(format!("http://{}/network_state", node_addr))?;
            
            println!("      Querying {}...", node_addr);
        }
        
        // Mock network state (in production, parse from bootstrap response)
        let network_state = NetworkState {
            merkle_root: [0xAAu8; 32],
            epoch: NetworkState::current_epoch(),
            node_count: 100,
            last_update: current_timestamp(),
        };
        
        Ok((network_state, to_query.to_vec()))
    }
    
    /// Download network Merkle tree from bootstrap nodes
    fn download_merkle_tree(
        &self,
        bootstrap_nodes: &[SocketAddr],
        network_state: &NetworkState,
    ) -> Result<Network, BootstrapError> {
        // In production, download actual Merkle tree from bootstrap nodes
        // For now, create mock network
        
        let mut network = Network::new();
        
        // Add ourselves to the network
        network.add_node(self.node_id);
        
        // In production, would download all nodes and reconstruct tree
        // For simplicity, just add mock nodes to match node_count
        for i in 0..(network_state.node_count - 1) {
            // NodeId is u32, not a tuple struct
            let node_id = (i as u32) + 100;  // Offset to avoid ID collision
            network.add_node(node_id);
        }
        
        Ok(network)
    }
    
    /// Verify that downloaded Merkle tree matches network commitment
    fn verify_merkle_root(
        &self,
        network: &Network,
        network_state: &NetworkState,
    ) -> Result<(), BootstrapError> {
        let computed_root = network.merkle_root();
        
        if computed_root.as_bytes() == &network_state.merkle_root {
            Ok(())
        } else {
            Err(BootstrapError::MerkleRootMismatch {
                expected: network_state.merkle_root,
                computed: *computed_root.as_bytes(),
            })
        }
    }
    
    /// Generate zk-SNARK membership proof
    fn generate_membership_proof(
        &self,
        network: &Network,
    ) -> Result<Vec<u8>, BootstrapError> {
        // Find our position in the tree
        let leaf_index = network.nodes.iter()
            .position(|n| n == &self.node_id)
            .ok_or(BootstrapError::NodeNotInTree)?;
        
        // Generate Merkle path
        let merkle_path = network.merkle_path(leaf_index);
        
        // In production, use Plonky2 to generate zk-SNARK proof
        // For now, return mock proof (serialized Merkle path)
        let proof = bincode::serialize(&merkle_path)
            .map_err(|_| BootstrapError::ProofGenerationFailed)?;
        
        Ok(proof)
    }
}

/// Get current Unix timestamp
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("System time before Unix epoch")
        .as_secs()
}

#[derive(Debug, thiserror::Error)]
pub enum BootstrapError {
    #[error("No bootstrap nodes available")]
    NoBootstrapNodes,
    
    #[error("All bootstrap nodes failed")]
    AllNodesFailed,
    
    #[error("Network request timeout")]
    Timeout,
    
    #[error("Merkle root mismatch: expected {expected:?}, computed {computed:?}")]
    MerkleRootMismatch {
        expected: [u8; 32],
        computed: [u8; 32],
    },
    
    #[error("Node not found in tree")]
    NodeNotInTree,
    
    #[error("Proof generation failed")]
    ProofGenerationFailed,
    
    #[error("Announcement creation failed: {0}")]
    AnnouncementError(#[from] crate::announcement::AnnouncementError),
    
    #[error("Network error: {0}")]
    Network(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_client() -> BootstrapClient {
        let keypair = KeyPair::generate();
        let node_id = NodeId([42u8; 32]);
        
        let descriptor = NodeDescriptor::new(
            keypair.public.to_bytes().to_vec(),
            vec!["127.0.0.1:8080".parse().unwrap()],
            1,
            NodeCapabilities::default(),
            1_000_000,
            Some("us-west".to_string()),
        );
        
        let config = BootstrapConfig::default();
        
        BootstrapClient::new(node_id, keypair, descriptor, config)
    }
    
    #[test]
    fn test_bootstrap_client_creation() {
        let client = create_test_client();
        assert_eq!(client.config.bootstrap_nodes.len(), 3);
        assert_eq!(client.config.query_count, 3);
    }
    
    #[test]
    fn test_bootstrap_sequence() {
        let client = create_test_client();
        
        let result = client.bootstrap();
        assert!(result.is_ok());
        
        let bootstrap_result = result.unwrap();
        assert_eq!(bootstrap_result.network_state.node_count, 100);
        assert!(bootstrap_result.network.nodes.len() == 100);
        assert!(bootstrap_result.announcement.membership_proof.len() > 0);
    }
    
    #[test]
    fn test_merkle_root_verification() {
        let client = create_test_client();
        
        let mut network = Network::new();
        network.add_node(client.node_id);
        
        let network_state = NetworkState {
            merkle_root: *network.merkle_root().as_bytes(),
            epoch: 1,
            node_count: 1,
            last_update: current_timestamp(),
        };
        
        // Should succeed
        assert!(client.verify_merkle_root(&network, &network_state).is_ok());
        
        // Wrong root should fail
        let wrong_state = NetworkState {
            merkle_root: [0xFFu8; 32],
            epoch: 1,
            node_count: 1,
            last_update: current_timestamp(),
        };
        
        assert!(client.verify_merkle_root(&network, &wrong_state).is_err());
    }
    
    #[test]
    fn test_membership_proof_generation() {
        let client = create_test_client();
        
        let mut network = Network::new();
        network.add_node(client.node_id);
        
        let proof = client.generate_membership_proof(&network);
        assert!(proof.is_ok());
        assert!(proof.unwrap().len() > 0);
    }
    
    #[test]
    fn test_bootstrap_config_defaults() {
        let config = BootstrapConfig::default();
        
        assert_eq!(config.bootstrap_nodes.len(), 3);
        assert_eq!(config.query_count, 3);
        assert_eq!(config.timeout_secs, 10);
        assert_eq!(config.max_retries, 3);
    }
}
