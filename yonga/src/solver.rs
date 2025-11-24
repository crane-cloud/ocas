use std::collections::{HashMap, HashSet};
use std::result::Result::Ok;
use crate::utility::{Config, Node, Service};
use crate::api_client::ApiClient;
use crate::utility::{Network, Resource, resource_diff, get_node_by_id, resource_sum, resource_sum_sub, resource_int_subx};
use crate::node::{NodeTree, AggLinkEdge};
use crate::trace::ServiceGraph;
// use crate::nsga2::{MicroservicePlacementProblem, get_best_individual};


use optirustic::algorithms::{
    Algorithm, MaxGenerationValue, NSGA2Arg, StoppingConditionType, NSGA2
};
use optirustic::core::{Constraint, ObjectiveDirection, RelationalOperator, VariableValue};
use optirustic::operators::{PolynomialMutationArgs, SimulatedBinaryCrossoverArgs};

use crate::nsga2opticas::{OMicroservicePlacementProblem, opticas_get_best_individual};


use opticas::algorithms::{
    OAlgorithm, OMaxGenerationValue, NSGA2OPTICASArg, OStoppingConditionType, NSGA2OPTICAS
};
use opticas::core::{OConstraint, OObjectiveDirection, ORelationalOperator, OVariableValue};
use opticas::operators::{OPolynomialMutationArgs, OSimulatedBinaryCrossoverArgs};

#[derive(Debug, Clone)]
pub struct Coordinate {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug)]
pub struct Solver {
    pub config: Config,
    pub placement: Option<HashMap<Service, Option<HashSet<Node>>>>,
    pub api_client: ApiClient,
    // need to add solver value - f64
    pub obj_value: Option<f64>,
    pub revision: u32,
}

impl Solver {
    pub fn new(config: Config, api_client: ApiClient) -> Self {
        Solver {
            config,
            placement: None,
            api_client,
            obj_value: None,
            revision: 0,
        }
    }

    pub async fn solve_0(&mut self) -> Result<HashMap<Service, Option<HashSet<Node>>>, Box<dyn std::error::Error>> {
        println!("Running the Solver for placement 0 - review");

        self.placement = Some(HashMap::new());

        // Create an empty map that has a Node as key and values NR, and NE as f64
        let mut node_map: HashMap<Node, Coordinate> = HashMap::new();

        // Create an empty map to hold Node as key, and Resource & Network as tuple values
        let mut resource_map: HashMap<Node, (Resource, Network)> = HashMap::new();

        for node in &self.config.cluster.nodes {

            // Retrieve the node utilization - resources available on the node, safe network
            let node_utilization = self.api_client.get_node_utilization(&node.name).await?;

            // Retrieve the node environment
            let node_environment = self.api_client.get_node_environment(&node.name).await?;

            // get services on the node
            let services = self.api_client.get_node_services(&node.name).await?;

            if services.len() > 0 {
                // print the services
                // println!("Services on node {}: {:?}", node.name, services);

                // initialize the Resource for the services
                let mut service_resources = Resource::default();

                for service in services {
                    let service_utilization = self.api_client.get_node_service_utilization(&node.name, &service).await?;
                    service_resources = resource_sum(service_resources, service_utilization);
                }

                // Add the service_resources to the node_utilization
                let node_utilization = resource_sum(node_utilization, service_resources);

                // Populate the resource map
                resource_map.insert(node.clone(), (node_utilization, node_environment));
            }
             else {
                println!("No services on node {}", node.name);

                // Populate the resource map
                resource_map.insert(node.clone(), (node_utilization, node_environment));
             }

            // // print the services
            // println!("Services on node {}: {:?}", node.name, services);

            // // initialize the Resource for the services
            // let mut service_resources = Resource::default();

            // for service in services {
            //     let service_utilization = self.api_client.get_node_service_utilization(&node.name, &service).await?;
            //     service_resources = resource_sum(service_resources, service_utilization);
            // }

            // // Add the service_resources to the node_utilization
            // let node_utilization = resource_sum(node_utilization, service_resources);

            // // Populate the resource map
            // resource_map.insert(node.clone(), (node_utilization, node_environment));
        }

        println!("Resource Map: {:?}", resource_map);

        
        // Use the resource map to populate the node_map
        for (node, (resource, network)) in &resource_map {
            node_map.insert(node.clone(), Coordinate{
                x: self.compute_nr(resource, &resource_map),
                y: self.compute_network_cost(network, &resource_map), //compute_ne(network, &resource_map),
            });
        }

        // print the node map
        println!("Node Map: {:?}", node_map);

        // Get all coordinates in the map
        let coordinates: Vec<Coordinate> = node_map.values().cloned().collect();

        let mut distance_map: HashMap<Node, f64> = HashMap::new();

        // Compute the Euclidean distance for each node
        for (node, coordinate) in &node_map {
            let distance = compute_euclidean_distance(&coordinates, coordinate.x, coordinate.y);
            distance_map.insert(node.clone(), distance);

            // Print the coordinate and distance
            println!("Node: {}, Coordinate: ({}, {}), Distance: {}", node.name, coordinate.x, coordinate.y, distance);
        }

        // Compute the proportion of services to assign to a node
        let j = self.config.services.len() as u32;

        // Create the proportion map
        let mut proportion_map: HashMap<Node, u32> = HashMap::new();

        // For each node, compute the proportion of services to assign
        for (node, _) in &node_map {
            let proportion = compute_proportion(*distance_map.get(node).unwrap(), &distance_map, j);
            println!("Proportion for node {} is {}", node.name, proportion);
            proportion_map.insert(node.clone(), proportion);
        }

        //let (num_groups, groups) = self.config.group_services();
        let service_groups = self.config.grouped_services();

        // print the service groups
        // println!("Service Groups: {:?}", service_groups);

        let assignment_map = self.assign_services(&proportion_map, service_groups);

        // for (service, node) in &assignment_map {
        //     println!("Service: {}, Assigned Node: {}", service.name, node.name);
        // }

        let mut placement_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();

        for (service, node) in assignment_map {
            placement_map.entry(service).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(node);
        }

        // print the placement map
        print_placement_map(placement_map.clone());

        // update the Solver placement map
        self.placement = Some(placement_map.clone());

        // update the revision
        self.revision += 1;

        Ok(placement_map)
    }


