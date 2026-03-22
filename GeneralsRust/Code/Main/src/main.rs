////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: main.rs
//
// Main entry point for game application (equivalent to WinMain.cpp)
//
// This file provides the cross-platform Rust entry point that matches the
// initialization sequence from the C++ WinMain.cpp, including:
//   1. Command line parsing
//   2. Logging initialization
//   3. Localization setup
//   4. Window creation (winit)
//   5. Graphics context setup (wgpu)
//   6. Audio system initialization
//   7. Debug system initialization
//   8. Memory manager setup
//   9. Copy protection integration
//  10. Version info initialization
//  11. Single instance check (mutex)
//  12. Subsystem initialization
//  13. Game loop execution
//  14. Proper shutdown sequence
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use game_engine::common::system::game_memory::init_game_memory;
use generals_main::command_line::{self, CommandLineArgs};
use generals_main::subsystem_manager;
use log::{debug, error, info, warn, LevelFilter};
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event_loop::EventLoop,
    window::{Fullscreen, Window, WindowAttributes},
};

/// Game state machine - matches C++ GameEngine states
/// These states control the main game loop flow and determine which
/// subsystems are active and how input/rendering is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    /// Initial state - showing main menu, no game loaded
    /// Subsystems: UI, Input, Audio (menu music)
    Menu,

    /// Loading game assets and initializing game session
    /// Shows loading screen, loads map data, initializes game systems
    /// Subsystems: All systems initializing
    Loading,

    /// Active gameplay - game logic running, player can interact
    /// All subsystems active, game logic updating, network syncing
    Playing,

    /// Game paused - logic frozen, UI overlay showing pause menu
    /// Subsystems: UI active, GameLogic frozen, Network idle
    Paused,

    /// Shutting down - cleaning up resources before exit
    /// All subsystems shutting down in reverse order
    Exiting,
}

/// Frame timing state - matches C++ GameEngine frame limiter
/// Tracks timing information for frame rate limiting and delta time calculation
pub struct FrameTimer {
    /// Target frames per second (matches m_maxFPS in C++ GameEngine)
    target_fps: u32,

    /// Target frame duration in milliseconds
    frame_duration_ms: u32,

    /// Last frame timestamp
    last_frame_time: Instant,

    /// Frame counter for statistics
    frame_count: u64,

    /// Accumulated time for FPS calculation
    fps_accumulator: Duration,

    /// Current FPS reading
    current_fps: f32,

    /// Whether frame limiting is enabled (matches TheGlobalData->m_useFpsLimit)
    limit_enabled: bool,
}

impl FrameTimer {
    /// Create new frame timer with target FPS
    /// Matches C++ GameEngine::setFramesPerSecondLimit()
    pub fn new(target_fps: u32) -> Self {
        let frame_duration_ms = if target_fps > 0 {
            1000 / target_fps
        } else {
            16 // Default to ~60 FPS
        };

        Self {
            target_fps,
            frame_duration_ms,
            last_frame_time: Instant::now(),
            frame_count: 0,
            fps_accumulator: Duration::ZERO,
            current_fps: 0.0,
            limit_enabled: true,
        }
    }

    /// Begin new frame, returns delta time since last frame
    /// Matches C++ GameEngine::execute() frame timing logic
    pub fn begin_frame(&mut self) -> Duration {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;
        self.frame_count += 1;

        // Update FPS counter
        self.fps_accumulator += delta;
        if self.fps_accumulator.as_secs() >= 1 {
            self.current_fps = self.frame_count as f32 / self.fps_accumulator.as_secs_f32();
            self.frame_count = 0;
            self.fps_accumulator = Duration::ZERO;
        }

        delta
    }

    /// Wait to maintain target frame rate
    /// Matches C++ GameEngine::execute() frame limiting with Sleep(0) and timeGetTime()
    pub fn limit_frame_rate(&self) {
        if !self.limit_enabled || self.target_fps == 0 {
            return;
        }

        let elapsed = self.last_frame_time.elapsed();
        let target_duration = Duration::from_millis(self.frame_duration_ms as u64);

        if elapsed < target_duration {
            let sleep_duration = target_duration - elapsed;
            // Use spin loop for precise timing (matches C++ Sleep(0) loop)
            let deadline = Instant::now() + sleep_duration;
            while Instant::now() < deadline {
                std::thread::yield_now();
            }
        }
    }

    /// Get current FPS reading
    pub fn get_fps(&self) -> f32 {
        self.current_fps
    }

