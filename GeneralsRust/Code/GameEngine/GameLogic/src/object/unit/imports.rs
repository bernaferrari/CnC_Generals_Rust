//! Shared imports for split `impl Unit` / `impl UnitAIUpdate` sibling modules.
//!
//! `use super::*` in children only sees public parent items. This module
//! re-exports the crate-imports those methods need.

#![allow(unused_imports)]

pub(super) use crate::action_manager::{ActionManager, TheActionManager};
pub(super) use crate::ai::dock::AIDockMachine;
pub(super) use crate::ai::object_registry::get_legacy_object;
pub(super) use crate::ai::pathfind::PathfindLayerEnum;
pub(super) use crate::ai::pathfind::{Path as AiPath, PathfindLayerEnum as AiPathLayer};
pub(super) use crate::ai::pathfind_astar::PathfindLayerEnum as ClassicPathLayer;
pub(super) use crate::ai::pathfinding_system::PathfindLayerEnum as PfLayer;
pub(super) use crate::ai::states::{AIStateMachine, AIStateType};
pub(super) use crate::ai::turret::{TurretAI, TurretStateMachine};
pub(super) use crate::ai::{
    mood_matrix_adjustment, mood_matrix_parameters, search_qualifiers, AiCommandInterface,
    MoodMatrixAction, THE_AI,
};
pub(super) use crate::attack::CanAttackResult;
pub(super) use crate::common::ObjectID;
pub(super) use crate::common::VeterancyLevel;
pub(super) use crate::common::*;
pub(super) use crate::damage::{DamageInfo, DamageType, DeathType};
pub(super) use crate::helpers::{
    get_game_logic_random_value_real, FindPositionOptions, TheFXListStore, TheGameLogic,
    ThePartitionManager, TheTerrainLogic,
};
pub(super) use crate::locomotor::{
    update_movement_with_pathfinding, BodyDamageType, Locomotor, LocomotorAppearance, LocomotorSet,
    LocomotorSurfaceTypeMask, PathFollowingState, SURFACE_AIR,
};
pub(super) use crate::modules::{
    AIAttitudeType, AIUpdateInterface, AIUpdateInterfaceExt, ContainModuleInterfaceExt,
    PhysicsBehaviorExt, FAST_AS_POSSIBLE, UPDATE_SLEEP_NONE,
};
pub(super) use crate::object::draw::TerrainDecalType;
pub(super) use crate::object::object_factory::{get_object_factory, GameObjectInstance};
pub(super) use crate::object::registry::OBJECT_REGISTRY;
pub(super) use crate::object::update::ai_update_interface::GuardTargetType;
pub(super) use crate::object::update::{
    AssaultTransportAIUpdate, DeliverPayloadAIUpdate, DeployStyleAIUpdate, HackInternetAIUpdate,
    RailedTransportAIUpdate, TransportAIUpdate, WanderAIUpdate,
};
pub(super) use crate::object::update::{ChinookAIUpdate, DozerAIUpdate, JetAIUpdate, TurretAIData};
pub(super) use crate::object::{Object, TriggerInfo};
pub(super) use crate::path::{PathfindMap, Waypoint, PATHFIND_CELL_SIZE_F, PATHFIND_CLOSE_ENOUGH};
pub(super) use crate::physics::GRAVITY;
pub(super) use crate::player::PlayerIndex;
#[cfg(feature = "allow_surrender")]
pub(super) use crate::pow_truck_ai_update::{POWTruckAIUpdate, POWTruckAIUpdateData};
pub(super) use crate::supply_system::{SupplyTruckAIUpdate, WorkerAIUpdate};
pub(super) use crate::team::Team;
pub(super) use crate::upgrade::center::get_upgrade_center;
pub(super) use crate::weapon::{WeaponAntiMask, WeaponChoiceCriteria, WeaponSet, WeaponSlotType};
pub(super) use game_engine::common::system::{Snapshotable, Xfer};
pub(super) use log::error;
pub(super) use once_cell::sync::Lazy;
pub(super) use std::collections::HashMap;
pub(super) use std::sync::RwLock as StdRwLock;
pub(super) use std::sync::{Arc, Mutex, RwLock, Weak};
