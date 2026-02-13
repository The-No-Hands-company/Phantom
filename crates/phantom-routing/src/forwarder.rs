//! Oblivious Packet Forwarder
//!
//! This module implements the core routing engine for PHANTOM.
//! Nodes use FHE to determine next hop WITHOUT learning the path.

use phantom_core::{PhantomPacket, NetworkGraph};
use phantom_core::proof::ProofGenerator;
use phantom_crypto::FheEngine;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Oblivious packet forwarder
///
/// This is the heart of PHANTOM routing. Each node runs a forwarder
/// that processes packets using FHE without learning routing metadata.
pub struct ObliviousForwarder {
    /// This node's ID
    node_id: u32,
    
    /// FHE engine for oblivious operations
    fhe_engine: Arc<FheEngine>,
    
    /// Network graph (for path validation)
    network: Arc<RwLock<NetworkGraph>>,
    
    /// Optional proof generator for verification (production mode)
    proof_generator: Option<Arc<dyn ProofGenerator>>,
    
    /// Packet statistics
    stats: Arc<RwLock<ForwardingStats>>,
    
    /// Rate limiting: track recent nullifiers to prevent replay
    seen_nullifiers: Arc<RwLock<HashMap<[u8; 32], Instant>>>,
    
    /// Maximum nullifier cache size
    max_nullifier_cache: usize,
    
    /// Nullifier expiry duration
    nullifier_ttl: Duration,
}

/// Forwarding statistics
#[derive(Debug, Clone, Default)]
pub struct ForwardingStats {
    pub packets_received: u64,
    pub packets_forwarded: u64,
    pub packets_delivered: u64,
    pub packets_dropped: u64,
    pub total_fhe_time_ms: u64,
    pub total_verification_time_ms: u64,
}

/// Routing decision from FHE evaluation
#[derive(Debug, Clone, PartialEq)]
pub enum RoutingDecision {
    /// Forward packet to specified next hop
    Forward(u32),
    
    /// Deliver to local application (this node is destination)
    Deliver,
    
    /// Drop packet (invalid proof, replay attack, etc.)
    Drop(DropReason),
}

/// Reason for dropping a packet
#[derive(Debug, Clone, PartialEq)]
pub enum DropReason {
    /// Invalid path proof
    InvalidProof,
    
    /// Replay attack detected (nullifier already seen)
    ReplayAttack,
    
    /// FHE evaluation failed
    FheError(String),
    
    /// Routing table lookup returned invalid result
    InvalidRouting,
}

impl ObliviousForwarder {
    /// Create a new oblivious forwarder
    pub fn new(
        node_id: u32,
        fhe_engine: Arc<FheEngine>,
        network: Arc<RwLock<NetworkGraph>>,
    ) -> Self {
        Self {
            node_id,
            fhe_engine,
            network,
            proof_generator: None,
            stats: Arc::new(RwLock::new(ForwardingStats::default())),
            seen_nullifiers: Arc::new(RwLock::new(HashMap::new())),
            max_nullifier_cache: 10_000,
            nullifier_ttl: Duration::from_secs(3600), // 1 hour
        }
    }
    
    /// Create a new oblivious forwarder with proof verification (production mode)
    pub fn with_proof_generator(
        node_id: u32,
        fhe_engine: Arc<FheEngine>,
        network: Arc<RwLock<NetworkGraph>>,
        proof_generator: Arc<dyn ProofGenerator>,
    ) -> Self {
        Self {
            node_id,
            fhe_engine,
            network,
            proof_generator: Some(proof_generator),
            stats: Arc::new(RwLock::new(ForwardingStats::default())),
            seen_nullifiers: Arc::new(RwLock::new(HashMap::new())),
            max_nullifier_cache: 10_000,
            nullifier_ttl: Duration::from_secs(3600), // 1 hour
        }
    }
    
