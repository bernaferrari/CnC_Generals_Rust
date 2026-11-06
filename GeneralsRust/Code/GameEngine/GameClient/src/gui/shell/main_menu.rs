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

use crate::gui::WindowMsgData;
use crate::gui::callbacks::download_menu::download_menu_update;
use crate::gui::callbacks::message_box::{
    ExtendedMessageBoxFunc, MessageBoxFunc, MessageBoxReturnType as ExMessageBoxReturnType,
    ex_message_box_ok_cancel, message_box_ok, message_box_ok_cancel, quit_message_box_yes_no,
};
use crate::gui::campaign_launch_host_bridge::{
    HostCampaignLaunchDescriptor, MenuHostRequest, MenuLaunchSource,
};
use crate::gui::campaign_manager::GameDifficulty as CampaignDifficulty;
use crate::gui::game_window::WindowEventEffects;
use crate::gui::header_template::get_header_template_manager;
use crate::gui::menu_flags::get_dont_show_main_menu;
use crate::gui::shell::{
    queue_shell_operation, queue_shell_pop, queue_shell_push, queue_shell_reverse_animate_window,
    queue_shell_shutdown_complete, show_shell_map_if_available,
};
use crate::gui::window_manager::WindowLayout as ManagerWindowLayout;
use crate::gui::write_input_focus_response;
use crate::map_util::get_map_cache_manager;
use crate::message_stream::{GameMessage, GameMessageType};
use crate::shell_hooks::{
    SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED, SHELL_SCRIPT_HOOK_MAIN_MENU_NETWORK_SELECTED,
    SHELL_SCRIPT_HOOK_MAIN_MENU_ONLINE_SELECTED, SHELL_SCRIPT_HOOK_MAIN_MENU_OPTIONS_SELECTED,
    SHELL_SCRIPT_HOOK_MAIN_MENU_SKIRMISH_SELECTED, THE_SHELL_HOOK_NAMES,
};
use crate::system::SubsystemInterface;
use game_engine::common::global_data as runtime_global_data;
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::user_preferences::UserPreferences;
use game_network::download_manager::download_manager;
use game_network::gamespy::peer_defs::tear_down_gamespy;
use game_network::gamespy::peer_thread::get_peer_message_queue;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

mod button_events;
mod resolution_and_patches;
mod gadget_simulation;
mod host_inject;
pub(crate) use resolution_and_patches::build_window_ids;
pub use gadget_simulation::*;
pub use host_inject::*;
#[cfg(feature = "online_ui")]
#[path = "main_menu_online.rs"]
mod main_menu_online;

#[cfg(not(feature = "online_ui"))]
mod main_menu_online {
    use super::MainMenuState;

    pub enum OnlineHandoff {
        Unavailable,
    }

    pub fn start_online(state: &mut MainMenuState) -> OnlineHandoff {
        state.checking_for_patch_before_gamespy = false;
        state.checks_left_before_online = 0;
        state.online_cancel_window_open = false;
        state.button_pushed = false;
        OnlineHandoff::Unavailable
    }
}
use thiserror::Error;

#[cfg(feature = "online_ui")]
fn raise_gamespy_message_boxes() {
    crate::gamespy_overlay::raise_gs_message_box();
}

#[cfg(not(feature = "online_ui"))]
fn raise_gamespy_message_boxes() {}

#[cfg(feature = "online_ui")]
fn update_gamespy_overlays() {
    crate::gamespy_overlay::update_overlays();
}

#[cfg(not(feature = "online_ui"))]
fn update_gamespy_overlays() {}

fn http_startup() {}

fn http_cleanup() {}

fn stop_async_dns_check() {}

fn set_main_menu_cursor_visibility(visible: bool, effects: &mut WindowEventEffects<'_>) {
    effects.request_menu_host(MenuHostRequest::SetCursorVisibility(visible));
}

// ================================================================================================
// CONSTANTS
// ================================================================================================

/// Time constants - match C++ MainMenu.cpp
const SHOW_FRAMES_LIMIT: i32 = 20;
const INITIAL_GADGET_DELAY_DEFAULT: i32 = 210;
const CORNER: i32 = 10;

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

#[derive(Debug)]
pub(crate) enum PendingMainMenuAction {
    PushShellScreen(&'static str),
    PushCreditsThenRevealMain,
    ReverseTransitionGroup(&'static str),
    ShowOptionsLayout,
    SignalUiInteract(&'static str),
    ReverseAnimateWindow,
    StartPatchCheck,
    StartDownloadingPatches,
    LaunchWorldBuilder,
    QuitRequest,
    QuitNow,
    Focus(i32),
    CampaignCdDialog(GameDifficulty),
    ResolutionDialog(i32, i32),
    ShutdownComplete,
    DrainShellPushes,
    HostRequest(MenuHostRequest),
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
#[derive(Debug, Clone, Default)]
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
    residual_campaign_difficulty: Option<GameDifficulty>,

    // State flags - match C++ lines 78-190
    pub raise_message_boxes: bool,
    pub campaign_selected: bool,
    pub button_pushed: bool,
    pub is_shutting_down: bool,
    pub start_game: bool,
    /// The host is starting a match; this affects shutdown, not campaign launch.
    host_match_starting: bool,
    pub show_fade: bool,
    pub not_shown: bool,
    pub first_time_running_the_game: bool,
    pub show_logo: bool,
    pub logo_is_shown: bool,
    pub just_entered: bool,
    /// C++ MainMenuInit 525-530 hide pack applied. Latched only after
    /// MapBorder2 was found on the live WindowManager and hidden.
    pub init_dropdowns_hidden: bool,
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
            residual_campaign_difficulty: None,
            raise_message_boxes: true,
            campaign_selected: false,
            button_pushed: false,
            is_shutting_down: false,
            start_game: false,
            host_match_starting: false,
            show_fade: false,
            not_shown: true,
            first_time_running_the_game: true,
            show_logo: false,
            logo_is_shown: false,
            just_entered: false,
            init_dropdowns_hidden: false,
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
    state: MainMenuState,
    first_time_running_game: bool,
}

#[cfg(not(feature = "network"))]
fn network_compat<'a>(
    effects: &crate::gui::game_window::WindowEventEffects<'a>,
) -> &'a crate::game_network_compat::NetworkCompatState {
    effects
        .window_manager_owner
        .expect("main menu network stub requires WindowManagerOwner")
        .network_compat()
}


impl MainMenu {
    fn hide_receiving_window_with_lent(
        manager: &mut crate::gui::window_manager::WindowManager,
        parent_id: Option<u32>,
        id: Option<u32>,
        name: Option<&str>,
        hide: bool,
        selected: Option<&mut crate::gui::game_window::GameWindow>,
        lent: &mut [&mut crate::gui::game_window::GameWindow],
    ) {
        let lent_read: Vec<&crate::gui::game_window::GameWindow> =
            lent.iter().map(|window| &**window).collect();
        let selected_read = selected.as_deref();
        let scoped = parent_id
            .and_then(|parent| {
                manager.win_get_window_from_id_with_lent(
                    None,
                    parent as i32,
                    selected_read,
                    &lent_read,
                )
            })
            .and_then(|parent| {
                id.and_then(|id| {
                    manager.win_get_window_from_id_with_lent(
                        Some(&parent),
                        id as i32,
                        selected_read,
                        &lent_read,
                    )
                })
            });
        let window = scoped
            .or_else(|| {
                id.and_then(|id| {
                    manager.win_get_window_from_id_with_lent(
                        None,
                        id as i32,
                        selected_read,
                        &lent_read,
                    )
                })
            })
            .or_else(|| {
                name.and_then(|name| {
                    manager.find_window_by_name_with_lent(name, selected_read, &lent_read)
                })
            });
        let Some(window) = window else {
            return;
        };
        let mut lent_for_hide: Vec<&mut crate::gui::game_window::GameWindow> =
            lent.iter_mut().map(|window| &mut **window).collect();
        let _ = manager.hide_window_with_lent(&window, hide, selected, &mut lent_for_hide);
    }

