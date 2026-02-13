# PHANTOM Protocol - Codebase Instructions for AI Agents

## Project Vision & Overview
PHANTOM (Protocol for Hardened Anonymous Networking Through Oblivious Mathematics) is a **revolutionary anonymous networking protocol** designed to:
- **Make surveillance mathematically impossible**: Not just hard, but architecturally prevented through cryptography
- **Eliminate metadata leakage**: Nodes route packets they literally cannot decrypt (FHE-based oblivious routing)
- **Provide post-quantum security**: Resistant to both classical and quantum adversaries from day 1
- **Enable democratic economics**: Proof-of-personhood Sybil resistance, no plutocratic staking
- **Obsolete current systems**: Not "better Tor" - a fundamentally new anonymity primitive

**Current State**: Cryptographic foundation complete (PQ crypto, FHE, ZK scaffolding). Building protocol layer: packet construction → oblivious routing → zkVM proofs → anonymous node discovery. Starting with solid primitives, expanding toward full decentralized anonymous network.

## Development Philosophy ⚠️ CRITICAL

**NO SHORTCUTS. NO COMPROMISES.**

**This is a revolutionary cryptographic protocol development project**, not an MVP or prototype. Every feature must be:

- ✅ **Complete** - Full implementation, not placeholders or stubs
- ✅ **Production-ready** - Robust error handling, edge cases covered
- ✅ **No simplifications** - Don't cut corners to "save time" or "make it easier"
- ✅ **No workarounds** - Solve the real problem, not symptoms
- ✅ **No quickfixes** - Proper architectural solutions, not hacks
- ✅ **Real implementations** - Actual working code, not TODO comments

**When implementing features:**
- Do NOT use placeholder implementations ("TODO: implement later")
- Do NOT simplify complex features to make them "easier"
- Do NOT skip edge cases or error conditions
- Do NOT use workarounds instead of proper solutions
- Do NOT leave stub functions with mock data

**Remember**: We're building a protocol that will obsolete Tor, I2P, and Nym. We're creating a new anonymity primitive that makes surveillance architecturally impossible. Act accordingly.

## File Organization Rules ⚠️ CRITICAL

**Always follow this structure when creating files:**

- **`examples/`** - Working demonstration programs showing protocol features
  - Location: `crates/{crate_name}/examples/`
  - Purpose: Demonstrate cryptographic primitives, protocol features, network simulations
  - Naming: Descriptive names like `crypto_demo.rs`, `routing_simulation.rs`

- **`tests/`** - Integration and unit tests
  - Location: `crates/{crate_name}/tests/` for integration tests
  - Location: Inline `#[cfg(test)]` modules for unit tests
  - Purpose: Verify cryptographic correctness, protocol behavior, security properties
  - Naming: `test_*.rs` or inline test modules

- **`benches/`** - Performance benchmarks
  - Location: `crates/{crate_name}/benches/`
  - Purpose: Measure FHE operations, routing latency, proof generation time
  - Naming: `*_benchmarks.rs`

- **`crates/*/src/`** - Implementation code ONLY
  - Organized by module: `pq.rs` (post-quantum), `fhe.rs` (FHE), `zk.rs` (zero-knowledge)
  - No test files in src/ (use inline `#[cfg(test)]` modules)

- **`docs/`** - Documentation and research
  - Architecture docs, whitepapers, specifications, development guides
  - Allowed: `architecture.md`, `whitepaper/`, `DEV_GUIDE.md`, etc.

- **Root directory** - Workspace configuration ONLY
  - Allowed: `README.md`, `GETTING_STARTED.md`, `Cargo.toml`, `.gitignore`, etc.
  - NOT allowed: Source code, test files, example programs

**Before creating ANY file, determine correct location using these rules.**

## Architecture & Core Components

### Protocol Pipeline
```
Application → Packet Construction → FHE Routing Blob → zk-Proof Generation → Network Transmission
                                            ↓
                               Oblivious Forwarding (FHE evaluation)
                                            ↓
                               Proof Verification (zkVM)
                                            ↓
                               Next Hop (without metadata leakage)
```

### Key Crates (crates/)
- **`phantom-crypto/`**: Cryptographic primitives foundation
  - `pq.rs`: Post-quantum key exchange (Kyber-1024) and signatures (Dilithium-5)
  - `fhe.rs`: Fully Homomorphic Encryption using TFHE-rs for oblivious routing
  - `zk.rs`: Zero-knowledge proof system (Halo2-ready, RLN for rate limiting)
  - `primitives.rs`: Hash functions, key derivation, constant-time operations

