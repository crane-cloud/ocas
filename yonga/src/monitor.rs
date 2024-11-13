use tokio::time::{self, Duration};
use reqwest::Client;
//use serde::{Deserialize, Serialize};
use mongodb::{options::ClientOptions, Client as MongoClient, Collection};
use mongodb::bson::{doc, Document};
use std::{fs, error::Error};
use clap::{Arg, Command, ArgAction};
use regex::Regex;
use bson::DateTime;
use chrono::Utc;
use serde_json::Value;
use yonga::utility::{Config, Node, Resource, EnvironmentMetric, ServiceMetric, Service, Prometheus, ServicePrometheus, NodeMongo, Network};


// #[derive(Debug, Deserialize, Clone)]
// struct Cluster {
//     nodes: Vec<Node>,
//     prometheus: Prometheus,
// }

// #[derive(Debug, Deserialize, Clone)]
// struct DatabaseCollection {
//     name: String,
// }

// #[derive(Debug, Deserialize, Clone)]
// struct Database {
//     uri: String,
//     db: String,
//     collections: Vec<DatabaseCollection>,
// }

// #[derive(Debug, Serialize, Deserialize, Clone)]
// struct Service {
//     id: String,
//     name: String,
//     cache: String,
//     db: String
// }

// #[derive(Debug, Deserialize, Clone)]
// struct Config {
//     cluster: Cluster,
//     database: Database,
//     services: Vec<Service>,
// }


// #[derive(Debug, Deserialize, Serialize, Clone)]
// struct Node {
//     id: String,
//     name: String,
//     ip: String,
// }

// // Network metrics
// #[derive(Debug, Serialize, Deserialize, Clone)]
// struct Network {
//     available: bool,
//     bandwidth: f64,
//     latency: f64,
//     packet_loss: f64,
// }

// #[derive(Debug, Deserialize, Serialize, Clone)]
// struct Resource {
//     cpu: f64,
//     memory: f64,
//     disk: f64,
//     network: f64,
// }

// #[derive(Debug, Deserialize, Serialize, Clone)]
// struct EnvironmentMetric {
//     node: Node,
//     network: Network,
// }

// #[derive(Debug, Deserialize, Clone)]
// struct Prometheus {
//     url: String,
//     label: String,
//     stack: String,
//     query: String,
//     metric: String,
// }

// #[derive(Debug, Serialize, Deserialize, Clone)]
// struct ServicePrometheus {
//     name: String,
//     node: Vec<Node>
// }

// #[derive(Debug, Serialize, Deserialize)]
// struct ServiceMetric {
//     service: Service,
//     utilization: Resource,
// }


// #[derive(Debug, Serialize, Deserialize)]
// struct NodeMongo {
//     timestamp: DateTime,
//     metadata: Node,
//     resource: Resource,
//     environment: Vec<EnvironmentMetric>,
//     services: Vec<ServiceMetric>,
// }

// // convert NodeMongo to BSON Document
// impl Into<Document> for NodeMongo {
//     fn into(self) -> Document {
//         doc! {
//             "timestamp": self.timestamp,
//             "metadata": bson::to_bson(&self.metadata).unwrap(),
//             "resource": bson::to_bson(&self.resource).unwrap(),
//             "environment": bson::to_bson(&self.environment).unwrap(),
//             "services": bson::to_bson(&self.services).unwrap(),
//         }
//     }
// }



// function to return Node given a socket address
fn get_node_by_address(nodes: &[Node], address: &str) -> Option<Node> {
    // Split the address by ':' and take the first part (IP address)
    let ip_address = address.split(':').next().unwrap_or_default();
    
    // Find the node with the matching IP address and return a cloned version of it
    nodes.iter().find(|node| node.ip == ip_address).cloned()
}

// function that takes PromqlResult and returns ip address strings
fn get_ip_addresses(promql_result: &prometheus_http_query::response::PromqlResult) -> Vec<String> {
    // Ensure the result is a vector and iterate over it
    if let Some(vector_data) = promql_result.data().as_vector() {
        vector_data.iter().filter_map(|vector| {
            // Extract the "instance" metric, which typically contains the IP address
            vector.metric().get("instance").map(|value| value.as_str().to_string())
        }).collect()
    } else {
        // Return an empty vector if the result data is not a vector
        vec![]
    }
}

