use crate::api_client::ApiClient;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    let api_client = ApiClient::new("http://127.0.0.1:30000");

    loop {
        let node_utilization = api_client.get_node_utilization("node1").await.unwrap();
        println!("Node utilization: {:?}", node_utilization);

        let service_utilization = api_client.get_service_utilization("service1").await.unwrap();
        println!("Service utilization: {:?}", service_utilization);

        sleep(Duration::from_secs(5)).await;
    }
}