    pub async fn solve_1(
        &mut self,
        service_tree: ServiceGraph,
        node_tree: NodeTree,
    ) -> Result<HashMap<Service, Option<HashSet<Node>>>, Box<dyn std::error::Error>> {

        println!("Running the Solver for placement 1");

        self.placement = Some(HashMap::new());

        // get all services from the config
        let all_services = &self.config.services;

        // Find the longest service paths
        let (longest_paths, max_length) = service_tree.longest_paths();

        // Find the most popular services
        let (most_popular_services, _) = service_tree.most_popular_services();

        if longest_paths.len() as u32 == 0 || most_popular_services.len() == 0 || max_length == 0 {
            return Err("No microservices running or communicating".into());
        } else {
            println!("Longest Paths: {:?}", longest_paths);
            println!("Most Popular Services in Solve_1: {:?}", most_popular_services);
            println!("Max Length of the longest path/paths: {}", max_length);
        }

        // convert longest_paths to a Vec<Vec<Service>>
        let longest_paths: Vec<Vec<Service>> = longest_paths.iter().map(|path| {
            path.iter().map(|service_name| {
                get_services_by_names(service_name.clone(), &self.config.services).unwrap()
            }).collect()
        }).collect();

        // convert most_popular_services to a Vec<Service>
        let most_popular_services: Vec<Service> = most_popular_services.iter().map(|service_name| {
            get_services_by_names(service_name.clone(), &self.config.services).unwrap()
        }).collect();

        // Get services present in the tree (and translate names -> Service)
        let svc_tree = service_tree.get_services();
        let mut services_tree = Vec::new();
        for service_name in svc_tree {
            if let Some(svc) = get_services_by_names(service_name, &self.config.services) {
                services_tree.push(svc);
            }
        }

        // Compute best paths (note: NodeTree now works with node NAMES (String))
        let node_names = node_tree.get_nodes(); // Vec<String>
        let strong_paths = node_tree.compute_best_paths(node_names.clone(), max_length - 1);
        let (global_best_path_names, _global_best_cost) = node_tree.get_best_path(strong_paths.clone());

        println!(
            "Strongest overall path (names): {:?}",
            global_best_path_names
        );

        // Map the global_best_path names back to Vec<Node> using config
        let mut strongest_path_nodes: Vec<Node> = Vec::new();
        for name in &global_best_path_names {
            if let Some(n) = self.config.cluster.nodes.iter().find(|x| &x.name == name) {
                strongest_path_nodes.push(n.clone());
            } else {
                // If name not found in config, skip it (we only need the name mostly)
                println!("Warning: best-path node '{}' not found in config.cluster.nodes", name);
            }
        }

        if strongest_path_nodes.is_empty() {
            return Err("No valid nodes found for strongest path".into());
        }

        // Retrieve node utilization data
        let mut node_utilization_map: HashMap<Node, Resource> = HashMap::new();
        for node in &self.config.cluster.nodes {
            let node_utilization = self.api_client.get_node_utilization(&node.name).await?;
            node_utilization_map.insert(node.clone(), node_utilization);
        }

        // Retrieve service utilization data
        let mut service_utilization_map: HashMap<Service, Resource> = HashMap::new();
        for service in &self.config.services {
            let service_utilization = self.api_client.get_service_utilization(&service.name).await?;
            service_utilization_map.insert(service.clone(), service_utilization);
        }

        // create a placement map
        let mut placement_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();

        // use ResourceInt of each node and current utilization map to get remaining capacity
        let mut remaining_capacity: HashMap<Node, Resource> = HashMap::new();
        for (node, resource) in &node_utilization_map {
            // get the node resource int from config
            let resource_int = self
                .config
                .cluster
                .nodes
                .iter()
                .find(|n| n.name == node.name)
                .unwrap()
                .resource
                .clone();

            // get the remaining capacity
            let remaining = resource_diff(resource_int, resource.clone());
            remaining_capacity.insert(node.clone(), remaining);
        }

        // Place the most popular services on the root of the strongest path
        // root is the first node in `strongest_path_nodes`
        let root = &strongest_path_nodes[0];

        for service in &most_popular_services {
            if can_accommodate(root, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                println!("Placing service {} on root node {}", service.name, root.name);
                // place service and its dependencies on root
                let service_dep = get_dependencies(service, all_services);
                for dep in service_dep {
                    placement_map
                        .entry(dep.clone())
                        .or_insert_with(|| Some(HashSet::new()))
                        .as_mut()
                        .unwrap()
                        .insert(root.clone());
                }
            } else {
                // Fallback: try other nodes in the strongest path first, then overall nodes
                let mut placed = false;

                // try nodes from strongest_path_nodes (in order)
                for fallback_node in &strongest_path_nodes {
                    if can_accommodate(fallback_node, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                        println!("Placing service {} on fallback node {}", service.name, fallback_node.name);
                        placement_map
                            .entry(service.clone())
                            .or_insert_with(|| Some(HashSet::new()))
                            .as_mut()
                            .unwrap()
                            .insert(fallback_node.clone());
                        placed = true;
                        break;
                    }
                }

                // if still not placed, try all config nodes
                if !placed {
                    for cfg_node in &self.config.cluster.nodes {
                        if can_accommodate(cfg_node, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                            println!("Placing service {} on config fallback node {}", service.name, cfg_node.name);
                            placement_map
                                .entry(service.clone())
                                .or_insert_with(|| Some(HashSet::new()))
                                .as_mut()
                                .unwrap()
                                .insert(cfg_node.clone());
                            placed = true;
                            break;
                        }
                    }
                }
            }
        }

        // place the longest service chain/tree on the strongest path nodes (in order)
        for path in &longest_paths {
            for service in path {
                if !placement_map.contains_key(service) {
                    for node in &strongest_path_nodes {
                        if can_accommodate(node, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                            println!("Placing service [longest service chain] {} on node {}", service.name, node.name);
                            placement_map
                                .entry(service.clone())
                                .or_insert_with(|| Some(HashSet::new()))
                                .as_mut()
                                .unwrap()
                                .insert(node.clone());
                            break;
                        }
                    }
                }
            }
        }

        // Handle remaining services not placed yet
        for service in &self.config.services {
            if !placement_map.contains_key(service) {
                // try strongest path nodes first
                let mut placed = false;
                for node in &strongest_path_nodes {
                    if can_accommodate(node, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                        println!("Placing service [remaining] {} on node {}", service.name, node.name);
                        placement_map
                            .entry(service.clone())
                            .or_insert_with(|| Some(HashSet::new()))
                            .as_mut()
                            .unwrap()
                            .insert(node.clone());
                        placed = true;
                        break;
                    }
                }

                // otherwise try config nodes
                if !placed {
                    for cfg_node in &self.config.cluster.nodes {
                        if can_accommodate(cfg_node, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                            println!("Placing service [remaining fallback] {} on node {}", service.name, cfg_node.name);
                            placement_map
                                .entry(service.clone())
                                .or_insert_with(|| Some(HashSet::new()))
                                .as_mut()
                                .unwrap()
                                .insert(cfg_node.clone());
                            break;
                        }
                    }
                }
            }
        }

        Ok(placement_map)
    }

