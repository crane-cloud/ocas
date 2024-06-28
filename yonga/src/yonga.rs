// use std::process::Command;
// use std::io::{self, BufRead, BufReader};

// use clap::{App, Arg};

// fn main() -> io::Result<()> {
//     let matches = App::new("Yonga")
//         .version("0.1.0")
//         .arg(Arg::with_name("service")
//             .short('c')
//             .long("service")
//             .value_name("SERVICE")
//             .help("Sets the name of the service")
//             .takes_value(true)
//             .required(true))
//         .get_matches();

//     let service = matches.value_of("service").unwrap();

//     let services = Command::new("docker")
//         .arg("service")
//         .arg("ls")
//         .output()?;

//     if services.status.success() {
//         let reader = BufReader::new(&services.stdout[..]);

//         let mut service_vec = Vec::new(); // Initialize an empty vector

//         println!("Services in stack '{}':", service);
//         for line in reader.lines().skip(1) { // Skip the header line
//             let line = line?;
//             let parts: Vec<&str> = line.split_whitespace().collect();
//             if parts.len() >= 2 {
//                 service_vec.push(parts[1]); // Push the service name to the vector
//             }
//         }

//         // Print the collected service names
//         for service_name in &service_vec {
//             println!("{}", service_name);
//         }
//     } else {
//         // If the command failed, print its stderr
//         let stderr = String::from_utf8_lossy(&services.stderr);
//         eprintln!("Error: {}", stderr);
//     }

//     Ok(())
// }

fn main() {
    println!("Hello, world!");
}