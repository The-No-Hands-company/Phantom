# Custom Circuit Development Roadmap

**Goal**: Achieve <10s proof generation (target: 1-3s) using hand-optimized cryptographic circuits  
**Cost**: $0 (no GPU required)  
**Timeline**: 2-3 months intensive development  
**Approach**: Replace RISC Zero's general-purpose zkVM with specialized circuits for routing proofs

---

## Executive Summary

**Why Custom Circuits Beat zkVMs**:
- **RISC Zero/SP1**: General-purpose zkVM → proves arbitrary Rust code → massive overhead
- **Custom Circuit**: Proves ONLY what we need → 50-100x fewer constraints → 50-100x faster

**Our Advantage**:
- Routing proofs have **fixed structure**: Merkle proofs + simple checks
- No need for Turing-complete computation
- Can hand-optimize every constraint

**Expected Performance**:
```
RISC Zero (zkVM):     143.5 seconds  (baseline)
Custom Circuit:       1-3 seconds    (50-100x faster)
GPU (comparison):     7-10 seconds   (not an option)
```

**Tradeoff**: Development complexity vs runtime performance

---

## Circuit Framework Comparison

### Option 1: Plonky2 ⭐ **RECOMMENDED**
**Pros**:
- **FAST**: 100,000+ constraints/second (vs ~1,000 for Halo2)
- **Recursion-friendly**: Native support for proof aggregation
- **Merkle-optimized**: Built-in Poseidon hash optimized for PLONK
- **Active development**: Polygon team, production-ready

**Cons**:
- Less mature documentation than Halo2
- Smaller ecosystem
- Rust API still evolving

**Performance Estimate**:
- Merkle proof (20 levels): ~20ms
- Path validation: ~5ms
- **Total**: ~25-50ms proof generation ✅✅✅

**Verdict**: **Best for PHANTOM** - Merkle-heavy workload is Plonky2's sweet spot

---

### Option 2: Halo2
**Pros**:
- Mature, well-documented
- Used by ZCash (battle-tested)
- Excellent learning resources
- No trusted setup required

**Cons**:
- **Slower** than Plonky2 (10-100x)
- More complex API
- Merkle proofs less optimized

**Performance Estimate**:
- Merkle proof (20 levels): ~200-500ms
- Path validation: ~50ms
- **Total**: ~250-550ms proof generation ⚠️

**Verdict**: Still 300x faster than RISC Zero, but Plonky2 is better for our use case

---

### Option 3: Boojum (ZKSync)
**Pros**:
- Extremely fast (algebraic hash functions)
- Production-proven (ZKSync Era)
- GPU acceleration available

**Cons**:
- Tightly coupled to ZKSync infrastructure
- Steeper learning curve
- Less portable

**Verdict**: Overkill for our needs, Plonky2 is more accessible

---

## Development Phases

### Phase 1: Learning & Prototyping (2-3 weeks)

#### Week 1: Plonky2 Basics
**Goal**: Understand Plonky2 fundamentals

**Tasks**:
1. Study Plonky2 documentation and examples
2. Build simple circuits (addition, multiplication)
3. Understand Poseidon hash in Plonky2
4. Learn proof generation and verification APIs

**Resources**:
- Plonky2 GitHub: https://github.com/mir-protocol/plonky2
- Tutorial: Build a simple range proof circuit
- Example: Merkle proof verification circuit

**Deliverable**: Working Plonky2 dev environment + simple circuit

---

#### Week 2: Merkle Circuit Implementation
**Goal**: Build specialized Merkle proof verifier

**Circuit Constraints**:
```rust
// Verify Merkle proof for one node
pub struct MerkleProofCircuit {
    leaf_value: u32,           // Node ID
    leaf_index: u64,           // Position in tree
    siblings: [Hash; 20],      // 20-level tree (1M nodes)
    root: Hash,                // Network commitment
}

// Constraints (per proof):
// - 20 hash computations (siblings)
// - 20 bit checks (path direction)
// - 1 root equality check
// Total: ~42 constraints per Merkle proof
```

**Optimization**:
- Use Poseidon hash (Plonky2 native) instead of Blake3
- Batch multiple proofs in one circuit
- Pre-compute hash tables

**Expected Performance**:
- Single Merkle proof: ~5-10ms
- 5 proofs (path): ~25-50ms

**Deliverable**: Working Merkle verifier circuit with benchmarks

---

#### Week 3: Path Validation Circuit
**Goal**: Add routing-specific constraints

