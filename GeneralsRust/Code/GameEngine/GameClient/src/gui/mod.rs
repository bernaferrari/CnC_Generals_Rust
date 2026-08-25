//! GUI System Module
//!
//! This module provides the complete UI system for the game, converted from the original
//! Command & Conquer Generals GUI systems including GameWindow, GameWindowManager,
//! GameFont, and Shell systems.
//!
//! The system handles:
//! - Window hierarchy management with parent-child relationships
//! - Event handling and message dispatching
//! - Window layouts and positioning
//! - Window transitions and animations
//! - Font management and text rendering
//! - Shell-based menu navigation with stack management
//! - Menu theming and animation support
//! - Safe reference counting for window relationships
//!
//! # Architecture
//!
//! The system is built around several main components:
//! - [`GameWindow`] - Individual UI windows and controls
//! - [`WindowManager`] - Central coordinator for all window operations
//! - [`GameFont`] - Font representation and text rendering
//! - [`FontLibrary`] - Font loading and caching system
//! - [`Shell`] - Stack-based menu system for screen management
//! - [`ShellMenuScheme`] - Menu theming and decoration system
//!
//! # Example
//!
//! ```rust
//! use crate::gui::{WindowManager, GameWindow, WindowStatus, WindowMessage};
//! use crate::gui::{FontLibrary, FontDesc, Shell};
//!
//! // Create window manager
//! let mut window_manager = WindowManager::new();
//!
//! // Create a window
//! let window = window_manager.create_window(None, 100, 100, 200, 150);
//! window.set_text("Hello World");
//! window.set_status(WindowStatus::ENABLED | WindowStatus::VISIBLE);
//!
//! // Initialize font system
//! let mut font_library = FontLibrary::new();
//! font_library.init()?;
//! let font_desc = FontDesc::new("Arial", 12, false);
//! let font = font_library.get_font(&font_desc)?;
//!
//! // Initialize shell system
//! let mut shell = Shell::new();
//! shell.init()?;
//! shell.push("Menus/MainMenu.wnd", false)?;
//!
//! // Process events
//! window_manager.update();
//! shell.update()?;
//! ```

pub mod animate_window_manager;
pub mod callbacks;
pub mod campaign_launch_host_bridge;
pub mod campaign_manager;
pub mod campaign_playthrough;
pub mod challenge_generals;
pub use campaign_playthrough::{
    CampaignPlaythroughReport, CampaignPlaythroughStep, play_through_campaign,
    playable_challenge_campaign_names, resolve_campaign_map_file,
    run_retail_campaign_and_challenge_playthrough, start_challenge_playthrough,
};
pub use challenge_generals::{
    NUM_GENERALS, ResidualChallengeGeneralsAction, residual_challenge_generals_bio_name_len,
    residual_challenge_generals_difficulty, residual_challenge_generals_last_action,
    residual_challenge_generals_starts_enabled, residual_challenge_generals_template_num,
    simulate_challenge_generals_init, simulate_challenge_generals_prepare_default,
    simulate_challenge_generals_set_bio_name, simulate_challenge_generals_set_difficulty,
    simulate_challenge_generals_set_starts_enabled, simulate_challenge_generals_set_template_num,
};
pub mod command_panel;
pub mod control_bar;
pub mod custom_match_preferences;
pub mod disconnect_menu;
pub mod display_string;
pub mod establish_connections_menu;
pub mod font;
pub mod gadget;
pub mod gadgets;
pub mod game_font;
pub mod game_window;
pub mod game_window_global;
pub mod game_window_manager;
pub mod game_window_manager_script;
pub mod game_window_transitions;
pub mod game_window_transitions_styles;
pub mod gui_callbacks;
pub mod header_template;
pub mod ime_manager;
pub use ime_manager::{
    ResidualImeAction, drive_os_wnd_ime_clear_candidates_like_cpp,
    drive_os_wnd_ime_prepare_composition_cycle_like_cpp, drive_os_wnd_ime_result_like_cpp,
    residual_ime_candidate_count, residual_ime_is_composing, residual_ime_is_enabled,
    residual_ime_last_action, simulate_ime_candidate_list, simulate_ime_clear_candidates,
    simulate_ime_disable, simulate_ime_enable, simulate_ime_end_composition,
    simulate_ime_prepare_composition_cycle, simulate_ime_reset, simulate_ime_result_string,
    simulate_ime_start_composition, simulate_ime_update_composition,
};
pub mod challenge_game_info;
pub mod ingame_ui;
pub mod integrated_ui_system;
pub mod lan_preferences;
pub mod lan_setup;
pub mod load_screen;
pub mod loading_screen;
pub mod menu_flags;
pub mod menus;
pub mod options_host_bridge;
pub mod process_animate_window;
pub mod shell;
pub mod skirmish_preferences;
pub mod skirmish_setup;
pub mod ui_globals;
pub mod ui_renderer;
pub mod w3d_gadget_draw;
pub mod win_instance_data;
pub mod window_layout;
pub mod window_manager;
pub mod window_script;
pub mod window_video_manager;
pub use window_video_manager::{
    ResidualWindowVideoAction, WINDOW_VIDEO_PLAY_TYPE_NAMES, WINDOW_VIDEO_STATE_NAMES,
    WindowVideoPlayType, WindowVideoState, residual_window_video_last_action,
    residual_window_video_pause_all, residual_window_video_playing_count,
    residual_window_video_stop_all, simulate_window_video_init, simulate_window_video_pause_all,
    simulate_window_video_prepare_control_cycle, simulate_window_video_reset,
    simulate_window_video_resume_all, simulate_window_video_stop_all, simulate_window_video_update,
};

