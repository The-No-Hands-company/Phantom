//! Multi-Hop Forwarding Protocol
//!
//! Implements the complete packet forwarding engine with network I/O simulation.

use crate::forwarder::{ObliviousForwarder, RoutingDecision, DropReason};
use crate::wire_format::{serialize_packet, deserialize_packet};
use phantom_core::{PhantomPacket, NetworkGraph};
use phantom_crypto::FheEngine;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Network simulator for multi-hop routing
///
/// Simulates a network of PHANTOM nodes forwarding packets through
/// multiple hops using oblivious routing.
pub struct NetworkSimulator {
    /// All nodes in the network (node_id -> forwarder)
    nodes: HashMap<u32, Arc<ObliviousForwarder>>,
    
    /// Network graph
    network: Arc<RwLock<NetworkGraph>>,
    
    /// FHE engine (shared across all nodes)
    fhe_engine: Arc<FheEngine>,
    
    /// Packet delivery log (packet_id -> final_destination)
    delivered_packets: Arc<RwLock<HashMap<[u8; 32], u32>>>,
}

/// Forwarding result for a single hop
#[derive(Debug, Clone)]
pub struct HopResult {
    /// Node that processed the packet
    pub node_id: u32,
    
    /// Routing decision made
    pub decision: RoutingDecision,
    
    /// Time taken for FHE processing (ms)
    pub processing_time_ms: u64,
    
    /// Packet size on wire (bytes)
    pub packet_size: usize,
}

/// Complete path trace through the network
#[derive(Debug, Clone)]
pub struct PathTrace {
    /// All hops in the path
    pub hops: Vec<HopResult>,
    
    /// Total latency (ms)
    pub total_latency_ms: u64,
    
    /// Total bandwidth used (bytes)
    pub total_bytes: usize,
    
    /// Final status
    pub status: PathStatus,
}

/// Final status of a packet's journey
#[derive(Debug, Clone, PartialEq)]
pub enum PathStatus {
    /// Packet successfully delivered
    Delivered,
    
    /// Packet dropped at some hop
    Dropped(DropReason),
    
    /// Routing loop detected
    Loop,
    
    /// Maximum hops exceeded (TTL expired)
    MaxHopsExceeded,
}

