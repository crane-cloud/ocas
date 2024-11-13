use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use mongodb::{options::ClientOptions, Client as MongoClient, Database};
use prometheus_http_query::response;
use std::sync::Arc;
use clap::{Arg, Command, ArgAction};
use std::fs;

use yonga::utility::*;

#[derive(Debug)]
struct AppState {
    database: Database,
    config: Config,
}

#[derive(Debug)]
struct Environment {
    environment: Vec<EnvironmentMetric>,
}

impl Environment {
    fn new(environment: Vec<EnvironmentMetric>) -> Self {
        Self {
            environment,
        }
    }

    // function get Network by node name
    fn get_network_by_node_name(&self, node: Node) -> Option<Network> {
        for env in &self.environment {
            if env.node == node {
                return Some(env.network.clone());
            }
        }
        None
    }
}

// get - welcome message at /
#[get("/")]
async fn welcome() -> impl Responder {
    println!("Welcome to the Yonga API - Analyzer");
    HttpResponse::Ok().body("Welcome to the Yonga API - Analyzer \n")
}

// get - number of services in a node
#[get("/node/{node}/services/count")]
async fn get_service_count(state: web::Data<Arc<AppState>>, node: web::Path<String>) -> impl Responder {
    let node = node.into_inner();
    //println!("retrieving the number of services in node: {}", node);

    // check that the node is part of the config
    if !yonga::utility::is_node_in_config(&node, &state.config) {
        println!("Node not found in the configuration \n");
        return HttpResponse::NotFound().body("Node not found in the configuration \n");
    }

    let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&node);

    // get the latest document
    match yonga::utility::get_latest_document(&collection).await {
        Ok(latest_document) => {
            // Extract the number of services from the latest document
            let service_count = match latest_document.get_array("services") {
                Ok(array) => array.len(),
                Err(_) => 0,
            };
    
            println!("Success: service count: {}", service_count);
            let response = format!("{}", service_count);
            HttpResponse::Ok().body(response)
        }
        Err(response) => response,
    }
}

// get - resource utilization in a node
#[get("/node/{node}/utilization")]
async fn get_node_utilization(state: web::Data<Arc<AppState>>, node: web::Path<String>) -> impl Responder {
    let node = node.into_inner();

    // check that the node is part of the config
    if !yonga::utility::is_node_in_config(&node, &state.config) {
        println!("Node not found in the configuration\n");
        return HttpResponse::NotFound().body("Node not found in the configuration. \n");
    }

    //println!("retrieving resource utilization for node: {}", node);
    let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&node);

    // get the latest documents (10)
    match yonga::utility::get_latest_documents(&collection).await {
        Ok(latest_documents) => {
            let mut total_cpu = 0.0;
            let mut total_memory = 0.0;
            let mut total_disk = 0.0;
            let mut total_network = 0.0;

            // get the length
            let len = latest_documents.len() as f64;

            for document in latest_documents {
                // Extract resource metrics from the document
                let metrics = yonga::utility::extract_resource_metrics("resource", &document);

                total_cpu += metrics.clone().unwrap().cpu;
                total_memory += metrics.clone().unwrap().memory;
                total_disk += metrics.clone().unwrap().disk;
                total_network += metrics.clone().unwrap().network;
            }

            // get the average resource utilization metrics
            let average_metrics = Resource {
                cpu: total_cpu / len,
                memory: total_memory / len,
                disk: total_disk / len,
                network: total_network / len,
            };

            HttpResponse::Ok().json(average_metrics)
        }
        Err(response) => response,
    }

    // get the latest document
    // match yonga::utility::get_latest_document(&collection).await {
    //     Ok(latest_document) => {
    //         // Extract resource metrics from the latest document
    //         match yonga::utility::extract_resource_metrics("resource", &latest_document) {
    //             Ok(metrics) => HttpResponse::Ok().json(metrics),
    //             Err(_) => HttpResponse::InternalServerError().body("Failed to extract node resource metrics. \n"),
    //         }
    //     }
    //     Err(response) => response,
    // }

}