// Re-export main types for convenience
pub use game_window::{
    GCM_ADD_ENTRY, GCM_DEL_ALL, GCM_DEL_ENTRY, GCM_EDIT_DONE, GCM_GET_ITEM_DATA, GCM_GET_SELECTION,
    GCM_GET_TEXT, GCM_SELECTED, GCM_SET_ITEM_DATA, GCM_SET_SELECTION, GCM_SET_TEXT,
    GCM_UPDATE_TEXT, GLM_DOUBLE_CLICKED, GLM_RIGHT_CLICKED, GLM_SELECTED, GWS_PUSH_BUTTON,
    GWS_STATIC_TEXT, GWS_USER_WINDOW, GameWindow, WIN_COLOR_UNDEFINED, WindowCallbacks,
    WindowDrawData, WindowError, WindowId, WindowInputReturnCode, WindowInstanceData,
    WindowMessage, WindowMsgData, WindowMsgHandled, WindowMsgPayload, WindowRegion, WindowResult,
    WindowState, WindowStatus, WindowTextColors, WindowWidget, gadget_combo_box_add_entry,
    gadget_combo_box_get_item_data, gadget_combo_box_get_length, gadget_combo_box_get_selected_pos,
    gadget_combo_box_get_text, gadget_combo_box_hide_list, gadget_combo_box_reset,
    gadget_combo_box_set_ascii_only, gadget_combo_box_set_colors, gadget_combo_box_set_is_editable,
    gadget_combo_box_set_item_data, gadget_combo_box_set_letters_and_numbers_only,
    gadget_combo_box_set_max_chars, gadget_combo_box_set_max_display,
    gadget_combo_box_set_selected_pos, gadget_combo_box_set_text,
    gadget_list_box_get_bottom_visible_entry, gadget_list_box_get_column_width,
    gadget_list_box_get_num_columns, gadget_list_box_get_selected,
    gadget_list_box_get_top_visible_entry, gadget_list_box_is_full,
    gadget_list_box_set_audio_feedback, gadget_list_box_set_bottom_visible_entry,
    gadget_list_box_set_colors, gadget_list_box_set_top_visible_entry, is_window_msg_payload,
    payload, pop_payload, push_payload, replace_payload, with_payload, with_payload_mut,
    write_input_focus_response,
};
pub use game_window_transitions::GameWindowTransitionsHandler;

pub use window_manager::{
    CaptureFlags, ModalWindow, ReentryFallback, TabDirection, WindowLayout, WindowLayoutInfo,
    WindowManager, dispatch_os_key_to_window_manager, dispatch_os_mouse_to_window_manager,
    hide_window_rc, queue_create_layout, queue_set_focus, queue_window_manager_op,
    queue_window_manager_op_deferred, with_window_manager, with_window_manager_ref,
};

// Re-export font system types for convenience
pub use display_string::{
    DisplayString, DisplayStringHandle, DisplayStringManager, get_display_string_manager,
};
pub use font::{
    FontData, FontDesc, FontError, FontLibrary, FontMetrics, GameFont, get_font_library,
};
pub use header_template::{HeaderTemplate, HeaderTemplateManager, get_header_template_manager};

