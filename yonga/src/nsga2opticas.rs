use std::collections::HashMap;
use std::error::Error;

use opticas::core::{
    OChoice, OConstraint, OEvaluationResult, OEvaluator, OIndividual, 
    OOError, OObjective, OObjectiveDirection, OProblem, OVariableType, OVariableValue
};

use crate::node::AggLinkEdge;
use crate::utility::{Node, Service, Config, Resource};


// Define the structure for the multi-objective problem
#[derive(Debug)]
pub struct OMicroservicePlacementProblem {
    config: Config,
    service_comms: HashMap<(Service, Service), (u32, f64)>, // (number of messages, 99-% latency)
    node_comms: HashMap<Node, Vec<AggLinkEdge>>, // (node, (neighbour, link property))
    cost: HashMap<Node, f64>,
    max_opt_cost: f64,
    minmax_node_cost: (f64, f64),
    minmax_resource_imbalance: (f64, f64),
    utilization: HashMap<Service, Vec<Option<(Node, Resource)>>>, // Resource utilization per service
    node_resources: HashMap<Node, Resource>, // Available resources per node
    constraints: Option<Vec<OConstraint>>,
}


impl OMicroservicePlacementProblem {
    // Create the problem with the three objectives
    pub fn create(
        config: Config,
        service_comms: HashMap<(Service, Service), (u32, f64)>, // (number of messages, 99-% latency)
        node_comms: HashMap<Node, Vec<AggLinkEdge>>, // (node, (neighbour, link property))
        cost: HashMap<Node, f64>,
        max_opt_cost: f64,
        minmax_node_cost: (f64, f64),
        minmax_resource_imbalance: (f64, f64),
        utilization: HashMap<Service, Vec<Option<(Node, Resource)>>>, // Resource utilization per service
        node_resources: HashMap<Node, Resource>, // Available resources per node
        constraints: Option<Vec<OConstraint>>,
    ) -> Result<OProblem, OOError> {
        let objectives = vec![
            OObjective::new("communication_cost", OObjectiveDirection::OMinimise),
            OObjective::new("resource_cost", OObjectiveDirection::OMinimise),
            OObjective::new("resource_imbalance", OObjectiveDirection::OMinimise),
        ];


        let choices: Vec<u64> = config.cluster.nodes.iter().map(|node| node.id.clone() as u64).collect();

        let services = config.services.clone();
        //let nodes = config.cluster.nodes.clone();

        let variables: Vec<OVariableType> = services.iter().map(|service| {
            OVariableType::OChoice(OChoice::new(&service.name, choices.clone()))
        }).collect();

        let e = Box::new(OMicroservicePlacementProblem {
            config,
            service_comms,
            node_comms,
            max_opt_cost,
            minmax_node_cost,
            minmax_resource_imbalance,
            cost,
            utilization,
            node_resources,
            constraints: constraints.clone(),
        });

        OProblem::new(objectives, variables, constraints, e)
    }

    // Calculate the resource cost
    pub fn resource_cost(&self, placements: &HashMap<Service, Node>) -> f64 {
        // Calculate the total cost based on placements
        let total_cost: f64 = placements
            .iter()
            .map(|(_, node)| *self.cost.get(node).unwrap_or(&0.0))
            .sum();

        // get the length of the placements
        let num_placements = placements.len() as f64;

        let (min_cost, max_cost) = self.minmax_node_cost;

        // Find the minimum and maximum resource cost
        // let max_cost = self.cost.values()
        //     .filter_map(|&val| if val.is_finite() { Some(val) } else { None })  // Ignore NaN or infinite values
        //     .max_by(|a, b| a.partial_cmp(b).unwrap())
        //     .unwrap_or(0.0);  // Default to 0.0 if all values are NaN/invalid

        // let min_cost = self.cost.values()
        //     .filter_map(|&val| if val.is_finite() { Some(val) } else { None })  // Ignore NaN or infinite values
        //     .min_by(|a, b| a.partial_cmp(b).unwrap())
        //     .unwrap_or(0.0);  // Default to 0.0 if all values are NaN/invalid   

        let max_resource_cost = max_cost * num_placements;
        let _min_resource_cost = min_cost * num_placements;
    
        // Normalize the total cost based on the maximum observed resource cost
        // let normalized_cost = if max_resource_cost > 0.0 {
        //     total_cost / max_resource_cost
        // } else {
        //     0.0 // If max_cost is 0 (which is unlikely but possible), set cost to 0
        // };

        // Clamp the normalized cost between 0 and 1
        //let final_cost = normalized_cost.max(0.0).min(1.0);

        // Print the total cost, normalized cost, and final cost (optional)
        // println!("Total cost: {}, Normalized cost: {}, Final cost: {}", total_cost, normalized_cost, final_cost);

        let final_cost = total_cost / max_resource_cost;

        // print the costs
        // println!("Total cost: {}, Max resource cost: {}, Final cost: {}", total_cost, max_resource_cost, final_cost);

        if final_cost < 0.1 {
            println!("Total cost: {}, Max resource cost: {}, Final cost: {}", total_cost, max_resource_cost, final_cost);
            println!("Placement: {:?}", placements);
        }

        final_cost
    }

