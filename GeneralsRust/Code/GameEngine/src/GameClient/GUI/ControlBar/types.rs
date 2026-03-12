// FILE: types.rs
// Port of ControlBar types and enums from C++
// Original: ControlBar.h

use std::collections::HashMap;
use crate::common::types::*;

// Command options bitflags
pub const COMMAND_OPTION_NONE: u32 = 0x00000000;
pub const NEED_TARGET_ENEMY_OBJECT: u32 = 0x00000001;
pub const NEED_TARGET_NEUTRAL_OBJECT: u32 = 0x00000002;
pub const NEED_TARGET_ALLY_OBJECT: u32 = 0x00000004;
#[cfg(feature = "allow_surrender")]
pub const NEED_TARGET_PRISONER: u32 = 0x00000008;
pub const ALLOW_SHRUBBERY_TARGET: u32 = 0x00000010;
pub const NEED_TARGET_POS: u32 = 0x00000020;
pub const NEED_UPGRADE: u32 = 0x00000040;
pub const NEED_SPECIAL_POWER_SCIENCE: u32 = 0x00000080;
pub const OK_FOR_MULTI_SELECT: u32 = 0x00000100;
pub const CONTEXTMODE_COMMAND: u32 = 0x00000200;
pub const CHECK_LIKE: u32 = 0x00000400;
pub const ALLOW_MINE_TARGET: u32 = 0x00000800;
pub const ATTACK_OBJECTS_POSITION: u32 = 0x00001000;
pub const OPTION_ONE: u32 = 0x00002000;
pub const OPTION_TWO: u32 = 0x00004000;
pub const OPTION_THREE: u32 = 0x00008000;
pub const NOT_QUEUEABLE: u32 = 0x00010000;
pub const SINGLE_USE_COMMAND: u32 = 0x00020000;
pub const COMMAND_FIRED_BY_SCRIPT: u32 = 0x00040000;
pub const SCRIPT_ONLY: u32 = 0x00080000;
pub const IGNORES_UNDERPOWERED: u32 = 0x00100000;
pub const USES_MINE_CLEARING_WEAPONSET: u32 = 0x00200000;
pub const CAN_USE_WAYPOINTS: u32 = 0x00400000;
pub const MUST_BE_STOPPED: u32 = 0x00800000;

// Convenient bit masks to group command options together
pub const COMMAND_OPTION_NEED_TARGET: u32 =
    NEED_TARGET_ENEMY_OBJECT |
    NEED_TARGET_NEUTRAL_OBJECT |
    NEED_TARGET_ALLY_OBJECT |
    NEED_TARGET_POS |
    CONTEXTMODE_COMMAND;

pub const COMMAND_OPTION_NEED_OBJECT_TARGET: u32 =
    NEED_TARGET_ENEMY_OBJECT |
    NEED_TARGET_NEUTRAL_OBJECT |
    NEED_TARGET_ALLY_OBJECT;

// Command option names for parsing
pub const COMMAND_OPTION_NAMES: &[&str] = &[
    "NEED_TARGET_ENEMY_OBJECT",
    "NEED_TARGET_NEUTRAL_OBJECT",
    "NEED_TARGET_ALLY_OBJECT",
    #[cfg(feature = "allow_surrender")]
    "NEED_TARGET_PRISONER",
    #[cfg(not(feature = "allow_surrender"))]
    "unused-reserved",
    "ALLOW_SHRUBBERY_TARGET",
    "NEED_TARGET_POS",
    "NEED_UPGRADE",
    "NEED_SPECIAL_POWER_SCIENCE",
    "OK_FOR_MULTI_SELECT",
    "CONTEXTMODE_COMMAND",
    "CHECK_LIKE",
    "ALLOW_MINE_TARGET",
    "ATTACK_OBJECTS_POSITION",
    "OPTION_ONE",
    "OPTION_TWO",
    "OPTION_THREE",
    "NOT_QUEUEABLE",
    "SINGLE_USE_COMMAND",
    "---DO-NOT-USE---", // COMMAND_FIRED_BY_SCRIPT
    "SCRIPT_ONLY",
    "IGNORES_UNDERPOWERED",
    "USES_MINE_CLEARING_WEAPONSET",
    "CAN_USE_WAYPOINTS",
    "MUST_BE_STOPPED",
];

/// GUI Command Types
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GUICommandType {
    None = 0,
    DozerConstruct,
    DozerConstructCancel,
    UnitBuild,
    CancelUnitBuild,
    PlayerUpgrade,
    ObjectUpgrade,
    CancelUpgrade,
    AttackMove,
    Guard,
    GuardWithoutPursuit,
    GuardFlyingUnitsOnly,
    Stop,
    Waypoints,
    ExitContainer,
    Evacuate,
    ExecuteRailedTransport,
    BeaconDelete,
    SetRallyPoint,
    Sell,
    FireWeapon,
    SpecialPower,
    PurchaseScience,
    HackInternet,
    ToggleOvercharge,
    #[cfg(feature = "allow_surrender")]
    PowReturnToPrison,
    CombatDrop,
    SwitchWeapon,

    // Context sensitive command modes
    HijackVehicle,
    ConvertToCarBomb,
    SabotageBuilding,
    #[cfg(feature = "allow_surrender")]
    PickUpPrisoner,

    // Context-insensitive command mode(s)
    PlaceBeacon,

    SpecialPowerFromShortcut,
    SpecialPowerConstruct,
    SpecialPowerConstructFromShortcut,

    SelectAllUnitsOfType,

    NumCommands,
}

