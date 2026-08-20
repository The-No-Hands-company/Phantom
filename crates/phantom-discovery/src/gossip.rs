/// Gossip Protocol for Anonymous Announcement Propagation
/// 
/// PHANTOM uses epidemic-style gossip to distribute node announcements
/// across the network without revealing propagation paths.
/// 
/// ## Design Goals
/// 
/// - **Metadata resistance**: No source/origin tracking
/// - **Byzantine tolerance**: Invalid messages filtered via zk-proofs
/// - **Low bandwidth**: Bloom filters prevent redundant transmission
/// - **Eventual consistency**: All honest nodes receive all announcements

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use blake3::Hash;
use serde::{Deserialize, Serialize};
use crate::announcement::NodeAnnouncement;

/// Gossip message containing node announcements
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GossipMessage {
    /// Batch of announcements being propagated
    pub announcements: Vec<NodeAnnouncement>,
    
    /// Bloom filter of announcement IDs (for duplicate detection)
    /// Allows receivers to quickly check "do I already have this?"
    pub bloom_filter: BloomFilter,
    
    /// Message ID (random, prevents loop detection but not tracking)
    pub message_id: [u8; 16],
    
    /// TTL (hops remaining, prevents infinite propagation)
    pub ttl: u8,
}

/// Space-efficient probabilistic set membership
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BloomFilter {
    /// Bit array (1024 bytes = 8192 bits)
    bits: Vec<u8>,
    
    /// Number of hash functions (k=4 for 1% false positive rate)
    num_hashes: u8,
}

impl BloomFilter {
    /// Create new bloom filter (1KB, ~1% false positive rate for 100 items)
    pub fn new() -> Self {
        Self {
            bits: vec![0u8; 1024], // 8192 bits
            num_hashes: 4,
        }
    }
    
    /// Insert announcement ID into filter
    pub fn insert(&mut self, announcement_id: &Hash) {
        for i in 0..self.num_hashes {
            let bit_index = self.hash_index(announcement_id, i);
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;
            self.bits[byte_index] |= 1 << bit_offset;
        }
    }
    
    /// Check if announcement ID might be in filter (false positives possible)
    pub fn contains(&self, announcement_id: &Hash) -> bool {
        for i in 0..self.num_hashes {
            let bit_index = self.hash_index(announcement_id, i);
            let byte_index = bit_index / 8;
            let bit_offset = bit_index % 8;
            
            if (self.bits[byte_index] & (1 << bit_offset)) == 0 {
                return false;
            }
        }
        true
    }
    
    /// Compute bit index for hash function i
    fn hash_index(&self, announcement_id: &Hash, hash_num: u8) -> usize {
        let mut hasher = blake3::Hasher::new();
        hasher.update(announcement_id.as_bytes());
        hasher.update(&[hash_num]);
        
        let hash_output = hasher.finalize();
        let hash_u64 = u64::from_le_bytes(hash_output.as_bytes()[0..8].try_into().unwrap());
        
        // Map to bit range [0, 8192)
        (hash_u64 % 8192) as usize
    }
}

impl Default for BloomFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// Gossip protocol manager
pub struct GossipManager {
    /// Announcements we've seen (by announcement ID)
    seen_announcements: HashMap<Hash, NodeAnnouncement>,
    
    /// When we first saw each announcement
    seen_timestamps: HashMap<Hash, Instant>,
    
    /// Messages we've recently forwarded (to prevent loops)
    recent_messages: HashSet<[u8; 16]>,
    
    /// Gossip parameters
    config: GossipConfig,
}

/// Gossip protocol configuration
#[derive(Clone, Debug)]
pub struct GossipConfig {
    /// Maximum announcements per gossip message
    pub max_batch_size: usize,
    
    /// How many peers to forward each message to
    pub fanout: usize,
    
    /// Initial TTL for new messages
    pub initial_ttl: u8,
    
    /// How long to remember seen announcements (garbage collection)
    pub announcement_retention: Duration,
    
    /// How long to remember forwarded message IDs (loop prevention)
    pub message_id_retention: Duration,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 50,      // 50 announcements per message
            fanout: 3,                // Forward to 3 random peers
            initial_ttl: 10,          // 10-hop propagation limit
            announcement_retention: Duration::from_secs(3600), // 1 hour
            message_id_retention: Duration::from_secs(300),    // 5 minutes
        }
    }
}

impl GossipManager {
    /// Create new gossip manager
    pub fn new(config: GossipConfig) -> Self {
        Self {
            seen_announcements: HashMap::new(),
            seen_timestamps: HashMap::new(),
            recent_messages: HashSet::new(),
            config,
        }
    }
    
