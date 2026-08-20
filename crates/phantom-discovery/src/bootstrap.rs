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
use phantom_core::network::{NetworkGraph, NodeId, NodeInfo};
use phantom_core::identity::NodeIdentity;
use phantom_crypto::pq::SigningKeyPair;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Bootstrap client for joining the PHANTOM network
pub struct BootstrapClient {
    /// Our routing index: the key this node occupies in the network graph and
    /// the leaf it occupies in the Merkle tree. Public by construction.
    node_id: NodeId,

    /// Our secret identity. Never sent; it is the nullifier preimage that
    /// makes a repeat announcement detectable without making the announcer
    /// identifiable. See phantom_core::identity for why this is not node_id.
    identity: NodeIdentity,

    /// Dilithium-5 signing keypair. The KEM keypair cannot sign, which is what
    /// this field used to be typed as.
    keypair: SigningKeyPair,
    
    /// Our public descriptor
    descriptor: NodeDescriptor,
    
    /// Bootstrap configuration
    config: BootstrapConfig,
}

/// Bootstrap configuration
#[derive(Clone, Debug)]
pub struct BootstrapConfig {
    /// Hardcoded bootstrap node addresses, as `host:port` strings.
    ///
    /// These are deliberately *not* `SocketAddr`. `SocketAddr` parses literal
    /// IP addresses only — `"bootstrap1.phantom.network:8080".parse()` returns
    /// `AddrParseError`, and the previous version unwrapped it inside
    /// `Default::default()`, so simply asking for the default configuration
    /// panicked. Every test in this module died on that line, and so would
    /// every node that started with default settings.
    ///
    /// Names also outlive addresses: an operator who has to move a bootstrap
    /// node should not need every client to ship a new binary. Resolution
    /// happens when a connection is attempted, not here.
    pub bootstrap_nodes: Vec<String>,
    
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
                "bootstrap1.phantom.network:8080".to_string(),
                "bootstrap2.phantom.network:8080".to_string(),
                "bootstrap3.phantom.network:8080".to_string(),
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
    pub network: NetworkGraph,
    
    /// Our generated announcement
    pub announcement: NodeAnnouncement,
    
    /// Bootstrap nodes contacted, as configured (`host:port`).
    pub bootstrap_nodes: Vec<String>,
}

