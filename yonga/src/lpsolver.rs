// use std::path::PathBuf;
use std::collections::HashMap;
use std::error::Error;
use log::LevelFilter;
// use serde::ser;
use std::fs;
use clap::{Command, Arg, ArgAction};
use rand::Rng;
use rand::prelude::IteratorRandom;

use optirustic::algorithms::{
    Algorithm, MaxGenerationValue, NSGA2Arg, StoppingConditionType, NSGA2
};
use optirustic::core::{Choice, Constraint, EvaluationResult, Evaluator, Individual, OError, Objective, ObjectiveDirection, Problem, RelationalOperator, VariableType, VariableValue
};

use optirustic::operators::{PolynomialMutationArgs, SimulatedBinaryCrossoverArgs};

use yonga::node::AggLinkEdge;
use yonga::utility::{Node, Service, Config, Resource, resource_diff};

// Defint the structure for a Service Group
#[derive(Debug, Clone)]
pub struct ServiceGroup(pub String, pub Vec<String>);


// Define the structure for the multi-objective problem
#[derive(Debug)]
pub struct MicroservicePlacementProblem {
    config: Config,
    service_comms: HashMap<(Service, Service), (u32, f64)>, // (number of messages, 99-% latency)
    node_comms: HashMap<Node, Vec<AggLinkEdge>>, // (node, (neighbour, link property))
    cost: HashMap<Node, f64>,
    utilization: HashMap<Service, Vec<Option<(Node, Resource)>>>, // Resource utilization per service
    node_resources: HashMap<Node, Resource>, // Available resources per node
    constraints: Option<Vec<Constraint>>,
}

impl MicroservicePlacementProblem {
    // Create the problem with the three objectives
    pub fn create(
        config: Config,
        service_comms: HashMap<(Service, Service), (u32, f64)>, // (number of messages, 99-% latency)
        node_comms: HashMap<Node, Vec<AggLinkEdge>>, // (node, (neighbour, link property))
        cost: HashMap<Node, f64>,
        utilization: HashMap<Service, Vec<Option<(Node, Resource)>>>, // Resource utilization per service
        node_resources: HashMap<Node, Resource>, // Available resources per node
        constraints: Option<Vec<Constraint>>,
    ) -> Result<Problem, OError> {
        let objectives = vec![
            Objective::new("communication_cost", ObjectiveDirection::Minimise),
            Objective::new("resource_cost", ObjectiveDirection::Minimise),
            //Objective::new("latency", ObjectiveDirection::Minimise),
            Objective::new("resource_imbalance", ObjectiveDirection::Minimise),
        ];


        let choices: Vec<u64> = config.cluster.nodes.iter().map(|node| node.id.clone() as u64).collect();

        let services = config.services.clone();
        let nodes = config.cluster.nodes.clone();


        // // Get the min/max ids of the nodes
        // let min_node_id = nodes.iter().map(|node| node.id).min().unwrap();
        // let max_node_id = nodes.iter().map(|node| node.id).max().unwrap();

        // // print the min and max node ids
        // println!("Min Node ID: {}, Max Node ID: {}", min_node_id, max_node_id);

        // Create the decision variables: service-to-node placements
        // let variables: Vec<VariableType> = services.iter().map(|service| {
        //     VariableType::Integer(BoundedNumber::new(&service.name, min_node_id as u64, max_node_id as u64).unwrap())
        // }).collect();

        // let constraints: Vec<Constraint> = vec![Constraint::new(
        //     "frontend",
        //     RelationalOperator::EqualTo,
        //     4,
        // )];

        let variables: Vec<VariableType> = services.iter().map(|service| {
            VariableType::Choice(Choice::new(&service.name, choices.clone()))
        }).collect();


        //let constraints = None; // Add constraints if any

        // let constraints: Vec<Constraint> = vec![Constraint::new(
        //     "resource_imbalance",
        //     RelationalOperator::LessOrEqualTo,
        //     max_node_id as f64,
        // )];

        // create constraints for each service
        // let _constraint_min: Vec<Constraint> = services.iter().map(|service| {
        //     Constraint::new(
        //         &service.name,
        //         RelationalOperator::GreaterOrEqualTo,
        //         min_node_id as f64,
        //     )
        // }).collect();

        // let constraint_max: Vec<Constraint> = services.iter().map(|service| {
        //     Constraint::new(
        //         &service.name,
        //         RelationalOperator::LessOrEqualTo,
        //         max_node_id as f64,
        //     )
        // }).collect();

        //let constraints: Vec<Constraint> = Some(constraint_min.iter().chain(constraint_max.iter()).cloned().collect());



        // let constraints = {
        //     let mut constraints = HashMap::new();
        //     for service in &services {
        //         constraints.insert(service.name.clone(), (min_node_id, max_node_id));
        //     }
        //     Some(constraints)
        // };

        //let constraints = constraints.clone();

        let e = Box::new(MicroservicePlacementProblem {
            config,
            service_comms,
            node_comms,
            cost,
            utilization,
            node_resources,
            constraints: constraints.clone(),
        });

        Problem::new(objectives, variables, constraints, e)
    }

