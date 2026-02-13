//! Example: Basic cryptographic primitives demo

use phantom_crypto::{
    pq::{KeyPair, SigningKeyPair},
    fhe::FheEngine,
};

fn main() {
    println!("🔮 PHANTOM Cryptographic Primitives Demo\n");
    
    // 1. Post-Quantum Key Exchange
    println!("1️⃣  Post-Quantum Key Exchange (Kyber-1024)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let keypair = KeyPair::generate();
    println!("✓ Generated key pair");
    
    let (ciphertext, ss1) = KeyPair::encapsulate(&keypair.public).unwrap();
    println!("✓ Encapsulated shared secret");
    
    let ss2 = keypair.decapsulate(&ciphertext).unwrap();
    println!("✓ Decapsulated shared secret");
    
    assert_eq!(ss1.0, ss2.0);
    println!("✓ Shared secrets match!\n");
    
    // 2. Digital Signatures
    println!("2️⃣  Post-Quantum Signatures (Dilithium-5)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let signing_keypair = SigningKeyPair::generate();
    let message = b"PHANTOM: Surveillance is mathematically impossible";
    
    let signature = signing_keypair.sign(message).unwrap();
    println!("✓ Signed message");
    
    let valid = SigningKeyPair::verify(&signing_keypair.public, message, &signature).unwrap();
    assert!(valid);
    println!("✓ Signature verified!\n");
    
    // 3. Fully Homomorphic Encryption
    println!("3️⃣  Fully Homomorphic Encryption (TFHE)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("⏳ Generating FHE keys (this takes ~2 seconds)...");
    
    let fhe_engine = FheEngine::generate_keys();
    println!("✓ FHE keys generated");
    
    let secret_value = 42u8;
    let encrypted = fhe_engine.encrypt_u8(secret_value);
    println!("✓ Encrypted value: {}", secret_value);
    
    let decrypted = fhe_engine.decrypt_u8(&encrypted).unwrap();
    assert_eq!(secret_value, decrypted);
    println!("✓ Decrypted value: {}", decrypted);
    println!("✓ Values match!\n");
    
    // 4. Oblivious Routing Demo (simplified)
    println!("4️⃣  Oblivious Routing Table Lookup (Simplified)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Creating encrypted routing table:");
    println!("  Node 100 → 200");
    println!("  Node 101 → 201");
    println!("  Node 102 → 202");
    
    let routing_table = vec![
        (fhe_engine.encrypt_u32(100), fhe_engine.encrypt_u32(200)),
        (fhe_engine.encrypt_u32(101), fhe_engine.encrypt_u32(201)),
        (fhe_engine.encrypt_u32(102), fhe_engine.encrypt_u32(202)),
    ];
    println!("✓ Routing table encrypted");
    
    println!("\n⏳ Performing oblivious lookup (FHE evaluation ~200ms)...");
    println!("Query: I am node 101, what's my next hop?");
    
    let result = fhe_engine.lookup_routing_table(101, &routing_table).unwrap();
    let next_hop = fhe_engine.decrypt_u32(&result).unwrap();
    
    println!("✓ Next hop: {}", next_hop);
    assert_eq!(next_hop, 201);
    println!("✓ Correct routing decision made WITHOUT learning the path!\n");
    
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("🎉 All cryptographic primitives working!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\nThis demonstrates the foundation for:");
    println!("  • Quantum-resistant key exchange");
    println!("  • Unforgeable digital signatures");
    println!("  • Computation on encrypted data");
    println!("  • Routing without metadata leakage");
    println!("\nNext: Build the full protocol on top of these primitives! 🚀");
}
