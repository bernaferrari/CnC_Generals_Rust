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
use game_engine::common::rts::ScienceType;
use game_engine::ini::AudioEventRTS;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub mod beacon;
pub mod commands;
pub mod control_bar;
pub mod control_bar_beacon;
pub mod control_bar_command;
pub mod control_bar_command_processing;
pub mod control_bar_multi_select;
pub mod control_bar_observer;
pub mod control_bar_ocl_timer;
pub mod control_bar_print_positions;
pub mod control_bar_resizer;
pub mod control_bar_scheme;
pub mod control_bar_structure_inventory;
pub mod control_bar_under_construction;
pub mod multi_select;
pub mod observer;
pub mod resizer;
pub mod scheme;
pub mod structure_inventory;
pub mod under_construction;

pub use beacon::*;
pub use commands::*;
pub use control_bar::*;
pub use control_bar_print_positions::*;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlBarState {
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
}

/// Command button data matching C++ CommandButton
#[derive(Debug, Clone)]
pub struct CommandButton {
    pub command_name: String,
    pub command_type: gamelogic::commands::CommandType,
    pub button_image: String,
    pub button_border_type: String,
    pub text_label: String,
    pub text_label_placehold: String,
    pub descriptive_text: String,
    pub conflicting_element: String,
    pub cursor_name: String,
    pub invalid_cursor_name: String,
    pub unit_specific_sound: AudioEventRTS,
    pub max_shorable_instances: i32,
    pub options: u32, // CommandOption flags
    pub sciences: Vec<String>,
    pub sciences_ids: Vec<ScienceType>,
    pub upgrade: String,
    pub special_power: String,
    pub object: String,
    pub radius_cursor_type: String,
    pub purchase_cost: HashMap<String, i32>,
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

impl Default for ControlBarState {
    fn default() -> Self {
        Self::None
    }
}

impl Default for ControlBarContext {
    fn default() -> Self {
        Self {
            selected_objects: Vec::new(),
            player_id: 0,
            current_state: ControlBarState::None,
            available_commands: Vec::new(),
            construction_queue: Vec::new(),
        }
    }
}

impl Default for CommandButton {
    fn default() -> Self {
        Self {
            command_name: String::new(),
            command_type: gamelogic::commands::CommandType::Invalid,
            button_image: String::new(),
            button_border_type: String::new(),
            text_label: String::new(),
            text_label_placehold: String::new(),
            descriptive_text: String::new(),
            conflicting_element: String::new(),
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            unit_specific_sound: AudioEventRTS::default(),
            max_shorable_instances: 1,
            options: CommandOption::None as u32,
            sciences: Vec::new(),
            sciences_ids: Vec::new(),
            upgrade: String::new(),
            special_power: String::new(),
            object: String::new(),
            radius_cursor_type: String::new(),
            purchase_cost: HashMap::new(),
        }
    }
}