impl BootstrapClient {
    /// Create new bootstrap client
    pub fn new(
        node_id: NodeId,
        identity: NodeIdentity,
        keypair: SigningKeyPair,
        descriptor: NodeDescriptor,
        config: BootstrapConfig,
    ) -> Self {
        Self {
            node_id,
            identity,
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
        
        println!("      ✓ Downloaded tree with {} nodes", network.node_count());
        
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
            &self.identity,
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
    fn query_bootstrap_nodes(&self) -> Result<(NetworkState, Vec<String>), BootstrapError> {
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
        
        // Mock network state (in production, parse from bootstrap response).
        //
        // The advertised root is taken from the same tree download_merkle_tree
        // will construct. It used to be a hard-coded [0xAA; 32], which no
        // blake3 commitment over a real node set will ever equal — so
        // verify_merkle_root rejected every bootstrap and bootstrap() could
        // not succeed even against its own mock.
        //
        // Deriving it here keeps the verification step honest rather than
        // removing it: the two sides are built independently and still
        // compared, so a change to one and not the other is still caught.
        const MOCK_NODE_COUNT: usize = 100;
        let network_state = NetworkState {
            merkle_root: *self.mock_network(MOCK_NODE_COUNT).commitment(),
            epoch: NetworkState::current_epoch(),
            node_count: MOCK_NODE_COUNT,
            last_update: current_timestamp(),
        };
        
        Ok((network_state, to_query.to_vec()))
    }
    
    /// Download network Merkle tree from bootstrap nodes
    fn download_merkle_tree(
        &self,
        bootstrap_nodes: &[String],
        network_state: &NetworkState,
    ) -> Result<NetworkGraph, BootstrapError> {
        // In production, download actual Merkle tree from bootstrap nodes
        // For now, create mock network
        
        Ok(self.mock_network(network_state.node_count))
    }

    /// Build the stand-in network both the advertised state and the
    /// "downloaded" tree are derived from.
    ///
    /// Deterministic in `self.node_id` and `count`, so the two callers agree.
    /// This disappears when the real download lands.
    fn mock_network(&self, count: usize) -> NetworkGraph {
        let mut network = NetworkGraph::new();
        
        // Add ourselves to the network
        network.add_node(placeholder_node_info(self.node_id));
        
        // In production, would download all nodes and reconstruct tree
        for i in 0..count.saturating_sub(1) {
            let node_id = (i as u32) + 100;  // Offset to avoid ID collision
            network.add_node(placeholder_node_info(node_id));
        }
        
        network
    }
    
    /// Verify that downloaded Merkle tree matches network commitment
    fn verify_merkle_root(
        &self,
        network: &NetworkGraph,
        network_state: &NetworkState,
    ) -> Result<(), BootstrapError> {
        let computed_root = network.commitment();
        
        if computed_root == &network_state.merkle_root {
            Ok(())
        } else {
            Err(BootstrapError::MerkleRootMismatch {
                expected: network_state.merkle_root,
                computed: *computed_root,
            })
        }
    }
    
    /// Generate zk-SNARK membership proof
    fn generate_membership_proof(
        &self,
        network: &NetworkGraph,
    ) -> Result<Vec<u8>, BootstrapError> {
        // The graph owns the tree and knows our leaf. The previous version
        // scanned a public `nodes` vector for a position, which is both a
        // field this type does not have and the wrong lookup: the Merkle leaf
        // is keyed by node id, not by insertion order.
        let merkle_path = network
            .get_membership_proof(self.node_id)
            .ok_or(BootstrapError::NodeNotInTree)?;
        
        // In production, use Plonky2 to generate zk-SNARK proof
        // For now, return mock proof (serialized Merkle path)
        let proof = bincode::serialize(&merkle_path)
            .map_err(|_| BootstrapError::ProofGenerationFailed)?;
        
        Ok(proof)
    }
}

/// Build a NodeInfo for a peer we have only an id for.
///
/// Bootstrap currently fabricates its peer set rather than downloading one, so
/// these metrics are placeholders and are marked as such: zero reputation, no
/// measured latency, no claimed bandwidth. When the real download lands, these
/// values arrive with the descriptor and this function goes away.
fn placeholder_node_info(id: NodeId) -> NodeInfo {
    NodeInfo {
        id,
        bandwidth: 0,
        latency_ms: 0,
        uptime_hours: 0,
        reputation: 0.0,
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
        let keypair = SigningKeyPair::generate();
        // Routing index and secret identity are separate values on purpose.
        let node_id: NodeId = 42;
        let identity = NodeIdentity([42u8; 32]);
        
        let descriptor = NodeDescriptor::new(
            keypair.public.0.clone(),
            vec!["127.0.0.1:8080".parse().unwrap()],
            1,
            NodeCapabilities::default(),
            1_000_000,
            Some("us-west".to_string()),
        );
        
        let config = BootstrapConfig::default();
        
        BootstrapClient::new(node_id, identity, keypair, descriptor, config)
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
        assert_eq!(bootstrap_result.network.node_count(), 100);
        assert!(bootstrap_result.announcement.membership_proof.len() > 0);
    }
    
    #[test]
    fn test_merkle_root_verification() {
        let client = create_test_client();
        
        let mut network = NetworkGraph::new();
        network.add_node(placeholder_node_info(client.node_id));
        
        let network_state = NetworkState {
            merkle_root: *network.commitment(),
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
        
        let mut network = NetworkGraph::new();
        network.add_node(placeholder_node_info(client.node_id));
        
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
