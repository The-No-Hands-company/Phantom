/// PHANTOM Network Simulation - Large-Scale Testing Example
///
/// Demonstrates the network simulation framework for Phase 3 testing:
/// - 100-node network with 10% Byzantine nodes
/// - Realistic latency and packet loss
/// - Comprehensive metrics collection
/// - Success criteria evaluation
///
/// Usage:
///   cargo run --package phantom-simulation --example network_simulation --release
///
/// Options (edit code to configure):
///   - num_nodes: 10, 100, 500, 1000
///   - byzantine_ratio: 0.1 (10%), 0.3 (30%), 0.5 (50%)
///   - topology: Random, Mesh, Ring, SmallWorld, ScaleFree

use phantom_simulation::{
    SimulatedNetwork, NetworkConfig, TopologyType,
    ByzantineConfig, ByzantineAttack,
};
use std::time::Duration;
use anyhow::Result;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║    PHANTOM NETWORK SIMULATION - PHASE 3 STRESS TEST          ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    // Configuration
    let config = NetworkConfig {
        num_nodes: 100,  // Start with 100 nodes (can scale to 1000+)
        byzantine_ratio: 0.1,  // 10% malicious nodes
        avg_latency_ms: 50,
        latency_std_dev_ms: 20,
        packet_rate: 10,  // 10 packets/sec (conservative start)
        topology: TopologyType::Random,
        byzantine_config: ByzantineConfig::for_attack(
            ByzantineAttack::Mixed  // Mix of attack types
        ),
        simulation_mode: true,  // Reuse FHE keys for fast initialization
    };

    println!("Configuration:");
    println!("  Network size: {} nodes", config.num_nodes);
    println!("  Byzantine ratio: {:.1}%", config.byzantine_ratio * 100.0);
    println!("  Average latency: {} ms", config.avg_latency_ms);
    println!("  Packet rate: {} packets/sec", config.packet_rate);
    println!("  Topology: {:?}", config.topology);
    println!("  Byzantine attacks: Mixed (drop, malicious routing, delay, forge)");
    println!("  Simulation mode: {} (FHE key reuse)", config.simulation_mode);
    if config.simulation_mode {
        println!("    💡 Speedup: ~{}x faster initialization ({:.1}s → {:.1}s estimated)",
                 config.num_nodes, 
                 config.num_nodes as f64 * 0.8,
                 0.8);
    }
    println!();

    // Create network
    println!("Phase 1: Network Initialization");
    println!("───────────────────────────────────────────────────────────────");
    let start = std::time::Instant::now();
    let mut network = SimulatedNetwork::new(config)?;
    println!("✓ Network created in {:?}", start.elapsed());
    println!();

    // Run simulation
    println!("Phase 2: Simulation Execution");
    println!("───────────────────────────────────────────────────────────────");
    let simulation_duration = Duration::from_secs(60);  // 1 minute test
    println!("Running simulation for {:?}...", simulation_duration);
    
    let sim_start = std::time::Instant::now();
    network.run_simulation(simulation_duration)?;
    let sim_elapsed = sim_start.elapsed();
    
    println!("✓ Simulation complete in {:?}", sim_elapsed);
    println!();

    // Generate report
    println!("Phase 3: Metrics Analysis");
    println!("───────────────────────────────────────────────────────────────");
    let report = network.report_metrics();
    report.print_full_report();

    // Export results
    let report_path = "simulation_report.json";
    report.export_json(report_path)?;
    println!("Report exported to: {}", report_path);
    println!();

    // Summary
    println!("SIMULATION SUMMARY:");
    println!("  Total runtime: {:?}", sim_elapsed);
    let metrics = report.network_metrics;
    println!("  Success rate: {:.2}%", metrics.success_rate * 100.0);
    println!("  Average latency: {:?}", metrics.avg_latency);
    println!();

    if metrics.success_rate >= 0.90 {
        println!("✅ SUCCESS: Network performed within Phase 3 targets!");
    } else {
        println!("⚠️  WARNING: Success rate below 90% target");
        println!("   This indicates network stress or excessive Byzantine behavior");
    }

    Ok(())
}