- **`phantom-core/`**: Protocol data structures
  - `packet.rs`: PHANTOM packet format (FHE routing blob + zk-proof + payload + nullifier)
  - `network.rs`: Network graph, topology management, path selection

- **`phantom-routing/`**: Oblivious packet forwarding engine
  - `forwarder.rs`: FHE-based routing without metadata disclosure

- **`phantom-zkvm/`**: Zero-knowledge virtual machine integration
  - RISC Zero or SP1 for proving routing correctness without revealing paths

- **`phantom-discovery/`**: Anonymous node discovery
  - zk-set membership proofs (no DHT topology leaks)

- **`phantom-node/`**: Full node implementation
  - CLI, configuration, network stack, complete routing node

### PHANTOM Protocol Essentials
```rust
// Oblivious routing: node forwards without learning the path
let routing_blob = fhe_engine.encrypt_routing_table(path);
let proof = zkvm::prove_path_validity(path, network_commitment);
let packet = PhantomPacket::new(routing_blob, proof, payload, nullifier);

// Node receives packet - evaluates "should I forward this?"
let should_forward = fhe_engine.lookup_routing_table(my_id, packet.routing_blob);
// Node learns: "yes, forward" but NOT "to whom" or "from whom"

// Zero-knowledge proof verification
assert!(zkvm::verify(packet.proof, network_commitment));
```

## Development Workflows

### Building and Testing
```bash
# Build all crates
cargo build --all

# Build in release mode (optimized)
cargo build --all --release

# Run all tests
cargo test --all

# Run specific crate tests
cargo test --package phantom-crypto

# Run benchmarks (WARNING: FHE benchmarks are SLOW)
cargo bench --package phantom-crypto --bench pq_benchmarks    # Fast
cargo bench --package phantom-crypto --bench fhe_benchmarks   # VERY SLOW
```

### Running Demonstrations
```bash
# Cryptographic primitives demo
cargo run --package phantom-crypto --example crypto_demo --release

# Full node (when implemented)
cargo run --bin phantom-node --release

# Network simulation (when implemented)
cargo run --example network_simulation --release -- --nodes 100
```

### Code Quality
```bash
cargo fmt --all              # Format code
cargo clippy --all           # Lint with Clippy
cargo check --all            # Fast compilation check
cargo doc --no-deps --open   # Generate and view documentation
```

## Critical Patterns & Conventions

### 1. Cryptographic Primitive Pattern
When adding new cryptographic operations:
```rust
// 1. Add primitive to appropriate module (pq.rs, fhe.rs, zk.rs)
// 2. Ensure post-quantum security (use NIST PQ standards)
// 3. Write comprehensive unit tests (test vectors, edge cases)
// 4. Add benchmarks to measure performance
// 5. Document security assumptions and threat model
```

### 2. FHE Operations (Oblivious Routing Core)
All FHE operations use TFHE-rs with **integer** feature enabled:
```rust
use tfhe::prelude::*;
use tfhe::{ConfigBuilder, FheUint32, generate_keys, set_server_key};

// Generate keys (expensive, ~2 seconds)
let (client_key, server_key) = generate_keys(ConfigBuilder::default().build());

// Encrypt values
let encrypted = FheUint32::encrypt(value, &client_key);

// Set server key for homomorphic operations
set_server_key(server_key.clone());

// Homomorphic comparison (happens on encrypted data!)
let matches = encrypted_a.eq(&encrypted_b);
let result = matches.if_then_else(&true_val, &false_val);
```

**CRITICAL**: FHE operations are the core innovation. Never simplify or mock them.

### 3. Zero-Knowledge Proof Integration
zkVM proofs use RISC Zero or SP1 (when added):
```rust
// Prove path validity without revealing the path
let proof = zkvm::prove(circuit, proving_key)?;

// Verify proof (fast, ~5ms)
assert!(zkvm::verify(proof, verifying_key)?);
```

Proofs must be **sound** (impossible to forge) and **zero-knowledge** (reveal nothing about witness).

### 4. Packet Construction Pattern
PHANTOM packets have strict structure:
```rust
pub struct PhantomPacket {
    routing_blob: Vec<u8>,    // FHE-encrypted routing table
    path_proof: Vec<u8>,      // zk-SNARK of path validity
    payload: Vec<u8>,         // Encrypted application data
    nullifier: [u8; 32],      // Rate-limiting nullifier (prevents spam)
}
```

**Never** expose plaintext routing information. **Always** include zk-proof.

### 5. Error Handling Philosophy (Cloudflare-Proof)

