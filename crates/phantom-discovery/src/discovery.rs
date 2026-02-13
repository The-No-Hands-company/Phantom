//! Peer Discovery Service
//!
//! Query interface for finding nodes in the PHANTOM network.
//! 
//! Features:
//! - Filter by capabilities (routing, directory, FHE, zkVM)
//! - Filter by region (latency optimization)
//! - Filter by bandwidth capacity
//! - Return random subsets (prevent topology leakage)
//! - Rate limiting per query type

use crate::announcement::{NodeAnnouncement, NodeCapabilities, NodeDescriptor};
use crate::state::NetworkState;
use rand::seq::SliceRandom;
use rand::{thread_rng, Rng};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Peer discovery query service
pub struct DiscoveryService {
    /// Active node announcements (keyed by nullifier)
    announcements: HashMap<[u8; 32], NodeAnnouncement>,
    
    /// Network state (Merkle root, epoch)
    network_state: NetworkState,
    
    /// Configuration
    config: DiscoveryConfig,
    
    /// Query rate limiter (IP -> last query timestamp)
    rate_limiter: HashMap<String, u64>,
}

/// Discovery service configuration
#[derive(Clone, Debug)]
pub struct DiscoveryConfig {
    /// Maximum nodes returned per query
    pub max_results: usize,
    
    /// Minimum time between queries from same IP (seconds)
    pub rate_limit_secs: u64,
    
    /// Maximum announcement age to consider (seconds)
    pub max_announcement_age: u64,
    
    /// Prefer geographically diverse results
    pub prefer_diversity: bool,
}

impl Default for DiscoveryConfig {
    fn default() -> Self {
        Self {
            max_results: 50,          // Return up to 50 nodes
            rate_limit_secs: 10,      // 1 query per 10 seconds per IP
            max_announcement_age: 600, // 10 minutes
            prefer_diversity: true,   // Prefer geographically diverse nodes
        }
    }
}

/// Query filter for node discovery
#[derive(Clone, Debug, Default)]
pub struct DiscoveryQuery {
    /// Filter by routing capability
    pub requires_routing: Option<bool>,
    
    /// Filter by directory capability
    pub requires_directory: Option<bool>,
    
    /// Filter by FHE routing support
    pub requires_fhe: Option<bool>,
    
    /// Filter by zkVM verification support
    pub requires_zkvm: Option<bool>,
    
    /// Filter by minimum bandwidth (bytes/sec)
    pub min_bandwidth: Option<u64>,
    
    /// Filter by region (exact match)
    pub region: Option<String>,
    
    /// Number of results to return (capped by config.max_results)
    pub limit: usize,
    
    /// Require fresh announcements only (within 5 minutes)
    pub fresh_only: bool,
}

/// Discovery query result
#[derive(Clone, Debug)]
pub struct DiscoveryResult {
    /// Matching node descriptors
    pub nodes: Vec<NodeDescriptor>,
    
    /// Total matching nodes (before random sampling)
    pub total_matches: usize,
    
    /// Network state at query time
    pub network_state: NetworkState,
}

impl DiscoveryService {
    /// Create new discovery service
    pub fn new(config: DiscoveryConfig) -> Self {
        Self {
            announcements: HashMap::new(),
            network_state: NetworkState::new(),
            config,
            rate_limiter: HashMap::new(),
        }
    }
    
    /// Add node announcement to discovery pool
    pub fn add_announcement(&mut self, announcement: NodeAnnouncement) -> Result<(), DiscoveryError> {
        // Check if announcement is fresh
        if !announcement.is_fresh() {
            return Err(DiscoveryError::ExpiredAnnouncement);
        }
        
        // Check for duplicate nullifier
        if self.announcements.contains_key(&announcement.nullifier) {
            return Err(DiscoveryError::DuplicateNullifier);
        }
        
        // Add to pool
        self.announcements.insert(announcement.nullifier, announcement);
        
        Ok(())
    }
    
    /// Remove expired announcements (older than max_announcement_age)
    pub fn prune_expired(&mut self) -> usize {
        let now = current_timestamp();
        let max_age = self.config.max_announcement_age;
        
        let initial_count = self.announcements.len();
        
        self.announcements.retain(|_, announcement| {
            now.saturating_sub(announcement.timestamp) < max_age
        });
        
        initial_count - self.announcements.len()
    }
    
