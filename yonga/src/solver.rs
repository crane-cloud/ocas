use std::collections::{HashMap, HashSet};
use crate::utility::{Config, Node, Service};
use crate::api_client::ApiClient;
use crate::utility::{Network, Resource};

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
}

impl Solver {
    pub fn new(config: Config, api_client: ApiClient) -> Self {
        Solver {
            config,
            placement: None,
            api_client,
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
            // Retrieve the node utilization
            let node_utilization = self.api_client.get_node_utilization(&node.name).await?;

            // Retrieve the node environment
            let node_environment = self.api_client.get_node_environment(&node.name).await?;

            // Populate the resource map
            resource_map.insert(node.clone(), (node_utilization, node_environment));
        }

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

        Ok(placement_map)
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
    
        let ne = (bandwidth / max_bandwidth) * self.config.get_weight("bandwidth") +
                 (min_latency / latency) * self.config.get_weight("latency") +
                 (min_packet_loss / packet_loss) * self.config.get_weight("packet_loss") +
                 (available / max_available) * self.config.get_weight("available");
        ne
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
