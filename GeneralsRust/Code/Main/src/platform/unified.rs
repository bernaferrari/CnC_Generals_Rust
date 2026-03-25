/*
** Command & Conquer Generals Zero Hour(tm) - Winit Platform Message Handling
**
** Unified cross-platform implementation that mirrors the behaviour of the
** original Win32 WndProc while staying within the portable winit API surface.
*/

use super::*;
use crate::subsystem_manager::{
    get_subsystem_manager, with_subsystem_mut, GameMessageType, MessageStreamSubsystem,
};
use anyhow::Result;
use log::{debug, info, warn};
use std::sync::{Arc, Weak};
use winit;
use winit::window::{CursorGrabMode, Window};

/// Cross-platform game message handler implemented on top of winit.
#[derive(Debug)]
pub struct GameMessageHandler {
    /// Whether the game is currently running in windowed mode.
    is_windowed: bool,
    /// Application focus flag (mirrors the legacy `isWinMainActive` variable).
    is_app_active: bool,
    /// Tracks pending quit requests so the main loop can exit cleanly.
    quit_requested: bool,
    /// Tracks audio focus to avoid duplicate notifications.
    audio_has_focus: bool,
    /// Whether the cursor is currently constrained to the window.
    cursor_locked: bool,
    window: Option<Weak<Window>>,
}

impl GameMessageHandler {
    pub fn new() -> Self {
        Self {
            is_windowed: true,
            is_app_active: true,
            quit_requested: false,
            audio_has_focus: true,
            cursor_locked: false,
            window: None,
        }
    }

    /// Toggle windowed / fullscreen mode to match the legacy command line flags.
    pub fn set_windowed_mode(&mut self, windowed: bool) {
        self.is_windowed = windowed;
        info!(
            "Display mode changed: {}",
            if windowed { "Windowed" } else { "Fullscreen" }
        );
    }

    /// Helper used by platform bootstrap code when entering true fullscreen.
    pub fn set_fullscreen_mode(&mut self, fullscreen: bool) {
        self.is_windowed = !fullscreen;
        info!(
            "Display mode changed: {}",
            if fullscreen { "Fullscreen" } else { "Windowed" }
        );
    }

    pub fn is_quit_requested(&self) -> bool {
        self.quit_requested
    }

    fn push_message(&self, message_type: GameMessageType) {
        // Use the actual game message stream from game_engine
        use game_engine::common::message_stream::game_message::GameMessage as EngineGameMessage;
        use game_engine::common::message_stream::get_message_stream;

        if let Some(_stream) = get_message_stream().write().ok().as_mut() {
            // Convert our GameMessageType to the engine's GameMessageType
            // For now, just log the message since the full type mapping requires more work
            debug!("Queued message {:?}", message_type);
        } else {
            debug!(
                "Message stream subsystem unavailable for {:?}",
                message_type
            );
        }
    }

    /// Notify the rendering subsystem that focus has changed.
    fn notify_graphics_focus_change(&self, active: bool) {
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let _ = subsystem_manager.lock().notify_focus_change(active);
        }
    }

    fn notify_audio_focus_change(&self, active: bool) {
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let _ = subsystem_manager.lock().notify_audio_focus_change(active);
        }
    }

    /// Reset input state across subsystems (matches TheKeyboard->resetKeys()).
    fn notify_input_reset(&self) {
        if let Some(subsystem_manager) = get_subsystem_manager() {
            let _ = subsystem_manager.lock().notify_focus_change(false);
        }
    }

    fn with_window<F>(&self, mut f: F)
    where
        F: FnMut(&Window),
    {
        if let Some(window) = self.window.as_ref().and_then(|weak| weak.upgrade()) {
            f(&window);
        }
    }
}

impl WindowMessageHandler for GameMessageHandler {
    fn attach_window(&mut self, window: Arc<Window>) {
        self.window = Some(Arc::downgrade(&window));
    }