    /// Query for nodes matching criteria
    pub fn query(
        &mut self,
        query: DiscoveryQuery,
        requester_ip: &str,
    ) -> Result<DiscoveryResult, DiscoveryError> {
        // Check rate limit
        if !self.check_rate_limit(requester_ip)? {
            return Err(DiscoveryError::RateLimited);
        }
        
        // Filter announcements by query criteria
        let matching: Vec<&NodeAnnouncement> = self.announcements
            .values()
            .filter(|announcement| self.matches_query(announcement, &query))
            .collect();
        
        let total_matches = matching.len();
        
        // Randomize to prevent topology inference
        let mut rng = thread_rng();
        let mut selected = matching;
        selected.shuffle(&mut rng);
        
        // Limit results
        let limit = query.limit.min(self.config.max_results);
        selected.truncate(limit);
        
        // Apply diversity preference if enabled
        let nodes = if self.config.prefer_diversity && selected.len() > limit / 2 {
            self.select_diverse_nodes(&selected, limit)
        } else {
            selected.into_iter()
                .map(|a| a.descriptor.clone())
                .collect()
        };
        
        Ok(DiscoveryResult {
            nodes,
            total_matches,
            network_state: self.network_state.clone(),
        })
    }
    
    /// Update network state
    pub fn update_network_state(&mut self, state: NetworkState) {
        self.network_state = state;
    }
    
    /// Get current network state
    pub fn network_state(&self) -> &NetworkState {
        &self.network_state
    }
    
    /// Get number of active announcements
    pub fn active_announcements(&self) -> usize {
        self.announcements.len()
    }
    
    /// Check if an announcement matches query criteria
    fn matches_query(&self, announcement: &NodeAnnouncement, query: &DiscoveryQuery) -> bool {
        let descriptor = &announcement.descriptor;
        let capabilities = &descriptor.capabilities;
        
        // Check freshness
        if query.fresh_only && !announcement.is_fresh() {
            return false;
        }
        
        // Check capabilities
        if let Some(requires_routing) = query.requires_routing {
            if capabilities.routing != requires_routing {
                return false;
            }
        }
        
        if let Some(requires_directory) = query.requires_directory {
            if capabilities.directory != requires_directory {
                return false;
            }
        }
        
        if let Some(requires_fhe) = query.requires_fhe {
            if capabilities.fhe_routing != requires_fhe {
                return false;
            }
        }
        
        if let Some(requires_zkvm) = query.requires_zkvm {
            if capabilities.zkvm_verification != requires_zkvm {
                return false;
            }
        }
        
        // Check bandwidth
        if let Some(min_bandwidth) = query.min_bandwidth {
            if descriptor.bandwidth_capacity < min_bandwidth {
                return false;
            }
        }
        
        // Check region
        if let Some(ref region) = query.region {
            match &descriptor.region_hint {
                Some(node_region) => {
                    if node_region != region {
                        return false;
                    }
                }
                None => return false,
            }
        }
        
        true
    }
    
    /// Select geographically diverse nodes
    fn select_diverse_nodes(
        &self,
        candidates: &[&NodeAnnouncement],
        limit: usize,
    ) -> Vec<NodeDescriptor> {
        // Group by region
        let mut regions: HashMap<String, Vec<&NodeAnnouncement>> = HashMap::new();
        
        for announcement in candidates {
            let region = announcement.descriptor.region_hint
                .clone()
                .unwrap_or_else(|| "unknown".to_string());
            
            regions.entry(region)
                .or_insert_with(Vec::new)
                .push(announcement);
        }
        
        // Select evenly from each region
        let mut selected = Vec::new();
        let mut rng = thread_rng();
        
        while selected.len() < limit {
            let mut added = false;
            
            for region_nodes in regions.values_mut() {
                if !region_nodes.is_empty() && selected.len() < limit {
                    // Random selection from region
                    let idx = rng.gen::<usize>() % region_nodes.len();
                    let node = region_nodes.swap_remove(idx);
                    selected.push(node.descriptor.clone());
                    added = true;
                }
            }
            
            // Break if no more nodes to add
            if !added {
                break;
            }
        }
        
        selected
    }
    
