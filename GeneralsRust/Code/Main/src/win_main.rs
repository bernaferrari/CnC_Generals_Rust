////////////////////////////////////////////////////////////////////////////////
//
//  (c) 2001-2003 Electronic Arts Inc.
//
////////////////////////////////////////////////////////////////////////////////

// FILE: win_main.rs
//
// Entry point for game application
//
// Author: Colin Day, April 2001 (Converted to Rust)
//
///////////////////////////////////////////////////////////////////////////////

use crate::command_line;
use crate::runtime::attachments::AttachmentDispatcher;
use anyhow::{Context, Result};
use egui_winit::winit::{
    self,
    dpi::{LogicalSize, PhysicalPosition},
    event_loop::EventLoop,
    window::{Fullscreen, Window, WindowAttributes},
};
use log::{debug, error, info, warn};
#[cfg(target_os = "windows")]
use raw_window_handle::HasWindowHandle;
use std::env;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::runtime::Runtime;

// Import the GameMain function from our game engine
// use crate::game_engine::GameMain; // Removed - use cnc_game_engine instead

// NOTE: These static mut variables are used for Win32 FFI integration
// and must remain as raw pointers to maintain compatibility with Windows APIs.
// They represent opaque handles from the Windows API and are appropriate for FFI.

/// Application instance handle (equivalent to HINSTANCE)
/// SAFETY: This is only accessed from the main thread during Win32 initialization
pub static mut APPLICATION_INSTANCE: *mut c_void = std::ptr::null_mut();

/// Application window handle (equivalent to HWND)  
/// SAFETY: This is only accessed from the main thread during Win32 initialization
pub static mut APPLICATION_WINDOW: *mut c_void = std::ptr::null_mut();

/// Win32 mouse interface pointer
/// SAFETY: This is only accessed from the main thread during Win32 initialization
pub static mut THE_WIN32_MOUSE: *mut c_void = std::ptr::null_mut();

/// Whether application is windowed
pub static APPLICATION_IS_WINDOWED: AtomicBool = AtomicBool::new(false);

/// Message time from Windows
pub static THE_MESSAGE_TIME: AtomicU32 = AtomicU32::new(0);
static LAUNCHER_SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

// Constants from C++
const GENERALS_GUID: &str = "685EAFF2-3216-4265-B047-251C5F4B82F3";
const DEFAULT_XRESOLUTION: i32 = 800;
const DEFAULT_YRESOLUTION: i32 = 600;

/// Windows main entry point - exact equivalent of C++ WinMain
pub unsafe fn win_main(
    h_instance: *mut c_void,
    _h_prev_instance: *mut c_void,
    _lp_cmd_line: *const c_char,
    n_cmd_show: c_int,
) -> c_int {
    APPLICATION_INSTANCE = h_instance;

    // Convert WinMain arguments to simple main argc and argv - exactly like C++
    let args: Vec<String> = env::args().collect();
    let _argc = args.len() as c_int;

    // Create C-style argv (kept for parity/debug logging)
    let c_strings: Vec<CString> = args
        .iter()
        .map(|s| CString::new(s.as_str()).unwrap_or_else(|_| CString::new("").unwrap()))
        .collect();
    let mut _c_args: Vec<*mut c_char> = c_strings
        .iter()
        .map(|s| s.as_ptr() as *mut c_char)
        .collect();

    // Check for windowed mode flag - exactly like C++
    for arg in &args {
        if arg.to_lowercase() == "-win" {
            APPLICATION_IS_WINDOWED.store(true, Ordering::Relaxed);
        }
    }

    // Register windows class and create application window
    if !initialize_app_windows(
        h_instance,
        n_cmd_show,
        APPLICATION_IS_WINDOWED.load(Ordering::Relaxed),
    ) {
        return 0;
    }

    // Initialize debug system
    debug_init();
    init_memory_manager();

    // Initialize copy protection system (must be before version and mutex)
    if let Err(e) = init_copy_protection() {
        error!("Failed to initialize copy protection: {}", e);
        cleanup_and_exit();
        return 0;
    }

    // Initialize version info with copy protection integration
    init_version();

    // Check if launcher is running (matching C++ CopyProtect::isLauncherRunning)
    check_launcher_status();

    // Create mutex to prevent multiple instances - exactly like C++
    if !create_generals_mutex() {
        cleanup_and_exit();
        return 0;
    }

    // Notify launcher of game start (matching C++ CopyProtect::notifyLauncher)
    if let Err(e) = notify_launcher_game_start() {
        warn!("Failed to notify launcher of game start: {}", e);
        // Continue anyway - launcher notification failure shouldn't block game
    }

    // Run the actual game loop using the winit/wgpu pipeline for multi-platform parity.
    // This mirrors main.rs but is callable from the Win32 entry point.
    match launch_rts_runtime() {
        Ok(code) => {
            // Process copy protection messages during main loop
            // Note: In a real implementation, this would be integrated into the game loop
            process_copy_protection_messages();
            cleanup_and_exit();
            code
        }
        Err(e) => {
            error!("Failed to launch game from WinMain: {}", e);
            cleanup_and_exit();
            0
        }
    }
}

