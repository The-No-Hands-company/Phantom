//! PHANTOM Networking — libp2p transport layer for Phantom packets.
//!
//! Provides:
//! - PhantomSwarm: libp2p swarm with QUIC+TCP transport, Noise encryption, gossipsub
//! - PhantomBehaviour: custom network behaviour routing PhantomPackets
//! - RPC endpoint: local JSON-RPC interface for applications to submit packets

use anyhow::Result;
use futures::StreamExt;
use libp2p::{
    core::upgrade,
    gossipsub::{self, MessageId, TopicHash},
    identity::Keypair,
    noise, quic, tcp,
    swarm::{NetworkBehaviour, SwarmEvent},
    Multiaddr, PeerId, Swarm, SwarmBuilder,
};
use phantom_core::PhantomPacket;
use std::collections::HashSet;
use tokio::sync::mpsc;

// ── Phantom Network Behaviour ──────────────────────────────────────

#[derive(NetworkBehaviour)]
pub struct PhantomBehaviour {
    pub gossipsub: gossipsub::Behaviour,
}

// ── Incoming events from the network ───────────────────────────────

pub enum PhantomEvent {
    PacketReceived {
        from: PeerId,
        packet: PhantomPacket,
    },
    PeerConnected(PeerId),
    PeerDisconnected(PeerId),
}

// ── Config ─────────────────────────────────────────────────────────

pub struct PhantomNetworkConfig {
    pub listen_addr: Multiaddr,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub gossip_topic: String,
}

impl Default for PhantomNetworkConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/udp/9999/quic-v1".parse().unwrap(),
            bootstrap_peers: vec![],
            gossip_topic: "phantom-packets-v1".to_string(),
        }
    }
}

// ── Phantom Swarm (the main network interface) ─────────────────────

pub struct PhantomSwarm {
    pub swarm: Swarm<PhantomBehaviour>,
    pub local_peer_id: PeerId,
    pub topic: gossipsub::IdentTopic,
    event_rx: mpsc::UnboundedReceiver<PhantomEvent>,
    pub packet_tx: mpsc::UnboundedSender<(PeerId, Vec<u8>)>,
}

impl PhantomSwarm {
    /// Create and start a new Phantom swarm.
    pub async fn new(config: PhantomNetworkConfig) -> Result<Self> {
        let local_key = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());

        // Build the swarm with TCP + QUIC transport
        let mut swarm = SwarmBuilder::with_existing_identity(local_key)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                upgrade::yamux::Config::default,
            )?
            .with_quic()
            .with_behaviour(|key| {
                let message_id_fn = |msg: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    s.write(&msg.data);
                    MessageId::from(s.finish().to_string())
                };

                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .message_id_fn(message_id_fn)
                    .build()
                    .expect("valid gossipsub config");

                Ok(PhantomBehaviour {
                    gossipsub: gossipsub::Behaviour::new(
                        gossipsub::MessageAuthenticity::Signed(key.clone()),
                        gossipsub_config,
                    )?,
                })
            })?
            .build();

        let topic = gossipsub::IdentTopic::new(&config.gossip_topic);
        swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

        // Listen on the configured address
        swarm.listen_on(config.listen_addr.clone())?;
        tracing::info!("Phantom swarm listening on {}", config.listen_addr);

        // Bootstrap from known peers
        for addr in &config.bootstrap_peers {
            swarm.dial(addr.clone())?;
            tracing::info!("Bootstrapping peer: {}", addr);
        }

        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (packet_tx, mut packet_rx) = mpsc::unbounded_channel();

        // Spawn the swarm event loop
        let _swarm_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    swarm_event = swarm.select_next_some() => {
                        match swarm_event {
                            SwarmEvent::Behaviour(PhantomBehaviourEvent::Gossipsub(
                                gossipsub::Event::Message { propagation_source, message_id: _, message }
                            )) => {
                                if let Ok(packet) = bincode::deserialize::<PhantomPacket>(&message.data) {
                                    let _ = event_tx.send(PhantomEvent::PacketReceived {
                                        from: propagation_source,
                                        packet,
                                    });
                                }
                            }
                            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                                tracing::info!("Peer connected: {}", peer_id);
                                let _ = event_tx.send(PhantomEvent::PeerConnected(peer_id));
                            }
                            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                                tracing::debug!("Peer disconnected: {}", peer_id);
                                let _ = event_tx.send(PhantomEvent::PeerDisconnected(peer_id));
                            }
                            _ => {}
                        }
                    }
                    Some((peer_id, packet_bytes)) = packet_rx.recv() => {
                        if let Err(e) = swarm.behaviour_mut().gossipsub.publish(topic.clone(), packet_bytes) {
                            tracing::warn!("Failed to publish packet to {}: {}", peer_id, e);
                        }
                    }
                }
            }
        });

        Ok(Self {
            swarm,
            local_peer_id,
            topic,
            event_rx,
            packet_tx,
        })
    }

    /// Get the local peer ID.
    pub fn local_peer_id(&self) -> PeerId {
        self.local_peer_id
    }

    /// Send a Phantom packet to the network (broadcast via gossipsub).
    pub fn send_packet(&self, _peer: PeerId, packet: &PhantomPacket) -> Result<()> {
        let bytes = bincode::serialize(packet)?;
        self.packet_tx
            .send((PeerId::random(), bytes))
            .map_err(|e| anyhow::anyhow!("channel closed: {}", e))?;
        Ok(())
    }

    /// Receive the next network event (non-blocking poll).
    pub async fn next_event(&mut self) -> Option<PhantomEvent> {
        self.event_rx.recv().await
    }
}