impl NetworkSimulator {
    /// Create a new network simulator
    pub fn new(network: Arc<RwLock<NetworkGraph>>) -> Self {
        let fhe_engine = Arc::new(FheEngine::generate_keys());
        
        Self {
            nodes: HashMap::new(),
            network,
            fhe_engine,
            delivered_packets: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    
    /// Get reference to network graph
    pub fn network(&self) -> &Arc<RwLock<NetworkGraph>> {
        &self.network
    }
    
    /// Get reference to FHE engine
    pub fn fhe_engine(&self) -> &Arc<FheEngine> {
        &self.fhe_engine
    }
    
    /// Add a node to the network
    pub fn add_node(&mut self, node_id: u32) {
        let forwarder = Arc::new(ObliviousForwarder::new(
            node_id,
            self.fhe_engine.clone(),
            self.network.clone(),
        ));
        
        self.nodes.insert(node_id, forwarder);
    }
    
    /// Simulate forwarding a packet through the network
    ///
    /// Returns a trace of all hops taken.
    pub fn forward_packet(&self, packet: PhantomPacket, entry_node: u32) -> anyhow::Result<PathTrace> {
        let mut hops = Vec::new();
        let mut current_node = entry_node;
        let mut visited = std::collections::HashSet::new();
        let max_hops = 20; // TTL
        
        loop {
            // Check for loops
            if !visited.insert(current_node) {
                let total_latency_ms = hops.iter().map(|h: &HopResult| h.processing_time_ms).sum();
                let total_bytes = hops.iter().map(|h| h.packet_size).sum();
                return Ok(PathTrace {
                    hops,
                    total_latency_ms,
                    total_bytes,
                    status: PathStatus::Loop,
                });
            }
            
            // Check TTL
            if hops.len() >= max_hops {
                let total_latency_ms = hops.iter().map(|h| h.processing_time_ms).sum();
                let total_bytes = hops.iter().map(|h| h.packet_size).sum();
                return Ok(PathTrace {
                    hops,
                    total_latency_ms,
                    total_bytes,
                    status: PathStatus::MaxHopsExceeded,
                });
            }
            
            // Get node's forwarder
            let forwarder = self.nodes.get(&current_node)
                .ok_or_else(|| anyhow::anyhow!("Node {} not found in network", current_node))?;
            
            // Serialize packet to wire format
            let wire_bytes = serialize_packet(&packet)
                .map_err(|e| anyhow::anyhow!("Serialization failed: {}", e))?;
            let packet_size = wire_bytes.len();
            
            // Process packet at this node
            let start = std::time::Instant::now();
            let decision = forwarder.process_packet(&packet)?;
            let processing_time_ms = start.elapsed().as_millis() as u64;
            
            // Record hop
            hops.push(HopResult {
                node_id: current_node,
                decision: decision.clone(),
                processing_time_ms,
                packet_size,
            });
            
            // Handle routing decision
            match decision {
                RoutingDecision::Forward(next_hop) => {
                    // Deserialize and re-serialize (simulates network transmission)
                    let _received_packet = deserialize_packet(&wire_bytes)
                        .map_err(|e| anyhow::anyhow!("Deserialization failed: {}", e))?;
                    
                    current_node = next_hop;
                }
                RoutingDecision::Deliver => {
                    // Packet delivered successfully
                    self.delivered_packets.write().unwrap()
                        .insert(packet.packet_id, current_node);
                    
                    let total_latency_ms = hops.iter().map(|h| h.processing_time_ms).sum();
                    let total_bytes = hops.iter().map(|h| h.packet_size).sum();
                    return Ok(PathTrace {
                        hops,
                        total_latency_ms,
                        total_bytes,
                        status: PathStatus::Delivered,
                    });
                }
                RoutingDecision::Drop(reason) => {
                    let total_latency_ms = hops.iter().map(|h| h.processing_time_ms).sum();
                    let total_bytes = hops.iter().map(|h| h.packet_size).sum();
                    return Ok(PathTrace {
                        hops,
                        total_latency_ms,
                        total_bytes,
                        status: PathStatus::Dropped(reason),
                    });
                }
            }
        }
    }
    
    /// Get statistics for all nodes
    pub fn network_stats(&self) -> HashMap<u32, crate::forwarder::ForwardingStats> {
        self.nodes.iter()
            .map(|(&id, forwarder)| (id, forwarder.stats()))
            .collect()
    }
    
    /// Check if a packet was delivered
    pub fn was_delivered(&self, packet_id: &[u8; 32]) -> Option<u32> {
        self.delivered_packets.read().unwrap().get(packet_id).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phantom_core::packet::RoutingPath;
    use phantom_core::network::NodeInfo;
    
    fn create_test_network() -> (NetworkSimulator, Vec<u32>) {
        // Create a linear network: 100 -> 101 -> 102 -> 103 -> 104
        let network = Arc::new(RwLock::new(NetworkGraph::new()));
        
        let node_ids = vec![100, 101, 102, 103, 104];
        for &id in &node_ids {
            network.write().unwrap().add_node(NodeInfo {
                id,
                bandwidth: 1_000_000,
                latency_ms: 50,
                uptime_hours: 24,
                reputation: 0.9,
            });
        }
        
        // Connect nodes in a chain
        for i in 0..node_ids.len() - 1 {
            network.write().unwrap().add_edge(node_ids[i], node_ids[i + 1]);
        }
        
        let mut simulator = NetworkSimulator::new(network);
        for &id in &node_ids {
            simulator.add_node(id);
        }
        
        (simulator, node_ids)
    }
    
    #[test]
    #[ignore] // Slow test - FHE operations
    fn test_single_hop_forwarding() {
        let (simulator, nodes) = create_test_network();
        let fhe_engine = simulator.fhe_engine.clone();
        
        // A 2-hop path must be rejected: anonymity needs at least 3.
        //
        // This previously called .unwrap() on the Result and then .is_err() on
        // the unwrapped RoutingPath. The unwrap would have panicked on the very
        // rejection the assertion was checking for — and the assertion itself
        // did not typecheck, so this test had never been compiled.
        assert!(
            RoutingPath::new(vec![100, 101]).is_err(),
            "path validation requires 3+ hops"
        );
        
        // Create valid 3-hop path: 100 -> 101 -> 102
        let path = RoutingPath::new(vec![100, 101, 102]).unwrap();
        let payload = b"Test message".to_vec();
        let commitment = simulator.network.read().unwrap().commitment().clone();
        
        let packet = PhantomPacket::construct(
            path,
            payload,
            &fhe_engine,
            &commitment,
        ).unwrap();
        
        // Forward packet through network
        let trace = simulator.forward_packet(packet.clone(), 100).unwrap();
        
        // Should traverse: 100 -> 101 -> 102 (3 hops)
        assert_eq!(trace.hops.len(), 3);
        assert_eq!(trace.hops[0].node_id, 100);
        assert_eq!(trace.hops[1].node_id, 101);
        assert_eq!(trace.hops[2].node_id, 102);
        assert_eq!(trace.status, PathStatus::Delivered);
        
        // Verify delivery
        assert_eq!(simulator.was_delivered(&packet.packet_id), Some(102));
    }
    
    #[test]
    #[ignore] // Slow test - FHE operations
    fn test_multi_hop_forwarding() {
        let (simulator, _nodes) = create_test_network();
        let fhe_engine = simulator.fhe_engine.clone();
        
        // Create 5-hop path: 100 -> 101 -> 102 -> 103 -> 104
        let path = RoutingPath::new(vec![100, 101, 102, 103, 104]).unwrap();
        let payload = b"Long journey message".to_vec();
        let commitment = simulator.network.read().unwrap().commitment().clone();
        
        let packet = PhantomPacket::construct(
            path,
            payload,
            &fhe_engine,
            &commitment,
        ).unwrap();
        
        // Forward packet through network
        let trace = simulator.forward_packet(packet.clone(), 100).unwrap();
        
        // Should traverse all 5 hops
        assert_eq!(trace.hops.len(), 5);
        assert_eq!(trace.status, PathStatus::Delivered);
        
        // Verify latency is reasonable (5 hops × ~1-2s FHE per hop)
        println!("Total latency: {} ms", trace.total_latency_ms);
        println!("Total bandwidth: {} bytes", trace.total_bytes);
        
        // Each hop should have processing time
        for (i, hop) in trace.hops.iter().enumerate() {
            println!("Hop {}: node {} - {}ms - {} bytes",
                i, hop.node_id, hop.processing_time_ms, hop.packet_size);
        }
    }
    
    #[test]
    #[ignore] // Slow test - FHE operations
    fn test_replay_attack_detection() {
        let (simulator, _nodes) = create_test_network();
        let fhe_engine = simulator.fhe_engine.clone();
        
        // Create packet
        let path = RoutingPath::new(vec![100, 101, 102]).unwrap();
        let payload = b"Test message".to_vec();
        let commitment = simulator.network.read().unwrap().commitment().clone();
        
        let packet = PhantomPacket::construct(
            path,
            payload,
            &fhe_engine,
            &commitment,
        ).unwrap();
        
        // First transmission succeeds
        let trace1 = simulator.forward_packet(packet.clone(), 100).unwrap();
        assert_eq!(trace1.status, PathStatus::Delivered);
        
        // Second transmission should be blocked at first hop (replay detection)
        let trace2 = simulator.forward_packet(packet, 100).unwrap();
        assert_eq!(trace2.hops.len(), 1); // Dropped at first node
        assert!(matches!(trace2.status, PathStatus::Dropped(DropReason::ReplayAttack)));
    }
    
    #[test]
    fn test_network_stats() {
        let (mut simulator, nodes) = create_test_network();
        
        // Get initial stats
        let stats = simulator.network_stats();
        assert_eq!(stats.len(), nodes.len());
        
        for &node_id in &nodes {
            let node_stats = stats.get(&node_id).unwrap();
            assert_eq!(node_stats.packets_received, 0);
            assert_eq!(node_stats.packets_forwarded, 0);
        }
    }
}