    /// Enable or disable frame limiting
    pub fn set_limit_enabled(&mut self, enabled: bool) {
        self.limit_enabled = enabled;
    }
}

/// Parse log level string to LevelFilter
/// Matches C++ debug levels: ERROR, WARN, INFO, DEBUG, TRACE
fn parse_level(level: &str) -> LevelFilter {
    match level.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" | "warning" => LevelFilter::Warn,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    }
}

/// Main entry point - equivalent to WinMain() in C++ version
///
/// Initialization sequence (matches C++ WinMain.cpp lines 850-1043):
/// 1. Parse command line arguments
/// 2. Initialize logging system (DEBUG_INIT)
/// 3. Initialize memory manager
/// 4. Setup version information
/// 5. Check copy protection and launcher status
/// 6. Create single instance mutex
/// 7. Create window and event loop
/// 8. Initialize subsystems in correct order
/// 9. Run game loop (GameMain)
/// 10. Clean shutdown
#[tokio::main]
async fn main() {
    if let Err(err) = set_working_directory_to_executable() {
        warn!(
            "Failed to set working directory to executable path: {}",
            err
        );
    }

    // =========================================================================
    // PHASE 1: COMMAND LINE PARSING (matches WinMain.cpp:893-906)
    // =========================================================================
    let cmd_args = match command_line::initialize_command_line() {
        Ok(args) => args,
        Err(err) => {
            eprintln!("Failed to parse command line: {err:?}");
            std::process::exit(1);
        }
    };

    // Check for help flag
    if cmd_args.wants_help() {
        CommandLineArgs::print_help();
        return;
    }

    // =========================================================================
    // PHASE 2: LOCALIZATION SETUP
    // =========================================================================
    let initial_language = cmd_args
        .language
        .clone()
        .unwrap_or_else(|| "English".to_string());
    generals_main::localization::init(&initial_language);
    info!("Language set to: {}", initial_language);

    // =========================================================================
    // PHASE 3: LOGGING INITIALIZATION (matches DEBUG_INIT, WinMain.cpp:978)
    // =========================================================================
    let level = parse_level(&cmd_args.get_log_level());
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_level(level)
        .filter_module("generals_main::graphics", log::LevelFilter::Warn) // Reduce graphics noise
        .filter_module("generals_main::assets::models", log::LevelFilter::Info)
        .filter_module(
            "game_engine::common::system::big_file_system",
            log::LevelFilter::Warn,
        )
        .filter_module("wgpu_core", log::LevelFilter::Warn) // Reduce wgpu verbosity
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
        .init();

    // =========================================================================
    // PHASE 4: BANNER AND VERSION INFO
    // =========================================================================
    println!("================================================================================");
    println!("  Command & Conquer Generals Zero Hour");
    println!("  Rust Port - Version 0.1.0");
    println!("  (c) 2001-2003 Electronic Arts Inc.");
    println!("  GPL v3 License");
    println!("================================================================================");
    std::io::stdout().flush().unwrap();

    info!("Starting game initialization sequence...");

    // =========================================================================
    // PHASE 5: MEMORY MANAGER INITIALIZATION (matches WinMain.cpp:982-985)
    // =========================================================================
    if let Err(e) = init_game_memory() {
        error!("Failed to initialize game memory: {e}");
        cleanup_and_exit();
        std::process::exit(1);
    }
    debug!("Game memory initialized");

    // =========================================================================
    // PHASE 6: VERSION SYSTEM INITIALIZATION (matches WinMain.cpp:982-986)
    // =========================================================================
    generals_main::version::initialize_version_system_with_copy_protection();
    debug!("Version system initialized");

    // =========================================================================
    // PHASE 6: COPY PROTECTION CHECK (matches WinMain.cpp:988-998)
    // =========================================================================
    #[cfg(feature = "copy-protection")]
    {
        unsafe {
            let is_dev_mode = cfg!(debug_assertions) || cmd_args.developer_mode;
            let is_enabled = !std::env::args().any(|arg| arg == "--disable-copy-protection");

            generals_main::copy_protection::configure_copy_protection(is_dev_mode, is_enabled);

            if let Err(e) = generals_main::copy_protection::initialize_copy_protection() {
                error!("Copy protection initialization failed: {}", e);
                cleanup_and_exit();
                std::process::exit(1);
            }

            if !generals_main::copy_protection::is_launcher_running() {
                error!("Launcher is not running - exiting");
                cleanup_and_exit();
                std::process::exit(1);
            }

            debug!("Copy protection initialized successfully");
        }
    }

    // =========================================================================
    // PHASE 7: SINGLE INSTANCE CHECK (matches WinMain.cpp:1001-1026)
    // =========================================================================
    if !generals_main::single_instance::create_generals_mutex() {
        warn!("Another instance of Generals is already running");
        cleanup_and_exit();
        std::process::exit(0);
    }
    debug!("Single instance mutex created");

    // =========================================================================
    // PHASE 8: LAUNCHER NOTIFICATION (matches WinMain.cpp:1028-1038)
    // =========================================================================
    #[cfg(feature = "copy-protection")]
    {
        unsafe {
            if let Err(e) = generals_main::copy_protection::notify_launcher() {
                error!("Could not communicate with launcher: {}", e);
                cleanup_and_exit();
                std::process::exit(0);
            }
            debug!("Launcher notified successfully");
        }
    }

    // =========================================================================
    // PHASE 9: EVENT LOOP AND WINDOW CREATION (matches initializeAppWindows)
    // =========================================================================
    info!("Creating event loop and window...");

    // Create EventLoop (only one allowed per application)
    let event_loop = match EventLoop::new() {
        Ok(event_loop) => event_loop,
        Err(e) => {
            error!("Failed to create event loop: {}", e);
            cleanup_and_exit();
            std::process::exit(1);
        }
    };

    let (is_windowed, is_fullscreen) = resolve_window_mode(&cmd_args);
    let (width, height) = resolve_startup_resolution(&cmd_args);
    let mut window_attributes = Window::default_attributes()
        .with_title("Command & Conquer Generals - Rust Edition")
        .with_inner_size(LogicalSize::new(width as f64, height as f64))
        .with_resizable(true)
        .with_maximized(false)
        .with_decorations(true) // Ensure window has title bar
        .with_visible(true);

    // Handle fullscreen/windowed mode (matches ApplicationIsWindowed flag)
    if is_fullscreen {
        window_attributes = window_attributes.with_fullscreen(Some(Fullscreen::Borderless(None)));
    } else {
        window_attributes = window_attributes.with_position(PhysicalPosition::new(100, 100));
    }

    if is_windowed {
        window_attributes = window_attributes.with_fullscreen(None);
    }

    // =========================================================================
    // PHASE 11: GAME MAIN LOOP (matches GameMain call, WinMain.cpp:1043)
    // Subsystem initialization is owned by CnCGameEngine::new(), matching C++ ordering.
    // =========================================================================
    info!("Starting game main loop...");
    std::io::stdout().flush().unwrap();

    // Call GameMain (matching C++ pattern: WinMain -> GameMain)
    let cmd_args = Arc::new(cmd_args);
    if let Err(e) = game_main(cmd_args, event_loop, window_attributes).await {
        error!("Game failed: {}", e);
        cleanup_and_exit();
        std::process::exit(1);
    }

    // =========================================================================
    // PHASE 12: CLEAN SHUTDOWN
    // =========================================================================
    info!("Game exited successfully");
    cleanup_and_exit();
}

