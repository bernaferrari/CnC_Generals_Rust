//! GUI Callbacks System
//!
//! This module contains all the GUI callback functions and menu systems
//! that handle user interface interactions throughout the game.
//!
//! # State Management
//!
//! LAN/Skirmish shell transition flags live on WindowManagerOwner (MenuShellFlags).
//! Main-menu equivalents are owned on MainMenu state.


pub mod challenge_menu;
pub mod control_bar_callbacks;
pub mod difficulty_select;
pub mod diplomacy;
pub mod download_menu;
pub mod game_info_window;
pub mod generals_exp_points;
pub mod ime_candidate;
pub mod in_game_popup_message;
pub mod ingame_callbacks;
pub mod keyboard_options_menu;
pub mod lan_game_options_menu;
#[cfg(feature = "online_ui")]
pub mod lan_lobby_menu;
#[cfg(not(feature = "online_ui"))]
pub mod lan_lobby_menu {
    use crate::gui::{GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled};

    pub fn lan_lobby_init(
        _layout: &WindowLayout,
        _effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
    }
    pub fn lan_lobby_update(_layout: &WindowLayout) {}
    pub fn lan_lobby_shutdown(_layout: &WindowLayout, _immediate: bool) {}

    pub fn lan_lobby_input(
        _window: &GameWindow,
        _msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        _effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        WindowMsgHandled::Ignored
    }

    pub fn lan_lobby_system(
        _window: &GameWindow,
        _msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        WindowMsgHandled::Ignored
    }
}
pub mod lan_map_select_menu;
pub mod menu_callbacks;
pub mod message_box;
pub mod motd;
#[cfg(feature = "online_ui")]
pub mod network_direct_connect;
#[cfg(feature = "online_ui")]
pub mod online_callback_support;
// The online feature supplies the real callback implementations below.  Keep
// the no-op network table out of that build: exporting both tables creates
// duplicate callback names and makes every window-manager binding ambiguous.
#[cfg(not(feature = "online_ui"))]
mod network_stubs;
pub mod popup_communicator;
#[cfg(feature = "online_ui")]
pub mod popup_host_game;
#[cfg(feature = "online_ui")]
pub mod popup_join_game;
#[cfg(feature = "online_ui")]
pub mod popup_ladder_select;
#[cfg(feature = "online_ui")]
pub mod popup_player_info;
pub mod popup_replay;
pub mod popup_save_load;
pub mod quit_menu;
pub mod replay_controls;
pub mod replay_menu;
pub mod score_screen;
pub mod skirmish_game_options_menu;
pub mod skirmish_map_select_menu;
#[cfg(feature = "online_ui")]
pub mod wol_buddy_overlay;
#[cfg(feature = "online_ui")]
pub mod wol_custom_score_screen;
#[cfg(feature = "online_ui")]
pub mod wol_game_setup_menu;
#[cfg(feature = "online_ui")]
pub mod wol_ladder_screen;
#[cfg(feature = "online_ui")]
pub mod wol_lobby_menu;
#[cfg(feature = "online_ui")]
pub mod wol_locale_select_popup;
#[cfg(feature = "online_ui")]
pub mod wol_login_menu;
#[cfg(feature = "online_ui")]
pub mod wol_map_select_menu;
#[cfg(feature = "online_ui")]
pub mod wol_message_window;
#[cfg(feature = "online_ui")]
pub mod wol_quick_match_menu;
#[cfg(feature = "online_ui")]
pub mod wol_status_menu;
#[cfg(feature = "online_ui")]
pub mod wol_welcome_menu;
#[cfg(feature = "online_ui")]
pub mod wolqm_score_screen;

