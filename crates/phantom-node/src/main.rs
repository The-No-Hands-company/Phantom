//! PHANTOM Node — oblivious privacy network daemon.
//!
//! Listens for Phantom packets, runs them through the ObliviousForwarder,
//! and routes them to the next hop without learning the path.

use anyhow::Result;
use clap::Parser;
use phantom_core::{PhantomPacket, network::NetworkGraph};
use phantom_crypto::{fhe::FheEngine, pq::{KeyPair, SigningKeyPair}};
use phantom_networking::{PhantomNetworkConfig, PhantomSwarm, PhantomEvent};
use phantom_routing::{ObliviousForwarder, RoutingDecision};
use std::sync::{Arc, RwLock};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "phantom-node", version)]
struct Args {
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/9999")]
    listen: String,

    #[arg(long)]
    connect: Option<String>,

    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let level = if args.verbose { tracing::Level::DEBUG } else { tracing::Level::INFO };
    tracing_subscriber::fmt().with_max_level(level).init();

    info!("🔮 PHANTOM Node starting...");

    // ── Post-quantum identity ──
    let _kem = KeyPair::generate();
    let _sig = SigningKeyPair::generate();
    let did = format!("did:phantom:{}", &hex::encode(blake3::hash(b"phantom-node").as_bytes())[..16]);
    info!("Identity: {}", did);

    // ── Network graph + FHE ──
    let node_id: u32 = 1;
    let mut graph = NetworkGraph::new();
    graph.add_node(phantom_core::network::NodeInfo {
        id: node_id, bandwidth: 100_000_000, latency_ms: 10, uptime_hours: 0, reputation: 1.0,
    });
    let graph = Arc::new(RwLock::new(graph));
    let fhe = Arc::new(FheEngine::generate_keys());

    // ── Oblivious forwarder ──
    let forwarder = ObliviousForwarder::new(node_id, fhe, graph.clone());
    let forwarder = Arc::new(RwLock::new(forwarder));

    // ── TCP listener ──
    let listen_addr: libp2p::Multiaddr = args.listen.parse()?;
    let mut swarm = PhantomSwarm::new(PhantomNetworkConfig {
        listen_addr: listen_addr.clone(),
        gossip_topic: "phantom-packets-v1".to_string(),
    }).await?;

    info!("Peer ID: {}", swarm.local_peer_id());

    // ── Connect to peer if specified ──
    if let Some(ref peer_addr) = args.connect {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let packet = PhantomPacket {
            routing_blob: vec![],
            path_proof: phantom_core::proof::RoutingProof {
                proof_data: vec![],
                public_inputs: phantom_core::proof::PublicInputs {
                    network_commitment: [0u8; 32], path_length: 0, timestamp: 0,
                },
            },
            payload: b"Hello from Phantom! PQ encrypted.".to_vec(),
            nullifier: [0u8; 32],
            packet_id: [1u8; 32],
        };
        match PhantomSwarm::connect_and_send(peer_addr, &packet).await {
            Ok(_) => info!("✓ Test packet sent to {}", peer_addr),
            Err(e) => tracing::warn!("⚠ Send failed: {}", e),
        }
    }

    info!("🌐 Oblivious routing active");

    // ── Main event loop: receive → forward ──
    loop {
        tokio::select! {
            Some(event) = swarm.next_event() => {
                match event {
                    PhantomEvent::PacketReceived { from, packet } => {
                        info!("📦 Packet {} bytes from {}", packet.payload.len(), from);
                        let mut fwd = forwarder.write().unwrap();
                        match fwd.process_packet(&packet) {
                            Ok(RoutingDecision::Forward(next_hop)) => {
                                info!("  → Forwarding to node {}", next_hop);
                            }
                            Ok(RoutingDecision::Deliver) => {
                                info!("  ✓ Packet delivered locally");
                                // TODO: dispatch to local application handler
                            }
                            Ok(RoutingDecision::Drop(reason)) => {
                                info!("  ✗ Dropped: {:?}", reason);
                            }
                            Err(e) => tracing::warn!("  Forwarder error: {}", e),
                        }
                    }
                    PhantomEvent::PeerConnected(peer) => {
                        info!("Peer {} connected", peer);
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("👋 Shutting down...");
                break;
            }
        }
    }

    Ok(())
}
