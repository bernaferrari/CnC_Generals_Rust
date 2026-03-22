use anyhow::Result;
use std::sync::Arc;
use winit;
use winit::event::{Event, WindowEvent};

/// Application focus state for coordinating subsystems
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ApplicationFocusState {
    Active,
    Inactive,
    Minimized,
    Maximized,
}

/// Power management events for laptop/mobile support
#[derive(Debug, Clone, Copy)]
pub enum PowerEvent {
    QuerySuspend,
    ResumeSuspend,
    PowerStatusChange,
    BatteryLow,
}

/// System command events
#[derive(Debug, Clone, Copy)]
pub enum SystemCommand {
    Move,
    Size,
    Maximize,
    Minimize,
    KeyMenu,
    MonitorPower,
    Close,
}

/// Window message handler trait - cross-platform equivalent of WndProc
pub trait WindowMessageHandler {
    /// Provide access to the native window so the handler can toggle cursor state, etc.
    fn attach_window(&mut self, _window: Arc<winit::window::Window>) {}

    /// Handle application focus changes (WM_ACTIVATEAPP, WM_ACTIVATE equivalent)
    fn handle_focus_change(&mut self, state: ApplicationFocusState, active: bool) -> Result<()>;

    /// Handle power management events (WM_POWERBROADCAST equivalent)  
    fn handle_power_event(&mut self, event: PowerEvent) -> Result<bool>;

    /// Handle system commands (WM_SYSCOMMAND equivalent)
    fn handle_system_command(
        &mut self,
        command: SystemCommand,
        in_fullscreen: bool,
    ) -> Result<bool>;

    /// Handle close requests (WM_CLOSE, WM_QUERYENDSESSION equivalent)
    fn handle_close_request(&mut self, is_session_ending: bool) -> Result<bool>;

    /// Handle window resize (WM_SIZE equivalent)
    fn handle_resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) -> Result<()>;

    /// Handle cursor management (WM_SETCURSOR equivalent)
    fn handle_cursor_request(&mut self) -> Result<bool>;

    /// Handle window paint requests (WM_PAINT equivalent)
    fn handle_paint_request(&mut self) -> Result<()>;

    /// Handle keyboard/mouse focus (WM_SETFOCUS, WM_KILLFOCUS equivalent)
    fn handle_input_focus(&mut self, gained: bool) -> Result<()>;

    /// Query whether the platform layer has requested application shutdown.
    fn is_quit_requested(&self) -> bool {
        false
    }
}

/// Window message event - abstraction over platform-specific events
#[derive(Debug)]
pub enum WindowMessage {
    FocusChanged {
        state: ApplicationFocusState,
        active: bool,
    },
    PowerEvent(PowerEvent),
    SystemCommand(SystemCommand),
    CloseRequest {
        session_ending: bool,
    },
    Resize(winit::dpi::PhysicalSize<u32>),
    CursorRequest,
    PaintRequest,
    InputFocus {
        gained: bool,
    },
}

/// Cross-platform window message processor
pub struct WindowMessageProcessor {
    handler: Box<dyn WindowMessageHandler + Send + Sync>,
    is_fullscreen: bool,
    is_active: bool,
    focus_state: ApplicationFocusState,
}

impl WindowMessageProcessor {
    pub fn new(handler: Box<dyn WindowMessageHandler + Send + Sync>) -> Self {
        Self {
            handler,
            is_fullscreen: false,
            is_active: true,
            focus_state: ApplicationFocusState::Active,
        }
    }

    /// Process winit events and translate to our message system
    pub fn process_event(&mut self, event: &Event<()>) -> Result<bool> {
        match event {
            Event::WindowEvent { event, .. } => self.process_window_event(event),
            Event::Suspended => {
                self.handler.handle_power_event(PowerEvent::QuerySuspend)?;
                Ok(false)
            }
            Event::Resumed => {
                self.handler.handle_power_event(PowerEvent::ResumeSuspend)?;
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn process_window_event(&mut self, event: &WindowEvent) -> Result<bool> {
        match event {
            WindowEvent::Focused(focused) => {
                let new_state = if *focused {
                    ApplicationFocusState::Active
                } else {
                    ApplicationFocusState::Inactive
                };

                if new_state != self.focus_state {
                    self.focus_state = new_state;
                    self.is_active = *focused;
                    self.handler.handle_focus_change(new_state, *focused)?;
                }
                Ok(false)
            }
            WindowEvent::CloseRequested => Ok(self.handler.handle_close_request(false)?),
            WindowEvent::Resized(size) => {
                self.handler.handle_resize(*size)?;
                Ok(false)
            }
            WindowEvent::CursorEntered { .. } => {
                self.handler.handle_input_focus(true)?;
                Ok(false)
            }
            WindowEvent::CursorLeft { .. } => {
                self.handler.handle_input_focus(false)?;
                Ok(false)
            }
            WindowEvent::RedrawRequested => {
                self.handler.handle_paint_request()?;
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.is_fullscreen = fullscreen;
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn get_focus_state(&self) -> ApplicationFocusState {
        self.focus_state
    }

    pub fn attach_window(&mut self, window: Arc<winit::window::Window>) {
        self.handler.attach_window(window);
    }

    pub fn is_quit_requested(&self) -> bool {
        self.handler.is_quit_requested()
    }
}

mod unified;
pub use unified::*;

// Platform-specific implementations
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

/// Create a platform-specific message handler
pub fn create_platform_message_handler() -> Box<dyn WindowMessageHandler + Send + Sync> {
    Box::new(GameMessageHandler::new())
}

/// Initialize platform-specific subsystems
pub fn initialize_platform() -> Result<()> {
    #[cfg(target_os = "windows")]
    windows::initialize()?;

    #[cfg(target_os = "macos")]
    macos::initialize()?;

    #[cfg(target_os = "linux")]
    linux::initialize()?;

    Ok(())
}

/// Shutdown platform-specific subsystems
pub fn shutdown_platform() {
    #[cfg(target_os = "windows")]
    windows::shutdown();

    #[cfg(target_os = "macos")]
    macos::shutdown();

    #[cfg(target_os = "linux")]
    linux::shutdown();
}