    // pub async fn solve_lp_nsga2(
    //     &mut self,
    //     service_tree: ServiceGraph,
    //     node_tree: NodeTree,
    // ) -> Result<HashMap<Service, Option<HashSet<Node>>>, Box<dyn std::error::Error>> {

    //     let all_services = &self.config.services;
    //     let all_nodes = &self.config.cluster.nodes;

    //     // create service-service mappings from the service tree
    //     let service_comms = service_tree.get_service_pairs(all_services.clone());
    //     let node_comms = node_tree.get_tree();
    //     let service_resources = self.get_service_resources(&all_services.clone(), &all_nodes.clone()).await;

    //     // Create an empty map to hold Node as key, and Resource & Network as tuple values
    //     let mut resource_map: HashMap<Node, (Resource, Network)> = HashMap::new();

    //     for node in &self.config.cluster.nodes {
    //         let node_utilization = self.api_client.get_node_utilization(&node.name).await?;
    //         let node_environment = self.api_client.get_node_environment(&node.name).await?;
    //         resource_map.insert(node.clone(), (node_utilization, node_environment));
    //     }

    //     let mut node_costs: HashMap<Node, f64> = HashMap::new();

    //     for (node, (_resource, network)) in &resource_map {
    //         node_costs.insert(node.clone(), self.compute_network_cost(network, &resource_map));
    //     }

    //     let mut node_resources: HashMap<Node, Resource> = HashMap::new();
    //     for node in &self.config.cluster.nodes {
    //         let node_utilization = self.api_client.get_node_utilization(&node.name).await?;
    //         node_resources.insert(node.clone(), node_utilization);
    //     }

    //     let mut available_resources: HashMap<Node, Resource> = HashMap::new();
    //     for (node, resource) in &node_resources {
    //         let resource_int = self.config.cluster.nodes.iter().find(|n| n.name == node.name).unwrap().resource.clone();
    //         let available = resource_diff(resource_int, resource.clone());
    //         available_resources.insert(node.clone(), available);
    //     }

    //     // Create the constraints
    //     let mut constraints = Vec::new();

    //     //create node resource constraints
    //     for node in &available_resources {
    //         let (cpu, memory, disk, network) = (node.1.cpu, node.1.memory, node.1.disk, node.1.network);
    //         let mut resource_constraint: HashMap<u64, (f64, f64, f64, f64)> = HashMap::new();   
    //         resource_constraint.insert(node.0.id as u64, (cpu, memory, disk, network));
    //         let constraint = Constraint::new(&node.0.name, RelationalOperator::LessOrEqualTo, None, None, Some(resource_constraint));
    //         constraints.push(constraint);
    //     }

    //     // print all the constraints
    //     println!("Constraints: {:?}", constraints);


    //     // Create the problem
    //     let problem = MicroservicePlacementProblem::create(
    //         self.config.clone(),
    //         service_comms,
    //         node_comms.clone(), 
    //         node_costs,
    //         service_resources,
    //         available_resources,
    //         Some(constraints),

    //     )?;

    //     //let mutation_operator_options = PolynomialMutationArgs::default(&problem);
    //     let mutation_operator_options = PolynomialMutationArgs {
    //         // ensure different variable value (with integers)
    //         index_parameter: 1.0,
    //         // always force mutation
    //         variable_probability: 0.7,
    //     };

    //     // Customise the SBX and PM operators like in the paper
    //     let crossover_operator_options = SimulatedBinaryCrossoverArgs {
    //         distribution_index: 30.0,
    //         crossover_probability: 1.0,
    //         ..SimulatedBinaryCrossoverArgs::default()
    //     };

    //     // Setup the NSGA2 algorithm
    //     let args = NSGA2Arg {
    //         // use 100 individuals and stop the algorithm at 250 generations
    //         number_of_individuals: 100,
    //         stopping_condition: StoppingConditionType::MaxGeneration(MaxGenerationValue(250)),
    //         // use default options for the SBX and PM operators
    //         crossover_operator_options: Some(crossover_operator_options),
    //         mutation_operator_options: Some(mutation_operator_options),
    //         //mutation_operator_options: None,  
    //         // no need to evaluate the objective in parallel
    //         parallel: Some(false),
    //         // do not export intermediate solutions
    //         export_history: None,
    //         resume_from_file: None,
    //         // to reproduce results
    //         seed: Some(10),
    //     };

    //     let mut algo = NSGA2::new(problem, args)?;

    //     // initialize the timestamp 
    //     let timestamp0 = chrono::Utc::now().timestamp();

    //     // run the algorithm
    //     algo.run().unwrap();

    //     let (best, _value) = get_best_individual(&algo.get_results().individuals, ObjectiveDirection::Minimise);

    //     // get the best individual
    //     let best_individual = best.serialise();

    //     // get values of the best individual
    //     let best_values = best_individual.variable_values;

    //     // create the placement map
    //     let mut placement_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();

    //     for (service, var) in best_values {
    //         // get Service from service
    //         let service = get_services_by_names(service, &self.config.services).unwrap();
    //         match var {
    //             VariableValue::Choice(id) => {
    //                 let node = get_node_by_id(id as i64, &self.config.cluster.nodes).unwrap();
    //                 placement_map.entry(service.clone()).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(node);
    //             }
    //             // ignore the rest
    //             _ => {}
    //         }
    //     }

    //     // update the placement map
    //     self.placement = Some(placement_map.clone());

    //     // update the revision  
    //     self.revision += 1;

    //     // print the placement map
    //     print_placement_map(placement_map.clone());

    //     // get the timestamp
    //     let timestamp1 = chrono::Utc::now().timestamp();

    //     // print the time taken
    //     println!("Time taken to solve the problem: {} seconds", timestamp1 - timestamp0);

    //     Ok(placement_map)
    // }


