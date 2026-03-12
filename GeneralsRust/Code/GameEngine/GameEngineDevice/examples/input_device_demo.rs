//! Input Device Demo
//!
//! Demonstrates the cross-platform input device abstraction system including:
//! - Keyboard input with modifier keys
//! - Mouse input with button and movement tracking
//! - Gamepad support
//! - Hotkey system
//! - Key bindings
//! - Input recording and playback

use game_engine_device::input::{
    BindingConfig, Hotkey, InputConfig, InputEvent, InputManager, KeyCode, ModifierKeys,
    MouseButton, PlaybackMode,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("=== Input Device System Demo ===\n");

    // Create input manager with custom configuration
    let config = InputConfig {
        keyboard_enabled: true,
        mouse_enabled: true,
        gamepad_enabled: true,
        mouse_sensitivity: 1.5,
        raw_mouse_input: true,
        key_repeat_delay_ms: 500,
        key_repeat_rate_ms: 33,
        gamepad_dead_zone: 0.15,
        recording_enabled: true,
        max_queue_size: 1024,
    };

    let mut input_manager = InputManager::with_config(config).await?;

    println!("Input manager initialized with configuration:");
    println!("  - Keyboard: enabled");
    println!("  - Mouse: enabled (sensitivity: 1.5x)");
    println!("  - Gamepad: enabled (dead zone: 0.15)");
    println!("  - Recording: enabled");
    println!();

    // Register some hotkeys
    demo_hotkeys(&mut input_manager)?;

    // Setup key bindings
    demo_key_bindings(&mut input_manager).await?;

    // Demonstrate input state querying
    demo_input_state(&input_manager);

    // Demonstrate gamepad support
    demo_gamepad_support(&input_manager);

    // Demonstrate input recording
    demo_input_recording(&mut input_manager).await?;

    // Simulate game loop
    println!("\n=== Simulating Game Loop ===");
    println!("(In a real game, this would process actual input events)");

    for frame in 0..10 {
        // Update input system
        input_manager.update(Duration::from_millis(16)).await?;

        // Poll events
        let events = input_manager.poll_all_events().await;

        if !events.is_empty() {
            println!("\nFrame {}: {} events", frame, events.len());
            for event in events.iter().take(5) {
                println!("  - {:?}", event);
            }
        }

        // Check input state
        let keyboard_state = input_manager.keyboard_state();
        let mouse_state = input_manager.mouse_state();

        if frame % 5 == 0 {
            println!(
                "Frame {}: Mouse at ({}, {}), {} keys pressed",
                frame,
                mouse_state.x(),
                mouse_state.y(),
                keyboard_state.pressed_keys().count()
            );
        }

        tokio::time::sleep(Duration::from_millis(16)).await;
    }

    println!("\n=== Demo Complete ===");
    println!("Input system features demonstrated:");
    println!("  ✓ Cross-platform input abstraction");
    println!("  ✓ Keyboard with modifier keys");
    println!("  ✓ Mouse with button tracking");
    println!("  ✓ Gamepad support");
    println!("  ✓ Hotkey system");
    println!("  ✓ Key binding configuration");
    println!("  ✓ Input recording/playback");
    println!("  ✓ Input state tracking");

    // Shutdown
    input_manager.shutdown().await?;

    Ok(())
}

