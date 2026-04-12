/// Simulated PHANTOM node for large-scale network testing
///
/// A SimulatedNode represents a single participant in the PHANTOM network during simulation.
/// It includes:
/// - Realistic latency modeling (network delays, processing time)
/// - Configurable behavior (honest, Byzantine/malicious)
/// - Packet processing with FHE oblivious routing
/// - Metrics collection for performance analysis

use phantom_core::{PhantomPacket, NetworkGraph};
use phantom_core::network::NodeInfo;
use phantom_routing::{ObliviousForwarder, RoutingDecision};
use phantom_crypto::FheEngine;
use anyhow::Result;
use std::time::{Duration, Instant};
use std::sync::{Arc, RwLock};
use serde::{Serialize, Deserialize};
use rand::Rng;

/// Node behavior type - determines how the node processes packets
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum NodeBehavior {
    /// Honest node - follows protocol correctly
    Honest,
    
    /// Byzantine node - drops packets randomly
    DropPackets { drop_rate: f64 },
    
    /// Byzantine node - malicious routing (incorrect next hop)
    MaliciousRouting { malice_rate: f64 },
    
    /// Byzantine node - delays packets intentionally
    DelayAttack { delay_ms: u64 },
    
    /// Byzantine node - forges invalid packets
    ForgePackets { forge_rate: f64 },
}

impl NodeBehavior {
    /// Check if this is a Byzantine (malicious) behavior
    pub fn is_byzantine(&self) -> bool {
        !matches!(self, NodeBehavior::Honest)
    }
}

/// Simulated node in the PHANTOM network
pub struct SimulatedNode {
    /// Node identifier
    pub id: u32,
    
    /// Node information (bandwidth, latency, reputation)
    pub info: NodeInfo,
    
    /// Behavior type
    pub behavior: NodeBehavior,
    
    /// FHE oblivious routing engine
    forwarder: ObliviousForwarder,
    
    /// Packets processed by this node
    packets_processed: u64,
    
    /// Packets dropped by this node
    packets_dropped: u64,
    
    /// Total processing time (for latency statistics)
    total_processing_time: Duration,
    
    /// Network latency (simulated link delay)
    network_latency_ms: u64,
}

impl SimulatedNode {
    /// Create a new simulated node with given behavior
    pub fn new(
        id: u32,
        info: NodeInfo,
        behavior: NodeBehavior,
        network_latency_ms: u64,
        fhe_engine: FheEngine,
        network_graph: Arc<RwLock<NetworkGraph>>,
    ) -> Result<Self> {
        let forwarder = ObliviousForwarder::new(id, Arc::new(fhe_engine), network_graph);
        
        Ok(Self {
            id,
            info,
            behavior,
            forwarder,
            packets_processed: 0,
            packets_dropped: 0,
            total_processing_time: Duration::ZERO,
            network_latency_ms,
        })
    }
    
