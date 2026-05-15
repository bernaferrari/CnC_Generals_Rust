// FILE: replay_controls.rs
// Author: Bryan Cleveland - December 2001 (original C++), Rust port
// Description: GUI Control box for the playback controls
//
// Ported from: GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/ReplayControls.cpp

use std::any::Any;

/// Window message types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMsg {
    Create,
    Destroy,
    Selected,
    Char,
    InputFocus,
    Ignored,
}

/// Window message handling result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMsgHandledType {
    MsgHandled,
    MsgIgnored,
}

/// Window message data - generic container for message parameters
pub type WindowMsgData = Box<dyn Any>;

/// Represents a game window in the UI system
pub struct GameWindow {
    pub id: u32,
    pub name: String,
    pub visible: bool,
    pub enabled: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl GameWindow {
    pub fn new(id: u32, name: String) -> Self {
        GameWindow {
            id,
            name,
            visible: true,
            enabled: true,
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        }
    }

    pub fn win_get_window_id(&self) -> u32 {
        self.id
    }

    pub fn win_hide(&mut self, hide: bool) {
        self.visible = !hide;
    }

    pub fn win_set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// Replay playback controls state
pub struct ReplayControls {
    parent_window: Option<GameWindow>,
    is_playing: bool,
    is_paused: bool,
    current_frame: u32,
    total_frames: u32,
    playback_speed: f32,
}

impl ReplayControls {
    pub fn new() -> Self {
        ReplayControls {
            parent_window: None,
            is_playing: false,
            is_paused: false,
            current_frame: 0,
            total_frames: 0,
            playback_speed: 1.0,
        }
    }

    /// Initialize the replay controls
    pub fn init(&mut self, parent: GameWindow) {
        self.parent_window = Some(parent);
        self.is_playing = false;
        self.is_paused = false;
        self.current_frame = 0;
        self.playback_speed = 1.0;
    }

    /// Update the replay controls.
    ///
    /// When playing and not paused, advances the current frame counter by the playback speed.
    /// C++ ReplayControls delegates to the game logic's frame-advance loop; this Rust port
    /// tracks the logical frame position and signals completion when reaching total_frames.
    pub fn update(&mut self, delta_time: f32) {
        if self.is_playing && !self.is_paused {
            // At 30 logic FPS, one frame = 1/30s. Advance by speed * delta frames.
            let frames_to_advance = (self.playback_speed * delta_time * 30.0).round() as u32;
            if frames_to_advance > 0 {
                let new_frame = self.current_frame.saturating_add(frames_to_advance);
                self.current_frame = if self.total_frames > 0 {
                    new_frame.min(self.total_frames)
                } else {
                    new_frame
                };

                if self.total_frames > 0 && self.current_frame >= self.total_frames {
                    self.is_playing = false;
                    self.is_paused = true;
                }
            }
        }
    }

    /// Start playback
    pub fn play(&mut self) {
        self.is_playing = true;
        self.is_paused = false;
    }

    /// Pause playback
    pub fn pause(&mut self) {
        self.is_paused = true;
    }

    /// Stop playback
    pub fn stop(&mut self) {
        self.is_playing = false;
        self.is_paused = false;
        self.current_frame = 0;
    }

    /// Set playback speed
    pub fn set_speed(&mut self, speed: f32) {
        self.playback_speed = speed.max(0.1).min(4.0);
    }

    /// Seek to specific frame
    pub fn seek_to_frame(&mut self, frame: u32) {
        if frame <= self.total_frames {
            self.current_frame = frame;
        }
    }

    /// Set total number of frames in the replay
    pub fn set_total_frames(&mut self, total: u32) {
        self.total_frames = total;
    }

    /// Get playback progress as a 0.0..1.0 ratio
    pub fn get_progress(&self) -> f32 {
        if self.total_frames == 0 {
            0.0
        } else {
            self.current_frame as f32 / self.total_frames as f32
        }
    }

    /// Get current playback state
    pub fn is_playing(&self) -> bool {
        self.is_playing && !self.is_paused
    }

    pub fn is_paused(&self) -> bool {
        self.is_paused
    }

    pub fn get_current_frame(&self) -> u32 {
        self.current_frame
    }

    pub fn get_total_frames(&self) -> u32 {
        self.total_frames
    }

    pub fn get_playback_speed(&self) -> f32 {
        self.playback_speed
    }
}

impl Default for ReplayControls {
    fn default() -> Self {
        Self::new()
    }
}

/// Input procedure for the control bar
///
/// Matches C++ ReplayControls.cpp:16-22
pub fn replay_control_input(
    _window: &GameWindow,
    msg: WindowMsg,
    _mdata1: Option<&dyn Any>,
    _mdata2: Option<&dyn Any>,
) -> WindowMsgHandledType {
    match msg {
        WindowMsg::Char | WindowMsg::Selected => WindowMsgHandledType::MsgHandled,
        _ => WindowMsgHandledType::MsgIgnored,
    }
}

/// System callback for the control bar parent
///
/// Matches C++ ReplayControls.cpp:27-49
pub fn replay_control_system(
    _window: &GameWindow,
    msg: WindowMsg,
    _mdata1: Option<&dyn Any>,
    _mdata2: Option<&dyn Any>,
) -> WindowMsgHandledType {
    match msg {
        WindowMsg::Selected => WindowMsgHandledType::MsgHandled,
        WindowMsg::Create | WindowMsg::Destroy => WindowMsgHandledType::MsgHandled,
        _ => WindowMsgHandledType::MsgIgnored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_controls_creation() {
        let controls = ReplayControls::new();
        assert!(!controls.is_playing());
        assert!(!controls.is_paused());
        assert_eq!(controls.get_current_frame(), 0);
        assert_eq!(controls.get_playback_speed(), 1.0);
    }

    #[test]
    fn test_replay_controls_playback() {
        let mut controls = ReplayControls::new();

        controls.play();
        assert!(controls.is_playing());
        assert!(!controls.is_paused());

        controls.pause();
        assert!(!controls.is_playing());
        assert!(controls.is_paused());

        controls.stop();
        assert!(!controls.is_playing());
        assert!(!controls.is_paused());
        assert_eq!(controls.get_current_frame(), 0);
    }

    #[test]
    fn test_replay_controls_speed() {
        let mut controls = ReplayControls::new();

        controls.set_speed(2.0);
        assert_eq!(controls.get_playback_speed(), 2.0);

        controls.set_speed(0.5);
        assert_eq!(controls.get_playback_speed(), 0.5);

        // Test clamping
        controls.set_speed(10.0);
        assert_eq!(controls.get_playback_speed(), 4.0);

        controls.set_speed(0.01);
        assert_eq!(controls.get_playback_speed(), 0.1);
    }

    #[test]
    fn test_window_message_handlers() {
        let window = GameWindow::new(1, "TestWindow".to_string());

        let result = replay_control_input(&window, WindowMsg::Ignored, None, None);
        assert_eq!(result, WindowMsgHandledType::MsgIgnored);

        let result = replay_control_system(&window, WindowMsg::Selected, None, None);
        assert_eq!(result, WindowMsgHandledType::MsgHandled);
    }
}
