//! PHANTOM Node — oblivious privacy network daemon.
//!
//! Listens for Phantom packets, runs them through the ObliviousForwarder,
//! and routes them to the next hop without learning the path.
//!
//! Usage:
//!   phantom-node --listen /ip4/0.0.0.0/tcp/9100
//!   phantom-node --listen /ip4/0.0.0.0/tcp/9101 --peer 1=127.0.0.1:9100

use anyhow::Result;
use clap::Parser;
use phantom_core::{PhantomPacket, network::NetworkGraph, proof::{RoutingProof, PublicInputs, ProofGenerator}};
use phantom_crypto::{FheEngine, RelayKeyRegistry, pq::{KeyPair, SigningKeyPair}};
use phantom_networking::{PhantomNetworkConfig, PhantomSwarm, PhantomEvent};
use phantom_routing::{ObliviousForwarder, RoutingDecision};
use phantom_zkvm::Plonky2ProofGenerator;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{info, warn, debug};

/// Command-line arguments for the Phantom node.
#[derive(Parser, Debug)]
#[command(name = "phantom-node", about = "Oblivious Phantom routing node", version)]
struct Args {
    /// libp2p multiaddr to listen on.
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/9999")]
    listen: String,

    /// Peer routing entries of the form node_id=host:port (repeatable).
    /// Example: --peer 2=192.168.1.5:9100 --peer 3=192.168.1.6:9100
    #[arg(long = "peer", value_name = "NODE_ID=ADDR")]
    peers: Vec<String>,

    /// This node's numeric ID (default: 1).
    #[arg(long, default_value_t = 1)]
    node_id: u32,

    #[arg(short, long)]
    verbose: bool,
}

/// Parse a "node_id=host:port" peer string.
fn parse_peer(s: &str) -> Option<(u32, String)> {
    let (id_str, addr) = s.split_once('=')?;
    let id: u32 = id_str.parse().ok()?;
    Some((id, addr.to_string()))
}

/// Shared node state passed across async tasks.
struct NodeState {
    forwarder: Arc<ObliviousForwarder>,
    peer_addrs: Arc<RwLock<HashMap<u32, String>>>,
    proof_generator: Arc<Plonky2ProofGenerator>,
    network_commitment: [u8; 32],
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let level = if args.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO };
    tracing_subscriber::fmt().with_max_level(level).init();

    info!("🔮 PHANTOM Node {} starting…", args.node_id);

    // ── Post-quantum identity ──
    let _kem = KeyPair::generate();
    let _sig = SigningKeyPair::generate();
    let did = format!(
        "did:phantom:node:{}",
        &hex::encode(blake3::hash(args.node_id.to_string().as_bytes()).as_bytes())[..16]
    );
    info!("Identity: {}", did);

    // ── Peer address table (node_id → "host:port") ──
    let peer_addrs: Arc<RwLock<HashMap<u32, String>>> = Arc::new(RwLock::new(HashMap::new()));
    for entry in &args.peers {
        if let Some((id, addr)) = parse_peer(entry) {
            info!("  Peer {}: {}", id, addr);
            peer_addrs.write().unwrap().insert(id, addr);
        } else {
            warn!("  Ignoring malformed --peer '{}' (expected node_id=host:port)", entry);
        }
    }

    // ── Network graph + FHE engine ──
    let mut graph = NetworkGraph::new();
    graph.add_node(phantom_core::network::NodeInfo {
        id: args.node_id,
        bandwidth: 100_000_000,
        latency_ms: 10,
        uptime_hours: 0,
        reputation: 1.0,
    });
    for (id, _) in peer_addrs.read().unwrap().iter() {
        graph.add_node(phantom_core::network::NodeInfo {
            id: *id,
            bandwidth: 100_000_000,
            latency_ms: 20,
            uptime_hours: 0,
            reputation: 0.9,
        });
        graph.add_edge(args.node_id, *id);
    }
    let network_commitment = *graph.commitment();
    let graph = Arc::new(RwLock::new(graph));

    info!("Generating FHE keys (this takes a moment)…");
    let fhe = Arc::new(FheEngine::generate_keys());
    info!("  FHE keys ready.");

    // ── Per-relay key registry (each known peer gets its own FHE keypair) ──
    let mut relay_registry = RelayKeyRegistry::new();
    relay_registry.register_with_engine(args.node_id, fhe.clone());
    for id in peer_addrs.read().unwrap().keys().copied().collect::<Vec<_>>() {
        relay_registry.register_with_engine(id, fhe.clone()); // shared engine for now
    }

    // ── Oblivious forwarder ──
    let forwarder = Arc::new(ObliviousForwarder::new(args.node_id, fhe.clone(), graph));

    // ── ZK proof generator (Plonky2 SNARKs for routing proofs) ──
    info!("Initializing Plonky2 proof generator…");
    let proof_generator = Arc::new(
        Plonky2ProofGenerator::new(20, 7)
            .expect("Plonky2 circuit construction failed"),
    );
    info!("  Proof generator ready.");

    // ── TCP listener ──
    let listen_addr: libp2p::Multiaddr = args.listen.parse()?;
    let mut swarm = PhantomSwarm::new(PhantomNetworkConfig {
        listen_addr: listen_addr.clone(),
        gossip_topic: "phantom-packets-v1".to_string(),
    }).await?;

    info!("Peer ID: {}", swarm.local_peer_id());
    info!("🌐 Oblivious routing active — ctrl-c to stop");

    let state = NodeState {
        forwarder: forwarder.clone(),
        peer_addrs: peer_addrs.clone(),
        proof_generator: proof_generator.clone(),
        network_commitment,
    };

    // ── Main event loop: receive → decide → forward ──
    loop {
        tokio::select! {
            Some(event) = swarm.next_event() => {
                handle_event(event, &state).await;
            }
            _ = tokio::signal::ctrl_c() => {
                info!("👋 Shutting down.");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_event(event: PhantomEvent, state: &NodeState) {
    match event {
        PhantomEvent::PacketReceived { from, packet } => {
            debug!("📦 Packet {} bytes from {}", packet.payload.len(), from);

            match state.forwarder.process_packet(&packet) {
                Ok(RoutingDecision::Forward(next_hop)) => {
                    let addr = state.peer_addrs.read().unwrap().get(&next_hop).cloned();
                    if let Some(addr) = addr {
                        info!("  → Forwarding to node {} @ {}", next_hop, addr);
                        if let Err(e) = PhantomSwarm::connect_and_send(&addr, &packet).await {
                            warn!("  Forward failed: {}", e);
                        }
                    } else {
                        warn!("  → No address for next hop {}; dropping", next_hop);
                    }
                }
                Ok(RoutingDecision::Deliver) => {
                    // Verify the routing proof before accepting the payload.
                    if !packet.path_proof.proof_data.is_empty() {
                        match state.proof_generator.verify_path_proof(
                            &packet.path_proof,
                            &state.network_commitment,
                        ) {
                            Ok(true) => debug!("  ✓ Routing proof verified"),
                            Ok(false) => {
                                warn!("  ✗ Routing proof invalid — dropping");
                                return;
                            }
                            Err(e) => warn!("  ⚠ Proof verification error: {} (accepting)", e),
                        }
                    }
                    info!("  ✓ Packet delivered locally ({} bytes ciphertext)", packet.payload.len());
                }
                Ok(RoutingDecision::Drop(reason)) => {
                    debug!("  ✗ Dropped: {:?}", reason);
                }
                Err(e) => warn!("  Forwarder error: {}", e),
            }
        }
        PhantomEvent::PeerConnected(peer) => {
            info!("Peer {} connected", peer);
        }
    }
}