    pub async fn solve_lp_nsga2opticas(
        &mut self,
        service_tree: ServiceGraph,
        node_tree: NodeTree,
    ) -> Result<HashMap<Service, Option<HashSet<Node>>>, Box<dyn std::error::Error>> {

        let all_services = &self.config.services;
        let all_nodes = &self.config.cluster.nodes;

        // create service-service mappings from the service tree
        let service_comms = service_tree.get_service_pairs(all_services.clone());
        let node_comms = node_tree.get_tree();
        let service_resources = self.get_service_resources(&all_services.clone(), &all_nodes.clone()).await;

        // Print the service communications and node communications
        // println!("Service Communications: {:?}", service_comms);
        // println!("Node Communications: {:?}", node_comms);

        // print the service resources
        // println!("Service Resources: {:?}", service_resources);

        // // get the worst-case cost
        // println!("Worst-case cost: {}", node_tree.get_worst_cost());

        // // get max. message count from service_comms
        // println!("Max. message count: {}", service_tree.get_highest_message_count(&service_comms));

        let max_optimization_cost = node_tree.get_worst_cost() * service_tree.get_highest_message_count(&service_comms) as f64;

        // // print the max optimization cost
        // println!("Max Optimization Cost: {}", max_optimization_cost);

        // Create an empty map to hold Node as key, and Resource & Network as tuple values
        let mut resource_map: HashMap<Node, (Resource, Network)> = HashMap::new();

        for node in &self.config.cluster.nodes {
            let node_available = self.api_client.get_node_utilization(&node.name).await?; // base available resources | may include services using them.
            let node_environment = self.api_client.get_node_environment(&node.name).await?;

            //let available_resources = node_utilization.clone();

            // get services on the node
            let services = self.api_client.get_node_services(&node.name).await?;

            if services.len() > 0 {
                // print the services
                // println!("Services on node {}: {:?}", node.name, services);

                // initialize the Resource for the services
                let mut service_resources = Resource::default();

                for service in services {
                    let service_utilization = self.api_client.get_node_service_utilization(&node.name, &service).await?;
                    service_resources = resource_sum(service_resources, service_utilization);
                }

                // Add the service_resources to the node_utilization
                let node_available = resource_sum_sub(node_available, service_resources);

                // Populate the resource map
                resource_map.insert(node.clone(), (node_available, node_environment));
            }
             else {
                //println!("No services on node {}", node.name);

                // Populate the resource map
                resource_map.insert(node.clone(), (node_available, node_environment));
             }
            // resource_map.insert(node.clone(), (node_utilization, node_environment));
        }

        // print the resource map
        // println!("Resource Map: {:?}", resource_map);

        let mut node_utilization: HashMap<Node, Resource> = HashMap::new();

        // get available resources from the resource map
        for (node, (resource, _network)) in &resource_map {
            // get the node resource int from config
            let resource_int = self.config.cluster.nodes.iter().find(|n| n.name == node.name).unwrap().resource.clone();
            node_utilization.insert(node.clone(), resource_int_subx(resource_int, resource.clone()));
        }        

        // // print the available resources
        // println!("Base Utilization Map (+- services): {:?}", node_utilization);

        // Now compute node costs using node_comms (from node_tree.get_tree()) and base utilization
        let mut node_costs: HashMap<Node, f64> = HashMap::new();
        for (node, (_resource, _network)) in &resource_map {
            let cost = self.compute_node_cost(node, &node_comms, &node_utilization);
            node_costs.insert(node.clone(), cost);
        }

        // print the node costs
        // println!("Node Costs: {:?}", node_costs);        


        // let mut node_costs: HashMap<Node, f64> = HashMap::new();

        // for (node, (_resource, network)) in &resource_map {
        //     node_costs.insert(node.clone(), self.compute_network_cost(network, &resource_map));
        // }

        // // print the node costs
        // println!("Node Costs: {:?}", node_costs);

        // let mut node_utilization: HashMap<Node, Resource> = HashMap::new();

        // // get available resources from the resource map
        // for (node, (resource, _network)) in &resource_map {
        //     // get the node resource int from config
        //     let resource_int = self.config.cluster.nodes.iter().find(|n| n.name == node.name).unwrap().resource.clone();
        //     node_utilization.insert(node.clone(), resource_int_subx(resource_int, resource.clone()));
        // }

        // // print the available resources
        // println!("Base Utilization Map (+- services): {:?}", node_utilization);

        // Print the service tree
        // println!("Service Tree: {:?}", service_tree);
        
        // Find the most popular services
        let (most_popular_services, _) = service_tree.most_popular_services();
        // Find the least cost node
        let lowest_cost_node_id = self.get_lowest_cost_node(&node_costs).id.clone() as u64;

        let minmax_node_cost = self.get_min_max_costs(&node_costs);
        // print the minmax node cost
        // println!("MinMax Node Cost: {:?}", minmax_node_cost);

        // print the most popular services
        // println!("Most Popular Services: {:?}", most_popular_services);

        // print the least cost node
        // println!("Lowest Cost Node: {:?}", lowest_cost_node_id);

        // print the minmax resource imbalance
        let minmax_resource_imbalance = self.get_min_max_resource_imbalance(&node_utilization, &service_resources);
        // println!("MinMax Resource Imbalance: {:?}", minmax_resource_imbalance);

        // Create a constraint that places the most popular services on the least cost node
        let mut constraints = Vec::new();
        for service in &most_popular_services {
           let constraint = OConstraint::new(
                service, 
                ORelationalOperator::EqualTo, 
                Some(lowest_cost_node_id.clone()), 
                None, 
                None
            );
            constraints.push(constraint);
        }

        // Create the group constraints (similar services on the same node)
        // let service_groups = self.config.grouped_services();
        // for group in &service_groups {
        //     let len = group.1.len() as u64;
        //     if len > 1 {
        //         // create a name for the group
        //         let group_name = format!("{}_group", group.0);
        //         let services = Some(group.1.iter().map(|service| service.name.clone()).collect());
        //         let constraint = OConstraint::new(&group_name, ORelationalOperator::EqualTo, None, services, None);
        //         constraints.push(constraint);
        //     }
        // }


        // create node resource constraints
        // for node in &available_resources {
        //     let (cpu, memory, disk, network) = (node.1.cpu, node.1.memory, node.1.disk, node.1.network);
        //     let mut resource_constraint: HashMap<u64, (f64, f64, f64, f64)> = HashMap::new();   
        //     resource_constraint.insert(node.0.id as u64, (cpu, memory, disk, network));
        //     let constraint = Constraint::new(&node.0.name, RelationalOperator::LessOrEqualTo, None, None, Some(resource_constraint));
        //     constraints.push(constraint);
        // }

        // print all the constraints
        println!("Constraints: {:?}", constraints);


        // Create the problem
        let problem = OMicroservicePlacementProblem::create(
            self.config.clone(),
            service_comms,
            node_comms.clone(),
            node_costs,
            max_optimization_cost,
            minmax_node_cost,
            minmax_resource_imbalance,
            service_resources,
            node_utilization,
            Some(constraints),

        )?;

            //let mutation_operator_options = PolynomialMutationArgs::default(&problem);
        let mutation_operator_options = OPolynomialMutationArgs {
            // ensure different variable value (with integers)
            index_parameter: 1.0,
            // always force mutation
            variable_probability: 0.5,
        };

        // Customise the SBX and PM operators like in the paper
        let crossover_operator_options = OSimulatedBinaryCrossoverArgs {
            distribution_index: 30.0,
            crossover_probability: 1.0,
            ..OSimulatedBinaryCrossoverArgs::default()
        };

        // Setup the NSGA2 algorithm
        let args = NSGA2OPTICASArg {
            // use 100 individuals and stop the algorithm at 250 generations
            number_of_individuals: 100,
            stopping_condition: OStoppingConditionType::MaxGeneration(OMaxGenerationValue(250)),
            // use default options for the SBX and PM operators
            crossover_operator_options: Some(crossover_operator_options),
            mutation_operator_options: Some(mutation_operator_options),
            //mutation_operator_options: None,  
            // no need to evaluate the objective in parallel
            parallel: Some(false),
            // do not export intermediate solutions
            export_history: None,
            resume_from_file: None,
            // to reproduce results
            seed: Some(10),
        };

        let mut algo = NSGA2OPTICAS::new(problem, args)?;

        // set the timestamp
        let timestamp0 = chrono::Utc::now().timestamp();

        // run the algorithm
        algo.run().unwrap();

        let (best, value) = opticas_get_best_individual(&algo.get_results().individuals, OObjectiveDirection::OMinimise);

        // get the best individual
        let best_individual = best.serialise();

        // get values of the best individual
        let best_values = best_individual.variable_values;

        // print the best values
        // println!("Objective solution value: {:?}", value);

        // create the placement map
        let mut placement_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();

        for (service, var) in best_values {
            // get Service from service
            let service = get_services_by_names(service, &self.config.services).unwrap();
            match var {
                OVariableValue::OChoice(id) => {
                    let node = get_node_by_id(id as i64, &self.config.cluster.nodes).unwrap();
                    placement_map.entry(service.clone()).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(node);
                }
                // ignore the rest
                //_ => {}
            }
        }

        // compute the placement difference
        let diff = self.compute_placement_diff(&placement_map, &self.placement.as_ref().unwrap());

        if self.revision <= 1 {
            println!("First run of solve_lp");
            self.obj_value = Some(value);
            self.revision += 1;
            self.placement = Some(placement_map.clone());

            // print the placement map
            print_placement_map(placement_map.clone());

            // get the timestamp
            let timestamp1 = chrono::Utc::now().timestamp();

            // print the time taken
            println!("Time taken to solve the problem: {} seconds", timestamp1 - timestamp0);

            Ok(placement_map)
        }

        else {
            if value > self.obj_value.unwrap() {
            //if diff > (0.5 * self.config.services.len() as f64) && value > self.obj_value.unwrap() {
                println!("Placement objectives value {} > current value {}", value, self.obj_value.unwrap());
                Err("No solution found".into())
            }

            else {
                println!("Placement objectives value {} <= current value {}", value, self.obj_value.unwrap());
                self.obj_value = Some(value);
                self.revision += 1;
                self.placement = Some(placement_map.clone());

                // print the placement map  
                // println!("Placement Map: {:?}", placement_map);
                print_placement_map(placement_map.clone());

                // get the timestamp
                let timestamp1 = chrono::Utc::now().timestamp();

                // print the time taken
                println!("Time taken to solve the problem: {} seconds", timestamp1 - timestamp0);

                Ok(placement_map)
            }
        }
    }

