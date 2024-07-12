use serde::{Deserialize, Serialize};
// use serde_yaml;
// use std::fs;
// use clap::{Arg, Command, ArgAction};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
pub struct StackConfig {
    pub version: String,
    pub services: HashMap<String, ServiceConfig>,
    pub volumes: HashMap<String, Volume>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Volume {}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
pub struct Deploy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replicas: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub restart_policy: Option<RestartPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Resources>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestartPolicy {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Placement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Logging {
    pub driver: String,
    pub options: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Resources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<Resource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reservations: Option<Resource>,
}

#[derive(Debug, Serialize, Deserialize)]
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

pub fn populate_volumes(config: &mut StackConfig) {
    let mut volumes_map = HashMap::new();

    for service in config.services.values() {
        if let Some(service_volumes) = &service.volumes {
            for volume in service_volumes {
                // Assuming volume name is the same as service name for simplicity
                if let Some(index) = volume.find(':') {
                    let volume_name = &volume[..index];
                    volumes_map.entry(volume_name.to_string()).or_insert(Volume {});
                }
            }
        }
    }

    // Assign the populated volumes map to StackConfig
    config.volumes = volumes_map;
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

//     // Modify the replicas attribute
//     for (_, service) in &mut config.services {
//         if let Some(deploy) = &mut service.deploy {
//             if let Some(replicas) = deploy.replicas {
//                 deploy.replicas = Some(replicas * 2);  // Double the replicas for demonstration
//             }
//         }
//     }

//     // Populate the volumes section if not already populated
//     populate_volumes(&mut config);

//     // Serialize the modified configuration back to YAML
//     let new_yaml_str = serde_yaml::to_string(&config)?;

//     // Set the new YAML path
//     let yaml_path = "docker-compose_update.yml";

//     // Write the modified YAML back to the file
//     fs::write(yaml_path, new_yaml_str)?;

//     println!("YAML config updated successfully.");
//     Ok(())
// }
