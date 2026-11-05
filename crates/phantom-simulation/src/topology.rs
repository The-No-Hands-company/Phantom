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
        if num_nodes == 0 { return Ok(graph); }

        let k = 4;   // each node's degree in the initial ring lattice
        let beta = 0.1f64; // rewiring probability

        for i in 0..num_nodes {
            graph.add_node(create_default_node_info((i as u32) + 1));
        }

        let mut rng = rand::thread_rng();

        // Build a mutable adjacency list so we can rewire edges in place.
        let mut adj: Vec<std::collections::HashSet<usize>> =
            vec![std::collections::HashSet::new(); num_nodes];

        // Ring lattice: node i ↔ i+1, i+2, …, i+k/2  (mod n)
        for i in 0..num_nodes {
            for d in 1..=(k / 2) {
                let j = (i + d) % num_nodes;
                adj[i].insert(j);
                adj[j].insert(i);
            }
        }

        // Rewiring pass (considers each directed "right" edge once)
        for i in 0..num_nodes {
            for d in 1..=(k / 2) {
                if rng.gen::<f64>() >= beta { continue; }

                let old_j = (i + d) % num_nodes;

                // Pick a random new target ≠ i and not already adjacent to i
                let mut new_j = rng.gen_range(0..num_nodes);
                let mut tries = 0usize;
                while (new_j == i || adj[i].contains(&new_j)) && tries < num_nodes {
                    new_j = rng.gen_range(0..num_nodes);
                    tries += 1;
                }
                if tries == num_nodes { continue; }

                // Swap: remove i↔old_j, add i↔new_j
                adj[i].remove(&old_j);
                adj[old_j].remove(&i);
                adj[i].insert(new_j);
                adj[new_j].insert(i);
            }
        }

        // Write final edges into the graph (emit each undirected pair once)
        for i in 0..num_nodes {
            for &j in &adj[i] {
                if i < j {
                    graph.add_edge((i as u32) + 1, (j as u32) + 1);
                }
            }
        }

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
        
        // degree[i] = current degree of node i+1 in the seed graph
        let mut degree: Vec<u64> = vec![(m - 1) as u64; m];
        degree.resize(num_nodes, 0);

        let mut rng = rand::thread_rng();

        // Barabási–Albert preferential attachment
        for i in m..num_nodes {
            let info = create_default_node_info((i as u32) + 1);
            graph.add_node(info);

            let total_degree: u64 = degree[..i].iter().sum();
            let attach_m = m.min(i);
            let mut targets: std::collections::HashSet<usize> = std::collections::HashSet::new();

            while targets.len() < attach_m {
                let chosen = if total_degree == 0 {
                    rng.gen_range(0..i)
                } else {
                    // Roulette-wheel selection proportional to degree
                    let r = rng.gen_range(0..total_degree);
                    let mut cumulative = 0u64;
                    let mut sel = 0usize;
                    for k in 0..i {
                        cumulative += degree[k];
                        if cumulative > r {
                            sel = k;
                            break;
                        }
                    }
                    sel
                };
                targets.insert(chosen);
            }

            for &t in &targets {
                graph.add_edge((i as u32) + 1, (t as u32) + 1);
                degree[i] += 1;
                degree[t] += 1;
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
