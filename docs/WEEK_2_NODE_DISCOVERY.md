# Week 2: Anonymous Node Discovery Protocol

**Date**: November 25, 2025  
**Goal**: Enable nodes to join/leave network anonymously using zero-knowledge membership proofs  
**Status**: Starting implementation

---

## 🎯 Objectives

### Primary Goal
Build a **self-organizing anonymous network** where:
- Nodes can announce themselves **without revealing identity**
- Other nodes can verify membership **without learning which node**
- Sybil attacks prevented via **nullifier system**
- Network state synchronized via **Merkle root consensus**

### Success Criteria
- [x] Merkle tree infrastructure (DONE - already exists in `phantom-core`)
- [ ] Network state management (epochs, Merkle root updates)
- [ ] Anonymous node announcements (gossip protocol)
- [ ] Membership proof generation (Plonky2 circuits)
- [ ] Nullifier tracking (prevent spam/Sybil)
- [ ] 100-node simulation working

---

## 📊 Architecture Overview

### Components

```
┌─────────────────────────────────────────────────────────┐
│                    Network State                        │
│  - Merkle Root (commitment to all nodes)                │
│  - Epoch (time-based updates)                           │
│  - Node Count                                            │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│              Node Announcement (Gossip)                  │
│  - Membership Proof (zk-SNARK)                           │
│  - Nullifier (prevent double-announce)                   │
│  - Timestamp (freshness check)                           │
│  - Node Info (IP, port, bandwidth - encrypted)           │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│           Verification (Each Node Checks)                │
│  1. Verify membership proof (Plonky2)                    │
│  2. Check nullifier uniqueness (not seen before)         │
│  3. Validate timestamp (within epoch)                    │
│  4. Add node to local routing table                      │
└─────────────────────────────────────────────────────────┘
```

### Data Structures

```rust
// Network state (shared across all nodes)
pub struct NetworkState {
    pub merkle_root: [u8; 32],      // Commitment to all nodes
    pub epoch: u64,                  // Current time epoch (e.g., every 10 minutes)
    pub node_count: usize,           // Total nodes in network
    pub last_update: u64,            // Unix timestamp
}

// Anonymous node announcement
pub struct NodeAnnouncement {
    pub membership_proof: Vec<u8>,   // Plonky2 proof
    pub nullifier: [u8; 32],         // Unique per (node, epoch) - prevents spam
    pub timestamp: u64,              // Unix timestamp
    pub node_info_encrypted: Vec<u8>,// Encrypted IP/port/bandwidth
}

// Membership proof (zk-SNARK)
pub struct MembershipProof {
    pub proof_data: Vec<u8>,         // Plonky2 proof bytes
    pub public_inputs: PublicInputs, // What verifier sees
}

pub struct PublicInputs {
    pub merkle_root: [u8; 32],       // Which network the node claims to be in
    pub nullifier: [u8; 32],         // Unique identifier for this announcement
    pub epoch: u64,                  // Which epoch this is for
}
```

---

## 📅 Week 2 Implementation Plan

### Day 1 (Monday): Network State Management

**Goal**: Implement network state tracking and epoch-based updates

**Tasks**:
1. Create `NetworkState` struct in `phantom-discovery/src/state.rs`
2. Implement epoch calculation (10-minute intervals)
3. Add Merkle root update logic
4. Write state serialization/deserialization
5. Add unit tests for state transitions

**Deliverable**: `NetworkState` struct with epoch management

**Code Skeleton**:
```rust
// phantom-discovery/src/state.rs
pub struct NetworkState {
    merkle_root: [u8; 32],
    epoch: u64,
    node_count: usize,
    last_update: u64,
}

impl NetworkState {
    pub fn new() -> Self { ... }
    pub fn current_epoch() -> u64 { ... }
    pub fn update_merkle_root(&mut self, new_root: [u8; 32]) { ... }
    pub fn is_epoch_valid(&self, epoch: u64) -> bool { ... }
}
```

---

### Day 2 (Tuesday): Membership Proof Circuit

**Goal**: Build Plonky2 circuit for anonymous membership proofs

**Tasks**:
1. Create membership circuit in `phantom-circuit/src/membership.rs`
2. Implement Merkle path verification circuit
3. Add nullifier generation (hash of node_id + epoch)
4. Add epoch freshness check
5. Write circuit tests

**Deliverable**: Plonky2 circuit proving "I'm in the Merkle tree"

**Circuit Logic**:
```rust
// Private inputs (only prover knows):
// - node_id: [u8; 32]
// - merkle_proof: Vec<Hash>
// - leaf_index: u64

// Public inputs (verifier sees):
// - merkle_root: [u8; 32]
// - nullifier: hash(node_id || epoch)
// - epoch: u64

// Circuit constraints:
// 1. Verify Merkle proof from leaf to root
// 2. Compute nullifier = hash(node_id || epoch)
// 3. Check epoch within valid range
```