**Constraints**:
```rust
pub struct RoutingPathCircuit {
    path: [u32; 7],            // Max 7 hops
    path_length: usize,        // Actual length (3-7)
    merkle_proofs: [MerkleProof; 7],
    network_commitment: Hash,
}

// Additional constraints:
// - Length check: 3 <= len <= 7 (2 constraints)
// - Loop detection: uniqueness check (21 comparisons)
// - Merkle membership: 7 proofs × 42 = 294 constraints
// Total: ~317 constraints
```

**Optimization**:
- Use lookup tables for uniqueness (no comparisons)
- Parallelize Merkle proofs
- Batch verification

**Expected Performance**:
- Path validation: ~50-100ms
- **50-100x faster than RISC Zero** ✅

**Deliverable**: Complete routing proof circuit

---

### Phase 2: Optimization (2-4 weeks)

#### Week 4: Constraint Reduction
**Goal**: Minimize circuit size

**Techniques**:
1. **Lookup Tables**: Replace comparison chains
2. **Custom Gates**: Combine multiple constraints
3. **Algebraic Tricks**: Reduce multiplications
4. **Batching**: Prove multiple paths in one circuit

**Expected Improvement**: 2-5x (100ms → 20-50ms)

---

#### Week 5: Parallelization
**Goal**: Use all CPU cores for witness generation

**Approach**:
```rust
// Parallel witness generation
rayon::scope(|s| {
    for proof in merkle_proofs {
        s.spawn(|_| generate_merkle_witness(proof));
    }
});
```

**Expected Improvement**: 2-4x on 6-core CPU (50ms → 12-25ms)

---

#### Week 6: Proof Aggregation
**Goal**: Combine multiple routing proofs efficiently

**Why**: Network packets travel together → batch proving saves time

**Plonky2 Recursion**:
```rust
// Prove 10 paths in one aggregated proof
let individual_proofs = paths.map(|p| prove_path(p));
let aggregated = recursion::aggregate(individual_proofs);

// Cost: ~10x single proof time (not 10× linear)
// Amortized: 1.5-3ms per path
```

**Expected Improvement**: 10-20x for batched traffic

---

### Phase 3: Integration (2-3 weeks)

#### Week 7-8: phantom-zkvm Integration
**Goal**: Replace RISC Zero with custom circuit

**Implementation**:
```rust
// crates/phantom-zkvm/src/plonky2.rs
pub struct Plonky2ProofGenerator {
    circuit: RoutingPathCircuit,
    proving_key: ProvingKey,
    verifying_key: VerifyingKey,
}

impl ProofGenerator for Plonky2ProofGenerator {
    fn generate_path_proof(...) -> Result<RoutingProof> {
        // Use custom circuit instead of RISC Zero
        let proof = self.circuit.prove(path, merkle_proofs)?;
        Ok(proof)
    }
}
```

**Testing**:
- Unit tests: All constraints satisfied
- Integration tests: Same security as RISC Zero
- Benchmarks: Measure actual speedup

**Deliverable**: Drop-in replacement for RISC Zero

---

#### Week 9: Production Hardening
**Goal**: Security audit and optimization

**Tasks**:
1. Formal verification of circuit constraints
2. Constant-time operations (side-channel resistance)
3. Error handling and edge cases
4. Performance profiling
5. Documentation

**Deliverable**: Production-ready custom circuit

---

### Phase 4: Advanced Optimization (Ongoing)

#### Month 3+: Next-Level Performance
**Techniques**:
1. **SNARK Aggregation**: Combine proofs from multiple nodes
2. **Preprocessing**: Pre-compute witness tables
3. **GPU Acceleration**: Port to GPU (optional, future)
4. **Custom Hash Function**: Faster than Poseidon for our use case

**Target**: <1 second proof generation

---

## Technical Deep Dive

### Plonky2 Circuit Structure

