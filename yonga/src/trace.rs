use serde::{Deserialize, Serialize};
use serde_json::Value;
use chrono::{DateTime, Utc};
use mongodb::Collection;
use mongodb::bson::doc;
// use mongodb::Client;
// use tokio;
use futures::stream::StreamExt;
use anyhow::Result;
use std::{collections::{HashMap, HashSet}, hash::Hash};

use crate::utility::{self, Service};


#[derive(Debug, Serialize, Deserialize, Clone)]
struct Process {
    serviceName: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TraceEntry {
    traceID: String,
    spanID: String,
    operationName: String,
    #[serde(deserialize_with = "deserialize_date_time")]
    startTime: DateTime<Utc>,
    duration: i64,
    references: Vec<serde_json::Value>,
    process: Process,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TreeNode {
    entry: TraceEntry,
    children: Vec<TreeNode>,
}

#[derive(Debug, Default, Clone)]
struct EdgeAttributes {
    durations: Vec<i64>, // Store durations to compute percentiles
    message_count: usize,
}

// Custom deserializer function for DateTime
fn deserialize_date_time<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let bson_date = bson::DateTime::deserialize(deserializer)?;
    Ok(bson_date.to_chrono())
}

// Function to get trace entries from MongoDB
pub async fn get_trace_entries(collection: &Collection<TraceEntry>, limit: i64) -> Result<Vec<TraceEntry>> {
    let cursor = collection.find(None, None).await?;
    let mut trace_entries = Vec::new();
    
    let mut cursor = cursor.take(limit as usize);

    while let Some(doc) = cursor.next().await {
        match doc {
            Ok(entry) => trace_entries.push(entry),
            Err(e) => eprintln!("Error reading document: {}", e),
        }
    }

    Ok(trace_entries)
}

// Function to build trees from trace entries
pub fn build_trees(entries: Vec<TraceEntry>) -> HashMap<String, TreeNode> {
    let mut node_map: HashMap<String, TreeNode> = HashMap::new();
    let mut roots: HashMap<String, TreeNode> = HashMap::new();

    for entry in entries.clone() {
        let node = TreeNode {
            entry: entry.clone(),
            children: Vec::new(),
        };
        node_map.insert(entry.spanID.clone(), node);
    }

    for entry in entries {
        let is_root = entry.references.is_empty();
        let mut has_parent = false;

        for reference in entry.references {
            if let Some(parent_span_id) = reference.get("spanID").and_then(Value::as_str) {
                if let Some(child_node) = node_map.remove(&entry.spanID) {
                    if let Some(parent_node) = node_map.get_mut(parent_span_id) {
                        parent_node.children.push(child_node);
                        has_parent = true;
                    } else {
                        // If parent is not found, insert the child back into node_map
                        node_map.insert(entry.spanID.clone(), child_node);
                    }
                }
            }
        }

        if is_root || !has_parent {
            if let Some(root_node) = node_map.remove(&entry.spanID) {
                roots.insert(entry.spanID.clone(), root_node);
            }
        }
    }

    roots
}

#[derive(Debug, Clone)]
struct ServiceNode {
    name: String,
    edges: HashMap<String, EdgeAttributes>,
}

#[derive(Debug, Clone)]
pub struct ServiceGraph {
    nodes: HashMap<String, ServiceNode>,
}

impl ServiceGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, from: String, to: String, duration: i64) {
        let node = self.nodes.entry(from.clone()).or_insert_with(|| ServiceNode {
            name: from.clone(),
            edges: HashMap::new(),
        });

        let edge = node.edges.entry(to.clone()).or_insert_with(EdgeAttributes::default);

        // Accumulate the duration and increment the message count
        edge.durations.push(duration);
        edge.message_count += 1;

