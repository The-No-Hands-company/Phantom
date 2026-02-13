# Week 3: Anonymous Routing Integration

**Goal**: Combine discovery (Week 2) with oblivious routing (Week 1) to create complete anonymous packet forwarding.

**Duration**: 7 days  
**Complexity**: High  
**Status**: Starting

---

## 🎯 Overview

Week 3 integrates all PHANTOM components built so far:
- **Week 1**: Post-quantum crypto + FHE oblivious routing
- **Week 2**: Anonymous node discovery with zk-proofs
- **Week 3**: Put them together for complete anonymous routing

---

## 📅 Day-by-Day Plan

### **Day 1: Peer Selection System** (2-3 hours)

**Goal**: Select routing peers using membership proofs

**Components**:
```rust
// PeerSelector: Chooses peers from discovered nodes
struct PeerSelector {
    discovered_nodes: Vec<NodeInfo>,
    selection_strategy: SelectionStrategy,
}

// NodeInfo: Metadata about discovered nodes
struct NodeInfo {
    nullifier: Nullifier,      // From announcement
    last_seen: u64,            // Epoch
    reliability: f64,          // Uptime metric
    capacity: u64,             // Bandwidth
}
```

**Deliverables**:
- `phantom-routing/src/peer_selector.rs` (200 lines)
- Selection strategies (random, weighted, latency-based)
- Tests (5-7 tests)

---

### **Day 2: Path Construction** (3-4 hours)

**Goal**: Build multi-hop paths through selected peers

**Components**:
```rust
// PathBuilder: Constructs anonymous routes
struct PathBuilder {
    peer_selector: PeerSelector,
    min_hops: usize,
    max_hops: usize,
}

// Path: Complete route through network
struct Path {
    hops: Vec<NodeInfo>,
    path_id: [u8; 32],
    created_at: u64,
}
```

**Features**:
- Configurable hop count (default: 3-5 hops)
- Path diversity (avoid node reuse)
- Geographic distribution (if available)
- Byzantine resistance (exclude known bad nodes)

**Deliverables**:
- `phantom-routing/src/path_builder.rs` (250 lines)
- Path validation logic
- Tests (7-9 tests)

---

### **Day 3: Route Validation with zkVM** (3-4 hours)

**Goal**: Generate zk-proofs that paths are valid

**Components**:
```rust
// RouteValidator: Proves path validity without revealing path
struct RouteValidator {
    zkvm_prover: ZkVMProver,
}

// RouteProof: zk-SNARK of path validity
struct RouteProof {
    proof_bytes: Vec<u8>,
    path_commitment: [u8; 32],
    hop_count: u8,
}
```

**Circuit Logic**:
```
Inputs (Private):
- path: Vec<NodeInfo>
- peer_announcements: Vec<Announcement>

Inputs (Public):
- path_commitment: hash(path)
- network_merkle_root: [u8; 32]
- hop_count: u8

Circuit:
1. For each hop, verify membership proof
2. Verify hop count matches
3. Verify path commitment
4. Output public inputs
```

**Deliverables**:
- `phantom-circuit/src/route.rs` (300 lines)
- Route validation circuit
- Integration with phantom-zkvm
- Tests (5-7 tests)

---

### **Day 4: FHE Routing Integration** (3-4 hours)

**Goal**: Combine path construction with oblivious forwarding

**Components**:
```rust
// ObliviousRouter: Routes packets using FHE
struct ObliviousRouter {
    fhe_engine: FheEngine,
    path_builder: PathBuilder,
}

// RoutingPacket: Complete packet with FHE routing blob
struct RoutingPacket {
    routing_blob: Vec<u8>,     // FHE-encrypted routing table
    route_proof: RouteProof,   // zk-SNARK of path validity
    payload: Vec<u8>,          // Encrypted data
    nullifier: [u8; 32],       // Rate limiting
}
```

**Integration Flow**:
1. PathBuilder selects peers
2. FheEngine encrypts routing table
3. RouteValidator generates proof
4. ObliviousRouter forwards packet

**Deliverables**:
- `phantom-routing/src/oblivious_router.rs` (350 lines)
- Complete routing pipeline
- Tests (8-10 tests)

---

### **Day 5: Encrypted Communication Channels** (2-3 hours)

**Goal**: End-to-end encryption over anonymous routes

**Components**:
```rust
// SecureChannel: E2E encrypted communication
struct SecureChannel {
    router: ObliviousRouter,
    kyber_keypair: KyberKeypair,
}

// EncryptedMessage: Complete encrypted packet
struct EncryptedMessage {
    routing_packet: RoutingPacket,
    ciphertext: Vec<u8>,
    ephemeral_key: Vec<u8>,
}
```

**Features**:
- Post-quantum key exchange (Kyber-1024)
- Forward secrecy (new keys per message)
- Authenticated encryption (ChaCha20-Poly1305)
- Replay protection

**Deliverables**:
- `phantom-routing/src/secure_channel.rs` (200 lines)
- E2E encryption layer
- Tests (6-8 tests)

---

### **Day 6: Integration Testing** (3-4 hours)

**Goal**: Test complete routing pipeline

