# Merkle Membership Circuit for PHANTOM

**Date**: November 21, 2025  
**Purpose**: Zero-knowledge proofs that node ∈ network without revealing node identity

## Overview

PHANTOM nodes must prove they're part of the network without revealing their identity. This prevents:
- **DHT topology leaks**: Traditional anonymity networks reveal network structure
- **Sybil attacks**: Fake nodes can't join without valid membership proofs
- **Metadata analysis**: Adversaries can't map network graph

## Design Goals

1. **Zero-knowledge**: Prove "I'm in the network" without revealing which node
2. **Succinct**: Proof size <200KB, verification <10ms
3. **Dynamic**: Support network changes (nodes join/leave)
4. **Scalable**: Handle 10K-1M nodes efficiently
5. **Composable**: Integrate with routing path proofs

## Architecture

### 1. Network Commitment

**Structure**: Merkle tree of network nodes

```
                    Root Hash (32 bytes)
                   /                    \
            H(L1,R1)                    H(L2,R2)
           /        \                   /        \
      H(N1,N2)    H(N3,N4)         H(N5,N6)    H(N7,N8)
      /    \      /    \           /    \      /    \
     N1   N2    N3    N4         N5    N6    N7    N8
     
Each node: NodeID (32 bytes) + PublicKey (64 bytes) + Metadata (32 bytes)
```

**Parameters**:
- Tree depth: `log2(num_nodes)` (e.g., 20 for 1M nodes)
- Leaf size: 128 bytes (NodeID + PubKey + Metadata)
- Hash function: Blake3 (fast, 32-byte output)

**Root Hash**: Published in network consensus (blockchain/DHT)

### 2. Membership Proof

**What node proves**:
- "I know a leaf in the Merkle tree"
- "The leaf contains my NodeID"
- "I have the private key for this node"
- **Without revealing**: Which leaf, which position in tree

**Proof Components**:
1. **Merkle path**: `log2(N)` sibling hashes (e.g., 20 hashes for 1M nodes)
2. **Zero-knowledge wrapper**: Hide which path was used
3. **Signature**: Prove knowledge of private key

### 3. Circuit Design

#### Public Inputs
- `network_root`: [u8; 32] - Merkle root of current network
- `nullifier`: [u8; 32] - Unique identifier for this proof (prevents replay)
- `timestamp`: u64 - Proof generation time (freshness)

#### Private Inputs (Witness)
- `node_id`: [u8; 32] - My node identifier
- `public_key`: [u8; 64] - My public key (Dilithium-5)
- `private_key`: [u8; 64] - My private key (secret)
- `merkle_path`: Vec<[u8; 32]> - Sibling hashes from leaf to root
- `leaf_index`: u64 - Position in tree (secret)

#### Circuit Constraints

```rust
// Pseudocode for zkVM guest program
pub fn verify_membership() {
    // 1. Read public inputs
    let network_root: [u8; 32] = env::read();
    let nullifier: [u8; 32] = env::read();
    let timestamp: u64 = env::read();
    
    // 2. Read private witness
    let node_id: [u8; 32] = env::read();
    let public_key: [u8; 64] = env::read();
    let private_key: [u8; 64] = env::read();
    let merkle_path: Vec<[u8; 32]> = env::read();
    let leaf_index: u64 = env::read();
    
    // 3. Verify timestamp freshness (within 1 hour)
    let now = current_timestamp();
    assert!(now - timestamp < 3600, "Proof too old");
    
    // 4. Compute leaf hash
    let leaf_data = concat!(node_id, public_key, metadata);
    let leaf_hash = blake3::hash(&leaf_data);
    
    // 5. Verify Merkle path
    let mut current_hash = leaf_hash;
    for (i, sibling) in merkle_path.iter().enumerate() {
        let bit = (leaf_index >> i) & 1;
        current_hash = if bit == 0 {
            blake3::hash(&concat!(current_hash, sibling))
        } else {
            blake3::hash(&concat!(sibling, current_hash))
        };
    }
    assert_eq!(current_hash, network_root, "Invalid Merkle proof");
    
    // 6. Prove knowledge of private key
    let signature = sign_dilithium(private_key, &nullifier);
    assert!(verify_dilithium(&public_key, &nullifier, &signature), "Invalid signature");
    
    // 7. Commit to public outputs
    env::commit(&true);
}
```

