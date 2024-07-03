use serde::{Deserialize, Serialize};
use bson::DateTime;
use actix_web::HttpResponse;
use mongodb::{bson::doc, bson::Document, options::{FindOptions, FindOneOptions}};

#[derive(Debug, Deserialize, Clone)]
pub struct Cluster {
    pub nodes: Vec<Node>,
    pub prometheus: Prometheus,
}

#[derive(Debug, Deserialize, Clone)]
struct DatabaseCollection {
    name: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Database {
    pub uri: String,
    pub db: String,
    pub collections: Vec<DatabaseCollection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Service {
    pub id: String,
    pub name: String,
    pub cache: String,
    pub db: String
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub cluster: Cluster,
    pub database: Database,
    pub services: Vec<Service>,
}


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Node {
    pub id: String,
    pub name: String,
    pub ip: String,
}

// Network metrics
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Network {
    pub available: bool,
    pub bandwidth: f64,
    pub latency: f64,
    pub packet_loss: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Resource {
    pub cpu: f64,
    pub memory: f64,
    pub disk: f64,
    pub network: f64,
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
struct ServicePrometheus {
    name: String,
    node: Vec<Node>
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceMetric {
    service: Service,
    utilization: Resource,
}


#[derive(Debug, Serialize, Deserialize)]
struct NodeMongo {
    timestamp: DateTime,
    metadata: Node,
    resource: Resource,
    environment: Vec<EnvironmentMetric>,
    services: Vec<ServiceMetric>,
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
                            let available = network_doc.get_bool("available").unwrap_or(false);
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
                            available: false,
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
        .sort(doc! { "_id": -1 })
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