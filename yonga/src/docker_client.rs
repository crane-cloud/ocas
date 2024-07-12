use std::process::Command;
use std::str;

pub struct DockerClient;

impl DockerClient {
    pub fn new() -> Self {
        DockerClient
    }

    pub fn run_command(&self, args: &[&str]) -> Result<std::process::Output, String> {
        let output = Command::new("docker")
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        
        if !output.status.success() {
            return Err(format!("Command failed with status: {}", output.status));
        }

        Ok(output)
    }

    pub fn list_containers(&self) -> Result<String, String> {
        let output = self.run_command(&["ps", "-a"])?;

        let stdout = str::from_utf8(&output.stdout)
            .map_err(|_| "Failed to parse output as UTF-8".to_string())?
            .to_string();

        Ok(stdout)
    }

    pub fn start_container(&self, container_id: &str) -> Result<String, String> {
        let output = self.run_command(&["start", container_id])?;

        let stdout = str::from_utf8(&output.stdout)
            .map_err(|_| "Failed to parse output as UTF-8".to_string())?
            .to_string();

        Ok(stdout)
    }

    pub fn stop_container(&self, container_id: &str) -> Result<String, String> {
        let output = self.run_command(&["stop", container_id])?;

        let stdout = str::from_utf8(&output.stdout)
            .map_err(|_| "Failed to parse output as UTF-8".to_string())?
            .to_string();

        Ok(stdout)
    }

    pub fn remove_container(&self, container_id: &str) -> Result<String, String> {
        let output = self.run_command(&["rm", container_id])?;

        let stdout = str::from_utf8(&output.stdout)
            .map_err(|_| "Failed to parse output as UTF-8".to_string())?
            .to_string();

        Ok(stdout)
    }   

    // docker stack deploy hotelreservation -c hotelreservation.yml
    pub fn deploy_stack(&self, stack_name: &str, compose_file: &str) -> Result<String, String> {
        let output = self.run_command(&["stack", "deploy", stack_name, "-c", compose_file])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }

    // docker list stack services
    pub fn list_stack_services(&self, stack_name: &str) -> Result<String, String> {
        let output = self.run_command(&["stack", "services", stack_name])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }

    // docker stack rm
    pub fn remove_stack(&self, stack_name: &str) -> Result<String, String> {
        let output = self.run_command(&["stack", "rm", stack_name])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }

    // docker service scale
    pub fn scale_service(&self, service_name: &str, replicas: &str) -> Result<String, String> {
        let output = self.run_command(&["service", "scale", service_name, "=", replicas])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }

    // add or update a placement constraint
    pub fn update_placement(&self, service_name: &str, constraint: &str) -> Result<String, String> {
        let output = self.run_command(&["service", "update", "--constraint-add", constraint, service_name])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }   

}
