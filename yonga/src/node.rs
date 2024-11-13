use std::collections::{BinaryHeap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use mongodb::Collection;
use mongodb::bson::doc;
use mongodb::options::AggregateOptions;
use futures::stream::StreamExt;
use anyhow::Result;
use log::error;
use std::cmp::Ordering;
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
    pub destination: Node,
    pub edge: f64,
}

#[derive(Debug)]
pub struct NodeTree {
    config: Config,
    tree: HashMap<Node, Vec<AggLinkEdge>>,
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

    pub fn add_edge(&mut self, source: Node, destination: Node, edge: f64) {
        if let Some(edges) = self.tree.get_mut(&source) {
            edges.push(AggLinkEdge {
                destination,
                edge,
            });
        } else {
            self.tree.insert(source, vec![AggLinkEdge {
                destination,
                edge,
            }]);
        }
    }

    // Aggregates edges and calculates weighted edges based on the 99th percentile
    pub fn aggregate_edges(&mut self, node_graph: &NodeGraph, maxmin_network: &Network) {

        //let maxmin_network = self.get_maxmin_network();

        for server_node in &node_graph.nodes {
            // Collect the network data for all edges associated with this server_node
            let mut edge_networks: HashMap<Node, Vec<Network>> = HashMap::new();
            for edge in &server_node.edges {
                // eliminate inf values from the network
                let network = edge.network.clone();

                if network.bandwidth == f64::INFINITY || network.latency == f64::INFINITY || network.packet_loss == f64::INFINITY  || network.available == f64::INFINITY {
                    // print the network
                    println!("Network with inf values: {:?}", network);
                    continue;
                }
                
                
                edge_networks.entry(edge.destination.clone())
                    .or_insert_with(Vec::new)
                    .push(edge.network.clone());
            }

            for (destination, networks) in edge_networks {
                let aggregated_edge = Network::aggregate_network(self.config.clone(), &networks, maxmin_network);
                self.add_edge(server_node.node.clone(), destination, aggregated_edge);
            }
        }
    }