/// GameMain - equivalent to GameMain() in C++ GameMain.cpp
///
/// Creates the game engine, initializes it, runs it, then cleans up.
/// This is the main game loop that handles all rendering, input, and logic updates.
///
/// # Arguments
/// * `cmd_args` - Parsed command line arguments
/// * `window` - The main application window (winit)
/// * `event_loop` - The event loop for handling OS events
///
/// # Returns
/// * `Ok(())` on successful execution and clean shutdown
/// * `Err(e)` on any error during game execution
async fn game_main(
    cmd_args: Arc<CommandLineArgs>,
    event_loop: EventLoop<()>,
    window_attributes: WindowAttributes,
) -> anyhow::Result<()> {
    info!("Entering game main loop...");

    // Run the main CNC game engine (which handles all initialization including assets)
    // This runs the complete game loop including:
    // - Asset loading from BIG files
    // - Graphics rendering (wgpu)
    // - Audio playback (rodio)
    // - Input handling (winit events)
    // - Game logic updates
    // - UI rendering
    // - Network synchronization
    // - Save/Load system
    generals_main::cnc_game_engine::run_cnc_game(event_loop, window_attributes, cmd_args).await?;

    // Cleanup happens automatically with Rust RAII
    info!("Game main loop completed");
    Ok(())
}