    // Create the objective functions
    pub fn resource_cost(&self, placements: &HashMap<Service, Node>) -> f64 {
        placements.iter().map(|(_, node)| {
            *self.cost.get(node).unwrap_or(&0.0)
        }).sum()
    }

    // Calculate the communication cost based on service-to-service communication and node-to-node paths
    pub fn communication_cost(&self, placements: &HashMap<Service, Node>) -> f64 {
        let mut total_cost = 0.0;
        for ((s1, s2), (message_count, _latency)) in &self.service_comms {
            if let Some(node1) = placements.get(s1) {
                if let Some(node2) = placements.get(s2) {
                    if node1 != node2 {
                        // Get the path quality for the path from node1 to node2
                        let path_quality = self.node_comms
                            .get(node1)
                            .and_then(|edges| edges.iter().find(|edge| edge.destination == *node2))
                            .map_or(1.0, |edge| edge.edge); // Default to path quality 1.0x if not found!!!!!
                        total_cost += *message_count as f64 * path_quality;
                    }
                }
            }
        }
        total_cost
    }

    // Consider remaining resources in the resource imbalance objective
    pub fn resource_imbalance(&self, placements: &HashMap<Service, Node>) -> f64 {
        let mut available_resources: HashMap<Node, Resource> = self.node_resources.clone();
    
        for (service, node) in placements {
            if let Some(service_utilization) = self.utilization.get(service) {
                let mut total_util = Resource::default();
                for util in service_utilization {
                    if let Some((nodex, resource)) = util {
                        total_util.add(&resource);
    
                        if let Some(nodex_resource) = available_resources.get_mut(&nodex) {
                            nodex_resource.add(&resource);
                        } else {
                            eprintln!("Node {:?} not found in available_resources", nodex);
                        }
                    }
                }
    
                if let Some(node_resource) = available_resources.get_mut(node) {
                    node_resource.sub(&total_util);
                } else {
                    eprintln!("Node {:?} not found in available_resources", node);
                }
            }
        }
    
        let mut resource_sums = Resource::default();
        let resource_counts = available_resources.len() as f64;
    
        for resource in available_resources.values() {
            resource_sums.add(resource);
        }
    
        let avg_cpu = resource_sums.cpu / resource_counts;
        let avg_memory = resource_sums.memory / resource_counts;
        let avg_disk = resource_sums.disk / resource_counts;
        let avg_network = resource_sums.network / resource_counts;
    
        let mut variance = Resource::default();
    
        for resource in available_resources.values() {
            let diff = Resource {
                cpu: resource.cpu - avg_cpu,
                memory: resource.memory - avg_memory,
                disk: resource.disk - avg_disk,
                network: resource.network - avg_network,
            };
            variance.add(&diff);
        }
    
        let imbalance = (variance.cpu + variance.memory + variance.disk + variance.network) / resource_counts;
    
        imbalance
    }
    

