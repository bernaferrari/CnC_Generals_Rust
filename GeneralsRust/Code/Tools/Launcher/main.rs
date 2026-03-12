//! Main Module
//!
//! Corresponds to C++ file: Tools/Launcher/main.cpp
//!
//! This module provides the game launcher entry point.
//!
//! The launcher is responsible for:
//! - Loading configuration files (.lcf format)
//! - Finding and applying patches
//! - Managing game process execution
//! - Handling patch updates and re-launches
mod protect;

mod bfish;
use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use std::env;
use std::fs;
use std::path::Path;

/// Update return value constant - matches C++ UPDATE_RETVAL
/// If a program returns this value, it means it wants to check for patches
#[allow(dead_code)]
const UPDATE_RETVAL: i32 = 123456789;

/// Configuration file extension
const CONFIG_EXTENSION: &str = ".lcf";

/// Main entry point for the launcher
///
/// Matches C++ main() function in Tools/Launcher/main.cpp
///
/// Flow:
/// 1. Parse command-line arguments
/// 2. Load configuration file
/// 3. Find and apply patches
/// 4. Launch game executable
/// 5. Handle update requests
fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Command & Conquer Generals Zero Hour Launcher");
    info!("Version: 0.1.0");

    // Get command-line arguments
    let args: Vec<String> = env::args().collect();

    if args.is_empty() {
        error!("No arguments provided");
        anyhow::bail!("Failed to get executable path");
    }

    // Get current working directory before changing it
    let original_cwd = env::current_dir().context("Failed to get current working directory")?;

    debug!("Original working directory: {:?}", original_cwd);
    debug!("Launcher executable: {:?}", args[0]);

    // Get the directory where the launcher is installed
    let launcher_path = Path::new(&args[0]);
    if let Some(launcher_dir) = launcher_path.parent() {
        env::set_current_dir(launcher_dir).with_context(|| {
            format!("Failed to change to launcher directory: {:?}", launcher_dir)
        })?;
        debug!("Changed to launcher directory: {:?}", launcher_dir);
    }

    // Determine config file name from executable name
    // Change extension from .exe to .lcf (Launcher ConFig)
    let config_name = get_config_filename(&args[0])?;
    info!("Configuration file: {}", config_name);

    // Load configuration
    if !Path::new(&config_name).exists() {
        error!("Configuration file not found: {}", config_name);
        eprintln!("You must run the game from its install directory.");
        eprintln!("Launcher config file missing: {}", config_name);
        std::process::exit(1);
    }

    let config_content = fs::read_to_string(&config_name)
        .with_context(|| format!("Failed to read config file: {}", config_name))?;

    debug!("Configuration loaded successfully");

    // Handle special "GrabPatches" mode
    if args.len() >= 2 && args[1] == "GrabPatches" {
        info!("Running in patch grab mode");
        run_patch_grab_mode(&config_content)?;
        env::set_current_dir(original_cwd)?;
        return Ok(());
    }

    // Normal launcher mode
    info!("Running in normal launcher mode");
    run_launcher_mode(&args, &config_content)?;

    // Restore original directory
    env::set_current_dir(original_cwd)?;

    info!("Launcher exiting normally");
    Ok(())
}

/// Get configuration filename from executable name
/// Changes extension from .exe to .lcf
fn get_config_filename(exe_path: &str) -> Result<String> {
    let path = Path::new(exe_path);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .context("Failed to extract filename stem")?;

    Ok(format!("{}{}", stem, CONFIG_EXTENSION))
}

/// Run in patch grab mode - download and apply patches only
fn run_patch_grab_mode(_config: &str) -> Result<()> {
    info!("Patch grab mode - checking for patches...");

    // In a full implementation, this would:
    // 1. Start the patchgrabber process (patchget.dat)
    // 2. Wait for it to complete
    // 3. Apply any downloaded patches

    warn!("Patch system not yet implemented");
    Ok(())
}

/// Run in normal launcher mode - main game execution
fn run_launcher_mode(args: &[String], _config: &str) -> Result<()> {
    info!("Starting game launcher...");

    // In a full implementation, this would:
    // 1. Find and apply any available patches
    // 2. Launch the game executable
    // 3. Wait for the game to exit
    // 4. Check if game requested patch update (exit code UPDATE_RETVAL)
    // 5. If so, download and apply patches, then re-launch

    // For now, just log the configuration
    debug!("Command-line arguments: {:?}", args);

    warn!("Game execution not yet implemented");
    warn!("This is a minimal launcher stub for compilation");

    Ok(())
}
