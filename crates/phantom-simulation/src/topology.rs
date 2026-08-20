/// Network topology generation for realistic PHANTOM simulations
///
/// Supports multiple topology types:
/// - Random: Erdős–Rényi random graph
/// - SmallWorld: Watts-Strogatz small-world network
/// - ScaleFree: Barabási–Albert preferential attachment
/// - Mesh: Full mesh (every node connected)
/// - Ring: Simple ring topology

use phantom_core::NetworkGraph;
use phantom_core::network::NodeInfo;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use rand::Rng;

/// Network topology type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TopologyType {
    /// Random graph (Erdős–Rényi)
    Random,
    
    /// Small-world network (Watts-Strogatz)
    SmallWorld,
    
    /// Scale-free network (Barabási–Albert)
    ScaleFree,
    
    /// Full mesh (all nodes connected)
    Mesh,
    
    /// Ring topology
    Ring,
}

/// Network topology generator
pub struct TopologyGenerator {
    topology_type: TopologyType,
}

impl TopologyGenerator {
    pub fn new(topology_type: TopologyType) -> Self {
        Self { topology_type }
    }
    
    /// Generate network topology with specified number of nodes
    pub fn generate(&self, num_nodes: usize) -> Result<NetworkGraph> {
        tracing::info!("Generating {:?} topology with {} nodes", self.topology_type, num_nodes);
        
        let graph = match self.topology_type {
            TopologyType::Random => self.generate_random(num_nodes)?,
            TopologyType::SmallWorld => self.generate_small_world(num_nodes)?,
            TopologyType::ScaleFree => self.generate_scale_free(num_nodes)?,
            TopologyType::Mesh => self.generate_mesh(num_nodes)?,
            TopologyType::Ring => self.generate_ring(num_nodes)?,
        };
        
        let stats = graph.stats();
        tracing::info!("Topology generated: {} nodes, {} edges, avg degree {:.2}",
                      stats.node_count, stats.edge_count, stats.avg_degree);
        
        Ok(graph)
    }
    
    /// Generate random graph (Erdős–Rényi model)
    fn generate_random(&self, num_nodes: usize) -> Result<NetworkGraph> {
        let mut graph = NetworkGraph::new();
        let edge_probability = 0.1;  // 10% edge probability
        
        // Add nodes
        for i in 0..num_nodes {
            let info = create_default_node_info((i as u32) + 1);
            graph.add_node(info);
        }
        
        // Add random edges
        let mut rng = rand::thread_rng();
        for i in 0..num_nodes {
            for j in (i + 1)..num_nodes {
                if rng.gen_bool(edge_probability) {
                    graph.add_edge((i as u32) + 1, (j as u32) + 1);
                    graph.add_edge((j as u32) + 1, (i as u32) + 1);  // Bidirectional
                }
            }
        }
        
        Ok(graph)
    }
    
    /// Generate small-world network (Watts-Strogatz model)
    fn generate_small_world(&self, num_nodes: usize) -> Result<NetworkGraph> {
        let mut graph = NetworkGraph::new();
        let k = 4;  // Each node connects to k nearest neighbors
        let _beta = 0.1;  // Rewiring probability
        
        // Add nodes
        for i in 0..num_nodes {
            let info = create_default_node_info((i as u32) + 1);
            graph.add_node(info);
        }
        
        // Create ring lattice
        for i in 0..num_nodes {
            for j in 1..=(k / 2) {
                let neighbor = (i + j) % num_nodes;
                graph.add_edge((i as u32) + 1, (neighbor as u32) + 1);
                graph.add_edge((neighbor as u32) + 1, (i as u32) + 1);
            }
        }
        
        // TODO: Implement rewiring with probability beta
        // For now, just return the ring lattice
        
        Ok(graph)
    }
    
    /// Generate scale-free network (Barabási–Albert model)
    fn generate_scale_free(&self, num_nodes: usize) -> Result<NetworkGraph> {
        let mut graph = NetworkGraph::new();
        let m = 3;  // Number of edges to attach from new node
        
        // Start with small complete graph
        for i in 0..m {
            let info = create_default_node_info((i as u32) + 1);
            graph.add_node(info);
        }
        
        for i in 0..m {
            for j in (i + 1)..m {
                graph.add_edge((i as u32) + 1, (j as u32) + 1);
                graph.add_edge((j as u32) + 1, (i as u32) + 1);
            }
        }
        
        // Add remaining nodes with preferential attachment
        for i in m..num_nodes {
            let info = create_default_node_info((i as u32) + 1);
            graph.add_node(info);
            
            // TODO: Implement preferential attachment based on node degree
            // For now, just connect to first m nodes
            for j in 0..m {
                graph.add_edge((i as u32) + 1, (j as u32) + 1);
                graph.add_edge((j as u32) + 1, (i as u32) + 1);
            }
        }
        
        Ok(graph)
    }
    
    /// Generate full mesh (all nodes connected)
    fn generate_mesh(&self, num_nodes: usize) -> Result<NetworkGraph> {
        let mut graph = NetworkGraph::new();
        
        // Add nodes
        for i in 0..num_nodes {
            let info = create_default_node_info((i as u32) + 1);
            graph.add_node(info);
        }
        
        // Connect all pairs
        for i in 0..num_nodes {
            for j in (i + 1)..num_nodes {
                graph.add_edge((i as u32) + 1, (j as u32) + 1);
                graph.add_edge((j as u32) + 1, (i as u32) + 1);
            }
        }
        
        Ok(graph)
    }
    
    /// Generate ring topology
    fn generate_ring(&self, num_nodes: usize) -> Result<NetworkGraph> {
        let mut graph = NetworkGraph::new();
        
        // Add nodes
        for i in 0..num_nodes {
            let info = create_default_node_info((i as u32) + 1);
            graph.add_node(info);
        }
        
        // Connect in ring
        for i in 0..num_nodes {
            let next = (i + 1) % num_nodes;
            graph.add_edge((i as u32) + 1, (next as u32) + 1);
            graph.add_edge((next as u32) + 1, (i as u32) + 1);
        }
        
        Ok(graph)
    }
}

/// Create default node info for topology generation
fn create_default_node_info(id: u32) -> NodeInfo {
    NodeInfo {
        id,
        bandwidth: 1_000_000_000,  // 1 Gbps
        latency_ms: 50,
        uptime_hours: 720,
        reputation: 0.95,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_mesh_topology() {
        let gen = TopologyGenerator::new(TopologyType::Mesh);
        let graph = gen.generate(10).unwrap();
        let stats = graph.stats();
        
        assert_eq!(stats.node_count, 10);
        // A complete graph on n nodes has n*(n-1)/2 edges. This asserted
        // n*(n-1) — the number of *directed* adjacency entries, which is
        // double the edge count stats() reports. The test never ran: the
        // workspace test build aborted on phantom-discovery long before
        // reaching this crate.
        assert_eq!(stats.edge_count, 10 * 9 / 2);
    }
    
    #[test]
    fn test_ring_topology() {
        let gen = TopologyGenerator::new(TopologyType::Ring);
        let graph = gen.generate(10).unwrap();
        let stats = graph.stats();
        
        assert_eq!(stats.node_count, 10);
        // A ring of n nodes has n edges, not 2n. Each node has two
        // neighbours, which is 2n adjacency entries and n edges.
        assert_eq!(stats.edge_count, 10);
    }
}