    /// Process a packet according to node behavior
    ///
    /// Returns:
    /// - `Ok(Some(next_hop))` if packet should be forwarded
    /// - `Ok(None)` if packet reaches destination
    /// - `Err(...)` if packet processing fails
    pub fn process_packet(&mut self, _packet: &PhantomPacket) -> Result<Option<u32>> {
        let start = Instant::now();
        
        // Apply Byzantine behavior
        match self.behavior {
            NodeBehavior::Honest => {
                // Process normally
            }
            
            NodeBehavior::DropPackets { drop_rate } => {
                if rand::thread_rng().gen_bool(drop_rate) {
                    self.packets_dropped += 1;
                    return Ok(None);  // Drop packet
                }
            }
            
            NodeBehavior::MaliciousRouting { malice_rate } => {
                if rand::thread_rng().gen_bool(malice_rate) {
                    // Return random incorrect next hop
                    let malicious_hop = rand::thread_rng().gen_range(1..1000);
                    self.packets_processed += 1;
                    self.total_processing_time += start.elapsed();
                    return Ok(Some(malicious_hop));
                }
            }
            
            NodeBehavior::DelayAttack { delay_ms } => {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
            
            NodeBehavior::ForgePackets { forge_rate } => {
                // In real implementation, would try to forge invalid zkSNARK proofs
                // For simulation, we just mark the attempt
                if rand::thread_rng().gen_bool(forge_rate) {
                    tracing::warn!("Node {} attempted packet forgery", self.id);
                    self.packets_dropped += 1;
                    return Ok(None);
                }
            }
        }
        
        // Perform oblivious routing lookup
        // Note: ObliviousForwarder::forward needs packet and network graph
        // For simulation, we'll use a simplified approach
        // TODO: Integrate with actual ObliviousForwarder once API is stable
        let decision = RoutingDecision::Forward(self.id + 1);  // Simplified for now
        
        // Update metrics
        self.packets_processed += 1;
        self.total_processing_time += start.elapsed();
        
        // Simulate network latency
        std::thread::sleep(Duration::from_millis(self.network_latency_ms));
        
        match decision {
            RoutingDecision::Forward(next_hop) => Ok(Some(next_hop)),
            RoutingDecision::Deliver => Ok(None),  // Packet reached destination
            RoutingDecision::Drop(reason) => {
                self.packets_dropped += 1;
                tracing::debug!("Node {} dropped packet: {:?}", self.id, reason);
                Ok(None)
            }
        }
    }
    
    /// Get node statistics
    pub fn stats(&self) -> NodeStats {
        NodeStats {
            node_id: self.id,
            behavior: self.behavior,
            packets_processed: self.packets_processed,
            packets_dropped: self.packets_dropped,
            drop_rate: if self.packets_processed > 0 {
                self.packets_dropped as f64 / (self.packets_processed + self.packets_dropped) as f64
            } else {
                0.0
            },
            avg_processing_time: if self.packets_processed > 0 {
                self.total_processing_time / self.packets_processed as u32
            } else {
                Duration::ZERO
            },
            network_latency: Duration::from_millis(self.network_latency_ms),
        }
    }
    
    /// Reset statistics (for repeated simulation runs)
    pub fn reset_stats(&mut self) {
        self.packets_processed = 0;
        self.packets_dropped = 0;
        self.total_processing_time = Duration::ZERO;
    }
}

/// Statistics for a simulated node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeStats {
    pub node_id: u32,
    pub behavior: NodeBehavior,
    pub packets_processed: u64,
    pub packets_dropped: u64,
    pub drop_rate: f64,
    pub avg_processing_time: Duration,
    pub network_latency: Duration,
}

impl NodeStats {
    /// Print formatted statistics
    pub fn print(&self) {
        println!("Node {} [{:?}]:", self.node_id, self.behavior);
        println!("  Processed: {} packets", self.packets_processed);
        println!("  Dropped: {} packets ({:.2}%)", self.packets_dropped, self.drop_rate * 100.0);
        println!("  Avg processing time: {:?}", self.avg_processing_time);
        println!("  Network latency: {:?}", self.network_latency);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use phantom_core::NetworkGraph;
    
    fn make_test_network_graph() -> Arc<RwLock<NetworkGraph>> {
        Arc::new(RwLock::new(NetworkGraph::new()))
    }
    
    #[test]
    fn test_honest_node_creation() {
        let info = NodeInfo {
            id: 100,
            bandwidth: 1_000_000,
            latency_ms: 50,
            uptime_hours: 100,
            reputation: 0.95,
        };
        
        let fhe = FheEngine::generate_keys();
        let node = SimulatedNode::new(
            100,
            info,
            NodeBehavior::Honest,
            50,
            fhe,
            make_test_network_graph(),
        );
        
        assert!(node.is_ok());
        let node = node.unwrap();
        assert_eq!(node.id, 100);
        assert_eq!(node.behavior, NodeBehavior::Honest);
    }
    
    #[test]
    fn test_byzantine_node_types() {
        assert!(!NodeBehavior::Honest.is_byzantine());
        assert!(NodeBehavior::DropPackets { drop_rate: 0.5 }.is_byzantine());
        assert!(NodeBehavior::MaliciousRouting { malice_rate: 0.3 }.is_byzantine());
        assert!(NodeBehavior::DelayAttack { delay_ms: 1000 }.is_byzantine());
    }
}