/// Initialize application windows - equivalent to C++ initializeAppWindows
unsafe fn initialize_app_windows(
    _h_instance: *mut c_void,
    _n_cmd_show: c_int,
    _run_windowed: bool,
) -> bool {
    true
}

/// Initialize debug system - equivalent to C++ DEBUG_INIT
unsafe fn debug_init() {
    // Match `main.rs` logging bootstrap for the WinMain entry point.
    // Use `try_init` to avoid panicking if another entry point already initialized logging.
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_module("wgpu_core", log::LevelFilter::Warn)
        .filter_module("wgpu_hal", log::LevelFilter::Warn)
        .try_init();

    // Initialize the in-engine debug/crash reporting system.
    if let Err(err) = crate::debug_system::initialize_debug_system(None) {
        error!("Failed to initialize debug system: {}", err);
    }
}

/// Initialize memory manager - equivalent to C++ initMemoryManager
unsafe fn init_memory_manager() {
    // The C++ code initializes a custom memory manager here.
    // Rust uses the global allocator and does not require explicit initialization.
}

/// Initialize version info - equivalent to C++ version setup
unsafe fn init_version() {
    crate::version::initialize_version_system_with_copy_protection();
}

/// Create Generals mutex - equivalent to C++ GeneralsMutex creation
unsafe fn create_generals_mutex() -> bool {
    crate::single_instance::create_generals_mutex()
}

/// Synchronous GameMain wrapper - equivalent to C++ GameMain call
unsafe fn game_main_sync(_argc: c_int, _argv: *mut *mut c_char) {
    // Historically dispatched WW3D attachments; now we rely on the real engine run path.
    let attachments = ww3d_renderer_3d::Renderer::with_global_mut(|renderer| {
        Ok(renderer.take_pending_attachments())
    })
    .unwrap_or_default();
    AttachmentDispatcher::dispatch(attachments);
}

/// Cleanup and exit - equivalent to C++ cleanup in WinMain
unsafe fn cleanup_and_exit() {
    // Shutdown copy protection system (matching C++ CopyProtect::shutdown)
    if let Err(e) = crate::copy_protection::shutdown() {
        error!("Failed to shutdown copy protection: {}", e);
    }

    // Match the WinMain cleanup ordering:
    // - TheVersion: cleaned up automatically by Rust drops.
    // - Memory manager: no-op (global allocator).
    // - Debug shutdown: flush file logs if configured.
    crate::debug_system::flush_debug_logs();

    debug!("Win32 cleanup completed");
}

/// Create game engine - equivalent to C++ CreateGameEngine
pub unsafe fn create_game_engine() -> *mut c_void {
    // WinMain historically returned a newly allocated engine pointer.
    // `Win32GameEngine` construction requires a live window/event loop, so for this FFI entry
    // point we return the cross-platform engine instance that can be initialized later.
    let engine = crate::game_engine::CrossPlatformGameEngine::new();
    Box::into_raw(Box::new(engine)) as *mut c_void
}

// Copy protection integration functions (matching C++ WinMain.cpp)

/// Initialize copy protection system (matching C++ CopyProtect initialization)
unsafe fn init_copy_protection() -> Result<()> {
    // Configure copy protection based on build type and command line arguments
    let is_dev_mode = cfg!(debug_assertions) || env::args().any(|arg| arg == "--dev-mode");
    let is_enabled = !env::args().any(|arg| arg == "--disable-copy-protection");

    info!(
        "Initializing copy protection: dev_mode={}, enabled={}",
        is_dev_mode, is_enabled
    );

    crate::copy_protection::configure_copy_protection(is_dev_mode, is_enabled);
    crate::copy_protection::initialize_copy_protection()
}

