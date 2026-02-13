# Week 4: FHE Optimization - Final Strategy

**Date**: November 26, 2025  
**Status**: Pragmatic approach - accept CPU limitations, focus on alternatives

---

## 🎯 Week 4 Revised Goal

**Original Goal**: Reduce FHE lookup from 2.5s/hop → <500ms/hop (5x speedup)  
**Reality Check**: 99.8% of time is in TFHE-rs core operations that cannot be CPU-optimized  
**New Goal**: Accept current performance, explore alternatives, document GPU path

---

## ✅ What We've Already Tried (Phase 1-2)

### Attempted Optimizations
1. ✅ Pre-encrypt zero value → **0ms gain** (negligible overhead)
2. ✅ Cache server key globally → **0ms gain** (negligible overhead)
3. ✅ Optimize deserialization → **0ms gain** (0.46ms total, irrelevant)
4. ✅ TFHE parameter reduction → **0ms gain** (wrong parameter set)
5. ✅ Batch encryption (Rayon) → **3x speedup achieved** (already implemented)

### Key Findings
- **FHE operations breakdown** (per 5-entry lookup):
  - `FheUint32::eq()`: ~400ms × 5 = 2.0s
  - `FheUint32::if_then_else()`: ~400ms × 5 = 2.0s  
  - `FheUint32::+()`: ~300ms × 5 = 1.5s
  - **Total**: ~5.5s of pure TFHE-rs operations
  
- **Non-FHE overhead**: <0.2% of total time
- **Conclusion**: Cannot optimize TFHE-rs library operations on CPU

---

## 🚀 Week 4 Alternative Strategy

### Option A: Accept Current Performance ⭐ RECOMMENDED
**Rationale**: PHANTOM is revolutionary even at 6s latency

**Arguments**:
1. **Tor latency**: ~5s for similar path length (but leaks metadata!)
2. **PHANTOM advantage**: Mathematically impossible to surveil (worth the cost)
3. **Community GPU nodes**: Let GPU-equipped operators run fast nodes
4. **v1.0 baseline**: Ship working protocol now, optimize later

**Action Items**:
- [x] Document CPU baseline: 2.5s/hop (acceptable for initial deployment)
- [ ] Update docs: CPU vs GPU performance expectations
- [ ] Focus on protocol completeness (Node CLI, testnet)
- [ ] Defer GPU optimization to Week 8+ (when budget allows)

### Option B: Cloud GPU Validation
**Cost**: $10-50 for AWS g4dn.xlarge testing  
**Benefit**: Prove 10x speedup is achievable (250ms/hop → 1.25s for 5-hop)

**Action Items**:
- [ ] Write GPU benchmark script (CONCRETE or cuFHE)
- [ ] Rent cloud GPU for 4-8 hours
- [ ] Validate 10x speedup claim
- [ ] Document GPU setup for community

**Status**: Deferred to Week 8 (after testnet validation)

### Option C: Algorithmic Bypass (Experimental)
**Idea**: Reduce number of FHE operations per lookup

**Approaches**:
1. **FHE lookup table** (binary search on encrypted data)
   - Current: O(n) linear scan (5 comparisons)
   - Optimized: O(log n) binary tree (3 comparisons)
   - **Gain**: Minimal (5 → 3 ops = 1.67x, but adds overhead)
   
2. **Sparse routing tables** (reduce table size)
   - Current: Full routing table (all 5 hops)
   - Optimized: Next-hop-only lookup (1 entry per node)
   - **Gain**: 5x fewer FHE ops! But requires multi-round trips
   - **Trade-off**: Latency goes from 5 × 2.5s = 12.5s to 5 × (2.5s + RTT)
   
3. **Hybrid FHE/cleartext** (SECURITY RISK!)
   - **DON'T DO THIS** - defeats oblivious routing property

**Recommendation**: Option C.2 (sparse tables) worth exploring if latency budget allows round trips

---

## 📊 Performance Target Reassessment

### Current Performance (CPU Baseline)
| Operation | Time | Status |
|-----------|------|--------|
| FHE key generation | 0.8s | ✅ One-time, acceptable |
| FHE batch encrypt (5 values) | 2.75ms | ✅ Rayon optimized |
| FHE oblivious lookup | **2.5s/hop** | ⚠️ TFHE-rs bottleneck |
| zkSNARK proof gen | 159ms | ✅ Production-ready |
| zkSNARK verification | 13ms | ✅ Excellent |
| **5-hop total latency** | **~13s** | ⚠️ 2.6x slower than target |

### Achievable Performance (with GPU)
| Operation | CPU | GPU (projected) | Speedup |
|-----------|-----|-----------------|---------|
| FHE oblivious lookup | 2.5s | **250ms** | 10x |
| 5-hop total latency | 13s | **1.5s** | 8.7x |

**GPU Validation**: Zama benchmarks show 10-20x speedup with CUDA

---

## 🎯 Week 4 Deliverables

### Primary Goal: Protocol Completeness
Instead of chasing GPU performance we can't achieve on this hardware, focus on:

1. **Full Node CLI** (Week 5 scope, start early)
   - Command-line interface for running PHANTOM node
   - Configuration management (keys, network parameters)
   - Packet sending/receiving API
   
2. **Testnet Preparation** (Week 6 scope)
   - Multi-node simulation infrastructure
   - Network bootstrapping protocol
   - Monitoring and observability