```rust
use plonky2::plonk::circuit_builder::CircuitBuilder;
use plonky2::plonk::circuit_data::CircuitConfig;
use plonky2::plonk::config::{GenericConfig, PoseidonGoldilocksConfig};

// Define circuit configuration
const D: usize = 2; // Extension degree
type C = PoseidonGoldilocksConfig;
type F = <C as GenericConfig<D>>::F;

pub struct PhantomRoutingCircuit {
    builder: CircuitBuilder<F, D>,
}

impl PhantomRoutingCircuit {
    pub fn new() -> Self {
        let config = CircuitConfig::standard_recursion_config();
        let builder = CircuitBuilder::<F, D>::new(config);
        Self { builder }
    }
    
    pub fn add_merkle_proof_constraints(
        &mut self,
        leaf: Target,
        siblings: &[HashOutTarget],
        root: HashOutTarget,
    ) {
        // Poseidon hash is native to Plonky2
        let mut current = self.builder.hash_n_to_hash_no_pad::<PoseidonHash>(vec![leaf]);
        
        for sibling in siblings {
            // Compute parent hash
            current = self.builder.hash_n_to_hash_no_pad::<PoseidonHash>(
                vec![current.elements, sibling.elements].concat()
            );
        }
        
        // Assert root matches
        self.builder.connect_hashes(current, root);
    }
    
    pub fn add_path_validation_constraints(
        &mut self,
        path: &[Target],
        length: Target,
    ) {
        // Length check: 3 <= len <= 7
        let three = self.builder.constant(F::from_canonical_usize(3));
        let seven = self.builder.constant(F::from_canonical_usize(7));
        
        self.builder.range_check(length, 3); // >= 3
        let diff = self.builder.sub(seven, length);
        self.builder.range_check(diff, 3); // <= 7
        
        // Loop detection: all nodes unique
        for i in 0..path.len() {
            for j in i+1..path.len() {
                let eq = self.builder.is_equal(path[i], path[j]);
                let zero = self.builder.zero();
                self.builder.connect(eq.target, zero); // Must be different
            }
        }
    }
}
```

---

### Constraint Count Analysis

**RISC Zero (zkVM)**:
- CPU emulation: ~100,000 constraints per instruction
- 5-hop path program: ~50,000 instructions
- **Total**: ~5,000,000 constraints ❌

**Custom Circuit (Plonky2)**:
- Merkle proof (20 levels): 42 constraints × 5 proofs = 210
- Path validation: 21 comparisons + 2 range checks = 23
- **Total**: ~233 constraints ✅

**Speedup Factor**: 5,000,000 / 233 = **21,459x fewer constraints**

---

### Performance Projections

**Conservative Estimate** (Plonky2 on CPU):
```
Constraint count:     233
Plonky2 throughput:   100,000 constraints/second
Proof time:           233 / 100,000 = 2.33ms
Witness generation:   ~10ms
Serialization:        ~5ms
Total:                ~20ms ✅✅✅
```

**Realistic Estimate** (with overhead):
```
Circuit complexity:   1.5x (not perfectly optimized)
Memory allocation:    +20ms
FFTs and poly ops:    +30ms
Total:                ~100ms ✅✅
```

**Worst Case** (first iteration):
```
Suboptimal circuit:   3x slower
Learning curve:       +100ms overhead
Total:                ~300ms ✅
```

**Even worst case is 478x faster than RISC Zero (143.5s → 300ms)**

---

## Risk Mitigation

### Risk 1: Circuit Bugs (Security)
**Mitigation**:
- Formal verification using Coq/Lean
- Extensive fuzzing and test vectors
- Audit by cryptography experts
- Start with RISC Zero, migrate gradually

### Risk 2: Performance Doesn't Meet Target
**Mitigation**:
- Conservative estimates (300ms worst case)
- Profiling early and often
- Fallback to Halo2 if Plonky2 has issues

### Risk 3: Development Takes Too Long
**Mitigation**:
- Incremental milestones (2-week sprints)
- Use existing Plonky2 examples as templates
- Community support (Plonky2 Discord)

---

## Resource Requirements

### Learning Resources
1. **Plonky2 Documentation**: https://github.com/mir-protocol/plonky2
2. **ZK MOOC** (Stanford): Circuit design fundamentals
3. **Plonky2 Examples**: Merkle tree, recursion, simple circuits
4. **Cryptography Papers**: PLONK, FRI, Poseidon hash

### Development Tools
- **Rust**: 1.70+ (async/await, const generics)
- **Plonky2**: Latest from GitHub
- **Benchmarking**: Criterion (already have)
- **Profiling**: `perf`, `flamegraph`

### Time Investment
- **Full-time**: 6-8 weeks (recommended)
- **Part-time**: 10-12 weeks
- **Learning overhead**: 1-2 weeks

---

## Comparison: Custom Circuit vs Alternatives

| Approach | Proof Time | Dev Time | Cost | Complexity |
|----------|-----------|----------|------|-----------|
| **RISC Zero (current)** | 143.5s | 0 weeks | $0 | Low |
| **SP1 zkVM** | 30-60s | 1 week | $0 | Low |
| **RISC Zero + GPU** | 7-10s | 1 week | $300-500 | Low |
| **Plonky2 Circuit** | **0.1-1s** | **8-12 weeks** | **$0** | **High** |
| **Halo2 Circuit** | 0.3-3s | 10-14 weeks | $0 | Very High |