// Re-export shell system types for convenience
pub use challenge_game_info::{
    challenge_game_info_exists, clear_challenge_game_info, ensure_challenge_game_info,
    init_challenge_game_info, restore_map_and_template, set_challenge_slot0_and_map,
    snapshot_map_and_template, with_challenge_game_info, with_challenge_game_info_mut,
};
pub use custom_match_preferences::CustomMatchPreferencesStore;
pub use lan_preferences::LanPreferences;
pub use lan_setup::get_lan_setup;
pub use shell::{
    AnimateWindowManager, AnimationType, Color, Coord2D, GameDifficulty, LayoutState,
    ResidualShellMapAction, Shell, ShellError, ShellHandle, ShellMenuScheme,
    ShellMenuSchemeManager, ShowSide, WindowLayout as ShellWindowLayout, WindowRect,
    clear_deferred_shell_pushes, dispatch_os_click_named_window, drain_deferred_shell_pushes,
    drive_os_wnd_open_challenge_menu_like_cpp, drive_os_wnd_open_skirmish_like_cpp,
    drive_os_wnd_start_campaign_like_cpp, drive_os_wnd_start_china_campaign_like_cpp,
    drive_os_wnd_start_gla_campaign_like_cpp, drive_os_wnd_start_usa_campaign_like_cpp, get_shell,
    last_os_wnd_widget_tree_click_ok, log_named_window_screen_rect, mark_host_match_start,
    note_os_wnd_widget_tree_hit, notify_physical_main_menu_gadget_gbm_selected,
    os_wnd_widget_tree_nav_ok, os_wnd_widget_under_cursor_name, queue_shell_hide,
    queue_shell_operation, queue_shell_pop, queue_shell_push, queue_shell_reverse_animate_window,
    queue_shell_show, queue_shell_shutdown_complete, queue_shell_window_animation,
    residual_last_campaign_difficulty, residual_shell_map_is_on, residual_shell_map_last_action,
    reveal_main_menu_first_input_like_cpp, show_shell_map_if_available,
    simulate_main_menu_campaign_side_button_gadget_selected,
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
    simulate_main_menu_skirmish_button_latch_only, simulate_shell_map_hide,
    simulate_shell_map_prepare_cycle, simulate_shell_map_show, simulate_shell_map_toggle,
    soft_reveal_main_menu_for_host_inject, tick_main_menu_transitions, try_with_shell_mut,
    with_shell_mut, with_shell_ref,
};
pub use skirmish_preferences::SkirmishPreferences;
pub use skirmish_setup::get_skirmish_setup;

// Re-export gadget system types for convenience
pub use gadgets::{
    Color as GadgetColor, Gadget, GadgetId, GadgetManager, GadgetMessage, GadgetState, GadgetTheme,
    GadgetValue, InputEvent, KeyCode, KeyModifiers, MouseButton, Rect,
    button::{ButtonCallback, ButtonStyle, ClockMode, PushButton, PushButtonBuilder},
    slider::{
        HorizontalSlider, SliderCallback, SliderConfig, SliderOrientation, SliderStyle,
        VerticalSlider,
    },
    text::{
        StaticText, TextAlignment, TextConfig, TextEntry, TextEntryCallback, ValidationMode,
        VerticalAlignment,
    },
};

// Re-export menu system types for convenience
pub use menus::{
    DisconnectMenu, EstablishConnectionsMenu, EstablishConnectionsMenuState, NATConnectionState,
    get_disconnect_menu, get_establish_connections_menu,
};

