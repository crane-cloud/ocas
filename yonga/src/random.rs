use std::collections::{HashMap, HashSet};
use crate::utility::{Config, Node, Service};
use crate::stack::{self, StackConfig};
use chrono::Local;
use std::process::Command;
use rand::seq::SliceRandom;
use rand::thread_rng;
use rand::Rng;

#[derive(Debug)]
pub struct Random {
    pub config: Config, //YAML
    pub placement: Option<HashMap<Service, Option<HashSet<Node>>>>,
    pub stack_name: String,
    pub stack_config: StackConfig, //Docker Swarm
}

impl Random {
    pub fn new(config: Config, placement: Option<HashMap<Service, Option<HashSet<Node>>>>, stack_name: String, stack_config: StackConfig) -> Self {
        Random {
            config,
            placement,
            stack_name,
            stack_config,
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

    pub async fn random_0(&mut self) -> Result<HashMap<Service, Option<HashSet<Node>>>, Box<dyn std::error::Error>> {
        println!("Running the Random Strategy 0");

        self.placement = Some(HashMap::new());

        // create the placement
        let assignment_map = self.assign_services();

        self.placement = Some(assignment_map);

        let placement_map = self.placement.clone().unwrap();

        //print the placement_map
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


    fn assign_services(&mut self) -> HashMap<Service, Option<HashSet<Node>>> {
        let mut assignment_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();
        let mut rng = thread_rng();
    
        // Shuffle nodes
        let mut nodes = self.config.cluster.nodes.clone();
        nodes.shuffle(&mut rng);
        let services = self.config.services.clone();
    
        for service in services {
            // Randomly select the number of nodes to assign to this service
            let num_nodes_to_assign = rng.gen_range(1..2);
            
            // Select a random set of nodes for this service
            let selected_nodes: HashSet<Node> = nodes
                .choose_multiple(&mut rng, num_nodes_to_assign)
                .cloned()
                .collect();
            
            assignment_map.insert(service.clone(), Some(selected_nodes));
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
                let compose_file = format!("random_{}.yml", Local::now().format("%Y-%m-%d_%H-%M-%S"));

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

