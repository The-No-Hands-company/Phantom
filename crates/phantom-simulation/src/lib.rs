/// PHANTOM Protocol - Network Simulation Framework
///
/// Provides comprehensive network simulation capabilities for testing PHANTOM at scale:
/// - Simulated nodes with realistic latency and bandwidth constraints
/// - Byzantine node behavior (malicious routing, packet dropping)
/// - Network topology generation (power-law, clustered, random)
/// - Comprehensive metrics collection and analysis
/// - Stress testing infrastructure for 100-1000+ node networks
///
/// # Architecture
///
/// The simulation framework is designed to test PHANTOM's core properties at scale:
/// 1. **Oblivious routing under load** - Can FHE handle 1000s of packets?
/// 2. **Byzantine resistance** - Does the protocol degrade gracefully?
/// 3. **Network congestion** - What happens under high packet rates?
/// 4. **Latency distribution** - Real-world performance characteristics
///
/// # Example Usage
///
/// ```no_run
/// use phantom_simulation::{SimulatedNetwork, NetworkConfig};
/// use std::time::Duration;
///
/// # fn main() -> anyhow::Result<()> {
/// // NetworkConfig has eight fields, not four — topology, byzantine_config,
/// // latency_std_dev_ms and simulation_mode were missing from the struct
/// // literal this example used to build, so it could not compile. Default
/// // fills them in, which is also what a caller usually wants.
/// let config = NetworkConfig {
///     num_nodes: 100,
///     byzantine_ratio: 0.1,  // 10% malicious
///     ..Default::default()
/// };
///
/// let mut network = SimulatedNetwork::new(config)?;
/// network.run_simulation(Duration::from_secs(60))?;
/// let metrics = network.report_metrics();
///
/// println!("Packet success rate: {:.2}%", metrics.network_metrics.success_rate * 100.0);
/// # Ok(())
/// # }
/// ```
///
/// Marked `no_run`: it compiles as part of the test suite but is not executed,
/// because a sixty-second simulation of a hundred nodes does not belong in a
/// doctest. The previous version was neither compiled nor run — it omitted
/// `use std::time::Duration` and used `?` outside any function.

pub mod node;
pub mod network;
pub mod metrics;
pub mod topology;
pub mod byzantine;

pub use node::{SimulatedNode, NodeBehavior};
pub use network::{SimulatedNetwork, NetworkConfig};
pub use metrics::{NetworkMetrics, SimulationReport};
pub use topology::{TopologyGenerator, TopologyType};
pub use byzantine::{ByzantineConfig, ByzantineAttack};

/// Re-exports from core PHANTOM crates for convenience
pub use phantom_core::{PhantomPacket, NetworkGraph};
pub use phantom_core::network::NodeInfo;
pub use phantom_routing::{ObliviousForwarder, RoutingDecision};
pub use phantom_crypto::FheEngine;

/// Simulation version and build information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        assert!(!VERSION.is_empty());
    }
}
