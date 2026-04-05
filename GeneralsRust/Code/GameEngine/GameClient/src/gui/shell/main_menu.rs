//! # Main Menu UI System
//!
//! Port of MainMenu.cpp - Main menu window callbacks and management
//! Author: Colin Day, October 2001 (C++ original)
//! Ported to Rust: 2025
//!
//! ## Description
//! This module implements the main menu system for Command & Conquer Generals Zero Hour.
//! It handles:
//! - Main menu initialization and shutdown
//! - Button callbacks for menu navigation (Single Player, Multiplayer, Options, Exit, etc.)
//! - Dropdown menu transitions and animations
//! - Campaign selection and difficulty settings
//! - Movie playback triggers
//! - Shell map transitions
//! - Resolution change handling
//! - CD check for campaign mode
//!
//! ## Architecture
//! The main menu operates on a state machine with multiple dropdown menus:
//! - DROPDOWN_NONE: No dropdown active
//! - DROPDOWN_SINGLE: Single player menu
//! - DROPDOWN_MULTIPLAYER: Multiplayer menu
//! - DROPDOWN_MAIN: Main menu
//! - DROPDOWN_LOADREPLAY: Load/Replay menu
//! - DROPDOWN_DIFFICULTY: Difficulty selection menu
//!
//! ## C++ Compatibility
//! This is a faithful port of /GeneralsMD/Code/GameEngine/Source/GameClient/GUI/GUICallbacks/Menus/MainMenu.cpp
//! All constants, state transitions, and callbacks match the original C++ implementation.

use crate::gui::callbacks::download_menu::download_menu_update;
use crate::gui::callbacks::message_box::{
    message_box_ok, message_box_ok_cancel, quit_message_box_yes_no, MessageBoxFunc,
};
use crate::gui::campaign_manager::{get_campaign_manager, GameDifficulty as CampaignDifficulty};
use crate::gui::challenge_generals::{
    get_challenge_generals_mut, GameDifficulty as ChallengeGameDifficulty,
};
use crate::gui::header_template::get_header_template_manager;
use crate::gui::menu_flags::get_dont_show_main_menu;
use crate::gui::shell::{get_shell, request_shell_menu_scheme};
use crate::gui::window_manager::{
    with_window_manager, with_window_manager_ref, WindowLayout as ManagerWindowLayout,
};
use crate::helpers::set_mouse_cursor_visibility;
use crate::helpers::{TheControlBar, TheInGameUI};
use crate::map_util::get_map_cache_manager;
use crate::message_stream::{get_message_stream, GameMessageType};
use crate::system::SubsystemInterface;
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::system::copy_protection::{get_protection_manager, ProtectionStatus};
use game_engine::common::user_preferences::UserPreferences;
use game_network::download_manager::download_manager;
use game_network::gamespy::peer_defs::tear_down_gamespy;
use game_network::gamespy::peer_thread::get_peer_message_queue;
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use thiserror::Error;

#[cfg(feature = "network")]
fn raise_gamespy_message_boxes() {
    crate::gamespy_overlay::raise_gs_message_box();
}

#[cfg(not(feature = "network"))]
fn raise_gamespy_message_boxes() {}

#[cfg(feature = "network")]
fn update_gamespy_overlays() {
    crate::gamespy_overlay::update_overlays();
}

#[cfg(not(feature = "network"))]
fn update_gamespy_overlays() {}

fn http_startup() {}

fn http_cleanup() {}

fn stop_async_dns_check() {}

fn set_main_menu_cursor_visibility(visible: bool) {
    set_mouse_cursor_visibility(visible);
}

// ================================================================================================
// CONSTANTS
// ================================================================================================

/// Time constants - match C++ MainMenu.cpp
const SHOW_FRAMES_LIMIT: i32 = 20;
const INITIAL_GADGET_DELAY_DEFAULT: i32 = 210;
const CORNER: i32 = 10;
static FIRST_TIME_RUNNING_GAME: AtomicBool = AtomicBool::new(true);

// ================================================================================================
// MESSAGE CONSTANTS (C++ COMPAT)
// ================================================================================================

const GWM_CREATE: u32 = 1;
const GWM_DESTROY: u32 = 2;
const GWM_CHAR: u32 = 21;
const GWM_INPUT_FOCUS: u32 = 23;
const GWM_MOUSE_POS: u32 = 24;

const GGM_LEFT_DRAG: u32 = 16384;
const GBM_MOUSE_ENTERING: u32 = GGM_LEFT_DRAG + 6;
const GBM_MOUSE_LEAVING: u32 = GGM_LEFT_DRAG + 7;
const GBM_SELECTED: u32 = GGM_LEFT_DRAG + 8;

// ================================================================================================
// ENUMERATIONS
// ================================================================================================

/// Dropdown menu types
/// Matches C++ MainMenu.cpp lines 66-76
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum DropdownType {
    None = 0,
    Single,
    Multiplayer,
    Main,
    LoadReplay,
    Difficulty,
    Count, // keep last
}

impl DropdownType {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(DropdownType::None),
            1 => Some(DropdownType::Single),
            2 => Some(DropdownType::Multiplayer),
            3 => Some(DropdownType::Main),
            4 => Some(DropdownType::LoadReplay),
            5 => Some(DropdownType::Difficulty),
            _ => None,
        }
    }
}

/// Show side types for faction logos
/// Matches C++ MainMenu.cpp lines 166-174
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ShowSide {
    None = 0,
    Training,
    USA,
    GLA,
    China,
    Skirmish,
}

impl ShowSide {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(ShowSide::None),
            1 => Some(ShowSide::Training),
            2 => Some(ShowSide::USA),
            3 => Some(ShowSide::GLA),
            4 => Some(ShowSide::China),
            5 => Some(ShowSide::Skirmish),
            _ => None,
        }
    }
}

/// Game difficulty levels
/// Matches C++ GameDifficulty enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum GameDifficulty {
    Easy = 0,
    Normal = 1,
    Hard = 2,
}

/// Message box return types
/// Matches C++ MessageBoxReturnType
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageBoxReturnType {
    Close,
    KeepOpen,
}

#[derive(Debug, Clone)]
enum PendingMainMenuAction {
    PushShellScreen(&'static str),
    ReverseTransitionGroup(&'static str),
    ShowOptionsLayout,
    SignalUiInteract(&'static str),
    ReverseAnimateWindow,
    StartPatchCheck,
    StartDownloadingPatches,
    LaunchWorldBuilder,
    QuitRequest,
}

// ================================================================================================
// ERROR TYPES
// ================================================================================================

#[derive(Error, Debug)]
pub enum MainMenuError {
    #[error("Window not found: {0}")]
    WindowNotFound(String),
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: DropdownType,
        to: DropdownType,
    },
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),
    #[error("Shutdown failed: {0}")]
    ShutdownFailed(String),
    #[error("Campaign not selected")]
    CampaignNotSelected,
    #[error("CD check failed")]
    CDCheckFailed,
}

pub type MainMenuResult<T> = Result<T, MainMenuError>;

// ================================================================================================
// WINDOW ID STORAGE
// ================================================================================================

/// Window IDs for main menu buttons and windows
/// Matches C++ static NameKeyType variables (lines 90-123)
#[derive(Debug, Clone)]
pub struct WindowIds {
    pub main_menu_id: u32,
    pub skirmish_id: u32,
    pub online_id: u32,
    pub network_id: u32,
    pub options_id: u32,
    pub exit_id: u32,
    pub motd_id: u32,
    pub world_builder_id: u32,
    pub get_update_id: u32,
    pub button_training_id: u32,
    pub button_challenge_id: u32,
    pub button_usa_id: u32,
    pub button_gla_id: u32,
    pub button_china_id: u32,
    pub button_usa_recent_save_id: u32,
    pub button_usa_load_game_id: u32,
    pub button_gla_recent_save_id: u32,
    pub button_gla_load_game_id: u32,
    pub button_china_recent_save_id: u32,
    pub button_china_load_game_id: u32,
    pub button_single_player_id: u32,
    pub button_multi_player_id: u32,
    pub button_multi_back_id: u32,
    pub button_single_back_id: u32,
    pub button_load_replay_back_id: u32,
    pub button_replay_id: u32,
    pub button_load_replay_id: u32,
    pub button_load_id: u32,
    pub button_credits_id: u32,
    pub button_easy_id: u32,
    pub button_medium_id: u32,
    pub button_hard_id: u32,
    pub button_diff_back_id: u32,
}

impl Default for WindowIds {
    fn default() -> Self {
        Self {
            main_menu_id: 0,
            skirmish_id: 0,
            online_id: 0,
            network_id: 0,
            options_id: 0,
            exit_id: 0,
            motd_id: 0,
            world_builder_id: 0,
            get_update_id: 0,
            button_training_id: 0,
            button_challenge_id: 0,
            button_usa_id: 0,
            button_gla_id: 0,
            button_china_id: 0,
            button_usa_recent_save_id: 0,
            button_usa_load_game_id: 0,
            button_gla_recent_save_id: 0,
            button_gla_load_game_id: 0,
            button_china_recent_save_id: 0,
            button_china_load_game_id: 0,
            button_single_player_id: 0,
            button_multi_player_id: 0,
            button_multi_back_id: 0,
            button_single_back_id: 0,
            button_load_replay_back_id: 0,
            button_replay_id: 0,
            button_load_replay_id: 0,
            button_load_id: 0,
            button_credits_id: 0,
            button_easy_id: 0,
            button_medium_id: 0,
            button_hard_id: 0,
            button_diff_back_id: 0,
        }
    }
}

// ================================================================================================
// DISPLAY SETTINGS
// ================================================================================================

/// Display settings for resolution management
/// Matches C++ DisplaySettings structure
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplaySettings {
    pub x_res: i32,
    pub y_res: i32,
    pub bit_depth: i32,
    pub windowed: bool,
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            x_res: 1024,
            y_res: 768,
            bit_depth: 32,
            windowed: false,
        }
    }
}

// ================================================================================================
// MAIN MENU STATE
// ================================================================================================

/// Main menu state
/// Matches C++ static variables (lines 78-190)
pub struct MainMenuState {
    // Window IDs
    pub window_ids: WindowIds,

