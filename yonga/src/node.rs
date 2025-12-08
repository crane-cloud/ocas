use std::collections::{BinaryHeap, HashMap};

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use mongodb::Collection;
use mongodb::bson::doc;
use mongodb::options::AggregateOptions;
use futures::stream::StreamExt;
use anyhow::Result;
use log::error;
use std::cmp::Ordering;
use std::cmp::Reverse;
// use clap::{Arg, Command, ArgAction};
// use std::fs;

use crate::utility::{Node, EnvironmentMetric, Network, Config};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NodeEntryMongo {
    #[serde(deserialize_with = "deserialize_date_time")]
    timestamp: DateTime<Utc>,
    metadata: Node,
    environment: Vec<EnvironmentMetric>,
}

#[derive(Debug, Clone)]
struct ServerNode {
    node: Node,
    edges: Vec<LinkEdge>,
}

#[derive(Debug, Clone)]
struct LinkEdge {
    destination: Node,
    network: Network,
}

#[derive(Debug, Clone)]
pub struct NodeGraph {
    nodes: Vec<ServerNode>,
}


#[derive(Debug, Clone)]
pub struct AggLinkEdge {
    pub destination: String,
    pub edge: f64,
}

#[derive(Debug)]
pub struct NodeTree {
    config: Config,
    tree: HashMap<String, Vec<AggLinkEdge>>,   // src → [(dst, cost)]
}


// Wrapper to allow ordering of floats
#[derive(Debug, Clone, Copy)]
struct ComparableFloat(f64);

impl PartialEq for ComparableFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl Eq for ComparableFloat {}

impl PartialOrd for ComparableFloat {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl Ord for ComparableFloat {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse the order for max-heap (largest value comes first)
        other.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
    }
}


impl NodeTree {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            tree: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, source: String, destination: String, edge: f64) {
        let edges = self.tree.entry(source).or_insert_with(Vec::new);