// get - service resource utilization on a node
#[get("/node/{node}/service/{service}/utilization")]
async fn get_node_service_utilization(state: web::Data<Arc<AppState>>, path: web::Path<(String, String)>) -> impl Responder {

    let (node, service) = path.into_inner();

    // if !yonga::utility::is_node_in_config(&node, &state.config) || !yonga::utility::is_service_in_config(&service, &state.config) {
    //     println!("Node or service not found in the configuration \n");
    //     return HttpResponse::NotFound().body("Node or service not found in the configuration \n");
    // }

    println!("retrieving resource utilization for service: {} on node: {}", service, node);

    let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&node);

    // get the latest document
    match get_latest_document(&collection).await {
        Ok(latest_document) => {
            // print 
            let service = format!("{}", service);
            match extract_service_metrics(&service, &latest_document) {
                Ok(metrics) => HttpResponse::Ok().json(metrics),
                Err(_) => HttpResponse::InternalServerError().body("Failed to extract service resource metrics.\n"),
            }
        }
        Err(response) => response,
    }
}


// get - total utilization of a service in the cluster
#[get("/service/{service}/utilization")]
async fn get_service_utilization(state: web::Data<Arc<AppState>>, service: web::Path<String>) -> impl Responder {
    let service = service.into_inner();

    if !yonga::utility::is_service_in_config(&service, &state.config) {
        println!("Service not found in the configuration \n");
        return HttpResponse::NotFound().body("Service not found in the configuration \n");
    }

    //println!("retrieving cluster resource utilization metrics for service {}", service);

    // iterate through all nodes in the config
    let mut total_cpu = 0.0;
    let mut total_memory = 0.0;
    let mut total_disk = 0.0;
    let mut total_network = 0.0;

    let mut node_count = 0;

    // initialize replica count
    // let mut replica_count = 0;
    let service = format!("{}{}", state.config.cluster.prometheus.stack, service);

    for node in &state.config.cluster.nodes {

        let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&node.name);

        // check if the service is in a node
        if !yonga::utility::is_service_in_node(&service, &collection).await {
            //println!("Service not found in node: {} \n", node.name);
            continue;
        }

        let mut node_cpu = 0.0;
        let mut node_memory = 0.0;
        let mut node_disk = 0.0;
        let mut node_network = 0.0;

        // increment the node count if the service is found
        node_count += 1;

        //println!("[found] retrieving resource utilization for service: {} on node: {}", service, node.name);

    //     // get the latest document
    //     match get_latest_document(&collection).await {
    //         Ok(latest_document) => {
    //             //let service = format!("{}{}", state.config.cluster.prometheus.stack, service);
    //             match extract_service_metrics(&service, &latest_document) {
    //                 Ok(metrics) => {
    //                     total_cpu += metrics.cpu;
    //                     total_memory += metrics.memory;
    //                     total_disk += metrics.disk;
    //                     total_network += metrics.network;
    //                 }
    //                 Err(_) => {
    //                     println!("Failed to extract service resource metrics \n");
    //                     return HttpResponse::InternalServerError().body("Failed to extract service resource metrics \n");
    //                 }
    //             } 
    //         }
    //         Err(response) => return response,
    //     }
    // }

    // // provide the total resource utilization metrics
    // let total_metrics = Resource {
    //     cpu: total_cpu,
    //     memory: total_memory,
    //     disk: total_disk,
    //     network: total_network,
    // };

    // HttpResponse::Ok().json(total_metrics)


        // get the latest documents (10)
        match yonga::utility::get_latest_documents(&collection).await {
            Ok(latest_documents) => {
                // let mut node_cpu = 0.0;
                // let mut node_memory = 0.0;
                // let mut node_disk = 0.0;
                // let mut node_network = 0.0;

                // get the length
                let len = latest_documents.len() as f64;

                for document in latest_documents {
                    // Extract resource metrics from the document
                    match yonga::utility::extract_service_metrics(&service, &document) {
                        Ok(metrics) => {
                            node_cpu += metrics.cpu;
                            node_memory += metrics.memory;
                            node_disk += metrics.disk;
                            node_network += metrics.network;
                        }
                        Err(_) => {
                            println!("Failed to extract service resource metrics \n");
                            return HttpResponse::InternalServerError().body("Failed to extract service resource metrics \n");
                        }
                    }
                }

                // get the average resource utilization metrics
                total_cpu = node_cpu / len;
                total_memory = node_memory / len;
                total_disk = node_disk / len;
                total_network = node_network / len;

                //HttpResponse::Ok().json(average_metrics)
            }
            Err(_) => {
                println!("Failed to retrieve the latest documents \n");
                return HttpResponse::InternalServerError().body("Failed to retrieve the latest documents \n");
            }
        }
    }

    let total_metrics = Resource {
        cpu: total_cpu / node_count as f64,
        memory: total_memory / node_count as f64,
        disk: total_disk / node_count as f64,
        network: total_network / node_count as f64,
    };

    return HttpResponse::Ok().json(total_metrics);
}

