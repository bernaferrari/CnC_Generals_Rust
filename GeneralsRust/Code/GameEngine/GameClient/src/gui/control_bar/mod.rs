////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Control Bar System
//!
//! Rust conversion of the C++ ControlBar system that provides context-sensitive
//! command interface for RTS games. This is the main UI system for commanding
//! units, buildings, and managing game state.
//!
//! Converted from: GameClient/GUI/ControlBar/
//! Original Author: Colin Day, March 2002
#![allow(missing_docs)]
#![allow(ambiguous_glob_reexports)]
#![allow(unused_imports)]

use crate::gui::GameWindow;
use crate::system::SubsystemInterface;
use game_engine::common::rts::{ScienceType, WeaponSlotType};
use game_engine::ini::AudioEventRTS;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub mod beacon;
pub mod commands;
#[path = "control_bar_impl/mod.rs"]
pub mod control_bar;
pub mod control_bar_beacon;
pub mod control_bar_command;
pub mod control_bar_command_processing;
pub mod control_bar_multi_select;
mod host_command_bridge;
pub use control_bar_multi_select::{
    MULTI_SELECT_MAX_COMMANDS_PER_SET, MULTI_SELECT_OK_FOR_MULTI_SELECT_BIT,
    ResidualMultiSelectAction, ResidualMultiSelectUnit, residual_multi_select_actionable_count,
    residual_multi_select_attack_move_kept, residual_multi_select_common_command_count,
    residual_multi_select_last_action, residual_multi_select_portrait_agrees,
    residual_multi_select_selected_count, simulate_multi_select_clear,
    simulate_multi_select_populate, simulate_multi_select_prepare_divergent,
    simulate_multi_select_prepare_same_commands,
};
pub mod control_bar_observer;
pub mod control_bar_ocl_timer;
pub use control_bar_ocl_timer::{
    OCLTimerDisplayState, ResidualOclTimerAction, format_ocl_timer_display, ocl_frames_to_display,
    residual_ocl_timer_last_action, residual_ocl_timer_progress_milli, residual_ocl_timer_seconds,
    should_update_timer_text, simulate_ocl_timer_format, simulate_ocl_timer_frames_to_display,
    simulate_ocl_timer_prepare_display, simulate_ocl_timer_should_update,
};
pub mod control_bar_print_positions;
pub use control_bar_print_positions::{
    CONTROL_BAR_PRINT_HIDDEN_SCRIPT, CONTROL_BAR_PRINT_OUTPUT_FILE, CONTROL_BAR_PRINT_PARENT_NAME,
    ResidualControlBarPrintPositionsAction, residual_control_bar_print_positions_last_action,
    residual_control_bar_print_positions_line_len,
    simulate_control_bar_print_positions_format_line,
    simulate_control_bar_print_positions_parent_name,
    simulate_control_bar_print_positions_prepare_sample,
    simulate_control_bar_print_positions_script_names,
};
pub mod control_bar_resizer;
pub use control_bar_resizer::{
    IniControlBarResizer, ResidualControlBarResizerAction, ResizerWindow,
    residual_control_bar_resizer_base_resolution, residual_control_bar_resizer_last_action,
    residual_control_bar_resizer_window_count, simulate_control_bar_resizer_add_window,
    simulate_control_bar_resizer_clear, simulate_control_bar_resizer_get_optimal_size,
    simulate_control_bar_resizer_prepare_default, simulate_control_bar_resizer_resize,
    simulate_control_bar_resizer_set_base_resolution,
};
pub mod control_bar_scheme;
pub mod control_bar_structure_inventory;
pub use control_bar_structure_inventory::{
    MAX_STRUCTURE_INVENTORY_BUTTONS, ResidualStructureInventoryAction,
    STRUCTURE_INVENTORY_EVACUATE_COMMAND_NAME, STRUCTURE_INVENTORY_EVACUATE_ID,
    STRUCTURE_INVENTORY_EXIT_COMMAND_NAME, STRUCTURE_INVENTORY_STOP_COMMAND_NAME,
    STRUCTURE_INVENTORY_STOP_ID, StructureInventoryOccupant, occupant_from_presentation,
    residual_structure_inventory_evacuate_visible, residual_structure_inventory_exit_visible,
    residual_structure_inventory_garrisoned_count, residual_structure_inventory_last_action,
    residual_structure_inventory_max_garrison, residual_structure_inventory_stop_visible,
    simulate_structure_inventory_clear, simulate_structure_inventory_evacuate_command_name,
    simulate_structure_inventory_exit_command_name, simulate_structure_inventory_populate,
    simulate_structure_inventory_prepare_occupied, simulate_structure_inventory_stop_command_name,
};
pub mod control_bar_under_construction;
pub use control_bar_under_construction::{
    ResidualUnderConstructionAction, UNDER_CONSTRUCTION_CANCEL_COMMAND_NAME,
    format_under_construction_percent_text, residual_under_construction_cancel_visible,
    residual_under_construction_is_completed, residual_under_construction_last_action,
    residual_under_construction_percent, simulate_under_construction_cancel_command_name,
    simulate_under_construction_complete, simulate_under_construction_populate,
    simulate_under_construction_prepare_cycle, simulate_under_construction_update_percent,
};
pub mod multi_select;
pub mod observer;
pub mod resizer;
pub mod scheme;
pub use scheme::{
    CONTROL_BAR_SCHEME_NAMES_8X6, DefaultControlBarSchemeManager, ResidualControlBarSchemeAction,
    residual_control_bar_scheme_has_current, residual_control_bar_scheme_last_action,
    residual_control_bar_scheme_loaded_count, simulate_control_bar_scheme_clear,
    simulate_control_bar_scheme_get_current, simulate_control_bar_scheme_load,
    simulate_control_bar_scheme_prepare_default,
};
pub mod structure_inventory;
pub mod under_construction;

