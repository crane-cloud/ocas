use std::collections::{HashMap, HashSet};
use crate::utility::{Config, Node, Service};
use crate::stack::{self, StackConfig};
use chrono::Local;
use rand::seq::SliceRandom;
use rand::thread_rng;

//use crate::api_client::ApiClient;
use std::process::Command;
// use crate::utility::{Network, Resource};

#[derive(Debug)]
pub struct Spread {
    pub config: Config, //YAML
    pub placement: Option<HashMap<Service, Option<HashSet<Node>>>>,
    pub stack_name: String,
    pub stack_config: StackConfig, //Docker Swarm
    //pub api_client: ApiClient,
}

impl Spread {
    pub fn new(config: Config, placement: Option<HashMap<Service, Option<HashSet<Node>>>>, stack_name: String, stack_config: StackConfig) -> Self {
        Spread {
            config,
            placement,
            stack_name,
            stack_config,
            //api_client,
        }
    }

    pub fn run_deploy(&self, stack_name: &str, compose_file: &str) -> Result<std::process::Output, String> {
        let output = Command::new("./target/debug/deploy")
            .arg("--stack")
            .arg("deploy")
            .arg("--name")
            .arg(stack_name)
            .arg("--file")
            .arg(compose_file)
            .output()
            .map_err(|e| e.to_string())?;
        
        if !output.status.success() {
            return Err(format!("Command failed with status: {}", output.status));
        }

        Ok(output)
    }

    pub async fn spread_0(&mut self) -> Result<HashMap<Service, Option<HashSet<Node>>>, Box<dyn std::error::Error>> {
        println!("Running the Spread Strategy 0");

        self.placement = Some(HashMap::new());

        //let placement_map = self.spread_services().await?;

        // get the total number of services
        let j = self.config.services.len() as f32;

        // get the total number of nodes
        let i = self.config.cluster.nodes.len() as f32;

        // determine the number of services to assign to each node   
        let mut proportion_map: HashMap<Node, u32> = HashMap::new();

        // determine the number of services to assign to each node
        for node in &self.config.cluster.nodes {
            let node_proportion = (j / i).ceil() as u32;
            proportion_map.insert(node.clone(), node_proportion);
        }

        // print the proportion map
        // for (node, proportion) in &proportion_map {
        //     println!("Node: {:?}, Proportion: {:?}", node, proportion);
        // }

        // create the placement
        let assignment_map = self.assign_services(&proportion_map, 1, self.config.services.clone());

        self.placement = Some(assignment_map);

        let placement_map = self.placement.clone().unwrap();

        // print the placement_map
        // for (service, nodes) in &placement_map {
        //     println!("Service: {:?}", service);
        //     match nodes {
        //         Some(node_set) => {
        //             for node in node_set {
        //                 println!("Node: {:?}", node);
        //             }
        //         },
        //         None => {
        //             println!("No nodes assigned");
        //         }
        //     }
        // }

        Ok(placement_map)
    }

    fn assign_services(&self, proportion_map: &HashMap<Node, u32>, _groups: u32, services: Vec<Service>) -> HashMap<Service, Option<HashSet<Node>>> {
        let mut rng = thread_rng();
        let mut shuffled_services = services.clone();
        shuffled_services.shuffle(&mut rng);
    
        let mut assignment_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();
        let mut node_capacity: HashMap<Node, u32> = proportion_map.clone();
    
        let mut nodes: Vec<&Node> = proportion_map.keys().collect();
    
        // Iterate over all services
        for service in shuffled_services {
            let mut assigned = false;
    
            nodes.shuffle(&mut rng); // Shuffle nodes before each assignment
    
            for node in &nodes {
                if let Some(capacity) = node_capacity.get_mut(node) {
                    if *capacity > 0 {
                        assignment_map.entry(service.clone())
                                      .or_insert_with(|| Some(HashSet::new()))
                                      .as_mut()
                                      .unwrap()
                                      .insert((*node).clone());
                        *capacity -= 1;
                        assigned = true;
                        break;
                    }
                }
            }
    
            if !assigned {
                assignment_map.insert(service.clone(), None);
            }
        }
    
        assignment_map
    
    }

    pub fn run(&mut self, placement_map: Option<HashMap<Service, Option<HashSet<Node>>>>) {
        match placement_map {
            Some(map) => {
                // Update and clean up the stack config
                let mut local_stack_config = self.stack_config.clone();
                stack::update_node_constraints(&mut local_stack_config, map);
                stack::populate_volumes(&mut local_stack_config);
                stack::delete_null_placement(&mut local_stack_config);

                // Update the stack config
                self.stack_config = local_stack_config;

                // Create the YAML
                let yaml_str = serde_yaml::to_string(&self.stack_config).unwrap();

                // Provide the YAML to the deploy binary
                let compose_file = format!("spread_{}.yml", Local::now().format("%Y-%m-%d_%H-%M-%S"));

                // Write the YAML to a file
                std::fs::write(&compose_file, yaml_str).expect("Failed to write the YAML configuration file");

                // Deploy the stack
                if let Err(e) = self.run_deploy(&self.stack_name, &compose_file) {
                    println!("Failed to deploy: {}", e);

                    // Clean up the file
                    std::fs::remove_file(&compose_file).expect("Failed to remove the YAML configuration file");
                }

                else {
                    println!("Deployed the stack successfully!");

                    // Clean up the file
                    std::fs::remove_file(&compose_file).expect("Failed to remove the YAML configuration file");
                }
            },
            None => {
                println!("No placement solution provided");
            }
        }
    }
}