    // State flags - match C++ lines 78-190
    pub raise_message_boxes: bool,
    pub campaign_selected: bool,
    pub button_pushed: bool,
    pub is_shutting_down: bool,
    pub start_game: bool,
    pub show_fade: bool,
    pub not_shown: bool,
    pub first_time_running_the_game: bool,
    pub show_logo: bool,
    pub logo_is_shown: bool,
    pub just_entered: bool,
    pub launch_challenge_menu: bool,
    pub dont_allow_transitions: bool,
    pub checking_for_patch_before_gamespy: bool,
    pub cant_connect_before_online: bool,
    pub checks_left_before_online: i32,
    pub time_through_online: u32,
    pub online_cancel_window_open: bool,

    // Dropdown state
    pub drop_down: DropdownType,
    pub pending_drop_down: DropdownType,

    // Show state
    pub show_frames: i32,
    pub show_side: ShowSide,
    pub initial_gadget_delay: i32,

    // Display settings
    pub old_disp_settings: DisplaySettings,
    pub new_disp_settings: DisplaySettings,
    pub disp_changed: bool,

    // Dropdown windows (indexed by DropdownType as i32)
    pub dropdown_windows: HashMap<DropdownType, Option<u32>>,

    // Timing
    pub last_mouse_pos: (i32, i32),
    pub mouse_anchor_initialized: bool,

    // Deferred shell/UI actions. These must execute outside the menu state lock because
    // shell push/pop can synchronously re-enter menu shutdown callbacks.
    pub pending_actions: Vec<PendingMainMenuAction>,
    pub system_created: bool,
}

impl Default for MainMenuState {
    fn default() -> Self {
        let mut dropdown_windows = HashMap::new();
        dropdown_windows.insert(DropdownType::None, None);
        dropdown_windows.insert(DropdownType::Single, None);
        dropdown_windows.insert(DropdownType::Multiplayer, None);
        dropdown_windows.insert(DropdownType::Main, None);
        dropdown_windows.insert(DropdownType::LoadReplay, None);
        dropdown_windows.insert(DropdownType::Difficulty, None);

        Self {
            window_ids: WindowIds::default(),
            raise_message_boxes: true,
            campaign_selected: false,
            button_pushed: false,
            is_shutting_down: false,
            start_game: false,
            show_fade: false,
            not_shown: true,
            first_time_running_the_game: true,
            show_logo: false,
            logo_is_shown: false,
            just_entered: false,
            launch_challenge_menu: false,
            dont_allow_transitions: false,
            checking_for_patch_before_gamespy: false,
            cant_connect_before_online: false,
            checks_left_before_online: 0,
            time_through_online: 0,
            online_cancel_window_open: false,
            drop_down: DropdownType::None,
            pending_drop_down: DropdownType::None,
            show_frames: 0,
            show_side: ShowSide::None,
            initial_gadget_delay: INITIAL_GADGET_DELAY_DEFAULT,
            old_disp_settings: DisplaySettings::default(),
            new_disp_settings: DisplaySettings::default(),
            disp_changed: false,
            dropdown_windows,
            last_mouse_pos: (0, 0),
            mouse_anchor_initialized: false,
            pending_actions: Vec::new(),
            system_created: false,
        }
    }
}

// ================================================================================================
// MAIN MENU SYSTEM
// ================================================================================================

/// Main Menu UI System
/// Port of C++ MainMenu.cpp functionality
pub struct MainMenu {
    state: Arc<RwLock<MainMenuState>>,
}

impl MainMenu {
    fn find_live_window(
        &self,
        state: &MainMenuState,
        id: Option<u32>,
        name: Option<&str>,
    ) -> Option<std::rc::Rc<std::cell::RefCell<crate::gui::game_window::GameWindow>>> {
        with_window_manager_ref(|manager| {
            let parent = manager.get_window_by_id(state.window_ids.main_menu_id as i32);
            if let (Some(parent), Some(id)) = (parent.as_ref(), id) {
                if let Some(window) = manager.find_window_from_id(parent, id as i32) {
                    return Some(window);
                }
            }
            if let Some(id) = id {
                if let Some(window) = manager.get_window_by_id(id as i32) {
                    return Some(window);
                }
            }
            name.and_then(|window_name| manager.find_window_by_name(window_name))
        })
    }

    fn hide_window_by_id(
        &self,
        state: &MainMenuState,
        id: u32,
        fallback_name: Option<&str>,
        hide: bool,
    ) {
        if let Some(window) = self.find_live_window(state, Some(id), fallback_name) {
            let _ = window.borrow_mut().hide(hide);
        }
    }

    fn hide_window_by_name(&self, state: &MainMenuState, name: &str, hide: bool) {
        if let Some(window) =
            self.find_live_window(state, Some(NameKeyGenerator::name_to_key(name)), Some(name))
        {
            let _ = window.borrow_mut().hide(hide);
        }
    }

    fn set_dropdown_hidden(&self, state: &MainMenuState, dropdown: DropdownType, hide: bool) {
        // Explicit compat state mirrors C++ GameWindow::hide() per-dropdown visibility,
        // so replacement frontends don't need to re-derive C++ menu transition rules.
        if let Some(Some(window_id)) = state.dropdown_windows.get(&dropdown) {
            self.hide_window_by_id(state, *window_id, None, hide);
        }
    }

    fn show_only_dropdown(&self, state: &MainMenuState, dropdown: DropdownType) {
        for candidate in [
            DropdownType::Single,
            DropdownType::Multiplayer,
            DropdownType::Main,
            DropdownType::LoadReplay,
            DropdownType::Difficulty,
        ] {
            self.set_dropdown_hidden(state, candidate, candidate != dropdown);
        }
    }

    fn reveal_hidden_main_menu(&self, state: &mut MainMenuState) {
        state.initial_gadget_delay = 1;
        state.drop_down = DropdownType::Main;
        self.show_only_dropdown(state, DropdownType::Main);
        self.transition_set_group("MainMenuFade", true);
        self.transition_set_group("MainMenuDefaultMenu", false);
        set_main_menu_cursor_visibility(true);
        state.not_shown = false;
    }

    fn queue_signal_ui_interact(state: &mut MainMenuState, hook: &'static str) {
        Self::queue_action(state, PendingMainMenuAction::SignalUiInteract(hook));
    }

    fn sync_cpp_startup_visibility(&self, state: &MainMenuState) {
        // The C++ init block that hid these controls is commented out, so keep the
        // runtime visibility untouched here as well.
        let _ = state;
    }