// Re-export main types
pub use challenge_menu::*;
pub use control_bar_callbacks::*;
pub use difficulty_select::*;
pub use diplomacy::*;
pub use download_menu::*;
pub use game_info_window::*;
pub use generals_exp_points::*;
pub use ime_candidate::*;
pub use in_game_popup_message::*;
pub use ingame_callbacks::*;
pub use keyboard_options_menu::*;
pub use lan_game_options_menu::*;
pub use lan_map_select_menu::*;
pub use menu_callbacks::*;
pub use message_box::*;
pub use motd::*;
#[cfg(feature = "online_ui")]
pub use network_direct_connect::*;
#[cfg(not(feature = "online_ui"))]
pub use network_stubs::*;
pub use popup_communicator::*;
#[cfg(feature = "online_ui")]
pub use popup_host_game::*;
#[cfg(feature = "online_ui")]
pub use popup_join_game::*;
#[cfg(feature = "online_ui")]
pub use popup_ladder_select::*;
#[cfg(feature = "online_ui")]
pub use popup_player_info::*;
pub use popup_replay::*;
pub use popup_save_load::*;
pub use quit_menu::*;
pub use replay_controls::*;
pub use replay_menu::*;
pub use score_screen::*;
pub use skirmish_game_options_menu::*;
pub use skirmish_map_select_menu::*;
#[cfg(feature = "online_ui")]
pub use wol_buddy_overlay::*;
#[cfg(feature = "online_ui")]
pub use wol_custom_score_screen::*;
#[cfg(feature = "online_ui")]
pub use wol_game_setup_menu::*;
#[cfg(feature = "online_ui")]
pub use wol_ladder_screen::*;
#[cfg(feature = "online_ui")]
pub use wol_lobby_menu::*;
#[cfg(feature = "online_ui")]
pub use wol_locale_select_popup::*;
#[cfg(feature = "online_ui")]
pub use wol_login_menu::*;
#[cfg(feature = "online_ui")]
pub use wol_map_select_menu::*;
#[cfg(feature = "online_ui")]
pub use wol_message_window::*;
#[cfg(feature = "online_ui")]
pub use wol_quick_match_menu::*;
#[cfg(feature = "online_ui")]
pub use wol_status_menu::*;
#[cfg(feature = "online_ui")]
pub use wol_welcome_menu::*;
#[cfg(feature = "online_ui")]
pub use wolqm_score_screen::*;

// ============================================================================
// Menu shell flags (LAN / Skirmish) — owned on WindowManagerOwner.
// C++ kept these as file-static bools per menu; Rust keeps them on the
// receiving client owner so multiple GameClients never share TLS Cells.
// Main-menu / popup equivalents already live on MainMenu / popup state.
// ============================================================================

use crate::gui::window_manager::WindowManagerOwner;
pub use crate::gui::window_manager::MenuShellFlags;

fn flags(owner: &WindowManagerOwner) -> &MenuShellFlags {
    owner.menu_shell_flags()
}


// LAN state accessors
#[inline]
pub fn lan_button_pushed(owner: &WindowManagerOwner) -> bool {
    flags(owner).lan_button_pushed.get()
}
#[inline]
pub fn set_lan_button_pushed(owner: &WindowManagerOwner, v: bool) {
    flags(owner).lan_button_pushed.set(v);
}
#[inline]
pub fn lan_is_shutting_down(owner: &WindowManagerOwner) -> bool {
    flags(owner).lan_is_shutting_down.get()
}
#[inline]
pub fn set_lan_is_shutting_down(owner: &WindowManagerOwner, v: bool) {
    flags(owner).lan_is_shutting_down.set(v);
}
#[inline]
pub fn lan_is_initing(owner: &WindowManagerOwner) -> bool {
    flags(owner).lan_is_initing.get()
}
#[inline]
pub fn set_lan_is_initing(owner: &WindowManagerOwner, v: bool) {
    flags(owner).lan_is_initing.set(v);
}
#[inline]
pub fn lan_slot_updates_enabled(owner: &WindowManagerOwner) -> bool {
    flags(owner).lan_slot_list_updates_enabled.get()
}
#[inline]
pub fn set_lan_slot_updates_enabled(owner: &WindowManagerOwner, v: bool) {
    flags(owner).lan_slot_list_updates_enabled.set(v);
}

// Skirmish state accessors
#[inline]
pub fn skirmish_button_pushed(owner: &WindowManagerOwner) -> bool {
    flags(owner).skirmish_button_pushed.get()
}
#[inline]
pub fn set_skirmish_button_pushed(owner: &WindowManagerOwner, v: bool) {
    flags(owner).skirmish_button_pushed.set(v);
}
#[inline]
pub fn skirmish_is_shutting_down(owner: &WindowManagerOwner) -> bool {
    flags(owner).skirmish_is_shutting_down.get()
}
#[inline]
pub fn set_skirmish_is_shutting_down(owner: &WindowManagerOwner, v: bool) {
    flags(owner).skirmish_is_shutting_down.set(v);
}
#[inline]
pub fn skirmish_is_initing(owner: &WindowManagerOwner) -> bool {
    flags(owner).skirmish_is_initing.get()
}
#[inline]
pub fn set_skirmish_is_initing(owner: &WindowManagerOwner, v: bool) {
    flags(owner).skirmish_is_initing.set(v);
}
#[inline]
pub fn skirmish_slot_updates_enabled(owner: &WindowManagerOwner) -> bool {
    flags(owner).skirmish_slot_list_updates_enabled.get()
}
#[inline]
pub fn set_skirmish_slot_updates_enabled(owner: &WindowManagerOwner, v: bool) {
    flags(owner).skirmish_slot_list_updates_enabled.set(v);
}

pub mod owned_credits_menu;

pub mod control_bar_observation;