// Re-export callback system types for convenience
pub use callbacks::{
    // Control bar callbacks
    ControlBarCallbacks,
    ControlBarObserverCallbacks,
    ControlBarState,
    ControlBarSystem,
    CreditsMenu,
    // Diplomacy callbacks
    DiplomacyCallbacks,
    DiplomacySystem,
    DiplomaticRelationship,
    ExtendedMessageBoxCallbacks,

    IdleWorkerCallbacks,
    // In-game callbacks
    InGameChatCallbacks,
    InGameChatType,
    InGameUISystem,
    LanLobbyMenu,
    LeftHUDCallbacks,
    MainMenu,
    MapSelectMenu,
    // Menu callbacks
    MenuCallbacks,
    MenuManager,
    MessageBoxButton,
    // Message box callbacks
    MessageBoxCallbacks,
    MessageBoxResult,
    MessageBoxSystem,
    MessageBoxType,
    OptionsMenu,
    PlayerInfo,
    PlayerStatus,
    QuitMessageBoxCallbacks,
    ReplayControlCallbacks,
    SinglePlayerMenu,
    destroy_quit_menu,
    ex_message_box_cancel,
    ex_message_box_ok,
    ex_message_box_ok_cancel,
    ex_message_box_yes_no,
    ex_message_box_yes_no_cancel,
    get_control_bar_system,
    get_diplomacy_system,
    get_ingame_ui_system,
    get_menu_manager,
    get_message_box_system,
    hide_control_bar,
    hide_diplomacy,
    hide_in_game_chat,
    hide_quit_menu,
    is_diplomacy_active,
    is_in_game_chat_active,
    message_box_cancel,
    message_box_ok,
    message_box_ok_cancel,
    message_box_yes_no,
    message_box_yes_no_cancel,
    quit_message_box_yes_no,

    reset_diplomacy,
    reset_in_game_chat,
    set_in_game_chat_type,
    show_control_bar,

    show_in_game_chat,
    show_message_box,
    show_quit_dialog,

    simulate_skirmish_start_button_gadget_selected,
    toggle_control_bar,
    toggle_diplomacy,
    toggle_in_game_chat,
    toggle_quit_menu,
};

// Re-export in-game UI types
pub use ingame_ui::{
    DrawableID, HintData, HintType, InGameUI, InGameUIError, InGameUIIniSettings, MessageText,
    MilitarySubtitle, Minimap, MinimapIcon, MouseCursor, MouseMode, PlacementPreview,
    ResourceDisplay, SelectionBox, SelectionState,
};

// Re-export command panel types
pub use command_panel::{
    CommandButton, CommandButtonState, CommandButtonType, CommandPanel, CommandPanelContext,
    CommandPanelError,
};

// Re-export UI renderer types
pub use ui_renderer::{
    RenderStats, UIBlendMode, UIDrawCommand, UIRect, UIRenderer, UIRendererError,
};

// Re-export integrated UI system types
pub use integrated_ui_system::{
    IntegratedUIError, IntegratedUISystem, IntegratedUISystemBuilder, UICommand,
};

pub use ui_globals::{
    cursor_tooltip_already_submitted, set_ui_renderer, submit_cursor_tooltip, tick_cursor_tooltip,
    with_ui_renderer, with_ui_renderer_mut,
};

/// Maximum number of windows that can be created
pub const MAX_WINDOWS: usize = 576;

/// Cursor movement tolerance (squared)
pub const CURSOR_MOVE_TOL_SQ: i32 = 4;

/// Default tooltip delay in frames
pub const TOOLTIP_DELAY: i32 = 10;

/// Maximum tooltip text length
pub const TOOLTIP_MAX_LEN: usize = 64;

/// Maximum number of draw data entries per window state
pub const MAX_DRAW_DATA: usize = 9;

/// User-defined message base value
pub const GWM_USER: u32 = 32768;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_creation() {
        let mut window_manager = WindowManager::new();
        let window = window_manager.create_window(None, 0, 0, 100, 100);
        assert!(window.is_ok());

        let window = window.unwrap();
        assert_eq!(window.borrow().get_size(), (100, 100));
        assert_eq!(window.borrow().get_position(), (0, 0));
    }

    #[test]
    fn test_window_hierarchy() {
        let mut window_manager = WindowManager::new();

        let parent = window_manager.create_window(None, 0, 0, 200, 200).unwrap();
        let child = window_manager
            .create_window(Some(&parent), 10, 10, 50, 50)
            .unwrap();

        assert!(parent.borrow().is_child(&*child.borrow()));
        let child_parent = child.borrow().get_parent();
        assert!(child_parent.is_some());
    }

    #[test]
    fn test_window_status() {
        let mut window_manager = WindowManager::new();
        let window = window_manager.create_window(None, 0, 0, 100, 100).unwrap();

        window.borrow_mut().set_status(WindowStatus::ENABLED);
        assert!(window.borrow().get_status().contains(WindowStatus::ENABLED));

        window.borrow_mut().hide(true).unwrap();
        assert!(window.borrow().is_hidden());
    }
}
