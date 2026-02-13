//! Path Validation and Quality Metrics
//!
//! Validates multi-hop routing paths and computes quality scores for path selection.
//!
//! ## Quality Metrics
//!
//! Paths are scored on multiple dimensions:
//! - **Latency**: Estimated round-trip time
//! - **Reliability**: Product of node uptimes
//! - **Diversity**: Geographic and operator distribution
//! - **Capacity**: Minimum bandwidth along path
//! - **Anonymity**: Path length and node selection entropy
//!
//! ## Example
//!
//! ```rust,no_run
//! use phantom_routing::{PathValidator, PhantomPath};
//!
//! # fn main() -> anyhow::Result<()> {
//! # let path = unimplemented!();
//! let validator = PathValidator::new()
//!     .max_latency_ms(2000)
//!     .min_reliability(0.95)
//!     .min_capacity(100_000);
//!
//! let result = validator.validate_path(&path)?;
//! println!("Path quality: {:.2}", result.overall_score);
//! # Ok(())
//! # }
//! ```

use crate::path_builder::PhantomPath;
use anyhow::Result;
use serde::{Serialize, Deserialize};

/// Validator for routing paths with quality metrics
#[derive(Clone, Debug)]
pub struct PathValidator {
    /// Maximum acceptable latency in milliseconds
    max_latency_ms: u64,
    /// Minimum acceptable reliability score (0.0-1.0)
    min_reliability: f64,
    /// Minimum acceptable capacity in bytes/sec
    min_capacity: u64,
    /// Minimum acceptable anonymity score (0.0-1.0)
    min_anonymity: f64,
    /// Weights for quality scoring
    weights: QualityWeights,
}

/// Weights for different quality metrics
#[derive(Clone, Debug)]
pub struct QualityWeights {
    /// Weight for latency (lower is better)
    pub latency_weight: f64,
    /// Weight for reliability (higher is better)
    pub reliability_weight: f64,
    /// Weight for capacity (higher is better)
    pub capacity_weight: f64,
    /// Weight for anonymity (higher is better)
    pub anonymity_weight: f64,
}

impl Default for QualityWeights {
    fn default() -> Self {
        Self {
            latency_weight: 0.3,
            reliability_weight: 0.3,
            capacity_weight: 0.2,
            anonymity_weight: 0.2,
        }
    }
}

/// Result of path validation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether the path passes all validation checks
    pub valid: bool,
    /// Overall quality score (0.0-1.0)
    pub overall_score: f64,
    /// Individual metric scores
    pub metrics: QualityMetrics,
    /// Validation errors (if any)
    pub errors: Vec<String>,
}

/// Detailed quality metrics for a path
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Latency score (normalized 0.0-1.0)
    pub latency_score: f64,
    /// Reliability score (0.0-1.0)
    pub reliability_score: f64,
    /// Capacity score (normalized 0.0-1.0)
    pub capacity_score: f64,
    /// Anonymity score (0.0-1.0)
    pub anonymity_score: f64,
    /// Diversity score (0.0-1.0)
    pub diversity_score: f64,
    
    // Raw metrics
    /// Total estimated latency in milliseconds
    pub total_latency_ms: u64,
    /// Path reliability (product of node reliabilities)
    pub path_reliability: f64,
    /// Minimum capacity along path in bytes/sec
    pub min_capacity: u64,
    /// Number of hops
    pub hop_count: usize,
}

impl PathValidator {
    /// Create a new path validator with default settings
    pub fn new() -> Self {
        Self {
            max_latency_ms: 4000,      // 4 seconds max
            min_reliability: 0.90,      // 90% minimum
            min_capacity: 10_000,       // 10 KB/s minimum
            min_anonymity: 0.7,         // 70% minimum
            weights: QualityWeights::default(),
        }
    }

    /// Set maximum acceptable latency
    pub fn max_latency_ms(mut self, ms: u64) -> Self {
        self.max_latency_ms = ms;
        self
    }

    /// Set minimum acceptable reliability
    pub fn min_reliability(mut self, score: f64) -> Self {
        self.min_reliability = score.clamp(0.0, 1.0);
        self
    }

    /// Set minimum acceptable capacity
    pub fn min_capacity(mut self, bytes_per_sec: u64) -> Self {
        self.min_capacity = bytes_per_sec;
        self
    }

    /// Set minimum acceptable anonymity score
    pub fn min_anonymity(mut self, score: f64) -> Self {
        self.min_anonymity = score.clamp(0.0, 1.0);
        self
    }

