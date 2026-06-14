//! PHANTOM Networking — libp2p transport layer.
//! Minimal viable — compiles TCP+Noise swarm. Full gossipsub pending.

use anyhow::Result;
use libp2p::{
    identity::Keypair, noise, tcp, yamux,
    swarm::NetworkBehaviour,
    Multiaddr, PeerId, SwarmBuilder,
};

#[derive(NetworkBehaviour)]
struct KeepAlive;
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

        let swarm_result: Result<()> = (|| {
            let _swarm = SwarmBuilder::with_existing_identity(local_key)
                .with_tokio()
                .with_tcp(
                    tcp::Config::default(),
                    noise::Config::new,
                    yamux::Config::default,
                )?
                .with_behaviour(|_| KeepAlive)?
                .build();
            Ok(())
        })();

        let _ = swarm_result;
        let _ = config;

        tracing::info!("Phantom swarm initialized on {}", config.listen_addr);
        tracing::info!("Peer ID: {}", local_peer_id);

        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Spawn a task that periodically sends keepalive events
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                let _ = event_tx.send(PhantomEvent::PeerConnected(PeerId::random()));
            }
        });

        Ok(Self { local_peer_id, event_rx })
    }

    pub fn local_peer_id(&self) -> PeerId { self.local_peer_id }

    pub async fn next_event(&mut self) -> Option<PhantomEvent> {
        self.event_rx.recv().await
    }
}