    pub fn assign_service_to_node(&mut self, service: &Service, node: &Node) {
        if let Some(service_util) = self.utilization.get(service){
            let mut total_util = Resource::default();
            let service_util = service_util.clone();
            for util in service_util {
                if let Some((node, resource)) = util {
                    total_util.add(&resource);
    
                    if let Some(node_resource) = self.node_resources.get_mut(&node) {
                        node_resource.sub(&resource);
                    } else {
                        eprintln!("Node {:?} not found in node_resources", node);
                    }
                }
            }
    
            if let Some(node_resource) = self.node_resources.get_mut(node) {
                node_resource.add(&total_util);
            } else {
                eprintln!("Node {:?} not found in node_resources", node);
            }
        }
    }

    // pub fn latency(&self, placements: &HashMap<Service, Node>) -> f64 {
    //     let mut total_latency = 0.0;
    //     for (s1, n1) in placements {
    //         for (s2, n2) in placements {
    //             if s1 != s2 && n1 != n2 {
    //                 total_latency += *self.latency.get(&(s1.clone(), s2.clone())).unwrap_or(&0.0);
    //             }
    //         }
    //     }
    //     total_latency
    // }

}

impl Evaluator for MicroservicePlacementProblem {
    fn evaluate(&self, i: &Individual) -> Result<EvaluationResult, Box<dyn Error>> {
        let mut placements: HashMap<Service, Node> = HashMap::new();

        // Decode variables from the individual into service-to-node mapping
        for (_index, service) in self.config.services.iter().enumerate() {

            let variable_value = i.get_variable_value(&service.name)?; // Use service name as the key?

            //println!("Variable Value for service {:?}: {:?}", service.name, variable_value);

            // Ensure we handle the `VariableValue` appropriately
            match variable_value {
                // Assuming `VariableValue::Choice` contains the selected node's name
                VariableValue::Choice(node_name) => {
                    if let Some(node) = self.config.cluster.nodes.iter().find(|n| n.id as u64 == *node_name) {
                        placements.insert(service.clone(), node.clone());
                    } else {
                        return Err(format!("Node with name '{}' not found", node_name).into());
                    }
                },
                // Handle other possible cases if necessary
                _ => {
                    return Err("Unexpected variable value type".into());
                },
            }

            // let id_node = i.get_integer_value(&service.name)?; // Use service name as the key

            // if let Some(node) = self.config.cluster.nodes.iter().find(|n| n.id as u64 == id_node) {
            //     placements.insert(service.clone(), node.clone());
            // }
        }

        // print the number of placements before optimization
        // println!("Number of placements before optimization: {:?}", placements.len());
        //println!("Placements before optimization: {:?}", placements);

        // Calculate each objective
        let mut objectives = HashMap::new();

        objectives.insert("resource_cost".to_string(), self.resource_cost(&placements));
        objectives.insert("communication_cost".to_string(), self.communication_cost(&placements));
        //objectives.insert("latency".to_string(), self.latency(&placements));
        objectives.insert("resource_imbalance".to_string(), self.resource_imbalance(&placements));

        // get the node id in a placement for a service identified by the name frontend
        // let frontend = placements.get(&self.config.services.iter().find(|s| s.name == "frontend").unwrap()).unwrap().id as u64;

        //let mut constraints: HashMap<String, u64> = HashMap::new();
        // constraints.insert("frontend".to_string(), frontend);
        let mut constraints: HashMap<String, (Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>)> = HashMap::new();

        for constraint in &self.constraints.clone().unwrap() {
            let name = constraint.name();
            if let Some(_value) = constraint.target() {
                let v = placements.get(&self.config.services.iter().find(|s| s.name == name).unwrap()).unwrap().id as u64;
                constraints.insert(name.to_string(), (Some(v), None, None));
            } else if let Some(services) = constraint.services() {
                let services:Vec<HashMap<String, u64>> = services.iter().map(|service| {
                    let v = placements.get(&self.config.services.iter().find(|s| s.name == service.clone()).unwrap()).unwrap().id as u64;
                    let mut map = HashMap::new();
                    map.insert(service.to_string(), v);
                    map
                }).collect();
                constraints.insert(name.to_string(), (None, Some(services.clone()), None));
            } else if let Some(_resource) = constraint.resource() {
                for node in &self.config.cluster.nodes {
                    let mut resource_constraint: HashMap<u64, (f64, f64, f64, f64)> = HashMap::new();
                    // initialize the resource requests for each node
                    let mut r = Resource::default();
                    // determine resource requests for each service placed on this node in placements
                    for (service, _) in &placements {
                        if let Some(service_util) = self.utilization.get(service){
                            let service_util = service_util.clone();
                            for util in service_util {
                                if let Some((nodex, resource)) = util {
                                    if node.clone() == nodex {
                                        r.add(&resource);
                                    }
                                }
                            }
                        }
                    }
                    resource_constraint.insert(node.id as u64, (r.cpu, r.memory, r.disk, r.network));
                    // add the constraint
                    constraints.insert(node.name.clone(), (None, None, Some(resource_constraint)));
                }
            }
        }

        Ok(EvaluationResult {
            constraints: Some(constraints),
            objectives,
        })
    }
}