// retrieve a list of services from prometheus
async fn get_services_prometheus(config: &Config, client: &Client, prometheus: &Prometheus) -> Result<Vec<ServicePrometheus>, Box<dyn Error>> {
    // print the parameters
    println!("Prometheus URL: {}", prometheus.url);

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
                
                // strip the stack from the service name - to check if part of the config
                let service_trimmed = service.trim_start_matches(prometheus.stack.as_str());

                // check if the service is in the config
                if !config.services.iter().any(|s| s.name == service_trimmed) {
                    continue;
                }

                let service_formatted = format!(r#""{}""#, service.as_str()); //hotelreservation_frontend
                let q_a = prometheus.query.replace("\"_\"", &service_formatted); //"sum(rate(metric{container_label_com_docker_swarm_service_name=\"_\"}[2m])) by (instance)"
                let q = q_a.replace("metric", prometheus.metric.as_str());

                let result = prometheus_client.query(q).get().await?;

                //println!("Query for service {}: {:?}", service, result.data().as_vector());

                if result.data().as_vector().is_some() {
                    let ip_addresses = get_ip_addresses(&result);

                    println!("Address(es) for service {:?}: {:?}", service, ip_addresses);

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
            println!("{:?}", services_prometheus);
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
        ("cpu", "sum(rate(container_cpu_usage_seconds_total{label=\"{}\"}[1m])) by (instance)"),
        ("memory", "sum(container_memory_working_set_bytes{name!~\"POD\", label=\"{}\"}) by (name)"),
        ("disk_r", "container_fs_reads_bytes_total{label=\"{}\"}"),
        ("disk_w", "container_fs_writes_bytes_total{label=\"{}\"}"),
        ("network_rx", "sum(rate(container_network_receive_bytes_total{label=\"{}\"}[1m])) by (name)"),
        ("network_tx", "sum(rate(container_network_transmit_bytes_total{label=\"{}\"}[1m])) by (name)"),
    ];

    let mut metrics = std::collections::HashMap::new();

    for (key, query_template) in &queries {
        let query = query_template.replace("label", &config.cluster.prometheus.label).replace("\"{}\"", &service_formatted);
        //println!("Query for service {}: {}", service.name, query);
        match prometheus_client.query(query).get().await {
            Ok(result) => metrics.insert(*key, result),
            Err(_) => {
                println!("Failed to get metrics for service {}", service.name);
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
            cache: Some("".to_string()),
            db: Some("".to_string()),
            // cache: "".to_string(),
            // db: "".to_string(),
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

// get node resource metric from node_exporter
async fn get_node_resource(client: &Client, node: Node) -> Result<Option<Resource>, Box<dyn Error>> {
    let url = format!("http://{}:9100/metrics", node.ip);

    // print the action
    println!("Retrieving the resource metrics for node: {} on the URL: {}", node.name, url);
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

        // print the node resource
        println!("Node {}: CPU: {:.2} Cores, Memory: {:.2} MiB, Disk: {:.2} GiB, Network: {:.2} MiB", node.name, cpu, memory, disk, network);


        return Ok(Some(Resource {
            cpu: cpu,
            memory: memory,
            disk: disk,
            network: network,
        }));
    }

    Ok(None)
}

// get node environment metric from node_exporter
async fn get_node_environment_metric(client: &Client, node: Node, config: &Config) -> Result<Option<Vec<EnvironmentMetric>>, Box<dyn Error>> {
    let url = format!("http://{}:9100/metrics", node.ip);
    let response = client.get(&url).send().await?;

    if response.status().is_success() {
        let body = response.text().await?;

        let mut environment_metrics: Vec<EnvironmentMetric> = Vec::new();

        for node_e in &config.cluster.nodes {
            if node_e.name == node.name {
                continue;
            }

            let mut network = Network {
                available: 0.0,
                bandwidth: 0.0,
                latency: 1000.0,
                packet_loss: 100.0,
            };

            // Create regex patterns dynamically
            let re_availability = Regex::new(&format!(r#"availability\{{ip="{}",metric="availability",timestamp="[^"]+"\}} ([01])"#, node_e.ip))?;
            let re_bandwidth = Regex::new(&format!(r#"bandwidth\{{ip="{}",metric="bandwidth",timestamp="[^"]+"\}} ([0-9\.]+)"#, node_e.ip))?;
            let re_packet_loss = Regex::new(&format!(r#"packet_loss\{{ip="{}",metric="packet_loss",timestamp="[^"]+"\}} ([0-9\.]+)"#, node_e.ip))?;
            let re_latency = Regex::new(&format!(r#"latency\{{ip="{}",metric="latency",timestamp="[^"]+"\}} ([0-9\.]+)"#, node_e.ip))?;

            for line in body.lines() {
                if let Some(caps) = re_availability.captures(line) {
                    //network.available = &caps[1] == "1";
                    network.available = caps[1].parse::<f64>()?;
                } else if let Some(caps) = re_bandwidth.captures(line) {
                    network.bandwidth = caps[1].parse::<f64>()?;
                } else if let Some(caps) = re_packet_loss.captures(line) {
                    network.packet_loss = caps[1].parse::<f64>()?;
                } else if let Some(caps) = re_latency.captures(line) {
                    network.latency = caps[1].parse::<f64>()?;
                }
            }

            environment_metrics.push(EnvironmentMetric {
                node: node_e.clone(),
                network: network,
            });
        }

        return Ok(Some(environment_metrics));
    }
    Ok(None)
}

// function to determine if ServicePrometheus is in a node
fn is_service_in_node(service: &ServicePrometheus, node: &Node) -> bool {
    service.node.iter().any(|n| n.name == node.name)
}

async fn get_node_services_metrics(config: &Config, node: Node, services: Vec<ServicePrometheus>) -> Result<Vec<ServiceMetric>, Box<dyn Error>> {
    let mut service_metrics: Vec<ServiceMetric> = Vec::new();

    for service in services {
        // check if service is in node - skip if not
        if !is_service_in_node(&service, &node) {
            continue;
        }

        // get service metrics
        if let Some(service_metric) = get_service_metrics_prometheus(config, &service).await? {
            service_metrics.push(service_metric);
        }
    }

    Ok(service_metrics)
}

// get all the metrics
async fn get_node_mongo_metrics(client: &Client, node: Node, config: &Config, services: Vec<ServicePrometheus>) -> Result<Option<NodeMongo>, Box<dyn Error>> {

    let resource = get_node_resource(client, node.clone()).await?;
    let environment = get_node_environment_metric(client, node.clone(), &config).await?;
    let services = get_node_services_metrics(&config, node.clone(), services).await?;

    //ToDo: When to return Ok(None)

    if resource.is_none() || environment.is_none() {
        return Ok(None);
    }

    // Create a BSON UTC datetime
    let timestamp = DateTime::from_chrono(Utc::now());

    let node_mongo = NodeMongo {
        timestamp: timestamp,
        metadata: node.clone(),
        resource: resource.unwrap(),
        environment: environment.unwrap(),
        services: services,
    };

    Ok(Some(node_mongo))

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
    .get_matches();

    let config = matches.get_one::<String>("config").unwrap();    
    
    // parse the config file
    let config_str = fs::read_to_string(config).expect("Failed to read configuration file");
    let config: Config = serde_yaml::from_str(&config_str).expect("Failed to parse configuration file");

    // print the services in prometheus
    let client = Client::new();

    // get the services from prometheus
    let services = get_services_prometheus(&config, &client, &config.cluster.prometheus).await.unwrap();

    // print the number of services retrieved
    println!("Number of services retrieved: {}", services.len());


    // set up MongoDB client
    let mongo_client_options = ClientOptions::parse(&config.database.uri).await.unwrap();
    let mongo_client = MongoClient::with_options(mongo_client_options).unwrap();
    let database = mongo_client.database(&config.database.db);
    
    //Use node names as collection names
    let collections: Vec<Collection<Document>> = config.cluster.nodes.iter()
    .map(|node| database.collection::<Document>(&node.name))
    .collect();

    //Create a vector of collection names from node names
    let col_names: Vec<String> = config.cluster.nodes.iter()
    .map(|node| node.name.clone())
    .collect();

    //iterate over each collection and match with names in the vector
    for collection in collections {
        let coll_name = collection.name().to_string();
        match col_names.iter().find(|&x| x == &coll_name) {
            Some(_) => {

                // create config clone
                let config = config.clone();

                // create services clone
                let services = services.clone();

                // get the node in this collection
                let node = config.cluster.nodes.iter().find(|node| node.name == coll_name).unwrap().clone();

                print!("Node {} found in MongoDB - spawning a tokio thread \n", coll_name);

                // spawn a background task to fetch metrics periodically and push to MongoDB
                tokio::spawn(async move {
                    let mut interval = time::interval(Duration::from_secs(60));
                    let client = Client::new();
                    loop {
                        interval.tick().await;

                        if let Some(metrics) = get_node_mongo_metrics(&client, node.clone(), &config, services.clone()).await.unwrap() {
                            // println!("Node {}: pushing metrics {:?} to MongoDB", node.name, metrics);
                            push_metrics_to_mongo(&collection, metrics.into()).await;
                        }

                    }
                });
            }
            None => {
                println!("Collection '{}' does not match any node name", coll_name);
            }
        }
    }

    // // Handle Ctrl+C signal
    tokio::signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
    println!("Received Ctrl+C, shutting down.");

    Ok(())
}