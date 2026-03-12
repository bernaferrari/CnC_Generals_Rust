use generals_main::game_logic::{GameLogic, GameMode};
use generals_main::ui::layout_manager::UILayoutManager;
use std::sync::{Arc, Mutex};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", "=".repeat(80));
    println!("COMMAND & CONQUER GENERALS ZERO HOUR - UI LAYOUT DEMO");
    println!("{}", "=".repeat(80));
    println!("Demonstrating UI layout system matching C++ GameWindow architecture");
    println!();

    // Initialize layout manager with typical screen resolution
    let mut layout_manager = UILayoutManager::new(1024.0, 768.0);

    // Create main menu layout (matching C++ MainMenu.cpp)
    println!("1. Creating Main Menu Layout (matches C++ MainMenu.cpp):");
    println!("{}", "-".repeat(60));
    let main_menu_buttons = layout_manager.create_main_menu_layout();

    println!("Main Menu Elements:");
    for (name, &id) in &main_menu_buttons {
        if let Some(element) = layout_manager.get_element(id) {
            let rect = element.get_absolute_rect(&layout_manager);
            println!(
                "  {}: ({:.0}, {:.0}) {}x{} - \"{}\"",
                name, rect.x, rect.y, rect.width, rect.height, element.text
            );
        }
    }

    println!();

    // Create control bar layout (matching C++ ControlBar.cpp)
    println!("2. Creating Control Bar Layout (matches C++ ControlBar.cpp):");
    println!("{}", "-".repeat(60));
    let control_bar_elements = layout_manager.create_control_bar_layout();

    println!("Control Bar Elements:");
    for (name, &id) in &control_bar_elements {
        if let Some(element) = layout_manager.get_element(id) {
            let rect = element.get_absolute_rect(&layout_manager);
            println!(
                "  {}: ({:.0}, {:.0}) {}x{}",
                name, rect.x, rect.y, rect.width, rect.height
            );
        }
    }

    println!();

    // Demonstrate UI state management
    println!("3. UI State Management (matches C++ GameWindowManager):");
    println!("{}", "-".repeat(60));

    // Show all main menu elements initially
    let visible_count = layout_manager.get_all_visible_elements().len();
    println!("Total visible elements: {}", visible_count);

    // Hide main menu, show control bar (simulate game start)
    for &id in main_menu_buttons.values() {
        layout_manager.set_element_visible(id, false);
    }
    for &id in control_bar_elements.values() {
        layout_manager.set_element_visible(id, true);
    }

    let visible_after = layout_manager.get_all_visible_elements().len();
    println!(
        "After switching to in-game UI: {} visible elements",
        visible_after
    );

    println!();

    // Demonstrate mouse interaction simulation
    println!("4. Mouse Interaction Simulation:");
    println!("{}", "-".repeat(60));

    // Simulate clicking on different areas
    let test_positions = vec![
        (200.0, 400.0, "Main menu area"),
        (900.0, 650.0, "Command panel area"),
        (65.0, 680.0, "Minimap area"),
        (500.0, 300.0, "Game view area"),
    ];

    for (x, y, description) in test_positions {
        if let Some(element_id) = layout_manager.find_element_at_position(x, y) {
            if let Some(element) = layout_manager.get_element(element_id) {
                println!(
                    "  Click at ({:.0}, {:.0}) - {}: Hit \"{}\"",
                    x, y, description, element.name
                );
            }
        } else {
            println!(
                "  Click at ({:.0}, {:.0}) - {}: No UI element",
                x, y, description
            );
        }
    }

    println!();

    // Demonstrate resize handling
    println!("5. Window Resize Handling:");
    println!("{}", "-".repeat(60));
    let original_size = layout_manager.get_screen_size();
    println!(
        "Original resolution: {}x{}",
        original_size.0, original_size.1
    );

    // Resize to different resolution
    layout_manager.resize(1920.0, 1080.0);
    let new_size = layout_manager.get_screen_size();
    println!("After resize: {}x{}", new_size.0, new_size.1);

    // Show how elements scaled
    if let Some(&minimap_id) = control_bar_elements.get("Minimap") {
        if let Some(minimap) = layout_manager.get_element(minimap_id) {
            let rect = minimap.get_absolute_rect(&layout_manager);
            println!(
                "Minimap after resize: ({:.0}, {:.0}) {:.0}x{:.0}",
                rect.x, rect.y, rect.width, rect.height
            );
        }
    }

    println!();

    // Demonstrate integration with game logic
    println!("6. Game Logic Integration:");
    println!("{}", "-".repeat(60));

    let game_logic = Arc::new(Mutex::new(GameLogic::new()));

    // Simulate game state changes
    {
        let mut logic = game_logic.lock().unwrap();
        println!(
            "Game state: In Game = {}, Paused = {}",
            logic.isInGame(),
            logic.is_paused()
        );

        logic.start_new_game(GameMode::Skirmish);
        println!("Started skirmish game");
        println!(
            "Game state: In Game = {}, Paused = {}",
            logic.isInGame(),
            logic.is_paused()
        );
    }

    println!();

    // Show final statistics
    println!("7. Final Statistics:");
    println!("{}", "-".repeat(60));
    let total_elements = layout_manager.get_all_visible_elements().len() + main_menu_buttons.len(); // Include hidden main menu elements
    println!("Total UI elements created: {}", total_elements);
    println!(
        "Currently visible elements: {}",
        layout_manager.get_all_visible_elements().len()
    );

    // Show element hierarchy
    println!("\nElement Hierarchy (matches C++ GameWindow parent/child structure):");
    let visible_elements = layout_manager.get_all_visible_elements();
    for &element_id in &visible_elements {
        if let Some(element) = layout_manager.get_element(element_id) {
            let indent = if element.name.contains("Button") {
                "    "
            } else {
                ""
            };
            println!(
                "{}{}\" (ID: {}, Z-Order: {})",
                indent, element.name, element.id, element.z_order
            );
        }
    }

    println!();
    println!("{}", "=".repeat(80));
    println!("DEMO COMPLETED");
    println!("{}", "=".repeat(80));
    println!("This demonstrates the core UI architecture that matches:");
    println!("- C++ GameWindow system for element management");
    println!("- C++ GameWindowManager for layout and hierarchy");
    println!("- C++ MainMenu.cpp button layout and positioning");
    println!("- C++ ControlBar.cpp HUD element arrangement");
    println!("- Proper scaling, visibility, and interaction handling");
    println!();
    println!("The actual WGPU rendering will be added to provide pixel-perfect");
    println!("visual matching with the original C++ implementation.");

    Ok(())
}