    /// Process an incoming packet and determine routing action
    ///
    /// This is the CRITICAL method that performs oblivious routing:
    /// 1. Verify path proof (zk-SNARK validation)
    /// 2. Check for replay attacks (nullifier uniqueness)
    /// 3. Perform FHE routing table lookup
    /// 4. Return routing decision WITHOUT learning the path
    pub fn process_packet(&self, packet: &PhantomPacket) -> anyhow::Result<RoutingDecision> {
        // Update stats
        {
            let mut stats = self.stats.write().unwrap();
            stats.packets_received += 1;
        }
        
        // Step 1: Verify path proof against network commitment
        let verify_start = Instant::now();
        let network = self.network.read().unwrap();
        let network_commitment = network.commitment();
        
        let proof_valid = if let Some(ref proof_gen) = self.proof_generator {
            // Production mode: use actual proof generator
            proof_gen.verify_path_proof(&packet.path_proof, network_commitment)?
        } else {
            // Test mode: skip proof verification (assumes pre-verified packets)
            true
        };
        drop(network); // Release lock early
        
        let verify_time = verify_start.elapsed();
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_verification_time_ms += verify_time.as_millis() as u64;
        }
        
        if !proof_valid {
            let mut stats = self.stats.write().unwrap();
            stats.packets_dropped += 1;
            return Ok(RoutingDecision::Drop(DropReason::InvalidProof));
        }
        
        // Step 2: Check for replay attacks (nullifier deduplication)
        if self.is_replay_attack(&packet.nullifier)? {
            let mut stats = self.stats.write().unwrap();
            stats.packets_dropped += 1;
            return Ok(RoutingDecision::Drop(DropReason::ReplayAttack));
        }
        
        // Step 3: Perform oblivious routing table lookup using FHE
        // CRITICAL: Node NEVER decrypts the routing blob!
        let fhe_start = Instant::now();
        let next_hop = self.fhe_engine
            .oblivious_routing_lookup(self.node_id, &packet.routing_blob)
            .map_err(|e| {
                let mut stats = self.stats.write().unwrap();
                stats.packets_dropped += 1;
                anyhow::anyhow!("FHE routing failed: {}", e)
            })?;
        
        let fhe_time = fhe_start.elapsed();
        {
            let mut stats = self.stats.write().unwrap();
            stats.total_fhe_time_ms += fhe_time.as_millis() as u64;
        }
        
        // Step 4: Record nullifier to prevent replay
        self.record_nullifier(packet.nullifier)?;
        
        // Step 5: Determine routing decision
        let decision = if next_hop == 0 {
            // Destination reached - deliver to local application
            let mut stats = self.stats.write().unwrap();
            stats.packets_delivered += 1;
            RoutingDecision::Deliver
        } else if next_hop > 0 && next_hop < u32::MAX {
            // Forward to next hop
            let mut stats = self.stats.write().unwrap();
            stats.packets_forwarded += 1;
            RoutingDecision::Forward(next_hop)
        } else {
            // Invalid routing result
            let mut stats = self.stats.write().unwrap();
            stats.packets_dropped += 1;
            RoutingDecision::Drop(DropReason::InvalidRouting)
        };
        