// ── Cover Traffic Generator ────────────────────────────────────────

/// Generates dummy Phantom packets at random intervals to prevent
/// traffic analysis. An observer cannot distinguish real packets
/// from cover traffic.
pub struct CoverTraffic {
    /// Minimum interval between cover packets (ms)
    pub min_interval_ms: u64,
    /// Maximum interval between cover packets (ms)
    pub max_interval_ms: u64,
    /// Whether cover traffic is enabled
    pub enabled: bool,
}

impl Default for CoverTraffic {
    fn default() -> Self {
        Self {
            min_interval_ms: 2000,  // 2 seconds
            max_interval_ms: 15000, // 15 seconds
            enabled: true,
        }
    }
}

impl CoverTraffic {
    /// Start generating cover traffic on the swarm.
    /// Spawns a background task that periodically sends dummy packets.
    pub fn start(
        config: CoverTraffic,
        packet_tx: tokio::sync::mpsc::UnboundedSender<(libp2p::PeerId, Vec<u8>)>,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if !config.enabled {
                return;
            }
            tracing::info!(
                "Cover traffic started (interval: {}-{}ms)",
                config.min_interval_ms, config.max_interval_ms
            );

            loop {
                let delay_ms = {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    rng.gen_range(config.min_interval_ms..=config.max_interval_ms)
                };
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

                // Generate a dummy packet with random payload
                let dummy_payload: Vec<u8> = (0..256).map(|_| rand::random::<u8>()).collect();
                let dummy_packet = phantom_core::PhantomPacket {
                    routing_blob: vec![0; 64],
                    path_proof: phantom_core::proof::RoutingProof {
                        proof_data: vec![0; 32],
                        public_inputs: phantom_core::proof::PublicInputs {
                            network_commitment: [0u8; 32],
                            path_length: 1,
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs(),
                        },
                    },
                    payload: dummy_payload,
                    nullifier: [0u8; 32],
                    packet_id: rand::random::<[u8; 32]>(),
                };

                let data = bincode::serialize(&dummy_packet).unwrap_or_default();
                let _ = packet_tx.send((libp2p::PeerId::random(), data));

                tracing::trace!("Cover packet sent ({} bytes)", dummy_packet.payload.len());
            }
        })
    }
}

use std::hash::{DefaultHasher, Hasher};
use rand::Rng;