    pub fn assign_services(
        &self,
        proportion_map: &HashMap<Node, u32>,
        grouped_services: Vec<(String, Vec<Service>)>
    ) -> HashMap<Service, Node> {
        let mut assignment_map: HashMap<Service, Node> = HashMap::new();
        
        // Convert remaining capacity into a Vec for easy access and sorting
        let mut nodes: Vec<(Node, u32)> = proportion_map.iter().map(|(node, &capacity)| (node.clone(), capacity)).collect();
        
        // Sort nodes by capacity in descending order
        nodes.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Sort groups by size (number of services) in descending order
        let mut service_groups: Vec<(String, Vec<Service>)> = grouped_services.clone();
        service_groups.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    
        for (group, services) in service_groups {
            let group_size = services.len() as u32;
    
            // Find the node with the highest remaining capacity
            if let Some((node, capacity)) = nodes.first_mut() {
                // Assign the group to this node
                assignment_map.extend(services.iter().map(|service| (service.clone(), node.clone())));
                
                // Update the remaining capacity of the node
                let new_capacity = capacity.saturating_sub(group_size);
                *capacity = new_capacity; // Update the node's capacity
    
                // Print the assignment and current capacity
                println!("Assigned group {} with {} services to node '{}'. Remaining capacity: {}", group, group_size, node.name, new_capacity);
                
                // Sort nodes again to ensure the highest capacity node is first
                nodes.sort_by(|a, b| b.1.cmp(&a.1));
            } else {
                println!("Warning: No available nodes to assign the group {} of {} services.", group, group_size);
            }
        }
    
        assignment_map
    }

    /// Connectivity-based cost: uses node_comms aggregated outgoing edge values.
    /// We assume *higher edge value = better connectivity*. Cost := 1 - (local_avg / global_max_avg)
    /// Returns a value in [0, 1] (0 = best connectivity).
    pub fn compute_connectivity_cost(
        &self,
        node: &Node,
        node_comms: &HashMap<String, Vec<AggLinkEdge>>,
    ) -> f64 {
        // Get outgoing edges for this node by name
        let edges = node_comms.get(&node.name);

        // If the node has no recorded edges -> treat as worst connectivity (cost = 1.0)
        let local_avg = match edges {
            Some(list) if !list.is_empty() => {
                list.iter().map(|e| e.edge).sum::<f64>() / (list.len() as f64)
            }
            _ => {
                return 1.0;
            }
        };

        // Find maximum average among all nodes to normalize
        let mut global_max_avg = 0.0;
        for (_n, list) in node_comms.iter() {
            if !list.is_empty() {
                let avg = list.iter().map(|e| e.edge).sum::<f64>() / (list.len() as f64);
                if avg > global_max_avg {
                    global_max_avg = avg;
                }
            }
        }

        // If no global max (shouldn't happen) treat as worst
        if global_max_avg == 0.0 {
            return 1.0;
        }

        // Since higher edge => better, cost should be inverted: lower cost for higher avg
        let ratio = local_avg / global_max_avg;
        let cost = 1.0 - ratio;

        // Clamp just in case of rounding
        cost.max(0.0).min(1.0)
    }

    /// Resource pressure cost (0 = no pressure, 1 = fully saturated).
    /// Uses base_utilization: HashMap<Node, Resource> where Resource fields are f64 usage.
    pub fn compute_resource_pressure(
        &self,
        node: &Node,
        base_utilization: &HashMap<Node, Resource>,
    ) -> f64 {
        // Map Node.resource (ResourceInt) -> Resource (f64)
        let cap = Resource {
            cpu: node.resource.cpu as f64,
            memory: node.resource.memory as f64,
            disk: node.resource.disk as f64,
            network: node.resource.network as f64,
        };

        // Monitored usage (if missing, treat as zero)
        let used = base_utilization.get(node).cloned().unwrap();

        let safe_frac = |u: f64, t: f64| -> f64 {
            if t <= 0.0 {
                // If total capacity unknown or zero, treat as high pressure (1.0)
                1.0
            } else {
                (u / t).max(0.0)
            }
        };

        let cpu_p = safe_frac(used.cpu, cap.cpu);
        let mem_p = safe_frac(used.memory, cap.memory);
        let disk_p = safe_frac(used.disk, cap.disk);
        let net_p = safe_frac(used.network, cap.network);

        // average fractional utilization; clamp to 0..1
        let avg_pressure = (cpu_p + mem_p + disk_p + net_p) / 4.0;
        avg_pressure.min(1.0).max(0.0)
    }    