pub fn generate_service_comms(config: &Config) -> HashMap<(Service, Service), (u32, f64)> {
    let mut service_comms: HashMap<(Service, Service), (u32, f64)> = HashMap::new();
    for service in &config.services {
        for other_service in &config.services {
            if service != other_service {
                let message_count = rand::random::<u32>() % 1000;
                let latency = rand::random::<f64>() * 100.0;
                service_comms.insert((service.clone(), other_service.clone()), (message_count, latency));
            }
        }
    }
    service_comms
}

pub fn generate_node_comms(config: &Config) -> HashMap<Node, Vec<AggLinkEdge>> {
    let mut node_comms: HashMap<Node, Vec<AggLinkEdge>> = HashMap::new();
    for node in &config.cluster.nodes {
        let mut neighbours = vec![];
        for other_node in &config.cluster.nodes {
            if node != other_node {
                let link = AggLinkEdge {
                    destination: other_node.clone(),
                    edge: rand::random::<f64>() * 100.0,
                };
                neighbours.push(link);
            }
        }
        node_comms.insert(node.clone(), neighbours);
    }
    node_comms
}

pub fn generate_node_costs(config: &Config) -> HashMap<Node, f64> {
    let mut node_costs: HashMap<Node, f64> = HashMap::new();
    for node in &config.cluster.nodes {
        let cost = rand::random::<f64>() * 100.0;
        node_costs.insert(node.clone(), cost);
    }
    node_costs
}

pub fn generate_service_resources(config: &Config) -> HashMap<Service, Vec<Option<(Node, Resource)>>> {
    let mut service_resources: HashMap<Service, Vec<Option<(Node, Resource)>>> = HashMap::new();
    let mut rng = rand::thread_rng();

    for service in &config.services {
        let mut resources = Vec::new();

        // Generate random assignments for each service
        for _ in 0..2 { // Adjust the number of assignments per service as needed
            // Safely get a random node from the cluster
            let node = config.cluster.nodes.iter().choose(&mut rng);

            // Optionally generate a resource if a node is selected
            let resource_option = node.map(|node| {
                let resource = Resource {
                    cpu: rng.gen_range(0.0..=0.2),
                    memory: rng.gen_range(0.0..=50.0),
                    disk: rng.gen_range(0.0..=10.0),
                    network: rng.gen_range(0.0..=25.0),
                };
                (node.clone(), resource)
            });

            resources.push(resource_option);
        }

        service_resources.insert(service.clone(), resources);
    }

    service_resources
}

pub fn generate_node_resources(config: &Config) -> HashMap<Node, Resource> {
    let mut node_resources: HashMap<Node, Resource> = HashMap::new();
    for node in &config.cluster.nodes {
        let resource = Resource {
            cpu: rand::random::<f64>() % 100.0,
            memory: rand::random::<f64>() % 100.0,
            disk: rand::random::<f64>() % 100.0,
            network: rand::random::<f64>() % 100.0,
        };
        node_resources.insert(node.clone(), resource);
    }
    node_resources
}

// pub fn generate_service_constraints(service: &Service, nodes: &Vec<Node>) -> Vec<Constraint> {
//     let mut constraints = vec![];
//     for node in nodes {
//         constraints.push(Constraint::new(
//             &service.name,
//             RelationalOperator::EqualTo,
//             node.id as u64,
//         ));
//     }
//     constraints
// }