**Expected Proof**:
- Generation time: ~100-200ms (Plonky2 on CPU)
- Verification time: ~10ms
- Proof size: ~400 KB

---

### Day 3 (Wednesday): Membership Proof Generator

**Goal**: Integrate circuit with proof generation API

**Tasks**:
1. Add `prove_membership()` to `Plonky2ProofGenerator`
2. Implement proof verification
3. Create `MembershipProof` struct
4. Add nullifier computation helper
5. Write integration tests

**Deliverable**: Working proof generation and verification

**API**:
```rust
// phantom-zkvm/src/plonky2.rs
impl Plonky2ProofGenerator {
    pub fn prove_membership(
        &self,
        node_id: &[u8; 32],
        merkle_proof: &MerkleProof,
        epoch: u64,
    ) -> Result<MembershipProof> {
        // Build circuit inputs
        // Generate proof
        // Return membership proof
    }
    
    pub fn verify_membership(
        &self,
        proof: &MembershipProof,
        merkle_root: &[u8; 32],
    ) -> Result<bool> {
        // Verify proof validity
        // Check nullifier uniqueness (not done here)
    }
}
```

---

### Day 4 (Thursday): Nullifier Tracking System

**Goal**: Prevent spam and Sybil attacks via nullifiers

**Tasks**:
1. Create `NullifierRegistry` in `phantom-discovery/src/nullifiers.rs`
2. Implement nullifier storage (in-memory with LRU eviction)
3. Add duplicate detection
4. Implement TTL (expire after epoch ends)
5. Write tests for spam prevention

**Deliverable**: Nullifier tracking prevents double-announcements

**Code**:
```rust
// phantom-discovery/src/nullifiers.rs
pub struct NullifierRegistry {
    seen: HashMap<Nullifier, (Epoch, Timestamp)>,
    max_size: usize,
}

impl NullifierRegistry {
    pub fn new(max_size: usize) -> Self { ... }
    
    pub fn has_seen(&self, nullifier: &Nullifier) -> bool { ... }
    
    pub fn register(&mut self, nullifier: Nullifier, epoch: Epoch) -> Result<()> {
        if self.has_seen(&nullifier) {
            return Err(ProtocolError::DuplicateNullifier);
        }
        self.seen.insert(nullifier, (epoch, current_time()));
        Ok(())
    }
    
    pub fn evict_expired(&mut self, current_epoch: Epoch) { ... }
}
```

**Spam Prevention**:
- Each node can announce **once per epoch**
- Nullifier = hash(node_id || epoch) → unique per (node, epoch)
- Old nullifiers evicted after epoch expires
- Max registry size: 100K nullifiers (~3 MB RAM)

---

### Day 5 (Friday): Node Announcement Protocol

**Goal**: Build gossip protocol for distributing announcements

**Tasks**:
1. Create `Announcer` in `phantom-discovery/src/announcer.rs`
2. Implement announcement creation
3. Add broadcast logic (gossip to N peers)
4. Implement announcement reception and validation
5. Write network simulation tests

**Deliverable**: Nodes can announce and verify each other

**Code**:
```rust
// phantom-discovery/src/announcer.rs
pub struct Announcer {
    proof_generator: Plonky2ProofGenerator,
    nullifier_registry: NullifierRegistry,
    network_state: NetworkState,
}

impl Announcer {
    pub fn create_announcement(
        &self,
        node_id: &[u8; 32],
        merkle_proof: &MerkleProof,
    ) -> Result<NodeAnnouncement> {
        let epoch = NetworkState::current_epoch();
        let membership_proof = self.proof_generator.prove_membership(
            node_id,
            merkle_proof,
            epoch,
        )?;
        
        let nullifier = compute_nullifier(node_id, epoch);
        
        Ok(NodeAnnouncement {
            membership_proof: bincode::serialize(&membership_proof)?,
            nullifier,
            timestamp: current_time(),
            node_info_encrypted: vec![], // TODO: encrypt node info
        })
    }
    
    pub fn verify_announcement(
        &mut self,
        announcement: &NodeAnnouncement,
    ) -> Result<()> {
        // 1. Check timestamp freshness
        if !is_timestamp_valid(announcement.timestamp) {
            return Err(ProtocolError::StaleAnnouncement);
        }
        
        // 2. Check nullifier uniqueness
        if self.nullifier_registry.has_seen(&announcement.nullifier) {
            return Err(ProtocolError::DuplicateNullifier);
        }
        
        // 3. Verify membership proof
        let proof: MembershipProof = bincode::deserialize(&announcement.membership_proof)?;
        if !self.proof_generator.verify_membership(&proof, &self.network_state.merkle_root)? {
            return Err(ProtocolError::InvalidProof);
        }
        
        // 4. Register nullifier
        self.nullifier_registry.register(announcement.nullifier, proof.public_inputs.epoch)?;
        
        Ok(())
    }
}
```

