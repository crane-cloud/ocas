// use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
// use mongodb::{bson::doc, options::FindOptions};
// use mongodb::{options::ClientOptions, Client as MongoClient, Collection};
// // use redis::AsyncCommands;
// use std::sync::Arc;
// use clap::{Arg, Command, ArgAction};
// use std::fs;

// use utility::{Config, Service};

// #[derive(Debug)]
// struct AppState {
//     mongo_client: MongoClient,
// }

// #[get("/data")]
// async fn get_data(state: web::Data<Arc<AppState>>) -> impl Responder {
//     let collection = state.mongo_client.database("test").collection("data");
//     let find_options = FindOptions::default();

//     // Example query
//     let query = doc! { "name": "example" };
//     let result = collection.find_one(query, Some(find_options)).await;

//     match result {
//         Ok(Some(doc)) => {
//             let response = format!("Data: {:?}", doc);
//             HttpResponse::Ok().body(response)
//         }
//         _ => HttpResponse::InternalServerError().finish(),
//     }
// }

// #[actix_web::main]
// async fn main() -> std::io::Result<()> {

//     let matches = Command::new("YongaAPI")
//     .arg(Arg::new("config")
//         .long("config")
//         .short('c')
//         .required(true)
//         .action(ArgAction::Set))
//     // .arg(Arg::new("cfg")
//     //     .short('c')
//     //     .action(ArgAction::Set))
//     .get_matches();

//     let config = matches.get_one::<String>("config").unwrap(); 

//     // parse the config file
//     let config_str = fs::read_to_string(config).expect("Failed to read configuration file");
//     let config: Config = serde_yaml::from_str(&config_str).expect("Failed to parse configuration file");

//     // set up MongoDB client
//     let mongo_client_options = ClientOptions::parse(&config.database.uri).await.unwrap();
//     let mongo_client = MongoClient::with_options(mongo_client_options).unwrap();
//     let database = mongo_client.database(&config.database.db);

//     // Create state shared across handlers
//     let app_state = Arc::new(AppState {
//         mongo_client,
//     });

//     // Start HTTP server
//     HttpServer::new(move || {
//         App::new()
//             .data(app_state.clone())
//             .service(get_data)
//     })
//     .bind("127.0.0.1:8888")?
//     .run()
//     .await
// }

fn main(){
    
}