**Constraint Count**:
- Blake3 hashes: ~5K RISC-V instructions each × 20 (tree depth) = **100K instructions**
- Dilithium signature verification: ~200K instructions
- **Total**: ~300K instructions

**Performance Estimates**:
- RISC Zero CPU: ~40s per proof
- RISC Zero GPU: ~8s per proof
- SP1 CPU: ~4s per proof
- SP1 GPU: ~800ms per proof ✅ Acceptable

### 4. Nullifier System

**Purpose**: Prevent proof replay, enable rate limiting

**Design**:
```rust
nullifier = Blake3(node_id || epoch || action)
```

**Parameters**:
- `node_id`: Your secret node identifier
- `epoch`: Time window (e.g., hourly epoch = timestamp / 3600)
- `action`: "join_network" | "route_packet" | "announce_presence"

**Properties**:
- Each node can only generate 1 proof per action per epoch
- Different actions have different nullifiers
- Nullifiers are unlinkable across epochs

**Usage**:
```rust
// Rate limit: 1 announcement per hour
let epoch = timestamp / 3600;
let nullifier = blake3(&concat!(node_id, epoch, "announce"));

// Network tracks: Set<[u8; 32]> (seen nullifiers)
if nullifiers.contains(&nullifier) {
    reject("Duplicate nullifier - spam detected");
}
nullifiers.insert(nullifier);
```

## Implementation Plan

### Phase 1: Merkle Tree Infrastructure (Week 1)

**Goal**: Build efficient Merkle tree for network state

**Tasks**:
1. Implement `MerkleTree` struct in `phantom-core`
2. Add `insert_node()`, `remove_node()`, `get_proof()` methods
3. Use Blake3 for hashing (fast)
4. Optimize for sparse trees (not all leaves filled)
5. Write comprehensive tests

**Code Skeleton**:
```rust
// crates/phantom-core/src/merkle.rs
pub struct MerkleTree {
    nodes: HashMap<u64, [u8; 32]>, // Sparse storage
    depth: usize,
    root: [u8; 32],
}

impl MerkleTree {
    pub fn new(depth: usize) -> Self;
    pub fn insert(&mut self, index: u64, leaf: &[u8]) -> Result<()>;
    pub fn remove(&mut self, index: u64) -> Result<()>;
    pub fn root(&self) -> [u8; 32];
    pub fn get_proof(&self, index: u64) -> Result<MerkleProof>;
}

pub struct MerkleProof {
    pub leaf_index: u64,
    pub siblings: Vec<[u8; 32]>,
}

impl MerkleProof {
    pub fn verify(&self, leaf: &[u8], root: &[u8; 32]) -> bool;
}
```

**Success Criteria**:
- Insert 10K nodes in <1s
- Generate proof in <1ms
- Verify proof in <0.5ms
- All tests passing

**Estimated Time**: 3 days

### Phase 2: zkVM Circuit Implementation (Week 2)

**Goal**: Prove Merkle membership in zero-knowledge

**Tasks**:
1. Create `guest/` directory in `phantom-zkvm`
2. Write guest program (membership circuit)
3. Implement host-side proof generation
4. Integrate with `ProofGenerator`
5. Benchmark performance

**File Structure**:
```
crates/phantom-zkvm/
├── guest/
│   ├── Cargo.toml
│   └── src/
│       └── main.rs          # Circuit implementation
├── methods/
│   └── guest/
│       └── Cargo.toml
└── src/
    ├── lib.rs               # Host-side API
    └── membership.rs        # Membership proof API
```