---

### Day 6 (Saturday): Integration and Testing

**Goal**: End-to-end testing with 100-node simulation

**Tasks**:
1. Create `examples/discovery_demo.rs`
2. Simulate 100-node network
3. Test node join (all nodes announce)
4. Test node leave (stop announcing)
5. Test spam prevention (duplicate nullifiers rejected)
6. Measure performance (announcements/sec)

**Deliverable**: Working 100-node anonymous discovery

**Demo Script**:
```rust
// examples/discovery_demo.rs
fn main() {
    // 1. Create network of 100 nodes
    let mut network = create_network(100);
    
    // 2. Build Merkle tree from all node IDs
    let mut tree = MerkleTree::new(10); // 2^10 = 1024 capacity
    for node in &network.nodes {
        tree.insert(&node.id);
    }
    
    // 3. Each node creates announcement
    for node in &network.nodes {
        let proof = tree.get_proof(node.index);
        let announcement = node.create_announcement(&proof);
        network.broadcast(announcement);
    }
    
    // 4. Verify all announcements
    let mut accepted = 0;
    for announcement in &network.announcements {
        if node.verify_announcement(announcement).is_ok() {
            accepted += 1;
        }
    }
    
    println!("✓ {}/{} announcements verified", accepted, network.nodes.len());
}
```

**Performance Targets**:
- Announcement creation: <200ms (proof generation)
- Announcement verification: <20ms (proof verify + nullifier check)
- 100 nodes announce: <20 seconds total
- Memory: <100 MB for 100-node registry

---

### Day 7 (Sunday): Documentation and Cleanup

**Goal**: Document node discovery protocol

**Tasks**:
1. Write `docs/NODE_DISCOVERY.md` specification
2. Document API in code comments
3. Update `ROADMAP.md` with Week 2 completion
4. Create diagrams (protocol flow, data structures)
5. Write testnet deployment guide

**Deliverable**: Complete documentation for node discovery

---

## 🎯 Success Metrics

### Functional Requirements
- [x] Merkle tree handles 100K nodes (already done)
- [ ] Membership proofs generated in <200ms
- [ ] Membership proofs verified in <20ms
- [ ] Nullifier system prevents spam (100% rejection of duplicates)
- [ ] 100-node network simulation works
- [ ] Nodes can join/leave anonymously

### Performance Requirements
- [ ] Proof generation: <200ms (Plonky2 on CPU)
- [ ] Proof verification: <20ms
- [ ] Proof size: <500 KB
- [ ] Nullifier registry: <100 MB RAM for 100K nodes
- [ ] Announcement broadcast: <1s to reach all nodes

### Security Requirements
- [ ] Zero-knowledge: Verifier learns nothing about node identity
- [ ] Sybil resistance: One announcement per (node, epoch)
- [ ] Replay resistance: Nullifiers prevent reuse
- [ ] Freshness: Timestamps prevent stale announcements

---

## 🚀 After Week 2

### Week 3: Testnet Deployment
- Package discovery protocol
- Deploy 10-20 node testnet
- Test with real internet latency
- Monitor for attacks

### Week 4+: Production Hardening
- Add economic layer (proof-of-personhood)
- Implement advanced Sybil resistance
- Optimize for 10K+ node networks
- Formal security audit

---

## 📚 Key Resources

### Existing Code (Reuse)
- `phantom-core/src/merkle.rs` - Merkle tree implementation ✅
- `phantom-circuit/src/merkle.rs` - Merkle circuit ✅
- `phantom-zkvm/src/plonky2.rs` - Plonky2 proof generator ✅

### New Files to Create
- `phantom-discovery/src/state.rs` - Network state management
- `phantom-discovery/src/nullifiers.rs` - Nullifier tracking
- `phantom-discovery/src/announcer.rs` - Announcement protocol
- `phantom-circuit/src/membership.rs` - Membership circuit
- `examples/discovery_demo.rs` - 100-node simulation

### External References
- Zcash Sapling nullifiers: https://z.cash/technology/sapling/
- Semaphore group membership: https://semaphore.appliedzkp.org/
- RLN (Rate Limiting Nullifier): https://rate-limiting-nullifier.github.io/rln-docs/

---

**Ready to start Day 1: Network State Management?**