**CRITICAL**: PHANTOM uses production-ready error handling learned from the Cloudflare outage (Nov 18, 2025).

**Golden Rule**: `panic!` for unrecoverable errors ONLY, `Result<T, E>` for everything else.

✅ **DO**:
- Return `Result<T, ProtocolError>` or `Result<T, CryptoError>` for all library functions
- Use `expect("clear message")` only for "impossible" failures (OOM, system errors)
- Validate inputs and return errors gracefully (no crash-on-invalid-input)
- Test error conditions with `assert!(result.is_err())`

❌ **DON'T**:
- Use `panic!()` for recoverable errors (invalid input, network failures)
- Use `unwrap()` in production code paths
- Crash on malformed data from Byzantine nodes
- Test with `#[should_panic]` for error cases

**Error Categories**:
- **Cryptographic errors**: Key generation failures, invalid ciphertexts, proof verification failures → `CryptoError`
- **Protocol errors**: Malformed packets, invalid routing tables, expired nullifiers → `ProtocolError`
- **Network errors**: Connection failures, timeout, Byzantine node behavior → `NetworkError`

**See**: `docs/ERROR_HANDLING.md` for comprehensive guide with Cloudflare lessons.

### 6. Security-Critical Code Review
Before merging any cryptographic code:
- [ ] Constant-time operations where needed (prevent timing attacks)
- [ ] No secret data in error messages or logs
- [ ] Proper key zeroization after use
- [ ] Side-channel resistance considered
- [ ] Threat model documented

## File Organization Conventions

### Documentation Structure (`docs/`)
- **Specification**: `language_specification.md` (formal grammar reference)
- **Architecture**: `compiler_architecture.md` (pipeline design)
- **Type System**: `type_system.md`, `type_system_summary.md`
- **Module System**: `module_system.md`, `module_system_enhancements.md`
- **Development**: `current_priorities.md`, `ROADMAP.md` (project status)

### Example Programs (`examples/`)
Numbered by complexity: `01_basic_concepts.nlpl` → `21_advanced_type_features.nlpl`
Use these as **integration test references** when validating new features.

### Test Organization (`tests/`)
- `test_lexer.py`: Tokenization tests
- `test_parser.py`: AST generation tests
- `test_interpreter.py`: Execution tests
- `test_stdlib.py`: Standard library tests
- `test_comprehensive_errors.py`: Error reporting validation

## Common Pitfalls

1. **Never simplify FHE operations** → They're slow but necessary. Use real TFHE-rs, not mocks.
2. **Don't skip cryptographic tests** → Security depends on correctness. Test edge cases thoroughly.
3. **Avoid metadata leakage** → PHANTOM's core promise is oblivious routing. Never log/expose routing info.
4. **Post-quantum is non-negotiable** → All crypto must be PQ-secure. Use Kyber/Dilithium/SPHINCS+.
5. **zkVM proofs are sound or worthless** → Use real zkVM (RISC Zero/SP1), not placeholder proofs.
6. **Constant-time operations matter** → Timing attacks are real. Use `constant_time_eq` for secrets.
7. **Feature flags for TFHE** → Must enable `integer` feature: `tfhe = { version = "1.4", features = ["integer"] }`
8. **Serialization of FHE ciphertexts** → Use `bincode` for compact serialization, not JSON.
9. **Server key must be set** → Before FHE operations: `set_server_key(server_key.clone())`
10. **Incremental but complete** → Each component must be production-ready before moving to next.

## Vision vs. Implementation Gap

PHANTOM aims to be a complete production-ready anonymous networking protocol, but it's being built **incrementally**:

**Completed** (production-ready):
- ✅ Post-quantum cryptography (Kyber-1024, Dilithium-5)
- ✅ Fully Homomorphic Encryption (TFHE-rs 1.4 with oblivious routing)
- ✅ Zero-knowledge proof scaffolding (Halo2-ready, RLN infrastructure)
- ✅ Cryptographic primitives (hashing, key derivation, constant-time ops)
- ✅ Comprehensive test suite and benchmarks

**In Progress** (next 2-4 weeks):
- 🚧 Core protocol (packet construction, serialization)
- 🚧 Oblivious routing engine (FHE-based forwarding)
- 🚧 Network graph and path selection

**Future Capabilities** (2-6 months):
- ⏳ zkVM integration (RISC Zero or SP1 for routing proofs)
- ⏳ Anonymous node discovery (zk-set membership, no DHT)
- ⏳ Full node implementation (CLI, networking, persistence)
- ⏳ Network simulation and testnet (100-10,000 nodes)
- ⏳ Performance optimization (GPU FHE, proof batching, <500ms latency)
- ⏳ Formal security proofs (machine-checked in Coq/Lean)
- ⏳ Economic layer (proof-of-personhood integration)
- ⏳ Production deployment (mainnet, grants, academic publication)

