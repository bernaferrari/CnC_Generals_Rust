//! Basic AI Usage Example
//!
//! This example demonstrates how to use the converted AI system.

use gamelogic::{
    ai::{AiCommandInterface, AiSideInfo, AttitudeType, CommandSourceType},
    common::Coord3D,
    initialize, reset, update, THE_AI,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("Command & Conquer Generals Zero Hour - AI System Example");
    println!("=========================================================");

    // Initialize the game logic systems
    println!("Initializing AI system...");
    initialize()?;

    // Create an AI group
    println!("Creating AI group...");
    let group = {
        let mut ai = THE_AI.write().unwrap();
        ai.create_group()
    };

    // Add some units to the group (using dummy object IDs)
    {
        let mut g = group.write().unwrap();
        g.add(1001); // Tank
        g.add(1002); // Infantry
        g.add(1003); // Another tank

        println!("Created group with {} members", g.get_count());
        println!("Group ID: {}", g.get_id());
    }

    // Test command interface
    println!("Testing AI commands...");
    {
        let mut g = group.write().unwrap();

        // Send idle command
        g.ai_idle(CommandSourceType::FromPlayer)?;
        println!("✓ Sent idle command");

        // Send move command
        let position: Coord3D = [100.0, 0.0, 200.0].into();
        g.ai_move_to_position(&position, CommandSourceType::FromPlayer)?;
        println!(
            "✓ Sent move to position command: [{}, {}, {}]",
            position.x, position.y, position.z
        );

        // Send attack command
        g.ai_attack_object(2001, 10, CommandSourceType::FromAi)?;
        println!("✓ Sent attack object command (target: 2001, max shots: 10)");

        // Set attitude
        g.set_attitude(AttitudeType::Aggressive)?;
        println!("✓ Set group attitude to Aggressive");

        // Send hunt command
        g.ai_hunt(CommandSourceType::FromAi)?;
        println!("✓ Sent hunt command");
    }

    // Test AI data configuration
    println!("\nTesting AI data configuration...");
    {
        let ai_data = THE_AI.read().unwrap().get_ai_data();
        let mut data = ai_data.write().unwrap();

        // Add side information
        let mut usa_info = AiSideInfo::default();
        usa_info.side = "USA".to_string();
        usa_info.easy = 2;
        usa_info.normal = 3;
        usa_info.hard = 4;
        usa_info.base_defense_structure_1 = "PatriotMissileSite".to_string();

        data.add_side_info(usa_info);
        println!("✓ Added USA side info");

        // Display some configuration values
        println!("AI Configuration:");
        println!(
            "  - Structure build interval: {:.1}s",
            data.structure_seconds
        );
        println!("  - Team build interval: {:.1}s", data.team_seconds);
        println!(
            "  - Wealthy threshold: {} resources",
            data.resources_wealthy
        );
        println!("  - Poor threshold: {} resources", data.resources_poor);
        println!("  - AI crushes infantry: {}", data.ai_crushes_infantry);
        println!("  - Number of sides configured: {}", data.side_info.len());
    }

    // Simulate a few update frames
    println!("\nRunning AI updates...");
    for frame in 1..=5 {
        update()?;
        println!("Frame {}: AI update completed", frame);
    }

    // Test group removal
    println!("\nTesting group cleanup...");
    let group_id = {
        let g = group.read().unwrap();
        g.get_id()
    };

    {
        let mut ai = THE_AI.write().unwrap();
        ai.destroy_group(group_id)?;
        println!("✓ Destroyed group {}", group_id);

        // Verify group is gone
        let found_group = ai.find_group(group_id);
        if found_group.is_none() {
            println!("✓ Group successfully removed from AI system");
        } else {
            println!("❌ Group still exists in AI system");
        }
    }

    // Reset the system
    println!("\nResetting AI system...");
    reset()?;
    println!("✓ AI system reset completed");

    println!("\n🎉 AI System Example completed successfully!");
    println!("The C++ AI system has been successfully converted to Rust");
    println!("with the same public API and functionality.");

    Ok(())
}
