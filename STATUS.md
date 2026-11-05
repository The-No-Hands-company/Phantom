# Phantom Protocol — Status

What the Phantom Protocol does today, and what it does not yet do. Each row names the test that proves it. For unfinished work, it names the test that is switched off until the work lands — that test passes, unchanged, the day it is done.

**Phantom is not yet used by any live TNHC service.** Sign-in, email and the dashboard currently use ordinary HTTPS.

| Capability | Status | Proved by |
| --- | --- | --- |
| Post-quantum signatures (Dilithium-5) | Done | `test_dilithium_signatures` (phantom-crypto) |
| Fully homomorphic encryption primitive | Done | `test_fhe_basic_encryption` (phantom-crypto) |
| Encrypted routing table | Partial — the route is FHE-encrypted and relays look up their next hop homomorphically; `RelayKeyRegistry` infrastructure added (each relay can hold its own keypair); per-relay key distribution protocol not yet wired into packet construction by default | `test_routing_table_lookup` (phantom-crypto) |
| Multi-hop forwarding | Partial — the test passes (about 47 seconds), but it is switched off in the default test run because the homomorphic encryption makes it slow, so routine checks do not exercise it | `test_multi_hop_forwarding` (phantom-routing) |
| Replay protection | Partial — nullifier now derived from content (routing path + plaintext payload hash) so the same logical message always produces the same nullifier; a relay that re-injects with a fresh packet_id is caught; test still switched off in default run (FHE is slow) | `test_replay_attack_detection` (phantom-routing) |
| Message contents encrypted on the wire | Done — ML-KEM-1024 encapsulation + ChaCha20-Poly1305; private key stays on the client; a relay sees only ciphertext | `payload_is_not_readable_on_the_wire` (phantom-routing) |
| Membership proof (zero-knowledge) | Done — Plonky2 zkSNARK circuit complete in phantom-circuit/membership.rs; `Plonky2ProofGenerator` wired in phantom-zkvm wraps it; `PhantomPacket::construct_with_proof` lets callers inject a real proof generator; `HashProofGenerator` remains the default in `construct()` for fast test builds | `test_end_to_end_phantom_routing` (phantom-routing) |
| Native post-quantum library | Done — `libphantom_wasm.so` (Kyber-1024 + Dilithium-5 + Blake3 via Bun FFI) built and present at `packages/phantom-sdk/wasm/target/release/libphantom_wasm.so`; services use mock SDK until `PHANTOM_REQUIRE_REAL=1` or `PHANTOM_NATIVE_LIB` env vars are set | `createNativeSDK()` (phantom-sdk/native.ts) |
| Path diversity (k-shortest paths) | Done — Yen's k-shortest-paths algorithm implemented in `NetworkGraph`; BFS helper respects removed edges and nodes per Yen's spec; five unit tests cover direct paths, diamond graphs, missing nodes, same source/dest and k<available | `test_k_shortest_paths_*` (phantom-core) |
| Small-world network topology | Done — Watts-Strogatz rewiring implemented in `TopologyGenerator::generate_small_world`; builds ring lattice then rewires each directed edge with probability β=0.1 to a random non-adjacent node; mutable adjacency list ensures no self-loops or duplicate edges | `generate_small_world` (phantom-simulation) |
| Scale-free network topology | Done — Barabási–Albert preferential attachment implemented in `TopologyGenerator::generate_scale_free`; roulette-wheel selection picks m targets proportional to their current degree for each new node | `generate_scale_free` (phantom-simulation) |
| Bootstrap — network state query | Done — `BootstrapClient::query_bootstrap_nodes` now makes a real HTTP GET to `http://{addr}/phantom/v1/network_state`; parses JSON `NetworkStateResponse`; falls back to mock state if all bootstrap nodes are unreachable so the node can still start in isolated environments | `test_full_bootstrap_sequence` (phantom-discovery) |

This page is published at https://tnhc.dev/phantom. Licensed MIT OR Apache-2.0, like the code.
