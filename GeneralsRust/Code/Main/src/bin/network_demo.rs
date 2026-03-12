//! Network System Demonstration
//!
//! This binary demonstrates the complete multiplayer networking system
//! for Command & Conquer Generals Zero Hour, showing how to:
//! - Initialize networking
//! - Create and join games
//! - Send commands and chat
//! - Handle synchronization

use generals_main::network::{
    init_network, ChatType, CommandTarget, LobbyCallbacks, NetworkConfig, NetworkInterface,
    PlayerInfo, UnitCommand, UnitCommandType,
};

use clap::{App, Arg};
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    let matches = App::new("C&C Generals Network Demo")
        .version("1.0.0")
        .author("Electronic Arts Inc.")
        .about("Demonstrates the multiplayer networking system for C&C Generals Zero Hour")
        .arg(
            Arg::with_name("mode")
                .short("m")
                .long("mode")
                .value_name("MODE")
                .help("Demo mode: host, client, or interactive")
                .possible_values(&["host", "client", "interactive"])
                .default_value("interactive"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .value_name("PORT")
                .help("Network port to use")
                .default_value("8088"),
        )
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .value_name("NAME")
                .help("Player name")
                .default_value("DemoPlayer"),
        )
        .get_matches();

    let mode = matches.value_of("mode").unwrap();
    let port: u16 = matches.value_of("port").unwrap().parse()?;
    let player_name = matches.value_of("name").unwrap();

    println!("=== Command & Conquer Generals Zero Hour Network Demo ===");
    println!("Mode: {}, Port: {}, Player: {}", mode, port, player_name);
    println!();

    match mode {
        "host" => run_host_demo(player_name, port).await?,
        "client" => run_client_demo(player_name, port).await?,
        "interactive" => run_interactive_demo(player_name, port).await?,
        _ => unreachable!(),
    }

    Ok(())
}

/// Run host demonstration
async fn run_host_demo(player_name: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎮 Starting HOST demonstration...");

    let network = init_host(player_name, port).await?;

    // Host a game
    {
        let net = network.read().await;
        net.host_game("Demo Game".to_string(), "Demo Map".to_string(), 4, None)
            .await?;
    }

    println!("✅ Game hosted successfully!");
    println!("📡 Waiting for players to join...");

    // Run game loop
    let mut frame_count = 0;
    loop {
        {
            let net = network.read().await;
            net.update().await?;

            let stats = net.get_statistics().await;

            if frame_count % 300 == 0 {
                // Every 5 seconds at 60fps
                println!(
                    "📊 Stats - Players: {}, Frame: {}, State: {:?}",
                    stats.connected_players, stats.current_frame, stats.state
                );

                if stats.connected_players > 1 {
                    println!("🚀 Multiple players connected! Starting game...");
                    net.start_game().await?;
                    break;
                }
            }
        }

        frame_count += 1;
        tokio::time::sleep(std::time::Duration::from_millis(16)).await; // ~60 FPS

        if frame_count > 1800 {
            // 30 seconds timeout
            println!("⏰ Timeout waiting for players");
            break;
        }
    }

    // Run game simulation
    run_game_simulation(network, true).await?;

    Ok(())
}

/// Run client demonstration
async fn run_client_demo(player_name: &str, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎮 Starting CLIENT demonstration...");

    let network = init_client(player_name, port + 1).await?; // Use different port

    // Look for games
    println!("🔍 Searching for games...");
    {
        let net = network.read().await;
        net.refresh_games().await?;
    }

    // Wait and try to join
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let mut joined = false;
    {
        let net = network.read().await;
        let games = net.get_available_games().await;

        if let Some(game) = games.first() {
            println!("🎯 Found game: '{}', attempting to join...", game.name);
            if let Ok(()) = net.join_game(&game.name, None).await {
                joined = true;
                println!("✅ Successfully joined game!");
            }
        } else {
            println!("❌ No games found");
        }
    }

    if !joined {
        println!("🤖 Creating AI opponent simulation...");
        return run_ai_simulation(network).await;
    }

    // Run game simulation
    run_game_simulation(network, false).await?;

    Ok(())
}

