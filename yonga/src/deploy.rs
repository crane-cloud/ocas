use clap::{Arg, Command, ArgAction};
use yonga::docker_client::DockerClient;
//use yonga::stack::StackConfig;

fn main() {
    let matches = Command::new("YongaDeploy")
        .arg(Arg::new("container")
            .long("container")
            .short('r')
            .action(ArgAction::Set))
        .arg(Arg::new("stack")
            .long("stack")
            .short('k')
            .action(ArgAction::Set))
        .arg(Arg::new("service")
            .long("service")
            .short('s')
            .action(ArgAction::Set))
        .arg(Arg::new("id")
            .long("id")
            .short('i')
            .required(false)
            .action(ArgAction::Set))
        .arg(Arg::new("name")
            .long("name")
            .short('n')
            .required(false)
            .action(ArgAction::Set))
        .arg(Arg::new("file")
            .long("file")
            .short('f')
            .required(false)
            .action(ArgAction::Set))
        .arg(Arg::new("replica")
            .long("replica")
            .short('p')
            .required(false)
            .action(ArgAction::Set))
        .arg(Arg::new("constraint")
            .long("constraint")
            .short('c')
            .required(false)
            .action(ArgAction::Set))
        .group(clap::ArgGroup::new("deploy_target")
            .args(&["container", "stack", "service"])
            .required(true))
        .get_matches();

    let docker_client = DockerClient::new();

    if let Some(container) = matches.get_one::<String>("container") {
        handle_container(container, &matches, &docker_client);
    }

    if let Some(stack) = matches.get_one::<String>("stack") {
        handle_stack(stack, &matches, &docker_client);
    }

    if let Some(service) = matches.get_one::<String>("service") {
        handle_service(service, &matches, &docker_client);
    }
}

fn handle_container(container: &str, matches: &clap::ArgMatches, docker_client: &DockerClient) {
    match container {
        "ls" => match docker_client.list_containers() {
            Ok(containers) => println!("{}", containers),
            Err(e) => eprintln!("Error listing containers: {}", e),
        },
        "start" => match matches.get_one::<String>("id") {
            Some(id) => match docker_client.start_container(id) {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error starting container: {}", e),
            },
            None => eprintln!("Container ID is required to start a container."),
        },
        "stop" => match matches.get_one::<String>("id") {
            Some(id) => match docker_client.stop_container(id) {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error stopping container: {}", e),
            },
            None => eprintln!("Container ID is required to stop a container."),
        },
        "rm" => match matches.get_one::<String>("id") {
            Some(id) => match docker_client.remove_container(id) {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error removing container: {}", e),
            },
            None => eprintln!("Container ID is required to remove a container."),
        },
        _ => eprintln!("Invalid container command."),
    }
}

fn handle_stack(stack: &str, matches: &clap::ArgMatches, docker_client: &DockerClient) {
    match stack {
        "deploy" => match (matches.get_one::<String>("name"), matches.get_one::<String>("file")) {
            (Some(name), Some(file)) => match docker_client.deploy_stack(name, file) {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error deploying stack: {}", e),
            },
            _ => eprintln!("Stack name and compose file are required to deploy a stack."),
        },
        "ls" => match matches.get_one::<String>("name") {
            Some(name) => match docker_client.list_stack_services(name) {
                Ok(services) => println!("{}", services),
                Err(e) => eprintln!("Error listing stack services: {}", e),
            },
            None => eprintln!("Stack name is required to list stack services."),
        },
        "rm" => match matches.get_one::<String>("name") {
            Some(name) => match docker_client.remove_stack(name) {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error removing stack: {}", e),
            },
            None => eprintln!("Stack name is required to remove a stack."),
        },
        _ => eprintln!("Invalid stack command."),
    }
}

fn handle_service(service: &str, matches: &clap::ArgMatches, docker_client: &DockerClient) {
    match service {
        "scale" => match (matches.get_one::<String>("name"), matches.get_one::<String>("replica")) {
            (Some(name), Some(replica)) => match docker_client.scale_service(name, replica) {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error scaling service: {}", e),
            },
            _ => eprintln!("Service name and replica count are required to scale a service."),
        },
        "update" => match (matches.get_one::<String>("name"), matches.get_one::<String>("constraint")) {
            (Some(name), Some(constraint)) => match docker_client.update_placement(name, constraint) {
                Ok(output) => println!("{}", output),
                Err(e) => eprintln!("Error updating service placement: {}", e),
            },
            _ => eprintln!("Service name and constraint are required to update service placement."),
        },
        _ => eprintln!("Invalid service command."),
    }
}