        // check if dest already exists — update cost instead of duplicate
        if let Some(existing) = edges.iter_mut().find(|e| e.destination == destination) {
            existing.edge = edge;
        } else {
            edges.push(AggLinkEdge { destination, edge });
        }
    }

    // Aggregates edges and calculates weighted edges based on the 99th percentile
    pub fn aggregate_edges(&mut self, node_graph: &NodeGraph, maxmin_network: &Network) {
        use std::collections::HashMap;

        self.tree.clear(); // fresh aggregation pass

        println!("\n=== Aggregating Edges Into NodeTree ===");

        // For each source node in the raw graph
        for server_node in &node_graph.nodes {
            let src = server_node.node.clone();

            // Group networks by DESTINATION NAME (not full Node — avoids duplicates)
            let mut edge_networks: HashMap<String, Vec<Network>> = HashMap::new();

            for edge in &server_node.edges {
                let dst_name = edge.destination.name.clone();

                // Skip infinite values
                let net = &edge.network;
                if net.bandwidth.is_infinite()
                    || net.latency.is_infinite()
                    || net.packet_loss.is_infinite()
                    || net.available.is_infinite()
                {
                    println!(
                        "Skipping INF sample {} -> {}: {:?}",
                        src.name, dst_name, net
                    );
                    continue;
                }

                // Group samples under destination by NAME
                edge_networks
                    .entry(dst_name)
                    .or_insert_with(Vec::new)
                    .push(net.clone());
            }

            // Now aggregate each group
            for (dst_name, samples) in edge_networks {
                if samples.is_empty() {
                    continue;
                }

                let agg_cost = Network::aggregate_network(self.config.clone(), &samples, maxmin_network);

                self.add_edge(src.name.clone(), dst_name.clone(), agg_cost);

                println!(
                    "Aggregated edge: {} -> {} | samples={} | cost={:.6}",
                    src.name, dst_name, samples.len(), agg_cost
                );
            }
        }

        println!("=== Aggregation Complete ===\n");
    }

    // A function to print the tree
    pub fn print_tree(&self) {
        for (src, edges) in &self.tree {
            println!("Node {}", src);
            for edge in edges {
                println!("  -> {} (cost = {:.6})", edge.destination, edge.edge);
            }
        }
    }

    // A function to print the graph
    pub fn print_graph(&self) {
        println!("=== Final Aggregated NodeGraph ===");
        for (src, edges) in &self.tree {
            println!("Node {}", src);
            for edge in edges {
                println!("  -> {} (cost = {:.6})", edge.destination, edge.edge);
            }
        }        
    }

    // A function to get the highest weighted edge overall
    pub fn get_worst_cost(&self) -> f64 {
        let mut worst_cost = 0.0;

        for (_, edges) in &self.tree {
            for edge in edges {
                if edge.edge > worst_cost {
                    worst_cost = edge.edge;
                }
            }
        }

        worst_cost
    }

    /// Computes minimal-cost paths from each starting node (name strings)
    /// to any reachable node, limited by a maximum path length (level).
    /// Returns: start_node -> (best_path, best_cost)
    pub fn compute_best_paths(
        &self,
        nodes: Vec<String>,
        level: usize,
    ) -> HashMap<String, (Vec<String>, f64)> 
    {
        let mut best_paths: HashMap<String, (Vec<String>, f64)> = HashMap::new();

        for start in nodes.iter() {
            // Min-heap: cost, node_name, path
            let mut heap: BinaryHeap<
                Reverse<(ComparableFloat, String, Vec<String>)>
            > = BinaryHeap::new();

            // Best known distances from start
            let mut best_costs: HashMap<String, f64> = HashMap::new();

            heap.push(Reverse((
                ComparableFloat(0.0),
                start.clone(),
                vec![start.clone()],
            )));
            best_costs.insert(start.clone(), 0.0);

            while let Some(Reverse((ComparableFloat(curr_cost), curr_node, curr_path))) = heap.pop() {
                // If this entry is not optimal anymore, skip it
                if let Some(&known) = best_costs.get(&curr_node) {
                    if curr_cost > known {
                        continue;
                    }
                }

                // Too long: max nodes = level + 1
                if curr_path.len() > level + 1 {
                    continue;
                }

                // Explore neighbors
                if let Some(neighs) = self.tree.get(&curr_node) {
                    for edge in neighs {
                        let next = edge.destination.clone();
                        let next_cost = curr_cost + edge.edge;

                        // Relax condition
                        let push = match best_costs.get(&next) {
                            Some(&known) => next_cost < known,
                            None => true,
                        };

                        if push {
                            best_costs.insert(next.clone(), next_cost);

                            let mut new_path = curr_path.clone();
                            new_path.push(next.clone());

                            heap.push(Reverse((
                                ComparableFloat(next_cost),
                                next.clone(),
                                new_path,
                            )));
                        }
                    }
                }
            }

            // Choose the best target
            if best_costs.is_empty() {
                best_paths.insert(start.clone(), (vec![start.clone()], 0.0));
                continue;
            }

            let (target, &best_cost) = best_costs
                .iter()
                .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap();

            // Reconstruct path using predecessor map
            let mut prev: HashMap<String, String> = HashMap::new();
            let mut dist: HashMap<String, f64> = HashMap::new();
            let mut heap2: BinaryHeap<Reverse<(ComparableFloat, String)>> = BinaryHeap::new();

            dist.insert(start.clone(), 0.0);
            heap2.push(Reverse((ComparableFloat(0.0), start.clone())));

            // Run Dijkstra again (small search) to reconstruct path
            while let Some(Reverse((ComparableFloat(d), u))) = heap2.pop() {
                if let Some(&best_known) = dist.get(&u) {
                    if d > best_known {
                        continue;
                    }
                }

                if u == *target {
                    break;
                }

                if let Some(neighs) = self.tree.get(&u) {
                    for e in neighs {
                        let v = e.destination.clone();
                        let alt = d + e.edge;

                        if alt < *dist.get(&v).unwrap_or(&f64::INFINITY) {
                            dist.insert(v.clone(), alt);
                            prev.insert(v.clone(), u.clone());
                            heap2.push(Reverse((ComparableFloat(alt), v.clone())));
                        }
                    }
                }
            }

            // Build final path
            let mut path_rev = vec![target.clone()];
            let mut cur = target.clone();

            while cur != *start {
                if let Some(p) = prev.get(&cur) {
                    cur = p.clone();
                    path_rev.push(cur.clone());
                } else {
                    // Could not reconstruct full path—fallback to trivial
                    path_rev.clear();
                    path_rev.push(start.clone());
                    break;
                }
            }

            path_rev.reverse();
            best_paths.insert(start.clone(), (path_rev, best_cost));
        }

        best_paths
    }   

    // A function that takes all the strong paths and returns one with the lowest path cost
    pub fn get_best_path(
        &self, 
        strong_paths: HashMap<String, (Vec<String>, f64)>
    ) -> (Vec<String>, f64) 
    {
        let mut best: (Vec<String>, f64) = (Vec::new(), f64::INFINITY);

        for (_, (path, cost)) in strong_paths {
            if cost < best.1 {
                best = (path, cost);
            }
        }

        best
    }

    // A function to get nodes in the tree
    pub fn get_nodes(&self) -> Vec<String> {
        self.tree.keys().cloned().collect()
    }


    pub fn get_tree(&self) -> &HashMap<String, Vec<AggLinkEdge>> {
        &self.tree
    }

}