    /// Combined node cost [0..1]. Lower = cheaper/better for placement.
    /// `node_comms`: HashMap keyed by node name -> outgoing edges
    /// `base_utilization`: monitored usage per Node
    pub fn compute_node_cost(
        &self,
        node: &Node,
        node_comms: &HashMap<String, Vec<AggLinkEdge>>,
        base_utilization: &HashMap<Node, Resource>,
    ) -> f64 {
        // compute sub-costs
        let conn_cost = self.compute_connectivity_cost(node, node_comms);
        let pressure_cost = self.compute_resource_pressure(node, base_utilization);

        // get weights from config if present; otherwise defaults
        // prefer network-first weight set A: connectivity heavier
        let w_conn = 0.5;
        let w_press = 0.5;

        // normalize weights to sum to 1
        let total_w = w_conn + w_press;
        let w_conn = if total_w == 0.0 { 0.5 } else { w_conn / total_w };
        let w_press = if total_w == 0.0 { 0.5 } else { w_press / total_w };

        // simple linear combination: smaller is better
        let final_cost = w_conn * conn_cost + w_press * pressure_cost;

        // Clamp to 0..1
        final_cost.max(0.0).min(1.0)
    }




    fn compute_nr(&mut self, utilization: &Resource, resource_map: &HashMap<Node, (Resource, Network)>) -> f64 {
        let mut max_cpu = 0.0;
        let mut max_memory = 0.0;
        let mut min_network = f64::MAX;
        let mut max_disk = 0.0;
    
        for (_, (resource, _)) in resource_map {
            if resource.cpu > max_cpu {
                max_cpu = resource.cpu;
            }
            if resource.memory > max_memory {
                max_memory = resource.memory;
            }
            if resource.network < min_network {
                min_network = resource.network;
            }
            if resource.disk > max_disk {
                max_disk = resource.disk;
            }
        }
    
        let cpu = utilization.cpu;
        let memory = utilization.memory;
        let network = utilization.network;
        let disk = utilization.disk;
    
        // Safeguard against division by zero
        let safe_div = |num: f64, denom: f64| if denom == 0.0 { f64::MAX } else { num / denom };
    
        let nr = safe_div(cpu, max_cpu) * self.config.get_weight("cpu") +
                 safe_div(memory, max_memory) * self.config.get_weight("memory") + 
                 safe_div(disk, max_disk) * self.config.get_weight("disk") +
                 safe_div(min_network, network) * self.config.get_weight("network");
    
        nr
    }
    
    fn _compute_ne(&mut self, network: &Network, resource_map: &HashMap<Node, (Resource, Network)>) -> f64 {
        let mut max_bandwidth = 0.0;
        let mut min_latency = f64::MAX;
        let mut min_packet_loss = f64::MAX;
        let mut max_available = 0.0;
    
        // Find best network attributes across all nodes
        for (_, (_, node_network)) in resource_map {
            if node_network.bandwidth > max_bandwidth {
                max_bandwidth = node_network.bandwidth;
            }
            if node_network.latency < min_latency {
                min_latency = node_network.latency;
            }
            if node_network.packet_loss < min_packet_loss {
                min_packet_loss = node_network.packet_loss;
            }
            if node_network.available > max_available {
                max_available = node_network.available;
            }
        }
    
        let bandwidth = network.bandwidth;
        let latency = network.latency;
        let packet_loss = network.packet_loss;
        let available = network.available as f64;
    
        // Debug print all attributes and their weights
        // println!("Bandwidth: {}, Latency: {}, Packet Loss: {}, Available: {}", bandwidth, latency, packet_loss, available);
        // println!("Max Bandwidth: {}, Min Latency: {}, Min Packet Loss: {}, Max Available: {}", max_bandwidth, min_latency, min_packet_loss, max_available);
        // println!("Weights: Bandwidth: {}, Latency: {}, Packet Loss: {}, Available: {}", 
        //          self.config.get_weight("bandwidth"), 
        //          self.config.get_weight("latency"), 
        //          self.config.get_weight("packet_loss"), 
        //          self.config.get_weight("available"));
    
        // Safeguard against division by zero
        let safe_div = |num: f64, denom: f64| if denom == 0.0 { 0.0 } else { num / denom };
    
        let ne = safe_div(bandwidth, max_bandwidth) * self.config.get_weight("bandwidth") +
                 safe_div(min_latency, latency) * self.config.get_weight("latency") +
                 safe_div(min_packet_loss, packet_loss) * self.config.get_weight("packet_loss") +
                 safe_div(available, max_available) * self.config.get_weight("available");
    
        ne
    }

    fn compute_network_cost(&mut self, network: &Network, resource_map: &HashMap<Node, (Resource, Network)>) -> f64 {
        let mut max_bandwidth = 0.0;
        let mut min_latency = f64::MAX;
        let mut min_packet_loss = f64::MAX;
        let mut max_available = 0.0;
    
        // Find best network attributes across all nodes
        for (_, (_, node_network)) in resource_map {
            if node_network.bandwidth > max_bandwidth {
                max_bandwidth = node_network.bandwidth;
            }
            if node_network.latency < min_latency {
                min_latency = node_network.latency;
            }
            if node_network.packet_loss < min_packet_loss {
                min_packet_loss = node_network.packet_loss;
            }
            if node_network.available > max_available {
                max_available = node_network.available;
            }
        }
    
        let bandwidth = network.bandwidth;
        let latency = network.latency;
        let packet_loss = network.packet_loss;
        let available = network.available as f64;
    
        // Safeguard against division by zero
        let safe_div = |num: f64, denom: f64| if denom == 0.0 { f64::MAX } else { num / denom };
    
        // Print debugging information
        println!("Bandwidth: {}, Latency: {}, Packet Loss: {}, Available: {}", bandwidth, latency, packet_loss, available);
        println!("Max Bandwidth: {}, Min Latency: {}, Min Packet Loss: {}, Max Available: {}", max_bandwidth, min_latency, min_packet_loss, max_available);
        println!("Weights: Bandwidth: {}, Latency: {}, Packet Loss: {}, Available: {}", 
                 self.config.get_weight("bandwidth"), 
                 self.config.get_weight("latency"), 
                 self.config.get_weight("packet_loss"), 
                 self.config.get_weight("available"));
    
        // Compute the cost (`ne`) based on inversed normalization for cost minimization
        let ne = safe_div(max_bandwidth, bandwidth) * self.config.get_weight("bandwidth") +
                 safe_div(latency, min_latency) * self.config.get_weight("latency") +
                 safe_div(packet_loss, min_packet_loss) * self.config.get_weight("packet_loss") +
                 safe_div(max_available, available) * self.config.get_weight("available");
    
        ne
    }