    /// Process incoming gossip message
    /// Returns new announcements we haven't seen before
    pub fn process_message(
        &mut self,
        message: &GossipMessage,
    ) -> Result<Vec<NodeAnnouncement>, GossipError> {
        // Check if we've already forwarded this message (prevent loops)
        if self.recent_messages.contains(&message.message_id) {
            return Ok(vec![]);
        }
        
        // Mark message as seen
        self.recent_messages.insert(message.message_id);
        
        // Filter out announcements we've already seen
        let mut new_announcements = Vec::new();
        
        for announcement in &message.announcements {
            let announcement_id = announcement.announcement_id();
            
            // Skip if bloom filter says we already have it
            if message.bloom_filter.contains(&announcement_id) 
                && self.seen_announcements.contains_key(&announcement_id) {
                continue;
            }
            
            // Check if announcement is fresh
            if !announcement.is_fresh() {
                continue;
            }
            
            // Store if new
            if !self.seen_announcements.contains_key(&announcement_id) {
                self.seen_announcements.insert(announcement_id, announcement.clone());
                self.seen_timestamps.insert(announcement_id, Instant::now());
                new_announcements.push(announcement.clone());
            }
        }
        
        Ok(new_announcements)
    }
    
    /// Create gossip message to broadcast new announcements
    pub fn create_message(
        &mut self,
        announcements: Vec<NodeAnnouncement>,
    ) -> GossipMessage {
        // Build bloom filter of all announcement IDs
        let mut bloom = BloomFilter::new();
        for announcement in &announcements {
            bloom.insert(&announcement.announcement_id());
        }
        
        // Generate random message ID
        let message_id: [u8; 16] = rand::random();
        self.recent_messages.insert(message_id);
        
        GossipMessage {
            announcements,
            bloom_filter: bloom,
            message_id,
            ttl: self.config.initial_ttl,
        }
    }
    
    /// Create forwarding message (decrement TTL)
    pub fn create_forward_message(
        &mut self,
        original: &GossipMessage,
    ) -> Option<GossipMessage> {
        if original.ttl <= 1 {
            return None; // Don't forward if TTL exhausted
        }
        
        Some(GossipMessage {
            announcements: original.announcements.clone(),
            bloom_filter: original.bloom_filter.clone(),
            message_id: original.message_id,
            ttl: original.ttl - 1,
        })
    }
    
    /// Get all known announcements
    pub fn get_all_announcements(&self) -> Vec<NodeAnnouncement> {
        self.seen_announcements.values().cloned().collect()
    }
    
    /// Get announcement by ID
    pub fn get_announcement(&self, id: &Hash) -> Option<&NodeAnnouncement> {
        self.seen_announcements.get(id)
    }
    
    /// Garbage collect old announcements and message IDs
    pub fn garbage_collect(&mut self) {
        let now = Instant::now();
        
        // Remove old announcements
        self.seen_announcements.retain(|id, _| {
            if let Some(timestamp) = self.seen_timestamps.get(id) {
                now.duration_since(*timestamp) < self.config.announcement_retention
            } else {
                false
            }
        });
        
        // Remove announcement timestamps for deleted announcements
        self.seen_timestamps.retain(|id, _| {
            self.seen_announcements.contains_key(id)
        });
        
        // Clear recent message IDs (simple strategy: clear all periodically)
        // In production, track timestamps per message ID
        if self.recent_messages.len() > 10000 {
            self.recent_messages.clear();
        }
    }
    
    /// Get statistics about gossip state
    pub fn stats(&self) -> GossipStats {
        GossipStats {
            total_announcements: self.seen_announcements.len(),
            recent_messages: self.recent_messages.len(),
        }
    }
}

