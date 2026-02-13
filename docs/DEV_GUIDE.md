# PHANTOM Development Guide

## Quick Start

### Prerequisites

1. **Rust toolchain** (1.75+)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update
```

2. **Build dependencies**
```bash
# Ubuntu/Debian
sudo apt update
sudo apt install build-essential pkg-config libssl-dev clang

# macOS
brew install llvm pkg-config openssl
```

3. **Optional: LaTeX** (for whitepaper compilation)
```bash
sudo apt install texlive-full  # Ubuntu
brew install --cask mactex      # macOS
```

### Building PHANTOM

```bash
# Clone repository
cd /run/media/zajferx/Data/dev/The-No-hands-Company/projects/Testing/encryptiondecentralization

# Build all crates
cargo build --release

# Run tests
cargo test --all

# Run benchmarks
cargo bench
```

### Project Structure

```
phantom/
├── Cargo.toml                   # Workspace configuration
├── README.md                    # Project overview
├── docs/
│   ├── architecture.md          # System design
│   ├── whitepaper/              # Academic paper
│   │   ├── main.tex
│   │   └── references.bib
│   └── DEV_GUIDE.md            # This file
├── crates/
│   ├── phantom-crypto/          # Cryptographic primitives
│   │   ├── src/
│   │   │   ├── pq.rs           # Post-quantum crypto (Kyber, Dilithium)
│   │   │   ├── fhe.rs          # Fully homomorphic encryption (TFHE)
│   │   │   ├── zk.rs           # Zero-knowledge proofs (Halo2)
│   │   │   └── primitives.rs   # Hash functions, utilities
│   │   └── Cargo.toml
│   ├── phantom-core/            # [TODO] Core protocol logic
│   ├── phantom-routing/         # [TODO] Oblivious routing engine
│   ├── phantom-zkvm/            # [TODO] SP1/RISC Zero integration
│   ├── phantom-discovery/       # [TODO] Node discovery (zk-RLN)
│   └── phantom-node/            # [TODO] Full node implementation
└── examples/                    # [TODO] Example applications
```

## Development Workflow

### 1. Understanding the Codebase

Start with the cryptographic primitives in `phantom-crypto`:

```bash
# Read the code
cd crates/phantom-crypto
cat src/lib.rs       # Module overview
cat src/pq.rs        # Post-quantum key exchange
cat src/fhe.rs       # FHE routing operations
cat src/zk.rs        # Zero-knowledge proofs

# Run tests
cargo test

# See what works
cargo test -- --nocapture
```

### 2. Running Benchmarks

```bash
cd crates/phantom-crypto

# Benchmark PQ crypto
cargo bench --bench pq_benchmarks

# Benchmark FHE operations
cargo bench --bench fhe_benchmarks

# Results saved to target/criterion/
```

### 3. Next Implementation Steps

The cryptographic foundation is ready. Here's what to build next:

#### Phase 1: Core Protocol (Week 1-2)

Create `crates/phantom-core`:

```rust
// phantom-core/src/packet.rs
pub struct PhantomPacket {
    pub routing_blob: EncryptedValue,  // FHE-encrypted routing table
    pub path_proof: Proof,              // zk-SNARK of path validity
    pub payload: Vec<u8>,               // Encrypted payload
    pub nullifier: [u8; 32],            // Rate-limit nullifier
}

impl PhantomPacket {
    pub fn construct(
        path: Vec<NodeId>,
        network_graph: &NetworkGraph,
        payload: Vec<u8>,
        fhe_engine: &FheEngine,
    ) -> Result<Self> {
        // TODO: Implement packet construction
    }
}
```

#### Phase 2: Routing Engine (Week 3-4)

Create `crates/phantom-routing`:

```rust
// phantom-routing/src/forwarder.rs
pub struct ObliviousForwarder {
    my_node_id: u32,
    fhe_engine: FheEngine,
    network_graph: NetworkGraph,
}

impl ObliviousForwarder {
    pub async fn forward_packet(&self, packet: PhantomPacket) -> Result<()> {
        // 1. Verify zk-proof
        // 2. FHE lookup: should I forward this?
        // 3. If yes, broadcast to neighbors
    }
}
```

#### Phase 3: zkVM Integration (Week 5-6)

Create `crates/phantom-zkvm`:

```rust
// Use RISC Zero to generate routing proofs
use risc0_zkvm::{default_prover, ExecutorEnv};

pub fn generate_routing_proof(
    path: Vec<u32>,
    network_commitment: Hash,
) -> Result<Proof> {
    // Run zkVM to prove path validity
    let env = ExecutorEnv::builder()
        .add_input(&path)
        .add_input(&network_commitment)
        .build()?;
    
    let prover = default_prover();
    let receipt = prover.prove_elf(env, ROUTING_ELF)?;
    
    Ok(Proof { data: receipt.journal })
}
```

#### Phase 4: Node Discovery (Week 7-8)

Create `crates/phantom-discovery`:

```rust
// Anonymous node discovery via zk-set membership
pub struct NodeDiscovery {
    semaphore_identity: Identity,
    known_nodes: Vec<AnonymousNodeInfo>,
}