    /// Set custom quality weights
    pub fn weights(mut self, weights: QualityWeights) -> Self {
        self.weights = weights;
        self
    }

    /// Validate a routing path and compute quality metrics
    pub fn validate_path(&self, path: &PhantomPath) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Compute quality metrics
        let metrics = self.compute_metrics(path);

        // Validation checks
        if metrics.total_latency_ms > self.max_latency_ms {
            errors.push(format!(
                "Latency too high: {}ms > {}ms",
                metrics.total_latency_ms, self.max_latency_ms
            ));
        }

        if metrics.path_reliability < self.min_reliability {
            errors.push(format!(
                "Reliability too low: {:.2} < {:.2}",
                metrics.path_reliability, self.min_reliability
            ));
        }

        if metrics.min_capacity < self.min_capacity {
            errors.push(format!(
                "Capacity too low: {} < {}",
                metrics.min_capacity, self.min_capacity
            ));
        }

        if metrics.anonymity_score < self.min_anonymity {
            errors.push(format!(
                "Anonymity too low: {:.2} < {:.2}",
                metrics.anonymity_score, self.min_anonymity
            ));
        }

        // Compute overall quality score
        let overall_score = self.compute_overall_score(&metrics);

        let valid = errors.is_empty();

        Ok(ValidationResult {
            valid,
            overall_score,
            metrics,
            errors,
        })
    }

    /// Compute detailed quality metrics for a path
    fn compute_metrics(&self, path: &PhantomPath) -> QualityMetrics {
        let total_latency_ms = path.estimated_latency_ms;
        let path_reliability = path.reliability_score;
        let min_capacity = self.compute_min_capacity(path);
        let hop_count = path.hop_count;
        let diversity_score = path.diversity_score;

        // Normalize metrics to 0.0-1.0 scores
        let latency_score = self.normalize_latency(total_latency_ms);
        let reliability_score = path_reliability; // Already 0.0-1.0
        let capacity_score = self.normalize_capacity(min_capacity);
        let anonymity_score = self.compute_anonymity_score(path);

        QualityMetrics {
            latency_score,
            reliability_score,
            capacity_score,
            anonymity_score,
            diversity_score,
            total_latency_ms,
            path_reliability,
            min_capacity,
            hop_count,
        }
    }

    /// Compute minimum capacity along the path (bottleneck)
    fn compute_min_capacity(&self, path: &PhantomPath) -> u64 {
        path.nodes.iter()
            .map(|n| n.capacity)
            .min()
            .unwrap_or(0)
    }

    /// Normalize latency to 0.0-1.0 score (lower is better)
    fn normalize_latency(&self, latency_ms: u64) -> f64 {
        // Score decreases linearly from 1.0 at 0ms to 0.0 at max_latency
        if latency_ms >= self.max_latency_ms {
            0.0
        } else {
            1.0 - (latency_ms as f64 / self.max_latency_ms as f64)
        }
    }

    /// Normalize capacity to 0.0-1.0 score (higher is better)
    fn normalize_capacity(&self, capacity: u64) -> f64 {
        // Score increases logarithmically
        // At min_capacity: score = 0.5
        // At 10x min_capacity: score = 1.0
        if capacity <= 0 {
            return 0.0;
        }

        let ratio = capacity as f64 / self.min_capacity as f64;
        if ratio < 1.0 {
            ratio * 0.5 // Below minimum: 0.0-0.5
        } else {
            0.5 + (ratio.log10() / 1.0).min(0.5) // Above minimum: 0.5-1.0
        }
    }

    /// Compute anonymity score based on path length and entropy
    fn compute_anonymity_score(&self, path: &PhantomPath) -> f64 {
        // Anonymity increases with path length (up to optimal ~5 hops)
        let optimal_hops = 5.0;
        let hop_score = if path.hop_count < 3 {
            path.hop_count as f64 / 3.0 // Too short: 0.0-1.0
        } else if path.hop_count <= 5 {
            1.0 // Optimal: 3-5 hops
        } else {
            // Longer paths have diminishing returns
            1.0 - ((path.hop_count as f64 - optimal_hops) / 10.0).min(0.3)
        };

        // Combine with diversity score
        (hop_score + path.diversity_score) / 2.0
    }

    /// Compute overall quality score as weighted average
    fn compute_overall_score(&self, metrics: &QualityMetrics) -> f64 {
        let w = &self.weights;
        let total_weight = w.latency_weight + w.reliability_weight + 
                          w.capacity_weight + w.anonymity_weight;

        (w.latency_weight * metrics.latency_score +
         w.reliability_weight * metrics.reliability_score +
         w.capacity_weight * metrics.capacity_score +
         w.anonymity_weight * metrics.anonymity_score) / total_weight
    }

    /// Compare two paths and return the better one
    pub fn compare_paths<'a>(
        &self,
        path_a: &'a PhantomPath,
        path_b: &'a PhantomPath,
    ) -> Result<&'a PhantomPath> {
        let result_a = self.validate_path(path_a)?;
        let result_b = self.validate_path(path_b)?;

        // Prefer valid paths over invalid ones
        match (result_a.valid, result_b.valid) {
            (true, false) => Ok(path_a),
            (false, true) => Ok(path_b),
            _ => {
                // Both valid or both invalid: compare scores
                if result_a.overall_score >= result_b.overall_score {
                    Ok(path_a)
                } else {
                    Ok(path_b)
                }
            }
        }
    }
}

