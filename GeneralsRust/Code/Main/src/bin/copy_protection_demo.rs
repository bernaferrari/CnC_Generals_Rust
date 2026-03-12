use anyhow::Result;
use generals_main::copy_protection::{
    check_for_message, configure_copy_protection, initialize_copy_protection, is_launcher_running,
    notify_launcher, shutdown, LauncherMessage,
};
use generals_main::single_instance::initialize_single_instance_protection_with_copy_protection;
use generals_main::version::initialize_version_system_with_copy_protection;
use log::{error, info, warn};
use std::thread;
use std::time::Duration;

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    info!("=== Copy Protection System Demo ===");

    // Configure copy protection (development mode enabled for demo)
    configure_copy_protection(true, true);
    info!("Copy protection configured for development mode");

    // Initialize copy protection system
    initialize_copy_protection()?;
    info!("Copy protection system initialized");

    // Initialize version system with copy protection integration
    initialize_version_system_with_copy_protection();

    // Initialize single instance protection with copy protection
    let _guard = match initialize_single_instance_protection_with_copy_protection() {
        Ok(guard) => {
            info!("Single instance protection initialized");
            Some(guard)
        }
        Err(e) => {
            warn!("Single instance protection failed: {}", e);
            None
        }
    };

    // Check launcher status
    let launcher_running = is_launcher_running();
    info!("Launcher running: {}", launcher_running);

    // Notify launcher of game start
    match notify_launcher() {
        Ok(()) => info!("Launcher notification sent successfully"),
        Err(e) => warn!("Failed to notify launcher: {}", e),
    }

    // Simulate game loop with copy protection checks
    info!("Starting simulated game loop...");
    for i in 0..5 {
        info!("Game loop iteration {}", i + 1);

        // Check for launcher messages
        match check_for_message() {
            Ok(Some(message)) => {
                info!("Received launcher message: {:?}", message);
            }
            Ok(None) => {
                // No messages - this is normal
            }
            Err(e) => {
                warn!("Error checking for launcher messages: {}", e);
            }
        }

        // Simulate some game work
        thread::sleep(Duration::from_millis(500));
    }

    // Shutdown copy protection system
    match shutdown() {
        Ok(()) => info!("Copy protection shutdown completed successfully"),
        Err(e) => error!("Copy protection shutdown failed: {}", e),
    }

    info!("=== Demo Complete ===");
    Ok(())
}