/// Run interactive demonstration
async fn run_interactive_demo(
    player_name: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎮 Starting INTERACTIVE demonstration...");

    let network = init_host(player_name, port).await?;

    loop {
        println!("\n=== Network Demo Menu ===");
        println!("1. Host game");
        println!("2. Join game");
        println!("3. Refresh games");
        println!("4. Send chat");
        println!("5. Send unit command");
        println!("6. Show status");
        println!("7. Exit");
        print!("Choose option: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        match input.trim() {
            "1" => {
                let net = network.read().await;
                if let Err(e) = net
                    .host_game(
                        "Interactive Game".to_string(),
                        "Test Map".to_string(),
                        4,
                        None,
                    )
                    .await
                {
                    println!("❌ Failed to host game: {}", e);
                } else {
                    println!("✅ Game hosted!");
                }
            }
            "2" => {
                let net = network.read().await;
                let games = net.get_available_games().await;
                if games.is_empty() {
                    println!("❌ No games available");
                } else {
                    for (i, game) in games.iter().enumerate() {
                        println!(
                            "{}: {} ({}/{})",
                            i + 1,
                            game.name,
                            game.current_players,
                            game.max_players
                        );
                    }
                    print!("Select game number: ");
                    io::stdout().flush()?;

                    let mut game_input = String::new();
                    io::stdin().read_line(&mut game_input)?;

                    if let Ok(choice) = game_input.trim().parse::<usize>() {
                        if choice > 0 && choice <= games.len() {
                            let game = &games[choice - 1];
                            if let Err(e) = net.join_game(&game.name, None).await {
                                println!("❌ Failed to join: {}", e);
                            } else {
                                println!("✅ Joined game!");
                            }
                        }
                    }
                }
            }
            "3" => {
                let net = network.read().await;
                net.refresh_games().await?;
                println!("🔄 Games refreshed");
            }
            "4" => {
                print!("Enter message: ");
                io::stdout().flush()?;
                let mut msg = String::new();
                io::stdin().read_line(&mut msg)?;

                let net = network.read().await;
                if let Err(e) = net
                    .send_chat(msg.trim().to_string(), ChatType::All, None)
                    .await
                {
                    println!("❌ Failed to send chat: {}", e);
                } else {
                    println!("💬 Chat sent!");
                }
            }
            "5" => {
                print!("Enter unit IDs (comma-separated): ");
                io::stdout().flush()?;
                let mut units_input = String::new();
                io::stdin().read_line(&mut units_input)?;

                let unit_ids: Vec<u32> = units_input
                    .trim()
                    .split(',')
                    .filter_map(|s| s.trim().parse().ok())
                    .collect();

                if !unit_ids.is_empty() {
                    let cmd = UnitCommand {
                        command_type: UnitCommandType::Move,
                        unit_ids,
                        target: Some(CommandTarget::Position(glam::Vec2::new(100.0, 100.0))),
                        parameters: Vec::new(),
                    };

                    let net = network.read().await;
                    if let Err(e) = net.send_unit_command(cmd).await {
                        println!("❌ Failed to send command: {}", e);
                    } else {
                        println!("⚡ Command sent!");
                    }
                } else {
                    println!("❌ Invalid unit IDs");
                }
            }
            "6" => {
                let net = network.read().await;
                let stats = net.get_statistics().await;
                println!("📊 Network Statistics:");
                println!("  State: {:?}", stats.state);
                println!("  Players: {}", stats.connected_players);
                println!("  Frame: {}", stats.current_frame);
                println!("  Sync: {:?}", stats.sync_state);
                println!(
                    "  Sent: {} bytes, Received: {} bytes",
                    stats.bytes_sent, stats.bytes_received
                );
                println!("  Uptime: {:?}", stats.uptime);
            }
            "7" => {
                println!("👋 Goodbye!");
                break;
            }
            _ => {
                println!("❌ Invalid option");
            }
        }

        // Update network
        let net = network.read().await;
        net.update().await?;
    }

    Ok(())
}

/// Initialize network as host
async fn init_host(
    player_name: &str,
    port: u16,
) -> Result<Arc<RwLock<NetworkInterface>>, Box<dyn std::error::Error>> {
    let network = init_network()?;

    let mut config = NetworkConfig::default();
    config.port = port;
    config.enable_lan = true;

    let local_player = PlayerInfo::new(
        1,
        player_name.to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
    );

    {
        let net = network.write().await;
        net.initialize(config).await?;
        net.set_local_player(local_player).await?;

        // Set up callbacks
        let callbacks = LobbyCallbacks {
            on_player_joined: Some(Box::new(|player| {
                println!("🎉 Player '{}' joined the game!", player.name);
            })),
            on_player_left: Some(Box::new(|player_id| {
                println!("👋 Player {} left the game", player_id);
            })),
            on_game_started: Some(Box::new(|| {
                println!("🚀 Game is starting!");
            })),
            ..Default::default()
        };
        net.set_lobby_callbacks(callbacks).await?;

        net.start().await?;
    }

    println!("🌐 Network initialized as HOST on port {}", port);
    Ok(network)
}