fn main() -> Result<(), Box<dyn Error>> {
    // Add log
    env_logger::builder().filter_level(LevelFilter::Info).init();

    let matches = Command::new("MILP_Solver")
    .arg(Arg::new("config")
        .long("config")
        .short('c')
        .required(true)
        .action(ArgAction::Set))
    .get_matches();

    let config = matches.get_one::<String>("config").unwrap();    
    
    // parse the config file
    let config_str = fs::read_to_string(config).expect("Failed to read configuration file");
    let config: Config = serde_yaml::from_str(&config_str).expect("Failed to parse configuration file");

    //let nodes = config.cluster.nodes.clone();

    // get service groups
    let service_groups = config.grouped_services();

    // Add sample random service_comms, node_comms, node_costs, service_resources, node_resources | usually dynamic data
    let service_comms = generate_service_comms(&config);
    let node_comms = generate_node_comms(&config);
    let node_costs = generate_node_costs(&config);
    let service_resources = generate_service_resources(&config);
    let node_resources = generate_node_resources(&config);

    // create a map with available resources per node
    let mut available_resources: HashMap<Node, Resource> = HashMap::new();
    for (node, resource) in &node_resources {
        let resource_int = config.cluster.nodes.iter().find(|n| n.name == node.name).unwrap().resource.clone();
        let available = resource_diff(resource_int, resource.clone());
        available_resources.insert(node.clone(), available);
    }

    let mut constraints = vec![];

    for group in &service_groups {
        let len = group.1.len() as u64;
        if len > 1 {
            // create a name for the group
            let group_name = format!("{}_group", group.0);
            let services = Some(group.1.iter().map(|service| service.name.clone()).collect());
            let _constraint = Constraint::new(&group_name, RelationalOperator::EqualTo, None, services, None);
            //constraints.push(constraint);
        }
    }

    let frontend_constraint = Constraint::new("frontend", RelationalOperator::EqualTo, Some(1), None, None);
    let profile_constraint = Constraint::new("profile", RelationalOperator::EqualTo, Some(1), None, None);

    constraints.push(frontend_constraint);
    constraints.push(profile_constraint);

    // create node resource constraints
    for node in &available_resources {
        let (cpu, memory, disk, network) = (node.1.cpu, node.1.memory, node.1.disk, node.1.network);
        let mut resource_constraint: HashMap<u64, (f64, f64, f64, f64)> = HashMap::new();   
        resource_constraint.insert(node.0.id as u64, (cpu, memory, disk, network));
        let constraint = Constraint::new(&node.0.name, RelationalOperator::LessOrEqualTo, None, None, Some(resource_constraint));
        constraints.push(constraint);
    }

    // print the constraints
    for constraint in &constraints {
        println!("Constraints: {:?}", constraint);
    }

    // generate some constraints - place the frontend on node 1, and users on node 3
    // let constraints = vec![
    //     Constraint::new("frontend", RelationalOperator::EqualTo, 1),
    //     Constraint::new("user", RelationalOperator::EqualTo, 3),
    // ];


    // print the random data
    // println!("Service Comms: {:?}", service_comms);
    // println!("Node Comms: {:?}", node_comms);
    // println!("Node Costs: {:?}", node_costs);
    // println!("Service Resources: {:?}", service_resources);
    // println!("Node Resources: {:?}", node_resources);

    println!("Available Resources: {:?}", available_resources);

    // Create the problem
    let problem = MicroservicePlacementProblem::create(
        config.clone(),
        service_comms,
        node_comms,
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
        variable_probability: 1.0,
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

    for individual in algo.get_results().individuals {
        let result = individual.serialise();

        // print the result
        // println!("Results: {:?}", result);

        if result.evaluated && result.is_feasible {
            // print the objective values
            println!("Objectives: {:?}", result.objective_values);

            // print the variable values
            println!("Variables: {:?}\n", result.variable_values);

        
        }
    }

    // print the result
    //println!("Results: {:?}", results);

    // Export serialised results at last generation
    // algo.save_to_json(&PathBuf::from("."), Some("MicroservicePlacement"))?;

    Ok(())
}