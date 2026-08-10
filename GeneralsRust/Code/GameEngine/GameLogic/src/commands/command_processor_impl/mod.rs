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
    get_command_queue_manager, CommandExecutionState, CommandPriority, QueuedCommand,
};
use super::rts_command::{RtsCommand, RtsCommandValidator};
use crate::action_manager::TheActionManager;
use crate::commands::get_selection_manager;
use crate::common::{
    audio::AudioEventRts, AsciiString, Bool, CommandSourceType, Coord3D, DrawableID, EvaEvent,
    FormationID, ICoord2D, IRegion2D, Int, KindOf, ObjectID, ObjectStatusTypes, PlayerMaskType,
    Real, Relationship, UnsignedInt,
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
use crate::object::object_factory::{get_object_factory, GameObjectInstance};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_manager::get_object_manager;
use crate::player::player_list;
use crate::system::beacon_manager::{get_beacon_manager, BeaconManager};
use crate::upgrade::center::THE_UPGRADE_CENTER;
use crate::weapon::{WeaponLockType, WeaponSetType, WeaponSlotType, NO_MAX_SHOTS_LIMIT};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::get_global_data as get_engine_global_data;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::rts::{ScienceType, SCIENCE_INVALID};
use game_engine::common::system::radar::{
    get_radar_system, Coord3D as RadarCoord3D, RadarEventType,
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
