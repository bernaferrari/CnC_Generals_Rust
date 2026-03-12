//! Example demonstrating the Profile Command Interface
//!
//! This example shows how to use the ProfileCmdInterface to control profiling
//! through commands, similar to the original C++ interface.

use profile::{
    execute_command_to_string, execute_command_with_stdout, CommandMode, Profile,
    ProfileCommandExecutor, ProfileCommandParser,
};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the profiler
    profile::init();

    println!("=== Profile Command Interface Example ===\n");

    // Example 1: Using execute_command_with_stdout
    println!("1. Getting help for profile commands:");
    execute_command_with_stdout("help", CommandMode::Normal, &[])?;
    println!();

    // Example 2: Using execute_command_to_string
    println!("2. Listing available result functions:");
    let (handled, output) = execute_command_to_string("result", CommandMode::Normal, &[])?;
    if handled {
        print!("{}", output);
    }
    println!();

    // Example 3: Add some patterns for profiling
    println!("3. Adding profiling patterns:");
    execute_command_with_stdout("add", CommandMode::Normal, &["+", "example.*"])?;
    execute_command_with_stdout("add", CommandMode::Normal, &["+", "demo.*"])?;
    execute_command_with_stdout("add", CommandMode::Normal, &["-", "demo.skip"])?;

    // Example 4: Set up some profiling data
    println!("4. Running example profiling:");

    // Start profiling ranges that match our patterns
    Profile::start_range(Some("example.main"))?;

    // Add some high-level profile data
    let high_level = Profile::high_level();
    let counter =
        high_level.add_profile("example.counter", "Example counter", "operations", 0, 0)?;

    counter.increment(42.0);
    counter.increment(24.0);

    let timer = high_level.add_profile("example.timer", "Example timer", "ms", 1, 0)?;

    timer.increment(123.5);

    Profile::stop_range(Some("example.main"))?;

    // Example 5: Add result functions
    println!("5. Setting up result output:");
    execute_command_with_stdout("result", CommandMode::Normal, &["console", "verbose"])?;
    execute_command_with_stdout(
        "result",
        CommandMode::Normal,
        &["csv_file", "example_results.csv"],
    )?;

    // Example 6: Using ProfileCommandExecutor trait
    println!("6. Using ProfileCommandExecutor trait:");
    let mut buffer = Vec::new();
    buffer.execute_profile_command("help", CommandMode::Script, &["result"])?;
    // In script mode, help produces no output
    println!("Script mode help output length: {}", buffer.len());

    // Example 7: Using ProfileCommandParser
    println!("7. Parsing command lines:");
    let command_lines = ["help result", "result console", "add + test.*", "view"];

    for line in &command_lines {
        let (cmd, args) = ProfileCommandParser::parse_command_line(line);
        println!("Command: '{}', Args: {:?}", cmd, args);

        // Execute the command
        let mut output = Vec::new();
        ProfileCommandParser::execute_command_line(&mut output, line, CommandMode::Normal)?;
        let output_str = String::from_utf8_lossy(&output);
        if !output_str.trim().is_empty() {
            println!("Output: {}", output_str.trim());
        }
    }

    println!("\n=== Example completed ===");
    println!("Result functions will be executed automatically on program exit.");

    Ok(())
}