// Custom deserializer function for DateTime
fn deserialize_date_time<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bson_date = bson::DateTime::deserialize(deserializer)?;
    Ok(bson_date.to_chrono())
}

impl NodeGraph {
    pub fn new(nodes: Vec<Node>) -> Self {
        let server_nodes = nodes.into_iter().map(|node| ServerNode {
            node,
            edges: Vec::new(),
        }).collect();

        Self {
            nodes: server_nodes,
        }
    }

    pub async fn build(
        &mut self,
        collections: Vec<Collection<NodeEntryMongo>>,
        limit: i64,
    ) -> Result<()> {

        for collection in collections {
            let entries = Self::get_latest_node_entries_aggregation(&collection, limit).await?;

            // // ----------------------------------------------------
            // // DEBUG PREVIEW: Print first 10 entries from MongoDB
            // // ----------------------------------------------------
            // println!("\n=== Previewing first 10 entries for collection '{}' ===",
            //     collection.name()
            // );

            // for (i, entry) in entries.iter().take(10).enumerate() {
            //     println!("--- Entry {} ---", i);
            //     println!("timestamp: {:?}", entry.timestamp);
            //     println!("metadata:  name='{}' id={} ip={}",
            //         entry.metadata.name,
            //         entry.metadata.id,
            //         entry.metadata.ip
            //     );

            //     for (j, env) in entry.environment.iter().enumerate() {
            //         println!(
            //             "  env[{}]: dst='{}' | network={{ available={}, bandwidth={}, latency={}, packet_loss={} }}",
            //             j,
            //             env.node.name,
            //             env.network.available,
            //             env.network.bandwidth,
            //             env.network.latency,
            //             env.network.packet_loss
            //         );
            //     }
            // }

            // println!("=== End of preview for '{}' ===\n", collection.name());            

            for entry in entries {
                let source_name = &entry.metadata.name;

                // match node by name (IDs may differ, resources differ)
                if let Some(server_node) = self.nodes.iter_mut().find(|n| n.node.name == *source_name) {

                    // push all samples (historical)
                    for env in entry.environment {
                        server_node.edges.push(LinkEdge {
                            destination: env.node.clone(),
                            network: env.network.clone(),
                        });
                    }

                } else {
                    log::warn!(
                        "Node not found in graph: name='{}', id={}, ip='{}' (MongoDB entry). \
                        Graph contains nodes: [{}]",
                        source_name,
                        entry.metadata.id,
                        entry.metadata.ip,
                        self.nodes
                            .iter()
                            .map(|n| format!("{}(id={}, ip={})", n.node.name, n.node.id, n.node.ip))
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
        }

        Ok(())
    }


    async fn get_latest_node_entries_aggregation(
        collection: &Collection<NodeEntryMongo>,
        limit: i64,
    ) -> Result<Vec<NodeEntryMongo>> {
        let pipeline = vec![
            doc! { "$sort": { "timestamp": -1 } },  // Sort by timestamp descending
            doc! { "$limit": limit },  // Limit the number of results
        ];

        let options = AggregateOptions::default();
        let mut cursor = collection.aggregate(pipeline, options).await?;
        let mut node_entries = Vec::new();

        while let Some(doc) = cursor.next().await {
            match doc {
                Ok(document) => {
                    if let Ok(entry) = mongodb::bson::from_document::<NodeEntryMongo>(document.clone()) {
                        node_entries.push(entry);
                    } else {
                        error!("Deserialization error for document: {:?}", document);
                    }
                }
                Err(e) => error!("Error reading document: {}", e),
            }
        }

        Ok(node_entries)
    }

    pub fn print_graph(&self) {
        for node in &self.nodes {
            println!("Node: {:?}", node.node);
            for edge in &node.edges {
                println!(
                    "  -> {:?} (Network: {:?} )",
                    edge.destination, edge.network
                );
            }
        }
    }

    // A function to determine the max. bandwidth, max. available, min. latency, and min. packet loss for the whole node graph
    pub fn get_maxmin_network(&self) -> Network {
        let mut all_available = Vec::new();
        let mut all_bandwidth = Vec::new();
        let mut all_latency = Vec::new();
        let mut all_packet_loss = Vec::new();

        // Collect all samples from all edges
        for node in &self.nodes {
            for edge in &node.edges {
                all_available.push(edge.network.available);
                all_bandwidth.push(edge.network.bandwidth);
                all_latency.push(edge.network.latency);
                all_packet_loss.push(edge.network.packet_loss);
            }
        }

        // Ensure we have samples
        if all_available.is_empty() {
            return Network::default();
        }

        // Sort each metric
        all_available.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_bandwidth.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_latency.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all_packet_loss.sort_by(|a, b| a.partial_cmp(b).unwrap());

        // Use robust baselines (p95/p99)
        let idx95 = ((all_available.len() as f64) * 0.95).floor() as usize;
        let idx05 = ((all_latency.len() as f64) * 0.05).floor() as usize;

        // Construct proper max/min baseline
        Network {
            available: all_available[idx95],    // global good-availability baseline
            bandwidth: all_bandwidth[idx95],    // global good-bandwidth baseline
            latency: all_latency[idx05],        // global low-latency baseline
            packet_loss: all_packet_loss[idx05] // global low-loss baseline
        }
    }
}


// #[cfg(test)]
// mod tests {
//     use super::*;

//     // Helper to create a mock Node quickly.
//     fn node(name: &str) -> Node {
//         Node {
//             name: name.to_string(),
//             ip: "127.0.0.1".into(),
//             id: 0,
//         }
//     }

//     fn edge(dst: &Node, cost: f64) -> AggLinkEdge {
//         AggLinkEdge {
//             destination: dst.clone(),
//             edge: cost,
//         }
//     }

//     fn build_tree(edges: Vec<(&str, &str, f64)>) -> NodeTree {
//         let mut tree = NodeTree::new(Config::default());

//         for (src, dst, cost) in edges {
//             tree.add_edge(node(src), node(dst), cost);
//         }

//         tree
//     }

//     // ------------------------------------------------------------
//     // 1. SIMPLE SHORTEST PATH TEST
//     // ------------------------------------------------------------
//     #[test]
//     fn test_shortest_path_simple() {
//         // A -> B (1), B -> C (1), A -> C (10)
//         let mut tree = build_tree(vec![
//             ("A", "B", 1.0),
//             ("B", "C", 1.0),
//             ("A", "C", 10.0),
//         ]);

//         let nodes = vec![node("A"), node("B"), node("C")];
//         let paths = tree.compute_best_paths(nodes.clone(), 5);

//         let (path, cost) = paths.get(&node("A")).unwrap();

//         assert_eq!(cost.clone(), 2.0);
//         let names: Vec<_> = path.iter().map(|n| n.name.as_str()).collect();
//         assert_eq!(names, vec!["A", "B", "C"]);
//     }

//     // ------------------------------------------------------------
//     // 2. LEVEL LIMIT ENFORCEMENT
//     // ------------------------------------------------------------
//     #[test]
//     fn test_level_limit() {
//         // A -> B -> C -> D
//         let tree = build_tree(vec![
//             ("A", "B", 1.0),
//             ("B", "C", 1.0),
//             ("C", "D", 1.0),
//         ]);

//         let nodes = vec![node("A")];

//         // level = 1 means max path length = 2 nodes (A -> B)
//         let paths = tree.compute_best_paths(nodes.clone(), 1);

//         let (path, cost) = paths.get(&node("A")).unwrap();
//         let names: Vec<_> = path.iter().map(|n| n.name.as_str()).collect();

//         assert_eq!(names, vec!["A", "B"]);
//         assert_eq!(*cost, 1.0);
//     }

//     // ------------------------------------------------------------
//     // 3. DISCONNECTED NODE HANDLING
//     // ------------------------------------------------------------
//     #[test]
//     fn test_disconnected_nodes() {
//         // A -> B, but C is isolated
//         let tree = build_tree(vec![
//             ("A", "B", 5.0),
//         ]);

//         let nodes = vec![node("A"), node("C")];
//         let paths = tree.compute_best_paths(nodes.clone(), 5);

//         // A should find A->B
//         let (_, cost_a) = paths.get(&node("A")).unwrap();
//         assert_eq!(*cost_a, 5.0);

//         // C should return itself only with cost 0
//         let (path_c, cost_c) = paths.get(&node("C")).unwrap();
//         assert_eq!(*cost_c, 0.0);
//         let names: Vec<_> = path_c.iter().map(|n| n.name.as_str()).collect();
//         assert_eq!(names, vec!["C"]);
//     }

//     // ------------------------------------------------------------
//     // 4. TIE COSTS — ANY VALID SHORTEST PATH ACCEPTED
//     // ------------------------------------------------------------
//     #[test]
//     fn test_tie_cost_paths() {
//         // A -> B (1), A -> C (1)
//         let tree = build_tree(vec![
//             ("A", "B", 1.0),
//             ("A", "C", 1.0),
//         ]);

//         let nodes = vec![node("A")];
//         let paths = tree.compute_best_paths(nodes.clone(), 5);

//         let (path, cost) = paths.get(&node("A")).unwrap();

//         assert_eq!(cost.clone(), 1.0);

//         let names: Vec<_> = path.iter().map(|n| n.name.as_str()).collect();
//         assert!(
//             names == vec!["A", "B"] ||
//             names == vec!["A", "C"],
//             "Expected path A->B or A->C but got {:?}", names
//         );
//     }

//     // ------------------------------------------------------------
//     // 5. ASYMMETRIC GRAPH – MUST FOLLOW DIRECTED EDGES
//     // ------------------------------------------------------------
//     #[test]
//     fn test_asymmetric_graph() {
//         // A -> B (1), but B -> A (50)
//         let tree = build_tree(vec![
//             ("A", "B", 1.0),
//             ("B", "A", 50.0),
//         ]);

//         let nodes = vec![node("A")];
//         let paths = tree.compute_best_paths(nodes.clone(), 5);

//         let (path, cost) = paths.get(&node("A")).unwrap();
//         let names: Vec<_> = path.iter().map(|n| n.name.as_str()).collect();

//         assert_eq!(names, vec!["A", "B"]);
//         assert_eq!(*cost, 1.0);
//     }

//     // ------------------------------------------------------------
//     // 6. TRIVIAL SINGLE-NODE GRAPH
//     // ------------------------------------------------------------
//     #[test]
//     fn test_single_node() {
//         let tree = NodeTree::new(Config::default());
//         let nodes = vec![node("X")];

//         let paths = tree.compute_best_paths(nodes.clone(), 5);
//         let (path, cost) = paths.get(&node("X")).unwrap();

//         assert_eq!(*cost, 0.0);
//         let names: Vec<_> = path.iter().map(|n| n.name.as_str()).collect();
//         assert_eq!(names, vec!["X"]);
//     }
// }



// // // Main function to execute the NodeGraph construction
// // #[tokio::main]
// // async fn main() -> Result<(), Box<dyn std::error::Error>> {
// //     let matches = Command::new("YongaNode")
// //     .arg(Arg::new("config")
// //         .long("config")
// //         .short('c')
// //         .required(true)
// //         .action(ArgAction::Set))
// //     .get_matches();

// //     let config = matches.get_one::<String>("config").unwrap();    
    
// //     // parse the config file
// //     let config_str = fs::read_to_string(config).expect("Failed to read configuration file");
// //     let config: Config = serde_yaml::from_str(&config_str).expect("Failed to parse configuration file");

// //     // Initialize logging
// //     env_logger::init();

// //     println!("Setting up the NodeGraph...");

// //     // Connect to MongoDB
// //     let client = Client::with_uri_str(&config.database.uri).await?;
// //     let database = client.database(&config.database.db);

// //     let nodes = config.cluster.nodes.clone();

// //     // create collections from nodes
// //     let node_collections = nodes.iter().map(|node| {
// //         database.collection::<NodeEntryMongo>(&node.name)
// //     }).collect();

// //     // Create a NodeGraph instance
// //     let mut node_graph = NodeGraph::new(nodes.clone());

// //     // Build the graph with the collections and limit
// //     node_graph.build(node_collections, 50).await?;

// //     // Print the entire node graph
// //     // node_graph.print_graph();

// //     // Create a NodeTree instance
// //     let mut node_tree = NodeTree::new(config);

// //     // Aggregate the edges
// //     node_tree.aggregate_edges(&node_graph);

// //     // Print the aggregated tree
// //     node_tree.print_tree();

// //     // get the strongest paths for each node
// //     let level = 1;
// //     let strong_paths = node_tree.compute_strong_paths(nodes.clone(), level);

// //     // Print the results
// //     for (start_node, (path, strength)) in strong_paths.clone() {
// //         println!("Strongest path from node {}:", start_node.name);
// //         println!("Total strength: {:.6}", strength);
// //         for node in path {
// //             println!("-> Node {} ({}:{})", node.name, node.ip, node.id);
// //         }
// //         println!();  // Blank line for readability
// //     }

// //     // print the strongest path
// //     let strongest_path = node_tree.get_strongest_path(strong_paths);
// //     println!("Strongest path overall:");
// //     println!("Total strength: {:.6}", strongest_path.1);
// //     for node in strongest_path.0 {
// //         println!("-> Node {} ({}:{})", node.name, node.ip, node.id);
// //     }


// //     Ok(())
// // }



