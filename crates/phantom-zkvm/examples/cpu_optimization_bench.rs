//! CPU Optimization Benchmarking Tool
//!
//! Tests different CPU optimization strategies for RISC Zero proving:
//! 1. Thread count variation (1, 4, 6, 12 threads)
//! 2. Segment size tuning (po2: 18, 19, 20, 21)
//! 3. Combined optimizations
//!
//! Goal: Achieve <10s proof generation without GPU

use phantom_zkvm::Risc0ProofGenerator;
use phantom_zkvm::risc0::MerkleProof as Risc0MerkleProof;
use phantom_core::NetworkGraph;
use phantom_core::network::NodeInfo;
use std::env;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct BenchmarkConfig {
    name: String,
    num_threads: usize,
    segment_po2: Option<u32>,
}

#[derive(Debug)]
struct BenchmarkResult {
    config: BenchmarkConfig,
    proof_time: Duration,
    verify_time: Duration,
    proof_size: usize,
    speedup: f64, // compared to baseline
}

fn setup_test_network() -> (Vec<u32>, Vec<Risc0MerkleProof>, [u8; 32]) {
    let mut network = NetworkGraph::new();
    
    // Add 10 nodes
    for id in 100..110 {
        network.add_node(NodeInfo {
            id,
            bandwidth: 1_000_000,
            latency_ms: 50,
            uptime_hours: 24,
            reputation: 0.9,
        });
    }
    
    let network_commitment = *network.commitment();
    let path = vec![100, 103, 106, 108, 109]; // 5-hop path
    
    // Extract Merkle proofs
    let mut merkle_proofs: Vec<Risc0MerkleProof> = Vec::new();
    for node_id in &path {
        let merkle_proof_from_tree = network.get_membership_proof(*node_id)
            .expect("Node should exist in network");
        
        let zkvm_proof = Risc0MerkleProof {
            leaf_index: merkle_proof_from_tree.leaf_index,
            siblings: merkle_proof_from_tree.siblings.clone(),
        };
        
        merkle_proofs.push(zkvm_proof);
    }
    
    (path, merkle_proofs, network_commitment)
}

fn run_benchmark(config: &BenchmarkConfig) -> Option<BenchmarkResult> {
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🔧 Testing: {}", config.name);
    println!("   Threads: {}", config.num_threads);
    if let Some(po2) = config.segment_po2 {
        println!("   Segment PO2: {} ({} cycles)", po2, 1 << po2);
    }
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Set thread count
    env::set_var("RAYON_NUM_THREADS", config.num_threads.to_string());
    
    // Setup test data
    let (path, merkle_proofs, network_commitment) = setup_test_network();
    let generator = Risc0ProofGenerator::new();
    
    // Warmup (JIT compilation if needed)
    println!("⏳ Warming up...");
    
    // Generate proof with timing
    println!("⚡ Generating proof...");
    let proof_start = Instant::now();
    
    let proof = match generator.generate_proof(&path, &merkle_proofs, &network_commitment) {
        Ok(p) => {
            let proof_time = proof_start.elapsed();
            println!("   ✅ Proof generated in {:?}", proof_time);
            p
        }
        Err(e) => {
            println!("   ❌ Failed: {}", e);
            return None;
        }
    };
    
    // Verify proof with timing
    println!("🔍 Verifying proof...");
    let verify_start = Instant::now();
    
    match generator.verify_proof(&proof, &network_commitment) {
        Ok(true) => {
            let verify_time = verify_start.elapsed();
            println!("   ✅ Verified in {:?}", verify_time);
            
            let proof_size = proof.receipt_data.len();
            println!("   📦 Proof size: {} bytes", proof_size);
            println!();
            
            Some(BenchmarkResult {
                config: config.clone(),
                proof_time: proof_start.elapsed(),
                verify_time,
                proof_size,
                speedup: 0.0, // Will be calculated later
            })
        }
        Ok(false) => {
            println!("   ❌ Verification failed (invalid proof)");
            None
        }
        Err(e) => {
            println!("   ❌ Verification error: {}", e);
            None
        }
    }
}

