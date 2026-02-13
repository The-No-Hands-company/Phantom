//! Performance benchmarks for the PHANTOM routing engine

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use phantom_routing::ObliviousForwarder;
use phantom_core::{PhantomPacket, NetworkGraph, packet::RoutingPath};
use phantom_core::network::NodeInfo;
use phantom_crypto::FheEngine;
use std::sync::{Arc, RwLock};

fn setup_network(node_count: usize) -> Arc<RwLock<NetworkGraph>> {
    let network = Arc::new(RwLock::new(NetworkGraph::new()));
    
    // Add nodes
    for id in 100..(100 + node_count as u32) {
        let mut net = network.write().unwrap();
        net.add_node(NodeInfo {
            id,
            bandwidth: 1_000_000_000,
            latency_ms: 50,
            uptime_hours: 720,
            reputation: 0.95,
        });
    }
    
    // Create mesh topology
    for from in 100..(100 + node_count as u32) {
        for to in (from + 1)..(100 + node_count as u32) {
            network.write().unwrap().add_edge(from, to);
        }
    }
    
    network
}

fn bench_packet_processing(c: &mut Criterion) {
    let mut group = c.benchmark_group("packet_processing");
    group.sample_size(10); // Small sample size - FHE is slow
    
    // Setup
    let fhe_engine = Arc::new(FheEngine::generate_keys());
    let network = setup_network(5);
    let forwarder = ObliviousForwarder::new(100, fhe_engine.clone(), network.clone());
    
    // Create test packet
    let path = RoutingPath::new(vec![100, 200, 300]).unwrap();
    let payload = vec![0u8; 1024]; // 1KB payload
    let commitment = network.read().unwrap().commitment().clone();
    
    let packet = PhantomPacket::construct(
        path,
        payload,
        &fhe_engine,
        &commitment,
    ).unwrap();
    
    group.bench_function("process_packet", |b| {
        b.iter(|| {
            forwarder.clear_nullifiers(); // Prevent replay detection in benchmark
            forwarder.process_packet(black_box(&packet)).unwrap()
        });
    });
    
    group.finish();
}

fn bench_routing_decision(c: &mut Criterion) {
    let mut group = c.benchmark_group("routing_decision");
    group.sample_size(10);
    
    let fhe_engine = Arc::new(FheEngine::generate_keys());
    let network = setup_network(5);
    
    for hop_count in [3, 5, 7] {
        let forwarder = ObliviousForwarder::new(100, fhe_engine.clone(), network.clone());
        
        let path_nodes: Vec<u32> = (100..(100 + hop_count as u32)).collect();
        let path = RoutingPath::new(path_nodes).unwrap();
        let payload = vec![0u8; 1024];
        let commitment = network.read().unwrap().commitment().clone();
        
        let packet = PhantomPacket::construct(
            path,
            payload,
            &fhe_engine,
            &commitment,
        ).unwrap();
        
        group.bench_with_input(
            BenchmarkId::from_parameter(hop_count),
            &packet,
            |b, pkt| {
                b.iter(|| {
                    forwarder.clear_nullifiers();
                    forwarder.process_packet(black_box(pkt)).unwrap()
                });
            },
        );
    }
    
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = bench_packet_processing, bench_routing_decision
}
criterion_main!(benches);
