//! PHANTOM Full Node — sovereign privacy network daemon.
//!
//! Starts a libp2p swarm, generates a Phantom identity, runs an oblivious
//! forwarder, and exposes a local JSON-RPC endpoint for applications.

use anyhow::Result;
use clap::Parser;
use libp2p::{Multiaddr, PeerId};
use phantom_core::packet::PhantomPacket;
use phantom_core::network::NetworkGraph;
use phantom_crypto::fhe::FheEngine;
use phantom_crypto::pq::{KeyPair, SigningKeyPair};
use phantom_discovery::{
    DiscoveryService, DiscoveryConfig,
    GossipManager, GossipConfig,
};
use phantom_networking::{PhantomNetworkConfig, PhantomSwarm, PhantomEvent, CoverTraffic};
use phantom_routing::ObliviousForwarder;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

#[derive(Parser, Debug)]
#[command(name = "phantom-node", version, about = "PHANTOM privacy network daemon")]
struct Args {
    #[arg(short, long, default_value = "config.toml")]
    config: String,

    #[arg(short = 'v', long)]
    verbose: bool,

    #[arg(long, default_value = "/ip4/0.0.0.0/udp/9999/quic-v1")]
    listen: String,

    #[arg(long)]
    bootstrap: Vec<String>,

    #[arg(long, default_value = "9900")]
    rpc_port: u16,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let level = if args.verbose {
        tracing::Level::DEBUG
    } else {
        tracing::Level::INFO
    };
    tracing_subscriber::fmt().with_max_level(level).init();

    info!("🔮 PHANTOM Node starting...");

    // ── Generate Phantom identity ──────────────────────────────────

    let kem = KeyPair::generate();
    let sig = SigningKeyPair::generate();
    let did = format!(
        "did:phantom:{}",
        &hex::encode(blake3::hash(b"phantom-node").as_bytes())[..16]
    );
    info!("Identity: {}", did);

    // ── Initialize network graph ───────────────────────────────────

    let mut graph = NetworkGraph::new();
    let node_id: u32 = rand::random();
    graph.add_node(phantom_core::network::NodeInfo {
        id: node_id,
        address: args.listen.clone(),
        capacity: 100,
        reliability: 1.0,
    });
    let graph = Arc::new(RwLock::new(graph));

    // ── Start the Phantom swarm ────────────────────────────────────

    let listen_addr: Multiaddr = args.listen.parse()?;
    let bootstrap_peers: Vec<Multiaddr> = args
        .bootstrap
        .iter()
        .filter_map(|a| a.parse().ok())
        .collect();

    let mut swarm = PhantomSwarm::new(PhantomNetworkConfig {
        listen_addr: listen_addr.clone(),
        bootstrap_peers,
        gossip_topic: "phantom-packets-v1".to_string(),
    })
    .await?;

    info!(
        "Swarm listening on {}, peer_id={}",
        listen_addr, swarm.local_peer_id()
    );

    // ── Start cover traffic ────────────────────────────────────────

    let cover_handle = CoverTraffic::start(CoverTraffic::default(), swarm.packet_tx.clone());

    // ── FHE engine + oblivious forwarder ───────────────────────────

    let fhe = FheEngine::generate_keys();
    let fhe = Arc::new(RwLock::new(fhe));

    let forwarder = ObliviousForwarder::new(
        node_id as u32,
        fhe.clone(),
        phantom_core::network::NetworkGraph::new(),
    );
    let forwarder = Arc::new(RwLock::new(forwarder));

    // ── Start discovery service ────────────────────────────────────

    let discovery = DiscoveryService::new(DiscoveryConfig::default());
    let gossip = GossipManager::new(GossipConfig::default());

    // ── Start RPC endpoint for local apps ──────────────────────────

    let rpc_addr = format!("127.0.0.1:{}", args.rpc_port);
    let _rpc_handle = tokio::spawn(run_rpc_server(
        rpc_addr.clone(),
        swarm.local_peer_id(),
    ));

    info!("RPC endpoint: {}", rpc_addr);
    info!("🌐 Ready to route packets obliviously");

    // ── Main event loop ────────────────────────────────────────────

    loop {
        tokio::select! {
            Some(event) = swarm.next_event() => {
                match event {
                    PhantomEvent::PacketReceived { from, packet } => {
                        info!("Packet received from {}: {:?}", from, packet.packet_id);
                        let mut fwd = forwarder.write().await;
                        match fwd.process_packet(packet) {
                            Ok(decision) => {
                                match decision {
                                    phantom_routing::RoutingDecision::Forward(next_hop) => {
                                        info!("Forwarding to node {}", next_hop);
                                    }
                                    phantom_routing::RoutingDecision::Deliver => {
                                        info!("Packet delivered locally");
                                        // TODO: dispatch to local application
                                    }
                                    phantom_routing::RoutingDecision::Drop(reason) => {
                                        warn!("Packet dropped: {:?}", reason);
                                    }
                                }
                            }
                            Err(e) => error!("Forwarder error: {}", e),
                        }
                    }
                    PhantomEvent::PeerConnected(peer) => {
                        info!("Peer {} connected", peer);
                    }
                    PhantomEvent::PeerDisconnected(peer) => {
                        info!("Peer {} disconnected", peer);
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

// ── Local RPC server (accepts packets from apps) ──────────────────

async fn run_rpc_server(addr: String, _node_peer_id: PeerId) -> Result<()> {
    let listener = TcpListener::bind(&addr).await?;
    info!("RPC listening on {}", addr);

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let (reader, mut writer) = socket.into_split();
            let mut lines = BufReader::new(reader).lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let response = match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(req) => handle_rpc_request(req).await,
                    Err(e) => serde_json::json!({"error": format!("invalid JSON: {}", e)}),
                };
                let _ = writer.write_all(format!("{}\n", response).as_bytes()).await;
            }
        });
    }
}

async fn handle_rpc_request(req: serde_json::Value) -> serde_json::Value {
    let method = req["method"].as_str().unwrap_or("");
    match method {
        "status" => serde_json::json!({
            "node": "phantom-node",
            "version": env!("CARGO_PKG_VERSION"),
            "protocol": "phantom-v1",
            "status": "running"
        }),
        "identity" => serde_json::json!({
            "identity": "not-yet-stored" // TODO: return actual DID
        }),
        _ => serde_json::json!({"error": "unknown method", "method": method}),
    }
}
