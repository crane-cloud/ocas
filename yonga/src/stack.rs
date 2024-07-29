use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use crate::utility::{Node, Service};
// use clap::{Command, ArgAction, Arg};
// use std::fs;


// #[derive(Debug, Serialize, Deserialize, Clone, Eq, Hash, PartialEq)]
// pub struct Service {
//     pub id: String,
//     pub name: String,
//     pub cache: Option<String>,
//     pub db: Option<String>,
// }


// #[derive(Debug, Deserialize, Serialize, Clone, Eq, PartialEq, Hash)]
// pub struct Node {
//     pub id: String,
//     pub name: String,
//     pub ip: String,
// }



#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StackConfig {
    pub version: String,
    pub services: HashMap<String, ServiceConfig>,
    pub volumes: HashMap<String, Volume>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Volume {}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServiceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ports: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deploy: Option<Deploy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    environment: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<Logging>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volumes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Deploy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,
    pub placement: Option<Placement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RestartPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Placement {
    pub constraints: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Logging {
    pub driver: String,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Resources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Resource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservations: Option<Resource>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Resource {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk: Option<String>,
}

/// Populates the volumes map in the StackConfig with volumes used by services, excluding the "jaeger" service.
pub fn populate_volumes(config: &mut StackConfig) {
    let mut volumes_map = HashMap::new();

    for (service_name, service_config) in &config.services {
        if service_name == "jaeger" {
            println!("Skipping the \"jaeger\" service");
            continue; // Skip the "jaeger" service
        }

        if let Some(service_volumes) = &service_config.volumes {
            for volume in service_volumes {
                if let Some(index) = volume.find(':') {
                    let volume_name = &volume[..index];
                    volumes_map.entry(volume_name.to_string()).or_insert_with(|| {
                        println!("Adding volume {} for service {}", volume_name, service_name);
                        Volume {}
                    });
                }
            }
        }
    }

    config.volumes = volumes_map;
}

pub fn update_replicas(config: &mut StackConfig, service: Service, replicas: u32) {
    if let Some(service_config) = config.services.get_mut(&service.name) {
        if let Some(deploy) = &mut service_config.deploy {
            deploy.replicas = Some(replicas);
        }
    }

}

pub fn update_node_constraints(config: &mut StackConfig, placement_map: HashMap<Service, Option<HashSet<Node>>>) {
    for (service, nodes) in placement_map {
        if let Some(nodes) = nodes {
            if let Some(service_config) = config.services.get_mut(&service.name) {
                if let Some(deploy) = &mut service_config.deploy {
                    if let Some(placement) = &mut deploy.placement {
                        if let Some(constraints) = &mut placement.constraints {
                            for node in nodes {
                                constraints.push(format!("node.labels.name == {}", node.name));
                                //[node.role == manager]
                            }
                        } else {
                            placement.constraints = Some(
                                nodes.into_iter()
                                    .map(|node| format!("node.labels.name == {}", node.name))
                                    .collect(),
                            );
                        }
                    } else {
                        deploy.placement = Some(Placement {
                            constraints: Some(
                                nodes.into_iter()
                                    .map(|node| format!("node.labels.name == {}", node.name))
                                    .collect(),
                            ),
                        });
                    }
                }
            }
        }
    }
}

pub fn delete_null_placement(config: &mut StackConfig) {
    // print the number of services
    println!("Number of services: {}", config.services.len());
    for service in config.services.values_mut() {
        if let Some(deploy) = &mut service.deploy {
            if let Some(placement) = &deploy.placement {
                // print the placement constraints
                // println!("Placement constraints for a service X: {:?}", placement.constraints);
                if placement.constraints.is_none() {
                    //println!("Deleting null placement constraints for a service X");
                    deploy.placement = Some(Placement {
                        constraints: Some(vec![]), // Assuming constraints is a Vec<String>
                    });
                }
            }

            else {
                // println!("No placement constraints for a service X");
                deploy.placement = Some(Placement {
                    constraints: Some(vec![]), // Assuming constraints is a Vec<String>
                });
            }
        }
    }
}

// fn main() -> Result<(), Box<dyn std::error::Error>> {
//     let matches = Command::new("YongaStack")
//         .arg(Arg::new("config")
//             .long("config")
//             .short('c')
//             .required(true)
//             .action(ArgAction::Set))
//         .get_matches();

//     let yaml_config = matches.get_one::<String>("config").unwrap();
    
//     // Read the config file
//     let yaml_str = fs::read_to_string(yaml_config).expect("Failed to read the YAML configuration file");

//     // Parse the YAML file
//     let mut config: StackConfig = serde_yaml::from_str(&yaml_str)?;

//     let placement_map: HashMap<Service, Option<HashSet<Node>>> = {
//         let mut map = HashMap::new();
//         map.insert(Service { id: "13".to_string(), name: "mongodb-profile".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                    Some([Node { id: "5".to_string(), name: "cr-bun".to_string(), ip: "196.43.171.248".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "10".to_string(), name: "memcached-profile".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                      Some([Node { id: "5".to_string(), name: "cr-bun".to_string(), ip: "196.43.171.248".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "4".to_string(), name: "geo".to_string(), cache: Some("".to_string()), db: Some("mongodb-geo".to_string()) },
//                         Some([Node { id: "3".to_string(), name: "cr-kla".to_string(), ip: "102.134.147.244".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "12".to_string(), name: "mongodb-geo".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "3".to_string(), name: "cr-kla".to_string(), ip: "102.134.147.244".to_string() }].iter().cloned().collect())); 
//         map.insert(Service { id: "8".to_string(), name: "reservation".to_string(), cache: Some("memcached-reserve".to_string()), db: Some("mongodb-reservation".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "15".to_string(), name: "mongodb-recommendation".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "5".to_string(), name: "rate".to_string(), cache: Some("memcached-rate".to_string()), db: Some("mongodb-rate".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "7".to_string(), name: "user".to_string(), cache: Some("".to_string()), db: Some("mongodb-user".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "11".to_string(), name: "memcached-reserve".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "2".to_string(), name: "profile".to_string(), cache: Some("memcached-profile".to_string()), db: Some("mongodb-profile".to_string()) },
//                         Some([Node { id: "5".to_string(), name: "cr-bun".to_string(), ip: "196.43.171.248".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "17".to_string(), name: "mongodb-user".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "16".to_string(), name: "mongodb-reservation".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "14".to_string(), name: "mongodb-rate".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "6".to_string(), name: "recommendation".to_string(), cache: Some("".to_string()), db: Some("mongodb-recommendation".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "3".to_string(), name: "search".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "3".to_string(), name: "cr-kla".to_string(), ip: "102.134.147.244".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "9".to_string(), name: "memcached-rate".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "1".to_string(), name: "cr-jhb".to_string(), ip: "129.232.230.130".to_string() }].iter().cloned().collect()));
//         map.insert(Service { id: "1".to_string(), name: "frontend".to_string(), cache: Some("".to_string()), db: Some("".to_string()) },
//                         Some([Node { id: "4".to_string(), name: "cr-lsk".to_string(), ip: "196.32.215.213".to_string() }].iter().cloned().collect()));
//         // Add more mappings as needed
//         map
//     };

//     // Modify the replicas attribute
//     for (_, service) in &mut config.services {
//         if let Some(deploy) = &mut service.deploy {
//             if let Some(replicas) = deploy.replicas {
//                 deploy.replicas = Some(replicas * 1);  // Double the replicas for demonstration
//             }
//         }
//     }

//     // Populate the volumes section if not already populated
//     populate_volumes(&mut config);

//     // Update the node constraints;
//     update_node_constraints(&mut config, placement_map);

//     // Clean up the null
//     delete_null_placement(&mut config);

//     // Serialize the modified configuration back to YAML
//     let new_yaml_str = serde_yaml::to_string(&config)?;

//     // Set the new YAML path
//     let yaml_path = "docker-compose_updatex.yml";

//     // Write the modified YAML back to the file
//     fs::write(yaml_path, new_yaml_str)?;

//     println!("YAML config updated successfully.");
//     Ok(())
// }