/// Byzantine attack configuration and behavior modeling
///
/// Defines different types of Byzantine (malicious) attacks that can be simulated:
/// - Packet dropping (denial of service)
/// - Malicious routing (send packets to wrong destinations)
/// - Timing attacks (intentional delays)
/// - Packet forgery (invalid cryptographic proofs)

use crate::node::NodeBehavior;
use serde::{Serialize, Deserialize};
use rand::Rng;

/// Byzantine attack types and their parameters
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum ByzantineAttack {
    /// Drop packets randomly with given probability
    RandomDrop { rate: f64 },
    
    /// Always drop packets (complete denial of service)
    AlwaysDrop,
    
    /// Malicious routing (send to wrong next hop)
    MaliciousRouting { rate: f64 },
    
    /// Intentional delay attack
    DelayAttack { delay_ms: u64 },
    
    /// Packet forgery (try to create invalid proofs)
    ForgePackets { rate: f64 },
    
    /// Mix of multiple attack types
    Mixed,
}

/// Configuration for Byzantine nodes in simulation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ByzantineConfig {
    /// Distribution of attack types
    pub attack_distribution: AttackDistribution,
    
    /// Default drop rate for RandomDrop attacks
    pub default_drop_rate: f64,
    
    /// Default malice rate for MaliciousRouting
    pub default_malice_rate: f64,
    
    /// Default delay for DelayAttack (milliseconds)
    pub default_delay_ms: u64,
    
    /// Default forge rate for ForgePackets
    pub default_forge_rate: f64,
}

impl Default for ByzantineConfig {
    fn default() -> Self {
        Self {
            attack_distribution: AttackDistribution::Uniform,
            default_drop_rate: 0.5,  // 50% packet drop
            default_malice_rate: 0.3,  // 30% malicious routing
            default_delay_ms: 1000,  // 1 second delay
            default_forge_rate: 0.2,  // 20% forgery attempts
        }
    }
}

impl ByzantineConfig {
    /// Select a random attack type based on distribution
    pub fn select_attack(&self) -> NodeBehavior {
        match self.attack_distribution {
            AttackDistribution::Uniform => {
                // Equal probability for each attack type
                let mut rng = rand::thread_rng();
                match rng.gen_range(0..4) {
                    0 => NodeBehavior::DropPackets { drop_rate: self.default_drop_rate },
                    1 => NodeBehavior::MaliciousRouting { malice_rate: self.default_malice_rate },
                    2 => NodeBehavior::DelayAttack { delay_ms: self.default_delay_ms },
                    3 => NodeBehavior::ForgePackets { forge_rate: self.default_forge_rate },
                    _ => unreachable!(),
                }
            }
            
            AttackDistribution::DropOnly => {
                NodeBehavior::DropPackets { drop_rate: self.default_drop_rate }
            }
            
            AttackDistribution::RoutingOnly => {
                NodeBehavior::MaliciousRouting { malice_rate: self.default_malice_rate }
            }
            
            AttackDistribution::DelayOnly => {
                NodeBehavior::DelayAttack { delay_ms: self.default_delay_ms }
            }
            
            AttackDistribution::ForgeOnly => {
                NodeBehavior::ForgePackets { forge_rate: self.default_forge_rate }
            }
        }
    }
    
    /// Create config for specific attack type
    pub fn for_attack(attack: ByzantineAttack) -> Self {
        let mut config = Self::default();
        
        match attack {
            ByzantineAttack::RandomDrop { rate } => {
                config.attack_distribution = AttackDistribution::DropOnly;
                config.default_drop_rate = rate;
            }
            ByzantineAttack::AlwaysDrop => {
                config.attack_distribution = AttackDistribution::DropOnly;
                config.default_drop_rate = 1.0;
            }
            ByzantineAttack::MaliciousRouting { rate } => {
                config.attack_distribution = AttackDistribution::RoutingOnly;
                config.default_malice_rate = rate;
            }
            ByzantineAttack::DelayAttack { delay_ms } => {
                config.attack_distribution = AttackDistribution::DelayOnly;
                config.default_delay_ms = delay_ms;
            }
            ByzantineAttack::ForgePackets { rate } => {
                config.attack_distribution = AttackDistribution::ForgeOnly;
                config.default_forge_rate = rate;
            }
            ByzantineAttack::Mixed => {
                config.attack_distribution = AttackDistribution::Uniform;
            }
        }
        
        config
    }
}

/// Distribution of attack types among Byzantine nodes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AttackDistribution {
    /// All attack types equally likely
    Uniform,
    
    /// Only packet dropping attacks
    DropOnly,
    
    /// Only malicious routing attacks
    RoutingOnly,
    
    /// Only delay attacks
    DelayOnly,
    
    /// Only packet forgery attacks
    ForgeOnly,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = ByzantineConfig::default();
        assert_eq!(config.attack_distribution, AttackDistribution::Uniform);
        assert_eq!(config.default_drop_rate, 0.5);
    }
    
    #[test]
    fn test_specific_attack_config() {
        let config = ByzantineConfig::for_attack(
            ByzantineAttack::RandomDrop { rate: 0.8 }
        );
        assert_eq!(config.attack_distribution, AttackDistribution::DropOnly);
        assert_eq!(config.default_drop_rate, 0.8);
    }
    
    #[test]
    fn test_attack_selection() {
        let config = ByzantineConfig::default();
        
        // Run multiple times to ensure randomness works
        for _ in 0..10 {
            let behavior = config.select_attack();
            assert!(behavior.is_byzantine());
        }
    }
}
