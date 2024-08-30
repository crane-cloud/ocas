use reqwest::Client;
// use serde::Deserialize;

use crate::utility::{Network, Resource};

#[derive(Debug, Clone)]
pub struct ApiClient {
    client: Client,
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        }
    }

    // Helper function to handle GET requests and parse JSON responses
    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, reqwest::Error> {
        let response = self.client.get(url).send().await?;
        let json = response.json::<T>().await?;
        Ok(json)
    }

    // Get the number of services in a node
    pub async fn get_service_count(&self, node: &str) -> Result<usize, reqwest::Error> {
        let url = format!("{}/node/{}/services/count", self.base_url, node);
        self.get_json(&url).await
    }

    // Get the services in a node
    pub async fn get_node_services(&self, node: &str) -> Result<Vec<String>, reqwest::Error> {
        let url = format!("{}/node/{}/services", self.base_url, node);
        self.get_json(&url).await
    }

    // Get the network environment for a node
    pub async fn get_node_environment(&self, node: &str) -> Result<Network, reqwest::Error> {
        let url = format!("{}/node/{}/environment", self.base_url, node);
        let environment = self.get_json::<Network>(&url).await?;
        // Print the environment for the node
        println!("Network Environment for node {}: {:?}", node, environment);
        Ok(environment)
    }

    // Get the total utilization of a service
    pub async fn get_service_utilization(&self, service: &str) -> Result<Resource, reqwest::Error> {
        let url = format!("{}/service/{}/utilization", self.base_url, service);
        self.get_json(&url).await
    }

    // Get the utilization of a node
    pub async fn get_node_utilization(&self, node: &str) -> Result<Resource, reqwest::Error> {
        let url = format!("{}/node/{}/utilization", self.base_url, node);
        let resource = self.get_json::<Resource>(&url).await?;
        // Print the resource for the node
        println!("Resource for node {}: {:?}", node, resource);
        Ok(resource)
    }

    // Get the utilization of a specific service on a node
    pub async fn get_node_service_utilization(&self, node: &str, service: &str) -> Result<Resource, reqwest::Error> {
        let url = format!("{}/node/{}/service/{}/utilization", self.base_url, node, service);
        self.get_json(&url).await
    }
}