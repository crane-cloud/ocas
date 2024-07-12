use clap::{Command, ArgAction, Arg};
use std::fs;
use yonga::stack::StackConfig;
use yonga::yonga::Yonga;
use yonga::utility::Config;
use yonga::solver::Solver;
use yonga::api_client::ApiClient;   

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("OCAS")
        .arg(Arg::new("compose") //docker-compose file
            .long("compose")
            .short('m')
            .required(true)
            .action(ArgAction::Set))
        .arg(Arg::new("placement") //placement strategy
            .long("placement")
            .short('p')
            .required(true)
            .action(ArgAction::Set))
        .arg(Arg::new("config") // configuration file
            .long("config")
            .short('c')
            //.required(true)
            .action(ArgAction::Set))
        .arg(Arg::new("url")
            .long("url")
            .short('u')
            .required(false)
            .action(ArgAction::Set))
        .get_matches();

    let yaml_config = matches.get_one::<String>("compose").unwrap();
    let strategy = matches.get_one::<String>("placement").unwrap(); // this can either be spread, binpack or random or yonga
    let cluster_config = matches.get_one::<String>("config").unwrap();
    let url = matches.get_one::<String>("url").unwrap(); // the base URL for the API client

    // Read the config file
    let yaml_str = fs::read_to_string(yaml_config).expect("Failed to read the YAML configuration file");
    let cluster_str = fs::read_to_string(cluster_config).expect("Failed to read the cluster configuration file");

    // Parse the YAML file
    let stack_config: StackConfig = serde_yaml::from_str(&yaml_str)?;
    let cluster_config: Config = serde_yaml::from_str(&cluster_str)?;

    // determine the strategy
    match strategy.as_str() {
        "spread" => {
            println!("Spread strategy selected");
            // spread the services across the nodes
            //placement_spread(&mut config);

        }
        "binpack" => {
            println!("Binpack strategy selected");
            // binpack the services across the nodes
            //placement_binpack(&mut config);
        }
        "random" => {
            println!("Random strategy selected");
            // randomly assign services to nodes
            //placement_binpack(&mut config);
        }
        "yonga" => {
            // create the solver & API client
            let api_client = ApiClient::new(url);
            let solver = Solver::new(cluster_config.clone(), api_client);

            let mut placement = Yonga::new(stack_config, cluster_config, solver);
            placement.start().await;
        }
        _ => {
            println!("No strategy selected - exiting");

            return Ok(());
        }
    }

    Ok(())
}