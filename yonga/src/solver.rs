use std::collections::{HashMap, HashSet};
use std::f64::consts::E;
use std::result::Result::Ok;
use crate::utility::{Config, Node, Service};
use crate::api_client::ApiClient;
use crate::utility::{Network, Resource, resource_diff, get_node_by_id};
use crate::node::NodeTree;
use crate::trace::ServiceGraph;
use crate::nsga2::{MicroservicePlacementProblem, get_best_individual};

//use crate::solver::StoppingConditionType::MaxGeneration


//use anyhow::Ok;
use optirustic::algorithms::{
    Algorithm, MaxGenerationValue, NSGA2Arg, StoppingConditionType, NSGA2
};
use optirustic::core::{Constraint, ObjectiveDirection, RelationalOperator, VariableType, VariableValue
};
use optirustic::operators::{PolynomialMutationArgs, SimulatedBinaryCrossoverArgs};

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
        println!("Running the Solver for placement 0");

        self.placement = Some(HashMap::new());

        // Create an empty map that has a Node as key and values NR, and NE as f64
        let mut node_map: HashMap<Node, Coordinate> = HashMap::new();

        // Create an empty map to hold Node as key, and Resource & Network as tuple values
        let mut resource_map: HashMap<Node, (Resource, Network)> = HashMap::new();

        for node in &self.config.cluster.nodes {

            // print the action
            println!("Retrieving the utilization and environment metrics for node: {}", node.name);

            // Retrieve the node utilization
            let node_utilization = self.api_client.get_node_utilization(&node.name).await?;

            // Retrieve the node environment
            let node_environment = self.api_client.get_node_environment(&node.name).await?;

            // Populate the resource map
            resource_map.insert(node.clone(), (node_utilization, node_environment));
        }

        println!("Resource Map: {:?}", resource_map);

        
        // Use the resource map to populate the node_map
        for (node, (resource, network)) in &resource_map {
            node_map.insert(node.clone(), Coordinate{
                x: self.compute_nr(resource, &resource_map),
                y: self.compute_ne(network, &resource_map),
            });
        }

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

        // trial the service groups
        let (num_groups, groups) = self.config.group_services();

        // println!("Number of groups: {}", num_groups);
        // for group in groups {
        //     println!("Group: {:?}", group);
        // }

        let assignment_map = self.assign_services(&proportion_map, num_groups, groups);

        for (service, node) in &assignment_map {
            println!("Service: {}, Assigned Node: {}", service.name, node.name);
        }

        let mut placement_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();

        for (service, node) in assignment_map {
            placement_map.entry(service).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(node);
        }

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
        }

        else {
            println!("Longest Paths: {:?}", longest_paths);
            println!("Most Popular Services: {:?}", most_popular_services);
            println!("Max Length of the longest path/paths: {}", max_length);
        }


        // convert longest_paths to a Vec of Services
        let longest_paths: Vec<Vec<Service>> = longest_paths.iter().map(|path| {
            path.iter().map(|service| {
                get_services_by_names(service.clone(), &self.config.services).unwrap()
            }).collect()
        }).collect();




        // convert most_popular_services to a Vec of Services
        let most_popular_services: Vec<Service> = most_popular_services.iter().map(|service| {
            get_services_by_names(service.clone(), &self.config.services).unwrap()
        }).collect();

        // Get services in the tree
        let svc_tree = service_tree.get_services();
        let mut services_tree = Vec::new();
        for service in svc_tree {
            let service = get_services_by_names(service, &self.config.services);
            if let Some(service) = service {
                services_tree.push(service);
            }
        }

        // print services in the tree
        // println!("Services in the tree: {:?}", services_tree);
    
        // Compute the strongest paths in the node tree
        let strong_paths = node_tree.compute_strong_paths(node_tree.get_nodes(), max_length - 1);
    
        // Find the strongest path overall
        let strongest_path = node_tree.get_strongest_path(strong_paths.clone());
    
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

        // print the service utilization map
        // println!("Service Utilization Map: {:?}", service_utilization_map);

        // create a placement map
        let mut placement_map: HashMap<Service, Option<HashSet<Node>>> = HashMap::new();

        // use ResourceInt of each node and current utilization map to get remaining capacity
        let mut remaining_capacity: HashMap<Node, Resource> = HashMap::new();
        for (node, resource) in &node_utilization_map {

            // get the node resource int from config
            let resource_int = self.config.cluster.nodes.iter().find(|n| n.name == node.name).unwrap().resource.clone();

            // get the remaining capacity
            let remaining = resource_diff(resource_int, resource.clone());
            remaining_capacity.insert(node.clone(), remaining);
        }

        // create a structure to keep track of assignments
        //let mut assignment_map: HashMap<Service, Node> = HashMap::new();

        // place the most popular services on the root of the strongest path
        for service in &most_popular_services {
            let root = &strongest_path.0[0];
            if can_accommodate(root, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                println!("Placing service {} on root node {}", service.name, root.name);
                // update the placement_map with the service and its dependencies
                let service_dep = get_dependencies(service, all_services);
                for dep in service_dep {
                    placement_map.entry(dep.clone()).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(root.clone());
                    //assignment_map.insert(dep.clone(), root.clone());
                }

            } else {
                // If a node can't accommodate a service, find another suitable node
                for (fallback_node, _) in strong_paths.iter() {
                    if can_accommodate(fallback_node, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                        println!("Placing service {} on fallback node {}", service.name, fallback_node.name);
                        placement_map.entry(service.clone()).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(fallback_node.clone());
                        //assignment_map.insert(service.clone(), fallback_node.clone());
                        break;
                    }
                }
            }
        }

        // place the longest service chain/tree on the strongest path
        for path in &longest_paths {
            for service in path {
                if !placement_map.contains_key(service) {
                    for node in strongest_path.0.iter() {
                        if can_accommodate(node, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                            println!("Placing service [longest service chain] {} on node {}", service.name, node.name);
                            placement_map.entry(service.clone()).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(node.clone());
                            break;
                        }
                    }
                }
            }
        }

        // Handle remaining services not in the longest paths or most popular
        for service in &self.config.services {
            if !placement_map.contains_key(service) {
                for node in strongest_path.0.iter() {
                    if can_accommodate(node, service, all_services, &mut remaining_capacity, &service_utilization_map) {
                        println!("Placing service [remaining] {} on node {}", service.name, node.name);
                        placement_map.entry(service.clone()).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(node.clone());
                        break;
                    }
                }
            }
        }
    
        //println!("Final Placement Map: {:?}", placement_map);
    
        Ok(placement_map)
    }

    pub async fn solve_lp(
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


        // Create an empty map to hold Node as key, and Resource & Network as tuple values
        let mut resource_map: HashMap<Node, (Resource, Network)> = HashMap::new();

        for node in &self.config.cluster.nodes {
            let node_utilization = self.api_client.get_node_utilization(&node.name).await?;
            let node_environment = self.api_client.get_node_environment(&node.name).await?;
            resource_map.insert(node.clone(), (node_utilization, node_environment));
        }

        let mut node_costs: HashMap<Node, f64> = HashMap::new();

        for (node, (_resource, network)) in &resource_map {
            node_costs.insert(node.clone(), self.compute_ne(network, &resource_map));
        }

        let mut node_resources: HashMap<Node, Resource> = HashMap::new();
        for node in &self.config.cluster.nodes {
            let node_utilization = self.api_client.get_node_utilization(&node.name).await?;
            node_resources.insert(node.clone(), node_utilization);
        }

        let mut available_resources: HashMap<Node, Resource> = HashMap::new();
        for (node, resource) in &node_resources {
            let resource_int = self.config.cluster.nodes.iter().find(|n| n.name == node.name).unwrap().resource.clone();
            let available = resource_diff(resource_int, resource.clone());
            available_resources.insert(node.clone(), available);
        }

        // Find the most popular services
        let (most_popular_services, _) = service_tree.most_popular_services();
        // Find the least cost node
        let lowest_cost_node_id = self.get_lowest_cost_node(&node_costs).id.clone() as u64;


        // print the most popular services
        println!("Most Popular Services: {:?}", most_popular_services);

        // print the least cost node
        println!("Lowest Cost Node: {:?}", lowest_cost_node_id);

        // Create a constraint that places the most popular services on the least cost node
        let mut constraints = Vec::new();
        // for service in &most_popular_services {
        //    let constraint = Constraint::new(
        //         service, 
        //         RelationalOperator::EqualTo, 
        //         Some(lowest_cost_node_id.clone()), 
        //         None, 
        //         None
        //     );
        //     constraints.push(constraint);
        // }

        // Create the group constraints (similar services on the same node)
        let service_groups = self.config.grouped_services();
        for group in &service_groups {
            let len = group.1.len() as u64;
            if len > 1 {
                // create a name for the group
                let group_name = format!("{}_group", group.0);
                let services = Some(group.1.iter().map(|service| service.name.clone()).collect());
                let constraint = Constraint::new(&group_name, RelationalOperator::EqualTo, None, services, None);
                constraints.push(constraint);
            }
        }


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
        let problem = MicroservicePlacementProblem::create(
            self.config.clone(),
            service_comms,
            node_comms.clone(),
            node_costs,
            service_resources,
            available_resources,
            Some(constraints),

        )?;

            //let mutation_operator_options = PolynomialMutationArgs::default(&problem);
        let mutation_operator_options = PolynomialMutationArgs {
            // ensure different variable value (with integers)
            index_parameter: 1.0,
            // always force mutation
            variable_probability: 0.7,
        };

        // Customise the SBX and PM operators like in the paper
        let crossover_operator_options = SimulatedBinaryCrossoverArgs {
            distribution_index: 30.0,
            crossover_probability: 1.0,
            ..SimulatedBinaryCrossoverArgs::default()
        };

        // Setup the NSGA2 algorithm
        let args = NSGA2Arg {
            // use 100 individuals and stop the algorithm at 250 generations
            number_of_individuals: 100,
            stopping_condition: StoppingConditionType::MaxGeneration(MaxGenerationValue(250)),
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

        let mut algo = NSGA2::new(problem, args)?;

        // run the algorithm
        algo.run().unwrap();

        let (best, value) = get_best_individual(&algo.get_results().individuals, ObjectiveDirection::Minimise);

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
                VariableValue::Choice(id) => {
                    let node = get_node_by_id(id as i64, &self.config.cluster.nodes).unwrap();
                    placement_map.entry(service.clone()).or_insert_with(|| Some(HashSet::new())).as_mut().unwrap().insert(node);
                }
                // ignore the rest
                _ => {}
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
            println!("Placement Map: {:?}", placement_map);
            Ok(placement_map)
        }

        else {
            if diff > (0.5 * self.config.services.len() as f64) && value > self.obj_value.unwrap() {
                println!("Placement difference {} > 50% or objectives value {} > current value {}", diff, value, self.obj_value.unwrap());
                Err("No solution found".into())
            }

            else {
                println!("Placement difference {} or objectives value {} is less than the current value - to deployment", diff, value);
                self.obj_value = Some(value);
                self.revision += 1;
                self.placement = Some(placement_map.clone());

                // print the placement map  
                println!("Placement Map: {:?}", placement_map);
                Ok(placement_map)
            }
        }
    }




    fn assign_services(
        &self,
        proportion_map: &HashMap<Node, u32>,
        num_groups: usize,
        grouped_services: Vec<Vec<Service>>
    ) -> HashMap<Service, Node> {
        let mut assignment_map: HashMap<Service, Node> = HashMap::new();
        let mut remaining_capacity: HashMap<Node, u32> = proportion_map.clone();
    
        let mut nodes: Vec<_> = remaining_capacity.iter_mut().collect();
        nodes.sort_by(|a, b| b.1.cmp(&a.1));
    
        for group_index in (0..num_groups).rev() {
            let group = &grouped_services[group_index];
    
            for service in group {
                for (node, capacity) in nodes.iter_mut() {
                    if **capacity > 0 {
                        assignment_map.insert(service.clone(), (*node).clone());
                        **capacity -= 1;
                        break;
                    }
                }
            }
        }
    
        assignment_map
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
    
        let nr = (cpu / max_cpu) * self.config.get_weight("cpu") +
                 (memory / max_memory) * self.config.get_weight("memory") + 
                 (disk / max_disk) * self.config.get_weight("disk") +
                 (min_network / network) * self.config.get_weight("network");
        nr
    }
    
    fn compute_ne(&mut self, network: &Network, resource_map: &HashMap<Node, (Resource, Network)>) -> f64 {
        let mut max_bandwidth = 0.0;
        let mut min_latency = f64::MAX;
        let mut min_packet_loss = f64::MAX;
        let mut max_available = 0.0;
    
        for (_, (_, network)) in resource_map {
            if network.bandwidth > max_bandwidth {
                max_bandwidth = network.bandwidth;
            }
            if network.latency < min_latency {
                min_latency = network.latency;
            }
            if network.packet_loss < min_packet_loss {
                min_packet_loss = network.packet_loss;
            }
            if network.available > max_available {
                max_available = network.available;
            }
        }
    
        let bandwidth = network.bandwidth;
        let latency = network.latency;
        let packet_loss = network.packet_loss;
        let available = network.available as f64;

        // print all attribues
        println!("Bandwidth: {}, Latency: {}, Packet Loss: {}, Available: {}", bandwidth, latency, packet_loss, available);
        println!("Max Bandwidth: {}, Min Latency: {}, Min Packet Loss: {}, Max Available: {}", max_bandwidth, min_latency, min_packet_loss, max_available);
        println!("Weights: Bandwidth: {}, Latency: {}, Packet Loss: {}, Available: {}", self.config.get_weight("bandwidth"), self.config.get_weight("latency"), self.config.get_weight("packet_loss"), self.config.get_weight("available"));
    
        let ne = (bandwidth / max_bandwidth) * self.config.get_weight("bandwidth") +
                 (min_latency / latency) * self.config.get_weight("latency") +
                 (min_packet_loss / packet_loss) * self.config.get_weight("packet_loss") +
                 (available / max_available) * self.config.get_weight("available");
        ne
    }

    async fn get_service_resources(&self, services: &Vec<Service>, nodes: &Vec<Node>) -> HashMap<Service, Vec<Option<(Node, Resource)>>> {
        let mut service_resources: HashMap<Service, Vec<Option<(Node, Resource)>>> = HashMap::new();

        for service in services {
            // determine the node for the service
            for node in nodes {
                let service_node_util = self.api_client.get_node_service_utilization(&node.name, &service.name);
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

fn compute_euclidean_distance(coordinates: &[Coordinate], nr: f64, ne: f64) -> f64 {
    let (lowest_x, lowest_y) = get_lowest_coordinates(coordinates);

    let distance = ((nr - lowest_x).powi(2) + (ne - lowest_y).powi(2)).sqrt();
    distance
}

fn compute_proportion(distance: f64, distance_map: &HashMap<Node, f64>, j: u32) -> u32 {
    let total_distance: f64 = distance_map.values().sum();

    let proportion = (distance / total_distance) * j as f64;

    proportion.round() as u32
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