    fn hide_window_by_id(
        state: &MainMenuState,
        id: u32,
        fallback_name: Option<&str>,
        hide: bool,
        effects: &mut WindowEventEffects<'_>,
        selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        effects.with_manager(|manager, effects| {
            Self::hide_receiving_window_with_lent(
                manager,
                Some(state.window_ids.main_menu_id),
                Some(id),
                fallback_name,
                hide,
                selected,
                &mut effects.lent_windows,
            );
        });
    }

    fn hide_window_by_name(
        state: &MainMenuState,
        name: &str,
        hide: bool,
        effects: &mut WindowEventEffects<'_>,
        selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        effects.with_manager(|manager, effects| {
            Self::hide_receiving_window_with_lent(
                manager,
                Some(state.window_ids.main_menu_id),
                Some(NameKeyGenerator::name_to_key(name)),
                Some(name),
                hide,
                selected,
                &mut effects.lent_windows,
            );
        });
    }

    fn dropdown_wnd_name(dropdown: DropdownType) -> Option<&'static str> {
        match dropdown {
            DropdownType::Single => Some("MainMenu.wnd:MapBorder"),
            DropdownType::Multiplayer => Some("MainMenu.wnd:MapBorder1"),
            DropdownType::Main => Some("MainMenu.wnd:MapBorder2"),
            DropdownType::LoadReplay => Some("MainMenu.wnd:MapBorder3"),
            DropdownType::Difficulty => Some("MainMenu.wnd:MapBorder4"),
            _ => None,
        }
    }

