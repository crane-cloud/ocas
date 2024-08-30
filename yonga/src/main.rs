use clap::{Command, ArgAction, Arg};
use yonga::spread::Spread;
use yonga::binpack::Binpack;
use yonga::random::Random;
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
        .arg(Arg::new("stack")
            .long("stack")
            .short('s')
            .required(false)
            .action(ArgAction::Set))
        .get_matches();

    let yaml_config = matches.get_one::<String>("compose").unwrap();
    let strategy = matches.get_one::<String>("placement").unwrap(); // this can either be spread, binpack or random or yonga
    let cluster_config = matches.get_one::<String>("config").unwrap();
    let url = matches.get_one::<String>("url").unwrap(); // the base URL for the API client
    let stack_name = matches.get_one::<String>("stack").unwrap(); // the name of the stack  

    // Read the config file
    let yaml_str = fs::read_to_string(yaml_config).expect("Failed to read the YAML configuration file");
    let cluster_str = fs::read_to_string(cluster_config).expect("Failed to read the cluster configuration file");

    // Parse the YAML file
    let stack_config: StackConfig = serde_yaml::from_str(&yaml_str)?;

    // populate the volumes
    //let stack_config_vol: StackConfig = stack_config.populate_volumes();

    let cluster_config: Config = serde_yaml::from_str(&cluster_str)?;

    // determine the strategy
    match strategy.as_str() {
        "spread" => {
            println!("Spread strategy selected");

            //let api_client = ApiClient::new(url);
            let mut spread = Spread::new(cluster_config.clone(), None, stack_name.to_string(), stack_config.clone());

           // get the placement
            let placement = spread.spread_0().await;

            match placement {
                Ok(map) => {
                    spread.run(Some(map));
                }
                Err(_) => {
                    println!("No placement solution found");
                }
            }

        }
        "binpack" => {
            println!("Binpack strategy selected");

            let api_client = ApiClient::new(url);
            let mut binpack = Binpack::new(cluster_config.clone(), None, stack_name.to_string(), stack_config.clone(), api_client);

            let placement = binpack.binpack_0().await;

            match placement {
                Ok(map) => {
                    binpack.run(Some(map));
                }
                Err(_) => {
                    println!("No placement solution found");
                }
            }


        }
        "random" => {
            println!("Random strategy selected");

            let mut random = Random::new(cluster_config.clone(), None, stack_name.to_string(), stack_config.clone());

            let placement = random.random_0().await;

            match placement {
                Ok(map) => {
                    random.run(Some(map));
                }
                Err(_) => {
                    println!("No placement solution found");
                }
            }
        }
        "yonga" => {
            // print the nodes and attributes
            println!("Yonga strategy selected");

            // create the solver & API client
            let api_client = ApiClient::new(url);
            let solver = Solver::new(cluster_config.clone(), api_client);

            let mut placement = Yonga::new(cluster_config, stack_name.to_string(), stack_config, solver);
            placement.start().await;
        }
        _ => {
            println!("No strategy selected - exiting");

            return Ok(());
        }
    }

    Ok(())
}