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
use log::{debug, error, info, warn};
#[cfg(target_os = "windows")]
use raw_window_handle::HasWindowHandle;
use std::env;
use std::ffi::c_void;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::runtime::Runtime;
use winit::{
    self,
    dpi::{LogicalSize, PhysicalPosition},
    event_loop::EventLoop,
    window::{Fullscreen, Window, WindowAttributes, WindowLevel},
};

// Import the GameMain function from our game engine
// use crate::game_engine::GameMain; // Removed - use cnc_game_engine instead

// NOTE: These static variables are used for Win32 FFI integration
// and use AtomicPtr to maintain compatibility with Windows APIs.
// They represent opaque handles from the Windows API and are appropriate for FFI.

/// Application instance handle (equivalent to HINSTANCE)
pub static APPLICATION_INSTANCE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Application window handle (equivalent to HWND)
pub static APPLICATION_WINDOW: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Win32 mouse interface pointer
pub static THE_WIN32_MOUSE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Whether application is windowed
pub static APPLICATION_IS_WINDOWED: AtomicBool = AtomicBool::new(false);

/// Message time from Windows
pub static THE_MESSAGE_TIME: AtomicU32 = AtomicU32::new(0);
static LAUNCHER_SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

// Constants from C++
const GENERALS_GUID: &str = "685EAFF2-3216-4265-B047-251C5F4B82F3";
const DEFAULT_XRESOLUTION: i32 = 800;
const DEFAULT_YRESOLUTION: i32 = 600;
const STARTUP_WINDOW_TITLE: &str = "Command and Conquer Generals";