/// Gossip manager statistics
#[derive(Clone, Debug)]
pub struct GossipStats {
    pub total_announcements: usize,
    pub recent_messages: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum GossipError {
    #[error("Invalid message format")]
    InvalidFormat,
    
    #[error("Message expired (TTL exhausted)")]
    Expired,
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::announcement::{NodeDescriptor, NodeCapabilities};
    use phantom_core::identity::NodeIdentity;
    use phantom_crypto::pq::SigningKeyPair;
    
    fn create_test_announcement(node_id: u8) -> NodeAnnouncement {
        let keypair = SigningKeyPair::generate();
        
        let descriptor = NodeDescriptor::new(
            vec![node_id],
            vec!["127.0.0.1:8080".parse().unwrap()],
            1,
            NodeCapabilities::default(),
            1_000_000,
            None,
        );
        
        NodeAnnouncement::new(
            descriptor,
            vec![0u8; 100],
            &NodeIdentity([node_id; 32]),
            1,
            &[0xBBu8; 32],
            &keypair,
        ).unwrap()
    }
    
    #[test]
    fn test_bloom_filter_basic() {
        let mut bloom = BloomFilter::new();
        
        let id1 = blake3::hash(b"announcement1");
        let id2 = blake3::hash(b"announcement2");
        let id3 = blake3::hash(b"announcement3");
        
        // Insert id1 and id2
        bloom.insert(&id1);
        bloom.insert(&id2);
        
        // Check membership
        assert!(bloom.contains(&id1));
        assert!(bloom.contains(&id2));
        
        // id3 might give false positive (low probability)
        // We can't assert !contains because bloom filters can have false positives
    }
    
    #[test]
    fn test_gossip_message_creation() {
        let mut manager = GossipManager::new(GossipConfig::default());
        
        let announcements = vec![
            create_test_announcement(1),
            create_test_announcement(2),
            create_test_announcement(3),
        ];
        
        let message = manager.create_message(announcements.clone());
        
        assert_eq!(message.announcements.len(), 3);
        assert_eq!(message.ttl, 10);
        
        // Bloom filter should contain all announcement IDs
        for announcement in &announcements {
            assert!(message.bloom_filter.contains(&announcement.announcement_id()));
        }
    }
    
    // A gossip message travels between nodes. Every test below used a single
    // GossipManager as both sender and receiver, so process_message saw the
    // message_id that create_message had just recorded in recent_messages and
    // correctly refused it as a loop — returning zero new announcements every
    // time. The loop prevention was working; the tests were describing a node
    // gossiping to itself. Each one now has a sender and a receiver.
    #[test]
    fn test_gossip_deduplication() {
        let mut sender = GossipManager::new(GossipConfig::default());
        let mut receiver = GossipManager::new(GossipConfig::default());
        
        let announcement = create_test_announcement(42);
        let message = sender.create_message(vec![announcement.clone()]);
        
        // First time: new announcement
        let new_announcements = receiver.process_message(&message).unwrap();
        assert_eq!(new_announcements.len(), 1);
        
        // Second time: duplicate (should be filtered)
        let new_announcements = receiver.process_message(&message).unwrap();
        assert_eq!(new_announcements.len(), 0);
    }
    
    #[test]
    fn test_message_loop_prevention() {
        let mut sender = GossipManager::new(GossipConfig::default());
        let mut receiver = GossipManager::new(GossipConfig::default());
        
        let announcement = create_test_announcement(42);
        let message = sender.create_message(vec![announcement]);
        
        // First forward: success
        let new_announcements = receiver.process_message(&message).unwrap();
        assert_eq!(new_announcements.len(), 1);
        
        // Second forward with same message_id: blocked
        let new_announcements = receiver.process_message(&message).unwrap();
        assert_eq!(new_announcements.len(), 0);
    }
    
    #[test]
    fn test_ttl_decrement() {
        let mut manager = GossipManager::new(GossipConfig::default());
        
        let announcement = create_test_announcement(42);
        let mut message = manager.create_message(vec![announcement]);
        message.ttl = 3;
        
        // Forward 1: TTL=2
        let forwarded = manager.create_forward_message(&message).unwrap();
        assert_eq!(forwarded.ttl, 2);
        
        // Forward 2: TTL=1
        let forwarded = manager.create_forward_message(&forwarded).unwrap();
        assert_eq!(forwarded.ttl, 1);
        
        // Forward 3: None (TTL exhausted)
        assert!(manager.create_forward_message(&forwarded).is_none());
    }
    
    #[test]
    fn test_gossip_stats() {
        let mut manager = GossipManager::new(GossipConfig::default());
        
        let announcements = vec![
            create_test_announcement(1),
            create_test_announcement(2),
            create_test_announcement(3),
        ];
        
        let mut receiver = GossipManager::new(GossipConfig::default());
        let message = manager.create_message(announcements);
        receiver.process_message(&message).unwrap();
        
        let stats = receiver.stats();
        assert_eq!(stats.total_announcements, 3);
        assert!(stats.recent_messages > 0);
    }
    
    #[test]
    fn test_get_all_announcements() {
        let mut manager = GossipManager::new(GossipConfig::default());
        
        let announcements = vec![
            create_test_announcement(1),
            create_test_announcement(2),
        ];
        
        let mut receiver = GossipManager::new(GossipConfig::default());
        let message = manager.create_message(announcements.clone());
        receiver.process_message(&message).unwrap();
        
        let all = receiver.get_all_announcements();
        assert_eq!(all.len(), 2);
    }
}
