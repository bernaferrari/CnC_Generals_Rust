//! # Shell GUI Components
//!
//! This module contains shell-specific GUI components including menu systems,
//! dialogs, and screen transitions.
//!
//! ## Modules
//! - `base`: Base shell system for menu navigation (original shell.rs)
//! - `main_menu`: Main menu UI system with all callbacks and state management
//! - `replay_menu`: Replay browsing and playback menu
//! - `replay_controls`: Replay playback control UI

pub mod base;
pub mod main_menu;
pub mod replay_controls;
pub mod replay_menu;
pub mod shell;
pub mod shell_menu_scheme;

// Re-export base shell types
pub use base::{
    get_shell, request_shell_menu_scheme, show_shell_map_if_available, try_with_shell_mut,
    AnimateWindowManager, AnimationType, BasicWindowLayout, Color, Coord2D, LayoutState, Shell,
    ShellError, ShellMenuScheme, ShellMenuSchemeImage, ShellMenuSchemeLine, ShellMenuSchemeManager,
    WindowLayout, WindowRect,
};

// Re-export main menu types
pub use main_menu::{
    DisplaySettings, DropdownType, GameDifficulty, MainMenu, MainMenuError, MainMenuResult,
    MainMenuState, ShowSide,
};

// Re-export replay menu types
pub use replay_menu::{
    get_unicode_time_buffer, parse_ascii_string_to_game_info as replay_parse_game_info,
    KeyCode as ReplayKeyCode, KeyState as ReplayKeyState, ReplayGameInfo, ReplayHeader,
    ReplayListEntry, ReplayMenu, SystemTimeValue,
};

// Re-export replay controls types
pub use replay_controls::{
    replay_control_input, replay_control_system, GameWindow as ReplayGameWindow, ReplayControls,
    WindowMsg, WindowMsgHandledType,
};