/// Windows main entry point - exact equivalent of C++ WinMain
pub unsafe fn win_main(
    h_instance: *mut c_void,
    _h_prev_instance: *mut c_void,
    _lp_cmd_line: *const c_char,
    n_cmd_show: c_int,
) -> c_int {
    APPLICATION_INSTANCE.store(h_instance, Ordering::Relaxed);

    if let Err(err) = set_working_directory_to_executable() {
        warn!("Failed to set working directory to executable path: {err}");
    }

    // Convert WinMain arguments to simple main argc and argv - exactly like C++
    let args = command_line::CommandLineArgs::startup_args();
    if command_line::CommandLineArgs::wants_dx_stack_dump_from_args(&args) {
        command_line::CommandLineArgs::emit_dx_stack_dump_from_args(&args);
        return 0;
    }
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
    if let Err(e) = init_memory_manager() {
        error!("Failed to initialize game memory: {}", e);
        cleanup_and_exit();
        return 0;
    }

    // Initialize version info with copy protection integration
    init_version();

    // Initialize copy protection system after version info, matching C++ WinMain.
    if let Err(e) = init_copy_protection() {
        error!("Failed to initialize copy protection: {}", e);
        cleanup_and_exit();
        return 0;
    }

    // Check if launcher is running (matching C++ CopyProtect::isLauncherRunning)
    if !check_launcher_status() {
        cleanup_and_exit();
        return 0;
    }

    // Create mutex to prevent multiple instances - exactly like C++
    if !create_generals_mutex() {
        cleanup_and_exit();
        return 0;
    }

    // Notify launcher of game start (matching C++ CopyProtect::notifyLauncher)
    if let Err(e) = notify_launcher_game_start() {
        error!("Could not talk to launcher: {e}");
        cleanup_and_exit();
        return 0;
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
    run_windowed: bool,
) -> bool {
    APPLICATION_IS_WINDOWED.store(run_windowed, Ordering::Relaxed);
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
unsafe fn init_memory_manager() -> Result<(), anyhow::Error> {
    // The C++ code initializes a custom memory manager here.
    game_engine::common::system::game_memory::init_memory_manager();
    Ok(())
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

fn set_working_directory_to_executable() -> anyhow::Result<()> {
    let exe_path = env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Executable path has no parent"))?;
    let current = env::current_dir()?;
    if current != exe_dir {
        env::set_current_dir(exe_dir)?;
    }

    if Path::new(".").canonicalize()? != exe_dir.canonicalize()? {
        return Err(anyhow::anyhow!(
            "Failed to normalize working directory to {}",
            exe_dir.display()
        ));
    }
    Ok(())
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
    let startup_args = command_line::CommandLineArgs::startup_args();
    let is_dev_mode = cfg!(debug_assertions) || startup_args.iter().any(|arg| arg == "--dev-mode");
    let is_enabled = !startup_args
        .iter()
        .any(|arg| arg == "--disable-copy-protection");

    info!(
        "Initializing copy protection: dev_mode={}, enabled={}",
        is_dev_mode, is_enabled
    );

    crate::copy_protection::configure_copy_protection(is_dev_mode, is_enabled);
    crate::copy_protection::initialize_copy_protection()
}

/// Check launcher status (matching C++ CopyProtect::isLauncherRunning)
unsafe fn check_launcher_status() -> bool {
    let launcher_running = crate::copy_protection::is_launcher_running();
    if launcher_running {
        info!("Launcher detected and running");
    } else {
        error!("Launcher is not running - about to bail");
    }

    launcher_running
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
    let (width, height) = resolve_startup_resolution(cmd_args);
    let (is_windowed, is_fullscreen) = resolve_window_mode(cmd_args);
    let startup_position = centered_startup_position(&event_loop, width, height);

    Ok((
        event_loop,
        startup_window_attributes(width, height, is_windowed, is_fullscreen, startup_position),
    ))
}

fn resolve_window_mode(cmd_args: &command_line::CommandLineArgs) -> (bool, bool) {
    // Match C++ parser behavior: last explicit mode flag wins by argument order.
    match cmd_args.last_window_mode_override() {
        Some(true) => (true, false),
        Some(false) => (false, true),
        None => {
            // Match the C++ WinMain default: fullscreen-style startup unless -win is present.
            (false, true)
        }
    }
}

fn centered_startup_position(
    _event_loop: &EventLoop<()>,
    _width: u32,
    _height: u32,
) -> Option<PhysicalPosition<i32>> {
    // winit 0.30 does not expose monitor geometry from EventLoop at bootstrap time.
    // Window centering is handled after creation via Window::primary_monitor().
    Some(PhysicalPosition::new(100, 100))
}

fn startup_window_attributes(
    width: u32,
    height: u32,
    is_windowed: bool,
    is_fullscreen: bool,
    startup_position: Option<PhysicalPosition<i32>>,
) -> WindowAttributes {
    let mut attributes = Window::default_attributes()
        .with_title(STARTUP_WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(width as f64, height as f64))
        .with_resizable(true)
        .with_maximized(false)
        .with_decorations(true)
        .with_visible(true)
        .with_window_level(if is_fullscreen {
            WindowLevel::AlwaysOnTop
        } else {
            WindowLevel::Normal
        });

    if let Some(position) = startup_position {
        attributes = attributes.with_position(position);
    }

    if is_fullscreen {
        attributes = attributes.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }

    if is_windowed {
        attributes = attributes.with_fullscreen(None);
    }

    attributes
}

#[cfg(test)]
mod tests {
    use super::{
        centered_startup_position_from_monitor, command_line, initialize_app_windows,
        resolve_window_mode, startup_window_attributes, APPLICATION_IS_WINDOWED,
        DEFAULT_XRESOLUTION, DEFAULT_YRESOLUTION, STARTUP_WINDOW_TITLE,
    };
    use std::sync::atomic::Ordering;
    use winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        window::{Fullscreen, WindowLevel},
    };

    #[test]
    fn last_explicit_window_mode_wins_for_winmain_startup_mode() {
        let args = vec![
            "generals".to_string(),
            "-fullscreen".to_string(),
            "-win".to_string(),
        ];

        let parsed = command_line::CommandLineArgs::parse_from_args(args).unwrap();
        assert_eq!(resolve_window_mode(&parsed), (true, false));

        let reverse = vec![
            "generals".to_string(),
            "-win".to_string(),
            "-fullscreen".to_string(),
        ];
        let parsed_reverse = command_line::CommandLineArgs::parse_from_args(reverse).unwrap();
        assert_eq!(resolve_window_mode(&parsed_reverse), (false, true));
    }

    #[test]
    fn startup_args_are_capped_for_winmain_parity() {
        let mut args = vec!["generals".to_string()];
        for index in 1..25 {
            args.push(format!("arg{index}"));
        }

        let capped = command_line::CommandLineArgs::limit_startup_args(args);
        assert_eq!(capped.len(), command_line::MAX_STARTUP_ARGS);
        assert_eq!(capped.last().map(String::as_str), Some("arg19"));
    }

    #[test]
    fn initialize_app_windows_tracks_startup_window_mode_flag() {
        APPLICATION_IS_WINDOWED.store(false, Ordering::Relaxed);

        unsafe {
            assert!(initialize_app_windows(std::ptr::null_mut(), 1, true));
        }
        assert!(APPLICATION_IS_WINDOWED.load(Ordering::Relaxed));

        unsafe {
            assert!(initialize_app_windows(std::ptr::null_mut(), 1, false));
        }
        assert!(!APPLICATION_IS_WINDOWED.load(Ordering::Relaxed));
    }

    #[test]
    fn centered_startup_position_matches_cpp_centering_math() {
        let centered = PhysicalPosition::new(570, 260);
        let helper_centered = centered_startup_position_from_monitor(
            PhysicalPosition::new(10, 20),
            PhysicalSize::new(1920, 1080),
            DEFAULT_XRESOLUTION as u32,
            DEFAULT_YRESOLUTION as u32,
        );
        assert_eq!(helper_centered, centered);
    }

    #[test]
    fn startup_window_attributes_match_cpp_semantics() {
        let attributes = startup_window_attributes(
            DEFAULT_XRESOLUTION as u32,
            DEFAULT_YRESOLUTION as u32,
            true,
            false,
            Some(PhysicalPosition::new(570, 260)),
        );

        assert_eq!(attributes.title, STARTUP_WINDOW_TITLE);
        assert_eq!(attributes.window_level, WindowLevel::Normal);
        assert_eq!(
            attributes.position,
            Some(PhysicalPosition::new(570, 260).into())
        );
        assert!(attributes.fullscreen.is_none());
        assert!(attributes.visible);
        assert!(attributes.decorations);
        assert!(attributes.resizable);
        assert!(!attributes.maximized);

        let fullscreen_attributes = startup_window_attributes(
            DEFAULT_XRESOLUTION as u32,
            DEFAULT_YRESOLUTION as u32,
            false,
            true,
            Some(PhysicalPosition::new(570, 260)),
        );

        assert_eq!(fullscreen_attributes.title, STARTUP_WINDOW_TITLE);
        assert_eq!(fullscreen_attributes.window_level, WindowLevel::AlwaysOnTop);
        assert!(matches!(
            fullscreen_attributes.fullscreen,
            Some(Fullscreen::Borderless(None))
        ));
    }
}

fn centered_startup_position_from_monitor(
    monitor_position: PhysicalPosition<i32>,
    monitor_size: winit::dpi::PhysicalSize<u32>,
    width: u32,
    height: u32,
) -> PhysicalPosition<i32> {
    PhysicalPosition::new(
        monitor_position.x + ((monitor_size.width as i32 - width as i32) / 2),
        monitor_position.y + ((monitor_size.height as i32 - height as i32) / 2),
    )
}

fn parse_u32_option(cmd_args: &command_line::CommandLineArgs, option: &str) -> Option<u32> {
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

fn resolve_startup_resolution(cmd_args: &command_line::CommandLineArgs) -> (u32, u32) {
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
