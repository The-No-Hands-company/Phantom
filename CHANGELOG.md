# Phantom Protocol — Changelog

All notable changes to the Phantom Protocol core are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/).

---

## [Unreleased]

### Added
- **HTTP bootstrap** — `BootstrapClient` now makes a real HTTP GET to
  `http://{addr}/phantom/v1/network_state`; parses a JSON `NetworkStateResponse`
  (merkle_root, epoch, node_count, last_update); falls back gracefully to a
  local mock when all bootstrap nodes are unreachable so a node can start in
  development or air-gapped environments.
- **Watts-Strogatz small-world topology** — `TopologyGenerator::generate_small_world`
  now implements the full WS model: ring lattice seeding followed by a rewiring
  pass that replaces each directed edge with probability β=0.1 with a random
  non-adjacent target. No self-loops; no duplicate edges.
- **Barabási–Albert scale-free topology** — `TopologyGenerator::generate_scale_free`
  now uses roulette-wheel preferential-attachment selection: each new node picks
  m connection targets with probability proportional to the current degree of each
  candidate, producing a realistic power-law degree distribution.

---

## [0.5.0] — 2026-11-05

### Added
- **Yen's k-shortest-paths** in `NetworkGraph` — BFS helper respects per-iteration
  removed-edge and removed-node sets; min-heap (`BinaryHeap<Reverse<PathByLen>>`)
  ensures the shortest available candidate is always next. Replaces the stub that
  returned only direct 1-hop paths. Five unit tests added.
- **Content-based nullifiers** — `PhantomPacket::construct()` now derives the
  rate-limiting nullifier from `hash(hops ‖ hash(plaintext_payload))` instead of
  `hash(packet_id)`. Identical messages over identical paths always collide;
  re-injecting with a fresh `packet_id` no longer bypasses replay protection.
- **`PhantomPacket::construct_with_proof()`** — new constructor accepting a
  `dyn ProofGenerator`; callers can inject a `Plonky2ProofGenerator` for real
  zkSNARK routing proofs rather than the hash-based dev generator.
- **Parallel FHE routing-table construction** — `PhantomPacket::construct()` now
  calls `FheEngine::build_routing_table()` which uses Rayon for parallel per-hop
  encryption (~5× faster on a 5-hop path vs. serial `encrypt_u32()` calls).
- **`RelayKeyRegistry`** — each relay node can register its own `FheEngine`
  keypair; `phantom-node` initialises the registry at startup and assigns an
  engine per known peer.
- **FHE routing blob in `PhantomPath`** — `PhantomPath::construct_routing_blob(fhe)`
  now produces a real bincode-serialised `Vec<(EncryptedValue, EncryptedValue)>`
  encrypted under the supplied `FheEngine`; node IDs are derived from the first
  four bytes of each nullifier.
- **Plonky2ProofGenerator wired into phantom-node** — `phantom-node` initialises
  a `Plonky2ProofGenerator` at startup and verifies `path_proof` on every locally
  delivered packet via the `ProofGenerator` trait.
- **Announcement proof verification** — `NodeAnnouncement::verify()` now rejects
  proofs whose `network_commitment` doesn't match the local network, and proofs
  whose timestamp is older than one hour.
- **Nightly Rust toolchain pin** — `rust-toolchain.toml` pins the Phantom
  workspace to `nightly` so Plonky2's `#![feature]` gates compile without
  errors on the CI matrix.

### Changed
- `construct_routing_blob` signature changed from `(&self) -> Result<Vec<u8>>`
  to `(&self, fhe: &FheEngine) -> Result<Vec<u8>>`. The old plaintext placeholder
  blob is removed.
- `k_shortest_paths` stub replaced with Yen's algorithm; now returns up to k
  paths instead of always exactly one.

---

## [0.4.0] — 2026-10-06

### Added
- Native shared library `libphantom_wasm.so` (Kyber-1024, Dilithium-5, Blake3)
  compiled via `cargo build --release` in `packages/phantom-sdk/wasm`; Bun FFI
  loads it via `createNativeSDK()` when `PHANTOM_REQUIRE_REAL=1` is set.
- `PhantomApp` class in `phantom-sdk/integration.ts` — per-service identity
  manager that initialises a DID and registers with Nexus-Cloud on startup.
- `ObliviousForwarder` — core FHE-based routing engine; processes packets with
  `process_packet(&packet)` returning `RoutingDecision::{Forward, Deliver, Drop}`.
- Merkle-tree membership proofs via `NetworkGraph::get_membership_proof()`.
- `Plonky2ProofGenerator` and `MembershipCircuit` in phantom-circuit / phantom-zkvm.

### Fixed
- `forwarder.write()` on a read-only method — `phantom-node` now holds
  `Arc<ObliviousForwarder>` directly since `process_packet` takes `&self`.

---

## [0.3.0] — 2026-09-15

### Added
- Sparse routing table (`SparseRoutingTable`) for oblivious O(1) hop lookup.
- Wire format (`WireHeader`, `serialize_packet`, `deserialize_packet`).
- `PacketConstructor` — high-level builder combining KEM encapsulation,
  FHE routing-table encryption and Dilithium-5 signature.
- `FheEngine::oblivious_lookup()` — homomorphic evaluation of routing table
  without decrypting the path.

---

## [0.2.0] — 2026-08-01

### Added
- `PhantomPacket` data structure with routing_blob, path_proof, payload,
  nullifier and packet_id fields.
- `RoutingPath` with hop validation (3–7 hops, no loops).
- Peer selector (`PeerSelector`, `SelectionStrategy`).
- Path builder (`PathBuilder`, `PhantomPath`) with diversity and latency scoring.
- Path validator (`PathValidator`) with configurable quality weights.

---

## [0.1.0] — 2026-07-01

### Added
- Initial Phantom Protocol workspace: phantom-core, phantom-crypto,
  phantom-routing, phantom-discovery, phantom-node, phantom-networking,
  phantom-simulation, phantom-zkvm, phantom-circuit.
- Post-quantum primitives: ML-KEM-1024 key encapsulation, Dilithium-5 signatures,
  Blake3 hashing, ChaCha20-Poly1305 AEAD.
- Fully Homomorphic Encryption via TFHE-rs: `FheEngine`, `EncryptedValue`,
  `ClientKey`, `ServerKey`.
- DID format `did:phantom:user:<blake3-hex>` and `did:phantom:node:<blake3-hex>`.
- `NetworkGraph` with Merkle-tree commitment over the node set.
