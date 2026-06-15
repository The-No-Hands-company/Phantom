//! PHANTOM Networking — TCP transport with Phantom packet serialization.

use anyhow::Result;
use libp2p::{identity::Keypair, Multiaddr, PeerId};
use phantom_core::PhantomPacket;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
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
    PacketReceived { from: PeerId, packet: PhantomPacket },
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

        // TCP listener with packet deserialization
        tokio::spawn(async move {
            let addr = format!("0.0.0.0:{}", port);
            let listener = match TcpListener::bind(&addr).await {
                Ok(l) => l,
                Err(e) => { tracing::warn!("Bind failed: {}", e); return; }
            };
            tracing::info!("✓ Listening on tcp/{}", port);

            loop {
                let (mut stream, _addr) = match listener.accept().await {
                    Ok(c) => c,
                    Err(e) => { tracing::warn!("Accept error: {}", e); continue; }
                };

                let tx = event_tx.clone();
                tokio::spawn(async move {
                    // Read length-prefixed bincode packet
                    let mut len_buf = [0u8; 4];
                    if stream.read_exact(&mut len_buf).await.is_err() { return; }
                    let len = u32::from_le_bytes(len_buf) as usize;
                    if len > 10_000_000 { return; } // 10MB limit

                    let mut data = vec![0u8; len];
                    if stream.read_exact(&mut data).await.is_err() { return; }

                    match bincode::deserialize::<PhantomPacket>(&data) {
                        Ok(packet) => {
                            let _ = tx.send(PhantomEvent::PacketReceived {
                                from: PeerId::random(),
                                packet,
                            });
                        }
                        Err(e) => tracing::debug!("Deserialize failed: {}", e),
                    }
                });
            }
        });

        Ok(Self { local_peer_id, event_rx })
    }

    pub fn local_peer_id(&self) -> PeerId { self.local_peer_id }
    pub async fn next_event(&mut self) -> Option<PhantomEvent> { self.event_rx.recv().await }

    pub fn serialize_packet(packet: &PhantomPacket) -> Result<Vec<u8>> {
        bincode::serialize(packet).map_err(|e| anyhow::anyhow!("serialize: {}", e))
    }

    pub async fn connect_and_send(addr: &str, packet: &PhantomPacket) -> Result<()> {
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
        if let Protocol::Tcp(port) = proto { return port; }
    }
    9999
}