impl GUICommandType {
    pub fn from_name(name: &str) -> Option<Self> {
        GUI_COMMAND_NAMES.iter()
            .position(|&n| n.eq_ignore_ascii_case(name))
            .and_then(|i| Self::from_u32(i as u32))
    }

    pub fn from_u32(value: u32) -> Option<Self> {
        if value < Self::NumCommands as u32 {
            Some(unsafe { std::mem::transmute(value) })
        } else {
            None
        }
    }

    pub fn to_name(&self) -> &'static str {
        GUI_COMMAND_NAMES[*self as usize]
    }
}

pub const GUI_COMMAND_NAMES: &[&str] = &[
    "NONE",
    "DOZER_CONSTRUCT",
    "DOZER_CONSTRUCT_CANCEL",
    "UNIT_BUILD",
    "CANCEL_UNIT_BUILD",
    "PLAYER_UPGRADE",
    "OBJECT_UPGRADE",
    "CANCEL_UPGRADE",
    "ATTACK_MOVE",
    "GUARD",
    "GUARD_WITHOUT_PURSUIT",
    "GUARD_FLYING_UNITS_ONLY",
    "STOP",
    "WAYPOINTS",
    "EXIT_CONTAINER",
    "EVACUATE",
    "EXECUTE_RAILED_TRANSPORT",
    "BEACON_DELETE",
    "SET_RALLY_POINT",
    "SELL",
    "FIRE_WEAPON",
    "SPECIAL_POWER",
    "PURCHASE_SCIENCE",
    "HACK_INTERNET",
    "TOGGLE_OVERCHARGE",
    #[cfg(feature = "allow_surrender")]
    "POW_RETURN_TO_PRISON",
    "COMBATDROP",
    "SWITCH_WEAPON",
    "HIJACK_VEHICLE",
    "CONVERT_TO_CARBOMB",
    "SABOTAGE_BUILDING",
    #[cfg(feature = "allow_surrender")]
    "PICK_UP_PRISONER",
    "PLACE_BEACON",
    "SPECIAL_POWER_FROM_SHORTCUT",
    "SPECIAL_POWER_CONSTRUCT",
    "SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT",
    "SELECT_ALL_UNITS_OF_TYPE",
];

/// Command button mapped border type
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandButtonMappedBorderType {
    None = 0,
    Build,
    Upgrade,
    Action,
    System,
    Count,
}

impl CommandButtonMappedBorderType {
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "NONE" => Some(Self::None),
            "BUILD" => Some(Self::Build),
            "UPGRADE" => Some(Self::Upgrade),
            "ACTION" => Some(Self::Action),
            "SYSTEM" => Some(Self::System),
            _ => None,
        }
    }
}

/// Control Bar Context
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlBarContext {
    None = 0,
    Command,
    StructureInventory,
    Beacon,
    UnderConstruction,
    MultiSelect,
    ObserverInfo,
    ObserverList,
    OclTimer,
    NumContexts,
}

/// Context Parents
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextParent {
    Master = 0,
    PurchaseScience,
    Command,
    BuildQueue,
    Beacon,
    UnderConstruction,
    ObserverInfo,
    ObserverList,
    OclTimer,
    NumContextParents,
}

/// CB Command Status
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CBCommandStatus {
    NotUsed = 0,
    Used,
}

/// Command Availability
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandAvailability {
    Restricted,
    Available,
    Active,
    Hidden,
    NotReady,
    CantAfford,
}

/// Control Bar Stages
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlBarStages {
    Default = 0,
    Squished,
    Low,
    Hidden,
    MaxStages,
}

// Constants for UI layout
pub const MAX_COMMANDS_PER_SET: usize = 18;
pub const MAX_RIGHT_HUD_UPGRADE_CAMEOS: usize = 5;
pub const MAX_PURCHASE_SCIENCE_RANK_1: usize = 4;
pub const MAX_PURCHASE_SCIENCE_RANK_3: usize = 15;
pub const MAX_PURCHASE_SCIENCE_RANK_8: usize = 4;
pub const MAX_STRUCTURE_INVENTORY_BUTTONS: usize = 10;
pub const MAX_BUILD_QUEUE_BUTTONS: usize = 9;
pub const MAX_SPECIAL_POWER_SHORTCUTS: usize = 11;

/// Side Select Window Data State
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideSelectState {
    None = 0,
    State1,
    State2,
    State3,
    State4,
    State5,
    State6,
}
