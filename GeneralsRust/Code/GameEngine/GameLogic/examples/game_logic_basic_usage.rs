use gamelogic::runtime::{GameLogic, GameLogicConfig};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::init();

    println!("GameLogic Basic Usage Example");
    println!("==============================");

    // Create a new simulation instance
    let config = GameLogicConfig::default();
    let mut game_logic = GameLogic::with_config(config);

    println!("Created GameLogic instance");
    println!("Target FPS: {}", game_logic.config().target_fps);
    println!("Max players: {}", game_logic.config().max_players);

    // Simulate a few frames
    println!("\nSimulating ticks:");
    for i in 0..5 {
        let result = game_logic.tick();
        println!(
            "  Tick {}: frame_index={} events={}",
            i + 1,
            result.frame_index,
            result.events.len()
        );
    }

    println!("\nGameLogic basic usage example completed successfully!");
    Ok(())
}