    /// Create a new MainMenu instance
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(MainMenuState::default())),
        }
    }

    /// Initialize the main menu
    /// Port of MainMenuInit() - C++ lines 429-653
    pub fn init(
        &mut self,
        layout: &dyn std::any::Any,
        user_data: Option<&dyn std::any::Any>,
    ) -> MainMenuResult<()> {
        log::info!("MainMenuInit: Initializing main menu");

        let mut state = self.state.write().unwrap();

        if let Some(global) = get_global_data() {
            global.write().break_the_movie = false;
        }

        get_shell().show_shell_map(true);
        set_main_menu_cursor_visibility(true);

        // Reset state - matches C++ lines 431-442
        state.button_pushed = false;
        state.is_shutting_down = false;
        state.start_game = false;
        state.show_logo = false;
        state.logo_is_shown = false;
        state.launch_challenge_menu = false;
        state.dont_allow_transitions = false;
        state.first_time_running_the_game = FIRST_TIME_RUNNING_GAME.swap(false, Ordering::SeqCst);
        state.drop_down = DropdownType::None;
        state.pending_drop_down = DropdownType::None;
        state.show_frames = 0;
        state.show_side = ShowSide::None;
        state.mouse_anchor_initialized = false;
        state.last_mouse_pos = (0, 0);
        state.pending_actions.clear();

        // Initialize dropdown windows array - matches C++ lines 441-442
        for i in 0..(DropdownType::Count as usize) {
            if let Some(dropdown_type) = DropdownType::from_i32(i as i32) {
                state.dropdown_windows.insert(dropdown_type, None);
            }
        }
        // Initialize window IDs - matches C++ lines 444-480
        state.window_ids = build_window_ids();

        state.dropdown_windows.insert(
            DropdownType::Single,
            Some(NameKeyGenerator::name_to_key("MainMenu.wnd:MapBorder")),
        );
        state.dropdown_windows.insert(
            DropdownType::Multiplayer,
            Some(NameKeyGenerator::name_to_key("MainMenu.wnd:MapBorder1")),
        );
        state.dropdown_windows.insert(
            DropdownType::Main,
            Some(NameKeyGenerator::name_to_key("MainMenu.wnd:MapBorder2")),
        );
        state.dropdown_windows.insert(
            DropdownType::LoadReplay,
            Some(NameKeyGenerator::name_to_key("MainMenu.wnd:MapBorder3")),
        );
        state.dropdown_windows.insert(
            DropdownType::Difficulty,
            Some(NameKeyGenerator::name_to_key("MainMenu.wnd:MapBorder4")),
        );
        // Hide dropdown windows except main - matches C++ lines 525-526
        for i in 1..(DropdownType::Count as i32) {
            if let Some(dropdown_type) = DropdownType::from_i32(i) {
                if let Some(Some(window_id)) = state.dropdown_windows.get(&dropdown_type) {
                    self.hide_window_by_id(&state, *window_id, None, true);
                }
            }
        }
        state.drop_down = DropdownType::None;
        // Initial hide of faction windows - matches C++ initialHide() lines 360-425
        self.initial_hide(&state);

        // Hide selective buttons - matches C++ line 530
        self.show_selective_buttons(&state, ShowSide::None);

        // Set up the version number and debug buttons would go here (lines 532-570)

        // Show the layout - matches C++ line 579
        if let Some(layout) = layout.downcast_ref::<ManagerWindowLayout>() {
            layout.hide(false);
            with_window_manager(|manager| manager.bring_layout_forward(layout));
        }

        if let Ok(mut map_cache) = get_map_cache_manager().lock() {
            map_cache.update_cache();
        }

        if get_peer_message_queue()
            .and_then(|queue| queue.lock().ok().map(|queue| !queue.is_connected()))
            .unwrap_or(false)
        {
            tear_down_gamespy();
        }

        // Load main menu scheme - matches C++ line 618
        request_shell_menu_scheme("MainMenu");
        state.raise_message_boxes = true;
        // Campaign not selected - matches C++ line 630
        state.campaign_selected = false;

        self.hide_window_by_name(&state, "MainMenu.wnd:MainMenuRuler", true);
        self.sync_cpp_startup_visibility(&state);

        // Handle first time running - matches C++ lines 632-646
        if state.first_time_running_the_game {
            log::debug!("First time running the game - hiding mouse and fading");
            state.not_shown = true;
            set_main_menu_cursor_visibility(false);
            self.transition_reverse("FadeWholeScreen");
        } else {
            state.show_fade = true;
            // Match C++ MainMenuUpdate startup cadence: set justEntered and tick down to 1
            // before applying the default logo fade transition group.
            state.just_entered = true;
            state.initial_gadget_delay = 2;
            self.hide_window_by_name(&state, "MainMenu.wnd:MainMenuRuler", false);
        }

        let focus_id = state.window_ids.main_menu_id as i32;
        drop(state);
        self.focus_window(focus_id);

        log::info!("MainMenuInit: Initialization complete");
        Ok(())
    }

    /// Shutdown the main menu
    /// Port of MainMenuShutdown() - C++ lines 658-690
    pub fn shutdown(
        &mut self,
        layout: &dyn std::any::Any,
        user_data: Option<&dyn std::any::Any>,
    ) -> MainMenuResult<()> {
        log::info!("MainMenuShutdown: Shutting down main menu");

        let mut state = self.state.write().unwrap();

        if !state.start_game {
            state.is_shutting_down = true;
        }

        // Cancel patch check callback - matches C++ line 663
        Self::cancel_patch_check_callback_state(&mut state);

        // Check if we are doing an immediate pop - matches C++ line 666
        let pop_immediate = user_data
            .and_then(|data| data.downcast_ref::<bool>())
            .copied()
            .unwrap_or(false);

        if pop_immediate {
            // Complete shutdown immediately - matches C++ lines 673-682
            self.finish_shutdown_complete(Some(layout), &mut state)?;
            drop(state);
            self.complete_shell_shutdown()?;
            log::info!("Main menu shutdown complete");
            return Ok(());
        }

        if !state.start_game {
            // TheShell->reverseAnimatewindow() - matches C++ line 686
            log::debug!("Reversing window animation");
            get_shell().reverse_animate_window();
        }

        log::info!("MainMenuShutdown: Shutdown complete");
        Ok(())
    }

    /// Update the main menu
    /// Port of MainMenuUpdate() - C++ lines 809-952
    pub fn update(
        &mut self,
        layout: &dyn std::any::Any,
        user_data: Option<&dyn std::any::Any>,
    ) -> MainMenuResult<()> {
        let mut state = self.state.write().unwrap();
        let mut focus_target = None;
        let mut pending_actions = Vec::new();
        if TheGameLogic::is_in_game()
            && TheGameLogic::get_game_mode() != gamelogic::system::game_logic::GAME_SHELL
        {
            return Ok(());
        }

        if get_dont_show_main_menu() && state.just_entered {
            state.just_entered = false;
        }

        if let Ok(mut manager_guard) = download_manager().lock() {
            let should_update = manager_guard
                .as_ref()
                .map(|manager| !manager.is_done())
                .unwrap_or(false);
            if should_update {
                if let Some(manager) = manager_guard.as_mut() {
                    let _ = manager.update();
                }
                if let Some(layout) = layout.downcast_ref::<ManagerWindowLayout>() {
                    download_menu_update(layout, None);
                }
            }
        }

        if state.just_entered {
            if state.initial_gadget_delay == 1 {
                self.transition_set_group("MainMenuDefaultMenuLogoFade", false);
                focus_target = Some(state.window_ids.main_menu_id as i32);
                state.initial_gadget_delay = 2;
                state.just_entered = false;
                log::debug!("Just entered - setting up main menu transitions");
            } else {
                state.initial_gadget_delay -= 1;
            }
        }

        // Handle transitions - matches C++ lines 847-848
        if state.dont_allow_transitions {
            if self.transitions_finished() {
                state.dont_allow_transitions = false;
            }
        }

        // Show logo logic - matches C++ lines 850-878
        if state.show_logo && !state.dont_allow_transitions {
            match state.show_side {
                ShowSide::Training => {
                    self.transition_set_group("MainMenuFactionTraining", false);
                    log::debug!("Showing training faction logo");
                }
                ShowSide::China => {
                    self.transition_set_group("MainMenuFactionChina", false);
                    log::debug!("Showing China faction logo");
                }
                ShowSide::GLA => {
                    self.transition_set_group("MainMenuFactionGLA", false);
                    log::debug!("Showing GLA faction logo");
                }
                ShowSide::USA => {
                    self.transition_set_group("MainMenuFactionUS", false);
                    log::debug!("Showing USA faction logo");
                }
                ShowSide::Skirmish => {
                    self.transition_set_group("MainMenuFactionSkirmish", false);
                    log::debug!("Showing Skirmish faction logo");
                }
                ShowSide::None => {}
            }
            state.show_logo = false;
        }

        // Raise message boxes - matches C++ lines 901-905
        if state.raise_message_boxes {
            raise_gamespy_message_boxes();
            state.raise_message_boxes = false;
        }

        // HTTP and GameSpy updates - matches C++ lines 907-908
        self.http_think_wrapper(&mut state);
        update_gamespy_overlays();

        // Check if we should start the game - matches C++ lines 933-936
        if state.start_game {
            if get_shell().is_anim_finished() && self.transitions_finished() {
                self.do_game_start(&mut state)?;
            }
        }

        // Check if shutdown is complete - matches C++ lines 939-942
        if state.is_shutting_down {
            if get_shell().is_anim_finished() && self.transitions_finished() {
                self.finish_shutdown_complete(Some(layout), &mut state)?;
                drop(state);
                self.complete_shell_shutdown()?;
                log::info!("Main menu shutdown complete");
                return Ok(());
            }
        }

        pending_actions.append(&mut state.pending_actions);
        drop(state);
        if let Some(id) = focus_target {
            self.focus_window(id);
        }
        self.execute_pending_actions(pending_actions);

        Ok(())
    }

    /// Handle input messages
    /// Port of MainMenuInput() - C++ lines 957-1016
    pub fn input(&mut self, window: u32, msg: u32, data1: u32, data2: u32) -> bool {
        let mut state = self.state.write().unwrap();

        if !state.not_shown {
            return false; // MSG_IGNORED
        }

        match msg {
            // GWM_MOUSE_POS - matches C++ lines 968-995
            GWM_MOUSE_POS => {
                let mouse_x = (data1 as u16) as i16 as i32;
                let mouse_y = ((data1 >> 16) as u16) as i16 as i32;

                if mouse_x == 0 && mouse_y == 0 {
                    return false;
                }

                if !state.mouse_anchor_initialized {
                    state.last_mouse_pos = (mouse_x, mouse_y);
                    state.mouse_anchor_initialized = true;
                    return false;
                }

                let (last_x, last_y) = state.last_mouse_pos;
                if (mouse_x - last_x).abs() > 20 || (mouse_y - last_y).abs() > 20 {
                    log::debug!("Mouse moved significantly: ({}, {})", mouse_x, mouse_y);

                    if state.not_shown {
                        self.reveal_hidden_main_menu(&mut state);
                        return true; // MSG_HANDLED
                    }
                }
            }

            // GWM_CHAR - matches C++ lines 996-1009
            GWM_CHAR => {
                if state.not_shown {
                    self.reveal_hidden_main_menu(&mut state);
                    return true; // MSG_HANDLED
                }
            }

            _ => {}
        }

        false // MSG_IGNORED
    }

    /// Handle system messages
    /// Port of MainMenuSystem() - C++ lines 1021-1688
    pub fn system(&mut self, window: u32, msg: u32, data1: u32, data2: u32) -> bool {
        let mut state = self.state.write().unwrap();
        let mut pending_actions = Vec::new();

        match msg {
            // GWM_CREATE - matches C++ lines 1031-1035
            GWM_CREATE => {
                if window != build_window_ids().main_menu_id {
                    return false;
                }
                if state.system_created {
                    return true;
                }
                state.system_created = true;
                http_startup();
                log::debug!("Main menu window created");
                return true;
            }

            // GWM_DESTROY - matches C++ lines 1038-1046
            GWM_DESTROY => {
                if window != build_window_ids().main_menu_id {
                    return false;
                }
                if !state.system_created {
                    return true;
                }
                state.system_created = false;
                http_cleanup();
                tear_down_gamespy();
                stop_async_dns_check();
                log::debug!("Main menu window destroyed");
                return true;
            }

            // GWM_INPUT_FOCUS - matches C++ lines 1049-1058
            GWM_INPUT_FOCUS => {
                if window != build_window_ids().main_menu_id {
                    return false;
                }
                return Self::write_input_focus_response(data1, data2 as usize);
            }

            // GBM_MOUSE_ENTERING - matches C++ lines 1060-1176
            GBM_MOUSE_ENTERING => {
                let control_id = data1;
                self.handle_mouse_entering(&mut state, control_id);
                pending_actions.append(&mut state.pending_actions);
                drop(state);
                self.execute_pending_actions(pending_actions);
                return true;
            }

            // GBM_MOUSE_LEAVING - matches C++ lines 1178-1277
            GBM_MOUSE_LEAVING => {
                let control_id = data1;
                self.handle_mouse_leaving(&mut state, control_id);
                pending_actions.append(&mut state.pending_actions);
                drop(state);
                self.execute_pending_actions(pending_actions);
                return true;
            }

            // GBM_SELECTED - matches C++ lines 1279-1677
            GBM_SELECTED => {
                let control_id = data1;
                if state.button_pushed {
                    return true;
                }

                if let Err(e) = self.handle_button_selected(&mut state, control_id) {
                    log::error!("Error handling button selection: {}", e);
                }
                pending_actions.append(&mut state.pending_actions);
                drop(state);
                self.execute_pending_actions(pending_actions);
                return true;
            }

            _ => {}
        }

        false // MSG_IGNORED
    }

    // ============================================================================================
    // PRIVATE HELPER METHODS
    // ============================================================================================

    fn transition_set_group(&self, group: &str, immediate: bool) {
        with_window_manager(|manager| manager.transition_set_group(group, immediate));
    }

    fn queue_action(state: &mut MainMenuState, action: PendingMainMenuAction) {
        state.pending_actions.push(action);
    }

    fn execute_pending_actions(&self, actions: Vec<PendingMainMenuAction>) {
        for action in actions {
            match action {
                PendingMainMenuAction::PushShellScreen(screen) => {
                    if let Err(err) = get_shell().push(screen, false) {
                        log::warn!("Main menu push failed for {}: {}", screen, err);
                    }
                }
                PendingMainMenuAction::ReverseTransitionGroup(group) => {
                    self.transition_reverse(group);
                }
                PendingMainMenuAction::ShowOptionsLayout => {
                    let mut shell = get_shell();
                    if let Some(layout) = shell.get_options_layout(true) {
                        if let Err(err) = layout.run_init(None) {
                            log::warn!("Options layout init failed: {}", err);
                        }
                        layout.hide(false);
                        layout.bring_forward();
                    }
                }
                PendingMainMenuAction::SignalUiInteract(hook) => {
                    TheScriptEngine::signal_ui_interact(hook);
                }
                PendingMainMenuAction::ReverseAnimateWindow => {
                    get_shell().reverse_animate_window();
                }
                PendingMainMenuAction::StartPatchCheck => self.start_patch_check(),
                PendingMainMenuAction::StartDownloadingPatches => {
                    self.start_downloading_patches();
                }
                PendingMainMenuAction::LaunchWorldBuilder => self.launch_world_builder(),
                PendingMainMenuAction::QuitRequest => self.perform_quit_request(),
            }
        }
    }

    fn perform_quit_request(&self) {
        let windowed = get_global_data()
            .map(|data| data.read().windowed)
            .unwrap_or(true);
        if windowed {
            Self::perform_quit_now();
            return;
        }

        let yes: MessageBoxFunc = Box::new(Self::perform_quit_now);
        let _ = quit_message_box_yes_no(
            &crate::game_text::GameText::fetch("GUI:QuitPopupTitle"),
            &crate::game_text::GameText::fetch("GUI:QuitPopupMessage"),
            Some(yes),
            None,
        );
    }

    fn perform_quit_now() {
        TheScriptEngine::signal_ui_interact("ShellMainMenuExitSelected");
        if let Err(err) = get_shell().pop() {
            log::warn!("Main menu quit pop failed: {}", err);
        }
        if let Some(engine) = get_game_engine() {
            engine.lock().set_quitting(true);
        }
        if TheGameLogic::is_in_game() {
            let message_stream = get_message_stream();
            let mut stream = message_stream.write().unwrap();
            stream.append_message(GameMessageType::ClearGameData);
        }
    }

    fn start_patch_check(&self) {
        let mut state = self.state.write().unwrap();
        state.checking_for_patch_before_gamespy = true;
        state.cant_connect_before_online = false;
        state.checks_left_before_online = 4;
        state.time_through_online = state.time_through_online.wrapping_add(1);
        state.online_cancel_window_open = true;

        log::info!("StartPatchCheck requested");
    }

    fn start_downloading_patches(&self) {
        let has_queued_downloads = download_manager()
            .lock()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .map(|manager| manager.is_active() || manager.is_file_queued_for_download())
            })
            .unwrap_or(false);

        if !has_queued_downloads {
            get_main_menu().handle_canceled_download(true);
            log::info!("StartDownloadingPatches requested with empty queue");
            return;
        }

        with_window_manager(|manager| {
            if let Ok((layout, _info)) =
                manager.create_layout_with_windows("Menus/DownloadMenu.wnd")
            {
                {
                    let layout_ref = layout.borrow();
                    layout_ref.run_init(None);
                    layout_ref.hide(false);
                }
                layout.borrow_mut().bring_forward();
            }
        });

        get_main_menu().handle_canceled_download(false);

        if let Ok(mut guard) = download_manager().lock() {
            if let Some(manager) = guard.as_mut() {
                let _ = manager.download_next_queued_file();
            }
        }
        log::info!("StartDownloadingPatches requested");
    }

    fn launch_world_builder(&self) {
        let candidates: &[&str] = if cfg!(debug_assertions) {
            &["WorldBuilderD.exe", "WorldBuilderI.exe", "WorldBuilder.exe"]
        } else {
            &["WorldBuilder.exe", "WorldBuilderI.exe", "WorldBuilderD.exe"]
        };
        let launched = candidates
            .iter()
            .any(|exe| std::process::Command::new(exe).spawn().is_ok());
        if !launched {
            let _ = message_box_ok(
                &crate::game_text::GameText::fetch("GUI:WorldBuilder"),
                &crate::game_text::GameText::fetch("GUI:WorldBuilderLoadFailed"),
                None,
            );
        }
    }

    fn to_campaign_difficulty(diff: GameDifficulty) -> CampaignDifficulty {
        match diff {
            GameDifficulty::Easy => CampaignDifficulty::Easy,
            GameDifficulty::Normal => CampaignDifficulty::Normal,
            GameDifficulty::Hard => CampaignDifficulty::Hard,
        }
    }

    fn to_challenge_difficulty(diff: GameDifficulty) -> ChallengeGameDifficulty {
        match diff {
            GameDifficulty::Easy => ChallengeGameDifficulty::Easy,
            GameDifficulty::Normal => ChallengeGameDifficulty::Normal,
            GameDifficulty::Hard => ChallengeGameDifficulty::Hard,
        }
    }

    fn transition_reverse(&self, group: &str) {
        with_window_manager(|manager| manager.transition_reverse(group));
    }

    fn transition_remove(&self, group: &str, skip_pending: bool) {
        with_window_manager(|manager| manager.transition_remove(group, skip_pending));
    }

    fn transitions_finished(&self) -> bool {
        with_window_manager_ref(|manager| manager.transitions_finished())
    }

    fn focus_window(&self, id: i32) {
        with_window_manager(|manager| {
            manager.request_focus(id);
        });
    }

    /// Handle mouse entering a control
    /// Port of GBM_MOUSE_ENTERING handler - C++ lines 1060-1176
    fn handle_mouse_entering(&self, state: &mut MainMenuState, control_id: u32) {
        // Check which control the mouse is entering
        if control_id == state.window_ids.online_id {
            Self::queue_signal_ui_interact(state, "ShellMainMenuOnlineHighlighted");
            log::debug!("Mouse entering Online button");
        } else if control_id == state.window_ids.network_id {
            Self::queue_signal_ui_interact(state, "ShellMainMenuNetworkHighlighted");
            log::debug!("Mouse entering Network button");
        } else if control_id == state.window_ids.options_id {
            Self::queue_signal_ui_interact(state, "ShellMainMenuOptionsHighlighted");
            log::debug!("Mouse entering Options button");
        } else if control_id == state.window_ids.exit_id {
            Self::queue_signal_ui_interact(state, "ShellMainMenuExitHighlighted");
            log::debug!("Mouse entering Exit button");
        } else if control_id == state.window_ids.button_challenge_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::Training;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_set_group("MainMenuFactionTraining", false);
            log::debug!("Mouse entering Challenge button");
        } else if control_id == state.window_ids.skirmish_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::Skirmish;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_set_group("MainMenuFactionSkirmish", false);
            log::debug!("Mouse entering Skirmish button");
        } else if control_id == state.window_ids.button_usa_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::USA;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_set_group("MainMenuFactionUS", false);
            log::debug!("Mouse entering USA button");
        } else if control_id == state.window_ids.button_gla_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::GLA;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_set_group("MainMenuFactionGLA", false);
            log::debug!("Mouse entering GLA button");
        } else if control_id == state.window_ids.button_china_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::China;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_set_group("MainMenuFactionChina", false);
            log::debug!("Mouse entering China button");
        }
    }

    /// Handle mouse leaving a control
    /// Port of GBM_MOUSE_LEAVING handler - C++ lines 1178-1277
    fn handle_mouse_leaving(&self, state: &mut MainMenuState, control_id: u32) {
        if control_id == state.window_ids.online_id {
            Self::queue_signal_ui_interact(state, "ShellMainMenuOnlineUnhighlighted");
            log::debug!("Mouse leaving Online button");
        } else if control_id == state.window_ids.network_id {
            Self::queue_signal_ui_interact(state, "ShellMainMenuNetworkUnhighlighted");
            log::debug!("Mouse leaving Network button");
        } else if control_id == state.window_ids.options_id {
            Self::queue_signal_ui_interact(state, "ShellMainMenuOptionsUnhighlighted");
            log::debug!("Mouse leaving Options button");
        } else if control_id == state.window_ids.exit_id {
            Self::queue_signal_ui_interact(state, "ShellMainMenuExitUnhighlighted");
            log::debug!("Mouse leaving Exit button");
        } else if control_id == state.window_ids.button_challenge_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_reverse("MainMenuFactionTraining");
            log::debug!("Mouse leaving Challenge button");
        } else if control_id == state.window_ids.skirmish_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_reverse("MainMenuFactionSkirmish");
            log::debug!("Mouse leaving Skirmish button");
        } else if control_id == state.window_ids.button_usa_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_reverse("MainMenuFactionUS");
            log::debug!("Mouse leaving USA button");
        } else if control_id == state.window_ids.button_gla_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_reverse("MainMenuFactionGLA");
            log::debug!("Mouse leaving GLA button");
        } else if control_id == state.window_ids.button_china_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            self.transition_reverse("MainMenuFactionChina");
            log::debug!("Mouse leaving China button");
        }
    }

    /// Handle button selected
    /// Port of GBM_SELECTED handler - C++ lines 1279-1677
    fn handle_button_selected(
        &self,
        state: &mut MainMenuState,
        control_id: u32,
    ) -> MainMenuResult<()> {
        // Don't allow mouse click slop during transitions - matches C++ lines 1304-1310
        if control_id != state.window_ids.button_easy_id
            && control_id != state.window_ids.button_medium_id
            && control_id != state.window_ids.button_hard_id
            && self.transitions_finished()
        {
            state.launch_challenge_menu = false;
        }

        // Handle each button type - matches C++ lines 1313-1673
        if control_id == state.window_ids.button_single_player_id {
            // Single Player button - C++ lines 1313-1324
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Single;
            self.show_only_dropdown(&state, DropdownType::Single);
            self.transition_remove("MainMenuDefaultMenu", false);
            self.transition_reverse("MainMenuDefaultMenuBack");
            self.transition_set_group("MainMenuSinglePlayerMenu", false);
            log::info!("Single Player button selected");
        } else if control_id == state.window_ids.button_single_back_id {
            // Single Player Back button - C++ lines 1326-1335
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.button_pushed = false;
            state.drop_down = DropdownType::Main;
            self.show_only_dropdown(&state, DropdownType::Main);
            self.transition_remove("MainMenuSinglePlayerMenu", false);
            self.transition_reverse("MainMenuSinglePlayerMenuBack");
            self.transition_set_group("MainMenuDefaultMenu", false);
            state.dont_allow_transitions = true;
            log::info!("Single Player Back button selected");
        } else if control_id == state.window_ids.button_multi_back_id {
            // Multiplayer Back button - C++ lines 1336-1346
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Main;
            self.show_only_dropdown(&state, DropdownType::Main);
            self.transition_remove("MainMenuMultiPlayerMenu", false);
            self.transition_reverse("MainMenuMultiPlayerMenuReverse");
            self.transition_set_group("MainMenuDefaultMenu", false);
            log::info!("Multiplayer Back button selected");
        } else if control_id == state.window_ids.button_load_replay_back_id {
            // Load Replay Back button - C++ lines 1347-1357
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Main;
            self.show_only_dropdown(&state, DropdownType::Main);
            self.transition_remove("MainMenuLoadReplayMenu", false);
            self.transition_reverse("MainMenuLoadReplayMenuBack");
            self.transition_set_group("MainMenuDefaultMenu", false);
            log::info!("Load Replay Back button selected");
        } else if control_id == state.window_ids.button_credits_id {
            // Credits button - C++ lines 1359-1368
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/CreditsMenu.wnd"),
            );
            state.drop_down = DropdownType::Main;
            self.show_only_dropdown(&state, DropdownType::Main);
            self.transition_reverse("MainMenuDefaultMenu");
            log::info!("Credits button selected");
        } else if control_id == state.window_ids.button_multi_player_id {
            // Multiplayer button - C++ lines 1369-1380
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::Multiplayer;
            self.show_only_dropdown(&state, DropdownType::Multiplayer);
            self.transition_remove("MainMenuDefaultMenu", false);
            self.transition_reverse("MainMenuDefaultMenuBack");
            self.transition_set_group("MainMenuMultiPlayerMenu", false);
            log::info!("Multiplayer button selected");
        } else if control_id == state.window_ids.button_load_replay_id {
            // Load Replay button - C++ lines 1381-1392
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = false;
            state.drop_down = DropdownType::LoadReplay;
            self.show_only_dropdown(&state, DropdownType::LoadReplay);
            self.transition_remove("MainMenuDefaultMenu", false);
            self.transition_reverse("MainMenuDefaultMenuBack");
            self.transition_set_group("MainMenuLoadReplayMenu", false);
            log::info!("Load Replay button selected");
        } else if control_id == state.window_ids.button_load_id {
            // Load Game button - C++ lines 1393-1409
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::LoadReplay;
            self.show_only_dropdown(&state, DropdownType::LoadReplay);
            self.transition_reverse("MainMenuLoadReplayMenuBackTransition");
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/SaveLoad.wnd"),
            );
            log::info!("Load Game button selected");
        } else if control_id == state.window_ids.button_replay_id {
            // Replay button - C++ lines 1410-1419
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::LoadReplay;
            self.show_only_dropdown(&state, DropdownType::LoadReplay);
            self.transition_reverse("MainMenuLoadReplayMenuBackTransition");
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/ReplayMenu.wnd"),
            );
            log::info!("Replay button selected");
        } else if control_id == state.window_ids.skirmish_id {
            // Skirmish button - C++ lines 1420-1443
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.launch_challenge_menu = false;
            state.button_pushed = true;
            state.campaign_selected = true;
            state.drop_down = DropdownType::Single;
            self.show_only_dropdown(&state, DropdownType::Single);
            self.transition_remove("MainMenuFactionSkirmish", false);
            self.transition_reverse("MainMenuSinglePlayerMenuBackSkirmish");
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/SkirmishGameOptionsMenu.wnd"),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::SignalUiInteract("ShellMainMenuSkirmishSelected"),
            );
            log::info!("Skirmish button selected");
        } else if control_id == state.window_ids.online_id {
            // Online button - C++ lines 1444-1457
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::Multiplayer;
            self.show_only_dropdown(&state, DropdownType::Multiplayer);
            self.transition_reverse("MainMenuMultiPlayerMenuTransitionToNext");
            Self::queue_action(state, PendingMainMenuAction::StartPatchCheck);
            state.drop_down = DropdownType::None;
            log::info!("Online button selected");
        } else if control_id == state.window_ids.network_id {
            // Network button - C++ lines 1458-1469
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.button_pushed = true;
            state.drop_down = DropdownType::Multiplayer;
            self.show_only_dropdown(&state, DropdownType::Multiplayer);
            self.transition_reverse("MainMenuMultiPlayerMenuTransitionToNext");
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/LanLobbyMenu.wnd"),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::SignalUiInteract("ShellMainMenuNetworkSelected"),
            );
            log::info!("Network button selected");
        } else if control_id == state.window_ids.options_id {
            // Options button - C++ lines 1470-1484
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            Self::queue_action(
                state,
                PendingMainMenuAction::SignalUiInteract("ShellMainMenuOptionsSelected"),
            );
            Self::queue_action(state, PendingMainMenuAction::ShowOptionsLayout);
            log::info!("Options button selected");
        } else if control_id == state.window_ids.world_builder_id {
            // World Builder button - C++ lines 1485-1497
            Self::queue_action(state, PendingMainMenuAction::LaunchWorldBuilder);
            log::info!("World Builder button selected");
        } else if control_id == state.window_ids.get_update_id {
            // Get Update button - C++ lines 1498-1501
            Self::queue_action(state, PendingMainMenuAction::StartDownloadingPatches);
            log::info!("Get Update button selected");
        } else if control_id == state.window_ids.exit_id {
            // Exit button - C++ lines 1502-1521
            self.quit_callback(state);
            log::info!("Exit button selected");
        } else if control_id == state.window_ids.button_challenge_id {
            // Challenge button - C++ lines 1522-1538
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }

            state.drop_down = DropdownType::Difficulty;
            self.transition_set_group("MainMenuFactionTraining", false);
            self.hide_window_by_name(state, "MainMenu.wnd:WinFactionTraining", true);
            self.transition_reverse("MainMenuSinglePlayerMenuBackTraining");
            self.transition_set_group("MainMenuDifficultyMenuTraining", false);
            state.campaign_selected = true;
            state.show_logo = false;
            state.show_side = ShowSide::Training;
            state.launch_challenge_menu = true;
            log::info!("Challenge button selected");
        } else if control_id == state.window_ids.button_usa_id {
            // USA Campaign button - C++ lines 1560-1587
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.launch_challenge_menu = false;
            state.drop_down = DropdownType::Difficulty;
            get_campaign_manager().set_campaign("USA");
            self.transition_set_group("MainMenuFactionUS", false);
            self.transition_remove("MainMenuFactionUS", true);
            self.hide_window_by_name(state, "MainMenu.wnd:WinFactionUS", true);
            self.transition_reverse("MainMenuSinglePlayerMenuBackUS");
            self.transition_set_group("MainMenuDifficultyMenuUS", false);
            state.campaign_selected = true;
            state.logo_is_shown = false;
            state.show_logo = false;
            state.show_side = ShowSide::USA;
            log::info!("USA Campaign button selected");
        } else if control_id == state.window_ids.button_gla_id {
            // GLA Campaign button - C++ lines 1588-1615
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.launch_challenge_menu = false;
            state.drop_down = DropdownType::Difficulty;
            get_campaign_manager().set_campaign("GLA");
            self.transition_set_group("MainMenuFactionGLA", false);
            self.transition_remove("MainMenuFactionGLA", true);
            self.hide_window_by_name(state, "MainMenu.wnd:WinFactionGLA", true);
            self.transition_reverse("MainMenuSinglePlayerMenuBackGLA");
            self.transition_set_group("MainMenuDifficultyMenuGLA", false);
            state.campaign_selected = true;
            state.logo_is_shown = false;
            state.show_logo = false;
            state.show_side = ShowSide::GLA;
            log::info!("GLA Campaign button selected");
        } else if control_id == state.window_ids.button_china_id {
            // China Campaign button - C++ lines 1616-1643
            if state.campaign_selected || state.dont_allow_transitions {
                return Ok(());
            }
            state.launch_challenge_menu = false;
            state.drop_down = DropdownType::Difficulty;
            get_campaign_manager().set_campaign("China");
            self.transition_set_group("MainMenuFactionChina", false);
            self.transition_remove("MainMenuFactionChina", true);
            self.hide_window_by_name(state, "MainMenu.wnd:WinFactionChina", true);
            self.transition_reverse("MainMenuSinglePlayerMenuBackChina");
            self.transition_set_group("MainMenuDifficultyMenuChina", false);
            state.campaign_selected = true;
            state.logo_is_shown = false;
            state.show_logo = false;
            state.show_side = ShowSide::China;
            log::info!("China Campaign button selected");
        } else if control_id == state.window_ids.button_easy_id {
            // Easy difficulty - C++ lines 1644-1650
            if state.dont_allow_transitions {
                return Ok(());
            }
            self.check_cd_before_campaign(state, GameDifficulty::Easy);
            log::info!("Easy difficulty selected");
        } else if control_id == state.window_ids.button_medium_id {
            // Medium difficulty - C++ lines 1651-1657
            if state.dont_allow_transitions {
                return Ok(());
            }
            self.check_cd_before_campaign(state, GameDifficulty::Normal);
            log::info!("Medium difficulty selected");
        } else if control_id == state.window_ids.button_hard_id {
            // Hard difficulty - C++ lines 1658-1664
            if state.dont_allow_transitions {
                return Ok(());
            }
            self.check_cd_before_campaign(state, GameDifficulty::Hard);
            log::info!("Hard difficulty selected");
        } else if control_id == state.window_ids.button_diff_back_id {
            // Difficulty Back button - C++ lines 1665-1673
            if state.dont_allow_transitions {
                return Ok(());
            }
            state.dont_allow_transitions = true;
            state.launch_challenge_menu = false;
            state.drop_down = DropdownType::Single;
            get_campaign_manager().set_campaign("");
            self.diff_reverse_side(state);
            state.campaign_selected = false;
            log::info!("Difficulty Back button selected");
        }

        Ok(())
    }

    /// Initial hide of faction windows
    /// Port of initialHide() - C++ lines 360-425
    fn initial_hide(&self, state: &MainMenuState) {
        for name in [
            "MainMenu.wnd:WinFactionGLA",
            "MainMenu.wnd:WinFactionChina",
            "MainMenu.wnd:WinFactionUS",
            "MainMenu.wnd:WinGrowMarker",
            "MainMenu.wnd:WinFactionTraining",
            "MainMenu.wnd:WinFactionTrainingSmall",
            "MainMenu.wnd:WinFactionTrainingMedium",
            "MainMenu.wnd:WinFactionSkirmish",
            "MainMenu.wnd:WinFactionSkirmishSmall",
            "MainMenu.wnd:WinFactionSkirmishMedium",
            "MainMenu.wnd:WinFactionUS",
            "MainMenu.wnd:WinFactionUSSmall",
            "MainMenu.wnd:WinFactionUSMedium",
            "MainMenu.wnd:WinFactionGLA",
            "MainMenu.wnd:WinFactionGLASmall",
            "MainMenu.wnd:WinFactionGLAMedium",
            "MainMenu.wnd:WinFactionChina",
            "MainMenu.wnd:WinFactionChinaSmall",
            "MainMenu.wnd:WinFactionChinaMedium",
        ] {
            self.hide_window_by_name(state, name, true);
        }
    }

    /// Show selective buttons based on faction
    /// Port of showSelectiveButtons() - C++ lines 217-225
    fn show_selective_buttons(&self, state: &MainMenuState, show: ShowSide) {
        self.hide_window_by_id(
            state,
            state.window_ids.button_usa_recent_save_id,
            Some("MainMenu.wnd:ButtonUSARecentSave"),
            show != ShowSide::USA,
        );
        self.hide_window_by_id(
            state,
            state.window_ids.button_usa_load_game_id,
            Some("MainMenu.wnd:ButtonUSALoadGame"),
            show != ShowSide::USA,
        );
        self.hide_window_by_id(
            state,
            state.window_ids.button_gla_recent_save_id,
            Some("MainMenu.wnd:ButtonGLARecentSave"),
            show != ShowSide::GLA,
        );
        self.hide_window_by_id(
            state,
            state.window_ids.button_gla_load_game_id,
            Some("MainMenu.wnd:ButtonGLALoadGame"),
            show != ShowSide::GLA,
        );
        self.hide_window_by_id(
            state,
            state.window_ids.button_china_recent_save_id,
            Some("MainMenu.wnd:ButtonChinaRecentSave"),
            show != ShowSide::China,
        );
        self.hide_window_by_id(
            state,
            state.window_ids.button_china_load_game_id,
            Some("MainMenu.wnd:ButtonChinaLoadGame"),
            show != ShowSide::China,
        );
    }

    /// Quit callback
    /// Port of quitCallback() - C++ lines 227-250
    fn quit_callback(&self, state: &mut MainMenuState) {
        state.button_pushed = true;
        Self::queue_action(state, PendingMainMenuAction::QuitRequest);
        log::info!("Quit callback - exiting game");
    }

    /// Setup game start
    /// Port of setupGameStart() - C++ lines 253-273
    fn setup_game_start(&self, state: &mut MainMenuState, map_name: &str, diff: GameDifficulty) {
        {
            let mut campaign_manager = get_campaign_manager();
            campaign_manager.set_game_difficulty(Self::to_campaign_difficulty(diff));
        }

        if state.launch_challenge_menu {
            if let Some(mut generals) = get_challenge_generals_mut() {
                generals.set_current_difficulty(Self::to_challenge_difficulty(diff));
            }
            state.campaign_selected = true;
            Self::queue_action(
                state,
                PendingMainMenuAction::PushShellScreen("Menus/ChallengeMenu.wnd"),
            );
            Self::queue_action(
                state,
                PendingMainMenuAction::ReverseTransitionGroup("MainMenuDifficultyMenuTraining"),
            );
            log::info!("Launching challenge menu with difficulty: {:?}", diff);
        } else {
            state.start_game = true;
            if let Some(data) = get_global_data() {
                data.write().pending_file = map_name.to_string();
            }
            Self::queue_action(state, PendingMainMenuAction::ReverseAnimateWindow);
            self.transition_set_group("FadeWholeScreen", false);
            log::info!("Starting game: {} with difficulty: {:?}", map_name, diff);
        }
    }

    /// Public wrapper used by external menu callbacks that must follow
    /// MainMenu::setupGameStart behavior (for example DifficultySelect popup flow).
    pub fn setup_game_start_from_callback(&mut self, map_name: &str, diff: GameDifficulty) {
        let mut state = self.state.write().unwrap();
        self.setup_game_start(&mut state, map_name, diff);
    }

    /// Prepare campaign game
    /// Port of prepareCampaignGame() - C++ lines 275-286
    fn prepare_campaign_game(&self, state: &mut MainMenuState, diff: GameDifficulty) {
        state.dont_allow_transitions = true;

        {
            let mut campaign_manager = get_campaign_manager();
            campaign_manager.set_game_difficulty(Self::to_campaign_difficulty(diff));
        }
        let mut prefs = UserPreferences::new();
        let _ = prefs.load("Options.ini");
        prefs.set_int("CampaignDifficulty", diff as i32);
        let _ = prefs.write();
        TheScriptEngine::set_global_difficulty(diff as i32);

        state.button_pushed = false;
        self.transition_reverse("MainMenuDifficultyMenuBack");
        let map_name = { get_campaign_manager().get_current_map() };
        if let Some(map_name) = map_name {
            self.setup_game_start(state, &map_name, diff);
        } else {
            log::warn!("prepare_campaign_game without current campaign map");
        }

        log::info!("Preparing campaign game with difficulty: {:?}", diff);
    }

    fn run_campaign_start_after_cd_check(&mut self, diff: GameDifficulty) {
        let mut state = self.state.write().unwrap();
        self.prepare_campaign_game(&mut state, diff);
    }

    fn cancel_campaign_start_after_cd_check(&mut self) {
        let mut state = self.state.write().unwrap();
        state.button_pushed = false;
    }

    fn is_first_cd_present() -> bool {
        get_protection_manager()
            .map(|mut manager| manager.comprehensive_validation().status == ProtectionStatus::Valid)
            .unwrap_or(true)
    }

    /// Check CD before starting campaign
    /// Port of checkCDBeforeCampaign() - C++ lines 323-335
    fn check_cd_before_campaign(&self, state: &mut MainMenuState, diff: GameDifficulty) {
        if !Self::is_first_cd_present() {
            state.button_pushed = false;
            let ok: MessageBoxFunc = Box::new(move || {
                let mut menu = get_main_menu();
                menu.run_campaign_start_after_cd_check(diff);
            });
            let cancel: MessageBoxFunc = Box::new(|| {
                let mut menu = get_main_menu();
                menu.cancel_campaign_start_after_cd_check();
            });
            let _ = message_box_ok_cancel(
                &crate::game_text::GameText::fetch("GUI:InsertCDPrompt"),
                &crate::game_text::GameText::fetch("GUI:InsertCDMessage"),
                Some(ok),
                Some(cancel),
            );
            return;
        }

        self.prepare_campaign_game(state, diff);
        log::info!("Checking CD before campaign start");
    }

    /// Do game start
    /// Port of doGameStart() - C++ lines 306-321
    fn do_game_start(&self, state: &mut MainMenuState) -> MainMenuResult<()> {
        state.start_game = false;

        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }

        let (difficulty, rank_points) = {
            let campaign_manager = get_campaign_manager();
            (
                campaign_manager.get_game_difficulty() as i32,
                campaign_manager.get_rank_points(),
            )
        };
        let message_stream = get_message_stream();
        let mut stream = message_stream.write().unwrap();
        let msg = stream.append_message(GameMessageType::NewGame);
        msg.append_integer_argument(gamelogic::system::game_logic::GAME_SINGLE_PLAYER);
        msg.append_integer_argument(difficulty);
        msg.append_integer_argument(rank_points);
        init_random_with_seed(0);

        state.is_shutting_down = true;
        log::info!("Starting new game");
        Ok(())
    }

    /// Finish the layout/state side of shutdownComplete() without re-entering shell shutdown.
    /// Port of the layout/state portion of shutdownComplete() - C++ lines 337-347.
    fn finish_shutdown_complete(
        &self,
        layout: Option<&dyn std::any::Any>,
        state: &mut MainMenuState,
    ) -> MainMenuResult<()> {
        if let Some(layout) = layout.and_then(|any| any.downcast_ref::<ManagerWindowLayout>()) {
            layout.hide(true);
        }
        state.is_shutting_down = false;
        Ok(())
    }

    fn complete_shell_shutdown(&self) -> MainMenuResult<()> {
        get_shell()
            .shutdown_complete(None, false)
            .map_err(|err| MainMenuError::ShutdownFailed(err.to_string()))
    }

    /// Reverse side for difficulty menu
    /// Port of diffReverseSide() - C++ lines 1690-1711
    fn diff_reverse_side(&self, state: &MainMenuState) {
        match state.show_side {
            ShowSide::Training => {
                self.transition_reverse("MainMenuDifficultyMenuTrainingBack");
                self.transition_set_group("MainMenuSinglePlayerTrainingMenuFromDiff", false);
                log::debug!("Reversing difficulty menu - Training");
            }
            ShowSide::USA => {
                self.transition_reverse("MainMenuDifficultyMenuUSBack");
                self.transition_set_group("MainMenuSinglePlayerUSAMenuFromDiff", false);
                log::debug!("Reversing difficulty menu - USA");
            }
            ShowSide::GLA => {
                self.transition_reverse("MainMenuDifficultyMenuGLABack");
                self.transition_set_group("MainMenuSinglePlayerGLAMenuFromDiff", false);
                log::debug!("Reversing difficulty menu - GLA");
            }
            ShowSide::China => {
                self.transition_reverse("MainMenuDifficultyMenuChinaBack");
                self.transition_set_group("MainMenuSinglePlayerChinaMenuFromDiff", false);
                log::debug!("Reversing difficulty menu - China");
            }
            _ => {}
        }
    }

    /// Accept resolution change
    /// Port of AcceptResolution() - C++ lines 704-710
    pub fn accept_resolution(&mut self) {
        let mut state = self.state.write().unwrap();
        state.old_disp_settings = state.new_disp_settings;
        state.disp_changed = false;
        log::info!(
            "Resolution change accepted: {}x{}",
            state.new_disp_settings.x_res,
            state.new_disp_settings.y_res
        );
    }

    /// Decline resolution change
    /// Port of DeclineResolution() - C++ lines 715-752
    pub fn decline_resolution(&mut self) -> MainMenuResult<()> {
        let new_disp_settings = self.rollback_resolution_state();

        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.x_resolution = new_disp_settings.x_res;
            global.y_resolution = new_disp_settings.y_res;
        }
        get_header_template_manager().header_notify_resolution_change();

        let mut prefs = UserPreferences::new();
        let _ = prefs.load("Options.ini");
        prefs.set_string(
            "Resolution",
            format!("{} {}", new_disp_settings.x_res, new_disp_settings.y_res),
        );
        let _ = prefs.write();

        {
            let mut shell = get_shell();
            let _ = shell.reset();
            let _ = shell.show_shell(true);
        }

        TheControlBar::hide_purchase_science();
        TheInGameUI::place_build_available(None, None);
        TheInGameUI::clear_pending_special_power();

        log::info!(
            "Resolution change declined - reverted to: {}x{}",
            new_disp_settings.x_res,
            new_disp_settings.y_res
        );
        Ok(())
    }

    fn rollback_resolution_state(&self) -> DisplaySettings {
        let mut state = self.state.write().unwrap();

        // Revert to old resolution
        // if (TheDisplay->setDisplayMode(...))
        state.disp_changed = false;
        state.new_disp_settings = state.old_disp_settings;
        state.new_disp_settings
    }

    pub fn set_pending_resolution_change(
        &mut self,
        old_settings: DisplaySettings,
        new_settings: DisplaySettings,
    ) {
        let mut state = self.state.write().unwrap();
        state.old_disp_settings = old_settings;
        state.new_disp_settings = new_settings;
        state.disp_changed = true;
    }

    /// Show resolution dialog
    /// Port of DoResolutionDialog() - C++ lines 757-773
    pub fn do_resolution_dialog(&self) {
        let (x_res, y_res) = {
            let state = self.state.read().unwrap();
            (state.new_disp_settings.x_res, state.new_disp_settings.y_res)
        };
        let title = crate::game_text::GameText::fetch("GUI:Resolution");
        let body = format!("{}: {}x{}", title, x_res, y_res);
        let ok: MessageBoxFunc = Box::new(|| {
            get_main_menu().accept_resolution();
        });
        let cancel: MessageBoxFunc = Box::new(|| {
            if let Err(err) = get_main_menu().decline_resolution() {
                log::warn!("DeclineResolution callback failed: {}", err);
            }
        });
        let _ = message_box_ok_cancel(&title, &body, Some(ok), Some(cancel));
    }

    /// Handle canceled download
    /// Port of HandleCanceledDownload() - C++ lines 203-211
    fn handle_canceled_download_state(&self, state: &mut MainMenuState, reset_dropdown: bool) {
        state.button_pushed = false;

        if reset_dropdown {
            self.set_dropdown_hidden(state, DropdownType::Main, false);
            self.transition_set_group("MainMenuDefaultMenuLogoFade", false);
            log::info!("Download canceled - resetting dropdown");
        }
    }

    fn cancel_patch_check_callback_state(state: &mut MainMenuState) {
        state.button_pushed = false;
        state.checking_for_patch_before_gamespy = false;
        state.cant_connect_before_online = false;
        state.checks_left_before_online = 0;
        state.online_cancel_window_open = false;
    }

    fn finish_online_handoff_state(state: &mut MainMenuState) {
        if !state.checking_for_patch_before_gamespy {
            return;
        }

        state.checking_for_patch_before_gamespy = false;
        state.cant_connect_before_online = false;
        state.checks_left_before_online = 0;
        state.online_cancel_window_open = false;
        TheScriptEngine::signal_ui_interact("ShellMainMenuOnlineSelected");
        log::info!("Patch check completed - entering online handoff");
    }

    fn write_input_focus_response(data1: u32, data2: usize) -> bool {
        if data1 == 1 && data2 != 0 {
            // SAFETY: this mirrors the legacy callback contract where mData2 is a Bool*.
            unsafe {
                *(data2 as *mut bool) = true;
            }
        }
        true
    }

    fn http_think_wrapper(&self, state: &mut MainMenuState) {
        if !state.checking_for_patch_before_gamespy {
            return;
        }

        if state.cant_connect_before_online {
            Self::cancel_patch_check_callback_state(state);
            log::info!("Patch check ended because servserv was unreachable");
            return;
        }

        if state.checks_left_before_online > 0 {
            state.checks_left_before_online -= 1;
            if state.checks_left_before_online == 0 {
                Self::finish_online_handoff_state(state);
            }
        } else {
            Self::finish_online_handoff_state(state);
        }
    }

    pub fn handle_canceled_download(&mut self, reset_dropdown: bool) {
        let mut state = self.state.write().unwrap();
        self.handle_canceled_download_state(&mut state, reset_dropdown);
    }
}