**Current Approach**: Build incrementally but **never compromise on security**. Each component must be cryptographically sound before moving to the next.

## Current Development State
**Phase: Protocol Layer Implementation** - Cryptographic foundation complete, building networking

- ✅ **Cryptographic Foundation**: Post-quantum (Kyber, Dilithium), FHE (TFHE-rs), ZK (scaffolding)
- ✅ **Oblivious Routing Primitive**: FHE-based routing table lookup working
- ✅ **Test Suite**: 10/11 tests passing, benchmarks for PQ and FHE operations
- ✅ **Documentation**: Architecture, whitepaper, development guides
- 🚧 **Core Protocol**: Packet format defined, construction logic in progress
- 🚧 **Network Graph**: Topology management, path selection algorithms
- ❌ **Routing Engine**: Oblivious forwarding with FHE evaluation (next milestone)
- ❌ **zkVM Integration**: RISC Zero or SP1 for path validity proofs (planned)
- ❌ **Node Discovery**: zk-set membership, anonymous announcements (planned)
- ❌ **Full Node**: CLI, networking stack, persistence layer (planned)
- ❌ **Testnet**: Multi-node simulation and deployment (planned)
- ❌ **Optimizations**: GPU FHE, proof batching, latency <500ms (planned)
- ❌ **Formal Verification**: Machine-checked security proofs in Coq/Lean (future)
- ❌ **Production Deployment**: Mainnet, economic layer, grant funding (future)

**Development Philosophy**: Build incrementally but completely. Each cryptographic component must be production-ready and security-auditable before integration into the protocol layer. No shortcuts on security properties.

## Quick Reference: Key Files by Task

| Task | Files to Modify |
|------|----------------|
| Add PQ crypto primitive | `crates/phantom-crypto/src/pq.rs` + tests |
| Add FHE operation | `crates/phantom-crypto/src/fhe.rs` + tests (use TFHE-rs API) |
| Add ZK proof | `crates/phantom-crypto/src/zk.rs` (Halo2 or zkVM integration) |
| Modify packet format | `crates/phantom-core/src/packet.rs` |
| Add routing logic | `crates/phantom-routing/src/forwarder.rs` |
| Add network primitive | `crates/phantom-core/src/network.rs` |
| Fix FHE bug | Check TFHE `integer` feature enabled, `set_server_key` called |
| Debug crypto issue | Run tests with `--nocapture`, check test vectors |
| Add benchmark | `crates/phantom-crypto/benches/{module}_benchmarks.rs` |
| Update documentation | `docs/architecture.md`, `docs/DEV_GUIDE.md`, `docs/whitepaper/main.tex` |

## References
- **Architecture**: `docs/architecture.md` - Complete system design with threat model, packet format, cryptographic primitives
- **Whitepaper**: `docs/whitepaper/main.tex` - Academic paper with formal security proofs (LaTeX)
- **Development Guide**: `docs/DEV_GUIDE.md` - Implementation roadmap, next steps, grant applications
- **Getting Started**: `GETTING_STARTED.md` - Quick start, testing, running demos
- **Cargo Workspace**: `Cargo.toml` - Workspace configuration, dependency versions

**Academic References**:
- Tor Design (Dingledine et al., 2004)
- Vuvuzela (van den Hooff et al., 2015) - Metadata-hiding messaging
- Loopix (Piotrowska et al., 2017) - Mix network design
- TFHE (Chillotti et al., 2020) - Fast fully homomorphic encryption
- Kyber (Bos et al., 2018) - Post-quantum KEM
- Dilithium (Ducas et al., 2018) - Post-quantum signatures
- RISC Zero zkVM (2024) - Verifiable computation

## Revolutionary Development Philosophy

PHANTOM is not "better Tor". It's a **new anonymity primitive** that makes today's mix networks obsolete.

The way Bitcoin made DigiCash obsolete.  
The way Ethereum made centralized smart contract platforms obsolete.

**PHANTOM makes surveillance architecturally impossible** - not through better incentives or clever economics, but through **mathematics**.

When implementing features, remember:
- We're breaking the anonymity trilemma (anonymity + low latency + Sybil resistance)
- We're building for a future where quantum computers are real
- We're creating a system that resists 90% Byzantine nodes
- We're making surveillance mathematically impossible, not just hard

This is frontier cryptography. Act accordingly.
