//! # GameClient Binary Entry Point
//!
//! Main executable for the Command & Conquer Generals Zero Hour GameClient

use game_client_rust::core::GameClient;
use game_client_rust::GameClientResult;

fn main() -> GameClientResult<()> {
    // Initialize logging
    env_logger::init();

    println!(
        "Command & Conquer Generals Zero Hour - GameClient Rust v{}",
        game_client_rust::VERSION
    );

    // Initialize the game client library
    game_client_rust::init()?;

    // Create and initialize the GameClient
    let _client = GameClient::new()?;
    println!("GameClient created successfully");

    // For now, just show that we can create the GameClient
    // In a full implementation, this would be the main game loop
    println!("GameClient initialization would happen here");
    println!("Main game loop would run here");

    Ok(())
}