    /// Check rate limit for requester
    fn check_rate_limit(&mut self, requester_ip: &str) -> Result<bool, DiscoveryError> {
        let now = current_timestamp();
        
        if let Some(&last_query) = self.rate_limiter.get(requester_ip) {
            if now.saturating_sub(last_query) < self.config.rate_limit_secs {
                return Ok(false);
            }
        }
        
        self.rate_limiter.insert(requester_ip.to_string(), now);
        Ok(true)
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
pub enum DiscoveryError {
    #[error("Announcement has expired")]
    ExpiredAnnouncement,
    
    #[error("Duplicate nullifier (already announced)")]
    DuplicateNullifier,
    
    #[error("Rate limited - too many queries")]
    RateLimited,
    
    #[error("Invalid query parameters")]
    InvalidQuery,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::announcement::{NodeAnnouncement, NodeDescriptor, NodeCapabilities};
    use phantom_core::network::NodeId;
    use phantom_crypto::pq::KeyPair;
    use std::net::SocketAddr;
    
    fn create_test_announcement(
        region: Option<String>,
        bandwidth: u64,
        capabilities: NodeCapabilities,
    ) -> NodeAnnouncement {
        let keypair = KeyPair::generate();
        
        let descriptor = NodeDescriptor::new(
            vec![1, 2, 3, 4],
            vec!["127.0.0.1:8080".parse().unwrap()],
            1,
            capabilities,
            bandwidth,
            region,
        );
        
        NodeAnnouncement::new(
            descriptor,
            vec![0u8; 100], // Mock membership proof
            &NodeId([42u8; 32]),
            1,
            &[0xBBu8; 32],
            &keypair,
        ).unwrap()
    }
    
    #[test]
    fn test_discovery_service_creation() {
        let service = DiscoveryService::new(DiscoveryConfig::default());
        assert_eq!(service.active_announcements(), 0);
    }
    
    #[test]
    fn test_add_announcement() {
        let mut service = DiscoveryService::new(DiscoveryConfig::default());
        
        let announcement = create_test_announcement(
            Some("us-west".to_string()),
            1_000_000,
            NodeCapabilities::default(),
        );
        
        assert!(service.add_announcement(announcement).is_ok());
        assert_eq!(service.active_announcements(), 1);
    }
    
    #[test]
    fn test_duplicate_nullifier_rejection() {
        let mut service = DiscoveryService::new(DiscoveryConfig::default());
        
        let announcement1 = create_test_announcement(
            Some("us-west".to_string()),
            1_000_000,
            NodeCapabilities::default(),
        );
        
        let announcement2 = announcement1.clone();
        
        assert!(service.add_announcement(announcement1).is_ok());
        assert!(matches!(
            service.add_announcement(announcement2),
            Err(DiscoveryError::DuplicateNullifier)
        ));
    }
    
    #[test]
    fn test_query_by_region() {
        let mut service = DiscoveryService::new(DiscoveryConfig::default());
        
        // Add nodes in different regions
        for i in 0..5 {
            let region = Some(format!("us-west-{}", i % 2));
            let announcement = create_test_announcement(
                region,
                1_000_000,
                NodeCapabilities::default(),
            );
            service.add_announcement(announcement).unwrap();
        }
        
        // Query for us-west-0 nodes
        let query = DiscoveryQuery {
            region: Some("us-west-0".to_string()),
            limit: 10,
            ..Default::default()
        };
        
        let result = service.query(query, "192.168.1.1").unwrap();
        
        // Should match ~half of nodes
        assert!(result.total_matches >= 2);
        assert!(result.nodes.len() >= 2);
        
        // Verify all returned nodes are from us-west-0
        for node in &result.nodes {
            assert_eq!(node.region_hint.as_ref().unwrap(), "us-west-0");
        }
    }
    
    #[test]
    fn test_query_by_capabilities() {
        let mut service = DiscoveryService::new(DiscoveryConfig::default());
        
        // Add nodes with different capabilities
        let caps_full = NodeCapabilities {
            routing: true,
            directory: true,
            fhe_routing: true,
            zkvm_verification: true,
        };
        
        let caps_basic = NodeCapabilities {
            routing: true,
            directory: false,
            fhe_routing: false,
            zkvm_verification: false,
        };
        
        service.add_announcement(create_test_announcement(None, 1_000_000, caps_full)).unwrap();
        service.add_announcement(create_test_announcement(None, 1_000_000, caps_basic.clone())).unwrap();
        service.add_announcement(create_test_announcement(None, 1_000_000, caps_basic)).unwrap();
        
        // Query for directory nodes
        let query = DiscoveryQuery {
            requires_directory: Some(true),
            limit: 10,
            ..Default::default()
        };
        
        let result = service.query(query, "192.168.1.1").unwrap();
        
        assert_eq!(result.total_matches, 1);
        assert_eq!(result.nodes.len(), 1);
    }
    
    #[test]
    fn test_query_by_bandwidth() {
        let mut service = DiscoveryService::new(DiscoveryConfig::default());
        
        // Add nodes with different bandwidth
        service.add_announcement(create_test_announcement(None, 500_000, NodeCapabilities::default())).unwrap();
        service.add_announcement(create_test_announcement(None, 1_000_000, NodeCapabilities::default())).unwrap();
        service.add_announcement(create_test_announcement(None, 10_000_000, NodeCapabilities::default())).unwrap();
        
        // Query for high-bandwidth nodes (>= 1 MB/s)
        let query = DiscoveryQuery {
            min_bandwidth: Some(1_000_000),
            limit: 10,
            ..Default::default()
        };
        
        let result = service.query(query, "192.168.1.1").unwrap();
        
        assert_eq!(result.total_matches, 2);
        assert_eq!(result.nodes.len(), 2);
        
        // Verify all returned nodes have sufficient bandwidth
        for node in &result.nodes {
            assert!(node.bandwidth_capacity >= 1_000_000);
        }
    }
    
    #[test]
    fn test_result_limit() {
        let mut service = DiscoveryService::new(DiscoveryConfig {
            max_results: 3,
            ..Default::default()
        });
        
        // Add 10 nodes
        for _ in 0..10 {
            service.add_announcement(create_test_announcement(
                None,
                1_000_000,
                NodeCapabilities::default(),
            )).unwrap();
        }
        
        // Query with limit of 100 (should cap at 3)
        let query = DiscoveryQuery {
            limit: 100,
            ..Default::default()
        };
        
        let result = service.query(query, "192.168.1.1").unwrap();
        
        assert_eq!(result.total_matches, 10);
        assert_eq!(result.nodes.len(), 3); // Capped by config.max_results
    }
    
    #[test]
    fn test_rate_limiting() {
        let mut service = DiscoveryService::new(DiscoveryConfig {
            rate_limit_secs: 5,
            ..Default::default()
        });
        
        service.add_announcement(create_test_announcement(
            None,
            1_000_000,
            NodeCapabilities::default(),
        )).unwrap();
        
        let query = DiscoveryQuery {
            limit: 10,
            ..Default::default()
        };
        
        // First query succeeds
        assert!(service.query(query.clone(), "192.168.1.1").is_ok());
        
        // Second query from same IP fails (rate limited)
        assert!(matches!(
            service.query(query, "192.168.1.1"),
            Err(DiscoveryError::RateLimited)
        ));
    }
    
    #[test]
    fn test_prune_expired() {
        let mut service = DiscoveryService::new(DiscoveryConfig {
            max_announcement_age: 1, // 1 second
            ..Default::default()
        });
        
        let mut announcement = create_test_announcement(
            None,
            1_000_000,
            NodeCapabilities::default(),
        );
        
        // Make announcement old
        announcement.timestamp -= 10;
        
        service.add_announcement(announcement).unwrap();
        assert_eq!(service.active_announcements(), 1);
        
        // Prune expired
        let pruned = service.prune_expired();
        assert_eq!(pruned, 1);
        assert_eq!(service.active_announcements(), 0);
    }
    
    #[test]
    fn test_diversity_selection() {
        let mut service = DiscoveryService::new(DiscoveryConfig {
            prefer_diversity: true,
            max_results: 4,
            ..Default::default()
        });
        
        // Add 3 nodes from us-west, 3 from eu-central
        for i in 0..3 {
            service.add_announcement(create_test_announcement(
                Some("us-west".to_string()),
                1_000_000,
                NodeCapabilities::default(),
            )).unwrap();
            
            service.add_announcement(create_test_announcement(
                Some("eu-central".to_string()),
                1_000_000,
                NodeCapabilities::default(),
            )).unwrap();
        }
        
        let query = DiscoveryQuery {
            limit: 4,
            ..Default::default()
        };
        
        let result = service.query(query, "192.168.1.1").unwrap();
        
        assert_eq!(result.nodes.len(), 4);
        
        // Count regions in result
        let us_west_count = result.nodes.iter()
            .filter(|n| n.region_hint.as_ref().unwrap() == "us-west")
            .count();
        
        let eu_central_count = result.nodes.iter()
            .filter(|n| n.region_hint.as_ref().unwrap() == "eu-central")
            .count();
        
        // Should have roughly balanced selection (2 from each region)
        assert_eq!(us_west_count, 2);
        assert_eq!(eu_central_count, 2);
    }
}
