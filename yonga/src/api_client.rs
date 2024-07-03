use reqwest::Client;
// use serde::Deserialize;

use crate::utility::Resource;

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
    // pub async fn get_node_services(&self, node: &str) -> Result<usize, reqwest::Error> {
    //     let url = format!("{}/node/{}/services/count", self.base_url, node);
    //     let response = self.client.get(&url).send().await?;
    //     let services = response.json::<Vec<String>>().await?;
    //     Ok(services.len())
    // }

    pub async fn get_node_utilization(&self, node: &str) -> Result<Resource, reqwest::Error> {
        let url = format!("{}/node/{}/utilization", self.base_url, node);
        let response = self.client.get(&url).send().await?;
        let resource = response.json::<Resource>().await?;
        Ok(resource)
    }

    pub async fn get_service_utilization(&self, service: &str) -> Result<Resource, reqwest::Error> {
        let url = format!("{}/service/{}/utilization", self.base_url, service);
        let response = self.client.get(&url).send().await?;
        let resource = response.json::<Resource>().await?;
        Ok(resource)
    }
}