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

    // get - number of services in a node
    pub async fn get_service_count(&self, node: &str) -> Result<usize, reqwest::Error> {
        let url = format!("{}/node/{}/services/count", self.base_url, node);
        let response = self.client.get(&url).send().await?;
        let service_count = response.json::<usize>().await?;
        Ok(service_count)
    }

    // get - services in a node
    pub async fn get_node_services(&self, node: &str) -> Result<Vec<String>, reqwest::Error> {
        let url = format!("{}/node/{}/services", self.base_url, node);
        let response = self.client.get(&url).send().await?;
        let services = response.json::<Vec<String>>().await?;
        Ok(services)
    }

    // get - node environment
    pub async fn get_node_environment(&self, node: &str) -> Result<Network, reqwest::Error> {
        let url = format!("{}/node/{}/environment", self.base_url, node);
        let response = self.client.get(&url).send().await?;
        let environment = response.json::<Network>().await?;
        // print environment for node
        println!("Network for node {}: {:?}", node, environment);
        Ok(environment)
    }

    // get - service utilization [total]
    pub async fn get_service_utilization(&self, service: &str) -> Result<Resource, reqwest::Error> {
        let url = format!("{}/service/{}/utilization", self.base_url, service);
        let response = self.client.get(&url).send().await?;
        let resource = response.json::<Resource>().await?;
        Ok(resource)
    }

    // get - node utilization
    pub async fn get_node_utilization(&self, node: &str) -> Result<Resource, reqwest::Error> {
        let url = format!("{}/node/{}/utilization", self.base_url, node);
        let response = self.client.get(&url).send().await?;
        let resource = response.json::<Resource>().await?;
        // print resource for node
        println!("Resource for node {}: {:?}", node, resource);
        Ok(resource)
    }

    // get - node service utilization
    pub async fn get_node_service_utilization(&self, node: &str, service: &str) -> Result<Resource, reqwest::Error> {
        let url = format!("{}/node/{}/service/{}/utilization", self.base_url, node, service);
        let response = self.client.get(&url).send().await?;
        let resource = response.json::<Resource>().await?;
        Ok(resource)
    }

}