    // Calculate the communication cost
    pub fn communication_cost(&self, max_cost: f64, placements: &HashMap<Service, Node>) -> f64 {
        let mut total_cost = 0.0;

        // get the length of the service comms
        let service_comms_len = self.service_comms.len();

        // get the max cost - multiply the max cost by the number of service comms
        let max_cost = max_cost * service_comms_len as f64;

        // Iterate over the service communication pairs
        for ((s1, s2), (message_count, _latency)) in &self.service_comms {
            if let (Some(node1), Some(node2)) = (placements.get(s1), placements.get(s2)) {
                if node1 == node2 {
                    // Services are on the same node, no communication cost
                    continue;
                } else {
                    // Services are on different nodes, get the path cost
                    let path_cost = self.node_comms
                        .get(node1)
                        .and_then(|edges| edges.iter().find(|edge| edge.destination == *node2))
                        .map_or(f64::INFINITY, |edge| edge.edge);

                    // Calculate the total communication cost for this pair
                    let comm_cost = *message_count as f64 * path_cost;
                    
                    // Check if the cost is finite before adding
                    if comm_cost.is_finite() {
                        
                        total_cost += comm_cost;

                    } else {
                        println!("Infinite cost encountered - Message count: {}, Path cost: {} from {} to {} ", message_count, path_cost, node1.name, node2.name);
                    }
                }
            }
        }

        // Normalize the total cost based on the maximum observed communication cost
        let normalized_cost = if max_cost > 0.0 {
            total_cost / max_cost
        } else {
            0.0 // If max_cost is 0 (which is unlikely but possible), set cost to 0
        };

        // Clamp the normalized cost between 0 and 1
        let final_cost = normalized_cost.max(0.0).min(1.0);

        // Print the total cost, normalized cost, and final cost (optional)
        // println!("Total cost: {}, Normalized cost: {}, Final cost: {}", total_cost, normalized_cost, final_cost);

        final_cost
    }

    // Consider remaining resources in the resource imbalance objective