**Guest Program** (`guest/src/main.rs`):
```rust
#![no_std]
#![no_main]

sp1_zkvm::entrypoint!(main);

use sha3::{Digest, Sha3_256};

pub fn main() {
    // Read public inputs
    let network_root = sp1_zkvm::io::read::<[u8; 32]>();
    let nullifier = sp1_zkvm::io::read::<[u8; 32]>();
    let timestamp = sp1_zkvm::io::read::<u64>();
    
    // Read private witness
    let node_id = sp1_zkvm::io::read::<[u8; 32]>();
    let public_key = sp1_zkvm::io::read::<[u8; 64]>();
    let merkle_siblings = sp1_zkvm::io::read::<Vec<[u8; 32]>>();
    let leaf_index = sp1_zkvm::io::read::<u64>();
    
    // Verify timestamp (within 1 hour)
    // Note: In real impl, get time from environment
    assert!(timestamp > 0, "Invalid timestamp");
    
    // Compute leaf hash
    let mut leaf_data = [0u8; 128];
    leaf_data[0..32].copy_from_slice(&node_id);
    leaf_data[32..96].copy_from_slice(&public_key);
    // metadata would go in 96..128
    
    let mut hasher = Sha3_256::new();
    hasher.update(&leaf_data);
    let mut current_hash: [u8; 32] = hasher.finalize().into();
    
    // Verify Merkle path
    for (i, sibling) in merkle_siblings.iter().enumerate() {
        let bit = (leaf_index >> i) & 1;
        let mut hasher = Sha3_256::new();
        
        if bit == 0 {
            hasher.update(&current_hash);
            hasher.update(sibling);
        } else {
            hasher.update(sibling);
            hasher.update(&current_hash);
        }
        
        current_hash = hasher.finalize().into();
    }
    
    // Verify root matches
    assert_eq!(current_hash, network_root, "Invalid Merkle proof");
    
    // Commit to success
    sp1_zkvm::io::commit(&true);
}
```

**Host API** (`src/membership.rs`):
```rust
pub struct MembershipProof {
    pub proof: RoutingProof,
    pub nullifier: [u8; 32],
    pub timestamp: u64,
}

impl ProofGenerator {
    pub fn prove_membership(
        &self,
        node_id: &[u8; 32],
        public_key: &[u8; 64],
        merkle_proof: &MerkleProof,
        network_root: &[u8; 32],
    ) -> anyhow::Result<MembershipProof> {
        // Generate nullifier
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        let nullifier = self.generate_nullifier(node_id, timestamp);
        
        // Prepare witness
        let mut stdin = SP1Stdin::new();
        stdin.write(network_root);
        stdin.write(&nullifier);
        stdin.write(&timestamp);
        stdin.write(node_id);
        stdin.write(public_key);
        stdin.write(&merkle_proof.siblings);
        stdin.write(&merkle_proof.leaf_index);
        
        // Generate proof
        let proof = self.client.prove(&self.pk, stdin).run()?;
        
        Ok(MembershipProof {
            proof: RoutingProof::from_sp1(proof),
            nullifier,
            timestamp,
        })
    }
    
    pub fn verify_membership(
        &self,
        proof: &MembershipProof,
        network_root: &[u8; 32],
    ) -> anyhow::Result<bool> {
        // Check timestamp freshness
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs();
        if now - proof.timestamp > 3600 {
            return Ok(false);
        }
        
        // Verify proof
        self.client.verify(&proof.proof.into_sp1(), &self.vk)
    }
}
```

**Success Criteria**:
- Circuit compiles without errors
- Proof generation works (even if slow)
- Verification succeeds for valid proofs
- Invalid proofs correctly rejected

**Estimated Time**: 4 days

### Phase 3: Optimization (Week 3)

**Goal**: Achieve <1s proof generation with GPU