    async fn get_service_resources(&self, services: &Vec<Service>, nodes: &Vec<Node>) -> HashMap<Service, Vec<Option<(Node, Resource)>>> {
        let mut service_resources: HashMap<Service, Vec<Option<(Node, Resource)>>> = HashMap::new();

        for service in services {
            // determine the node for the service
            for node in nodes {
                let service_name_0 = service.name.clone();
                // add the stack name to the service name
                let service_name = format!("{}{}", self.config.cluster.prometheus.stack, service_name_0);

                let service_node_util = self.api_client.get_node_service_utilization(&node.name, &service_name);
                match service_node_util.await {
                    Ok(resource) => {
                        service_resources.entry(service.clone()).or_insert_with(|| Vec::new()).push(Some((node.clone(), resource)));
                    }
                    Err(_) => {
                        service_resources.entry(service.clone()).or_insert_with(|| Vec::new()).push(None);
                    }
                }
            }
        }

        service_resources
    }

    // A function that takes node costs and returns the node with the lowest cost
    fn get_lowest_cost_node(&self, node_costs: &HashMap<Node, f64>) -> Node {
        let mut lowest_cost = f64::MAX;
        let mut lowest_cost_node = Node::default();

        for (node, cost) in node_costs {
            if *cost < lowest_cost {
                lowest_cost = *cost;
                lowest_cost_node = node.clone();
            }
        }

        lowest_cost_node
    }

    // A function that takes node costs and returns the min and max costs
    fn get_min_max_costs(&self, node_costs: &HashMap<Node, f64>) -> (f64, f64) {
        let mut min_cost = f64::MAX;
        let mut max_cost = 0.0;

        for cost in node_costs.values() {
            if *cost < min_cost {
                min_cost = *cost;
            }
            if *cost > max_cost {
                max_cost = *cost;
            }
        }

        (min_cost, max_cost)
    }

    // A function that takes available resources and service utilization (requirements) to compute the min. and max. resource imbalance
    fn get_min_max_resource_imbalance(
        &self,
        node_utilization_map: &HashMap<Node, Resource>,
        service_resources: &HashMap<Service, Vec<Option<(Node, Resource)>>>
    ) -> (f64, f64) {
        // Create a node utilization map that uses node_id as key
        let mut utilization_map: HashMap<u64, Resource> = HashMap::new();
        for (node, resource) in node_utilization_map {
            utilization_map.insert(node.id as u64, resource.clone());
        }

        // Step 1: Compute maximum resource values for each type
        let mut max_resource = Resource {
            cpu: 0.0,
            memory: 0.0,
            disk: 0.0,
            network: 0.0,
        };
    
        for resource in utilization_map.values() {
            max_resource.cpu = max_resource.cpu.max(resource.cpu);
            max_resource.memory = max_resource.memory.max(resource.memory);
            max_resource.disk = max_resource.disk.max(resource.disk);
            max_resource.network = max_resource.network.max(resource.network);
        }
    
        // Step 2: Compute minimum resource values for each type
        let mut min_resource = Resource {
            cpu: f64::MAX,
            memory: f64::MAX,
            disk: f64::MAX,
            network: f64::MAX,
        };
    
        for resource in node_utilization_map.values() {
            min_resource.cpu = min_resource.cpu.min(resource.cpu);
            min_resource.memory = min_resource.memory.min(resource.memory);
            min_resource.disk = min_resource.disk.min(resource.disk);
            min_resource.network = min_resource.network.min(resource.network);
        }


    
        // Step 3: Compute total utilization of services
        let mut total_service_resources = Resource::default();

        for resources in service_resources.values() {
            for resource in resources.iter().filter_map(|r| r.as_ref().map(|(_, res)| res)) {
                total_service_resources.cpu += resource.cpu;
                total_service_resources.memory += resource.memory;
                total_service_resources.disk += resource.disk;
                total_service_resources.network += resource.network;
            }
        }

        // Print the total resources
        // println!("Total Service Resources: {:?}", total_service_resources);

        let mut max_utilization_map = utilization_map.clone();
        // create a new node with max. resources and add to the utilization map
        max_utilization_map.insert(u64::MAX, resource_sum(max_resource.clone(), total_service_resources.clone()));

        let mut min_utilization_map = utilization_map.clone();
        // create a new node with min. resources and add to the utilization map
        min_utilization_map.insert(u64::MIN, resource_sum(min_resource.clone(), total_service_resources.clone()));

        // Get the average utilization across all nodes in max_utilization_map and min_utilization_map
        let node_count_max = max_utilization_map.len() as f64;
        let mut max_variance = Resource::default();

        for resource in max_utilization_map.values() {
            max_variance.add(resource);
        }

        let node_count_min = min_utilization_map.len() as f64;
        let mut min_variance = Resource::default();

        for resource in min_utilization_map.values() {
            min_variance.add(resource);
        }

        let max_avg_resource = Resource {
            cpu: max_variance.cpu / node_count_max,
            memory: max_variance.memory / node_count_max,
            disk: max_variance.disk / node_count_max,
            network: max_variance.network / node_count_max,
        };

        let min_avg_resource = Resource {
            cpu: min_variance.cpu / node_count_min,
            memory: min_variance.memory / node_count_min,
            disk: min_variance.disk / node_count_min,
            network: min_variance.network / node_count_min,
        };

        // calculate the variance for each resource type
        let mut max_var = Resource::default();
        let mut min_var = Resource::default();

        for resource in max_utilization_map.values() {
            max_var.cpu += (resource.cpu - max_avg_resource.cpu).powi(2);
            max_var.memory += (resource.memory - max_avg_resource.memory).powi(2);
            max_var.disk += (resource.disk - max_avg_resource.disk).powi(2);
            max_var.network += (resource.network - max_avg_resource.network).powi(2);
        }

        for resource in min_utilization_map.values() {
            min_var.cpu += (resource.cpu - min_avg_resource.cpu).powi(2);
            min_var.memory += (resource.memory - min_avg_resource.memory).powi(2);
            min_var.disk += (resource.disk - min_avg_resource.disk).powi(2);
            min_var.network += (resource.network - min_avg_resource.network).powi(2);
        }
   
        // Step 6: Calculate standard deviation (imbalance) for max and min bounds
        let max_imbalance = (max_var.cpu + max_var.memory + max_var.disk + max_var.network).sqrt() / node_count_max;
        let min_imbalance = (min_var.cpu + min_var.memory + min_var.disk + min_var.network).sqrt() / node_count_min;
    
        (min_imbalance, max_imbalance)
    }