/// Cleanup and exit - equivalent to C++ cleanup in WinMain
///
/// Performs orderly shutdown of all systems (matches WinMain.cpp cleanup):
/// 1. Shutdown copy protection system
/// 2. Close single instance mutex
/// 3. Shutdown subsystems
/// 4. Shutdown memory manager
/// 5. Close debug log
///
/// This function ensures all resources are properly released before exit.
fn cleanup_and_exit() {
    debug!("Starting cleanup sequence...");

    // =========================================================================
    // STEP 1: Copy Protection Shutdown (matches WinMain.cpp:1045-1048)
    // =========================================================================
    #[cfg(feature = "copy-protection")]
    {
        unsafe {
            if let Err(e) = generals_main::copy_protection::shutdown() {
                error!("Failed to shutdown copy protection: {}", e);
            } else {
                debug!("Copy protection shutdown complete");
            }
        }
    }

    // =========================================================================
    // STEP 2: Close Single Instance Mutex
    // =========================================================================
    // Mutex is automatically closed when process exits (handled by OS)
    debug!("Single instance mutex will be released on exit");

    // =========================================================================
    // STEP 3: Shutdown Subsystems
    // =========================================================================
    if let Err(e) = subsystem_manager::shutdown_subsystem_manager() {
        error!("Failed to shutdown subsystem manager: {}", e);
    } else {
        debug!("Subsystem manager shutdown complete");
    }

    // =========================================================================
    // STEP 4: Version System Cleanup (matches delete TheVersion)
    // =========================================================================
    // Version system cleanup happens automatically in Rust

    // =========================================================================
    // STEP 5: Memory Manager Shutdown (matches shutdownMemoryManager())
    // =========================================================================
    // Rust's memory management handles this automatically

    // =========================================================================
    // STEP 6: Debug System Shutdown (matches DEBUG_SHUTDOWN())
    // =========================================================================
    debug!("Cleanup sequence completed");

    // Flush any remaining log messages
    log::logger().flush();
}

fn set_working_directory_to_executable() -> anyhow::Result<()> {
    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Executable path has no parent"))?;
    let current = std::env::current_dir()?;
    if current != exe_dir {
        std::env::set_current_dir(exe_dir)?;
    }

    if Path::new(".").canonicalize()? != exe_dir.canonicalize()? {
        return Err(anyhow::anyhow!(
            "Failed to normalize working directory to {}",
            exe_dir.display()
        ));
    }
    Ok(())
}

fn resolve_window_mode(cmd_args: &CommandLineArgs) -> (bool, bool) {
    // Match C++ parser behavior: last explicit mode flag wins by argument order.
    match cmd_args.last_window_mode_override() {
        Some(true) => (true, false),
        Some(false) => (false, true),
        None => {
            // C++ WinMain defaults to fullscreen-style startup unless -win is supplied.
            (false, true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{resolve_window_mode, CommandLineArgs};

    #[test]
    fn last_explicit_window_mode_wins_for_startup_mode() {
        let args = vec![
            "generals".to_string(),
            "-fullscreen".to_string(),
            "-win".to_string(),
        ];

        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(resolve_window_mode(&parsed), (true, false));

        let reverse = vec![
            "generals".to_string(),
            "-win".to_string(),
            "-fullscreen".to_string(),
        ];
        let parsed_reverse = CommandLineArgs::parse_from_args(reverse).unwrap();
        assert_eq!(resolve_window_mode(&parsed_reverse), (false, true));
    }
}

fn parse_u32_option(cmd_args: &CommandLineArgs, option: &str) -> Option<u32> {
    let Some(value) = cmd_args.get_option_value(option) else {
        return None;
    };

    match value.parse::<u32>() {
        Ok(0) => None,
        Ok(value) => Some(value),
        Err(err) => {
            warn!("Ignoring invalid {} value '{}': {err}", option, value);
            None
        }
    }
}

fn resolve_startup_resolution(cmd_args: &CommandLineArgs) -> (u32, u32) {
    const DEFAULT_XRESOLUTION: u32 = 800;
    const DEFAULT_YRESOLUTION: u32 = 600;

    let explicit_width = cmd_args
        .width
        .or_else(|| parse_u32_option(cmd_args, "xres"));
    let explicit_height = cmd_args
        .height
        .or_else(|| parse_u32_option(cmd_args, "yres"));

    match (explicit_width, explicit_height) {
        (Some(width), Some(height)) => (width, height),
        (Some(width), None) => (width, DEFAULT_YRESOLUTION),
        (None, Some(height)) => (DEFAULT_XRESOLUTION, height),
        (None, None) => (DEFAULT_XRESOLUTION, DEFAULT_YRESOLUTION),
    }
}