/// Initialize network as client
async fn init_client(
    player_name: &str,
    port: u16,
) -> Result<Arc<RwLock<NetworkInterface>>, Box<dyn std::error::Error>> {
    let network = init_network()?;

    let mut config = NetworkConfig::default();
    config.port = port;
    config.enable_lan = true;

    let local_player = PlayerInfo::new(
        2, // Different player ID
        player_name.to_string(),
        SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
    );

    {
        let net = network.write().await;
        net.initialize(config).await?;
        net.set_local_player(local_player).await?;

        // Set up callbacks
        let callbacks = LobbyCallbacks {
            on_game_found: Some(Box::new(|game| {
                println!(
                    "🎯 Found game: '{}' ({}/{} players)",
                    game.name, game.current_players, game.max_players
                );
            })),
            on_game_started: Some(Box::new(|| {
                println!("🚀 Game is starting!");
            })),
            ..Default::default()
        };
        net.set_lobby_callbacks(callbacks).await?;

        net.start().await?;
    }

    println!("🌐 Network initialized as CLIENT on port {}", port);
    Ok(network)
}

/// Run game simulation
async fn run_game_simulation(
    network: Arc<RwLock<NetworkInterface>>,
    is_host: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎮 Running game simulation...");

    let mut frame_count = 0;
    let simulation_frames = 600; // 10 seconds at 60fps

    while frame_count < simulation_frames {
        {
            let net = network.read().await;
            net.update().await?;

            // Send periodic commands
            if is_host && frame_count % 60 == 0 {
                // Every second
                let cmd = UnitCommand {
                    command_type: UnitCommandType::Move,
                    unit_ids: vec![1, 2, 3],
                    target: Some(CommandTarget::Position(glam::Vec2::new(
                        frame_count as f32,
                        100.0,
                    ))),
                    parameters: Vec::new(),
                };

                if net.is_ready_for_commands().await {
                    let _ = net.send_unit_command(cmd).await;
                    println!("⚡ Sent move command at frame {}", frame_count);
                }
            }

            // Send chat every 5 seconds
            if frame_count % 300 == 0 {
                let message = format!(
                    "Frame {} - Hello from {}!",
                    frame_count,
                    if is_host { "HOST" } else { "CLIENT" }
                );
                let _ = net.send_chat(message, ChatType::All, None).await;
            }

            // Show stats every 3 seconds
            if frame_count % 180 == 0 {
                let stats = net.get_statistics().await;
                println!(
                    "📊 Frame {}: Players: {}, State: {:?}, Sync: {:?}",
                    frame_count, stats.connected_players, stats.state, stats.sync_state
                );
            }
        }

        frame_count += 1;
        tokio::time::sleep(std::time::Duration::from_millis(16)).await; // ~60 FPS
    }

    println!("✅ Game simulation completed!");

    // Shutdown
    {
        let net = network.read().await;
        net.shutdown().await;
    }

    Ok(())
}

/// Run AI simulation (when no other players)
async fn run_ai_simulation(
    network: Arc<RwLock<NetworkInterface>>,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🤖 Running AI simulation...");

    // Host our own game for AI
    {
        let net = network.read().await;
        net.host_game(
            "AI Demo Game".to_string(),
            "AI Map".to_string(),
            1, // Single player
            None,
        )
        .await?;

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        net.start_game().await?;
    }

    let mut frame_count = 0;
    let simulation_frames = 300; // 5 seconds

    while frame_count < simulation_frames {
        {
            let net = network.read().await;
            net.update().await?;

            // AI commands every 30 frames
            if frame_count % 30 == 0 {
                let cmd = UnitCommand {
                    command_type: UnitCommandType::Move,
                    unit_ids: vec![(frame_count / 30) as u32 + 1],
                    target: Some(CommandTarget::Position(glam::Vec2::new(
                        (frame_count as f32 * 0.1).sin() * 100.0,
                        (frame_count as f32 * 0.1).cos() * 100.0,
                    ))),
                    parameters: Vec::new(),
                };

                if net.is_ready_for_commands().await {
                    let _ = net.send_unit_command(cmd).await;
                    println!("🤖 AI sent command at frame {}", frame_count);
                }
            }
        }

        frame_count += 1;
        tokio::time::sleep(std::time::Duration::from_millis(16)).await;
    }

    println!("✅ AI simulation completed!");

    // Shutdown
    {
        let net = network.read().await;
        net.shutdown().await;
    }

    Ok(())
}
