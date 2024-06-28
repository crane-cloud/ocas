use tokio::time::{self, Duration};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use mongodb::{options::ClientOptions, Client as MongoClient, Collection};
use mongodb::bson::{doc, Document};
use std::mem;
use std::{fs, error::Error};
use clap::{Arg, Command, ArgAction};
use regex::Regex;
use bson::DateTime;
use chrono::Utc;
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Node {
    id: String,
    name: String,
    ip: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Resource {
    cpu: f64,
    memory: f64,
    disk: f64,
    network: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Environment {
    node: Node,
    bandwidth: f64,
    latency: f64,
    packet_loss: f64,
}

#[derive(Debug, Deserialize, Clone)]
struct Prometheus {
    url: String,
    label: String,
    stack: String,
    query: String,
    metric: String,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ServicePrometheus {
    name: String,
    node: Vec<Node>
}

#[derive(Debug, Deserialize, Clone)]
struct Cluster {
    nodes: Vec<Node>,
    prometheus: Prometheus,
}

#[derive(Debug, Deserialize, Clone)]
struct Config {
    cluster: Cluster,
    database: Database,
    services: Vec<Service>,
}


//Node exporter metrics
#[derive(Debug, Serialize, Deserialize)]
struct NodeMetric {
    timestamp: DateTime,
    metadata: Node,
    resource: Resource,
    environment: Vec<Environment>,
    services: Vec<ServiceMetric>,
}

// Convert NodeMetric to BSON Document
impl Into<Document> for NodeMetric {
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

#[derive(Debug, Serialize, Deserialize)]
struct ServiceMetric {
    service: Service,
    utilization: Resource,
}

// function to return Node given a socket address
fn get_node_by_address(nodes: &[Node], address: &str) -> Option<Node> {
    let ip_address = address.split(':').next().unwrap_or_default(); // Extracts only the IP part
    nodes.iter().find(|node| node.ip == ip_address).cloned()
}

// function that takes PromqlResult and returns ip address strings
fn get_ip_addresses(promql_result: &prometheus_http_query::response::PromqlResult) -> Vec<String> {
    promql_result.data().as_vector().unwrap().iter().map(|vector| {
        vector.metric().get("instance").unwrap().as_str().to_string()
    }).collect()
}

// retrieve a list of services from prometheus
async fn get_services_prometheus(config: &Config, client: &Client, prometheus: &Prometheus) -> Result<Vec<ServicePrometheus>, Box<dyn Error>> {
    let url_service = format!("{}/api/v1/label/{}/values", prometheus.url, prometheus.label);

    let response = client.get(&url_service).send().await?;

    // initialize a vector to hold the services
    let mut services_prometheus: Vec<ServicePrometheus> = Vec::new();

    if response.status().is_success() {
        let body = response.json::<Value>().await?;

        let prometheus_client = prometheus_http_query::Client::try_from(prometheus.url.as_str()).unwrap();


        if let Some(services) = body["data"].as_array() {
            // get name of the services and create a vector with names
            let services: Vec<String> = services.iter().filter_map(|service| {
                service.as_str().map(|name| name.to_string())
            }).collect();

            for service in services {
                
                // strip the stack from the service name
                let service_trimmed = service.trim_start_matches(prometheus.stack.as_str());

                // check if the service is in the config
                if !config.services.iter().any(|s| s.name == service_trimmed) {
                    continue;
                }

                let service_formatted = format!(r#""{}""#, service.as_str());
                let q_a = prometheus.query.replace("\"_\"", &service_formatted);
                let q = q_a.replace("metric", prometheus.metric.as_str());

                let result = prometheus_client.query(q).get().await?;

                if result.data().as_vector().is_some() {
                    let ip_addresses = get_ip_addresses(&result);

                    //println!("Address for service {:?}: {:?}", service, ip_addresses);

                    let nodes = ip_addresses.iter().filter_map(|address| {
                        get_node_by_address(&config.cluster.nodes, address)
                    }).collect();

                    services_prometheus.push(ServicePrometheus {
                        name: service,
                        node: nodes,
                    });
                }
            }
            // print the prometheus services
            // println!("{:?}", services_prometheus);
            return Ok(services_prometheus);
        }
    }

    Ok(Vec::new())
}


// get service metrics from prometheus for the prometheus services
async fn get_service_metrics_prometheus(config: &Config, service: &ServicePrometheus) -> Result<Option<ServiceMetric>, Box<dyn Error>> {
    let prometheus_client = match prometheus_http_query::Client::try_from(config.cluster.prometheus.url.as_str()) {
        Ok(client) => client,
        Err(_) => {
            //println!("Failed to create prometheus client");
            return Ok(None)
        },
    };

    let service_formatted = format!(r#""{}""#, service.name);

    let queries = [
        ("cpu", "sum(rate(container_cpu_usage_seconds_total{label=\"{}\"}[5m])) by (instance)"),
        ("memory", "sum(container_memory_working_set_bytes{name!~\"POD\", label=\"{}\"}) by (name)"),
        ("disk_r", "container_fs_reads_bytes_total{label=\"{}\"}"),
        ("disk_w", "container_fs_writes_bytes_total{label=\"{}\"}"),
        ("network_rx", "sum(rate(container_network_receive_bytes_total{label=\"{}\"}[10m])) by (name)"),
        ("network_tx", "sum(rate(container_network_transmit_bytes_total{label=\"{}\"}[10m])) by (name)"),
    ];

    let mut metrics = std::collections::HashMap::new();

    for (key, query_template) in &queries {
        let query = query_template.replace("label", &config.cluster.prometheus.label).replace("\"{}\"", &service_formatted);
        println!("Query for service {}: {}", service.name, query);
        match prometheus_client.query(query).get().await {
            Ok(result) => metrics.insert(*key, result),
            Err(_) => {
                //println!("Failed to get metrics for service {}", service.name);
                return Ok(None)
            },
        };
    }

    let cpu_utilization: f64 = metrics.get("cpu").and_then(|data| data.data().as_vector())
        .map_or(0.0, |vector| vector.iter().map(|vector| vector.sample().value()).sum());

    let memory_utilization: f64 = metrics.get("memory").and_then(|data| data.data().as_vector())
        .map_or(0.0, |vector| vector.iter().map(|vector| vector.sample().value()).sum::<f64>() / 1_048_576.0); // MB

    let disk: f64 = metrics.get("disk_r").and_then(|disk_r| metrics.get("disk_w").and_then(|disk_w| {
        disk_r.data().as_vector().zip(disk_w.data().as_vector()).map(|(r_vec, w_vec)| {
            r_vec.iter().zip(w_vec.iter()).map(|(r, w)| r.sample().value() + w.sample().value()).sum::<f64>() / 1_073_741_824.0 // GB
        })
    })).unwrap_or(0.0);

    let network: f64 = metrics.get("network_rx").and_then(|network_rx| metrics.get("network_tx").and_then(|network_tx| {
        network_rx.data().as_vector().zip(network_tx.data().as_vector()).map(|(rx_vec, tx_vec)| {
            rx_vec.iter().zip(tx_vec.iter()).map(|(rx, tx)| rx.sample().value() + tx.sample().value()).sum::<f64>() / 1_048_576.0 // MB
        })
    })).unwrap_or(0.0);

    let service_metric = ServiceMetric {
        service: Service {
            id: service.name.clone(),
            name: service.name.clone(),
            cache: "".to_string(),
            db: "".to_string(),
        },
        utilization: Resource {
            cpu: cpu_utilization,
            memory: memory_utilization,
            disk: disk,
            network: network,
        },
    };

    Ok(Some(service_metric))
}


//Retrieve metrics from Node Exporter URL
async fn get_node_exporter_metrics(client: &Client, node: Node) -> Result<Option<NodeMetric>, Box<dyn Error>> {
    let url = format!("http://{}:9100/metrics", node.ip);

    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        let body = response.text().await?;
        let mut cpu_times = std::collections::HashMap::new();
        let mut memory_b = 0.0;
        let mut disk_b = 0.0;
        let mut network_b = 0.0;

        let re_cpu = Regex::new(r#"^node_cpu_seconds_total\{cpu="(\d+)",mode="(idle|system|user|nice|iowait|irq|softirq|steal)"\} (.*)"#)?;
        let re_memory_b = Regex::new(r#"^node_memory_MemAvailable_bytes (.*)"#)?;
        let re_disk_b = Regex::new(r#"^node_filesystem_free_bytes\{.*mountpoint="/"\} (.*)"#)?;
        let re_network_rx = Regex::new(r#"^node_network_receive_bytes_total\{.*\} (.*)"#)?;
        let re_network_tx = Regex::new(r#"^node_network_transmit_bytes_total\{.*\} (.*)"#)?;


        for line in body.lines() {
            if let Some(caps) = re_cpu.captures(line) {
                let cpu: u32 = caps[1].parse()?;
                let mode = &caps[2];
                let value: f64 = caps[3].parse()?;

                let entry = cpu_times.entry(cpu).or_insert((0.0, 0.0));
                entry.0 += value;
                if mode == "idle" {
                    entry.1 += value;
                }
            } else if let Some(caps) = re_memory_b.captures(line) {
                memory_b = caps[1].parse::<f64>()?;
            } else if let Some(caps) = re_disk_b.captures(line) {
                disk_b = caps[1].parse::<f64>()?;
            } else if re_network_rx.is_match(line) || re_network_tx.is_match(line) {
                let bytes: f64 = line.split_whitespace().last().unwrap_or("0").parse()?;
                network_b += bytes;
            }
        }

        let mut cpu = 0.0;
        for (_, (total, idle)) in cpu_times {
            if total > 0.0 {
                let utilization = 1.0 - (idle / total);
                cpu += 1.0 - utilization;
            }
        }

        let memory = memory_b / 1_048_576.0; // Convert bytes to MiB
        let disk = disk_b / 1_073_741_824.0; // Convert bytes to GiB
        let network = network_b / 1_048_576.0; // Convert bytes to MiB

        // Create a BSON UTC datetime
        let timestamp = DateTime::from_chrono(Utc::now());

        return Ok(Some(NodeMetric {
            timestamp,
            metadata: node,
            resource: Resource { cpu, memory, disk, network },
            environment: Vec::new(),
            services: Vec::new(),
        }));
    }

    Ok(None)
}


// Function to push Document metrics to MongoDB
async fn push_metrics_to_mongo(collection: &Collection<Document>, metrics: Document) {
    match collection.insert_one(metrics, None).await {
        Ok(_) => println!("Successfully inserted document into MongoDB"),
        Err(e) => eprintln!("Failed to insert document into MongoDB: {}", e),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {

    let matches = Command::new("YongaMonitor")
    .arg(Arg::new("config")
        .long("config")
        .short('c')
        .required(true)
        .action(ArgAction::Set))
    // .arg(Arg::new("cfg")
    //     .short('c')
    //     .action(ArgAction::Set))
    .get_matches();

    let config = matches.get_one::<String>("config").unwrap();    
    
    // parse the config file
    let config_str = fs::read_to_string(config).expect("Failed to read configuration file");
    let config: Config = serde_yaml::from_str(&config_str).expect("Failed to parse configuration file");

    // print the services in prometheus
    let client = Client::new();

    // get the services from prometheus
    let services = get_services_prometheus(&config, &client, &config.cluster.prometheus).await.unwrap();

    for service in services {
        // get metrics for the service
        let service_metrics = get_service_metrics_prometheus(&config, &service).await.unwrap();

        // print the metrics
        println!("{:?}", service_metrics);
    }


    // // set up MongoDB client
    // let mongo_client_options = ClientOptions::parse(&config.database.uri).await.unwrap();
    // let mongo_client = MongoClient::with_options(mongo_client_options).unwrap();
    // let database = mongo_client.database(&config.database.db);
    
    // //Use node names as collection names
    // let collections: Vec<Collection<Document>> = config.cluster.nodes.iter()
    // .map(|node| database.collection::<Document>(&node.name))
    // .collect();

    // //Create a vector of collection names from node names
    // let col_names: Vec<String> = config.cluster.nodes.iter()
    // .map(|node| node.name.clone())
    // .collect();

    // //iterate over each collection and match with names in the vector
    // for collection in collections {
    //     let coll_name = collection.name().to_string();
    //     match col_names.iter().find(|&x| x == &coll_name) {
    //         Some(_) => {
    //             let client = Client::new();

    //             // get the node in this collection
    //             let node = config.cluster.nodes.iter().find(|node| node.name == coll_name).unwrap().clone();

    //             // spawn a background task to fetch metrics periodically and push to MongoDB
    //             tokio::spawn(async move {
    //                 let mut interval = time::interval(Duration::from_secs(60));
    //                 loop {
    //                     interval.tick().await;
    //                         if let Some(metrics) = get_node_exporter_metrics(&client, node.clone()).await.unwrap() {
    //                             push_metrics_to_mongo(&collection, metrics.into()).await;
    //                         }
    //                 }
    //             });
    //         }
    //         None => {
    //             println!("Collection '{}' does not match any node name", coll_name);
    //         }
    //     }
    // }

    // // Handle Ctrl+C signal
    // tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
    // println!("Received Ctrl+C, shutting down.");

    Ok(())
}