**Tasks**:
1. Enable SP1 GPU backend
2. Profile circuit bottlenecks
3. Optimize hash functions (use Blake3 instead of SHA3 if faster)
4. Reduce circuit size (minimize RISC-V instructions)
5. Benchmark on multiple GPUs

**Optimization Techniques**:
- **Use native ops**: SP1 has optimized Blake3/SHA256
- **Reduce tree depth**: Balance security vs performance
- **Batch verification**: Verify multiple proofs together
- **Proof caching**: Cache proofs for static network periods

**Target Performance**:
- Proof generation (GPU): <1s
- Proof verification (CPU): <10ms
- Proof size: <200KB
- Memory usage: <4GB

**Estimated Time**: 3 days

### Phase 4: Integration (Week 4)

**Goal**: Integrate membership proofs into PHANTOM protocol

**Tasks**:
1. Add membership proof to `PhantomPacket`
2. Update node discovery protocol
3. Implement network consensus for Merkle root
4. Add nullifier tracking (spam prevention)
5. Write integration tests

**Packet Format Update**:
```rust
pub struct PhantomPacket {
    routing_blob: Vec<u8>,           // FHE-encrypted routing table
    path_proof: RoutingProof,        // zkVM proof (path validity)
    membership_proof: MembershipProof, // NEW: zkVM proof (node ∈ network)
    payload: Vec<u8>,                // Encrypted application data
    nullifier: [u8; 32],             // Rate-limiting nullifier
}
```

**Network Consensus**:
```rust
pub struct NetworkState {
    merkle_root: [u8; 32],
    epoch: u64,
    node_count: usize,
    last_update: u64,
}

impl NetworkState {
    pub fn update_root(&mut self, new_root: [u8; 32]) {
        self.merkle_root = new_root;
        self.epoch += 1;
        self.last_update = current_timestamp();
    }
}
```

**Success Criteria**:
- Nodes can prove membership
- Invalid membership proofs rejected
- Network state synchronizes correctly
- Nullifier spam prevention works

**Estimated Time**: 4 days

## Performance Analysis

### Merkle Tree Depth Trade-offs

| Network Size | Tree Depth | Proof Size | Proof Gen | Verify |
|--------------|------------|------------|-----------|--------|
| 1K nodes | 10 | 320 bytes | ~400ms | ~5ms |
| 10K nodes | 14 | 448 bytes | ~600ms | ~7ms |
| 100K nodes | 17 | 544 bytes | ~750ms | ~8.5ms |
| 1M nodes | 20 | 640 bytes | ~900ms | ~10ms |

**Recommendation**: Start with depth=20 (supports 1M nodes)

### Circuit Size

**RISC-V Instructions**:
- Blake3 hash: ~5K instructions
- Merkle verification (depth 20): ~100K instructions
- Signature verification: ~200K instructions
- **Total**: ~300K instructions

**SP1 Performance**:
- CPU: ~4s per proof
- GPU: ~800ms per proof
- Batch (10 proofs): ~5s (500ms each)

### Network Overhead

**Per Packet**:
- Membership proof: ~150KB
- Path proof: ~64 bytes
- **Total overhead**: ~150KB

**Optimization**: Batch proofs
- Generate 1 membership proof per epoch (1 hour)
- Reuse for all packets in that epoch
- Amortized cost: ~0 overhead

## Security Analysis

### Threat Model

**Adversary Capabilities**:
- Control 90% of network nodes
- Unlimited computational power
- Access to all network traffic

**Security Goals**:
1. **Anonymity**: Adversary cannot deanonymize nodes
2. **Unforgeability**: Cannot create fake membership proofs
3. **Rate limiting**: Cannot spam network with fake proofs

### Cryptographic Assumptions

1. **Blake3 collision resistance**: Finding `x ≠ y` with `H(x) = H(y)` is infeasible
2. **zkVM soundness**: Cannot create valid proof for false statement
3. **Dilithium security**: Cannot forge signatures (post-quantum secure)

