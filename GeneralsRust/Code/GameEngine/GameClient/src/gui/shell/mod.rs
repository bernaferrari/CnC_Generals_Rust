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
    AnimateWindowManager, AnimationType, BasicWindowLayout, Color, Coord2D, LayoutState,
    ResidualShellMapAction, Shell, ShellError, ShellHandle, ShellMenuScheme, ShellMenuSchemeImage,
    ShellMenuSchemeLine, ShellMenuSchemeManager, WindowLayout, WindowRect, get_shell,
    queue_shell_hide, queue_shell_operation, queue_shell_pop, queue_shell_push,
    queue_shell_reverse_animate_window, queue_shell_show, queue_shell_shutdown_complete,
    queue_shell_window_animation, request_shell_menu_scheme, residual_shell_map_is_on,
    residual_shell_map_last_action, shell_anim_finished_for_layout, show_shell_map_if_available,
    simulate_shell_map_hide, simulate_shell_map_prepare_cycle, simulate_shell_map_show,
    simulate_shell_map_toggle, try_with_shell_mut, with_shell_mut, with_shell_ref,
};

// Re-export main menu types
pub use main_menu::{
    DisplaySettings, DropdownType, GameDifficulty, MainMenu, MainMenuError, MainMenuResult,
    MainMenuState, ShowSide, clear_deferred_shell_pushes, dispatch_os_click_named_window,
    drain_deferred_shell_pushes, drive_os_wnd_open_challenge_menu_like_cpp,
    drive_os_wnd_open_skirmish_like_cpp, drive_os_wnd_start_campaign_like_cpp,
    drive_os_wnd_start_china_campaign_like_cpp, drive_os_wnd_start_gla_campaign_like_cpp,
    drive_os_wnd_start_usa_campaign_like_cpp, last_os_wnd_widget_tree_click_ok,
    log_named_window_screen_rect, mark_host_match_start, note_os_wnd_widget_tree_hit,
    notify_physical_main_menu_gadget_gbm_selected, os_wnd_widget_tree_nav_ok,
    os_wnd_widget_under_cursor_name, residual_last_campaign_difficulty,
    reveal_main_menu_first_input_like_cpp, simulate_main_menu_campaign_side_button_gadget_selected,
    simulate_main_menu_campaign_start_residual,
    simulate_main_menu_challenge_button_gadget_selected,
    simulate_main_menu_credits_button_gadget_selected,
    simulate_main_menu_difficulty_button_gadget_selected,
    simulate_main_menu_load_game_button_gadget_selected,
    simulate_main_menu_multiplayer_button_gadget_selected,
    simulate_main_menu_options_button_gadget_selected,
    simulate_main_menu_replay_button_gadget_selected,
    simulate_main_menu_single_player_button_gadget_selected,
    simulate_main_menu_skirmish_button_gadget_selected,
    simulate_main_menu_skirmish_button_latch_only, soft_reveal_main_menu_for_host_inject,
    tick_main_menu_transitions,
};

// Re-export replay menu types
pub use replay_menu::{
    KeyCode as ReplayKeyCode, KeyState as ReplayKeyState, ReplayGameInfo, ReplayHeader,
    ReplayListEntry, ReplayMenu, SystemTimeValue, get_unicode_time_buffer,
    parse_ascii_string_to_game_info as replay_parse_game_info, populate_replay_file_listbox,
};

// Re-export replay controls types
pub use replay_controls::{
    GameWindow as ReplayGameWindow, ReplayControls, WindowMsg, WindowMsgHandledType,
    replay_control_input, replay_control_system,
};