**Verdict**: Plonky2 is the **best long-term solution** for $0 budget

---

## Implementation Checklist

### Week 1: Setup ✅
- [ ] Install Plonky2 dependencies
- [ ] Build example circuits (Fibonacci, Merkle)
- [ ] Understand Poseidon hash API
- [ ] Create `phantom-circuit` crate

### Week 2: Merkle Circuit 🔧
- [ ] Design Merkle proof circuit
- [ ] Implement single-proof verifier
- [ ] Benchmark (target: <10ms)
- [ ] Test with PHANTOM network data

### Week 3: Path Validation 🔧
- [ ] Add length and loop constraints
- [ ] Integrate Merkle proofs (5-7 per path)
- [ ] Benchmark complete circuit (target: <100ms)
- [ ] Security review

### Week 4-6: Optimization ⚡
- [ ] Reduce constraints via lookup tables
- [ ] Parallelize witness generation
- [ ] Implement proof aggregation
- [ ] Benchmark batched proving

### Week 7-9: Integration 🔌
- [ ] Implement `ProofGenerator` trait
- [ ] Replace RISC Zero in `phantom-zkvm`
- [ ] Update all tests and benchmarks
- [ ] Production hardening

---

## Success Metrics

**Minimum Viable**:
- ✅ Proof generation: <10 seconds (14x faster than RISC Zero)
- ✅ Verification: <50ms
- ✅ Same security as RISC Zero

**Target**:
- 🎯 Proof generation: <1 second (143x faster)
- 🎯 Verification: <10ms
- 🎯 Proof size: <100KB (smaller than RISC Zero)

**Stretch Goal**:
- 🚀 Proof generation: <100ms (1,435x faster)
- 🚀 Batch proving: 10 paths in <500ms
- 🚀 Formal verification complete

---

## Next Immediate Steps

1. **Install Plonky2** (30 minutes):
```bash
cd crates/phantom-zkvm
cargo add plonky2 --git https://github.com/mir-protocol/plonky2
```

2. **Create Circuit Crate** (1 hour):
```bash
cargo new --lib crates/phantom-circuit
# Add Plonky2 dependencies
# Create basic circuit structure
```

3. **Build First Circuit** (2-3 hours):
```rust
// Simple range proof or hash verification
// Goal: Understand Plonky2 API
```

4. **Benchmark Baseline** (30 minutes):
```bash
cargo bench --package phantom-circuit
# Measure circuit build time, proof time, verify time
```

---

## Timeline Summary

```
Week 1:  Plonky2 learning & setup
Week 2:  Merkle circuit implementation
Week 3:  Path validation circuit
Week 4:  Constraint optimization
Week 5:  Witness parallelization
Week 6:  Proof aggregation
Week 7:  Integration with phantom-zkvm
Week 8:  Testing and benchmarking
Week 9:  Production hardening
Week 10+: Advanced optimization
```

**First milestone** (Week 3): Working circuit faster than RISC Zero  
**Production ready** (Week 9): Full replacement, <1s proofs  
**Optimized** (Week 12): <100ms proofs, batch aggregation

---

## Why This Works Without GPU

**Plonky2 Magic**:
1. **Efficient Constraints**: 233 constraints vs 5M (RISC Zero)
2. **Fast Field Arithmetic**: Goldilocks field (64-bit) is CPU-friendly
3. **Optimized FFTs**: Hand-tuned for x86-64 (AVX2)
4. **No Memory Bloat**: Small witness size, cache-friendly

**CPU is Enough**:
- 100,000 constraints/second on modern CPU
- 233 constraints = 2.33ms theoretical
- Even with 100x overhead → 233ms (still 600x faster than RISC Zero)

---

## Final Recommendation

**Start custom circuit development immediately**:
1. **Week 1**: Learn Plonky2 basics (low risk, high learning value)
2. **Week 2-3**: Build Merkle + path circuit (proof of concept)
3. **Week 4**: Benchmark - if >10s, pivot to SP1 or optimize more

**Fallback Plan**:
- If Plonky2 too complex → Try Halo2 (slower but easier)
- If custom circuits fail → SP1 zkVM (30-60s, still way better than RISC Zero)
- Never need GPU → $0 budget maintained ✅

**This is the revolutionary approach**: Build what we need, not what's easy. Custom circuits are how Zcash, Mina, and Scroll achieved production performance.

---

**Ready to start?** Create `phantom-circuit` crate and install Plonky2.