    // A function to print the tree
    pub fn print_tree(&self) {
        for (node, edges) in &self.tree {
            println!("Node: {}", node.name);
            for edge in edges {
                println!("  -> {} (Weighted Edge: {:?})", edge.destination.name, edge.edge);
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

    // Compute the best paths (minimal cost) for each node to a level
    pub fn compute_best_paths(&self, nodes: Vec<Node>, level: usize) -> HashMap<Node, (Vec<Node>, f64)> {
        let mut best_paths: HashMap<Node, (Vec<Node>, f64)> = HashMap::new();

        for start in nodes.iter() {
            let mut heap = BinaryHeap::new();
            let mut path_costs: HashMap<Node, (f64, Vec<Node>)> = HashMap::new();  // Track the minimal cost and path
            let mut visited: HashSet<Node> = HashSet::new();

            // Push the start node with cost 0
            heap.push((ComparableFloat(0.0), vec![start.clone()]));
            visited.insert(start.clone());

            while let Some((current_cost, path)) = heap.pop() {
                let current_node = path.last().unwrap();

                // Stop if the path length exceeds the desired level
                if path.len() > level + 1 {
                    continue;
                }

                // Update paths if this one has a lower cost or is not yet recorded
                if let Some((best_cost, _)) = path_costs.get(current_node) {
                    if current_cost.0 >= *best_cost {
                        continue;
                    }
                }

                path_costs.insert(current_node.clone(), (current_cost.0, path.clone()));

                // Explore neighbors to update costs
                if let Some(neighbors) = self.tree.get(current_node) {
                    for edge in neighbors {
                        if !path.contains(&edge.destination) {
                            let mut new_path = path.clone();
                            new_path.push(edge.destination.clone());
                            let new_cost = ComparableFloat(current_cost.0 + edge.edge);

                            // Add the new path to the heap if it's cheaper
                            if new_cost.0 < current_cost.0 {
                                heap.push((new_cost, new_path.clone()));
                            }
                        }
                    }
                }
            }

            // Collect the best (minimal cost) path for this start node
            let mut ordered_paths: Vec<_> = path_costs.into_iter().collect();
            ordered_paths.sort_by(|(_, (cost1, _)), (_, (cost2, _))| 
                cost1.partial_cmp(&cost2).unwrap_or(Ordering::Equal)  // Sort by cost in ascending order
            );

            // Get the minimal cost path
            if let Some((_, (best_cost, best_path))) = ordered_paths.first() {
                best_paths.insert(start.clone(), (best_path.clone(), *best_cost));
            }
        }

        best_paths
    }

    // A function that takes all the strong paths and returns one with the lowest path cost
    pub fn get_best_path(&self, strong_paths: HashMap<Node, (Vec<Node>, f64)>) -> (Vec<Node>, f64) {
        let mut best_path = (Vec::new(), f64::INFINITY);

        for (_, (path, cost)) in strong_paths {
            if cost < best_path.1 {
                best_path = (path, cost);
            }
        }

        best_path
    }

    // A function to get nodes in the tree
    pub fn get_nodes(&self) -> Vec<Node> {
        self.tree.keys().cloned().collect()
    }


    pub fn get_tree(&self) -> &HashMap<Node, Vec<AggLinkEdge>> {
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
            if let Ok(node_entries) = Self::get_latest_node_entries_aggregation(&collection, limit).await {
                for entry in node_entries {
                    let node = entry.metadata.clone();
                    if let Some(server_node) = self.nodes.iter_mut().find(|n| n.node == node) {
                        for env in entry.environment {
                            let destination = env.node.clone();
                            let edge = LinkEdge {
                                destination,
                                network: env.network,
                            };
                            server_node.edges.push(edge);
                        }
                    } else {
                        eprintln!("Node not found in graph: {}", node.name);
                    }
                }
            } else {
                eprintln!("Error fetching node entries from collection: {:?}", collection.name());
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
        let mut network = Network {
            bandwidth: 0.0,
            available: 0.0,
            latency: f64::INFINITY, // Initialize to infinity
            packet_loss: f64::INFINITY, // Initialize to infinity
        };

        for node in &self.nodes {
            for edge in &node.edges {
                network.bandwidth = network.bandwidth.max(edge.network.bandwidth);
                network.available = network.available.max(edge.network.available);
                network.latency = network.latency.min(edge.network.latency);
                network.packet_loss = network.packet_loss.min(edge.network.packet_loss);
            }
        }

        // Adjust packet loss if it's zero
        if network.packet_loss == 0.0 {
            network.packet_loss = 0.0001; // Set to an arbitrary low value
        }

        // Adjust latency if it's zero
        if network.latency == 0.0 {
            network.latency = 0.0001; // Set to an arbitrary low value
        }

        network
    }
}

// // Main function to execute the NodeGraph construction
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let matches = Command::new("YongaNode")
//     .arg(Arg::new("config")
//         .long("config")
//         .short('c')
//         .required(true)
//         .action(ArgAction::Set))
//     .get_matches();

//     let config = matches.get_one::<String>("config").unwrap();    
    
//     // parse the config file
//     let config_str = fs::read_to_string(config).expect("Failed to read configuration file");
//     let config: Config = serde_yaml::from_str(&config_str).expect("Failed to parse configuration file");

//     // Initialize logging
//     env_logger::init();

//     println!("Setting up the NodeGraph...");

//     // Connect to MongoDB
//     let client = Client::with_uri_str(&config.database.uri).await?;
//     let database = client.database(&config.database.db);

//     let nodes = config.cluster.nodes.clone();

//     // create collections from nodes
//     let node_collections = nodes.iter().map(|node| {
//         database.collection::<NodeEntryMongo>(&node.name)
//     }).collect();

//     // Create a NodeGraph instance
//     let mut node_graph = NodeGraph::new(nodes.clone());

//     // Build the graph with the collections and limit
//     node_graph.build(node_collections, 50).await?;

//     // Print the entire node graph
//     // node_graph.print_graph();

//     // Create a NodeTree instance
//     let mut node_tree = NodeTree::new(config);

//     // Aggregate the edges
//     node_tree.aggregate_edges(&node_graph);

//     // Print the aggregated tree
//     node_tree.print_tree();

//     // get the strongest paths for each node
//     let level = 1;
//     let strong_paths = node_tree.compute_strong_paths(nodes.clone(), level);

//     // Print the results
//     for (start_node, (path, strength)) in strong_paths.clone() {
//         println!("Strongest path from node {}:", start_node.name);
//         println!("Total strength: {:.6}", strength);
//         for node in path {
//             println!("-> Node {} ({}:{})", node.name, node.ip, node.id);
//         }
//         println!();  // Blank line for readability
//     }

//     // print the strongest path
//     let strongest_path = node_tree.get_strongest_path(strong_paths);
//     println!("Strongest path overall:");
//     println!("Total strength: {:.6}", strongest_path.1);
//     for node in strongest_path.0 {
//         println!("-> Node {} ({}:{})", node.name, node.ip, node.id);
//     }


//     Ok(())
// }