3. **Documentation Updates**
   - CPU vs GPU performance guide
   - Hardware requirements (CPU nodes = 13s, GPU nodes = 1.5s)
   - Community contribution guide (invite GPU operators)

### Secondary Goal: Sparse Routing Table Experiment
- [ ] Implement next-hop-only lookup (reduce FHE ops by 5x)
- [ ] Benchmark: Single lookup = 500ms (vs 2.5s)
- [ ] Trade-off analysis: 5 × (500ms + RTT) vs 1 × (12.5s)
- [ ] Decision: Accept if network latency < 2s

---

## 🧪 Sparse Routing Table Design

### Current Design (Full Path Encryption)
```rust
// Sender encrypts ENTIRE path (all 5 hops)
let routing_table = vec![
    (encrypt(node_1), encrypt(node_2)),  // Hop 1 → 2
    (encrypt(node_2), encrypt(node_3)),  // Hop 2 → 3
    (encrypt(node_3), encrypt(node_4)),  // Hop 3 → 4
    (encrypt(node_4), encrypt(node_5)),  // Hop 4 → 5
    (encrypt(node_5), encrypt(EXIT)),    // Hop 5 → exit
];

// Node does 5-entry lookup → 2.5s
let next_hop = fhe_engine.lookup(my_id, routing_table); // O(5) FHE ops
```

### Proposed Design (Sparse Next-Hop)
```rust
// Sender encrypts ONLY next hop for THIS node
let routing_table = vec![
    (encrypt(my_id), encrypt(next_hop)),  // 1 entry!
];

// Node does 1-entry lookup → 500ms
let next_hop = fhe_engine.lookup(my_id, routing_table); // O(1) FHE ops
```

**Trade-offs**:
- ✅ **5x fewer FHE operations** (2.5s → 500ms per hop)
- ✅ **Smaller packets** (1 entry vs 5 entries = 5x size reduction)
- ⚠️ **Multi-round protocol** (5 sequential lookups vs 1 batch)
- ⚠️ **Network latency dependency** (RTT becomes significant)

**Performance Math**:
- Full path: 1 × 12.5s = **12.5s total**
- Sparse path: 5 × (500ms + 100ms RTT) = **3s total** ✅ **Beats target!**

**Decision**: Implement and benchmark sparse routing table as Week 4 deliverable

---

## 📋 Week 4 Action Plan

### Day 1: Accept CPU Baseline
- [x] Document current FHE performance (2.5s/hop)
- [ ] Update `docs/CURRENT_STATUS.md` with CPU limitations
- [ ] Write GPU validation plan (defer to Week 8)

### Day 2-3: Sparse Routing Table
- [ ] Implement `FheEngine::lookup_sparse_table()` (1-entry lookups)
- [ ] Modify `PhantomPacket` to support sparse format
- [ ] Write benchmark comparing full vs sparse

### Day 4-5: Multi-Round Protocol
- [ ] Implement sequential forwarding (node → node → node)
- [ ] Add network latency simulation (50-200ms RTT)
- [ ] Benchmark: Sparse + RTT vs Full path

### Day 6: Analysis & Decision
- [ ] Compare performance: Full (12.5s) vs Sparse (3s est.)
- [ ] Validate security properties preserved
- [ ] Document trade-offs

### Day 7: Documentation & Next Steps
- [ ] Write `docs/SPARSE_ROUTING.md` (design doc)
- [ ] Update architecture with sparse routing option
- [ ] Begin Week 5: Full Node CLI

---

## 🎉 Expected Week 4 Outcome

### Best Case: Sparse Routing Works
- **Performance**: 3s for 5-hop routing ✅ (beats <4s target!)
- **Method**: Sparse tables + multi-round protocol
- **Trade-off**: Network latency dependency (acceptable for v1.0)

### Worst Case: Sparse Routing Fails
- **Performance**: 12.5s for 5-hop routing (2.6x slower than target)
- **Fallback**: Accept CPU baseline, document GPU requirements
- **v1.0 Strategy**: Ship protocol, let GPU community optimize

### Realistic Outcome: Hybrid Approach
- **CPU nodes**: 12.5s latency (sparse tables if network allows)
- **GPU nodes**: 1.5s latency (community-contributed, future)
- **v1.0 Deployment**: Mixed network (CPU + GPU nodes)

---

## 💡 Key Insight

**PHANTOM's revolutionary value is oblivious routing, not speed.**

Even at 12.5s latency, PHANTOM provides a security property that **does not exist** in:
- Tor (onion routing leaks hop count)
- I2P (garlic routing leaks partial paths)
- Nym (mixnets leak timing metadata)

**Comparison**:
- **Tor**: 5s latency, metadata leakage = **vulnerable**
- **PHANTOM**: 12.5s latency, FHE oblivious routing = **architecturally secure**

**Conclusion**: Ship PHANTOM v1.0 with CPU baseline. Speed is secondary to security.

---

## 📚 References

- **TFHE-rs Performance**: https://docs.zama.ai/tfhe-rs/benchmarks
- **GPU Acceleration**: Zama CONCRETE (10-20x speedup with CUDA)
- **Sparse Routing**: Inspired by Tor's onion routing (adapted for FHE)

---

**Status**: Week 4 strategy revised - pragmatic CPU approach + sparse routing experiment  
**Next**: Implement sparse routing table (Days 2-6), then Week 5: Full Node CLI