// get the environment metrics for a node
#[get("/node/{node}/environment")]
async fn get_node_environment(state: web::Data<Arc<AppState>>, node: web::Path<String>) -> impl Responder {
    let node = node.into_inner();

    if !yonga::utility::is_node_in_config(&node, &state.config) {
        println!("Node not found in the configuration");
        return HttpResponse::NotFound().body("Node not found in the configuration\n");
    }

    println!("Retrieving the environment metrics for node: {}", node);

    // Initialize accumulators
    let mut total_available = 0.0;
    let mut total_bandwidth = 0.0;
    let mut total_packet_loss = 0.0;
    let mut total_latency = 0.0;

    let mut node_count = 0.0;

    // get node in state by name
    let real_node = state.config.cluster.nodes.iter().find(|n| n.name == node).unwrap();

    for n in &state.config.cluster.nodes {
        if n.name == node {
            continue;
        }

        let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&n.name);



        match yonga::utility::get_latest_documents(&collection).await {
            Ok(latest_documents) => {
                let len = latest_documents.len() as f64;

                if len == 0.0 {
                    continue;
                }

                let mut node_total_available = 0.0;
                let mut node_total_bandwidth = 0.0;
                let mut node_total_packet_loss = 0.0;
                let mut node_total_latency = 0.0;

                for document in latest_documents {
                    let metrics = yonga::utility::extract_environment_metrics(&document);
                    let environment = Environment::new(metrics);

                    if let Some(network) = environment.get_network_by_node_name(real_node.clone()) {
                        node_total_available += network.available;
                        node_total_bandwidth += network.bandwidth;
                        node_total_packet_loss += network.packet_loss;
                        node_total_latency += network.latency;
                    }
                }

                total_available += node_total_available / len;
                total_bandwidth += node_total_bandwidth / len;
                total_packet_loss += node_total_packet_loss / len;
                total_latency += node_total_latency / len;
                node_count += 1.0;
            }
            Err(_) => {
                println!("Failed to retrieve the latest documents for node: {}", n.name);
                return HttpResponse::InternalServerError().body("Failed to retrieve the latest documents\n");
            }
        }
    }

    if node_count == 0.0 {
        return HttpResponse::NotFound().body("No metrics found for other nodes\n");
    }

    let average_network = Network {
        available: total_available / node_count,
        bandwidth: total_bandwidth / node_count,
        packet_loss: total_packet_loss / node_count,
        latency: total_latency / node_count,
    };

    println!("Average Network for node: {} - {:?}", node, average_network);

    HttpResponse::Ok().json(average_network)
}

