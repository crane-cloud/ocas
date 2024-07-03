// src/docker_client.rs

use std::process::{Command, Output};
use std::str;

pub struct DockerClient;

impl DockerClient {
    pub fn new() -> Self {
        DockerClient
    }

    pub fn run_command(&self, args: &[&str]) -> Result<Output, String> {
        let output = Command::new("docker")
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(output)
        } else {
            Err(str::from_utf8(&output.stderr)
                .unwrap_or("Unknown error")
                .to_string())
        }
    }

    pub fn list_containers(&self) -> Result<String, String> {
        let output = self.run_command(&["ps", "-a"])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }

    pub fn start_container(&self, container_id: &str) -> Result<String, String> {
        let output = self.run_command(&["start", container_id])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }

    pub fn stop_container(&self, container_id: &str) -> Result<String, String> {
        let output = self.run_command(&["stop", container_id])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }

    pub fn remove_container(&self, container_id: &str) -> Result<String, String> {
        let output = self.run_command(&["rm", container_id])?;

        Ok(str::from_utf8(&output.stdout)
            .unwrap_or("Failed to parse output")
            .to_string())
    }
}