impl Default for PathValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer_selector::NodeCapabilities;
    use crate::path_builder::PathBuilder;

    fn create_test_node(id: u8, bandwidth: f64, reliability: f64) -> NodeInfo {
        let mut nullifier = [0u8; 32];
        nullifier[0] = id;

        NodeInfo {
            nullifier,
            last_seen_epoch: 100,
            capabilities: NodeCapabilities {
                fhe_routing: true,
                zkvm_verification: true,
                max_packet_size: 1024,
            },
            reliability,
            capacity: (bandwidth * 1000.0) as u64,
            selection_count: 0,
        }
    }

    fn create_test_path(nodes: Vec<NodeInfo>) -> PhantomPath {
        let builder = PathBuilder::new().min_hops(nodes.len());
        builder.build_path(nodes).unwrap()
    }

    #[test]
    fn test_validator_creation() {
        let validator = PathValidator::new();
        assert_eq!(validator.max_latency_ms, 4000);
        assert_eq!(validator.min_reliability, 0.90);
    }

    #[test]
    fn test_valid_path() {
        let validator = PathValidator::new();
        
        let nodes = vec![
            create_test_node(1, 100.0, 0.97),
            create_test_node(2, 100.0, 0.97),
            create_test_node(3, 100.0, 0.97),
        ];
        let path = create_test_path(nodes);

        let result = validator.validate_path(&path).unwrap();
        assert!(result.valid, "Path should be valid. Errors: {:?}", result.errors);
        assert!(result.overall_score > 0.5);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_low_reliability_path() {
        let validator = PathValidator::new().min_reliability(0.95);
        
        let nodes = vec![
            create_test_node(1, 100.0, 0.8), // Low reliability
            create_test_node(2, 100.0, 0.9),
            create_test_node(3, 100.0, 0.9),
        ];
        let path = create_test_path(nodes);

        let result = validator.validate_path(&path).unwrap();
        assert!(!result.valid);
        assert!(!result.errors.is_empty());
        assert!(result.errors[0].contains("Reliability too low"));
    }

    #[test]
    fn test_high_latency_path() {
        let validator = PathValidator::new().max_latency_ms(200);
        
        let nodes = vec![
            create_test_node(1, 100.0, 0.95),
            create_test_node(2, 100.0, 0.95),
            create_test_node(3, 100.0, 0.95),
            create_test_node(4, 100.0, 0.95),
            create_test_node(5, 100.0, 0.95),
        ];
        let path = create_test_path(nodes);

        let result = validator.validate_path(&path).unwrap();
        assert!(!result.valid);
        assert!(result.errors[0].contains("Latency too high"));
    }

    #[test]
    fn test_low_capacity_path() {
        let validator = PathValidator::new().min_capacity(50_000);
        
        let nodes = vec![
            create_test_node(1, 100.0, 0.97),
            create_test_node(2, 10.0, 0.97), // Low bandwidth bottleneck
            create_test_node(3, 100.0, 0.97),
        ];
        let path = create_test_path(nodes);

        let result = validator.validate_path(&path).unwrap();
        assert!(!result.valid, "Path should be invalid due to low capacity");
        assert!(!result.errors.is_empty());
        assert!(result.errors.iter().any(|e| e.contains("Capacity too low")), 
            "Should have capacity error. Errors: {:?}", result.errors);
    }

    #[test]
    fn test_quality_metrics_computation() {
        let validator = PathValidator::new();
        
        let nodes = vec![
            create_test_node(1, 100.0, 0.9),
            create_test_node(2, 80.0, 0.85),
            create_test_node(3, 120.0, 0.95),
        ];
        let path = create_test_path(nodes);

        let result = validator.validate_path(&path).unwrap();
        let metrics = &result.metrics;

        // Check basic metrics
        assert_eq!(metrics.hop_count, 3);
        assert!(metrics.total_latency_ms > 0);
        
        // Reliability should be product of individual scores
        let expected_reliability = 0.9 * 0.85 * 0.95;
        assert!((metrics.path_reliability - expected_reliability).abs() < 0.01);
        
        // Min capacity should be the bottleneck
        assert_eq!(metrics.min_capacity, 80_000); // 80 KB/s
        
        // Scores should be in valid range
        assert!(metrics.latency_score >= 0.0 && metrics.latency_score <= 1.0);
        assert!(metrics.reliability_score >= 0.0 && metrics.reliability_score <= 1.0);
        assert!(metrics.capacity_score >= 0.0 && metrics.capacity_score <= 1.0);
        assert!(metrics.anonymity_score >= 0.0 && metrics.anonymity_score <= 1.0);
    }

    #[test]
    fn test_anonymity_score() {
        let validator = PathValidator::new();
        
        // Short path: lower anonymity
        let short_nodes = vec![
            create_test_node(1, 100.0, 0.95),
            create_test_node(2, 100.0, 0.95),
        ];
        let short_path = create_test_path(short_nodes);
        let short_result = validator.validate_path(&short_path).unwrap();
        
        // Optimal path: higher anonymity
        let optimal_nodes = vec![
            create_test_node(1, 100.0, 0.95),
            create_test_node(2, 100.0, 0.95),
            create_test_node(3, 100.0, 0.95),
            create_test_node(4, 100.0, 0.95),
            create_test_node(5, 100.0, 0.95),
        ];
        let optimal_path = create_test_path(optimal_nodes);
        let optimal_result = validator.validate_path(&optimal_path).unwrap();
        
        // Optimal path should have higher anonymity score
        assert!(optimal_result.metrics.anonymity_score > short_result.metrics.anonymity_score);
    }

    #[test]
    fn test_path_comparison() {
        let validator = PathValidator::new();
        
        // High quality path
        let good_nodes = vec![
            create_test_node(1, 100.0, 0.95),
            create_test_node(2, 100.0, 0.95),
            create_test_node(3, 100.0, 0.95),
        ];
        let good_path = create_test_path(good_nodes);
        
        // Lower quality path
        let bad_nodes = vec![
            create_test_node(1, 50.0, 0.8),
            create_test_node(2, 50.0, 0.8),
            create_test_node(3, 50.0, 0.8),
        ];
        let bad_path = create_test_path(bad_nodes);
        
        let better_path = validator.compare_paths(&good_path, &bad_path).unwrap();
        
        // Should select the good path
        assert_eq!(better_path.nodes[0].reliability, 0.95);
    }

    #[test]
    fn test_custom_weights() {
        let weights = QualityWeights {
            latency_weight: 0.5,
            reliability_weight: 0.3,
            capacity_weight: 0.1,
            anonymity_weight: 0.1,
        };
        
        let validator = PathValidator::new().weights(weights);
        
        let nodes = vec![
            create_test_node(1, 100.0, 0.95),
            create_test_node(2, 100.0, 0.95),
            create_test_node(3, 100.0, 0.95),
        ];
        let path = create_test_path(nodes);
        
        let result = validator.validate_path(&path).unwrap();
        
        // Should compute score with custom weights
        assert!(result.overall_score > 0.0);
    }

    #[test]
    fn test_latency_normalization() {
        let validator = PathValidator::new().max_latency_ms(1000);
        
        // Zero latency: perfect score
        assert_eq!(validator.normalize_latency(0), 1.0);
        
        // Half max: 0.5 score
        assert!((validator.normalize_latency(500) - 0.5).abs() < 0.01);
        
        // Max latency: 0.0 score
        assert_eq!(validator.normalize_latency(1000), 0.0);
        
        // Over max: 0.0 score
        assert_eq!(validator.normalize_latency(2000), 0.0);
    }

    #[test]
    fn test_capacity_normalization() {
        let validator = PathValidator::new().min_capacity(10_000);
        
        // Zero capacity: 0.0 score
        assert_eq!(validator.normalize_capacity(0), 0.0);
        
        // Below minimum: < 0.5 score
        assert!(validator.normalize_capacity(5_000) < 0.5);
        
        // At minimum: 0.5 score
        assert!((validator.normalize_capacity(10_000) - 0.5).abs() < 0.01);
        
        // Above minimum: > 0.5 score
        assert!(validator.normalize_capacity(100_000) > 0.5);
    }
}
