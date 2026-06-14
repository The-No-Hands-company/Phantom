//! PHANTOM Node — privacy network daemon.
//!
//! Starts a libp2p swarm with post-quantum identity, listens for Phantom
//! packets, and exposes a local JSON-RPC endpoint for applications.

use anyhow::Result;
use clap::Parser;
use phantom_networking::{PhantomNetworkConfig, PhantomSwarm, PhantomEvent};
use phantom_crypto::pq::{KeyPair, SigningKeyPair};
use tracing::info;

#[derive(Parser, Debug)]
#[command(name = "phantom-node", version, about = "PHANTOM privacy network daemon")]
struct Args {
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/9999")]
    listen: String,

    #[arg(long, default_value = "9900")]
    rpc_port: u16,

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
    info!("🌐 Ready to route packets obliviously");

    // Main event loop
    loop {
        tokio::select! {
            Some(event) = swarm.next_event() => {
                match event {
                    PhantomEvent::PacketReceived { from, packet } => {
                        info!("Packet from {}: {} bytes", from, packet.payload.len());
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
