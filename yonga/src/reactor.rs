use yonga::api_client::ApiClient;
use tokio::time::{sleep, Duration};
use clap::{Arg, ArgAction, Command};
use std::fs;
use yonga::utility::Config;


struct YongaReactor {
    config: Config,
    api_client: ApiClient,
    react: (bool, Option<Vec<f64>>),
    // node_threshold: f64,
    // service_threshold: f64,
    // environment_threshold: f64,
}

impl YongaReactor {
    pub fn new(config: Config, api_client: ApiClient) -> Self {
        Self {
            config,
            api_client,
            react: (false, None),
            // node_threshold: 0.8,
            // service_threshold: 0.8,
            // environment_threshold: 0.8,
        }
    }

    // define types of reaction - with data it should transmit:

    pub fn react(&mut self) -> (bool, Option<Vec<f64>>) {
        // node utilization | should be below a threshold
        // service utilization | should be below a threshold
        // environment | should not change beyond [get the initial state]

        // for node in self.config.cluster.nodes.iter() {
        //     let node_utilization = self.api_client.get_node_utilization(node);
        //     if node_utilization > self.config.node_threshold {
        //         self.react = (true, Some(vec![node_utilization]));
        //         return (true, Some(vec![node_utilization]));
        //     }
        // }


        (false, None)
    }

    pub async fn run(&mut self) {
        loop {
            let (react, data) = self.react();
            if react {
                // send a notification
                println!("Reacting to the situation with data {:?}", data);
            }
            sleep(Duration::from_secs(30)).await;
        }
    }
}


#[tokio::main]
async fn main() {

    let matches = Command::new("YongaReactor")
    .arg(Arg::new("config")
        .long("config")
        .short('c')
        .required(true)
        .action(ArgAction::Set))
    .arg(Arg::new("api")
        .long("api")
        .short('a')
        .required(true)
        .action(ArgAction::Set))
    .get_matches();

    let config = matches.get_one::<String>("config").unwrap();
    let api = matches.get_one::<String>("api").unwrap();   
    
    // parse the config file
    let config_str = fs::read_to_string(config).expect("Failed to read configuration file");
    let config: Config = serde_yaml::from_str(&config_str).expect("Failed to parse configuration file");


    let api_client = ApiClient::new(api);

    let mut reactor = YongaReactor::new(config, api_client);

    reactor.run().await;
}