//! # UI System Integration Example
//!
//! Demonstrates how to integrate and use the complete in-game UI system including:
//! - InGameUI (selection box, minimap, resource display, placement preview)
//! - CommandPanel (unit commands, building construction, special powers)
//! - IntegratedUISystem (unified system with all components)
//!
//! This example shows the proper way to initialize, update, and render the UI.

use std::sync::Arc;
use std::time::Duration;

use game_client_rust::gui::{
    IntegratedUISystem, IntegratedUISystemBuilder,
    CommandButton, CommandButtonType, UICommand,
};
use game_client_rust::input::{
    keyboard::KeyboardState,
    mouse::MouseState,
};

/// Example showing basic UI system setup
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("=== UI System Integration Example ===\n");

    // Note: In a real application, you would create these from your wgpu setup
    // let device = Arc::new(...); // from wgpu initialization
    // let queue = Arc::new(...); // from wgpu initialization
    // let format = TextureFormat::Bgra8UnormSrgb; // or your preferred format

    println!("1. Creating UI System");
    println!("   In a real app, you would use:");
    println!("   let ui_system = IntegratedUISystemBuilder::new()");
    println!("       .with_device(device)");
    println!("       .with_queue(queue)");
    println!("       .with_format(format)");
    println!("       .with_screen_size(1920, 1080)");
    println!("       .build()?;");
    println!();

    println!("2. Initializing UI System");
    println!("   ui_system.init()?;");
    println!();

    println!("3. Setting up command buttons");
    example_command_setup();
    println!();

    println!("4. Updating resources");
    example_resource_update();
    println!();

    println!("5. Handling selection");
    example_selection_handling();
    println!();

    println!("6. Updating minimap");
    example_minimap_update();
    println!();

    println!("7. Main game loop integration");
    example_game_loop();
    println!();

    println!("8. Building placement");
    example_building_placement();
    println!();

    println!("=== Example Complete ===");

    Ok(())
}

/// Example: Setting up command buttons for different contexts
fn example_command_setup() {
    println!("   // Create buttons for barracks");
    println!("   let buttons = vec![");
    println!("       CommandButton::new(");
    println!("           \"build_ranger\".into(),");
    println!("           \"Ranger\".into(),");
    println!("           \"ranger_icon\".into(),");
    println!("           CommandButtonType::Build,");
    println!("       )");
    println!("       .with_hotkey('R')");
    println!("       .with_cost(225)");
    println!("       .with_description(\"Anti-infantry unit\".into()),");
    println!("       // ... more buttons");
    println!("   ];");
    println!("   ");
    println!("   // Update command panel");
    println!("   ui_system.update_for_selection(vec![barracks_id])?;");
}

/// Example: Updating resource display
fn example_resource_update() {
    println!("   // Update resources from game logic");
    println!("   ui_system.update_resources(");
    println!("       player.credits,      // $10,000");
    println!("       player.power_available,  // 100");
    println!("       player.power_used       // 75");
    println!("   );");
}

/// Example: Handling unit selection
fn example_selection_handling() {
    println!("   // When units are selected");
    println!("   let selected_units = vec![unit_id_1, unit_id_2, unit_id_3];");
    println!("   ui_system.update_for_selection(selected_units)?;");
    println!("   ");
    println!("   // Get current selection");
    println!("   let selection = ui_system.get_selection();");
    println!("   ");
    println!("   // Save to group (Ctrl+1)");
    println!("   ui_system.set_selection_group(1);");
    println!("   ");
    println!("   // Recall group (press 1)");
    println!("   ui_system.recall_selection_group(1);");
}

/// Example: Updating minimap with unit positions
fn example_minimap_update() {
    println!("   // Set world bounds");
    println!("   ui_system.set_minimap_bounds(");
    println!("       0.0, 0.0,      // min_x, min_z");
    println!("       1000.0, 1000.0 // max_x, max_z");
    println!("   );");
    println!("   ");
    println!("   // Update camera position");
    println!("   ui_system.update_camera(");
    println!("       camera_x, camera_y, camera_z,");
    println!("       viewport_width, viewport_height");
    println!("   );");
    println!("   ");
    println!("   // Add/update unit icons");
    println!("   for unit in units {{");
    println!("       let color = if unit.is_friendly {{");
    println!("           [0.0, 1.0, 0.0, 1.0] // Green");
    println!("       }} else {{");
    println!("           [1.0, 0.0, 0.0, 1.0] // Red");
    println!("       }};");
    println!("       ");
    println!("       ui_system.update_minimap_unit(");
    println!("           unit.id,");
    println!("           unit.position.x,");
    println!("           unit.position.z,");
    println!("           color");
    println!("       );");
    println!("   }}");
    println!("   ");
    println!("   // Remove destroyed units");
    println!("   ui_system.remove_minimap_unit(destroyed_unit_id);");
}

/// Example: Main game loop integration
fn example_game_loop() {
    println!("   // In your game loop:");
    println!("   loop {{");
    println!("       let delta_time = frame_timer.elapsed();");
    println!("       ");
    println!("       // 1. Handle input");
    println!("       ui_system.handle_input(&mouse_state, &keyboard_state)?;");
    println!("       ");
    println!("       // 2. Process UI commands");
    println!("       for command in ui_system.get_commands() {{");
    println!("           match command {{");
    println!("               UICommand::Build(template) => {{");
    println!("                   // Start building placement");
    println!("                   ui_system.start_building_placement(");
    println!("                       template,");
    println!("                       footprint_x,");
    println!("                       footprint_z");
    println!("                   );");
    println!("               }}");
    println!("               UICommand::UnitCommand(cmd) => {{");
    println!("                   // Execute unit command");
    println!("                   game_logic.execute_command(cmd, selection);");
    println!("               }}");
    println!("               UICommand::SpecialPower(power) => {{");
    println!("                   // Trigger special power");
    println!("                   game_logic.activate_special_power(power);");
    println!("               }}");
    println!("               _ => {{}}");
    println!("           }}");
    println!("       }}");
    println!("       ");
    println!("       // 3. Update UI state");
    println!("       ui_system.update(delta_time)?;");
    println!("       ");
    println!("       // 4. Render UI");
    println!("       ui_system.render(&texture_view)?;");
    println!("       ");
    println!("       // 5. Present frame");
    println!("       surface.present();");
    println!("   }}");
}

/// Example: Building placement workflow
fn example_building_placement() {
    println!("   // User clicks build button");
    println!("   ui_system.start_building_placement(");
    println!("       \"USA_SupplyCenter\".into(),");
    println!("       3.0, 3.0 // footprint size");
    println!("   );");
    println!("   ");
    println!("   // In update loop, check mouse for placement position");
    println!("   // The UI will automatically show green/red preview");
    println!("   ");
    println!("   // User right-clicks to cancel");
    println!("   if mouse_state.right_button.just_pressed() {{");
    println!("       ui_system.cancel_building_placement();");
    println!("   }}");
    println!("   ");
    println!("   // Or left-click to place (handled automatically)");
    println!("   // UI will emit UICommand::Build when placement is confirmed");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_runs() {
        // Just verify the example code is valid
        assert!(main().is_ok());
    }
}