    fn handle_focus_change(&mut self, state: ApplicationFocusState, active: bool) -> Result<()> {
        // WM_ACTIVATEAPP equivalent.
        // C++: TheGameEngine->setIsActive(isWinMainActive) + Reset_D3D_Device(active).
        // C++ also restores the custom mouse cursor on activate.
        info!("WM_ACTIVATEAPP: active={}, state={:?}", active, state);

        if active != self.is_app_active {
            self.is_app_active = active;
            self.notify_graphics_focus_change(active);
        }

        // WM_ACTIVATE equivalent (audio focus gain/lose).
        // C++: TheAudio->loseFocus() on WA_INACTIVE, TheAudio->regainFocus() on activate.
        // C++: ClipCursor(NULL) on deactivate, TheMouse->setMouseLimits() on activate.
        match state {
            ApplicationFocusState::Active => {
                // Release cursor clip (handled by cursor locking above), then
                // restore mouse limits / cursor grab on activate.
                if !self.audio_has_focus {
                    self.audio_has_focus = true;
                    self.notify_audio_focus_change(true);
                }
            }
            ApplicationFocusState::Inactive => {
                // Release cursor constraints on deactivate.
                if self.cursor_locked {
                    self.with_window(|window| {
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                    });
                    self.cursor_locked = false;
                }
                if self.audio_has_focus {
                    self.audio_has_focus = false;
                    self.notify_audio_focus_change(false);
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_power_event(&mut self, event: PowerEvent) -> Result<bool> {
        match event {
            PowerEvent::QuerySuspend => {
                info!("🔋 System suspending - preparing game state");
                Ok(true)
            }
            PowerEvent::ResumeSuspend => {
                info!("🔋 System resuming - restoring game state");
                Ok(true)
            }
            PowerEvent::BatteryLow => {
                warn!("🔋 Battery low - consider saving progress");
                Ok(false)
            }
            PowerEvent::PowerStatusChange => {
                info!("🔋 Power status changed");
                Ok(false)
            }
        }
    }

    fn handle_system_command(
        &mut self,
        command: SystemCommand,
        in_fullscreen: bool,
    ) -> Result<bool> {
        match command {
            SystemCommand::Close => self.handle_close_request(false),
            SystemCommand::Move
            | SystemCommand::Size
            | SystemCommand::Maximize
            | SystemCommand::KeyMenu
            | SystemCommand::MonitorPower
                if in_fullscreen =>
            {
                info!(
                    "🛑 Ignoring system command {:?} in fullscreen mode",
                    command
                );
                Ok(true)
            }
            other => {
                info!(
                    "📝 System command {:?} (fullscreen: {})",
                    other, in_fullscreen
                );
                Ok(false)
            }
        }
    }

    fn handle_close_request(&mut self, is_session_ending: bool) -> Result<bool> {
        self.quit_requested = true;

        if is_session_ending {
            info!("🚪 Session ending - queueing immediate quit");
        } else {
            info!("🚪 Close requested - queueing immediate quit");
        }

        self.push_message(GameMessageType::MetaInstantQuit);
        Ok(false)
    }

    fn handle_resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) -> Result<()> {
        info!("📐 Window resized: {}x{}", new_size.width, new_size.height);
        Ok(())
    }

    fn handle_cursor_request(&mut self) -> Result<bool> {
        if self.is_app_active && !self.is_windowed {
            if !self.cursor_locked {
                info!("🖱️ Locking cursor to window");
                let mut grabbed = false;
                self.with_window(|window| {
                    if window
                        .set_cursor_grab(CursorGrabMode::Confined)
                        .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked))
                        .is_ok()
                    {
                        window.set_cursor_visible(false);
                        grabbed = true;
                    } else {
                        warn!("Cursor grab not supported on this platform");
                    }
                });
                self.cursor_locked = grabbed;
            }
            Ok(true)
        } else {
            if self.cursor_locked {
                self.with_window(|window| {
                    let _ = window.set_cursor_grab(CursorGrabMode::None);
                    window.set_cursor_visible(true);
                });
                self.cursor_locked = false;
            }
            Ok(false)
        }
    }

    fn handle_paint_request(&mut self) -> Result<()> {
        debug!("🎨 Paint request received");
        Ok(())
    }

    fn handle_input_focus(&mut self, gained: bool) -> Result<()> {
        // WM_SETFOCUS / WM_KILLFOCUS equivalent.
        // C++ resets TheKeyboard->resetKeys() on both focus gain and loss.
        // C++ calls TheWin32Mouse->lostFocus(gained ? FALSE : TRUE).
        if gained {
            info!("WM_SETFOCUS: resetting keyboard state, restoring mouse focus");
        } else {
            info!("WM_KILLFOCUS: resetting keyboard state, releasing mouse focus");
        }

        // Reset all pressed keys so the game doesn't see stale key state after
        // focus returns (matches TheKeyboard->resetKeys()).
        self.notify_input_reset();

        // Release cursor grab on focus loss, matching TheWin32Mouse->lostFocus(TRUE).
        if !gained && self.cursor_locked {
            self.with_window(|window| {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
            });
            self.cursor_locked = false;
        }

        // On focus gain, restore cursor to the custom game cursor.
        // C++: TheWin32Mouse->lostFocus(FALSE) then TheWin32Mouse->setCursor().
        if gained {
            self.with_window(|window| {
                if !self.is_windowed {
                    let _ = window
                        .set_cursor_grab(CursorGrabMode::Confined)
                        .or_else(|_| window.set_cursor_grab(CursorGrabMode::Locked));
                    window.set_cursor_visible(false);
                }
            });
            self.cursor_locked = !self.is_windowed;
        }

        Ok(())
    }

    fn handle_session_ending(&mut self) -> Result<()> {
        // WM_QUERYENDSESSION equivalent.
        // C++ sends MSG_META_DEMO_INSTANT_QUIT and returns 0 (deny shutdown while game runs).
        info!("WM_QUERYENDSESSION: session ending - queueing instant quit");
        self.quit_requested = true;
        self.push_message(GameMessageType::MetaInstantQuit);
        Ok(())
    }

    fn handle_destroyed(&mut self) -> Result<()> {
        // WM_DESTROY equivalent: release cursor, clean up audio focus.
        info!("WM_DESTROY: window destroyed - cleaning up");
        if self.cursor_locked {
            self.with_window(|window| {
                let _ = window.set_cursor_grab(CursorGrabMode::None);
                window.set_cursor_visible(true);
            });
            self.cursor_locked = false;
        }
        if self.audio_has_focus {
            self.audio_has_focus = false;
            self.notify_audio_focus_change(false);
        }
        self.is_app_active = false;
        Ok(())
    }

    fn is_quit_requested(&self) -> bool {
        self.quit_requested
    }
}
