use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use futures::stream::StreamExt;
use mongodb::{bson::doc, options::{ClientOptions, FindOptions, FindOneOptions}, Client as MongoClient, Database};
use std::sync::Arc;
use clap::{Arg, Command, ArgAction};
use std::fs;

use yonga::utility::{Config, Resource};

#[derive(Debug)]
struct AppState {
    database: Database,
    config: Config,
}

// get - welcome message at /
#[get("/")]
async fn welcome() -> impl Responder {
    println!("Welcome to the Yonga API - Analyzer");
    HttpResponse::Ok().body("Welcome to the Yonga API - Analyzer \n")
}

// get - resource utilization in a node
#[get("/node/{node}/utilization")]
async fn get_node_utilization(state: web::Data<Arc<AppState>>, node: web::Path<String>) -> impl Responder {
    let node = node.into_inner();
    println!("Retrieving resource utilization for node: {}", node);
    let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&node);

    // Define query to retrieve the latest document
    let query = doc! {};
    let options = FindOneOptions::builder()
        .sort(doc! { "_id": -1 })
        .build();

    // Retrieve the latest document
    let latest_document = match collection.find_one(query, options).await {
        Ok(Some(document)) => document,
        Ok(None) => {
            println!("No documents found in the collection.");
            return HttpResponse::NotFound().body("No documents found in the collection.");
        }
        Err(e) => {
            println!("Failed to retrieve the latest document: {}", e);
            return HttpResponse::InternalServerError().body("Failed to retrieve the latest document.");
        }
    };

        // Extract resource metrics from the latest document
    match extract_resource_metrics(&latest_document) {
        Ok(metrics) => HttpResponse::Ok().json(metrics),
        Err(_) => HttpResponse::InternalServerError().body("Failed to extract resource metrics."),
    }

}


// get - service resource utilization
#[get("/service/{service}/utilization")]
async fn get_service_utilization(state: web::Data<Arc<AppState>>, service: web::Path<String>) -> impl Responder {
    let service = service.into_inner();
    println!("Retrieving resource utilization for service: {}", service);
    let collection: mongodb::Collection<mongodb::bson::Document> = state.database.collection(&service);

    // Define query to retrieve the latest document
    let query = doc! {};
    let options = FindOneOptions::builder()
        .sort(doc! { "_id": -1 })
        .build();

    // Retrieve the latest document
    let latest_document = match collection.find_one(query, options).await {
        Ok(Some(document)) => document,
        Ok(None) => {
            println!("No documents found in the collection.");
            return HttpResponse::NotFound().body("No documents found in the collection.");
        }
        Err(e) => {
            println!("Failed to retrieve the latest document: {}", e);
            return HttpResponse::InternalServerError().body("Failed to retrieve the latest document.");
        }
    };

    // Extract resource metrics from the latest document
    match extract_resource_metrics(&latest_document) {
        Ok(metrics) => HttpResponse::Ok().json(metrics),
        Err(_) => HttpResponse::InternalServerError().body("Failed to extract resource metrics."),
    }
}


// get all collections in the database
#[get("/collections")]
async fn get_collections(state: web::Data<Arc<AppState>>) -> impl Responder {
    println!("Retrieving the database collections");
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



// // get list of documents in a collection
// #[get("/collection/{collection}")]
// async fn get_documents(state: web::Data<Arc<AppState>>, collection: web::Path<String>) -> impl Responder {
//     let collection_name = collection.into_inner();
//     println!("Retrieving documents in collection: {}", collection_name);
//     let collection: mongodb::Collection<_> = state.database.collection(&collection_name);
//     let find_options = FindOptions::builder().build();
//     let mut cursor: mongodb::Cursor<_> = match collection.find(None, find_options).await {
//         Ok(cursor) => cursor,
//         Err(_) => {
//             println!("Failed to retrieve documents");
//             return HttpResponse::InternalServerError().body("Failed to retrieve documents");
//         }
//     };

//     let mut documents: Vec<_> = Vec::new();
//     while let Some(result) = cursor.next().await {
//         match result {
//             Ok(doc) => documents.push(doc),
//             Err(e) => {
//                 println!("Error retrieving document: {:?}", e);
//                 return HttpResponse::InternalServerError().body("Error retrieving documents");
//             }
//         }
//     }

//     println!("Success: documents: {:?}", documents);
//     let response = format!("Documents: {:?}", documents);
//     HttpResponse::Ok().body(response)
// }

// catch-all route for unknown routes
async fn not_found() -> impl Responder {
    println!("Route not found \n");
    HttpResponse::NotFound().body("Route not found \n")
}


// Function to extract resource metrics from a BSON document
fn extract_resource_metrics(document: &mongodb::bson::Document) -> Result<Resource, bson::de::Error> {
    let resource = document.get_document("resource").unwrap();
    Ok(Resource {
        cpu: extract_f64(&resource, "cpu"),
        memory: extract_f64(&resource, "memory"),
        disk: extract_f64(&resource, "disk"),
        network: extract_f64(&resource, "network"),
    })
}

// Helper function to extract f64 field from a BSON document
fn extract_f64(document: &mongodb::bson::Document, field: &str) -> f64 {
    match document.get(field) {
        Some(bson::Bson::Double(value)) => *value,
        Some(bson::Bson::Int32(value)) => *value as f64,
        Some(bson::Bson::Int64(value)) => *value as f64,
        _ => 0.0, // Default value if field doesn't exist or cannot be converted
    }
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
            .service(get_node_utilization)
            .default_service(web::route().to(not_found))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
