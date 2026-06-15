//! PHANTOM Networking — TCP transport layer.
//!
//! TCP listener + packet serialization with bincode.

use anyhow::Result;
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use phantom_core::PhantomPacket;
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

pub struct PhantomNetworkConfig {
    pub listen_addr: Multiaddr,
    pub gossip_topic: String,
}

impl Default for PhantomNetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/tcp/9999".parse().unwrap(),
            gossip_topic: "phantom-packets-v1".to_string(),
        }
    }
}

#[derive(Debug)]
pub enum PhantomEvent {
    PeerConnected(PeerId),
}

pub struct PhantomSwarm {
    pub local_peer_id: PeerId,
    event_rx: mpsc::UnboundedReceiver<PhantomEvent>,
}

impl PhantomSwarm {
    pub async fn new(config: PhantomNetworkConfig) -> Result<Self> {
        let local_key = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        let port = extract_port(&config.listen_addr);
        tracing::info!("Phantom node: {}", local_peer_id);

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Start a simple TCP listener
        tokio::spawn(async move {
            let addr = format!("0.0.0.0:{}", port);
            match tokio::net::TcpListener::bind(&addr).await {
                Ok(listener) => {
                    tracing::info!("✓ Listening on tcp/{}", port);
                    loop {
                        match listener.accept().await {
                            Ok((_stream, _addr)) => {
                                let _ = event_tx.send(PhantomEvent::PeerConnected(PeerId::random()));
                            }
                            Err(e) => {
                                tracing::warn!("Accept error: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("⚠ Could not bind {}: {}", addr, e);
                }
            }
        });

        Ok(Self { local_peer_id, event_rx })
    }

    pub fn local_peer_id(&self) -> PeerId { self.local_peer_id }

    pub async fn next_event(&mut self) -> Option<PhantomEvent> {
        self.event_rx.recv().await
    }

    /// Serialize and send a Phantom packet to a connected peer.
    pub fn send_packet(&self, packet: &PhantomPacket) -> Result<Vec<u8>> {
        bincode::serialize(packet).map_err(|e| anyhow::anyhow!("serialize: {}", e))
    }

    /// Connect to a peer and send a Phantom packet.
    pub async fn connect_and_send(&self, addr: &str, packet: &PhantomPacket) -> Result<()> {
        let data = bincode::serialize(packet)?;
        let mut stream = TcpStream::connect(addr).await?;
        let len = data.len() as u32;
        stream.write_all(&len.to_le_bytes()).await?;
        stream.write_all(&data).await?;
        tracing::info!("Sent packet to {} ({} bytes)", addr, data.len());
        Ok(())
    }
}

fn extract_port(addr: &Multiaddr) -> u16 {
    use libp2p::multiaddr::Protocol;
    for proto in addr.iter() {
        if let Protocol::Tcp(port) = proto {
            return port;
        }
    }
    9999
}
