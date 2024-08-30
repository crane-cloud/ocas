use chrono::Local;
use mongodb::Client;
use crate::stack::{self, StackConfig};
use crate::solver::Solver;
use crate::trace::{TraceEntry, ServiceGraph, get_trace_entries, build_trees};
use crate::node::{NodeTree, NodeGraph, NodeEntryMongo};
use crate::utility::Config;
use std::process::Command;

#[derive(Debug)]
pub struct Yonga {
    pub config: Config,
    pub stack_name: String,
    pub stack_config: StackConfig,
    pub running: bool,
    pub revision: u32,
    pub solver: Solver,
    // pub service_graph: Option<ServiceGraph>,
    // pub node_graph: Option<NodeGraph>,
}

impl Yonga {
    pub fn new(config: Config, stack_name: String, stack_config: StackConfig, solver: Solver) -> Self {
        Yonga {
            config,
            stack_name,
            stack_config,
            running: false,
            revision: 0,
            solver,
            // service_graph: None,
            // node_graph: None,
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

    pub async fn start(&mut self) {
        println!("Starting Yonga placement strategy");

        loop {
            if self.running {
                self.placement_1().await;
            } else {
                self.placement_0().await;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    }

    pub async fn placement_0(&mut self) -> Option<StackConfig> {
        println!("Running the Yonga placement strategy - placement 0");

        let placement_map = self.solver.solve_0().await;

        match placement_map {
            Ok(map) => {
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
                let compose_file = format!("yonga_{}.yml", Local::now().format("%Y-%m-%d_%H-%M-%S"));

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

                    // Update the run and revision
                    self.running = true;
                    self.revision += 1;

                    // Clean up the file
                    std::fs::remove_file(&compose_file).expect("Failed to remove the YAML configuration file");
                }

                None
            },
            Err(_) => {
                println!("No solution found");
                None
            }
        }
    }

    pub async fn placement_1(&mut self) -> Result<(), Box<dyn std::error::Error>> { 
        println!("Evaluating the current state of the placements - revision {}", self.revision);

        let client = Client::with_uri_str(&self.config.database.uri).await?;
        let database = client.database(&self.config.database.db);
        let collection_trace = database.collection::<TraceEntry>("trace");

        let nodes = self.config.cluster.nodes.clone();

        //create collections from nodes
        let collection_nodes: Vec<_> = nodes.iter().map(|node| {
            database.collection::<NodeEntryMongo>(&node.name)
        }).collect();

        //Retrieve trace entries with a limit
        let limit = 1000000000; // This can be any variable number

        let trace_entries = get_trace_entries(&collection_trace, limit).await.unwrap();

        let trees = build_trees(trace_entries);

        let mut service_tree = ServiceGraph::new();
        service_tree.build_from_traces(&trees);

        // Print the entire service graph
        // service_graph.print_graph();

        // Create a NodeGraph instance
        let mut node_graph = NodeGraph::new(nodes.clone());

        // Build the graph with the collections and limit
        node_graph.build(collection_nodes,limit).await?;

        // Create a NodeTree instance
        let mut node_tree = NodeTree::new(self.config.clone());

        // Aggregate the edges
        node_tree.aggregate_edges(&node_graph);


        let placement_map = self.solver.solve_1(service_tree, node_tree).await;

        match placement_map {
            Ok(map) => {

                // print the placement map
                // println!("Placement Map: {:?}", map);

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
                let compose_file = format!("yonga_{}.yml", Local::now().format("%Y-%m-%d_%H-%M-%S"));

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

                    // Update the run and revision
                    self.revision += 1;

                    // Clean up the file
                    std::fs::remove_file(&compose_file).expect("Failed to remove the YAML configuration file");
                }
            }
            Err(_) => {
                println!("No solution found");
            }
        }

        Ok(())

    }
}