        // Ensure the destination node exists in the graph
        self.nodes.entry(to.clone()).or_insert_with(|| ServiceNode {
            name: to,
            edges: HashMap::new(),
        });
    }

    pub fn extract_edges(&mut self, node: &TreeNode, path: Vec<String>) {
        let mut current_path = path.clone();
        current_path.push(node.entry.process.serviceName.clone());

        for child in &node.children {
            let child_process = &child.entry.process.serviceName;

            for i in 0..current_path.len() - 1 {
                let from = current_path[i].clone();
                let to = child_process.clone();

                self.add_edge(from, to, child.entry.duration);
            }

            self.extract_edges(child, current_path.clone());
        }
    }

    pub fn calculate_percentile(durations: &[i64], percentile: f64) -> f64 {
        if durations.is_empty() {
            return 0.0;
        }
    
        // Sort durations
        let mut sorted_durations: Vec<i64> = durations.to_vec();
        sorted_durations.sort();
    
        // Calculate rank
        let rank = percentile / 100.0 * (sorted_durations.len() as f64);
        let lower_index = rank.floor() as usize;
        let upper_index = rank.ceil() as usize;
    
        // Handle edge cases
        if lower_index == upper_index {
            return sorted_durations[lower_index] as f64 / 1000.0; // Convert from microseconds to milliseconds
        }
    
        // Interpolate between lower and upper values
        let lower_value = sorted_durations.get(lower_index).unwrap_or(&0);
        let upper_value = sorted_durations.get(upper_index).unwrap_or(&0);
    
        ((*lower_value as f64 + *upper_value as f64) / 2.0) / 1000.0 // Convert from microseconds to milliseconds
    }

    pub fn build_from_traces(&mut self, trees: &HashMap<String, TreeNode>) {
        for root in trees.values() {
            self.extract_edges(root, vec![]);
        }
    }

    pub fn print_graph(&self) {
        for node in self.nodes.values() {
            println!("Service: {}", node.name);
            for (destination, edge_attributes) in &node.edges {
                let duration_percentile_99 = Self::calculate_percentile(&edge_attributes.durations, 99.0);
                println!(
                    "  -> {} (99th Percentile Duration: {:.2} ms, Message Count: {})",
                    destination, duration_percentile_99, edge_attributes.message_count
                );
            }
        }
    }

    pub fn longest_paths(&self) -> (Vec<Vec<String>>, usize) {
        let mut longest_paths = Vec::new();
        let mut max_length = 0;

        for node in self.nodes.keys() {
            let mut visited = HashSet::new();
            let mut current_path = Vec::new();
            self.dfs_longest_paths(node, &mut visited, &mut current_path, &mut longest_paths, &mut max_length);
        }

        (longest_paths, max_length)
    }

    fn dfs_longest_paths(
        &self,
        current: &String,
        visited: &mut HashSet<String>,
        current_path: &mut Vec<String>,
        longest_paths: &mut Vec<Vec<String>>,
        max_length: &mut usize,
    ) {
        visited.insert(current.clone());
        current_path.push(current.clone());

        let neighbors = &self.nodes[current].edges.keys().cloned().collect::<Vec<String>>();

        if neighbors.is_empty() {
            if current_path.len() > *max_length {
                *max_length = current_path.len();
                longest_paths.clear();
                longest_paths.push(current_path.clone());
            } else if current_path.len() == *max_length {
                longest_paths.push(current_path.clone());
            }
        } else {
            for neighbor in neighbors {
                if !visited.contains(neighbor.as_str()) {
                    self.dfs_longest_paths(&neighbor, visited, current_path, longest_paths, max_length);
                }
            }
        }

        visited.remove(current);
        current_path.pop();
    }

    // Function to find the most popular services (those that connect to the most other services)
    pub fn most_popular_services(&self) -> (Vec<String>, usize) {
        let mut most_popular_services = Vec::new();
        let mut max_connections = 0;

        for (service, node) in &self.nodes {
            let connections = node.edges.len();
            if connections > max_connections {
                max_connections = connections;
                most_popular_services.clear();
                most_popular_services.push(service.clone());
            } else if connections == max_connections {
                most_popular_services.push(service.clone());
            }
        }

        (most_popular_services, max_connections)
    }

    // retrieve services from the Service Graph
    pub fn get_services(&self) -> Vec<String> {
        let mut services = Vec::new();
        for (service, _) in &self.nodes {
            services.push(service.clone());
        }
        services
    }

    // get service pairs from the Service Graph
    pub fn get_service_pairs(&self, services: Vec<Service>) -> HashMap<(Service, Service), (u32, f64)> {
        let mut service_pairs = HashMap::new();
        for service in &services {
            let service_name = service.name.clone();
            if let Some(node) = self.nodes.get(&service_name) {
                for (destination, params) in &node.edges {
                    let destination_service = utility::get_service_by_name(destination.clone(), &services).unwrap();
                    let (message_count, duration) = (params.message_count, Self::calculate_percentile(&params.durations, 99.0));
                    service_pairs.insert((service.clone(), destination_service), (message_count as u32, duration));
                }
            }
        }
        service_pairs
    }
}


// // Use the ServiceGraph within the main function
// #[tokio::main]
// async fn main() -> Result<(), Box<dyn std::error::Error>> {
//     println!("Starting the trace analysis");

//     // Connect to MongoDB
//     let client = Client::with_uri_str("mongodb://mongoadmin:949cad0977fb8daf334e@196.32.213.62:27017/").await?;
//     let database = client.database("yonga");
//     let collection = database.collection::<TraceEntry>("trace");

//     // Retrieve trace entries with a limit
//     let limit = 1000000000; // This can be any variable number
//     let trace_entries = get_trace_entries(&collection, limit).await?;

//     let trees = build_trees(trace_entries);

//     let mut service_graph = ServiceGraph::new();
//     service_graph.build_from_traces(&trees);

//     // Print the entire service graph
//     service_graph.print_graph();

//     // Find and print the longest paths
//     let (longest_paths, max_length) = service_graph.longest_paths();
//     println!("\nLongest Paths (Length: {}):", max_length);
//     for path in longest_paths {
//         println!("  {:?}", path);
//     }

//     // Find and print the most popular services
//     let (most_popular_services, max_connections) = service_graph.most_popular_services();
//     println!("\nMost Popular Services (Connections: {}):", max_connections);
//     for service in most_popular_services {
//         println!("  {}", service);
//     }

//     Ok(())
// }