impl NodeDiscovery {
    pub async fn announce(&self) -> Result<()> {
        // Generate zk-proof: "I'm in the approved set"
        let proof = self.generate_membership_proof()?;
        
        // Broadcast announcement (no IP, no identifiable info)
        self.gossip_network.broadcast(NodeAnnouncement {
            capabilities: self.capabilities(),
            proof,
            nullifier: self.compute_nullifier(),
        }).await
    }
}
```

## Testing Strategy

### Unit Tests

Each module has tests in `#[cfg(test)]` blocks:

```bash
# Test specific module
cargo test --package phantom-crypto pq::tests

# Test with output
cargo test -- --nocapture --test-threads=1
```

### Integration Tests

Create `tests/integration_test.rs`:

```rust
#[tokio::test]
async fn test_end_to_end_packet_forwarding() {
    // Create 5-node network
    let nodes = create_test_network(5).await;
    
    // Send packet from node 0 to node 4
    let packet = construct_packet(...);
    nodes[0].send(packet).await.unwrap();
    
    // Verify delivery
    let received = nodes[4].receive().await.unwrap();
    assert_eq!(received.payload, original_payload);
}
```

### Benchmarking Critical Paths

```rust
// benches/routing_benchmark.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_packet_forwarding(c: &mut Criterion) {
    let engine = FheEngine::generate_keys();
    let packet = create_test_packet();
    
    c.bench_function("forward_packet", |b| {
        b.iter(|| {
            forward_packet(black_box(&packet), black_box(&engine))
        })
    });
}

criterion_group!(benches, bench_packet_forwarding);
criterion_main!(benches);
```

## Performance Optimization

### FHE Acceleration

Current bottleneck: FHE operations take ~200ms per hop.

**Optimization strategies:**

1. **GPU acceleration** (CUDA/OpenCL)
   - TFHE-rs supports GPU via `cuda` feature
   - Target: <50ms per hop

2. **Proof batching**
   - Generate one proof for N packets
   - Amortize 500ms cost → 5ms/packet

3. **Pipelined forwarding**
   - Don't wait for each hop to complete
   - Stream packets through the network

### Memory Optimization

FHE operations are memory-intensive. Monitor with:

```bash
# Run with memory profiling
cargo build --release
valgrind --tool=massif ./target/release/phantom-node

# Analyze heap usage
ms_print massif.out.* | less
```

## Debugging Tips

### Enable detailed logging

```bash
export RUST_LOG=phantom=trace,phantom_crypto=debug
cargo run
```

### Using `dbg!()` macro

```rust
let routing_blob = construct_routing_blob(path);
dbg!(&routing_blob);  // Prints debug representation
```

### Conditional compilation for debugging

```rust
#[cfg(debug_assertions)]
println!("Debug: FHE lookup took {:?}", duration);
```

## Contributing

### Code Style

We use `rustfmt` and `clippy`:

```bash
# Format code
cargo fmt --all

# Lint
cargo clippy --all-targets --all-features -- -D warnings
```

### Commit Messages

Follow conventional commits:

```
feat(crypto): Add SPHINCS+ hash-based signatures
fix(routing): Correct FHE lookup for empty tables
docs(whitepaper): Add security proof for Theorem 2
perf(fhe): Optimize bootstrapping with GPU acceleration
```

### Pull Request Process

1. Fork the repository
2. Create feature branch: `git checkout -b feat/amazing-feature`
3. Commit changes: `git commit -m 'feat: Add amazing feature'`
4. Run tests: `cargo test --all`
5. Push branch: `git push origin feat/amazing-feature`
6. Open pull request with description

## Research & Academic Collaboration

### Compiling the Whitepaper

```bash
cd docs/whitepaper
pdflatex main.tex
bibtex main
pdflatex main.tex
pdflatex main.tex

# Output: main.pdf
```

### Adding Security Proofs

Edit `docs/whitepaper/main.tex` and add formal proofs in Appendix A.

Use standard cryptographic notation:
- $\mathsf{Enc}$ for encryption
- $\mathsf{negl}(\lambda)$ for negligible function
- Theorem/Proof environments

## Grant Applications

We're targeting:

1. **Ethereum Foundation - Privacy & Scaling Explorations**
   - Focus: zkVM integration + FHE acceleration
   - Amount: $100k-$500k
   - Deadline: Rolling

2. **Zcash Foundation Grants**
   - Focus: Shielded routing integration
   - Amount: $50k-$250k
   - Deadline: Quarterly

3. **Protocol Labs Research Grants**
   - Focus: Decentralized infrastructure
   - Amount: $25k-$100k
   - Deadline: Rolling

## FAQ

**Q: Why Rust?**  
A: Memory safety without garbage collection. Critical for crypto code.

**Q: Why not use Tor's code as a base?**  
A: We're building a fundamentally different primitive. Tor is TCP-based onion routing. PHANTOM is FHE-based oblivious routing.

**Q: Is this production-ready?**  
A: No. This is research-grade code (v0.1). Don't use for real anonymity yet.

**Q: How can I help?**  
A: We need cryptographers, distributed systems engineers, and formal verification experts. See CONTRIBUTING.md.

**Q: What's the endgame?**  
A: A new anonymity primitive that makes today's mix networks obsolete, the way Bitcoin made DigiCash obsolete.

---

**Next:** Read `docs/architecture.md` for the full system design.