/// Check launcher status (matching C++ CopyProtect::isLauncherRunning)
unsafe fn check_launcher_status() {
    let launcher_running = crate::copy_protection::is_launcher_running();
    if launcher_running {
        info!("Launcher detected and running");
    } else {
        debug!("Launcher not detected or running in development mode");
    }
}

/// Notify launcher of game start (matching C++ CopyProtect::notifyLauncher)
unsafe fn notify_launcher_game_start() -> Result<()> {
    crate::copy_protection::notify_launcher()
}

/// Check for launcher messages during game loop (matching C++ CopyProtect::checkForMessage)
/// This would typically be called from the main game loop
pub unsafe fn check_for_launcher_messages() -> Result<()> {
    if let Some(message) = crate::copy_protection::check_for_message()? {
        match message {
            crate::copy_protection::LauncherMessage::GameShutdown { .. } => {
                info!("Received shutdown message from launcher");
                LAUNCHER_SHUTDOWN_REQUESTED.store(true, Ordering::Relaxed);
            }
            crate::copy_protection::LauncherMessage::VersionCheck { .. } => {
                info!("Received version check message from launcher");
                let version = crate::version::get_version_for_copy_protection();
                if let Err(err) = crate::copy_protection::notify_launcher_version(&version) {
                    warn!("Failed to respond to launcher version check: {}", err);
                }
            }
            _ => {
                debug!("Received launcher message: {:?}", message);
            }
        }
    }
    Ok(())
}

/// Process copy protection messages (should be called periodically during game execution)
pub unsafe fn process_copy_protection_messages() {
    if let Err(e) = check_for_launcher_messages() {
        warn!("Error processing copy protection messages: {}", e);
    }
}

pub fn is_launcher_shutdown_requested() -> bool {
    LAUNCHER_SHUTDOWN_REQUESTED.load(Ordering::Relaxed)
}

/// Build window + event loop and run the cross-platform RTS path from WinMain.
fn launch_rts_runtime() -> Result<c_int> {
    let rt = Runtime::new().context("Failed to create Tokio runtime for WinMain path")?;
    let exit_code = rt.block_on(async {
        let cmd_args = command_line::initialize_command_line()
            .map_err(|e| anyhow::anyhow!("Failed to parse command line: {e}"))?;

        if let Some(lang) = cmd_args.language.as_ref() {
            crate::localization::init(lang);
        }

        initialize_logger(&cmd_args);

        let (event_loop, window_attributes) = build_window_attributes(&cmd_args)
            .map_err(|e| anyhow::anyhow!("Failed to configure window: {e}"))?;

        crate::cnc_game_engine::run_cnc_game(event_loop, window_attributes, Arc::new(cmd_args))
            .await
            .map_err(|e| anyhow::anyhow!("Game failed: {e}"))?;

        Ok::<c_int, anyhow::Error>(0)
    })?;

    Ok(exit_code)
}

fn build_window_attributes(
    cmd_args: &command_line::CommandLineArgs,
) -> Result<(EventLoop<()>, WindowAttributes)> {
    let event_loop = EventLoop::new().context("Failed to create event loop")?;
    let (width, height) = cmd_args.get_resolution();

    let mut attributes = Window::default_attributes()
        .with_title("C&C Generals - Rust")
        .with_inner_size(LogicalSize::new(width as f64, height as f64))
        .with_resizable(true)
        .with_maximized(false)
        .with_decorations(true)
        .with_visible(true);

    if cmd_args.fullscreen {
        attributes = attributes.with_fullscreen(Some(Fullscreen::Borderless(None)));
    } else {
        attributes = attributes.with_position(PhysicalPosition::new(100, 100));
    }

    if cmd_args.windowed {
        attributes = attributes.with_fullscreen(None);
    }

    Ok((event_loop, attributes))
}

fn initialize_logger(cmd_args: &command_line::CommandLineArgs) {
    let level = parse_level(&cmd_args.get_log_level());
    let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .filter_level(level)
        .filter_module("generals_main::graphics", log::LevelFilter::Warn)
        .filter_module("generals_main::assets::models", log::LevelFilter::Debug)
        .try_init();
}

fn parse_level(level: &str) -> log::LevelFilter {
    match level.to_lowercase().as_str() {
        "error" => log::LevelFilter::Error,
        "warn" | "warning" => log::LevelFilter::Warn,
        "debug" => log::LevelFilter::Debug,
        "trace" => log::LevelFilter::Trace,
        _ => log::LevelFilter::Info,
    }
}