**Test Scenarios**:
1. **Happy path**: 100 nodes, 50 paths, all succeed
2. **Byzantine nodes**: 33% malicious, routing still works
3. **Path diversity**: 1000 paths, no node appears >10% of time
4. **Performance**: Time to construct + forward 100 packets
5. **Failure recovery**: Nodes drop, routes rebuild

**Deliverables**:
- `phantom-routing/tests/integration_tests.rs` (400 lines)
- Comprehensive end-to-end tests
- Performance benchmarks
- Byzantine fault tests

---

### **Day 7: Demo & Documentation** (2-3 hours)

**Goal**: Complete example and documentation

**Deliverables**:
1. **Complete routing demo** (`examples/routing_demo.rs`)
   - Shows discovery → path selection → routing → delivery
   - 200-300 lines, fully working

2. **Architecture documentation** (`docs/ROUTING_INTEGRATION.md`)
   - How components integrate
   - Security properties
   - Performance characteristics

3. **API documentation**
   - Rustdoc for all public APIs
   - Usage examples
   - Migration guide from Week 2

4. **Week 3 completion report**
   - Summary of achievements
   - Test results
   - Performance metrics

---

## 🎯 Success Criteria

By end of Week 3, we'll have:

✅ **Complete routing pipeline**
- Discovery → peer selection → path construction → forwarding

✅ **Security properties verified**
- Oblivious routing (FHE prevents metadata leaks)
- Anonymous paths (zk-proofs hide route details)
- Spam resistance (nullifiers prevent abuse)
- Post-quantum security (Kyber + Dilithium)

✅ **Performance benchmarks**
- Path construction: <100ms
- Route proof generation: <200ms
- Packet forwarding: <500ms per hop
- E2E latency: <2 seconds for 5-hop route

✅ **Comprehensive tests**
- 50+ unit tests
- 10+ integration tests
- Byzantine fault tolerance verified
- 100% code coverage

---

## 📊 Expected Metrics

### Code
- **New code**: ~1,500 lines
- **Tests**: ~600 lines
- **Documentation**: ~500 lines
- **Total**: ~2,600 lines

### Performance Targets
- **Peer selection**: <10ms
- **Path construction**: <50ms
- **Route proof**: <150ms
- **FHE routing blob**: <100ms
- **Total overhead**: <300ms per packet

### Security
- **Oblivious routing**: ✅ (FHE-based)
- **Anonymous paths**: ✅ (zk-proofs)
- **Forward secrecy**: ✅ (Kyber ephemeral keys)
- **Replay protection**: ✅ (Nullifiers)
- **Quantum resistance**: ✅ (All PQ crypto)

---

## 🚀 Week 3 vs Prior Weeks

| Aspect | Week 1 | Week 2 | Week 3 |
|--------|--------|--------|--------|
| **Focus** | Crypto primitives | Node discovery | **Integration** |
| **Complexity** | Medium | Medium | **High** |
| **Components** | 3 crates | 2 crates | **All crates** |
| **Lines of code** | ~1,000 | ~1,600 | **~1,500** |
| **Tests** | 15 | 30 | **50+** |
| **Integration** | Standalone | Standalone | **Everything** |

---

## 💡 Key Challenges

### Technical
1. **Proof composition** - Combine membership + route proofs
2. **FHE performance** - Keep routing overhead low
3. **Path diversity** - Avoid centralization
4. **Byzantine resistance** - Handle malicious nodes

### Integration
1. **State management** - Coordinate discovered nodes + routes
2. **Error handling** - Graceful degradation when nodes fail
3. **Testing complexity** - Multi-component interactions

### Performance
1. **Latency budget** - <500ms per hop
2. **Throughput** - 100+ packets/second
3. **Memory** - Keep state compact

---

## 📁 File Structure After Week 3

```
phantom-routing/
├── src/
│   ├── lib.rs
│   ├── peer_selector.rs       # NEW (Day 1)
│   ├── path_builder.rs        # NEW (Day 2)
│   ├── oblivious_router.rs    # NEW (Day 4)
│   └── secure_channel.rs      # NEW (Day 5)
├── tests/
│   └── integration_tests.rs   # NEW (Day 6)
└── examples/
    └── routing_demo.rs        # NEW (Day 7)

phantom-circuit/
└── src/
    ├── membership.rs          # Week 2
    └── route.rs               # NEW (Day 3)

docs/
├── ROUTING_INTEGRATION.md     # NEW (Day 7)
└── WEEK_3_COMPLETE.md         # NEW (Day 7)
```

---

## 🎓 Learning Goals

By completing Week 3, you'll understand:
- How to compose multiple zk-SNARK circuits
- FHE performance optimization techniques
- Byzantine fault-tolerant routing
- End-to-end encrypted communication over anonymous networks
- Production cryptographic system integration

---

## ✨ Why This Matters

Week 3 is where PHANTOM goes from "interesting cryptographic primitives" to **"working anonymous network protocol"**.

After Week 3:
- ✅ You can route packets anonymously
- ✅ Nodes can't learn routing metadata
- ✅ All cryptographic properties verified
- ✅ Ready for network simulation (Week 4)

This is the **core** of PHANTOM. Everything else builds on this foundation.

---

**Ready to start Day 1?**

Let's build peer selection! 🚀