fn build_window_ids() -> WindowIds {
    WindowIds {
        main_menu_id: NameKeyGenerator::name_to_key("MainMenu.wnd:MainMenuParent"),
        skirmish_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonSkirmish"),
        online_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonOnline"),
        network_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonNetwork"),
        options_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonOptions"),
        exit_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonExit"),
        motd_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonMOTD"),
        world_builder_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonWorldBuilder"),
        get_update_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonGetUpdate"),
        button_training_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonTRAINING"),
        button_challenge_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonChallenge"),
        button_usa_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonUSA"),
        button_gla_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonGLA"),
        button_china_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonChina"),
        button_usa_recent_save_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonUSARecentSave",
        ),
        button_usa_load_game_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonUSALoadGame"),
        button_gla_recent_save_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonGLARecentSave",
        ),
        button_gla_load_game_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonGLALoadGame"),
        button_china_recent_save_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonChinaRecentSave",
        ),
        button_china_load_game_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonChinaLoadGame",
        ),
        button_single_player_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonSinglePlayer"),
        button_multi_player_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonMultiplayer"),
        button_multi_back_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonMultiBack"),
        button_single_back_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonSingleBack"),
        button_load_replay_back_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonLoadReplayBack",
        ),
        button_replay_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonReplay"),
        button_load_replay_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonLoadReplay"),
        button_load_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonLoadGame"),
        button_credits_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonCredits"),
        button_easy_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonEasy"),
        button_medium_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonMedium"),
        button_hard_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonHard"),
        button_diff_back_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonDiffBack"),
    }
}

