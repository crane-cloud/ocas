use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use crate::utility::{Config, Node, Service};
use crate::stack::{self, StackConfig};
use chrono::Local;
use crate::api_client::ApiClient;
use std::process::Command;
// use crate::utility::Resource;

#[derive(Debug)]
pub struct Binpack {
    pub config: Config, //YAML
    pub placement: Option<HashMap<Service, Option<HashSet<Node>>>>,
    pub stack_name: String,
    pub stack_config: StackConfig, //Docker Swarm
    pub api_client: ApiClient,
}

impl Binpack {
    pub fn new(config: Config, placement: Option<HashMap<Service, Option<HashSet<Node>>>>, stack_name: String, stack_config: StackConfig, api_client: ApiClient) -> Self {
        Binpack {
            config,
            placement,
            stack_name,
            stack_config,
            api_client,
        }
    }

    pub async fn get_bins(&self) -> HashMap<Node, u32> {

        let mut bins: HashMap<Node, u32> = HashMap::new();

        for node in &self.config.cluster.nodes {
            // retrieve the node capacity in cpu, memory, network & storage
            let node_utilization = self.api_client.get_node_utilization(&node.name).await;

            match node_utilization {
                Ok(utilization) => {
                    let cpu = (utilization.cpu) * self.config.get_weight("cpu");
                    let memory = (utilization.memory) * self.config.get_weight("memory");
                    let network = utilization.network * self.config.get_weight("network");
                    let storage = utilization.disk * self.config.get_weight("disk");

                    // calculate the bin capacity
                    let bin_capacity = (cpu + memory + network + storage) as u32;

                    bins.insert(node.clone(), bin_capacity);
                },
                Err(e) => {
                    println!("Failed to retrieve the node utilization: {}", e);
                    bins.insert(node.clone(), 0);
                }
                
            }
        }

        bins
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

    pub async fn binpack_0(&mut self) -> Result<HashMap<Service, Option<HashSet<Node>>>, Box<dyn std::error::Error>> {
        println!("Running the Binpack Strategy 0");

        self.placement = Some(HashMap::new());

        // get bins for each service
        let mut bins = self.get_bins().await;

        // print the bins
        for (node, bin) in &bins {
            println!("Node: {:?}, Bins: {:?}", node, bin);
        }

        // create the placement
        let assignment_map = self.assign_services(&mut bins, self.config.services.clone());

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


    fn assign_services(
        &mut self,
        bins: &mut HashMap<Node, u32>,
        services: Vec<Service>
    ) -> HashMap<Service, Option<HashSet<Node>>> {
        let mut assignment_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();
        
        // Collect nodes and bins into a mutable vector and sort by bin capacity in ascending order
        let mut nodes: Vec<_> = bins.iter_mut().collect();
        nodes.sort_by(|a, b| a.1.cmp(&b.1)); // Ascending order by bin capacity
    
        for service in services {
            let mut assigned = false;
            for (node, bin) in nodes.iter_mut() {
                if **bin > 0 {
                    // Check if the bin has enough capacity for the service
                    if **bin >= 300 { // Ensure the bin has enough capacity | assumption that each service requires a capacity of 300
                        // Use `or_insert_with` to insert a new `Some(HashSet::new())` if not present
                        let node_set = assignment_map.entry(service.clone())
                            .or_insert_with(|| Some(HashSet::new()))
                            .as_mut()
                            .unwrap();
                        
                        node_set.insert(node.clone());
                        **bin -= 300; // Adjust decrement based on your use case
                        assigned = true;
                        break;
                    } else {
                        // Optionally handle cases where the bin is insufficient
                        println!("Node {} does not have enough capacity for service {}", node.id, service.id);
                    }
                }
            }
    
            if !assigned {
                println!("Service {} could not be assigned to any node", service.id);
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
                let compose_file = format!("binpack_{}.yml", Local::now().format("%Y-%m-%d_%H-%M-%S"));

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

