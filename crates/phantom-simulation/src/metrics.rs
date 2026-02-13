/// Metrics collection and reporting for network simulations
///
/// Provides comprehensive performance and security metrics for PHANTOM simulations:
/// - Network-wide statistics (throughput, latency, success rate)
/// - Per-node statistics (processing time, drop rate)
/// - Byzantine behavior analysis
/// - Performance profiling and bottleneck identification

use crate::node::NodeStats;
use crate::network::NetworkConfig;
use serde::{Serialize, Deserialize};
use std::time::Duration;

/// Network-wide performance metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    /// Total number of nodes
    pub num_nodes: usize,
    
    /// Number of Byzantine (malicious) nodes
    pub num_byzantine: usize,
    
    /// Total packets sent into the network
    pub packets_sent: u64,
    
    /// Packets successfully delivered to destination
    pub packets_delivered: u64,
    
    /// Packets lost/dropped by network
    pub packets_lost: u64,
    
    /// Success rate (delivered / sent)
    pub success_rate: f64,
    
    /// Average end-to-end latency
    pub avg_latency: Duration,
    
    /// Median latency (50th percentile)
    pub median_latency: Duration,
    
    /// 99th percentile latency
    pub p99_latency: Duration,
}

impl NetworkMetrics {
    /// Print formatted metrics report
    pub fn print(&self) {
        println!("\n═══════════════════════════════════════════════════════════");
        println!("       NETWORK METRICS");
        println!("═══════════════════════════════════════════════════════════");
        println!("Network Size: {} nodes ({} Byzantine, {:.1}%)",
                 self.num_nodes,
                 self.num_byzantine,
                 (self.num_byzantine as f64 / self.num_nodes as f64) * 100.0);
        println!();
        println!("Throughput:");
        println!("  Packets sent: {}", self.packets_sent);
        println!("  Packets delivered: {}", self.packets_delivered);
        println!("  Packets lost: {}", self.packets_lost);
        println!("  Success rate: {:.2}%", self.success_rate * 100.0);
        println!();
        println!("Latency:");
        println!("  Average: {:?}", self.avg_latency);
        println!("  Median (p50): {:?}", self.median_latency);
        println!("  p99: {:?}", self.p99_latency);
        println!("═══════════════════════════════════════════════════════════\n");
    }
    
    /// Check if metrics meet Phase 3 success criteria
    pub fn check_success_criteria(&self) -> PhaseThreeResult {
        let mut passed = Vec::new();
        let mut failed = Vec::new();
        
        // Success rate >= 90%
        if self.success_rate >= 0.90 {
            passed.push(format!("✓ Success rate: {:.2}% (>= 90%)", self.success_rate * 100.0));
        } else {
            failed.push(format!("✗ Success rate: {:.2}% (< 90%)", self.success_rate * 100.0));
        }
        
        // Average latency < 10s for 5-hop routes
        if self.avg_latency < Duration::from_secs(10) {
            passed.push(format!("✓ Avg latency: {:?} (< 10s)", self.avg_latency));
        } else {
            failed.push(format!("✗ Avg latency: {:?} (>= 10s)", self.avg_latency));
        }
        
        // p99 latency < 20s
        if self.p99_latency < Duration::from_secs(20) {
            passed.push(format!("✓ p99 latency: {:?} (< 20s)", self.p99_latency));
        } else {
            failed.push(format!("✗ p99 latency: {:?} (>= 20s)", self.p99_latency));
        }
        
        PhaseThreeResult {
            passed,
            success: failed.is_empty(),
            failed,
        }
    }
}

/// Comprehensive simulation report with all metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationReport {
    /// Simulation configuration
    pub config: NetworkConfig,
    
    /// Network-wide metrics
    pub network_metrics: NetworkMetrics,
    
    /// Per-node statistics (all nodes)
    pub node_stats: Vec<NodeStats>,
    
    /// Statistics for honest nodes only
    pub honest_nodes: Vec<NodeStats>,
    
    /// Statistics for Byzantine nodes only
    pub byzantine_nodes: Vec<NodeStats>,
    
    /// Total simulation runtime
    pub runtime: Option<Duration>,
}

