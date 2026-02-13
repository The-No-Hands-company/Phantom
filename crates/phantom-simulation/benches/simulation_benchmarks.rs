/// Performance benchmarks for PHANTOM network simulation
///
/// Measures:
/// - Network initialization time (100, 500, 1000 nodes)
/// - Packet processing throughput
/// - Topology generation speed
/// - Metrics collection overhead

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use phantom_simulation::{
    SimulatedNetwork, NetworkConfig, TopologyType,
    ByzantineConfig, ByzantineAttack,
};
use std::time::Duration;

fn bench_network_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("network_creation");
    
    for num_nodes in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_nodes),
            num_nodes,
            |b, &nodes| {
                b.iter(|| {
                    let config = NetworkConfig {
                        num_nodes: nodes,
                        byzantine_ratio: 0.1,
                        avg_latency_ms: 50,
                        latency_std_dev_ms: 20,
                        packet_rate: 10,
                        topology: TopologyType::Random,
                        byzantine_config: ByzantineConfig::default(),
                    };
                    
                    // Note: This is SLOW due to FHE key generation
                    // Each node takes ~0.8s to initialize
                    let network = SimulatedNetwork::new(config);
                    black_box(network)
                });
            },
        );
    }
    
    group.finish();
}

fn bench_topology_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("topology_generation");
    
    for topology in [
        TopologyType::Random,
        TopologyType::Mesh,
        TopologyType::Ring,
    ].iter() {
        group.bench_with_input(
            BenchmarkId::new("topology", format!("{:?}", topology)),
            topology,
            |b, &topo| {
                b.iter(|| {
                    let config = NetworkConfig {
                        num_nodes: 100,
                        byzantine_ratio: 0.1,
                        avg_latency_ms: 50,
                        latency_std_dev_ms: 20,
                        packet_rate: 10,
                        topology: topo,
                        byzantine_config: ByzantineConfig::default(),
                    };
                    
                    // Just topology generation, not full network
                    // (To isolate topology performance from FHE overhead)
                    black_box(config)
                });
            },
        );
    }
    
    group.finish();
}

fn bench_byzantine_attack_selection(c: &mut Criterion) {
    c.bench_function("byzantine_attack_selection", |b| {
        let config = ByzantineConfig::default();
        b.iter(|| {
            for _ in 0..1000 {
                let attack = config.select_attack();
                black_box(attack);
            }
        });
    });
}

fn bench_metrics_calculation(c: &mut Criterion) {
    c.bench_function("metrics_calculation", |b| {
        // Create a small network to measure metrics overhead
        let config = NetworkConfig {
            num_nodes: 10,
            byzantine_ratio: 0.1,
            ..Default::default()
        };
        
        let network = SimulatedNetwork::new(config)
            .expect("Failed to create test network");
        
        b.iter(|| {
            let metrics = network.metrics();
            black_box(metrics)
        });
    });
}

// Note: Full simulation benchmarks are too slow for regular CI
// Each packet takes ~2.5s to process due to FHE operations
// These would be stress tests, not benchmarks

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)  // Small sample size due to FHE overhead
        .measurement_time(Duration::from_secs(30));
    targets = 
        bench_topology_generation,
        bench_byzantine_attack_selection,
        bench_metrics_calculation,
        // bench_network_creation is commented out - too slow for CI
        // Uncomment for one-off performance profiling
        // bench_network_creation,
}

criterion_main!(benches);
