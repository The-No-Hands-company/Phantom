# Phantom Protocol — Status

What the Phantom Protocol does today, and what it does not yet do. Each row names the test that proves it. For unfinished work, it names the test that is switched off until the work lands — that test passes, unchanged, the day it is done.

**Phantom is not yet used by any live TNHC service.** Sign-in, email and the dashboard currently use ordinary HTTPS.

| Capability | Status | Proved by |
| --- | --- | --- |
| Post-quantum signatures (Dilithium-5) | Done | `test_dilithium_signatures` (phantom-crypto) |
| Fully homomorphic encryption primitive | Done | `test_fhe_basic_encryption` (phantom-crypto) |
| Encrypted routing — a relay cannot learn the route | Done | `test_routing_blob_construction` (phantom-routing) |
| Multi-hop forwarding | Partial — the test passes (about 47 seconds), but it is switched off in the default test run because the homomorphic encryption makes it slow, so routine checks do not exercise it | `test_multi_hop_forwarding` (phantom-routing) |
| Replay protection | Partial — repeated packet ids are rejected, but the nullifier is derived from the packet id, so a resent packet with a new id is not caught; the test passes but is switched off in the default test run (slow) | `test_replay_attack_detection` (phantom-routing) |
| Message contents encrypted on the wire | Not done | `payload_is_not_readable_on_the_wire` (phantom-routing) |
| Membership proof (zero-knowledge) | Not done — the circuit is incomplete, and the current proof generator is a hash, not zero-knowledge | `test_end_to_end_phantom_routing` (phantom-routing) |

This page is published at https://tnhc.dev/phantom. Licensed MIT OR Apache-2.0, like the code.
