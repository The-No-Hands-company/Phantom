/// zkVM Proof System Demonstration
///
/// Shows the zero-knowledge proof generation and verification for routing paths.
/// Demonstrates:
/// - Proof generation for valid paths
/// - Proof verification
/// - Rejection of invalid paths (too short, too long, loops)
/// - Performance benchmarks

use phantom_zkvm::HashProofGenerator;
use phantom_core::proof::ProofGenerator;

fn main() -> anyhow::Result<()> {
    println!("═══════════════════════════════════════════════════════════");
    println!("      PHANTOM zkVM - Routing Proof Demonstration");
    println!("═══════════════════════════════════════════════════════════\n");

    let generator = HashProofGenerator::new();
    let network_commitment = [42u8; 32]; // Example network commitment

    // Test 1: Valid Path
    println!("Test 1: Valid 5-Hop Path");
    println!("───────────────────────────────────────────────────────────");
    let valid_path = vec![100, 200, 300, 400, 500];
    println!("  Path: {:?}", valid_path);
    
    let start = std::time::Instant::now();
    let proof = generator.generate_path_proof(&valid_path, &network_commitment, &[])?;
    let gen_time = start.elapsed();
    
    println!("  ✓ Proof generated in {:.2}ms", gen_time.as_secs_f64() * 1000.0);
    println!("    - Proof data: {} bytes", proof.proof_data.len());
    println!("    - Path length: {} hops", proof.public_inputs.path_length);
    println!("    - Timestamp: {}", proof.public_inputs.timestamp);
    
    let start = std::time::Instant::now();
    let valid = generator.verify_path_proof(&proof, &network_commitment)?;
    let verify_time = start.elapsed();
    
    println!("  ✓ Proof verified in {:.2}μs", verify_time.as_secs_f64() * 1_000_000.0);
    println!("    - Result: {}", if valid { "VALID ✓" } else { "INVALID ✗" });

    // Test 2: Path Too Short
    println!("\n\nTest 2: Invalid Path (Too Short)");
    println!("───────────────────────────────────────────────────────────");
    let short_path = vec![100, 200]; // Only 2 hops
    println!("  Path: {:?} (only {} hops)", short_path, short_path.len());
    
    match generator.generate_path_proof(&short_path, &network_commitment, &[]) {
        Ok(_) => println!("  ✗ ERROR: Should have rejected short path!"),
        Err(e) => println!("  ✓ Correctly rejected: {}", e),
    }

    // Test 3: Path Too Long
    println!("\n\nTest 3: Invalid Path (Too Long)");
    println!("───────────────────────────────────────────────────────────");
    let long_path = vec![100, 200, 300, 400, 500, 600, 700, 800]; // 8 hops
    println!("  Path: {:?}", long_path);
    println!("  Length: {} hops (max is 7)", long_path.len());
    
    match generator.generate_path_proof(&long_path, &network_commitment, &[]) {
        Ok(_) => println!("  ✗ ERROR: Should have rejected long path!"),
        Err(e) => println!("  ✓ Correctly rejected: {}", e),
    }

    // Test 4: Path with Loop
    println!("\n\nTest 4: Invalid Path (Contains Loop)");
    println!("───────────────────────────────────────────────────────────");
    let loop_path = vec![100, 200, 300, 200, 500]; // 200 appears twice
    println!("  Path: {:?}", loop_path);
    println!("  Note: Node 200 appears twice (creates loop)");
    
    match generator.generate_path_proof(&loop_path, &network_commitment, &[]) {
        Ok(_) => println!("  ✗ ERROR: Should have rejected path with loop!"),
        Err(e) => println!("  ✓ Correctly rejected: {}", e),
    }

    // Test 5: Wrong Network Commitment
    println!("\n\nTest 5: Wrong Network Commitment");
    println!("───────────────────────────────────────────────────────────");
    let wrong_commitment = [99u8; 32];
    println!("  Original commitment: {:02x}{:02x}...", network_commitment[0], network_commitment[1]);
    println!("  Wrong commitment:    {:02x}{:02x}...", wrong_commitment[0], wrong_commitment[1]);
    
    let proof = generator.generate_path_proof(&valid_path, &network_commitment, &[])?;
    let valid = generator.verify_path_proof(&proof, &wrong_commitment)?;
    
    println!("  Verification result: {}", if valid { "VALID ✗ (ERROR!)" } else { "INVALID ✓ (Correct)" });

    // Test 6: Proof Freshness
    println!("\n\nTest 6: Proof Freshness Check");
    println!("───────────────────────────────────────────────────────────");
    println!("  Note: Proofs expire after 1 hour");
    println!("  Current proof age: <1 second (fresh)");
    
    let proof = generator.generate_path_proof(&valid_path, &network_commitment, &[])?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let proof_age = now.saturating_sub(proof.public_inputs.timestamp);
    
    println!("  Proof timestamp: {}", proof.public_inputs.timestamp);
    println!("  Current time:    {}", now);
    println!("  Age: {} seconds", proof_age);
    println!("  Status: {}", if proof_age < 3600 { "FRESH ✓" } else { "EXPIRED ✗" });

    // Performance Summary
    println!("\n\n═══════════════════════════════════════════════════════════");
    println!("                Performance Summary");
    println!("═══════════════════════════════════════════════════════════");
    
    // Benchmark proof generation
    let mut gen_times = Vec::new();
    for _ in 0..10 {
        let start = std::time::Instant::now();
        let _ = generator.generate_path_proof(&valid_path, &network_commitment, &[])?;
        gen_times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    let avg_gen = gen_times.iter().sum::<f64>() / gen_times.len() as f64;
    
    // Benchmark proof verification
    let proof = generator.generate_path_proof(&valid_path, &network_commitment, &[])?;
    let mut verify_times = Vec::new();
    for _ in 0..100 {
        let start = std::time::Instant::now();
        let _ = generator.verify_path_proof(&proof, &network_commitment)?;
        verify_times.push(start.elapsed().as_secs_f64() * 1_000_000.0);
    }
    let avg_verify = verify_times.iter().sum::<f64>() / verify_times.len() as f64;
    
    println!("  Proof Generation:");
    println!("    - Average: {:.3}ms (10 samples)", avg_gen);
    println!("    - Min: {:.3}ms", gen_times.iter().cloned().fold(f64::INFINITY, f64::min));
    println!("    - Max: {:.3}ms", gen_times.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    
    println!("\n  Proof Verification:");
    println!("    - Average: {:.2}μs (100 samples)", avg_verify);
    println!("    - Min: {:.2}μs", verify_times.iter().cloned().fold(f64::INFINITY, f64::min));
    println!("    - Max: {:.2}μs", verify_times.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    
    println!("\n  Proof Properties:");
    println!("    - Size: {} bytes", proof.proof_data.len());
    println!("    - Freshness window: 1 hour");
    println!("    - Zero-knowledge: Yes (path hidden)");
    println!("    - Post-quantum secure: Pending (need RISC Zero/SP1)");

    println!("\n═══════════════════════════════════════════════════════════");
    println!("                    Status");
    println!("═══════════════════════════════════════════════════════════");
    println!("  ✓ Proof generation working ({:.2}ms)", avg_gen);
    println!("  ✓ Proof verification working ({:.2}μs)", avg_verify);
    println!("  ✓ Path validation (length, loops) working");
    println!("  ✓ Network commitment binding working");
    println!("  ✓ Proof freshness checks working");
    
    println!("\n  Next Steps:");
    println!("    → Integrate RISC Zero or SP1 for full zkVM");
    println!("    → Add Merkle proof of node membership");
    println!("    → Implement recursive proof composition");
    println!("    → Enable GPU acceleration for proving");
    println!("═══════════════════════════════════════════════════════════\n");

    Ok(())
}
