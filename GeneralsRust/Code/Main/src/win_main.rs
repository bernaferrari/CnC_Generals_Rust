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

use anyhow::{Context, Result};
use winit::{
    self,
    dpi::{LogicalSize, PhysicalPosition},
    event_loop::EventLoop,
    window::{Fullscreen, Window, WindowAttributes, WindowLevel},
};

// Constants from C++.
const DEFAULT_XRESOLUTION: i32 = 800;
const DEFAULT_YRESOLUTION: i32 = 600;
const STARTUP_WINDOW_TITLE: &str = "Command and Conquer Generals";


/// Build the host EventLoop. Windowed macOS must be Regular (not Accessory)
/// so `set_visible(true)` actually shows an OS window for CGEvent clicks.
/// Headless stays the default policy so the process can remain hidden.
pub fn create_host_event_loop(headless: bool) -> Result<EventLoop<()>> {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        let mut builder = EventLoop::builder();
        if !headless {
            builder
                .with_activation_policy(ActivationPolicy::Regular)
                .with_activate_ignoring_other_apps(true);
        }
        builder.build().context("Failed to create event loop")
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = headless;
        EventLoop::new().context("Failed to create event loop")
    }
}


pub fn centered_startup_position(
    _event_loop: &EventLoop<()>,
    width: u32,
    height: u32,
) -> Option<PhysicalPosition<i32>> {
    let (origin_x, origin_y, screen_w, screen_h) = primary_screen_geometry();
    Some(centered_startup_position_from_monitor(
        PhysicalPosition::new(origin_x, origin_y),
        winit::dpi::PhysicalSize::new(screen_w, screen_h),
        width,
        height,
    ))
}

fn primary_screen_geometry() -> (i32, i32, u32, u32) {
    #[cfg(target_os = "macos")]
    {
        #[repr(C)]
        struct CGPoint {
            x: f64,
            y: f64,
        }
        #[repr(C)]
        struct CGSize {
            width: f64,
            height: f64,
        }
        #[repr(C)]
        struct CGRect {
            origin: CGPoint,
            size: CGSize,
        }
        #[link(name = "CoreGraphics", kind = "framework")]
        // SAFETY: CoreGraphics C ABI declared with correct signatures;
        // calls below pass no pointers that outlive the expression.
        unsafe extern "C" {
            fn CGMainDisplayID() -> u32;
            fn CGDisplayBounds(display: u32) -> CGRect;
        }
        // SAFETY: CGMainDisplayID/CGDisplayBounds are thread-safe
        // queries returning values by copy; CGRect layout matches
        // the repr(C) mirror above.
        unsafe {
            let bounds = CGDisplayBounds(CGMainDisplayID());
            return (
                bounds.origin.x as i32,
                bounds.origin.y as i32,
                bounds.size.width.max(1.0) as u32,
                bounds.size.height.max(1.0) as u32,
            );
        }
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
        };
        // SAFETY: GetSystemMetrics takes an index and returns i32;
        // SM_CXSCREEN/SM_CYSCREEN indices are documented constants.
        unsafe {
            return (
                0,
                0,
                GetSystemMetrics(SM_CXSCREEN).max(1) as u32,
                GetSystemMetrics(SM_CYSCREEN).max(1) as u32,
            );
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        (0, 0, 1920, 1080)
    }
}

pub fn startup_window_attributes(
    width: u32,
    height: u32,
    is_windowed: bool,
    is_fullscreen: bool,
    startup_position: Option<PhysicalPosition<i32>>,
) -> WindowAttributes {
    // C++ initializeAppWindows: WS_POPUP (not resizable). Decorated caption
    // only with -win. Fullscreen is HWND_TOPMOST popup.
    let mut attributes = Window::default_attributes()
        .with_title(STARTUP_WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(width as f64, height as f64))
        .with_resizable(false)
        .with_maximized(false)
        .with_decorations(is_windowed)
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


#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_XRESOLUTION, DEFAULT_YRESOLUTION, STARTUP_WINDOW_TITLE,
        centered_startup_position_from_monitor, startup_window_attributes,
    };
    use crate::command_line;
    use winit::{
        dpi::{PhysicalPosition, PhysicalSize},
        window::{Fullscreen, WindowLevel},
    };

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
        assert!(!attributes.resizable);
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
