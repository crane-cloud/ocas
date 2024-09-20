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
    pub fn aggregate_edges(&mut self, node_graph: &NodeGraph) {
        for server_node in &node_graph.nodes {
            // Collect the network data for all edges associated with this server_node
            let mut edge_networks: HashMap<Node, Vec<Network>> = HashMap::new();
            for edge in &server_node.edges {
                edge_networks.entry(edge.destination.clone())
                    .or_insert_with(Vec::new)
                    .push(edge.network.clone());
            }

            for (destination, networks) in edge_networks {
                let aggregated_edge = Network::aggregate_network(self.config.clone(), &networks);
                self.add_edge(server_node.node.clone(), destination, aggregated_edge);
            }
        }
    }

    pub fn print_tree(&self) {
        for (node, edges) in &self.tree {
            println!("Node: {:?}", node);
            for edge in edges {
                println!("  -> {:?} (Weighted Edge: {:?})", edge.destination, edge.edge);
            }
        }
    }


    // Compute the strongest path for each node to a level
    pub fn compute_strong_paths(&self, nodes: Vec<Node>, level: usize) -> HashMap<Node, (Vec<Node>, f64)> {
        let mut strong_paths: HashMap<Node, (Vec<Node>, f64)> = HashMap::new();
    
        for start in nodes.iter() {
            let mut heap = BinaryHeap::new();
            let mut best_paths: HashMap<Node, (f64, Vec<Node>)> = HashMap::new();  // Track the best strength and path
            let mut visited: HashSet<Node> = HashSet::new();
    
            heap.push((ComparableFloat(0.0), vec![start.clone()]));
            visited.insert(start.clone());
    
            while let Some((current_strength, path)) = heap.pop() {
                let current_node = path.last().unwrap();
    
                // Stop if the path length exceeds the desired level
                if path.len() > level + 1 {
                    continue;
                }
    
                // Update best paths if the path is stronger or not present
                if let Some((best_strength, _)) = best_paths.get(current_node) {
                    if current_strength.0 <= *best_strength {
                        continue;
                    }
                }
    
                best_paths.insert(current_node.clone(), (current_strength.0, path.clone()));
    
                if let Some(neighbors) = self.tree.get(current_node) {
                    for edge in neighbors {
                        if !path.contains(&edge.destination) {
                            let mut new_path = path.clone();
                            new_path.push(edge.destination.clone());
                            let new_strength = ComparableFloat(current_strength.0 + edge.edge);
    
                            // Add new path to heap if it's stronger
                            if new_strength.0 > current_strength.0 {
                                heap.push((new_strength, new_path.clone()));
                            }
                        }
                    }
                }
            }
    
            // Collect the strongest path and sort by strength if needed
            let mut ordered_paths: Vec<_> = best_paths.into_iter().collect();
            ordered_paths.sort_by(|(_, (strength1, _)), (_, (strength2, _))| 
                strength2.partial_cmp(&strength1).unwrap_or(Ordering::Equal)  // Sort by strength in descending order
            );
    
            // Get the strongest path
            if let Some((_, (strongest_strength, strongest_path))) = ordered_paths.first() {
                strong_paths.insert(start.clone(), (strongest_path.clone(), *strongest_strength));
            }
        }
    
        strong_paths
    }

    // A function that takes all the strong paths and returns one with the highest strength
    pub fn get_strongest_path(&self, strong_paths: HashMap<Node, (Vec<Node>, f64)>) -> (Vec<Node>, f64) {
        let mut strongest_path = (Vec::new(), 0.0);
    
        for (_, (path, strength)) in strong_paths {
            if strength > strongest_path.1 {
                strongest_path = (path, strength);
            }
        }
    
        strongest_path
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
                        eprintln!("Node not found in graph: {:?}", node);
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