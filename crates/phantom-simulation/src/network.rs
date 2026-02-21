/// Large-scale network simulation orchestrator for PHANTOM
///
/// Manages the lifecycle of simulated network with hundreds or thousands of nodes:
/// - Network initialization and topology generation
/// - Packet injection and routing through multiple hops
/// - Byzantine node behavior simulation
/// - Metrics collection and analysis
/// - Stress testing under various conditions

use crate::node::{SimulatedNode, NodeBehavior, NodeStats};
use crate::metrics::{NetworkMetrics, SimulationReport};
use crate::topology::{TopologyGenerator, TopologyType};
use crate::byzantine::{ByzantineConfig, ByzantineAttack};

use phantom_core::{PhantomPacket, NetworkGraph};
use phantom_core::network::NodeInfo;
use phantom_crypto::FheEngine;
use anyhow::{Result, Context};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use rand::Rng;

/// Configuration for network simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// Number of nodes in the network
    pub num_nodes: usize,
    
    /// Ratio of Byzantine (malicious) nodes (0.0 to 1.0)
    pub byzantine_ratio: f64,
    
    /// Average network latency in milliseconds
    pub avg_latency_ms: u64,
    
    /// Standard deviation of latency (for realistic variation)
    pub latency_std_dev_ms: u64,
    
    /// Target packet injection rate (packets per second)
    pub packet_rate: usize,
    
    /// Network topology type
    pub topology: TopologyType,
    
    /// Byzantine attack configuration
    pub byzantine_config: ByzantineConfig,
    
    /// Simulation mode: reuse FHE keys for faster initialization
    /// WARNING: Only use for testing! Real networks need unique keys per node.
    pub simulation_mode: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            num_nodes: 100,
            byzantine_ratio: 0.1,  // 10% malicious
            avg_latency_ms: 50,
            latency_std_dev_ms: 20,
            packet_rate: 100,
            topology: TopologyType::Random,
            byzantine_config: ByzantineConfig::default(),
            simulation_mode: true,  // Enable by default for fast testing
        }
    }
}

/// Simulated PHANTOM network with multiple nodes
pub struct SimulatedNetwork {
    /// Configuration
    config: NetworkConfig,
    
    /// Simulated nodes (indexed by node ID)
    nodes: HashMap<u32, SimulatedNode>,
    
    /// Network topology graph
    network_graph: NetworkGraph,
    
    /// Simulation start time
    start_time: Option<Instant>,
    
    /// Total packets injected
    packets_sent: u64,
    
    /// Packets successfully delivered
    packets_delivered: u64,
    
    /// Packets lost/dropped
    packets_lost: u64,
    
    /// Per-packet latency measurements
    packet_latencies: Vec<Duration>,
}

impl SimulatedNetwork {
    /// Create a new simulated network with given configuration
    pub fn new(config: NetworkConfig) -> Result<Self> {
        tracing::info!("Creating simulated network with {} nodes ({:.1}% Byzantine)",
                      config.num_nodes, config.byzantine_ratio * 100.0);
        
                // Generate network topology
        let generator = TopologyGenerator::new(config.topology);
        let network_graph_inner = generator.generate(config.num_nodes)?;
        let network_graph = Arc::new(RwLock::new(network_graph_inner));
        
        let mut nodes = HashMap::new();
        let num_byzantine = (config.num_nodes as f64 * config.byzantine_ratio) as usize;
        
        tracing::info!("Initializing {} honest nodes and {} Byzantine nodes",
                      config.num_nodes - num_byzantine, num_byzantine);
        
        // Generate shared FHE keys once if in simulation mode (HUGE speedup)
        let shared_fhe_engine = if config.simulation_mode {
            tracing::info!("🚀 Simulation mode: Generating shared FHE keys (1x instead of {}x)", config.num_nodes);
            Some(FheEngine::generate_keys())
        } else {
            tracing::info!("Production mode: Generating unique FHE keys per node (slow)");
            None
        };
        
        for i in 0..config.num_nodes {
            let node_id = (i as u32) + 1;
            
            // Determine node behavior
            let behavior = if i < num_byzantine {
                // Byzantine node - assign attack type based on config
                config.byzantine_config.select_attack()
            } else {
                NodeBehavior::Honest
            };
            
            // Generate realistic latency with variation
            let latency = sample_latency(config.avg_latency_ms, config.latency_std_dev_ms);
            
            // Create node info
            let info = NodeInfo {
                id: node_id,
                bandwidth: 1_000_000_000,  // 1 Gbps
                latency_ms: latency as u32,
                uptime_hours: 720,  // 30 days
                reputation: if behavior == NodeBehavior::Honest { 0.95 } else { 0.3 },
            };
            
            // Create FHE engine for this node
            let fhe_engine = if let Some(ref shared_keys) = shared_fhe_engine {
                // Simulation mode: clone shared keys (instant)
                shared_keys.clone()
            } else {
                // Production mode: generate unique keys (~0.8s per node)
                FheEngine::generate_keys()
            };
            
            // Create simulated node
            let node = SimulatedNode::new(
                node_id,
                info.clone(),
                behavior,
                latency,
                fhe_engine,
                network_graph.clone(),
            )?;
            
            nodes.insert(node_id, node);
        }
        
        tracing::info!("Network initialization complete");
        
        Ok(Self {
            config,
            nodes,
            network_graph: Arc::try_unwrap(network_graph)
                .map(|rw| rw.into_inner().unwrap())
                .unwrap_or_else(|arc| arc.read().unwrap().clone()),
            start_time: None,
            packets_sent: 0,
            packets_delivered: 0,
            packets_lost: 0,
            packet_latencies: Vec::new(),
        })
    }
    
