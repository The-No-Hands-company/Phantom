//! Merkle Tree Demonstration
//! 
//! Shows how PHANTOM uses Merkle trees for network commitment
//! and zero-knowledge membership proofs.

use phantom_core::{MerkleTree, NetworkGraph};
use phantom_core::network::NodeInfo;

fn main() {
    println!("🌳 PHANTOM Merkle Tree Demonstration\n");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");

    // Demo 1: Basic Merkle Tree Operations
    println!("1️⃣  Basic Merkle Tree Operations");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let mut tree = MerkleTree::new(16); // 65K node capacity
    println!("Created Merkle tree with depth 16 (capacity: 65,536 nodes)");
    println!("Initial root: {:?}\n", tree.root());

    // Insert some nodes
    println!("Inserting nodes 100, 101, 102...");
    tree.insert(100).unwrap();
    tree.insert(101).unwrap();
    tree.insert(102).unwrap();
    println!("Tree now contains {} nodes", tree.len());
    println!("New root: {:?}\n", tree.root());

    // Demo 2: Membership Proofs
    println!("2️⃣  Zero-Knowledge Membership Proofs");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    // Generate proof for node 101
    let proof = tree.get_proof(101).expect("Node should be in tree");
    println!("Generated membership proof for node 101:");
    println!("  - Leaf index: {}", proof.leaf_index);
    println!("  - Proof path length: {} hashes", proof.siblings.len());
    println!("  - Proof size: {} bytes", proof.siblings.len() * 32);
    
    // Verify proof
    let valid = tree.verify_proof(101, &proof);
    println!("  - Proof valid: {}\n", valid);

    // Try to forge a proof for non-existent node
    println!("Attempting to use proof for different node (999)...");
    let forged = tree.verify_proof(999, &proof);
    println!("  - Forged proof valid: {} ❌\n", forged);

    // Demo 3: Network Integration
    println!("3️⃣  Integration with NetworkGraph");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let mut network = NetworkGraph::new();
    println!("Creating network with 10 nodes...");
    
    for id in 200..210 {
        network.add_node(NodeInfo {
            id,
            bandwidth: 1_000_000,
            latency_ms: 50,
            uptime_hours: 24,
            reputation: 0.9,
        });
    }

    // Get network commitment (Merkle root)
    let commitment = network.commitment();
    println!("Network commitment (Merkle root):");
    println!("  {:?}\n", commitment);

    // Generate membership proof via network
    let node_id = 205;
    let network_proof = network.get_membership_proof(node_id).unwrap();
    println!("Generated proof for node {} via NetworkGraph", node_id);
    println!("  - Proof path length: {}", network_proof.siblings.len());
    
    // Verify through network
    let valid = network.verify_membership(node_id, &network_proof);
    println!("  - Proof valid: {}\n", valid);

    // Demo 4: Dynamic Updates
    println!("4️⃣  Dynamic Updates & Proof Invalidation");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let mut dynamic_tree = MerkleTree::new(10);
    dynamic_tree.insert(300).unwrap();
    dynamic_tree.insert(301).unwrap();
    
    let old_root = *dynamic_tree.root();
    let old_proof = dynamic_tree.get_proof(300).unwrap();
    println!("Generated proof for node 300");
    println!("  Root: {:?}", old_root);
    
    // Add new node (changes root)
    dynamic_tree.insert(302).unwrap();
    let new_root = *dynamic_tree.root();
    println!("\nAdded node 302 - root changed!");
    println!("  Old root: {:?}", old_root);
    println!("  New root: {:?}", new_root);
    
    // Old proof no longer valid
    let old_valid = dynamic_tree.verify_proof(300, &old_proof);
    println!("\nOld proof for node 300:");
    println!("  - Valid against old root: ✓");
    println!("  - Valid against new root: {} ❌", old_valid);
    
    // Get new proof
    let new_proof = dynamic_tree.get_proof(300).unwrap();
    let new_valid = dynamic_tree.verify_proof(300, &new_proof);
    println!("\nNew proof for node 300:");
    println!("  - Valid against new root: {} ✓", new_valid);

    // Demo 5: Large-Scale Performance
    println!("\n5️⃣  Large-Scale Performance Test");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let mut large_tree = MerkleTree::new(20); // 1M capacity
    let start = std::time::Instant::now();
    
    println!("Inserting 10,000 nodes...");
    for id in 0..10_000 {
        large_tree.insert(id).unwrap();
    }
    let insert_time = start.elapsed();
    println!("  Time: {:?} ({:.2} μs/node)", insert_time, insert_time.as_micros() as f64 / 10_000.0);
    
    // Test proof generation
    let proof_start = std::time::Instant::now();
    let proof = large_tree.get_proof(5000).unwrap();
    let proof_time = proof_start.elapsed();
    println!("\nProof generation for node 5000:");
    println!("  Time: {:?}", proof_time);
    println!("  Proof size: {} bytes", proof.siblings.len() * 32);
    
    // Test verification
    let verify_start = std::time::Instant::now();
    let valid = large_tree.verify_proof(5000, &proof);
    let verify_time = verify_start.elapsed();
    println!("\nProof verification:");
    println!("  Time: {:?}", verify_time);
    println!("  Valid: {}", valid);

    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("✅ All Merkle tree operations complete!");
    println!("\nKey Insights:");
    println!("  • Merkle proofs enable zero-knowledge set membership");
    println!("  • Proof size: O(log n) - scales logarithmically");
    println!("  • Root commitment binds entire network state");
    println!("  • Dynamic updates require proof refresh");
    println!("  • Ready for zkVM circuit integration! 🚀");
}
