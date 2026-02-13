# SP1 zkVM Evaluation Guide

**Goal**: Determine if SP1 provides better CPU performance than RISC Zero for PHANTOM routing proofs.

**Expected Outcome**: 2-5x faster on CPU (30-60 seconds vs 143.5 seconds)

---

## Why SP1?

### Claims from SP1 Team
1. **10-100x faster** than RISC Zero on CPU for many workloads
2. **Better Rust integration** - more ergonomic guest API
3. **Efficient circuit compilation** - optimized constraint system
4. **Open-source** - auditable, community-driven

### Skepticism
- "10-100x" is marketing speak - real-world varies by workload
- Our workload: Merkle proof verification + constraint checking (not compute-heavy)
- Realistic expectation: 2-5x improvement on CPU

---

## Implementation Plan

### Phase 1: SP1 Setup (30 minutes)
```bash
# Install SP1 toolchain
curl -L https://sp1.succinct.xyz | bash
sp1up

# Add SP1 dependencies to Cargo.toml
[dependencies]
sp1-zkvm = "3.0"  # Latest version

[dev-dependencies]
sp1-sdk = "3.0"
```

### Phase 2: Port Guest Program (2-3 hours)
Convert `phantom-route-validator` from RISC Zero to SP1:

**RISC Zero guest** (`methods/guest/src/main.rs`):
```rust
use risc0_zkvm::guest::env;

fn main() {
    let public_inputs: PublicInputs = env::read();
    let routing_path: RoutingPath = env::read();
    let merkle_proofs: Vec<MerkleProof> = env::read();
    
    // Constraints...
    
    env::commit(&public_inputs);
}
```

**SP1 equivalent**:
```rust
sp1_zkvm::entrypoint!(main);

use sp1_zkvm::io::*;

pub fn main() {
    let public_inputs: PublicInputs = read();
    let routing_path: RoutingPath = read();
    let merkle_proofs: Vec<MerkleProof> = read();
    
    // Same constraints (no changes needed)
    
    commit(&public_inputs);
}
```

**Key Differences**:
- `risc0_zkvm::guest::env` → `sp1_zkvm::io`
- `env::read()` → `read()`
- `env::commit()` → `commit()`
- Entrypoint macro: `sp1_zkvm::entrypoint!` instead of RISC Zero's default

**Effort**: Low - mostly find-and-replace

---

### Phase 3: Host Integration (1-2 hours)
Create `phantom-zkvm/src/sp1.rs`:

```rust
use sp1_sdk::{ProverClient, SP1Stdin, SP1ProofKind};
use phantom_core::proof::{ProofGenerator, RoutingProof, PublicInputs};

pub struct Sp1ProofGenerator {
    client: ProverClient,
}

impl Sp1ProofGenerator {
    pub fn new() -> Self {
        Self {
            client: ProverClient::new(),
        }
    }
}

impl ProofGenerator for Sp1ProofGenerator {
    fn generate_path_proof(
        &self,
        path: &[u32],
        network_commitment: &[u8; 32],
        merkle_proofs: &[MerkleProof],
    ) -> Result<RoutingProof, anyhow::Error> {
        // Create stdin for guest
        let mut stdin = SP1Stdin::new();
        stdin.write(&public_inputs);
        stdin.write(&routing_path);
        stdin.write(&merkle_proofs);
        
        // Generate proof (SP1 chooses optimal proof system)
        let (pk, vk) = self.client.setup(PHANTOM_SP1_ELF);
        let proof = self.client.prove(&pk, stdin).run()?;
        
        // Extract public outputs
        let public_inputs: PublicInputs = proof.public_values.read();
        
        Ok(RoutingProof {
            proof_data: bincode::serialize(&proof)?,
            public_inputs,
        })
    }
    
    fn verify_path_proof(
        &self,
        proof: &RoutingProof,
        network_commitment: &[u8; 32],
    ) -> Result<bool, anyhow::Error> {
        let sp1_proof: SP1Proof = bincode::deserialize(&proof.proof_data)?;
        
        // Verify proof
        self.client.verify(&sp1_proof, &vk)?;
        
        // Check commitment matches
        Ok(proof.public_inputs.network_commitment == *network_commitment)
    }
}
```