impl SimulationReport {
    /// Generate comprehensive report
    pub fn print_full_report(&self) {
        println!("\n");
        println!("╔═══════════════════════════════════════════════════════════════╗");
        println!("║         PHANTOM NETWORK SIMULATION - FULL REPORT             ║");
        println!("╚═══════════════════════════════════════════════════════════════╝\n");
        
        // Configuration
        println!("CONFIGURATION:");
        println!("  Network size: {} nodes", self.config.num_nodes);
        println!("  Byzantine ratio: {:.1}%", self.config.byzantine_ratio * 100.0);
        println!("  Average latency: {} ms", self.config.avg_latency_ms);
        println!("  Packet rate: {} packets/sec", self.config.packet_rate);
        println!("  Topology: {:?}", self.config.topology);
        if let Some(runtime) = self.runtime {
            println!("  Runtime: {:?}", runtime);
        }
        println!();
        
        // Network metrics
        self.network_metrics.print();
        
        // Honest node analysis
        if !self.honest_nodes.is_empty() {
            println!("HONEST NODES ANALYSIS:");
            let total_processed: u64 = self.honest_nodes.iter().map(|s| s.packets_processed).sum();
            let total_dropped: u64 = self.honest_nodes.iter().map(|s| s.packets_dropped).sum();
            let avg_drop_rate = total_dropped as f64 / (total_processed + total_dropped) as f64;
            
            println!("  Count: {}", self.honest_nodes.len());
            println!("  Total processed: {}", total_processed);
            println!("  Total dropped: {}", total_dropped);
            println!("  Avg drop rate: {:.2}%", avg_drop_rate * 100.0);
            println!();
        }
        
        // Byzantine node analysis
        if !self.byzantine_nodes.is_empty() {
            println!("BYZANTINE NODES ANALYSIS:");
            let total_processed: u64 = self.byzantine_nodes.iter().map(|s| s.packets_processed).sum();
            let total_dropped: u64 = self.byzantine_nodes.iter().map(|s| s.packets_dropped).sum();
            let avg_drop_rate = total_dropped as f64 / (total_processed + total_dropped) as f64;
            
            println!("  Count: {}", self.byzantine_nodes.len());
            println!("  Total processed: {}", total_processed);
            println!("  Total dropped: {}", total_dropped);
            println!("  Avg drop rate: {:.2}%", avg_drop_rate * 100.0);
            println!();
        }
        
        // Success criteria check
        let result = self.network_metrics.check_success_criteria();
        result.print();
        
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║                     END OF REPORT                             ║");
        println!("╚═══════════════════════════════════════════════════════════════╝\n");
    }
    
    /// Export report to JSON file
    pub fn export_json(&self, path: &str) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        tracing::info!("Report exported to {}", path);
        Ok(())
    }
}

/// Phase 3 success criteria evaluation result
#[derive(Debug)]
pub struct PhaseThreeResult {
    pub passed: Vec<String>,
    pub failed: Vec<String>,
    pub success: bool,
}

impl PhaseThreeResult {
    pub fn print(&self) {
        println!("PHASE 3 SUCCESS CRITERIA:");
        println!("───────────────────────────────────────────────────────────");
        
        for item in &self.passed {
            println!("  {}", item);
        }
        
        for item in &self.failed {
            println!("  {}", item);
        }
        
        println!();
        if self.success {
            println!("  ✅ ALL CRITERIA PASSED");
        } else {
            println!("  ❌ SOME CRITERIA FAILED");
        }
        println!("───────────────────────────────────────────────────────────");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_success_criteria_all_pass() {
        let metrics = NetworkMetrics {
            num_nodes: 100,
            num_byzantine: 10,
            packets_sent: 1000,
            packets_delivered: 950,
            packets_lost: 50,
            success_rate: 0.95,
            avg_latency: Duration::from_secs(5),
            median_latency: Duration::from_secs(4),
            p99_latency: Duration::from_secs(12),
        };
        
        let result = metrics.check_success_criteria();
        assert!(result.success);
    }
    
    #[test]
    fn test_success_criteria_failure() {
        let metrics = NetworkMetrics {
            num_nodes: 100,
            num_byzantine: 10,
            packets_sent: 1000,
            packets_delivered: 800,  // Only 80% success
            packets_lost: 200,
            success_rate: 0.80,
            avg_latency: Duration::from_secs(15),  // Too slow
            median_latency: Duration::from_secs(14),
            p99_latency: Duration::from_secs(25),  // Too slow
        };
        
        let result = metrics.check_success_criteria();
        assert!(!result.success);
        assert_eq!(result.failed.len(), 3);  // All 3 criteria failed
    }
}