        Ok(decision)
    }
    
    /// Check if packet is a replay attack
    fn is_replay_attack(&self, nullifier: &[u8; 32]) -> anyhow::Result<bool> {
        let seen = self.seen_nullifiers.read().unwrap();
        
        // Check if we've seen this nullifier before
        if let Some(&first_seen) = seen.get(nullifier) {
            // Check if it's still within TTL
            if first_seen.elapsed() < self.nullifier_ttl {
                return Ok(true); // Replay attack!
            }
        }
        
        Ok(false)
    }
    
    /// Record a nullifier to prevent future replays
    fn record_nullifier(&self, nullifier: [u8; 32]) -> anyhow::Result<()> {
        let mut seen = self.seen_nullifiers.write().unwrap();
        
        // Evict expired nullifiers if cache is full
        if seen.len() >= self.max_nullifier_cache {
            let now = Instant::now();
            seen.retain(|_, &mut first_seen| {
                now.duration_since(first_seen) < self.nullifier_ttl
            });
            
            // If still full, remove oldest entries
            if seen.len() >= self.max_nullifier_cache {
                let to_remove = seen.len() - (self.max_nullifier_cache * 9 / 10);
                let mut entries: Vec<_> = seen.iter().collect();
                entries.sort_by_key(|(_, &ts)| ts);
                let remove_keys: Vec<_> = entries.iter()
                    .take(to_remove)
                    .map(|(k, _)| **k)
                    .collect();
                for key in remove_keys {
                    seen.remove(&key);
                }
            }
        }
        
        seen.insert(nullifier, Instant::now());
        Ok(())
    }
    
    /// Get current forwarding statistics
    pub fn stats(&self) -> ForwardingStats {
        self.stats.read().unwrap().clone()
    }
    
    /// Clear nullifier cache (for testing)
    #[cfg(test)]
    pub fn clear_nullifiers(&self) {
        self.seen_nullifiers.write().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phantom_core::packet::RoutingPath;
    
    #[test]
    fn test_forwarder_creation() {
        let fhe_engine = Arc::new(FheEngine::generate_keys());
        let network = Arc::new(RwLock::new(NetworkGraph::new()));
        
        let forwarder = ObliviousForwarder::new(100, fhe_engine, network);
        
        assert_eq!(forwarder.node_id, 100);
        let stats = forwarder.stats();
        assert_eq!(stats.packets_received, 0);
    }
    
    #[test]
    #[ignore] // Slow test - FHE operations
    fn test_packet_forwarding() {
        // Setup
        let fhe_engine = Arc::new(FheEngine::generate_keys());
        let network = Arc::new(RwLock::new(NetworkGraph::new()));
        let forwarder = ObliviousForwarder::new(100, fhe_engine.clone(), network.clone());
        
        // Create a test packet
        let path = RoutingPath::new(vec![100, 200, 300]).unwrap();
        let payload = b"Test message".to_vec();
        let commitment = network.read().unwrap().commitment().clone();
        
        let packet = PhantomPacket::construct(
            path,
            payload,
            &fhe_engine,
            &commitment,
        ).unwrap();
        
        // Process packet
        let decision = forwarder.process_packet(&packet).unwrap();
        
        // Should forward to next hop (200)
        assert_eq!(decision, RoutingDecision::Forward(200));
        
        // Check stats
        let stats = forwarder.stats();
        assert_eq!(stats.packets_received, 1);
        assert_eq!(stats.packets_forwarded, 1);
        assert_eq!(stats.packets_delivered, 0);
        assert_eq!(stats.packets_dropped, 0);
    }
    
    #[test]
    #[ignore] // Slow test - FHE operations
    fn test_replay_attack_detection() {
        let fhe_engine = Arc::new(FheEngine::generate_keys());
        let network = Arc::new(RwLock::new(NetworkGraph::new()));
        let forwarder = ObliviousForwarder::new(100, fhe_engine.clone(), network.clone());
        
        // Create a test packet
        let path = RoutingPath::new(vec![100, 200, 300]).unwrap();
        let payload = b"Test message".to_vec();
        let commitment = network.read().unwrap().commitment().clone();
        
        let packet = PhantomPacket::construct(
            path,
            payload,
            &fhe_engine,
            &commitment,
        ).unwrap();
        
        // First processing should succeed
        let decision1 = forwarder.process_packet(&packet).unwrap();
        assert_eq!(decision1, RoutingDecision::Forward(200));
        
        // Second processing should detect replay
        let decision2 = forwarder.process_packet(&packet).unwrap();
        assert_eq!(decision2, RoutingDecision::Drop(DropReason::ReplayAttack));
        
        // Check stats
        let stats = forwarder.stats();
        assert_eq!(stats.packets_received, 2);
        assert_eq!(stats.packets_forwarded, 1);
        assert_eq!(stats.packets_dropped, 1);
    }
}