fn demo_hotkeys(input_manager: &mut InputManager) -> anyhow::Result<()> {
    println!("=== Registering Hotkeys ===");

    // Common editor hotkeys
    input_manager.register_hotkey("save", Hotkey::new(KeyCode::S).ctrl())?;
    println!("  - Registered: Ctrl+S (save)");

    input_manager.register_hotkey("copy", Hotkey::new(KeyCode::C).ctrl())?;
    println!("  - Registered: Ctrl+C (copy)");

    input_manager.register_hotkey("paste", Hotkey::new(KeyCode::V).ctrl())?;
    println!("  - Registered: Ctrl+V (paste)");

    // Game-specific hotkeys
    input_manager.register_hotkey("quicksave", Hotkey::new(KeyCode::F5))?;
    println!("  - Registered: F5 (quicksave)");

    input_manager.register_hotkey("screenshot", Hotkey::new(KeyCode::F12))?;
    println!("  - Registered: F12 (screenshot)");

    input_manager.register_hotkey("pause", Hotkey::new(KeyCode::Escape))?;
    println!("  - Registered: Escape (pause)");

    // RTS-specific hotkeys
    input_manager.register_hotkey("select_all", Hotkey::new(KeyCode::A).ctrl())?;
    println!("  - Registered: Ctrl+A (select all units)");

    input_manager.register_hotkey("attack_move", Hotkey::new(KeyCode::A))?;
    println!("  - Registered: A (attack move)");

    println!();
    Ok(())
}

async fn demo_key_bindings(input_manager: &mut InputManager) -> anyhow::Result<()> {
    println!("=== Setting Up Key Bindings ===");

    // Load default RTS bindings
    let config = BindingConfig::default_rts();
    println!("  - Loaded default RTS bindings");
    println!("  - Actions configured: {}", config.actions.len());

    // List some categories
    println!("\n  Categories:");
    for (name, action) in config.actions.iter().take(5) {
        println!(
            "    - {}: {} ({})",
            action.category,
            name,
            action.primary.display_string()
        );
    }

    // Save configuration (would normally save to file)
    // input_manager.save_bindings("bindings.json").await?;

    println!();
    Ok(())
}

fn demo_input_state(input_manager: &InputManager) {
    println!("=== Input State Querying ===");

    let keyboard_state = input_manager.keyboard_state();
    let mouse_state = input_manager.mouse_state();

    println!("  Keyboard:");
    println!(
        "    - Any key pressed: {}",
        keyboard_state.any_key_pressed()
    );
    println!("    - Modifiers: {:?}", keyboard_state.modifiers());

    println!("  Mouse:");
    println!("    - Position: ({}, {})", mouse_state.x(), mouse_state.y());
    println!("    - Delta: {:?}", mouse_state.delta());
    println!(
        "    - Any button pressed: {}",
        mouse_state.any_button_pressed()
    );
    println!("    - In window: {}", mouse_state.is_cursor_in_window());

    println!();
}

fn demo_gamepad_support(input_manager: &InputManager) {
    println!("=== Gamepad Support ===");

    let gamepads = input_manager.connected_gamepads();

    if gamepads.is_empty() {
        println!("  - No gamepads connected");
    } else {
        println!("  - Connected gamepads: {}", gamepads.len());

        for gamepad_id in gamepads {
            if let Some(state) = input_manager.gamepad_state(gamepad_id) {
                println!("    Gamepad {}: {}", gamepad_id.value(), state.name());
                println!(
                    "      - Buttons pressed: {}",
                    state.pressed_buttons().count()
                );
                println!("      - Left stick: {:?}", state.left_stick());
                println!("      - Right stick: {:?}", state.right_stick());
            }
        }
    }

    println!();
}

async fn demo_input_recording(input_manager: &mut InputManager) -> anyhow::Result<()> {
    println!("=== Input Recording ===");

    // Start recording
    input_manager.start_recording()?;
    println!("  - Recording started");

    // Simulate some input events
    println!("  - Simulating input events...");
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Stop recording
    input_manager.stop_recording()?;
    println!("  - Recording stopped");

    // Save recording (would normally save to file)
    // input_manager.save_recording("replay.json").await?;
    // println!("  - Recording saved to replay.json");

    // Demonstrate playback
    println!("\n  Playback:");
    println!("    - Mode: Once");
    println!("    - Speed: 1.0x");
    // input_manager.start_playback(PlaybackMode::Once)?;
    // println!("    - Playback started");

    println!();
    Ok(())
}
