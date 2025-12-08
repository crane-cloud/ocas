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
    node_comms: HashMap<String, Vec<AggLinkEdge>>, // (node, (neighbour, link property))
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
        node_comms: HashMap<String, Vec<AggLinkEdge>>, // (node, (neighbour, link property))
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

    pub fn resource_cost(&self, placements: &HashMap<Service, Node>) -> f64 {
        // 1. Sum the cost of each node used in placements
        let total_raw_cost: f64 = placements
            .values()
            .map(|node| *self.cost.get(node).unwrap_or(&0.0))
            .sum();

        let service_count = placements.len() as f64;

        let (_min_cost, max_cost) = self.minmax_node_cost;

        // 2. Maximum possible cost (all services placed on worst node)
        let max_possible = max_cost * service_count;

        // 3. Normalize
        let normalized = if max_possible > 0.0 {
            total_raw_cost / max_possible
        } else {
            0.0
        };

        let final_cost = normalized.clamp(0.0, 1.0);

        // Debug low-cost cases
        if final_cost < 0.1 {
            println!(
                "[resource_cost] Raw: {}, MaxPossible: {}, FinalNorm: {}",
                total_raw_cost, max_possible, final_cost
            );
            println!("Placements: {:?}", placements);
        }

        final_cost
    }

    // Calculate the communication cost
    // Normalize communication cost using aggregated node communications
    pub fn communication_cost(
        &self,
        placement: &HashMap<Service, Node>,
        service_comms: &HashMap<(Service, Service), (u32, f64)>,
        max_cost: f64,
    ) -> f64 {
        let mut total_cost = 0.0;

        for ((svc1, svc2), (message_count, _)) in service_comms {
            if let (Some(node1), Some(node2)) = (placement.get(svc1), placement.get(svc2)) {
                if node1.name == node2.name {
                    // Same node → no comms cost
                    continue;
                }

                // --- FIXED: use node names, not Node structs ---
                let src = node1.name.clone();
                let dst = node2.name.clone();

                let path_cost = self.node_comms
                    .get(&src)
                    .and_then(|edges| edges.iter().find(|edge| edge.destination == dst))
                    .map_or(f64::INFINITY, |edge| edge.edge);

                let comm_cost = (*message_count as f64) * path_cost;

                if comm_cost.is_finite() {
                    total_cost += comm_cost;
                }
            }
        }

        // Normalize
        let normalized_cost = if max_cost > 0.0 {
            total_cost / max_cost
        } else {
            0.0
        };

        // Clamp 0..1
        normalized_cost.clamp(0.0, 1.0)
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
    fn evaluate(&self, i: &OIndividual) 
        -> Result<OEvaluationResult, Box<dyn Error>> 
    {
        let mut placements: HashMap<Service, Node> = HashMap::new();

        // Decode service → node assignments
        for service in &self.config.services {
            let variable_value = i.get_variable_value(&service.name)?;

            match variable_value {
                OVariableValue::OChoice(node_id_u64) => {
                    let node = self.config.cluster.nodes
                        .iter()
                        .find(|n| n.id as u64 == *node_id_u64)
                        .ok_or_else(|| format!("Node id {} not found", node_id_u64))?;

                    placements.insert(service.clone(), node.clone());
                }
                _ => return Err("Unexpected variable value".into()),
            }
        }

        // === OBJECTIVES =======================================================
        let mut objectives = HashMap::new();

        objectives.insert(
            "resource_cost".into(),
            self.resource_cost(&placements)
        );

        objectives.insert(
            "communication_cost".into(),
            self.communication_cost(
                &placements,
                &self.service_comms,
                self.max_opt_cost
            )
        );

        objectives.insert(
            "resource_imbalance".into(),
            self.resource_imbalance(&placements)
        );

        // === CONSTRAINTS ======================================================
        let mut constraint_results: HashMap<
            String,
            (
                Option<u64>, 
                Option<Vec<HashMap<String, u64>>>, 
                Option<HashMap<u64, (f64,f64,f64,f64)>>
            )
        > = HashMap::new();

        if let Some(constraints) = &self.constraints {
            for constraint in constraints {
                let cname = constraint.name().to_string();

                if let Some(target_node_id) = constraint.target() {
                    // Direct equality constraint
                    let svc = self.config.services
                        .iter()
                        .find(|s| s.name == cname)
                        .ok_or("Constraint service not found")?;

                    let placement_node = placements.get(svc).unwrap();
                    constraint_results.insert(
                        cname.clone(),
                        (Some(placement_node.id as u64), None, None)
                    );

                } else if let Some(services) = constraint.services() {
                    // Group constraint
                    let mut group_vals = vec![];

                    for sname in services {
                        let svc = self.config.services
                            .iter()
                            .find(|s| s.name == *sname)
                            .unwrap();

                        let node = placements.get(svc).unwrap();

                        let mut m = HashMap::new();
                        m.insert(sname.to_string(), node.id as u64);
                        group_vals.push(m);
                    }

                    constraint_results.insert(
                        cname.clone(),
                        (None, Some(group_vals), None)
                    );

                } else if let Some(_) = constraint.resource() {
                    // Resource constraint
                    let mut rmap = HashMap::new();

                    for node in &self.config.cluster.nodes {
                        let mut node_usage = Resource::default();

                        for (svc, pnode) in &placements {
                            if pnode.id == node.id {
                                if let Some(util) = self.utilization.get(svc) {
                                    for u in util {
                                        if let Some((_n, res)) = u {
                                            node_usage.add(res);
                                        }
                                    }
                                }
                            }
                        }

                        rmap.insert(
                            node.id as u64,
                            (node_usage.cpu, node_usage.memory, node_usage.disk, node_usage.network)
                        );
                    }

                    constraint_results.insert(
                        cname.clone(),
                        (None, None, Some(rmap))
                    );
                }
            }
        }

        Ok(OEvaluationResult {
            objectives,
            constraints: Some(constraint_results),
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