    /// C++ MainMenuInit 525-530 hides dropDownWindows[1..]. Index 1 is
    /// DROPDOWN_SINGLE, so all five dropdowns start hidden. It also runs
    /// initialHide + SHOW_NONE + MainMenuRuler (627-629).
    const CPP_INIT_HIDE_PACK_NAMES: &'static [&'static str] = &[
        "MainMenu.wnd:MapBorder",
        "MainMenu.wnd:MapBorder1",
        "MainMenu.wnd:MapBorder2",
        "MainMenu.wnd:MapBorder3",
        "MainMenu.wnd:MapBorder4",
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
        "MainMenu.wnd:WinFactionUSSmall",
        "MainMenu.wnd:WinFactionUSMedium",
        "MainMenu.wnd:WinFactionGLASmall",
        "MainMenu.wnd:WinFactionGLAMedium",
        "MainMenu.wnd:WinFactionChinaSmall",
        "MainMenu.wnd:WinFactionChinaMedium",
        "MainMenu.wnd:ButtonUSARecentSave",
        "MainMenu.wnd:ButtonUSALoadGame",
        "MainMenu.wnd:ButtonGLARecentSave",
        "MainMenu.wnd:ButtonGLALoadGame",
        "MainMenu.wnd:ButtonChinaRecentSave",
        "MainMenu.wnd:ButtonChinaLoadGame",
        "MainMenu.wnd:MainMenuRuler",
    ];

    /// Initial layout visibility is applied before the next init operation.
    fn hide_named_windows_live_or_deferred(names: &[&str], effects: &mut WindowEventEffects<'_>) {
        effects.with_manager(|manager, effects| {
            for name in names {
                Self::hide_receiving_window_with_lent(
                    manager,
                    None,
                    None,
                    Some(name),
                    true,
                    None,
                    &mut effects.lent_windows,
                );
            }
        });
    }

    /// True only when the live WM has all five C++ dropdowns hidden.
    fn all_dropdowns_hidden_on_live_wm(effects: &mut WindowEventEffects<'_>) -> bool {
        effects.with_manager(|manager, effects| {
            [
                "MainMenu.wnd:MapBorder",
                "MainMenu.wnd:MapBorder1",
                "MainMenu.wnd:MapBorder2",
                "MainMenu.wnd:MapBorder3",
                "MainMenu.wnd:MapBorder4",
            ]
            .iter()
            .all(|name| {
                manager
                    .find_window_by_name(name)
                    .is_some_and(|window| window.borrow().is_hidden())
            })
        })
    }

    /// Apply C++ MainMenuInit hide pack. Returns true only if all dropdowns
    /// were found on the live WindowManager and hidden. Never calls
    /// `transition_set_group`.
    fn apply_cpp_init_hide_pack(effects: &mut WindowEventEffects<'_>) -> bool {
        Self::hide_named_windows_live_or_deferred(Self::CPP_INIT_HIDE_PACK_NAMES, effects);
        Self::all_dropdowns_hidden_on_live_wm(effects)
    }

    fn set_dropdown_hidden(
        _state: &MainMenuState,
        dropdown: DropdownType,
        hide: bool,
        effects: &mut WindowEventEffects<'_>,
        selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        let Some(name) = Self::dropdown_wnd_name(dropdown) else {
            return;
        };
        effects.with_manager(|manager, effects| {
            Self::hide_receiving_window_with_lent(
                manager,
                None,
                None,
                Some(name),
                hide,
                selected,
                &mut effects.lent_windows,
            );
        });
    }

    /// C++ changes only DROPDOWN_MAIN here, at this exact callback point.
    fn win_hide_dropdown_main_false(
        effects: &mut WindowEventEffects<'_>,
        selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        effects.with_manager(|manager, effects| {
            Self::hide_receiving_window_with_lent(
                manager,
                None,
                None,
                Some("MainMenu.wnd:MapBorder2"),
                false,
                selected,
                &mut effects.lent_windows,
            );
        });
    }

    fn reveal_hidden_main_menu(
        state: &mut MainMenuState,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        state.initial_gadget_delay = 1;
        state.drop_down = DropdownType::Main;
        // C++ MainMenuInput GWM_CHAR: dropDownWindows[DROPDOWN_MAIN]->winHide(FALSE)
        // only. Other borders were already hidden by MainMenuInit. Do not
        // re-hide them here (show_only_dropdown + transition_set_group stalls).
        Self::win_hide_dropdown_main_false(effects, selected.as_deref_mut());
        Self::transition_set_group("MainMenuFade", true, effects, selected.as_deref_mut());
        Self::transition_set_group(
            "MainMenuDefaultMenu",
            false,
            effects,
            selected.as_deref_mut(),
        );
        set_main_menu_cursor_visibility(true, effects);
        state.not_shown = false;
    }

    fn queue_signal_ui_interact(state: &mut MainMenuState, hook: &'static str) {
        Self::queue_action(state, PendingMainMenuAction::SignalUiInteract(hook));
    }

    fn sync_cpp_startup_visibility(state: &MainMenuState) {
        // The C++ init block that hid these controls is commented out, so keep the
        // runtime visibility untouched here as well.
        let _ = state;
    }

    pub fn complete_source_campaign_shutdown(&mut self) {
        self.state.is_shutting_down = false;
    }
    pub fn campaign_start_latched(&self, side: ShowSide) -> bool {
        let state = &self.state;
        state.campaign_selected
            && state.show_side == side
            && state.start_game
            && !state.launch_challenge_menu
    }
    pub fn challenge_latched(&self) -> bool {
        self.state.campaign_selected && self.state.launch_challenge_menu
    }
    pub fn skirmish_latched(&self) -> bool {
        self.state.button_pushed
            && self.state.campaign_selected
            && !self.state.launch_challenge_menu
    }
    pub fn residual_last_campaign_difficulty(&self) -> Option<GameDifficulty> {
        self.state.residual_campaign_difficulty
    }

    pub fn complete_source_campaign_start(&mut self) {
        self.state.start_game = false;
        self.state.is_shutting_down = true;
    }

    pub(crate) fn take_pending_actions(&mut self) -> Vec<PendingMainMenuAction> {
        std::mem::take(&mut self.state.pending_actions)
    }

    /// Create a new MainMenu instance
    pub fn new() -> Self {
        Self {
            state: MainMenuState::default(),
            first_time_running_game: true,
        }
    }

    /// Initialize the main menu
    /// Port of MainMenuInit() - C++ lines 429-653
    pub fn init(
        &mut self,
        layout: &dyn std::any::Any,
        user_data: Option<&dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> MainMenuResult<()> {
        log::info!("MainMenuInit: Initializing main menu");

        let mut state = &mut self.state;
        log::info!("MainMenuInit: receiving state borrowed");

        if let Some(global) = get_global_data() {
            global.write().break_the_movie = false;
        }
        log::info!("MainMenuInit: global data updated");

        // C++ MainMenuInit calls TheShell->showShellMap while TheShell may already
        // be on the stack for push/do_push. Use the re-entrancy-safe helper so we
        // never panic on RefCell double-borrow during smoke-test menu entry.
        crate::gui::shell::base::show_shell_map_if_available(true, effects);
        log::info!("MainMenuInit: show_shell_map_if_available done");
        set_main_menu_cursor_visibility(true, effects);
        log::info!("MainMenuInit: cursor visibility set");

        // Reset state - matches C++ lines 431-442
        state.button_pushed = false;
        state.is_shutting_down = false;
        state.start_game = false;
        state.host_match_starting = false;
        state.show_logo = false;
        state.logo_is_shown = false;
        state.launch_challenge_menu = false;
        state.dont_allow_transitions = false;
        state.first_time_running_the_game =
            std::mem::replace(&mut self.first_time_running_game, false);
        state.drop_down = DropdownType::None;
        state.pending_drop_down = DropdownType::None;
        state.show_frames = 0;
        state.show_side = ShowSide::None;
        state.mouse_anchor_initialized = false;
        state.last_mouse_pos = (0, 0);
        state.pending_actions.clear();
        log::info!("MainMenuInit: state fields reset");

        // Initialize dropdown windows array - matches C++ lines 441-442
        for i in 0..(DropdownType::Count as usize) {
            if let Some(dropdown_type) = DropdownType::from_i32(i as i32) {
                state.dropdown_windows.insert(dropdown_type, None);
            }
        }
        // Initialize window IDs - matches C++ lines 444-480
        state.window_ids = build_window_ids();
        log::info!("MainMenuInit: window ids built");

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
        log::info!("MainMenuInit: hide dropdowns then show layout");
        // C++ MainMenuInit 525-530 + 627-629 (ruler), then 579 layout->hide(FALSE).
        // Do not latch success from dummy with_window_manager_ref lookups.
        // C++ info.windows is roots only; hide(false) must not clear authored
        // HIDDEN on Clock / GetUpdate / StaticTextSelectDifficulty.
        let hide_ok = Self::apply_cpp_init_hide_pack(effects);
        if let Some(layout) = layout.downcast_ref::<ManagerWindowLayout>() {
            layout.hide_receiving(false, effects);
        }
        state.init_dropdowns_hidden = if hide_ok {
            Self::apply_cpp_init_hide_pack(effects)
        } else {
            false
        };
        state.drop_down = DropdownType::None;
        state.just_entered = true;
        state.initial_gadget_delay = 2;
        log::info!("MainMenuInit: layout shown and dropdown hide attempted");

        // Map-cache rebuild off the main thread (was a multi-minute stall).
        // CommonServices is TLS — re-bind the parent's publish on the worker or
        // update_cache panics in get_map_cache / global_data_arc.
        let map_cache_common =
            game_engine::common::common_services::try_common_services();
        std::thread::spawn(move || {
            let _common_tls = map_cache_common.map(
                game_engine::common::common_services::CommonServicesScope::from_services,
            );
            if let Ok(mut map_cache) = get_map_cache_manager().lock() {
                map_cache.update_cache();
            }
        });

        if get_peer_message_queue(network_compat(effects))
            .and_then(|queue| queue.lock().ok().map(|queue| !queue.is_connected()))
            .unwrap_or(false)
        {
            tear_down_gamespy();
        }

        state.raise_message_boxes = true;
        // Campaign not selected - matches C++ line 630
        state.campaign_selected = false;

        // Skip hide_window_by_name / focus_window during push (same stall class).
        // Handle first time running - matches C++ lines 632-646
        if state.first_time_running_the_game {
            log::debug!("First time running the game - hiding mouse and fading");
            state.not_shown = true;
            set_main_menu_cursor_visibility(false, effects);
            // Defer reverse fade to first update; transition_reverse can re-enter WM.
        } else {
            state.show_fade = true;
            // Match C++ MainMenuUpdate startup cadence: set justEntered and tick down to 1
            // before applying the default logo fade transition group.
            state.just_entered = true;
            state.initial_gadget_delay = 2;
        }

        log::info!("MainMenuInit: Initialization complete");
        Ok(())
    }

    /// Shutdown the main menu
    /// Port of MainMenuShutdown() - C++ lines 658-690
    pub fn shutdown(
        &mut self,
        layout: &dyn std::any::Any,
        user_data: Option<&dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> MainMenuResult<()> {
        log::info!("MainMenuShutdown: Shutting down main menu");

        let mut state = &mut self.state;

        if !state.start_game && !state.host_match_starting {
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
            Self::finish_shutdown_complete(Some(layout), &mut state, effects)?;
            log::info!("Main menu shutdown complete");
            return Ok(());
        }

        if !state.start_game && !state.host_match_starting {
            // TheShell->reverseAnimatewindow() - matches C++ line 686.  This
            // callback runs inside Shell::push, so queue it at that lifecycle
            // boundary rather than taking a second mutable shell borrow.
            log::debug!("Reversing window animation");
            Self::queue_action(&mut state, PendingMainMenuAction::ReverseAnimateWindow);
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
        effects: &mut WindowEventEffects<'_>,
    ) -> MainMenuResult<()> {
        let mut state = &mut self.state;
        let mut focus_target = None;
        let mut pending_actions = Vec::new();
        if effects.menu_world().in_game && !effects.menu_world().in_shell_game {
            return Ok(());
        }

        if get_dont_show_main_menu(effects) && state.just_entered {
            state.just_entered = false;
        }

        if let Ok(mut manager_guard) = download_manager(network_compat(effects)).lock() {
            let should_update = manager_guard
                .as_ref()
                .map(|manager| !manager.is_done())
                .unwrap_or(false);
            if should_update {
                if let Some(manager) = manager_guard.as_mut() {
                    let _ = manager.update();
                }
                if let Some(layout) = layout.downcast_ref::<ManagerWindowLayout>() {
                    effects.with_manager(|manager, effects| {
                        download_menu_update(layout, None, manager, effects)
                    });
                }
            }
        }

        if state.just_entered || !state.init_dropdowns_hidden {
            // C++ hides only in MainMenuInit. Retry only while first-run
            // (notShown) and no dropdown nav yet. After CHAR or SOLO PLAY,
            // re-running the pack hides MapBorder and undoes 1313-1324.
            if !state.init_dropdowns_hidden
                && state.not_shown
                && state.drop_down == DropdownType::None
            {
                state.init_dropdowns_hidden = Self::apply_cpp_init_hide_pack(effects);
            }
        }
        if state.just_entered {
            if state.initial_gadget_delay == 1 {
                Self::transition_set_group("MainMenuDefaultMenuLogoFade", false, effects, None);
                focus_target = Some(state.window_ids.main_menu_id as i32);
                state.initial_gadget_delay = 2;
                state.just_entered = false;
                log::debug!("Just entered - setting up main menu transitions");
            } else {
                state.initial_gadget_delay -= 1;
            }
        }

        // C++ waits for a >20px mouse move or GWM_CHAR to reveal the first-run
        // menu (MainMenu.cpp:982-1005), because a human is watching. The
        // runtime-host windowed smoke run has no physical input, so the menu
        // would stay hidden forever. Run the exact C++ reveal sequence once,
        // gated to that mode so interactive sessions keep C++ behavior.
        if state.not_shown
            && state.just_entered == false
            && std::env::var("GENERALS_RUNTIME_HOST_WND").as_deref() == Ok("1")
        {
            log::info!("MainMenuUpdate: unattended runtime-host run — revealing main menu");
            Self::reveal_hidden_main_menu(&mut state, effects, None);
        }

        // Handle transitions - matches C++ lines 847-848
        if state.dont_allow_transitions && Self::transitions_finished(effects) {
            state.dont_allow_transitions = false;
        }

        // Show logo logic - matches C++ lines 850-878
        if state.show_logo && !state.dont_allow_transitions {
            match state.show_side {
                ShowSide::Training => {
                    Self::transition_set_group("MainMenuFactionTraining", false, effects, None);
                    log::debug!("Showing training faction logo");
                }
                ShowSide::China => {
                    Self::transition_set_group("MainMenuFactionChina", false, effects, None);
                    log::debug!("Showing China faction logo");
                }
                ShowSide::GLA => {
                    Self::transition_set_group("MainMenuFactionGLA", false, effects, None);
                    log::debug!("Showing GLA faction logo");
                }
                ShowSide::USA => {
                    Self::transition_set_group("MainMenuFactionUS", false, effects, None);
                    log::debug!("Showing USA faction logo");
                }
                ShowSide::Skirmish => {
                    Self::transition_set_group("MainMenuFactionSkirmish", false, effects, None);
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
        Self::http_think_wrapper(&mut state);
        update_gamespy_overlays();

        // Check if we should start the game - matches C++ lines 933-936
        if state.start_game
            && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
            && Self::transitions_finished(effects)
        {
            // Prefix UI actions precede the source clear. The post-clear branch
            // resumes from the current request's weak layout receipt on its host.
            if let Some(id) = focus_target.take() {
                effects
                    .pending_main_menu_actions
                    .push((PendingMainMenuAction::Focus(id), None));
            }
            pending_actions.append(&mut state.pending_actions);
            Self::do_game_start(&mut state, layout, effects)?;
            pending_actions.append(&mut state.pending_actions);
            Self::execute_pending_actions(pending_actions, effects);
            return Ok(());
        }

        // Check if shutdown is complete - matches C++ lines 939-942
        if state.is_shutting_down
            && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
            && Self::transitions_finished(effects)
        {
            Self::finish_shutdown_complete(Some(layout), &mut state, effects)?;
            Self::queue_action(&mut state, PendingMainMenuAction::ShutdownComplete);
            log::info!("Main menu shutdown complete");
            return Ok(());
        }

        pending_actions.append(&mut state.pending_actions);
        if let Some(id) = focus_target {
            effects
                .pending_main_menu_actions
                .push((PendingMainMenuAction::Focus(id), None));
        }
        Self::execute_pending_actions(pending_actions, effects);
        // C++ TheShell->push is synchronous from GBM_SELECTED. We queue
        // during process_mouse_event; drain here so the next Shell::update
        // boundary applies Options/Load/Credits/Skirmish/Challenge.
        effects
            .pending_main_menu_actions
            .push((PendingMainMenuAction::DrainShellPushes, None));

        Ok(())
    }

    /// Handle input messages
    /// Port of MainMenuInput() - C++ lines 957-1016
    pub fn input(
        &mut self,
        window: u32,
        msg: u32,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        self.input_with_selected(window, msg, data1, data2, effects, None)
    }
    pub(crate) fn input_for_window(
        &mut self,
        window: &mut crate::gui::game_window::GameWindow,
        msg: u32,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        let id = window.get_id() as u32;
        self.input_with_selected(id, msg, data1, data2, effects, Some(window))
    }
    fn input_with_selected(
        &mut self,
        window: u32,
        msg: u32,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) -> bool {
        let mut state = &mut self.state;

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
                        Self::reveal_hidden_main_menu(&mut state, effects, selected.as_deref_mut());
                        return true; // MSG_HANDLED
                    }
                }
            }

            // GWM_CHAR - matches C++ lines 996-1009
            GWM_CHAR if state.not_shown => {
                Self::reveal_hidden_main_menu(&mut state, effects, selected.as_deref_mut());
                return true; // MSG_HANDLED
            }

            _ => {}
        }

        false // MSG_IGNORED
    }

    /// Handle system messages
    /// Port of MainMenuSystem() - C++ lines 1021-1688
    pub fn system(
        &mut self,
        window: u32,
        msg: u32,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        self.system_with_camera_input(window, msg, data1, data2, None, effects, None)
    }
    pub(crate) fn system_for_window(
        &mut self,
        window: &mut crate::gui::game_window::GameWindow,
        msg: u32,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        let id = window.get_id() as u32;
        self.system_with_camera_input(id, msg, data1, data2, None, effects, Some(window))
    }
    pub(crate) fn select_options_with_camera_input(
        &mut self,
        window: u32,
        observation: crate::gui::callbacks::OptionsCameraInputObservation,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        self.system_with_camera_input(
            window,
            GBM_SELECTED,
            build_window_ids().options_id as WindowMsgData,
            0,
            Some(observation),
            effects,
            None,
        )
    }
    fn system_with_camera_input(
        &mut self,
        window: u32,
        msg: u32,
        data1: WindowMsgData,
        data2: WindowMsgData,
        camera_input: Option<crate::gui::callbacks::OptionsCameraInputObservation>,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) -> bool {
        let mut state = &mut self.state;
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
            // data2 is a WindowMsgPayload token (from set_focus push_payload), NOT a
            // raw *mut bool. Dereferencing data2 as a pointer caused Menu SIGSEGV
            // (EXC_BAD_ACCESS / PAC failure at 0x8000000001000000) during WM update.
            GWM_INPUT_FOCUS => {
                if window != build_window_ids().main_menu_id {
                    return false;
                }
                // Accept focus when gaining (data1 != 0); write via payload arena.
                if data1 != 0 {
                    let _ = crate::gui::game_window::replace_payload(
                        effects,
                        data2,
                        crate::gui::WindowMsgPayload::Bool(true),
                    );
                }
                return true;
            }

            // GBM_MOUSE_ENTERING - matches C++ lines 1060-1176
            GBM_MOUSE_ENTERING => {
                let control_id = data1 as u32;
                Self::handle_mouse_entering(
                    &mut state,
                    control_id,
                    effects,
                    selected.as_deref_mut(),
                );
                pending_actions.append(&mut state.pending_actions);
                Self::execute_pending_actions_with_camera_input(
                    pending_actions,
                    camera_input,
                    effects,
                );
                return true;
            }

            // GBM_MOUSE_LEAVING - matches C++ lines 1178-1277
            GBM_MOUSE_LEAVING => {
                let control_id = data1 as u32;
                Self::handle_mouse_leaving(
                    &mut state,
                    control_id,
                    effects,
                    selected.as_deref_mut(),
                );
                pending_actions.append(&mut state.pending_actions);
                Self::execute_pending_actions_with_camera_input(
                    pending_actions,
                    camera_input,
                    effects,
                );
                return true;
            }

            // GBM_SELECTED - matches C++ lines 1279-1677
            GBM_SELECTED => {
                let control_id = data1 as u32;
                log::info!(
                    "MainMenu GBM_SELECTED control_id={control_id} solo={} skirmish={} dont_allow={} button_pushed={}",
                    state.window_ids.button_single_player_id,
                    state.window_ids.skirmish_id,
                    state.dont_allow_transitions,
                    state.button_pushed
                );
                if state.button_pushed {
                    return true;
                }

                if let Err(e) = Self::handle_button_selected(
                    &mut state,
                    control_id,
                    effects,
                    selected.as_deref_mut(),
                ) {
                    log::error!("Error handling button selection: {}", e);
                }
                pending_actions.append(&mut state.pending_actions);
                Self::execute_pending_actions_with_camera_input(
                    pending_actions,
                    camera_input,
                    effects,
                );
                return true;
            }

            _ => {}
        }

        false // MSG_IGNORED
    }

    // ============================================================================================
    // PRIVATE HELPER METHODS
    // ============================================================================================

    fn transition_set_group(
        group: &str,
        immediate: bool,
        effects: &mut WindowEventEffects<'_>,
        selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        // When the receiving manager is already loaned (gadget callback under
        // with_manager), queue onto the same owner so complete_event_callbacks
        // drains the transition after the nested borrow ends.
        if effects.manager.is_none() {
            let owner = effects
                .window_manager_owner
                .expect("WND operation requires receiving manager");
            if owner.try_with_manager(|_| ()).is_none() {
                let group = group.to_string();
                effects.queue_window_operation(move |manager, _effects| {
                    manager.transition_set_group(&group, immediate, None);
                });
                return;
            }
        }
        effects.with_manager(|manager, effects| {
            let mut lent: Vec<_> = effects
                .lent_windows
                .iter_mut()
                .map(|window| &mut **window)
                .collect();
            manager.transition_set_group_with_lent(group, immediate, selected, &mut lent)
        });
    }

    fn queue_action(state: &mut MainMenuState, action: PendingMainMenuAction) {
        state.pending_actions.push(action);
    }

    fn execute_pending_actions(
        actions: Vec<PendingMainMenuAction>,
        effects: &mut WindowEventEffects<'_>,
    ) {
        Self::execute_pending_actions_with_camera_input(actions, None, effects);
    }
    fn execute_pending_actions_with_camera_input(
        actions: Vec<PendingMainMenuAction>,
        camera_input: Option<crate::gui::callbacks::OptionsCameraInputObservation>,
        effects: &mut WindowEventEffects<'_>,
    ) {
        effects
            .pending_main_menu_actions
            .extend(actions.into_iter().map(|action| (action, camera_input)));
    }

    pub(crate) fn execute_action(
        action: PendingMainMenuAction,
        camera_input: Option<crate::gui::callbacks::OptionsCameraInputObservation>,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        match action {
            PendingMainMenuAction::PushShellScreen(screen) => {
                effects
                    .window_manager_owner
                    .expect("menu requires receiving window owner")
                    .queue_shell_operation_with_effects(move |shell, effects| {
                        let _ = shell.push(screen, false, effects);
                    });
            }
            PendingMainMenuAction::PushCreditsThenRevealMain => {
                effects
                    .window_manager_owner
                    .expect("menu requires receiving window owner")
                    .queue_shell_operation_with_effects(|shell, effects| {
                        // Shell::push can run the current layout's shutdown
                        // callback. Complete that call before changing the
                        // main dropdown and its transition, as in C++.
                        let _ = shell.push("Menus/CreditsMenu.wnd", false, effects);
                        Self::win_hide_dropdown_main_false(effects, None);
                        Self::transition_reverse("MainMenuDefaultMenu", effects, None);
                    });
            }
            PendingMainMenuAction::ReverseTransitionGroup(group) => {
                Self::transition_reverse(group, effects, selected.as_deref_mut())
            }
            PendingMainMenuAction::ShowOptionsLayout => {
                effects
                    .window_manager_owner
                    .expect("menu requires receiving window owner")
                    .queue_shell_operation_with_effects(move |shell, effects| {
                        if let Some(layout) = shell.get_options_layout(true) {
                            if let Err(err) = layout.run_init_with_effects(
                                camera_input
                                    .as_ref()
                                    .map(|value| value as &dyn std::any::Any),
                                effects,
                            ) {
                                log::warn!("Options layout init failed: {err}");
                            }
                            layout.hide_with_effects(false, effects);
                            layout.bring_forward_with_effects(effects);
                        }
                    });
            }
            PendingMainMenuAction::SignalUiInteract(hook) => {
                effects.request_menu_host(MenuHostRequest::SignalUiInteract(hook))
            }
            PendingMainMenuAction::ReverseAnimateWindow => {
                queue_shell_reverse_animate_window(effects)
            }
            PendingMainMenuAction::StartPatchCheck => {
                effects.with_main_menu(|menu, _| menu.start_patch_check())
            }
            PendingMainMenuAction::StartDownloadingPatches => {
                Self::start_downloading_patches(effects)
            }
            PendingMainMenuAction::LaunchWorldBuilder => Self::launch_world_builder(effects),
            PendingMainMenuAction::QuitRequest => Self::perform_quit_request(effects),
            PendingMainMenuAction::QuitNow => Self::perform_quit_now(effects),
            PendingMainMenuAction::Focus(id) => Self::focus_window(id, effects),
            PendingMainMenuAction::CampaignCdDialog(diff) => {
                Self::show_campaign_cd_dialog(diff, effects)
            }
            PendingMainMenuAction::ResolutionDialog(x, y) => {
                Self::show_resolution_dialog(x, y, effects)
            }
            PendingMainMenuAction::ShutdownComplete => {
                let _ = Self::complete_shell_shutdown(effects);
            }
            PendingMainMenuAction::DrainShellPushes => drain_deferred_shell_pushes(effects),
            PendingMainMenuAction::HostRequest(request) => effects.request_menu_host(request),
        }
    }

    fn perform_quit_request(effects: &mut WindowEventEffects<'_>) {
        let windowed = get_global_data()
            .map(|data| data.read().windowed)
            .unwrap_or(true);
        if windowed {
            effects.with_main_menu(|menu, effects| menu.confirm_quit(effects));
            return;
        }

        let yes: MessageBoxFunc = Box::new(|effects| {
            effects.with_main_menu(|menu, effects| menu.confirm_quit(effects));
        });
        let _ = effects.with_manager(|manager, effects| {
            quit_message_box_yes_no(
                &crate::game_text::GameText::fetch("GUI:QuitPopupTitle"),
                &crate::game_text::GameText::fetch("GUI:QuitPopupMessage"),
                Some(yes),
                None,
                manager,
                effects,
            )
        });
    }

    fn confirm_quit(&mut self, effects: &mut WindowEventEffects<'_>) {
        let mut state = &mut self.state;
        state.button_pushed = true;

        effects
            .pending_main_menu_actions
            .push((PendingMainMenuAction::QuitNow, None));
    }

    fn perform_quit_now(effects: &mut WindowEventEffects<'_>) {
        effects.request_menu_host(MenuHostRequest::SignalUiInteract(
            THE_SHELL_HOOK_NAMES[SHELL_SCRIPT_HOOK_MAIN_MENU_EXIT_SELECTED as usize],
        ));
        queue_shell_pop(effects);
        effects.request_menu_host(MenuHostRequest::Quit);
        if effects.menu_world().in_game {
            effects
                .emitted_messages
                .push(GameMessage::new(GameMessageType::ClearGameData));
        }
    }

    fn start_patch_check(&mut self) {
        let mut state = &mut self.state;
        state.checking_for_patch_before_gamespy = true;
        state.cant_connect_before_online = false;
        state.checks_left_before_online = 4;
        state.time_through_online = state.time_through_online.wrapping_add(1);
        state.online_cancel_window_open = true;

        log::info!("StartPatchCheck requested");
    }

    fn start_downloading_patches(effects: &mut WindowEventEffects<'_>) {
        let has_queued_downloads = download_manager(network_compat(effects))
            .lock()
            .ok()
            .and_then(|guard| {
                guard
                    .as_ref()
                    .map(|manager| manager.is_active() || manager.is_file_queued_for_download())
            })
            .unwrap_or(false);

        if !has_queued_downloads {
            effects
                .with_main_menu(|menu, effects| menu.handle_canceled_download(true, effects, None));
            log::info!("StartDownloadingPatches requested with empty queue");
            return;
        }

        let created = effects.with_manager(|manager, effects| {
            manager.create_layout_with_windows_with_effects("Menus/DownloadMenu.wnd", effects)
        });
        if let Ok((layout, _info)) = created {
            {
                let layout_ref = layout.borrow();
                layout_ref.run_init_with_effects(None, effects);
                layout_ref.hide_receiving(false, effects);
            }
            effects.with_manager(|manager, effects| layout.borrow_mut().bring_forward(manager));
        }

        effects.with_main_menu(|menu, effects| menu.handle_canceled_download(false, effects, None));

        if let Ok(mut guard) = download_manager(network_compat(effects)).lock() {
            if let Some(manager) = guard.as_mut() {
                let _ = manager.download_next_queued_file();
            }
        }
        log::info!("StartDownloadingPatches requested");
    }

    fn launch_world_builder(effects: &mut WindowEventEffects<'_>) {
        let candidates: &[&str] = if cfg!(debug_assertions) {
            &["WorldBuilderD.exe", "WorldBuilderI.exe", "WorldBuilder.exe"]
        } else {
            &["WorldBuilder.exe", "WorldBuilderI.exe", "WorldBuilderD.exe"]
        };
        let launched = candidates
            .iter()
            .any(|exe| std::process::Command::new(exe).spawn().is_ok());
        if !launched {
            let _ = effects.with_manager(|manager, effects| {
                message_box_ok(
                    &crate::game_text::GameText::fetch("GUI:WorldBuilder"),
                    &crate::game_text::GameText::fetch("GUI:WorldBuilderLoadFailed"),
                    None,
                    manager,
                    effects,
                )
            });
        }
    }

    fn to_campaign_difficulty(diff: GameDifficulty) -> CampaignDifficulty {
        match diff {
            GameDifficulty::Easy => CampaignDifficulty::Easy,
            GameDifficulty::Normal => CampaignDifficulty::Normal,
            GameDifficulty::Hard => CampaignDifficulty::Hard,
        }
    }

    fn transition_reverse(
        group: &str,
        effects: &mut WindowEventEffects<'_>,
        selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        if effects.manager.is_none() {
            let owner = effects
                .window_manager_owner
                .expect("WND operation requires receiving manager");
            if owner.try_with_manager(|_| ()).is_none() {
                let group = group.to_string();
                effects.queue_window_operation(move |manager, _effects| {
                    manager.transition_reverse(&group, None);
                });
                return;
            }
        }
        effects.with_manager(|manager, effects| {
            let mut lent: Vec<_> = effects
                .lent_windows
                .iter_mut()
                .map(|window| &mut **window)
                .collect();
            manager.transition_reverse_with_lent(group, selected, &mut lent)
        });
    }

    fn transition_remove(
        group: &str,
        skip_pending: bool,
        effects: &mut WindowEventEffects<'_>,
        selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        if effects.manager.is_none() {
            let owner = effects
                .window_manager_owner
                .expect("WND operation requires receiving manager");
            if owner.try_with_manager(|_| ()).is_none() {
                let group = group.to_string();
                effects.queue_window_operation(move |manager, _effects| {
                    manager.transition_remove(&group, skip_pending, None);
                });
                return;
            }
        }
        effects.with_manager(|manager, effects| {
            let mut lent: Vec<_> = effects
                .lent_windows
                .iter_mut()
                .map(|window| &mut **window)
                .collect();
            manager.transition_remove_with_lent(group, skip_pending, selected, &mut lent)
        });
    }

    fn transitions_finished(effects: &mut WindowEventEffects<'_>) -> bool {
        effects.with_manager_ref(|manager| manager.transitions_finished())
    }

    fn focus_window(id: i32, effects: &mut WindowEventEffects<'_>) {
        effects.with_manager(|manager, effects| {
            manager.request_focus(id);
        });
    }

    /// Handle mouse entering a control
    /// Port of GBM_MOUSE_ENTERING handler - C++ lines 1060-1176
    fn handle_mouse_entering(
        state: &mut MainMenuState,
        control_id: u32,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
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

            Self::transition_set_group(
                "MainMenuFactionTraining",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::debug!("Mouse entering Challenge button");
        } else if control_id == state.window_ids.skirmish_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::Skirmish;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            Self::transition_set_group(
                "MainMenuFactionSkirmish",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::debug!("Mouse entering Skirmish button");
        } else if control_id == state.window_ids.button_usa_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::USA;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            Self::transition_set_group(
                "MainMenuFactionUS",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::debug!("Mouse entering USA button");
        } else if control_id == state.window_ids.button_gla_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::GLA;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            Self::transition_set_group(
                "MainMenuFactionGLA",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::debug!("Mouse entering GLA button");
        } else if control_id == state.window_ids.button_china_id {
            if state.dont_allow_transitions && !state.campaign_selected {
                state.show_logo = true;
                state.show_side = ShowSide::China;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            Self::transition_set_group(
                "MainMenuFactionChina",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::debug!("Mouse entering China button");
        }
    }

    /// Handle mouse leaving a control
    /// Port of GBM_MOUSE_LEAVING handler - C++ lines 1178-1277
    fn handle_mouse_leaving(
        state: &mut MainMenuState,
        control_id: u32,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
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

            Self::transition_reverse("MainMenuFactionTraining", effects, selected.as_deref_mut());
            log::debug!("Mouse leaving Challenge button");
        } else if control_id == state.window_ids.skirmish_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            Self::transition_reverse("MainMenuFactionSkirmish", effects, selected.as_deref_mut());
            log::debug!("Mouse leaving Skirmish button");
        } else if control_id == state.window_ids.button_usa_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            Self::transition_reverse("MainMenuFactionUS", effects, selected.as_deref_mut());
            log::debug!("Mouse leaving USA button");
        } else if control_id == state.window_ids.button_gla_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            Self::transition_reverse("MainMenuFactionGLA", effects, selected.as_deref_mut());
            log::debug!("Mouse leaving GLA button");
        } else if control_id == state.window_ids.button_china_id {
            if state.dont_allow_transitions && !state.campaign_selected && state.show_logo {
                state.show_logo = false;
                state.show_side = ShowSide::None;
            }

            if state.campaign_selected || state.dont_allow_transitions {
                return;
            }

            Self::transition_reverse("MainMenuFactionChina", effects, selected.as_deref_mut());
            log::debug!("Mouse leaving China button");
        }
    }


    /// Initial hide of faction windows
    /// Port of initialHide() - C++ lines 360-425
    fn initial_hide(state: &MainMenuState, effects: &mut WindowEventEffects<'_>) {
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
            Self::hide_window_by_name(state, name, true, effects, None);
        }
    }

    /// Show selective buttons based on faction
    /// Port of showSelectiveButtons() - C++ lines 217-225
    fn show_selective_buttons(
        state: &MainMenuState,
        show: ShowSide,
        effects: &mut WindowEventEffects<'_>,
    ) {
        Self::hide_window_by_id(
            state,
            state.window_ids.button_usa_recent_save_id,
            Some("MainMenu.wnd:ButtonUSARecentSave"),
            show != ShowSide::USA,
            effects,
            None,
        );
        Self::hide_window_by_id(
            state,
            state.window_ids.button_usa_load_game_id,
            Some("MainMenu.wnd:ButtonUSALoadGame"),
            show != ShowSide::USA,
            effects,
            None,
        );
        Self::hide_window_by_id(
            state,
            state.window_ids.button_gla_recent_save_id,
            Some("MainMenu.wnd:ButtonGLARecentSave"),
            show != ShowSide::GLA,
            effects,
            None,
        );
        Self::hide_window_by_id(
            state,
            state.window_ids.button_gla_load_game_id,
            Some("MainMenu.wnd:ButtonGLALoadGame"),
            show != ShowSide::GLA,
            effects,
            None,
        );
        Self::hide_window_by_id(
            state,
            state.window_ids.button_china_recent_save_id,
            Some("MainMenu.wnd:ButtonChinaRecentSave"),
            show != ShowSide::China,
            effects,
            None,
        );
        Self::hide_window_by_id(
            state,
            state.window_ids.button_china_load_game_id,
            Some("MainMenu.wnd:ButtonChinaLoadGame"),
            show != ShowSide::China,
            effects,
            None,
        );
    }

    fn request_quit(state: &mut MainMenuState) {
        Self::queue_action(state, PendingMainMenuAction::QuitRequest);
        log::info!("Quit requested from main menu");
    }

    /// Setup game start
    /// Port of setupGameStart() - C++ lines 253-273
    fn setup_game_start(
        state: &mut MainMenuState,
        map_name: &str,
        diff: GameDifficulty,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        let Some(campaign_manager) = effects.campaign_manager.as_deref_mut() else {
            return;
        };
        campaign_manager.set_game_difficulty(Self::to_campaign_difficulty(diff));

        if state.launch_challenge_menu {
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
            // Shell menus retain the INI GlobalData Arc, while AuthorityOnly Main
            // consumes NewGame from the runtime GlobalData singleton. C++ had one
            // TheWritableGlobalData, so keep the selected campaign map coherent in
            // both residences before the reverse animation can emit NewGame.
            let pending_file = map_name.to_string();
            if let Some(data) = get_global_data() {
                data.write().pending_file = pending_file.clone();
            }
            runtime_global_data::write().pending_file = pending_file;
            Self::queue_action(state, PendingMainMenuAction::ReverseAnimateWindow);
            Self::transition_set_group("FadeWholeScreen", false, effects, selected.as_deref_mut());
            log::info!("Starting game: {} with difficulty: {:?}", map_name, diff);
        }
    }

    /// Public wrapper used by external menu callbacks that must follow
    /// MainMenu::setupGameStart behavior (for example DifficultySelect popup flow).
    pub fn setup_game_start_from_callback(
        &mut self,
        map_name: &str,
        diff: GameDifficulty,
        effects: &mut WindowEventEffects<'_>,
    ) {
        let mut state = &mut self.state;
        Self::setup_game_start(&mut state, map_name, diff, effects, None);
    }

    /// Prepare campaign game
    /// Port of prepareCampaignGame() - C++ lines 275-286
    fn prepare_campaign_game(
        state: &mut MainMenuState,
        diff: GameDifficulty,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        state.dont_allow_transitions = true;
        let map_name = {
            let Some(campaign_manager) = effects.campaign_manager.as_deref_mut() else {
                return;
            };
            campaign_manager.set_game_difficulty(Self::to_campaign_difficulty(diff));
            campaign_manager.get_current_map().unwrap_or_default()
        };
        let mut prefs = UserPreferences::new();
        let _ = prefs.load("Options.ini");
        prefs.set_int("CampaignDifficulty", diff as i32);
        let _ = prefs.write();
        effects.request_menu_host(MenuHostRequest::SetDifficulty(diff as i32));

        state.button_pushed = false;
        Self::transition_reverse(
            "MainMenuDifficultyMenuBack",
            effects,
            selected.as_deref_mut(),
        );
        // C++ setupGameStart(TheCampaignManager->getCurrentMap(), diff)
        // even when the map string is empty (Challenge uses launchChallengeMenu).
        Self::setup_game_start(state, &map_name, diff, effects, selected.as_deref_mut());

        log::info!("Preparing campaign game with difficulty: {:?}", diff);
    }

    fn run_campaign_start_after_cd_check(
        &mut self,
        diff: GameDifficulty,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        let mut state = &mut self.state;
        Self::prepare_campaign_game(&mut state, diff, effects, selected.as_deref_mut());
    }

    fn retry_campaign_start_after_cd_check(
        &mut self,
        diff: GameDifficulty,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) -> ExMessageBoxReturnType {
        self.retry_campaign_start_after_cd_check_with_status(
            diff,
            Self::is_first_cd_present(),
            effects,
            selected.as_deref_mut(),
        )
    }

    fn retry_campaign_start_after_cd_check_with_status(
        &mut self,
        diff: GameDifficulty,
        cd_present: bool,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) -> ExMessageBoxReturnType {
        if !cd_present {
            return ExMessageBoxReturnType::KeepOpen;
        }

        self.run_campaign_start_after_cd_check(diff, effects, selected.as_deref_mut());
        ExMessageBoxReturnType::Close
    }

    fn cancel_campaign_start_after_cd_check(&mut self) {
        let mut state = &mut self.state;
        state.button_pushed = false;
    }

    fn is_first_cd_present() -> bool {
        // C++ IsFirstCDPresent (CDCheck.h) — not launcher copy-protection.
        crate::cd_check::is_first_cd_present()
    }

    /// Check CD before starting campaign
    /// Port of checkCDBeforeCampaign() - C++ lines 323-335
    fn check_cd_before_campaign(
        state: &mut MainMenuState,
        diff: GameDifficulty,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        if !Self::is_first_cd_present() {
            state.button_pushed = false;
            effects
                .pending_main_menu_actions
                .push((PendingMainMenuAction::CampaignCdDialog(diff), None));
            return;
        }

        Self::prepare_campaign_game(state, diff, effects, selected.as_deref_mut());
        log::info!("Checking CD before campaign start");
    }

    fn show_campaign_cd_dialog(diff: GameDifficulty, effects: &mut WindowEventEffects<'_>) {
        let ok: ExtendedMessageBoxFunc = Box::new(move |_user_data, effects| {
            effects.with_main_menu(|menu, effects| {
                menu.retry_campaign_start_after_cd_check(diff, effects, None)
            })
        });
        let cancel: ExtendedMessageBoxFunc = Box::new(|_user_data, effects| {
            effects.with_main_menu(|menu, _| menu.cancel_campaign_start_after_cd_check());
            ExMessageBoxReturnType::Close
        });
        let _ = effects.with_manager(|manager, effects| {
            ex_message_box_ok_cancel(
                &crate::game_text::GameText::fetch("GUI:InsertCDPrompt"),
                &crate::game_text::GameText::fetch("GUI:InsertCDMessage"),
                None,
                Some(ok),
                Some(cancel),
                manager,
                effects,
            )
        });
    }

    /// Do game start
    /// Port of doGameStart() - C++ lines 306-321
    fn do_game_start(
        state: &mut MainMenuState,
        layout: &dyn std::any::Any,
        effects: &mut WindowEventEffects<'_>,
    ) -> MainMenuResult<()> {
        state.start_game = false;

        let Some(campaign_manager) = effects.campaign_manager.as_deref_mut() else {
            return Ok(());
        };
        let (difficulty, rank_points, campaign_name, campaign_player_faction, map_name) = {
            let campaign = campaign_manager.get_current_campaign();
            (
                campaign_manager.get_game_difficulty() as i32,
                campaign_manager.get_rank_points(),
                campaign
                    .map(|campaign| campaign.name.clone())
                    .unwrap_or_default(),
                campaign
                    .map(|campaign| campaign.player_faction_name.clone())
                    .unwrap_or_default(),
                campaign_manager.get_current_map().unwrap_or_default(),
            )
        };
        // C++ has one GameInfo/GlobalData path from MainMenu through
        // GameLogic.  Main's AuthorityOnly host needs the same campaign
        // identity at the moment this exact MSG_NEW_GAME is enqueued; a later
        // HUD faction is not a valid substitute.  Normal campaigns do not
        // select a challenge template here.
        let descriptor = HostCampaignLaunchDescriptor {
            map_name,
            campaign_name,
            campaign_player_faction,
            is_challenge: false,
            game_info_starting_cash: None,
            player_template_name: None,
            player_template_index: None,
            game_mode_code: gamelogic::system::game_logic::GAME_SINGLE_PLAYER,
            difficulty_code: difficulty,
            rank_points,
            max_fps: None,
        };
        let clear_previous = effects.menu_world().in_game;
        let source_layout = layout
            .downcast_ref::<ManagerWindowLayout>()
            .and_then(|layout| layout.get_first_window())
            .and_then(|window| window.borrow().get_layout())
            .map(|layout| Rc::downgrade(&layout));
        Self::queue_action(
            state,
            PendingMainMenuAction::HostRequest(MenuHostRequest::StartCampaign {
                descriptor,
                clear_previous,
                source: MenuLaunchSource::MainMenu,
                main_menu_layout: source_layout,
            }),
        );
        log::info!("Starting new game");
        Ok(())
    }

    /// Finish the layout/state side of shutdownComplete() without re-entering shell shutdown.
    /// Port of the layout/state portion of shutdownComplete() - C++ lines 337-347.
    fn finish_shutdown_complete(
        layout: Option<&dyn std::any::Any>,
        state: &mut MainMenuState,
        effects: &mut WindowEventEffects<'_>,
    ) -> MainMenuResult<()> {
        if let Some(layout) = layout.and_then(|any| any.downcast_ref::<ManagerWindowLayout>()) {
            layout.hide_receiving(true, effects);
        }
        state.is_shutting_down = false;
        Ok(())
    }

    fn complete_shell_shutdown(effects: &mut WindowEventEffects<'_>) -> MainMenuResult<()> {
        queue_shell_shutdown_complete(effects, false);
        Ok(())
    }

    /// Reverse side for difficulty menu
    /// Port of diffReverseSide() - C++ lines 1690-1711
    fn diff_reverse_side(
        state: &MainMenuState,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        match state.show_side {
            ShowSide::Training => {
                Self::transition_reverse(
                    "MainMenuDifficultyMenuTrainingBack",
                    effects,
                    selected.as_deref_mut(),
                );
                Self::transition_set_group(
                    "MainMenuSinglePlayerTrainingMenuFromDiff",
                    false,
                    effects,
                    selected.as_deref_mut(),
                );
                log::debug!("Reversing difficulty menu - Training");
            }
            ShowSide::USA => {
                Self::transition_reverse(
                    "MainMenuDifficultyMenuUSBack",
                    effects,
                    selected.as_deref_mut(),
                );
                Self::transition_set_group(
                    "MainMenuSinglePlayerUSAMenuFromDiff",
                    false,
                    effects,
                    selected.as_deref_mut(),
                );
                log::debug!("Reversing difficulty menu - USA");
            }
            ShowSide::GLA => {
                Self::transition_reverse(
                    "MainMenuDifficultyMenuGLABack",
                    effects,
                    selected.as_deref_mut(),
                );
                Self::transition_set_group(
                    "MainMenuSinglePlayerGLAMenuFromDiff",
                    false,
                    effects,
                    selected.as_deref_mut(),
                );
                log::debug!("Reversing difficulty menu - GLA");
            }
            ShowSide::China => {
                Self::transition_reverse(
                    "MainMenuDifficultyMenuChinaBack",
                    effects,
                    selected.as_deref_mut(),
                );
                Self::transition_set_group(
                    "MainMenuSinglePlayerChinaMenuFromDiff",
                    false,
                    effects,
                    selected.as_deref_mut(),
                );
                log::debug!("Reversing difficulty menu - China");
            }
            _ => {}
        }
    }

}

impl Default for MainMenu {
    fn default() -> Self {
        Self::new()
    }
}



// ================================================================================================
// TESTS
// ================================================================================================

#[cfg(test)]
#[path = "main_menu_tests.rs"]
mod tests;