// get all collections in the database
#[get("/collections")]
async fn get_collections(state: web::Data<Arc<AppState>>) -> impl Responder {
    //println!("Retrieving the database collections");
    let collections = state.database.list_collection_names(None).await;

    match collections {
        Ok(collections) => {
            println!("Success: collections: {:?}", collections);
            let response = format!("Collections: {:?}", collections);
            HttpResponse::Ok().body(response)
        }
        _ => {
            println!("Failed to retrieve collections");
            HttpResponse::InternalServerError().body("Failed to retrieve collections")
        }
    }
}

// get number of documents in a collection
#[get("/collection/{collection}/count")]
async fn get_document_count(state: web::Data<Arc<AppState>>, collection: web::Path<String>) -> impl Responder {
    let collection_name = collection.into_inner();
    println!("Retrieving document count in collection: {}", collection_name);
    let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&collection_name);
    let count = match collection.count_documents(None, None).await {
        Ok(count) => count,
        Err(_) => {
            println!("Failed to retrieve document count");
            return HttpResponse::InternalServerError().body("Failed to retrieve document count");
        }
    };

    println!("Success: document count: {}", count);
    let response = format!("Document count: {}", count);
    HttpResponse::Ok().body(response)
}

// get services in a node
#[get("/node/{node}/services")]
async fn get_node_services(state: web::Data<Arc<AppState>>, node: web::Path<String>) -> impl Responder {
    let node = node.into_inner();
    println!("retrieving the services in node: {}", node);

    // check that the node is part of the config
    if !yonga::utility::is_node_in_config(&node, &state.config) {
        println!("Node not found in the configuration");
        return HttpResponse::NotFound().body("Node not found in the configuration");
    }

    let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&node);

    // get the latest document
    match yonga::utility::get_latest_document(&collection).await {
        Ok(latest_document) => {
            // Extract the services array from the latest document
            let services = match latest_document.get_array("services") {
                Ok(array) => array,
                Err(_) => {
                    println!("Failed to extract the services array from the document");
                    return HttpResponse::InternalServerError().body("Failed to extract the services array from the document");
                }
            };

            let mut service_names = Vec::new();
            for service_bson in services.iter() {
                if let Some(service_doc) = service_bson.as_document() {
                    if let Some(service_obj) = service_doc.get_document("service").ok() {
                        if let Ok(name) = service_obj.get_str("name") {
                            service_names.push(name.to_string());
                        }
                    }
                }
            }

            //println!("Success: services: {:?}", service_names);
            //let response = format!("Services: {:?}", service_names);
            let response = serde_json::to_string(&service_names).unwrap();
            // let response = service_names;
            HttpResponse::Ok().body(response)
        }
        Err(response) => response,
    }
}


// catch-all route for unknown routes
async fn not_found() -> impl Responder {
    println!("Route not found \n");
    HttpResponse::NotFound().body("Route not found \n")
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let matches = Command::new("YongaAPI")
        .arg(Arg::new("config")
            .long("config")
            .short('c')
            .required(true)
            .action(ArgAction::Set))
        .arg(Arg::new("port")
            .long("port")
            .short('p')
            .required(true)
            .action(ArgAction::Set))
        .get_matches();

    let config = matches.get_one::<String>("config").unwrap();
    let port = (matches.get_one::<String>("port").unwrap()).parse::<u16>().unwrap();

    // parse the config file
    let config_str = fs::read_to_string(config).expect("Failed to read configuration file");
    let config: Config = serde_yaml::from_str(&config_str).expect("Failed to parse configuration file");

    // set up MongoDB client
    let mongo_client_options = ClientOptions::parse(&config.database.uri).await.unwrap();
    let mongo_client = MongoClient::with_options(mongo_client_options).unwrap();
    let database = mongo_client.database(&config.database.db);

    // Create state shared across handlers
    let app_state = Arc::new(AppState {
        database,
        config,
    });

    // Start HTTP server
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .service(welcome)
            .service(get_collections)
            .service(get_document_count)
            .service(get_node_service_utilization)
            .service(get_node_utilization)
            .service(get_service_count)
            .service(get_service_utilization)
            .service(get_node_environment)
            .service(get_node_services)
            .default_service(web::route().to(not_found))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
