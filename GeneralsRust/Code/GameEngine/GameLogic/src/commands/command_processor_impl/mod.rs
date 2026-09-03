////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

//! Command Processor - Command execution engine
//!
//! This module provides the command execution system that processes
//! commands from the queue and translates them into game actions.
//! Matches C++ command processing and GameLogic integration.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use super::command::{Command, CommandType, CommandValidation};
use super::command_queue::{
    CommandExecutionState, CommandPriority, QueuedCommand, get_command_queue_manager,
};
use super::rts_command::{RtsCommand, RtsCommandValidator};
use crate::action_manager::TheActionManager;
use crate::commands::get_selection_manager;
use crate::common::{
    AsciiString, Bool, CommandSourceType, Coord3D, DrawableID, EvaEvent, FormationID, ICoord2D,
    IRegion2D, Int, KindOf, ObjectID, ObjectStatusTypes, PlayerMaskType, Real, Relationship,
    UnsignedInt, audio::AudioEventRts,
};
use crate::control_bar;
use crate::helpers::{
    TheAudio, TheEva, TheGameLogic, TheGameText, TheInGameUI, TheTerrainLogic, TheThingFactory,
};
use crate::modules::{
    AIUpdateInterfaceExt, ContainModuleInterfaceExt,
    SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface,
    SpecialPowerUpdateInterface as EngineSpecialPowerUpdateInterface,
};
use crate::object::object_factory::{GameObjectInstance, get_object_factory};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::player::player_list;
use crate::system::beacon_manager::{BeaconManager, get_beacon_manager};
use crate::upgrade::center::get_upgrade_center;
use crate::weapon::{NO_MAX_SHOTS_LIMIT, WeaponLockType, WeaponSetType, WeaponSlotType};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::get_global_data as get_engine_global_data;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::rts::{SCIENCE_INVALID, ScienceType};
use game_engine::common::system::radar::{
    Coord3D as RadarCoord3D, RadarEventType, get_radar_system,
};

include!("types.rs");
include!("handler_move_attack.rs");
include!("handler_build.rs");
include!("handler_group.rs");
include!("handler_special.rs");
include!("handler_guard.rs");
include!("handler_unit.rs");
include!("handler_misc.rs");
include!("handler_dispatch.rs");
include!("processor.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const COMMAND_PROCESSOR_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("handler_build.rs"),
    include_str!("handler_dispatch.rs"),
    include_str!("handler_group.rs"),
    include_str!("handler_guard.rs"),
    include_str!("handler_misc.rs"),
    include_str!("handler_move_attack.rs"),
    include_str!("handler_special.rs"),
    include_str!("handler_unit.rs"),
    include_str!("processor.rs"),
    include_str!("types.rs"),
);