pub use beacon::*;
pub use commands::*;
pub use control_bar::*;
pub use control_bar_print_positions::*;
#[cfg(test)]
pub(crate) use host_command_bridge::acquire_host_control_bar_bridge_test_guard;
pub use host_command_bridge::{
    HostControlBarInputProvenance, HostControlBarPublishedRequest, HostControlBarRequest,
    HostControlBarTarget, HostMinimapInteraction, HostMinimapMouseButton,
    clear_host_control_bar_requests, clear_host_dismiss_in_game_popup_message_requests,
    host_control_bar_bridge_enabled, set_host_control_bar_bridge_enabled,
    take_host_control_bar_published_requests, take_host_control_bar_requests,
    take_host_minimap_interactions, with_host_control_bar_input_provenance,
};
pub(crate) use host_command_bridge::{
    HostMinimapInteractionRequest, host_control_bar_input_provenance_for_current_dispatch,
    host_request_from_button, host_request_from_button_with_weapon_slot,
    publish_host_cancel_structure_placement, publish_host_control_bar_request,
    publish_host_dismiss_in_game_popup_message, publish_host_minimap_interaction,
    publish_host_production_pause, publish_host_queue_cancel, publish_host_select_next_idle_worker,
};
pub use multi_select::*;
pub use observer::*;
pub use resizer::*;
pub use scheme::*;
pub use structure_inventory::*;
pub use under_construction::*;

/// Command options matching C++ enum CommandOption
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandOption {
    None = 0x00000000,
    NeedTargetEnemyObject = 0x00000001,
    NeedTargetNeutralObject = 0x00000002,
    NeedTargetAllyObject = 0x00000004,
    #[cfg(feature = "allow_surrender")]
    NeedTargetPrisoner = 0x00000008,
    AllowShrubberyTarget = 0x00000010,
    NeedTargetPos = 0x00000020,
    NeedUpgrade = 0x00000040,
    NeedSpecialPowerScience = 0x00000080,
    OkForMultiSelect = 0x00000100,
    ContextmodeCommand = 0x00000200,
    CheckLike = 0x00000400,
    AllowMineTarget = 0x00000800,
    AttackObjectsPosition = 0x00001000,
    OptionOne = 0x00002000,
    OptionTwo = 0x00004000,
    OptionThree = 0x00008000,
    NotQueueable = 0x00010000,
    SingleUseCommand = 0x00020000,
    CommandFiredByScript = 0x00040000,
    ScriptOnly = 0x00080000,
    IgnoresUnderpowered = 0x00100000,
    UsesMineClearingWeaponSet = 0x00200000,
    CanUseWaypoints = 0x00400000,
    MustBeStopped = 0x00800000,
}

/// Command source type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandSourceType {
    None,
    FromUser,
    FromScript,
    FromAI,
}

/// Production type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionType {
    Unit,
    Structure,
    Upgrade,
    SpecialPower,
}

/// Maximum number of build queue buttons displayed in the UI.
/// C++: MAX_BUILD_QUEUE_BUTTONS
pub const MAX_BUILD_QUEUE_BUTTONS: usize = 9;

/// Control bar state - mirrors C++ ControlBarContext enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlBarState {
    #[default]
    None,
    Command,
    MultiSelect,
    Observer,
    UnderConstruction,
    StructureInventory,
    Beacon,
    OclTimer,
}

/// Command availability result - mirrors C++ CommandAvailability
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAvailability {
    Available,
    Restricted,
    Active,
    Hidden,
    NotReady,
    CantAfford,
}

/// Production type in the build queue - mirrors C++ ProductionType
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueProductionType {
    Invalid,
    Unit,
    Upgrade,
}

