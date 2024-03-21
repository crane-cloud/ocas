use std::process::Command;
use std::io::{self, BufRead, BufReader};

fn main() -> io::Result<()> {

    // Execute the command and capture its output
    let output = Command::new("ls")
        //.arg("-l")
        .output()?;
    
    // Check if the command was successful
    if output.status.success() {

        println!("Command was a success!!!");

        // If the command succeeded, process its output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines = stdout.lines();

        // Filter the output (for example, keep only lines containing ".txt")
        let filtered_lines = lines.filter(|line| line.contains(".txt"));

        // Print the filtered lines
        for line in filtered_lines {
            println!("{}", line);
        }
    } else {
        // If the command failed, print its stderr
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Error: {}", stderr);
    }

    Ok(())
}