### Attack Scenarios

**Attack 1**: Forge membership proof for non-existent node
- **Defense**: zkVM soundness + Merkle proof verification
- **Probability**: Negligible (2^-256)

**Attack 2**: Replay old membership proof
- **Defense**: Timestamp freshness check + nullifier system
- **Probability**: 0 (deterministically prevented)

**Attack 3**: Grind nullifiers to spam network
- **Defense**: Nullifier = H(secret_node_id || epoch || action)
- **Probability**: 0 (cannot grind without knowing secret_node_id)

**Attack 4**: Sybil attack (many fake nodes)
- **Defense**: Proof-of-personhood (future integration)
- **Status**: Planned but not yet implemented

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_merkle_tree_insert() {
    let mut tree = MerkleTree::new(10);
    tree.insert(0, &node_data).unwrap();
    assert!(tree.get_proof(0).is_ok());
}

#[test]
fn test_merkle_proof_verification() {
    let tree = create_test_tree(100);
    let proof = tree.get_proof(42).unwrap();
    assert!(proof.verify(&leaf_data, &tree.root()));
}

#[test]
fn test_membership_proof_generation() {
    let proof = generator.prove_membership(
        &node_id, &pub_key, &merkle_proof, &root
    ).unwrap();
    assert!(generator.verify_membership(&proof, &root).unwrap());
}

#[test]
fn test_invalid_membership_rejected() {
    let proof = create_invalid_proof();
    assert!(!generator.verify_membership(&proof, &root).unwrap());
}

#[test]
fn test_nullifier_prevents_replay() {
    let null1 = generate_nullifier(&node_id, epoch, "action");
    let null2 = generate_nullifier(&node_id, epoch, "action");
    assert_eq!(null1, null2); // Same epoch = same nullifier
}
```

### Integration Tests
```rust
#[test]
fn test_network_state_update() {
    let mut state = NetworkState::new();
    let new_root = compute_root_after_join(&new_node);
    state.update_root(new_root);
    assert_eq!(state.epoch, 1);
}

#[test]
fn test_packet_with_membership_proof() {
    let packet = PhantomPacket::construct(
        path, routing_blob, payload, membership_proof
    );
    assert!(packet.verify_membership_proof(&network_root).unwrap());
}
```

### Performance Tests
```rust
#[bench]
fn bench_merkle_proof_generation(b: &mut Bencher) {
    let tree = create_large_tree(100_000);
    b.iter(|| tree.get_proof(rand::random()));
}

#[bench]
fn bench_membership_proof_generation(b: &mut Bencher) {
    b.iter(|| generator.prove_membership(...));
}
```

## Future Enhancements

### 1. Proof-of-Personhood Integration
Add biometric or social graph verification to prevent Sybil attacks:
- WorldCoin integration
- BrightID social graph
- Gitcoin Passport

### 2. Dynamic Network Updates
Efficient updates when nodes join/leave:
- Incremental Merkle tree updates
- Epoch-based root rotation
- Delta proofs for updates

### 3. Recursive Proof Composition
Combine membership + routing proofs:
- Single proof for both properties
- Smaller total proof size
- Faster verification

### 4. Anonymous Credentials
Issue credentials to nodes:
- Reputation scores
- Role-based access
- Service tiers

## References

- **Merkle Trees**: Merkle, R. C. (1987). "A Digital Signature Based on a Conventional Encryption Function"
- **zkSNARKs**: Ben-Sasson et al. (2014). "Succinct Non-Interactive Zero Knowledge"
- **RLN**: Semaphore (2021). "Rate-Limiting Nullifier"
- **SP1**: Succinct Labs (2024). "SP1: A Performant, 100% Open-Source zkVM"

---

**Next Steps**: Begin Phase 1 (Merkle tree implementation) after zkVM integration complete
