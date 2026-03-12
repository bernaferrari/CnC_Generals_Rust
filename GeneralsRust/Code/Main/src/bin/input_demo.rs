//! Input System Demonstration
//!
//! This is a separate demo binary that shows off the RTS input system
//! without running the full game.

use env_logger;
use generals_main::demo_input_system;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Command & Conquer Generals - Input System Demo");
    println!("===============================================");
    println!("This demo shows the RTS input system functionality");
    println!("without running the full game graphics.");
    println!();

    // Run the input system demonstration
    demo_input_system();

    println!();
    println!("Demo completed! To run the full game use:");
    println!("  cargo run --bin generals");

    Ok(())
}