    /// Run simulation for specified duration
    pub fn run_simulation(&mut self, duration: Duration) -> Result<()> {
        tracing::info!("Starting simulation for {:?}", duration);
        self.start_time = Some(Instant::now());
        
        let target_packets = (duration.as_secs() as usize) * self.config.packet_rate;
        tracing::info!("Target: {} packets at {} packets/sec", target_packets, self.config.packet_rate);
        
        // Simulation loop
        let _interval = Duration::from_secs(1) / self.config.packet_rate as u32;
        
        for _ in 0..target_packets {
            // TODO: Implement packet injection and routing
            // For now, just simulate the structure
            
            if self.start_time.unwrap().elapsed() >= duration {
                break;
            }
        }
        
        tracing::info!("Simulation complete: {} packets sent, {} delivered, {} lost",
                      self.packets_sent, self.packets_delivered, self.packets_lost);
        
        Ok(())
    }
    
    /// Get current network metrics
    pub fn metrics(&self) -> NetworkMetrics {
        NetworkMetrics {
            num_nodes: self.config.num_nodes,
            num_byzantine: (self.config.num_nodes as f64 * self.config.byzantine_ratio) as usize,
            packets_sent: self.packets_sent,
            packets_delivered: self.packets_delivered,
            packets_lost: self.packets_lost,
            success_rate: if self.packets_sent > 0 {
                self.packets_delivered as f64 / self.packets_sent as f64
            } else {
                0.0
            },
            avg_latency: if !self.packet_latencies.is_empty() {
                self.packet_latencies.iter().sum::<Duration>() / self.packet_latencies.len() as u32
            } else {
                Duration::ZERO
            },
            median_latency: calculate_median_latency(&self.packet_latencies),
            p99_latency: calculate_percentile_latency(&self.packet_latencies, 0.99),
        }
    }
    
    /// Generate comprehensive simulation report
    pub fn report_metrics(&self) -> SimulationReport {
        let network_metrics = self.metrics();
        
        // Collect per-node statistics
        let node_stats: Vec<NodeStats> = self.nodes
            .values()
            .map(|node| node.stats())
            .collect();
        
        // Separate honest and Byzantine statistics
        let honest_stats: Vec<_> = node_stats.iter()
            .filter(|s| !s.behavior.is_byzantine())
            .cloned()
            .collect();
        
        let byzantine_stats: Vec<_> = node_stats.iter()
            .filter(|s| s.behavior.is_byzantine())
            .cloned()
            .collect();
        
        SimulationReport {
            config: self.config.clone(),
            network_metrics,
            node_stats,
            honest_nodes: honest_stats,
            byzantine_nodes: byzantine_stats,
            runtime: self.start_time.map(|t| t.elapsed()),
        }
    }
    
    /// Reset simulation for another run
    pub fn reset(&mut self) {
        self.start_time = None;
        self.packets_sent = 0;
        self.packets_delivered = 0;
        self.packets_lost = 0;
        self.packet_latencies.clear();
        
        for node in self.nodes.values_mut() {
            node.reset_stats();
        }
        
        tracing::info!("Simulation reset");
    }
}

/// Sample latency from normal distribution with given mean and std dev
fn sample_latency(mean_ms: u64, std_dev_ms: u64) -> u64 {
    let mut rng = rand::thread_rng();
    let sample: f64 = rng.gen_range(-2.0..2.0);  // Rough normal approximation
    let latency = (mean_ms as f64 + sample * std_dev_ms as f64).max(1.0) as u64;
    latency
}

/// Calculate median latency from samples
fn calculate_median_latency(latencies: &[Duration]) -> Duration {
    if latencies.is_empty() {
        return Duration::ZERO;
    }
    
    let mut sorted = latencies.to_vec();
    sorted.sort();
    sorted[sorted.len() / 2]
}

/// Calculate percentile latency (e.g., p99 = 0.99)
fn calculate_percentile_latency(latencies: &[Duration], percentile: f64) -> Duration {
    if latencies.is_empty() {
        return Duration::ZERO;
    }
    
    let mut sorted = latencies.to_vec();
    sorted.sort();
    let index = ((sorted.len() as f64 * percentile) as usize).min(sorted.len() - 1);
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_network_creation() {
        let config = NetworkConfig {
            num_nodes: 10,
            byzantine_ratio: 0.2,
            ..Default::default()
        };
        
        let network = SimulatedNetwork::new(config);
        assert!(network.is_ok());
        
        let network = network.unwrap();
        assert_eq!(network.nodes.len(), 10);
    }
    
    #[test]
    fn test_median_latency() {
        let latencies = vec![
            Duration::from_millis(10),
            Duration::from_millis(50),
            Duration::from_millis(30),
            Duration::from_millis(40),
            Duration::from_millis(20),
        ];
        
        let median = calculate_median_latency(&latencies);
        assert_eq!(median, Duration::from_millis(30));
    }
}
