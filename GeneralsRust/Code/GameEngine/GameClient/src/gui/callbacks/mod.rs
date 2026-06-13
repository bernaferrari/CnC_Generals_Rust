//! GUI Callbacks System
//!
//! This module contains all the GUI callback functions and menu systems
//! that handle user interface interactions throughout the game.
//!
//! # State Management
//!
//! GUI callbacks use thread-local Cell<bool> for state flags, matching C++ static globals.
//! The GUI is single-threaded, so Cell provides zero-overhead interior mutability.
//! Using thread_local! ensures each thread has its own state (main thread only in practice).

use std::cell::Cell;

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
pub mod lan_map_select_menu;
pub mod menu_callbacks;
pub mod message_box;
pub mod motd;
#[cfg(feature = "online_ui")]
pub mod network_direct_connect;
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
// GLOBAL GUI STATE - Thread-local Cell<bool> for zero-overhead single-threaded access
// Matches C++ static variable patterns exactly
// ============================================================================

thread_local! {
    // LAN menu state - matches C++ LANbuttonPushed, LANisShuttingDown, s_isIniting
    static LAN_BUTTON_PUSHED: Cell<bool> = Cell::new(false);
    static LAN_IS_SHUTTING_DOWN: Cell<bool> = Cell::new(false);
    static LAN_IS_INITING: Cell<bool> = Cell::new(false);
    static LAN_SLOT_LIST_UPDATES_ENABLED: Cell<bool> = Cell::new(true);

    // Skirmish menu state
    static SKIRMISH_BUTTON_PUSHED: Cell<bool> = Cell::new(false);
    static SKIRMISH_IS_SHUTTING_DOWN: Cell<bool> = Cell::new(false);
    static SKIRMISH_IS_INITING: Cell<bool> = Cell::new(false);
    static SKIRMISH_SLOT_LIST_UPDATES_ENABLED: Cell<bool> = Cell::new(true);

    // Popup state
    static POPUP_BUTTON_PUSHED: Cell<bool> = Cell::new(false);
    static POPUP_IS_SHUTTING_DOWN: Cell<bool> = Cell::new(false);

    // Main menu state - matches C++ buttonPushed, isShuttingDown, startGame
    static MAIN_MENU_BUTTON_PUSHED: Cell<bool> = Cell::new(false);
    static MAIN_MENU_IS_SHUTTING_DOWN: Cell<bool> = Cell::new(false);
    static MAIN_MENU_START_GAME: Cell<bool> = Cell::new(false);
    static MAIN_MENU_DONT_ALLOW_TRANSITIONS: Cell<bool> = Cell::new(false);
    static MAIN_MENU_CAMPAIGN_SELECTED: Cell<bool> = Cell::new(false);
}

// LAN state accessors
#[inline]
pub fn lan_button_pushed() -> bool {
    LAN_BUTTON_PUSHED.with(Cell::get)
}
#[inline]
pub fn set_lan_button_pushed(v: bool) {
    LAN_BUTTON_PUSHED.with(|c| c.set(v));
}
#[inline]
pub fn lan_is_shutting_down() -> bool {
    LAN_IS_SHUTTING_DOWN.with(Cell::get)
}
#[inline]
pub fn set_lan_is_shutting_down(v: bool) {
    LAN_IS_SHUTTING_DOWN.with(|c| c.set(v));
}
#[inline]
pub fn lan_is_initing() -> bool {
    LAN_IS_INITING.with(Cell::get)
}
#[inline]
pub fn set_lan_is_initing(v: bool) {
    LAN_IS_INITING.with(|c| c.set(v));
}
#[inline]
pub fn lan_slot_updates_enabled() -> bool {
    LAN_SLOT_LIST_UPDATES_ENABLED.with(Cell::get)
}
#[inline]
pub fn set_lan_slot_updates_enabled(v: bool) {
    LAN_SLOT_LIST_UPDATES_ENABLED.with(|c| c.set(v));
}

// Skirmish state accessors
#[inline]
pub fn skirmish_button_pushed() -> bool {
    SKIRMISH_BUTTON_PUSHED.with(Cell::get)
}
#[inline]
pub fn set_skirmish_button_pushed(v: bool) {
    SKIRMISH_BUTTON_PUSHED.with(|c| c.set(v));
}
#[inline]
pub fn skirmish_is_shutting_down() -> bool {
    SKIRMISH_IS_SHUTTING_DOWN.with(Cell::get)
}
#[inline]
pub fn set_skirmish_is_shutting_down(v: bool) {
    SKIRMISH_IS_SHUTTING_DOWN.with(|c| c.set(v));
}
#[inline]
pub fn skirmish_is_initing() -> bool {
    SKIRMISH_IS_INITING.with(Cell::get)
}
#[inline]
pub fn set_skirmish_is_initing(v: bool) {
    SKIRMISH_IS_INITING.with(|c| c.set(v));
}
#[inline]
pub fn skirmish_slot_updates_enabled() -> bool {
    SKIRMISH_SLOT_LIST_UPDATES_ENABLED.with(Cell::get)
}
#[inline]
pub fn set_skirmish_slot_updates_enabled(v: bool) {
    SKIRMISH_SLOT_LIST_UPDATES_ENABLED.with(|c| c.set(v));
}

// Popup state accessors
#[inline]
pub fn popup_button_pushed() -> bool {
    POPUP_BUTTON_PUSHED.with(Cell::get)
}
#[inline]
pub fn set_popup_button_pushed(v: bool) {
    POPUP_BUTTON_PUSHED.with(|c| c.set(v));
}
#[inline]
pub fn popup_is_shutting_down() -> bool {
    POPUP_IS_SHUTTING_DOWN.with(Cell::get)
}
#[inline]
pub fn set_popup_is_shutting_down(v: bool) {
    POPUP_IS_SHUTTING_DOWN.with(|c| c.set(v));
}

// Main menu state accessors
#[inline]
pub fn main_menu_button_pushed() -> bool {
    MAIN_MENU_BUTTON_PUSHED.with(Cell::get)
}
#[inline]
pub fn set_main_menu_button_pushed(v: bool) {
    MAIN_MENU_BUTTON_PUSHED.with(|c| c.set(v));
}
#[inline]
pub fn main_menu_is_shutting_down() -> bool {
    MAIN_MENU_IS_SHUTTING_DOWN.with(Cell::get)
}
#[inline]
pub fn set_main_menu_is_shutting_down(v: bool) {
    MAIN_MENU_IS_SHUTTING_DOWN.with(|c| c.set(v));
}
#[inline]
pub fn main_menu_start_game() -> bool {
    MAIN_MENU_START_GAME.with(Cell::get)
}
#[inline]
pub fn set_main_menu_start_game(v: bool) {
    MAIN_MENU_START_GAME.with(|c| c.set(v));
}
#[inline]
pub fn main_menu_transitions_allowed() -> bool {
    !MAIN_MENU_DONT_ALLOW_TRANSITIONS.with(Cell::get)
}
#[inline]
pub fn set_main_menu_transitions_allowed(v: bool) {
    MAIN_MENU_DONT_ALLOW_TRANSITIONS.with(|c| c.set(!v));
}
#[inline]
pub fn main_menu_campaign_selected() -> bool {
    MAIN_MENU_CAMPAIGN_SELECTED.with(Cell::get)
}
#[inline]
pub fn set_main_menu_campaign_selected(v: bool) {
    MAIN_MENU_CAMPAIGN_SELECTED.with(|c| c.set(v));
}
