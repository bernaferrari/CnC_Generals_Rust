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
pub mod campaign_manager;
pub mod challenge_generals;
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
pub mod ingame_ui;
pub mod integrated_ui_system;
pub mod lan_preferences;
pub mod lan_setup;
pub mod load_screen;
pub mod loading_screen;
pub mod menu_flags;
pub mod menus;
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

// Re-export main types for convenience
pub use game_window::{
    gadget_list_box_get_bottom_visible_entry, gadget_list_box_get_column_width,
    gadget_list_box_get_num_columns, gadget_list_box_get_selected,
    gadget_list_box_get_top_visible_entry, gadget_list_box_is_full,
    gadget_list_box_set_audio_feedback, gadget_list_box_set_bottom_visible_entry,
    gadget_list_box_set_colors, gadget_list_box_set_top_visible_entry, write_input_focus_response,
    GameWindow, WindowCallbacks, WindowDrawData, WindowError, WindowId, WindowInputReturnCode,
    WindowInstanceData, WindowMessage, WindowMsgData, WindowMsgHandled, WindowRegion, WindowResult,
    WindowState, WindowStatus, WindowTextColors, WindowWidget, GLM_DOUBLE_CLICKED,
    GLM_RIGHT_CLICKED, GLM_SELECTED, GWS_PUSH_BUTTON, GWS_STATIC_TEXT, GWS_USER_WINDOW,
    WIN_COLOR_UNDEFINED,
};
pub use game_window_transitions::GameWindowTransitionsHandler;

pub use window_manager::{
    with_window_manager, with_window_manager_ref, CaptureFlags, ModalWindow, TabDirection,
    WindowLayout, WindowLayoutInfo, WindowManager,
};

// Re-export font system types for convenience
pub use display_string::{
    get_display_string_manager, DisplayString, DisplayStringHandle, DisplayStringManager,
};
pub use font::{
    get_font_library, FontData, FontDesc, FontError, FontLibrary, FontMetrics, GameFont,
};
pub use header_template::{get_header_template_manager, HeaderTemplate, HeaderTemplateManager};

// Re-export shell system types for convenience
pub use custom_match_preferences::CustomMatchPreferencesStore;
pub use lan_preferences::LanPreferences;
pub use lan_setup::get_lan_setup;
pub use shell::{
    get_shell, AnimateWindowManager, AnimationType, Color, Coord2D, LayoutState, Shell, ShellError,
    ShellMenuScheme, ShellMenuSchemeManager, WindowLayout as ShellWindowLayout, WindowRect,
};
pub use skirmish_preferences::SkirmishPreferences;
pub use skirmish_setup::get_skirmish_setup;

// Re-export gadget system types for convenience
pub use gadgets::{
    button::{ButtonCallback, ButtonStyle, ClockMode, PushButton, PushButtonBuilder},
    slider::{
        HorizontalSlider, SliderCallback, SliderConfig, SliderOrientation, SliderStyle,
        VerticalSlider,
    },
    text::{
        StaticText, TextAlignment, TextConfig, TextEntry, TextEntryCallback, ValidationMode,
        VerticalAlignment,
    },
    Color as GadgetColor, Gadget, GadgetId, GadgetManager, GadgetMessage, GadgetState, GadgetTheme,
    GadgetValue, InputEvent, KeyCode, KeyModifiers, MouseButton, Rect,
};

// Re-export menu system types for convenience
pub use menus::{
    get_disconnect_menu, get_establish_connections_menu, DisconnectMenu, EstablishConnectionsMenu,
    EstablishConnectionsMenuState, NATConnectionState,
};

// Re-export callback system types for convenience
pub use callbacks::{
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

    toggle_control_bar,
    toggle_diplomacy,
    toggle_in_game_chat,
    toggle_quit_menu,
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

pub use ui_globals::{set_ui_renderer, with_ui_renderer, with_ui_renderer_mut};

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