    pub fn resource_imbalance(&self, placements: &HashMap<Service, Node>) -> f64 {
        let mut adjusted_utilization = self.node_resources.clone();

        // Adjust node utilization based on placements by adding service utilization
        for (service, node) in placements {
            if let Some(service_utilization) = self.utilization.get(service) {
                if let Some(resource) = service_utilization.iter()
                    .find_map(|util| if let Some((util_node, res)) = util { 
                        if util_node == node { Some(res) } else { None } 
                    } else { None })
                {
                    if let Some(node_resource) = adjusted_utilization.get_mut(node) {
                        // Add the service resource to node's utilization
                        node_resource.cpu += resource.cpu;
                        node_resource.memory += resource.memory;
                        node_resource.disk += resource.disk;
                        node_resource.network += resource.network;
                    }
                }
            }
        }

        // Calculate total utilization and average utilization across all nodes
        let mut total_resources = Resource::default();
        let node_count = adjusted_utilization.len() as f64;

        for resource in adjusted_utilization.values() {
            total_resources.add(resource);
        }

        let avg_resource = Resource {
            cpu: total_resources.cpu / node_count,
            memory: total_resources.memory / node_count,
            disk: total_resources.disk / node_count,
            network: total_resources.network / node_count,
        };

        // Calculate variance for each resource type
        let mut variance = Resource::default();
        for resource in adjusted_utilization.values() {
            variance.cpu += (resource.cpu - avg_resource.cpu).powi(2);
            variance.memory += (resource.memory - avg_resource.memory).powi(2);
            variance.disk += (resource.disk - avg_resource.disk).powi(2);
            variance.network += (resource.network - avg_resource.network).powi(2);
        }

        // Calculate overall imbalance as the mean of variances
        let total_imbalance = ((variance.cpu + variance.memory + variance.disk + variance.network).sqrt() / node_count) / 
        self.minmax_resource_imbalance.0;

        // Print the total imbalance (optional)
        // println!("Total imbalance: {}", total_imbalance);

        total_imbalance
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

    // A function to calculate the maximum resource imbalance 
    pub fn max_resource_imbalance(&self) -> f64 {
        let mut total_resources = Resource::default();
        let mut max_resources = Resource::default();
    
        for resource in self.node_resources.values() {
            total_resources.add(resource);
    
            if resource.cpu > max_resources.cpu {
                max_resources.cpu = resource.cpu;
            }
            if resource.memory > max_resources.memory {
                max_resources.memory = resource.memory;
            }
            if resource.disk > max_resources.disk {
                max_resources.disk = resource.disk;
            }
            if resource.network > max_resources.network {
                max_resources.network = resource.network;
            }
        }
    
        let avg_cpu = total_resources.cpu / self.node_resources.len() as f64;
        let avg_memory = total_resources.memory / self.node_resources.len() as f64;
        let avg_disk = total_resources.disk / self.node_resources.len() as f64;
        let avg_network = total_resources.network / self.node_resources.len() as f64;
    
        let mut variance = Resource::default();
    
        for resource in self.node_resources.values() {
            let diff = Resource {
                cpu: resource.cpu - avg_cpu,
                memory: resource.memory - avg_memory,
                disk: resource.disk - avg_disk,
                network: resource.network - avg_network,
            };
            // Sum of squared differences
            variance.cpu += diff.cpu * diff.cpu;
            variance.memory += diff.memory * diff.memory;
            variance.disk += diff.disk * diff.disk;
            variance.network += diff.network * diff.network;
        }
    
        // Compute imbalance as the mean of squared differences (variance)
        let imbalance = (variance.cpu + variance.memory + variance.disk + variance.network) / self.node_resources.len() as f64;
    
        imbalance
    }

    // A function to calculate the minimum resource imbalance
    pub fn min_resource_imbalance(&self) -> f64 {
        let mut total_resources = Resource::default();
        let mut min_resources = Resource::default();
    
        for resource in self.node_resources.values() {
            total_resources.add(resource);
    
            if resource.cpu < min_resources.cpu {
                min_resources.cpu = resource.cpu;
            }
            if resource.memory < min_resources.memory {
                min_resources.memory = resource.memory;
            }
            if resource.disk < min_resources.disk {
                min_resources.disk = resource.disk;
            }
            if resource.network < min_resources.network {
                min_resources.network = resource.network;
            }
        }
    
        let avg_cpu = total_resources.cpu / self.node_resources.len() as f64;
        let avg_memory = total_resources.memory / self.node_resources.len() as f64;
        let avg_disk = total_resources.disk / self.node_resources.len() as f64;
        let avg_network = total_resources.network / self.node_resources.len() as f64;
    
        let mut variance = Resource::default();
    
        for resource in self.node_resources.values() {
            let diff = Resource {
                cpu: resource.cpu - avg_cpu,
                memory: resource.memory - avg_memory,
                disk: resource.disk - avg_disk,
                network: resource.network - avg_network,
            };
            // Sum of squared differences
            variance.cpu += diff.cpu * diff.cpu;
            variance.memory += diff.memory * diff.memory;
            variance.disk += diff.disk * diff.disk;
            variance.network += diff.network * diff.network;
        }
    
        // Compute imbalance as the mean of squared differences (variance)
        let imbalance = (variance.cpu + variance.memory + variance.disk + variance.network) / self.node_resources.len() as f64;
    
        imbalance
    }

}

impl OEvaluator for OMicroservicePlacementProblem {
    fn evaluate(&self, i: &OIndividual) -> Result<OEvaluationResult, Box<dyn Error>> {
        let mut placements: HashMap<Service, Node> = HashMap::new();

        // Decode variables from the individual into service-to-node mapping
        for (_index, service) in self.config.services.iter().enumerate() {

            let variable_value = i.get_variable_value(&service.name)?; // Use service name as the key?

            // Ensure we handle the `VariableValue` appropriately
            match variable_value {
                // Assuming `VariableValue::Choice` contains the selected node's name
                OVariableValue::OChoice(node_name) => {
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
        objectives.insert("communication_cost".to_string(), self.communication_cost(self.max_opt_cost, &placements));
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

        Ok(OEvaluationResult {
            constraints: Some(constraints),
            objectives,
        })
    }
}

// a function that takes individuals, and the direction (max/min) and returns the best individual
pub fn opticas_get_best_individual(individuals: &Vec<OIndividual>, direction: OObjectiveDirection) -> (OIndividual, f64) {
    let mut best_individual = individuals[0].clone();

    for individual in individuals {

        let a = sum_objective_values(individual);
        let b = sum_objective_values(&best_individual);

        if direction == OObjectiveDirection::OMinimise {
            if a < b {
                best_individual = individual.clone();
            }
        } else if direction == OObjectiveDirection::OMaximise {
            if a > b {
                best_individual = individual.clone();
            }
        }
    }

    // print the objective values of the best individual
    println!("Best individual obj. values: {:?}", best_individual.get_objective_values().unwrap());

    // print the constraints of the best individual
    println!("Best individual obj. vars: {:?}", best_individual.get_variable_values().unwrap());

    (best_individual.clone(), sum_objective_values(&best_individual))
}

// a function that takes an individual and returns a sum of its objective values
pub fn sum_objective_values(individual: &OIndividual) -> f64 {
    let mut sum = 0.0;
    for value in individual.get_objective_values().unwrap() {
        sum += value;
    }
    sum
}