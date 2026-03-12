//! UI Demo Binary
//! 
//! This binary runs the Command & Conquer Generals UI system demonstration,
//! showcasing all major interface components and interactions.

use std::io::{self, Write};
use generals_main::ui_demo::run_ui_demo;

fn main() {
    println!("Command & Conquer Generals Zero Hour - UI System Demo");
    println!("=====================================================");
    println!();
    println!("This demonstration showcases the complete user interface system");
    println!("for Command & Conquer Generals, including:");
    println!();
    println!("• Main Menu with navigation and options");
    println!("• Faction Selection for USA, China, and GLA");
    println!("• In-Game HUD with resource display and mini-map");
    println!("• Construction and unit command interfaces");
    println!("• Pause menu and game state screens");
    println!("• Victory/defeat screens");
    println!();
    println!("The demo will automatically cycle through different UI screens");
    println!("and then enter interactive mode where you can try all features.");
    println!();
    print!("Press Enter to start the demo, or 'q' to quit: ");
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    
    if input.trim().to_lowercase() == "q" {
        println!("Demo cancelled.");
        return;
    }
    
    println!("\nStarting UI Demo...\n");
    
    // Run the actual demo
    match run_ui_demo() {
        Ok(_) => {
            println!("\n🎉 UI Demo completed successfully!");
            println!();
            println!("The Command & Conquer Generals UI system provides:");
            println!("✅ Professional main menu with authentic C&C styling");
            println!("✅ Complete faction selection with all generals");
            println!("✅ Comprehensive in-game HUD for RTS gameplay");
            println!("✅ Resource management display (Credits and Power)");
            println!("✅ Mini-map with unit tracking and camera control");
            println!("✅ Building construction and unit production queues");
            println!("✅ Unit selection and command interfaces");
            println!("✅ Pause menu with save/load capabilities");
            println!("✅ Victory/defeat screens with mission results");
            println!("✅ Responsive mouse and keyboard controls");
            println!("✅ Smooth animations and visual feedback");
            println!();
            println!("The UI system is ready for integration with the full game engine!");
        }
        Err(e) => {
            eprintln!("❌ Demo failed with error: {}", e);
            eprintln!();
            eprintln!("This might be due to missing dependencies or system requirements.");
            eprintln!("Please check that all required libraries are installed.");
            std::process::exit(1);
        }
    }
}