impl Default for MainMenu {
    fn default() -> Self {
        Self::new()
    }
}

// Global main menu instance
static MAIN_MENU: OnceLock<Mutex<MainMenu>> = OnceLock::new();

pub fn get_main_menu() -> std::sync::MutexGuard<'static, MainMenu> {
    let lock = MAIN_MENU.get_or_init(|| Mutex::new(MainMenu::new()));
    lock.lock().expect("MainMenu mutex poisoned")
}

// ================================================================================================
// TESTS
// ================================================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_menu_creation() {
        let mut menu = MainMenu::new();
        let state = menu.state.read().unwrap();

        assert!(state.raise_message_boxes);
        assert!(!state.campaign_selected);
        assert!(!state.button_pushed);
        assert!(!state.is_shutting_down);
        assert!(!state.start_game);
        assert_eq!(state.drop_down, DropdownType::None);
        assert_eq!(state.show_side, ShowSide::None);
        assert!(!state.checking_for_patch_before_gamespy);
        assert!(!state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 0);
        assert_eq!(state.time_through_online, 0);
        assert!(!state.online_cancel_window_open);
    }

    #[test]
    fn test_dropdown_type_conversion() {
        assert_eq!(DropdownType::from_i32(0), Some(DropdownType::None));
        assert_eq!(DropdownType::from_i32(1), Some(DropdownType::Single));
        assert_eq!(DropdownType::from_i32(2), Some(DropdownType::Multiplayer));
        assert_eq!(DropdownType::from_i32(3), Some(DropdownType::Main));
        assert_eq!(DropdownType::from_i32(4), Some(DropdownType::LoadReplay));
        assert_eq!(DropdownType::from_i32(5), Some(DropdownType::Difficulty));
        assert_eq!(DropdownType::from_i32(99), None);
    }

    #[test]
    fn test_show_side_conversion() {
        assert_eq!(ShowSide::from_i32(0), Some(ShowSide::None));
        assert_eq!(ShowSide::from_i32(1), Some(ShowSide::Training));
        assert_eq!(ShowSide::from_i32(2), Some(ShowSide::USA));
        assert_eq!(ShowSide::from_i32(3), Some(ShowSide::GLA));
        assert_eq!(ShowSide::from_i32(4), Some(ShowSide::China));
        assert_eq!(ShowSide::from_i32(5), Some(ShowSide::Skirmish));
        assert_eq!(ShowSide::from_i32(99), None);
    }

    #[test]
    fn test_display_settings() {
        let settings = DisplaySettings::default();
        assert_eq!(settings.x_res, 1024);
        assert_eq!(settings.y_res, 768);
        assert_eq!(settings.bit_depth, 32);
        assert!(!settings.windowed);
    }

    #[test]
    fn test_resolution_accept() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap();
            state.new_disp_settings.x_res = 1920;
            state.new_disp_settings.y_res = 1080;
            state.disp_changed = true;
        }

        menu.accept_resolution();

        let state = menu.state.read().unwrap();
        assert_eq!(state.old_disp_settings.x_res, 1920);
        assert_eq!(state.old_disp_settings.y_res, 1080);
        assert!(!state.disp_changed);
    }

    #[test]
    fn test_resolution_rollback_state_restores_old_settings() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap();
            state.old_disp_settings = DisplaySettings {
                x_res: 1280,
                y_res: 720,
                bit_depth: 32,
                windowed: true,
            };
            state.new_disp_settings = DisplaySettings {
                x_res: 1920,
                y_res: 1080,
                bit_depth: 32,
                windowed: false,
            };
            state.disp_changed = true;
        }

        let reverted = menu.rollback_resolution_state();

        assert_eq!(reverted.x_res, 1280);
        assert_eq!(reverted.y_res, 720);

        let state = menu.state.read().unwrap();
        assert_eq!(state.new_disp_settings.x_res, 1280);
        assert_eq!(state.new_disp_settings.y_res, 720);
        assert!(!state.disp_changed);
    }

    #[test]
    fn test_handle_canceled_download() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap();
            state.button_pushed = true;
            state.drop_down = DropdownType::Difficulty;
            state.checking_for_patch_before_gamespy = true;
            state.cant_connect_before_online = true;
            state.checks_left_before_online = 4;
            state.online_cancel_window_open = true;
        }

        menu.handle_canceled_download(true);

        let state = menu.state.read().unwrap();
        assert!(!state.button_pushed);
        assert_eq!(state.drop_down, DropdownType::Difficulty);
        assert!(state.checking_for_patch_before_gamespy);
        assert!(state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 4);
        assert!(state.online_cancel_window_open);
    }

    #[test]
    fn test_cancel_patch_check_callback_clears_patch_state_and_window() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap();
            state.button_pushed = true;
            state.drop_down = DropdownType::Difficulty;
            state.checking_for_patch_before_gamespy = true;
            state.cant_connect_before_online = true;
            state.checks_left_before_online = 4;
            state.online_cancel_window_open = true;
        }

        {
            let mut state = menu.state.write().unwrap();
            MainMenu::cancel_patch_check_callback_state(&mut state);
        }

        let state = menu.state.read().unwrap();
        assert!(!state.button_pushed);
        assert_eq!(state.drop_down, DropdownType::Difficulty);
        assert!(!state.checking_for_patch_before_gamespy);
        assert!(!state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 0);
        assert!(!state.online_cancel_window_open);
    }

    #[test]
    fn test_initial_state() {
        let mut menu = MainMenu::new();
        let state = menu.state.read().unwrap();

        assert_eq!(state.initial_gadget_delay, INITIAL_GADGET_DELAY_DEFAULT);
        assert!(state.not_shown);
        assert!(state.first_time_running_the_game);
        assert!(!state.show_logo);
        assert!(!state.logo_is_shown);
        assert!(!state.checking_for_patch_before_gamespy);
        assert!(!state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 0);
        assert_eq!(state.time_through_online, 0);
        assert!(!state.online_cancel_window_open);
    }

    #[test]
    fn test_start_patch_check_sets_patch_state_without_cancelling_menu() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap();
            state.button_pushed = true;
            state.dont_allow_transitions = true;
        }

        menu.start_patch_check();

        let state = menu.state.read().unwrap();
        assert!(state.button_pushed);
        assert!(state.dont_allow_transitions);
        assert!(state.checking_for_patch_before_gamespy);
        assert!(!state.cant_connect_before_online);
        assert_eq!(state.checks_left_before_online, 4);
        assert_eq!(state.time_through_online, 1);
        assert!(state.online_cancel_window_open);
    }

    #[test]
    fn test_input_focus_writeback_sets_keyboard_focus() {
        let mut focus = false;

        assert!(MainMenu::write_input_focus_response(
            1,
            (&mut focus as *mut bool) as usize
        ));
        assert!(focus);

        focus = false;
        assert!(MainMenu::write_input_focus_response(
            0,
            (&mut focus as *mut bool) as usize
        ));
        assert!(!focus);
    }

    #[test]
    fn test_patch_check_http_think_completes_handoff_after_four_ticks() {
        let mut menu = MainMenu::new();

        {
            let mut state = menu.state.write().unwrap();
            state.checking_for_patch_before_gamespy = true;
            state.checks_left_before_online = 4;
            state.online_cancel_window_open = true;
        }

        {
            let mut state = menu.state.write().unwrap();
            menu.http_think_wrapper(&mut state);
            menu.http_think_wrapper(&mut state);
            menu.http_think_wrapper(&mut state);
            menu.http_think_wrapper(&mut state);
        }

        let state = menu.state.read().unwrap();
        assert!(!state.checking_for_patch_before_gamespy);
        assert_eq!(state.checks_left_before_online, 0);
        assert!(!state.online_cancel_window_open);
    }

    #[test]
    fn test_reveal_hidden_main_menu_sets_cpp_startup_state() {
        let mut menu = MainMenu::new();
        let mut state = MainMenuState::default();

        menu.reveal_hidden_main_menu(&mut state);

        assert_eq!(state.initial_gadget_delay, 1);
        assert_eq!(state.drop_down, DropdownType::Main);
        assert!(!state.not_shown);
    }

    #[test]
    fn test_main_menu_init_does_not_force_hide_startup_controls() {
        game_engine::common::ini::ini_game_data::init_global_data();
        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.initial_file.clear();
            global.shell_map_on = false;
        }

        let previous_first_time =
            FIRST_TIME_RUNNING_GAME.swap(false, std::sync::atomic::Ordering::SeqCst);

        let mut menu = MainMenu::new();
        let (layout, _info) = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/MainMenu.wnd")
                .expect("expected MainMenu.wnd to load")
        });

        let ids = build_window_ids();
        let get_update_id = ids.get_update_id as i32;
        let motd_id = ids.motd_id as i32;
        let map_pack_id = NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonGetMapPack") as i32;

        let capture_hidden = |id: i32| {
            with_window_manager(|manager| {
                manager
                    .get_window_by_id(id)
                    .map(|window| window.borrow().is_hidden())
            })
        };

        let before_get_update = capture_hidden(get_update_id);
        let before_motd = capture_hidden(motd_id);
        let before_map_pack = capture_hidden(map_pack_id);

        menu.init(&*layout.borrow(), None).unwrap();

        let after_get_update = capture_hidden(get_update_id);
        let after_motd = capture_hidden(motd_id);
        let after_map_pack = capture_hidden(map_pack_id);

        assert_eq!(after_get_update, before_get_update);
        assert_eq!(after_motd, before_motd);
        assert_eq!(after_map_pack, before_map_pack);

        FIRST_TIME_RUNNING_GAME.store(previous_first_time, std::sync::atomic::Ordering::SeqCst);
    }

    #[test]
    fn test_input_focus_does_not_reveal_hidden_menu() {
        let mut menu = MainMenu::new();
        {
            let mut state = menu.state.write().unwrap();
            state.window_ids = build_window_ids();
            state.not_shown = true;
        }

        let handled = menu.system(build_window_ids().main_menu_id, GWM_INPUT_FOCUS, 1, 0);
        assert!(handled);

        let state = menu.state.read().unwrap();
        assert!(state.not_shown);
        assert_eq!(state.drop_down, DropdownType::None);
    }

    #[test]
    fn test_mouse_hover_queues_cpp_shell_hooks() {
        let menu = MainMenu::new();
        let mut state = MainMenuState::default();
        state.window_ids.online_id = 11;
        state.window_ids.network_id = 12;
        state.window_ids.options_id = 13;
        state.window_ids.exit_id = 14;

        menu.handle_mouse_entering(&mut state, 11);
        menu.handle_mouse_entering(&mut state, 12);
        menu.handle_mouse_leaving(&mut state, 13);
        menu.handle_mouse_leaving(&mut state, 14);

        let hooks = state
            .pending_actions
            .iter()
            .filter_map(|action| match action {
                PendingMainMenuAction::SignalUiInteract(hook) => Some(*hook),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            hooks,
            vec![
                "ShellMainMenuOnlineHighlighted",
                "ShellMainMenuNetworkHighlighted",
                "ShellMainMenuOptionsUnhighlighted",
                "ShellMainMenuExitUnhighlighted",
            ]
        );
    }

    #[test]
    fn test_mouse_hover_transient_logo_state_matches_cpp() {
        let menu = MainMenu::new();
        let mut state = MainMenuState::default();
        state.window_ids = build_window_ids();
        state.dont_allow_transitions = true;
        state.campaign_selected = false;
        let usa_id = state.window_ids.button_usa_id;

        menu.handle_mouse_entering(&mut state, usa_id);
        assert!(state.show_logo);
        assert_eq!(state.show_side, ShowSide::USA);

        menu.handle_mouse_leaving(&mut state, usa_id);
        assert!(!state.show_logo);
        assert_eq!(state.show_side, ShowSide::None);
    }

    #[test]
    fn test_selected_actions_queue_cpp_selected_hooks() {
        let menu = MainMenu::new();
        let ids = build_window_ids();

        let mut skirmish_state = MainMenuState::default();
        skirmish_state.window_ids = ids.clone();
        menu.handle_button_selected(&mut skirmish_state, ids.skirmish_id)
            .unwrap();
        let skirmish_hooks = skirmish_state
            .pending_actions
            .iter()
            .filter_map(|action| match action {
                PendingMainMenuAction::SignalUiInteract(hook) => Some(*hook),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(skirmish_hooks, vec!["ShellMainMenuSkirmishSelected"]);

        let mut network_state = MainMenuState::default();
        network_state.window_ids = ids.clone();
        menu.handle_button_selected(&mut network_state, ids.network_id)
            .unwrap();
        let network_hooks = network_state
            .pending_actions
            .iter()
            .filter_map(|action| match action {
                PendingMainMenuAction::SignalUiInteract(hook) => Some(*hook),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(network_hooks, vec!["ShellMainMenuNetworkSelected"]);

        let mut options_state = MainMenuState::default();
        options_state.window_ids = ids;
        let options_id = options_state.window_ids.options_id;
        menu.handle_button_selected(&mut options_state, options_id)
            .unwrap();
        let options_hooks = options_state
            .pending_actions
            .iter()
            .filter_map(|action| match action {
                PendingMainMenuAction::SignalUiInteract(hook) => Some(*hook),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(options_hooks, vec!["ShellMainMenuOptionsSelected"]);
    }
}