/// Build queue entry data - mirrors C++ ControlBar::QueueEntry
#[derive(Debug, Clone)]
pub struct BuildQueueEntry {
    pub production_type: QueueProductionType,
    pub production_id: u32,
    pub upgrade_name: String,
}

/// Control bar context data
#[derive(Debug, Clone)]
pub struct ControlBarContext {
    pub selected_objects: Vec<u32>,
    pub player_id: u32,
    pub current_state: ControlBarState,
    pub available_commands: Vec<CommandButton>,
    pub construction_queue: Vec<ProductionItem>,
    pub observer_player_stats: Vec<(String, i32, i32, i32, i32, i32)>,
    pub last_recorded_inventory_count: u32,
    pub ui_dirty: bool,
    /// C++ `m_containData[].objectID` — per-slot occupant for StructureExit.
    pub contain_data: Vec<Option<u32>>,
    /// C++ `m_observerLookAtPlayer` index into ThePlayerList.
    pub observer_look_at_player: Option<i32>,
}

/// Command button data matching C++ CommandButton
#[derive(Debug, Clone)]
pub struct CommandButton {
    pub command_name: String,
    pub command_type: gamelogic::commands::CommandType,
    /// C++ `CommandButton::m_command` (`PLAYER_UPGRADE` vs `OBJECT_UPGRADE`).
    pub gui_command: String,
    pub button_image: String,
    pub button_border_type: String,
    pub text_label: String,
    pub text_label_placehold: String,
    pub descriptive_text: String,
    pub conflicting_element: String,
    pub cursor_name: String,
    pub invalid_cursor_name: String,
    pub unit_specific_sound: AudioEventRTS,
    /// Exact `MaxShotsToFire =` value parsed from the command button INI.
    ///
    /// This is weapon-order data, not a UI inventory limit: it must survive
    /// the host Control Bar bridge so buttons such as the MiG's one-missile
    /// salvo do not silently become unlimited attacks.
    pub max_shots_to_fire: i32,
    /// The exact `WeaponSlot =` parsed from the INI command button.
    ///
    /// This belongs to the live UI definition rather than the legacy
    /// GameLogic command-button mirror, so host-owned input can preserve the
    /// selected weapon without consulting a separate simulation.
    pub weapon_slot: WeaponSlotType,
    pub options: u32, // CommandOption flags
    pub sciences: Vec<String>,
    pub sciences_ids: Vec<ScienceType>,
    pub upgrade: String,
    pub special_power: String,
    pub object: String,
    pub radius_cursor_type: String,
    pub purchase_cost: HashMap<String, i32>,
    /// C++ inventory slot occupant (`m_containData.objectID`).
    pub exit_object_id: Option<u32>,
    /// C++ `GadgetButtonDrawOverlayImage` veterancy chevron.
    pub overlay_image: Option<String>,
    pub button_enabled: bool,
    pub button_hidden: bool,
}

/// Production item in queue
#[derive(Debug, Clone)]
pub struct ProductionItem {
    pub template_name: String,
    pub production_type: ProductionType,
    pub progress: f32,
    pub cost: HashMap<String, i32>,
    pub build_time: f32,
}

impl Default for ControlBarContext {
    fn default() -> Self {
        Self {
            selected_objects: Vec::new(),
            player_id: 0,
            current_state: ControlBarState::None,
            available_commands: Vec::new(),
            construction_queue: Vec::new(),
            observer_player_stats: Vec::new(),
            last_recorded_inventory_count: 0,
            ui_dirty: false,
            contain_data: Vec::new(),
            observer_look_at_player: None,
        }
    }
}

impl Default for CommandButton {
    fn default() -> Self {
        Self {
            command_name: String::new(),
            command_type: gamelogic::commands::CommandType::Invalid,
            gui_command: String::new(),
            button_image: String::new(),
            button_border_type: String::new(),
            text_label: String::new(),
            text_label_placehold: String::new(),
            descriptive_text: String::new(),
            conflicting_element: String::new(),
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            unit_specific_sound: AudioEventRTS::default(),
            max_shots_to_fire: i32::MAX,
            weapon_slot: WeaponSlotType::Primary,
            options: CommandOption::None as u32,
            sciences: Vec::new(),
            sciences_ids: Vec::new(),
            upgrade: String::new(),
            special_power: String::new(),
            object: String::new(),
            radius_cursor_type: String::new(),
            purchase_cost: HashMap::new(),
            exit_object_id: None,
            overlay_image: None,
            button_enabled: true,
            button_hidden: false,
        }
    }
}

impl CommandButton {
    /// Stable C++ `WeaponSlotType` numbering without a lossy enum cast.
    pub(crate) fn weapon_slot_number(&self) -> u32 {
        match self.weapon_slot {
            WeaponSlotType::Primary => 0,
            WeaponSlotType::Secondary => 1,
            WeaponSlotType::Tertiary => 2,
        }
    }
}
