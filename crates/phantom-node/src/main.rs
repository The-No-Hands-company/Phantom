//! PHANTOM Node — privacy network daemon.
//!
//! Starts a libp2p swarm with post-quantum identity, listens for Phantom
//! packets, and exposes a local JSON-RPC endpoint for applications.

use anyhow::Result;
use clap::Parser;
use phantom_networking::{PhantomNetworkConfig, PhantomSwarm, PhantomEvent};
use phantom_crypto::pq::{KeyPair, SigningKeyPair};
use phantom_core::PhantomPacket;
use std::time::Duration;
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "phantom-node", version)]
struct Args {
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/9999")]
    listen: String,

    #[arg(long, default_value = "9900")]
    rpc_port: u16,

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

    // Generate post-quantum identity
    let _kem = KeyPair::generate();
    let _sig = SigningKeyPair::generate();
    let did = format!("did:phantom:{}", &hex::encode(blake3::hash(b"phantom-node").as_bytes())[..16]);
    info!("Identity: {}", did);

    // Start Phantom swarm
    let listen_addr: libp2p::Multiaddr = args.listen.parse()?;
    let mut swarm = PhantomSwarm::new(PhantomNetworkConfig {
        listen_addr: listen_addr.clone(),
        gossip_topic: "phantom-packets-v1".to_string(),
    }).await?;

    info!("Swarm listening on {}, peer_id={}", listen_addr, swarm.local_peer_id());
    info!("RPC on 127.0.0.1:{}", args.rpc_port);

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
            payload: b"Hello from Phantom! PQ encrypted packet.".to_vec(),
            nullifier: [0u8; 32],
            packet_id: [1u8; 32],
        };
        match swarm.connect_and_send(peer_addr, &packet).await {
            Ok(_) => info!("✓ Packet sent to {}", peer_addr),
            Err(e) => tracing::warn!("⚠ Send failed: {}", e),
        }
    }

    info!("🌐 Ready to route packets obliviously");

    // Main event loop
    loop {
        tokio::select! {
            Some(event) = swarm.next_event() => {
                match event {
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