---

### Phase 4: Benchmarking (30 minutes)
Create `examples/sp1_demo.rs` (copy from `risc0_demo.rs`):

```rust
use phantom_zkvm::Sp1ProofGenerator;

fn main() {
    let generator = Sp1ProofGenerator::new();
    
    // Same test as RISC Zero demo
    let start = std::time::Instant::now();
    let proof = generator.generate_proof(&path, &merkle_proofs, &commitment)?;
    let proof_time = start.elapsed();
    
    println!("SP1 proof time: {:?}", proof_time);
}
```

**Run benchmark**:
```bash
cargo run --example sp1_demo --release
```

**Compare**:
| zkVM | CPU Time | GPU Time | Proof Size | Verification |
|------|---------|---------|-----------|--------------|
| RISC Zero | 143.5s | ~7-10s (CUDA) | 275KB | 32.9ms |
| SP1 | **?** | **?** | **?** | **?** |

---

### Phase 5: Decision Matrix (15 minutes)

**If SP1 is faster on CPU**:
- Use SP1 for testnet deployment (no GPU required)
- Evaluate SP1 GPU support later
- Keep RISC Zero as fallback

**If SP1 is comparable or slower**:
- Stick with RISC Zero (more mature, better docs)
- Focus on GPU acquisition for production
- SP1 not worth the migration effort

**If SP1 has issues**:
- Guest compilation errors → Not worth debugging
- Proof verification failures → Security risk
- Poor documentation → High maintenance burden

---

## Success Criteria

✅ **Must Have**:
1. Guest program compiles without errors
2. Proofs verify correctly (same security as RISC Zero)
3. Performance measurement completes

⚠️ **Nice to Have**:
4. 2x faster than RISC Zero on CPU (143s → 70s)
5. Smaller proof size (<200KB)
6. Faster verification (<20ms)

❌ **Deal Breakers**:
- Proof generation fails
- Verification is unreliable
- Documentation is insufficient
- Migration takes >1 day

---

## Estimated Timeline

| Phase | Time | Blocker Risk |
|-------|------|-------------|
| SP1 Setup | 30min | Low (well-documented) |
| Port Guest | 2-3h | Medium (API differences) |
| Host Integration | 1-2h | Low (straightforward) |
| Benchmarking | 30min | Low (same test) |
| **Total** | **4-6 hours** | **Medium** |

---

## Alternative: Focus on GPU Instead

**If SP1 evaluation seems risky**:

### Option A: Buy NVIDIA GPU
- **Cost**: $300-500 (RTX 3060 used)
- **Time**: 2-3 days (shipping + setup)
- **Certainty**: High (10-20x speedup guaranteed by RISC Zero)

### Option B: Cloud GPU
- **Cost**: $0.50-1.00/hour (AWS p3.2xlarge)
- **Time**: 1 hour (setup)
- **Certainty**: High (same as Option A)

### Option C: Continue with CPU
- **Cost**: $0
- **Time**: 0 hours
- **Performance**: 143.5s (acceptable for testnet, not mainnet)

---

## Recommendation

**Try SP1 evaluation first** (4-6 hours investment):
- If successful → 2-5x faster on CPU (good for testnet)
- If unsuccessful → Fall back to RISC Zero + GPU plan

**GPU acquisition in parallel**:
- Research RTX 3060/3070 prices
- Prepare budget for hardware ($300-500)
- GPU will be needed for mainnet regardless of SP1 results

---

## Next Steps

1. **Immediate**: Install SP1 toolchain (`sp1up`)
2. **Week 3**: Port guest program and benchmark
3. **Week 4**: If SP1 < 60s, use for testnet. Otherwise, acquire GPU.
4. **Month 2**: Production deployment with GPU (RISC Zero or SP1)

---

**Status**: Ready to start SP1 evaluation  
**Decision Point**: After SP1 benchmark (4-6 hours from now)
