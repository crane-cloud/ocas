use rand::distributions::weighted;
use serde::{Deserialize, Deserializer, Serialize};
// use serde::de::Error as DeError;
use bson::DateTime;
use actix_web::HttpResponse;
use mongodb::{bson::doc, bson::Document, options::FindOneOptions, options::FindOptions};
use std::collections::{HashMap, HashSet};
use futures::stream::StreamExt; // For `next`

#[derive(Debug, Deserialize, Clone)]
pub struct Cluster {
    pub nodes: Vec<Node>,
    pub prometheus: Prometheus,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatabaseCollection {
    pub name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Database {
    pub uri: String,
    pub db: String,
    pub collections: Vec<DatabaseCollection>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
pub struct Service {
    pub id: String,
    pub name: String,
    pub cache: Option<String>,
    pub db: Option<String>,
}

impl Service {
    pub fn new(id: &str, name: &str, cache: Option<String>, db: Option<String>) -> Self {
        Service {
            id: id.to_string(),
            name: name.to_string(),
            cache,
            db,
        }
    }
}

// function that takes a String service name and returns a Service object
pub fn get_service_by_name(name: String, services: &Vec<Service>) -> Option<Service> {
    for service in services {
        if service.name == name {
            return Some(service.clone());
        }
    }
    None
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Weight {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub cluster: Cluster,
    pub database: Database,
    pub services: Vec<Service>,
    pub weights: Vec<Weight>,
}

// implement a function to return value of the weight when given name
impl Config {
    pub fn get_weight(&self, name: &str) -> f64 {
        for weight in &self.weights {
            if weight.name == name {
                return weight.value;
            }
        }
        0.0
    }

    // A helper function that groups services based on their relationships (db, cache)
    pub fn group_services(&self) -> Vec<HashSet<String>> {
        let mut groups: Vec<HashSet<String>> = Vec::new();
        let mut service_to_group: HashMap<String, usize> = HashMap::new();

        for service in &self.services {
            let mut current_group = HashSet::new();
            current_group.insert(service.name.clone());

            if let Some(cache) = &service.cache {
                if !cache.is_empty() {
                    current_group.insert(cache.clone());
                }
            }
            if let Some(db) = &service.db {
                if !db.is_empty() {
                    current_group.insert(db.clone());
                }
            }

            let mut merged_groups: Vec<usize> = Vec::new();
            for name in &current_group {
                if let Some(group_index) = service_to_group.get(name) {
                    merged_groups.push(*group_index);
                }
            }

            if merged_groups.is_empty() {
                let new_group_index = groups.len();
                for name in &current_group {
                    service_to_group.insert(name.clone(), new_group_index);
                }
                groups.push(current_group);
            } else {
                let mut merged_group = HashSet::new();
                for index in &merged_groups {
                    merged_group.extend(groups[*index].clone());
                }
                for name in &current_group {
                    merged_group.insert(name.clone());
                }
                let main_group_index = merged_groups[0];
                groups[main_group_index] = merged_group.clone();
                for name in &merged_group {
                    service_to_group.insert(name.clone(), main_group_index);
                }
                for &index in merged_groups.iter().skip(1) {
                    groups[index].clear();
                }
            }
        }

        groups.into_iter().filter(|g| !g.is_empty()).collect()
    }

    // A function to return named groups based on db or cache presence
    pub fn grouped_services(&self) -> Vec<(String, Vec<Service>)> {
        let groups = self.group_services();
        let mut grouped_services: Vec<(String, Vec<Service>)> = Vec::new();

        for group in groups {
            let mut group_services = Vec::new();
            let mut group_name = String::new();

            for service_name in &group {
                if let Some(service) = self.services.iter().find(|s| s.name == *service_name) {
                    group_services.push(service.clone());

                    // Set the main service's name as the group name if it has a cache or db
                    if group_name.is_empty() && (!service.cache.as_ref().unwrap_or(&"".to_string()).is_empty() || !service.db.as_ref().unwrap_or(&"".to_string()).is_empty()) {
                        group_name = service.name.clone();
                    }
                }
            }
            if group_name.is_empty() {
                // Fallback to the first service name if no cache/db is found
                group_name = group_services.first().unwrap().name.clone();
            }
            grouped_services.push((group_name, group_services));
        }

        grouped_services
    }    
    // A function to retrieve the Resource of a node from the config
    


}

#[derive(Debug, Deserialize, Serialize, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct Node {
    pub id: i64,
    pub name: String,
    pub ip: String,
    pub resource: ResourceInt,
}

impl Node {
    pub fn new(id: i64, name: &str, ip: &str, resource: ResourceInt) -> Self {
        Node {
            id: id,
            name: name.to_string(),
            ip: ip.to_string(),
            resource: resource,
        }
    }

    pub fn default() -> Self {
        Node {
            id: 0,
            name: "".to_string(),
            ip: "".to_string(),
            resource: ResourceInt {
                cpu: 0,
                memory: 0,
                disk: 0,
                network: 0,
            },
        }
    }

    // a function to return Resource of a node from resource
    // pub fn get_resource(&self, resource: String) -> Option<Resource> {
    //     if self.resource == resource {
    //         Some(Resource::default())
    //     } else {
    //         None
    //     }
    // }
}

// a function that takes node id and returns a Node object
pub fn get_node_by_id(id: i64, nodes: &Vec<Node>) -> Option<Node> {
    for node in nodes {
        if node.id == id {
            return Some(node.clone());
        }
    }
    None
}


// Network metrics
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Network {
    #[serde(deserialize_with = "deserialize_available")]
    pub available: f64,
    pub bandwidth: f64,
    pub latency: f64,
    pub packet_loss: f64,
}

impl Network {
    pub fn new(available: f64, bandwidth: f64, latency: f64, packet_loss: f64) -> Self {
        Network {
            available,
            bandwidth,
            latency,
            packet_loss,
        }
    }

    pub fn default() -> Self {
        Network {
            available: 0.0,
            bandwidth: 0.0,
            latency: 0.0,
            packet_loss: 0.0,
        }
    }

    pub fn aggregate_network(config: Config, net: &Vec<Network>, maxmin_network: &Network) -> f64 {
        let mut available_values: Vec<f64> = net.iter().map(|n| n.available).collect();
        let mut bandwidth_values: Vec<f64> = net.iter().map(|n| n.bandwidth).collect();
        let mut latency_values: Vec<f64> = net.iter().map(|n| n.latency).collect();
        let mut packet_loss_values: Vec<f64> = net.iter().map(|n| n.packet_loss).collect();
    
        available_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        bandwidth_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        latency_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        packet_loss_values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    
        let percentile_index = (net.len() as f64 * 0.99).ceil() as usize - 1;
    
        let p99_available = available_values[percentile_index];
        let p99_bandwidth = bandwidth_values[percentile_index];
        let p99_latency = latency_values[percentile_index];
        let p99_packet_loss = packet_loss_values[percentile_index];
    
        // Avoiding division by zero or extremely small values
        let safe_latency = if p99_latency < 1e-9 { 
            //println!("p99_latency: {}, maxmin_network.latency: {}", p99_latency, maxmin_network.latency);
            maxmin_network.latency 
        } 
        else { 
            p99_latency 
        }; // Minimum threshold for latency
        let safe_packet_loss = if p99_packet_loss < 1e-12 { 
            //println!("p99_packet_loss: {}, maxmin_network.packet_loss: {}", p99_packet_loss, maxmin_network.packet_loss);
            maxmin_network.packet_loss 
        } 
        else { 
            p99_packet_loss 
        }; // Minimum threshold for packet loss
    
        let weight_available = config.get_weight("available");
        let weight_bandwidth = config.get_weight("bandwidth");
        let weight_latency = config.get_weight("latency");
        let weight_packet_loss = config.get_weight("packet_loss");
    
        // Calculate each weighted value
        let weighted_available = weight_available * p99_available / maxmin_network.available;
        let weighted_bandwidth = weight_bandwidth * p99_bandwidth / maxmin_network.bandwidth;
    
        // Ensure that latency and packet loss do not exceed their respective weights
        let weighted_latency = weight_latency * maxmin_network.latency / safe_latency;
        let clamped_latency = weighted_latency.min(weight_latency); // Clamp to the maximum weight
    
        let weighted_packet_loss = weight_packet_loss * maxmin_network.packet_loss / safe_packet_loss;
        let clamped_packet_loss = weighted_packet_loss.min(weight_packet_loss); // Clamp to the maximum weight
    
        let weighted_value = weighted_available + weighted_bandwidth + clamped_latency + clamped_packet_loss;
    
        // Normalize the weighted value to between 0.0 and 1.0
        let normalized_value = weighted_value.min(1.0);
    
        // Invert the cost to make better network configurations have lower cost
        let cost = 1.0 - normalized_value;
    
        // Print the weighted value, normalized value, and cost
        // println!("Weighted value: {}, Normalized value: {}, Cost: {}", weighted_value, normalized_value, cost);
    
        cost
    }
}


// Custom deserializer for the `available` field
fn deserialize_available<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    let value = bson::Bson::deserialize(deserializer)?;
    match value {
        bson::Bson::Boolean(available) => Ok(if available { 1.0 } else { 0.0 }),
        bson::Bson::Double(v) => Ok(v),
        bson::Bson::Int32(v) => Ok(v as f64),
        bson::Bson::Int64(v) => Ok(v as f64),
        _ => Err(serde::de::Error::custom("Invalid type for available field")),
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Resource {
    pub cpu: f64,
    pub memory: f64,
    pub disk: f64,
    pub network: f64,
}

impl Resource {
    pub fn new(cpu: f64, memory: f64, disk: f64, network: f64) -> Self {
        Resource {
            cpu,
            memory,
            disk,
            network,
        }
    }

    pub fn default() -> Self {
        Resource {
            cpu: 0.0,
            memory: 0.0,
            disk: 0.0,
            network: 0.0,
        }
    }

    pub fn add(&mut self, other: &Resource) {
        self.cpu += other.cpu;
        self.memory += other.memory;
        self.disk += other.disk;
        self.network += other.network;
    }

    pub fn sub(&mut self, other: &Resource) {
        self.cpu -= other.cpu;
        self.memory -= other.memory;
        self.disk -= other.disk;
        self.network -= other.network;
    }
}


#[derive(Debug, Deserialize, Serialize, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct ResourceInt {
    pub cpu: u32,
    pub memory: u32,
    pub disk: u32,
    pub network: u32,
}

impl ResourceInt {
    pub fn new(cpu: u32, memory: u32, disk: u32, network: u32) -> Self {
        ResourceInt {
            cpu,
            memory,
            disk,
            network,
        }
    }

    pub fn default(node: Node) -> Self {
        ResourceInt {
            cpu: node.resource.cpu as u32,
            memory: node.resource.memory as u32,
            disk: node.resource.disk as u32,
            network: node.resource.network as u32,
        }
    }
}



#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EnvironmentMetric {
    pub node: Node,
    pub network: Network,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Prometheus {
    pub url: String,
    pub label: String,
    pub stack: String,
    pub query: String,
    pub metric: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServicePrometheus {
    pub name: String,
    pub node: Vec<Node>
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceMetric {
    pub service: Service,
    pub utilization: Resource,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct NodeMongo {
    pub timestamp: DateTime,
    pub metadata: Node,
    pub resource: Resource,
    pub environment: Vec<EnvironmentMetric>,
    pub services: Vec<ServiceMetric>,
}

// convert NodeMongo to BSON Document
impl Into<Document> for NodeMongo {
    fn into(self) -> Document {
        doc! {
            "timestamp": self.timestamp,
            "metadata": bson::to_bson(&self.metadata).unwrap(),
            "resource": bson::to_bson(&self.resource).unwrap(),
            "environment": bson::to_bson(&self.environment).unwrap(),
            "services": bson::to_bson(&self.services).unwrap(),
        }
    }
}

// function to determine if a node String (name) is part of a config
pub fn is_node_in_config(node: &str, config: &Config) -> bool {
    for n in &config.cluster.nodes {
        if n.name == node {
            return true;
        }
    }
    false
}


// function to determine of service is part of a config
pub fn is_service_in_config(service: &str, config: &Config) -> bool {
    for s in &config.services {
        if s.name == service {
            return true;
        }
    }
    false
}

// Function to extract resource metrics from a BSON document
pub fn extract_resource_metrics(kind: &str, document: &mongodb::bson::Document) -> Result<Resource, &'static str> {
    if let Some(resource_doc) = document.get_document(kind).ok(){
        let cpu = extract_f64(&resource_doc, "cpu");
        let memory = extract_f64(&resource_doc, "memory");
        let disk = extract_f64(&resource_doc, "disk");
        let network = extract_f64(&resource_doc, "network");

        return Ok(Resource {
            cpu,
            memory,
            disk,
            network,
        });
    }

    else {
        Err("Utilization key not found in document")
    }
}

// Function to extract the first resource utilization metric of a service
pub fn extract_service_metrics(service: &str, document: &Document) -> Result<Resource, &'static str> {
    if let Ok(services) = document.get_array("services") {
        // Print the length of the services array
        // println!("services array length: {}", services.len());

        for service_bson in services.iter() {
            if let Some(service_doc) = service_bson.as_document() {
                if let Some(service_obj) = service_doc.get_document("service").ok() {
                    if let Ok(name) = service_obj.get_str("name") {
                        if name == service {
                            //println!("Service found with name: {} - extract utilization metrics", name);
                            match extract_resource_metrics("utilization", service_doc){
                                Ok(metrics) => return Ok(metrics),
                                Err(_) => return Err("Failed to extract resource metrics in util"),
                            }
                        }
                    }
                }
            }
        }
    }
    Err("Failed to find or parse resource metrics")
}

// Helper function to extract environment metrics from a BSON document
pub fn extract_environment_metrics(document: &Document) -> Vec<EnvironmentMetric> {
    let mut environment_metrics = Vec::new();

    if let Ok(environment) = document.get_array("environment") {
        for env_bson in environment.iter() {
            if let Some(env_doc) = env_bson.as_document() {
                if let Ok(node_doc) = env_doc.get_document("node") {
                    if let Ok(node) = bson::from_bson(bson::Bson::Document(node_doc.clone())) {
                        let network = env_doc.get_document("network").map(|network_doc| {
                            // let available = network_doc.get_bool("available").unwrap_or(false);
                            // convert available to f64: true = 1.0, false = 0.0
                            // let available = if available { 1.0 } else { 0.0 };
                            let available = extract_f64(network_doc, "available");
                            let bandwidth = extract_f64(network_doc, "bandwidth");
                            let latency = extract_f64(network_doc, "latency");
                            let packet_loss = extract_f64(network_doc, "packet_loss");

                            Network {
                                available,
                                bandwidth,
                                latency,
                                packet_loss,
                            }
                        }).unwrap_or(Network {
                            available: 0.0,
                            bandwidth: 0.0,
                            latency: 0.0,
                            packet_loss: 0.0,
                        });

                        environment_metrics.push(EnvironmentMetric { node, network });
                    }
                }
            }
        }
    }

    environment_metrics
}


// Helper function to extract f64 field from a BSON document
pub fn extract_f64(document: &mongodb::bson::Document, field: &str) -> f64 {
    match document.get(field) {
        Some(bson::Bson::Double(value)) => *value,
        Some(bson::Bson::Int32(value)) => *value as f64,
        Some(bson::Bson::Int64(value)) => *value as f64,
        _ => 0.0, // Default value if field doesn't exist or cannot be converted
    }
}


// Helper function to retrieve the latest document from a collection
pub async fn get_latest_document(collection: &mongodb::Collection<Document>) -> Result<Document, HttpResponse> {
    let query = doc! {};
    let options = FindOneOptions::builder()
        .sort(doc! { "timestamp": -1 })
        .build();

    match collection.find_one(query, options).await {
        Ok(Some(document)) => Ok(document),
        Ok(None) => {
            println!("No documents found in the collection.");
            Err(HttpResponse::NotFound().body("No documents found in the collection."))
        }
        Err(e) => {
            println!("Failed to retrieve the latest document: {}", e);
            Err(HttpResponse::InternalServerError().body("Failed to retrieve the latest document."))
        }
    }
}

// Helper function to retrieve the latest 20 documents from a collection
pub async fn get_latest_documents(collection: &mongodb::Collection<Document>) -> Result<Vec<Document>, HttpResponse> {
    let query = doc! {};
    let options = FindOptions::builder()
        .sort(doc! { "timestamp": -1 })
        .limit(20)
        .build();

    match collection.find(query, options).await {
        Ok(mut cursor) => {
            let mut documents = Vec::new();
            while let Some(result) = cursor.next().await {
                match result {
                    Ok(document) => documents.push(document),
                    Err(e) => {
                        println!("Failed to retrieve a document: {}", e);
                        return Err(HttpResponse::InternalServerError().body("Failed to retrieve a document."));
                    }
                }
            }
            Ok(documents)
        }
        Err(e) => {
            println!("Failed to retrieve documents: {}", e);
            Err(HttpResponse::InternalServerError().body("Failed to retrieve documents."))
        }
    }
}

// Helper function to determine if a service is in a node's services. ToDo - should be part of node/service state
pub async fn is_service_in_node(service: &str, collection: &mongodb::Collection<Document>) -> bool {
    // Get the latest document
    let document = match get_latest_document(collection).await {
        Ok(doc) => doc,
        Err(_) => {
            //println!("Failed to retrieve the latest document.");
            return false;
        }
    };

    // Extract the services array from the document
    let services = match document.get_array("services") {
        Ok(services) => services,
        Err(_) => {
            //println!("Failed to extract the services array from the document.");
            return false;
        }
    };

    // Iterate through the services array
    for service_bson in services.iter() {
        if let Some(service_doc) = service_bson.as_document() {
            if let Some(service_obj) = service_doc.get_document("service").ok() {
                if let Ok(name) = service_obj.get_str("name") {
                    //println!("service {} being checked against {}", service, name);
                    if name == service {
                        return true;
                    }
                }
            }
        }
    }

    false
}


// a function that takes ResourceInt and Resource and returns the difference as a Resource
pub fn resource_diff(resource_int: ResourceInt, resource: Resource) -> Resource {
    // Resource {
    //     cpu: resource_int.cpu as f64 - resource.cpu,
    //     memory: (resource_int.memory *1000) as f64 - resource.memory,
    //     disk: (resource_int.disk * 1000) as f64 - resource.disk,
    //     network: (resource_int.network * 1000) as f64 - resource.network,
    // }
    Resource {
        cpu: resource.cpu,
        memory: resource.memory,
        disk: resource.disk,
        network: resource_int.network as f64 - resource.network,
    }
}

// a function that adds two Resource objects and returns the sum as a Resource
pub fn resource_sum(resource1: Resource, resource2: Resource) -> Resource {
    Resource {
        cpu: resource1.cpu + resource2.cpu,
        memory: resource1.memory + resource2.memory,
        disk: resource1.disk + resource2.disk,
        network: resource1.network + resource2.network,
    }
}

// a function that takes two resource objects and subs
pub fn resource_sub(resource1: Resource, resource2: Resource) -> Resource {
    Resource {
        cpu: resource1.cpu - resource2.cpu,
        memory: resource1.memory - resource2.memory,
        disk: resource1.disk - resource2.disk,
        network: resource1.network - resource2.network,
    }
}

// a function that subtracts resource from resource_int and returns the difference as a Resource
pub fn resource_int_sub(resource_int: ResourceInt, resource: Resource) -> Resource {
    Resource {
        cpu: resource_int.cpu as f64 - resource.cpu,
        memory: resource_int.memory as f64 - resource.memory,
        disk: resource_int.disk as f64 - resource.disk,
        network: resource_int.network as f64 - resource.network,
    }
}

// a function that adds/subtracts two resource objects
pub fn resource_sum_sub(resource1: Resource, resource2: Resource) -> Resource {
    Resource {
        cpu: resource1.cpu + resource2.cpu,
        memory: resource1.memory + resource2.memory,
        disk: resource1.disk + resource2.disk,
        network: resource1.network - resource2.network,
    }
}

// a function that takes a ResourceInt and adds/sums some properties
pub fn resource_int_subx(resource_int: ResourceInt, resource: Resource) -> Resource {
    Resource {
        cpu: resource_int.cpu as f64 - resource.cpu,
        memory: resource_int.memory as f64 - resource.memory,
        disk: (resource_int.disk as f64)/1000.0 - resource.disk,
        network: resource.network,
    }
}