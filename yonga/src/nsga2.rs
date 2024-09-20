// use std::path::PathBuf;
use std::collections::HashMap;
use std::error::Error;
// use log::LevelFilter;
// use std::fs;
// use clap::{Command, Arg, ArgAction};
// use rand::Rng;
// use rand::prelude::IteratorRandom;

// use optirustic::algorithms::{
//     Algorithm, MaxGeneration, NSGA2Arg, StoppingConditionType, NSGA2
// };
use optirustic::core::{BoundedNumber, Choice, Constraint, EvaluationResult, Evaluator, Individual, OError, Objective, ObjectiveDirection, Problem, RelationalOperator, VariableType, VariableValue
};

//use optirustic::operators::{PolynomialMutationArgs, SimulatedBinaryCrossoverArgs};

use crate::node::AggLinkEdge;
use crate::utility::{Node, Service, Config, Resource};


// Define the structure for the multi-objective problem
#[derive(Debug)]
pub struct MicroservicePlacementProblem {
    config: Config,
    service_comms: HashMap<(Service, Service), (u32, f64)>, // (number of messages, 99-% latency)
    node_comms: HashMap<Node, Vec<AggLinkEdge>>, // (node, (neighbour, link property))
    cost: HashMap<Node, f64>,
    utilization: HashMap<Service, Vec<Option<(Node, Resource)>>>, // Resource utilization per service
    node_resources: HashMap<Node, Resource>, // Available resources per node
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
    ) -> Result<Problem, OError> {
        let objectives = vec![
            Objective::new("communication_cost", ObjectiveDirection::Minimise),
            Objective::new("resource_cost", ObjectiveDirection::Minimise),
            //Objective::new("latency", ObjectiveDirection::Minimise),
            Objective::new("resource_imbalance", ObjectiveDirection::Minimise),
        ];


        //let choices: Vec<String> = config.cluster.nodes.iter().map(|node| node.name.clone()).collect();

        let services = config.services.clone();
        let nodes = config.cluster.nodes.clone();


        // Get the min/max ids of the nodes
        let min_node_id = nodes.iter().map(|node| node.id).min().unwrap();
        let max_node_id = nodes.iter().map(|node| node.id).max().unwrap();

        // print the min and max node ids
        println!("Min Node ID: {}, Max Node ID: {}", min_node_id, max_node_id);

        // Create the decision variables: service-to-node placements
        let variables: Vec<VariableType> = services.iter().map(|service| {
            VariableType::Integer(BoundedNumber::new(&service.name, min_node_id as i64, max_node_id as i64).unwrap())
        }).collect();

        // let variables: Vec<VariableType> = services.iter().map(|service| {
        //     VariableType::Choice(Choice::new(&service.name, choices.clone()))
        // }).collect();


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

        let e = Box::new(MicroservicePlacementProblem {
            config,
            service_comms,
            node_comms,
            cost,
            utilization,
            node_resources,
        });

        Problem::new(objectives, variables, None, e)
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

            // let variable_value = i.get_variable_value(&service.name)?; // Use service name as the key?

            // println!("Variable Value for service {:?}: {:?}", service.name, variable_value);

            // // Ensure we handle the `VariableValue` appropriately
            // match variable_value {
            //     // Assuming `VariableValue::Choice` contains the selected node's name
            //     VariableValue::Choice(node_name) => {
            //         if let Some(node) = self.config.cluster.nodes.iter().find(|n| n.name == *node_name) {
            //             placements.insert(service.clone(), node.clone());
            //         } else {
            //             return Err(format!("Node with name '{}' not found", node_name).into());
            //         }
            //     },
            //     // Handle other possible cases if necessary
            //     _ => {
            //         return Err("Unexpected variable value type".into());
            //     },
            // }

            let id_node = i.get_integer_value(&service.name)?; // Use service name as the key

            if let Some(node) = self.config.cluster.nodes.iter().find(|n| n.id == id_node) {
                placements.insert(service.clone(), node.clone());
            }
        }

        //println!("Placements before optimization: {:?}", placements);

        // Calculate each objective
        let mut objectives = HashMap::new();

        objectives.insert("resource_cost".to_string(), self.resource_cost(&placements));
        objectives.insert("communication_cost".to_string(), self.communication_cost(&placements));
        //objectives.insert("latency".to_string(), self.latency(&placements));
        objectives.insert("resource_imbalance".to_string(), self.resource_imbalance(&placements));


        let mut constraints: HashMap<String, f64> = HashMap::new();
        constraints.insert("frontend".to_string(), 1.0);


        Ok(EvaluationResult {
            constraints: Some(constraints),
            objectives,
        })
    }
}