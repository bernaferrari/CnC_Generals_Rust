//! Supply and Economy System
//!
//! Complete implementation of the C&C Generals supply collection and economy system.
//! Ports the C++ system from:
//! - SupplyCenterDockUpdate.cpp
//! - SupplyWarehouseDockUpdate.cpp
//! - SupplyTruckAIUpdate.cpp
//! - ResourceGatheringManager.cpp
//! - AutoDepositUpdate.cpp
//! - Player.cpp (money management)

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

use crate::action_manager::ActionManager;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{
    AsciiString, Coord3D as LogicCoord3D, KindOf, ModelConditionFlags, SECONDS_PER_LOGICFRAME_REAL,
};
use crate::compat::{ClassicState, register_classic_state};
use crate::helpers::{
    FindPositionOptions, TheAudio, TheGameLogic, TheGameText, TheInGameUI, ThePartitionManager,
};
use crate::modules::{
    AIUpdateInterface, BodyModuleInterfaceExt, SupplyTruckAIInterface, WorkerAIUpdateInterface,
};
use crate::object::Object;
use crate::object::drawable::DrawableExt;
use crate::object::production::get_construction_manager;
use crate::player::player_list;
use crate::resource;
use crate::state_machine::{
    State, StateConditionInfo, StateExitType, StateImplementation, StateMachine, StateReturnType,
    StateTransitionUserData,
};
use game_engine::common::system::snapshot::Snapshotable;
use game_engine::common::system::xfer::{Xfer, XferVersion};

include!("types.rs");
include!("money.rs");
include!("gathering.rs");
include!("warehouse.rs");
include!("center.rs");
include!("truck_states.rs");
include!("truck_ai.rs");
include!("auto_deposit.rs");
include!("piles.rs");
include!("worker.rs");
include!("player_snapshot.rs");
include!("tests.rs");

/// Concatenated live sources for residual `include_str!` scans.
pub const SUPPLY_SYSTEM_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("auto_deposit.rs"),
    include_str!("center.rs"),
    include_str!("gathering.rs"),
    include_str!("money.rs"),
    include_str!("piles.rs"),
    include_str!("player_snapshot.rs"),
    include_str!("truck_ai.rs"),
    include_str!("truck_states.rs"),
    include_str!("types.rs"),
    include_str!("warehouse.rs"),
    include_str!("worker.rs"),
);
