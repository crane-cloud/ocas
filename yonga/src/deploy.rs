use docker_client::DockerClient;

fn main() {
    let docker_client = DockerClient::new();

    // List containers
    match docker_client.list_containers() {
        Ok(output) => println!("Containers:\n{}", output),
        Err(e) => println!("Error listing containers: {}", e),
    }

    // Start a container
    let container_id = "your_container_id";
    match docker_client.start_container(container_id) {
        Ok(output) => println!("Started container:\n{}", output),
        Err(e) => println!("Error starting container: {}", e),
    }

    // Stop a container
    match docker_client.stop_container(container_id) {
        Ok(output) => println!("Stopped container:\n{}", output),
        Err(e) => println!("Error stopping container: {}", e),
    }

    // Remove a container
    match docker_client.remove_container(container_id) {
        Ok(output) => println!("Removed container:\n{}", output),
        Err(e) => println!("Error removing container: {}", e),
    }
}