    // A function that takes two placements and computes the difference
    fn compute_placement_diff(&self, placement1: &HashMap<Service, Option<HashSet<Node>>>, placement2: &HashMap<Service, Option<HashSet<Node>>>) -> f64 {
        let mut diff = 0.0;

        for (service, node) in placement1 {
            if let Some(nodes) = placement2.get(service) {
                if nodes.is_some() {
                    if nodes != node {
                        diff += 1.0;
                    }
                }
            }
        }

        diff
    }

}



fn get_lowest_coordinates(coordinates: &[Coordinate]) -> (f64, f64) {
    let mut lowest_x = coordinates[0].x;
    let mut lowest_y = coordinates[0].y;

    for coordinate in coordinates {
        if coordinate.x < lowest_x {
            lowest_x = coordinate.x;
        }
        if coordinate.y < lowest_y {
            lowest_y = coordinate.y;
        }
    }

    (lowest_x, lowest_y)
}

fn compute_euclidean_distance(coordinates: &[Coordinate], target_x: f64, target_y: f64) -> f64 {
    // Ensure coordinates is not empty
    if coordinates.is_empty() {
        return f64::MAX; // Or handle it as appropriate for your application
    }

    let (lowest_x, lowest_y) = get_lowest_coordinates(coordinates);
    let distance = ((target_x - lowest_x).powi(2) + (target_y - lowest_y).powi(2)).sqrt();

    // Return a small value if the distance is 0
    if distance == 0.0 {
        return 1e-10; // or any small positive value you prefer
    }

    distance
}

// fn compute_proportion(distance: f64, distance_map: &HashMap<Node, f64>, j: u32) -> u32 {
//     let total_distance: f64 = distance_map.values().sum();

//     let proportion = (distance / total_distance) * j as f64;

//     proportion.round() as u32
// }

fn compute_proportion(distance: f64, distance_map: &HashMap<Node, f64>, j: u32) -> u32 {
    // Handle zero distance to avoid division by zero
    if distance <= 0.0 {
        println!("Invalid distance: {}", distance);
        return 0;
    }

    // Calculate total inverse distance for all nodes
    let total_inverse_distance: f64 = distance_map.values()
        .map(|&d| 1.0 / d)
        .sum();

    // Calculate the proportion of services to assign to the node
    let proportion = (1.0 / distance) / total_inverse_distance * j as f64;

    // Print number of services and proportion
    // println!("Number of services: {}, Proportion: {}", j, proportion);

    // Round the proportion up to the next integer
    let rounded_proportion = proportion.ceil() as u32;

    // Ensure the proportion does not exceed the total number of services
    if rounded_proportion > j {
        return j;
    }

    rounded_proportion
}

// a function that takes a service and returns all dependencies based on db, cache attributes of Service
fn get_dependencies(service: &Service, services: &Vec<Service>) -> Vec<Service> { // need all the services to search
    let mut dependencies = Vec::new();

    //Dependencies: [Service { id: "3", name: "search", cache: Some(""), db: Some("") }, Service { id: "3_cache", name: "", cache: None, db: None }, Service { id: "3_db", name: "", cache: None, db: None }]
    //Dependencies: [Service { id: "1", name: "frontend", cache: Some(""), db: Some("") }, Service { id: "1_cache", name: "", cache: None, db: None }, Service { id: "1_db", name: "", cache: None, db: None }]

    dependencies.push(service.clone());

    // check if the service has a cache dependency
    if let Some(cache) = &service.cache {
        let cache_service = get_services_by_names(cache.to_string(), services);
        match cache_service {
            Some(cache_service) => {
                dependencies.push(cache_service.clone());
            }
            None => {
                println!("Cache service for service {} not found", service.name);
            }
        }
    }

    // check if the service has a db dependency
    if let Some(db) = &service.db {
        let db_service = get_services_by_names(db.to_string(), services);
        match db_service {
            Some(db_service) => {
                dependencies.push(db_service.clone());
            }
            None => {
                println!("DB service for service {} not found", service.name);
            }
        }
    }

    // print all dependencies
    // println!("Dependencies: {:?}", dependencies);

    dependencies
}

// function get the resource utilization of a service and its dependencies
fn get_service_deps_utilization(service: &Service, all_services: &Vec<Service>, service_utilization_map: &HashMap<Service, Resource>) -> Resource {
    let services = get_dependencies(service, all_services);

    let mut total_utilization = Resource::default();

    for service in services {
        let utilization = service_utilization_map.get(&service);
        match utilization {
            Some(utilization) => {
                total_utilization.cpu += utilization.cpu;
                total_utilization.memory += utilization.memory;
                total_utilization.disk += utilization.disk;
                total_utilization.network += utilization.network;
            }
            None => {
                println!("Service {} not found in the utilization map", service.name);
            }
        }
    }

    total_utilization
}

// A function to determine if a node can accommodate a service and its dependencies
fn can_accommodate(node: &Node, service: &Service, all_services: &Vec<Service>, remaining_capacity: &mut HashMap<Node, Resource>, service_utilization_map: &HashMap<Service, Resource>) -> bool {
    let service_utilization = get_service_deps_utilization(service, all_services, service_utilization_map);
    let node_capacity = remaining_capacity.get_mut(node).unwrap();

    if node_capacity.cpu >= service_utilization.cpu
        && node_capacity.memory >= service_utilization.memory
        && node_capacity.disk >= service_utilization.disk
        && node_capacity.network >= service_utilization.network
    {
        // Subtract the service's resource usage from the node's remaining capacity
        node_capacity.cpu -= service_utilization.cpu;
        node_capacity.memory -= service_utilization.memory;
        node_capacity.disk -= service_utilization.disk;
        node_capacity.network -= service_utilization.network;
        true
    } else {
        false
    }
}

// A function that takes a String service and Services and returns a Vec of Services
fn get_services_by_names(service: String, all_services: &Vec<Service>) -> Option<Service> {
    for s in all_services {
        if s.name == service {
            return Some(s.clone());
        }
    }

    None
}


fn print_placement_map(placement_map: HashMap<Service, Option<HashSet<Node>>>) {
    // Create a new map to hold nodes and their assigned services
    let mut node_service_map: HashMap<Node, HashSet<Service>> = HashMap::new();

    // Iterate over the placement_map to build the reverse mapping
    for (service, nodes_option) in placement_map {
        if let Some(nodes) = nodes_option {
            for node in nodes {
                // Insert the service into the corresponding node entry
                node_service_map
                    .entry(node.clone())
                    .or_insert_with(HashSet::new)
                    .insert(service.clone());
            }
        }
    }

    // Print each node and its assigned services
    for (node, services) in node_service_map {
        println!("Node: {}", node.name);
        for service in services {
            println!("  - Service: {}", service.name);
        }
    }
}