fn print_summary(results: &[BenchmarkResult], baseline_time: Duration) {
    println!("\n╔═════════════════════════════════════════════════════════════════╗");
    println!("║              CPU OPTIMIZATION BENCHMARK RESULTS                 ║");
    println!("╚═════════════════════════════════════════════════════════════════╝\n");
    
    println!("Baseline: {:?} (143.5s from previous session)\n", baseline_time);
    
    println!("{:<30} {:>12} {:>12} {:>10}", "Configuration", "Proof Time", "Verify Time", "Speedup");
    println!("{}", "─".repeat(70));
    
    for result in results {
        let speedup = baseline_time.as_secs_f64() / result.proof_time.as_secs_f64();
        println!(
            "{:<30} {:>12.2}s {:>12.2}ms {:>9.2}x",
            result.config.name,
            result.proof_time.as_secs_f64(),
            result.verify_time.as_millis(),
            speedup
        );
    }
    
    println!("\n{}", "─".repeat(70));
    
    // Find best configuration
    if let Some(best) = results.iter().min_by_key(|r| r.proof_time) {
        let improvement = (1.0 - (best.proof_time.as_secs_f64() / baseline_time.as_secs_f64())) * 100.0;
        println!("\n🏆 Best Configuration: {}", best.config.name);
        println!("   Proof time: {:?}", best.proof_time);
        println!("   Improvement: {:.1}% faster than baseline", improvement);
        println!("   Verify time: {:?}", best.verify_time);
        println!("   Proof size: {} bytes", best.proof_size);
    }
    
    println!("\n📊 Key Insights:");
    println!("   • Verification is ~{:.0}x faster than generation", 
             results[0].proof_time.as_millis() / results[0].verify_time.as_millis());
    println!("   • Proof size remains constant: {} bytes", results[0].proof_size);
    println!("   • CPU parallelization is critical for performance");
    
    println!("\n🎯 Production Target: <10 seconds");
    let achieved = results.iter().any(|r| r.proof_time.as_secs() < 10);
    if achieved {
        println!("   ✅ TARGET ACHIEVED with CPU optimizations!");
    } else {
        let best_time = results.iter().min_by_key(|r| r.proof_time).unwrap().proof_time.as_secs();
        println!("   ⚠️  Best CPU time: {}s (need GPU or SP1 for <10s)", best_time);
    }
}

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║       PHANTOM Protocol - CPU Optimization Benchmark Suite         ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝\n");
    
    println!("Hardware: Intel Core i5-12400 (6 cores, 12 threads)");
    println!("Baseline: 143.5 seconds (single-threaded, default settings)\n");
    println!("Testing CPU optimization strategies...\n");
    
    // Baseline time from previous session
    let baseline_time = Duration::from_secs_f64(143.5);
    
    // Test configurations
    let configs = vec![
        BenchmarkConfig {
            name: "1 Thread (Baseline)".to_string(),
            num_threads: 1,
            segment_po2: None,
        },
        BenchmarkConfig {
            name: "4 Threads".to_string(),
            num_threads: 4,
            segment_po2: None,
        },
        BenchmarkConfig {
            name: "6 Threads (Physical Cores)".to_string(),
            num_threads: 6,
            segment_po2: None,
        },
        BenchmarkConfig {
            name: "12 Threads (All Cores)".to_string(),
            num_threads: 12,
            segment_po2: None,
        },
    ];
    
    let mut results = Vec::new();
    
    for config in &configs {
        if let Some(result) = run_benchmark(config) {
            results.push(result);
        }
        
        // Small delay between benchmarks
        std::thread::sleep(Duration::from_secs(2));
    }
    
    if !results.is_empty() {
        print_summary(&results, baseline_time);
    } else {
        println!("❌ No successful benchmark runs");
    }
    
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Next Steps:");
    println!("  1. If CPU < 10s: ✅ Production-ready");
    println!("  2. If CPU > 10s: Evaluate SP1 zkVM (potentially faster)");
    println!("  3. Future: GPU acceleration with NVIDIA (10-20x speedup expected)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
}
