use std::collections::HashMap;
use std::error::Error;

use optirustic::core::{
    Choice, Constraint, EvaluationResult, Evaluator, Individual, 
    OError, Objective, ObjectiveDirection, Problem, VariableType, VariableValue
};

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
            Objective::new("resource_imbalance", ObjectiveDirection::Minimise),
        ];


        let choices: Vec<u64> = config.cluster.nodes.iter().map(|node| node.id.clone() as u64).collect();

        let services = config.services.clone();
        //let nodes = config.cluster.nodes.clone();

        let variables: Vec<VariableType> = services.iter().map(|service| {
            VariableType::Choice(Choice::new(&service.name, choices.clone()))
        }).collect();

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

}

impl Evaluator for MicroservicePlacementProblem {
    fn evaluate(&self, i: &Individual) -> Result<EvaluationResult, Box<dyn Error>> {
        let mut placements: HashMap<Service, Node> = HashMap::new();

        // Decode variables from the individual into service-to-node mapping
        for (_index, service) in self.config.services.iter().enumerate() {

            let variable_value = i.get_variable_value(&service.name)?; // Use service name as the key?

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

        }

        // Calculate each objective
        let mut objectives = HashMap::new();

        objectives.insert("resource_cost".to_string(), self.resource_cost(&placements));
        objectives.insert("communication_cost".to_string(), self.communication_cost(&placements));
        //objectives.insert("latency".to_string(), self.latency(&placements));
        objectives.insert("resource_imbalance".to_string(), self.resource_imbalance(&placements));

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

// a function that takes individuals, and the direction (max/min) and returns the best individual
pub fn get_best_individual(individuals: &Vec<Individual>, direction: ObjectiveDirection) -> (Individual, f64) {
    let mut best_individual = individuals[0].clone();

    for individual in individuals {

        let a = sum_objective_values(individual);
        let b = sum_objective_values(&best_individual);

        if direction == ObjectiveDirection::Minimise {
            if a < b {
                best_individual = individual.clone();
            }
        } else if direction == ObjectiveDirection::Maximise {
            if a > b {
                best_individual = individual.clone();
            }
        }
    }

    (best_individual.clone(), sum_objective_values(&best_individual))
}

// a function that takes an individual and returns a sum of its objective values
pub fn sum_objective_values(individual: &Individual) -> f64 {
    let mut sum = 0.0;
    for value in individual.get_objective_values().unwrap() {
        sum += value;
    }
    sum
}