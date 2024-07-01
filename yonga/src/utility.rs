use serde::{Deserialize, Serialize};
use mongodb::bson::{doc, Document};
use bson::DateTime;

#[derive(Debug, Deserialize, Clone)]
struct Cluster {
    nodes: Vec<Node>,
    prometheus: Prometheus,
}

#[derive(Debug, Deserialize, Clone)]
struct DatabaseCollection {
    name: String,
}

#[derive(Debug, Deserialize, Clone)]
struct Database {
    uri: String,
    db: String,
    collections: Vec<DatabaseCollection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Service {
    id: String,
    name: String,
    cache: String,
    db: String
}

#[derive(Debug, Deserialize, Clone)]
struct Config {
    cluster: Cluster,
    database: Database,
    services: Vec<Service>,
}


#[derive(Debug, Deserialize, Serialize, Clone)]
struct Node {
    id: String,
    name: String,
    ip: String,
}

// Network metrics
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Network {
    available: bool,
    bandwidth: f64,
    latency: f64,
    packet_loss: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Resource {
    cpu: f64,
    memory: f64,
    disk: f64,
    network: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct EnvironmentMetric {
    node: Node,
    network: Network,
}

#[derive(Debug, Deserialize, Clone)]
struct Prometheus {
    url: String,
    label: String,
    stack: String,
    query: String,
    metric: String,
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
