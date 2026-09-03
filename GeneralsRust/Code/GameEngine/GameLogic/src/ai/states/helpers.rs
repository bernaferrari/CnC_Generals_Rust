#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
use super::follow_path::*;
use super::follow_path_core::*;
use super::guard::*;
use super::hack::*;
use super::hunt::*;
use super::idle::*;
use super::r#move::*;
use super::rappel::*;
use super::state_machine::*;
use super::types::*;
use super::wait_busy::*;
use super::wander_panic::*;
use super::waypoint::*;
use super::*;

use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::dock::AIDockMachine;
use crate::ai::group::AIGroup;
use crate::ai::guard::{AIGuardMachine, GuardStateType};
use crate::ai::guard_retaliate::AIGuardRetaliateMachine;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::pathfind::Path;
use crate::ai::squad::Squad;
use crate::ai::tn_guard::{AITNGuardMachine, TNGuardStateType};
use crate::ai::{
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter, the_ai,
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::compat::{ClassicState, legacy_transition, register_classic_state};
use crate::control_bar::get_control_bar_bridge;
use crate::damage::DamageInfo;
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, get_game_logic_random_value};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE, PhysicsBehaviorExt,
};
use crate::object::production::AIFreeToExitType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::GRAVITY;
use crate::player::PlayerType;
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::engine::get_script_engine;
use crate::state_machine::*;
use crate::team::{Team, TeamID, TheTeamFactory};
use crate::terrain::get_terrain_logic;
use crate::waypoint::{Waypoint, WaypointId};
use crate::weapon::{
    NO_MAX_SHOTS_LIMIT, Weapon, WeaponChoiceCriteria, WeaponLockType, WeaponSlotType, WeaponStatus,
};
use game_engine::common::system::{GeometryType, Snapshotable, Xfer};

use crate::common::INVALID_ID;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Wave 257: host-only path has no dual-world factory objects.
#[inline]
pub(crate) fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

pub(crate) fn is_cliff_at(pos: &Coord3D) -> bool {
    get_terrain_logic()
        .read()
        .map(|terrain| terrain.is_cliff_cell(pos.x, pos.y))
        .unwrap_or(false)
}

pub(crate) fn normalize_angle(angle: Real) -> Real {
    let mut a = angle;
    let two_pi = std::f32::consts::PI * 2.0;
    while a > std::f32::consts::PI {
        a -= two_pi;
    }
    while a < -std::f32::consts::PI {
        a += two_pi;
    }
    a
}

pub(crate) fn is_in_region_no_z(region: &Region3D, position: &Coord3D) -> bool {
    position.x >= region.lo.x
        && position.x <= region.hi.x
        && position.y >= region.lo.y
        && position.y <= region.hi.y
}

pub(crate) fn is_point_on_wall(pos: &Coord3D) -> bool {
    let cell_pad = PATHFIND_CELL_SIZE_F * 0.5;
    // Host path: empty dual-world registry residual.
    if OBJECT_REGISTRY.is_empty() {
        return false;
    }
    for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
        if let Some(obj) = OBJECT_REGISTRY.get_object(obj_id) {
            if let Ok(obj_guard) = obj.read() {
                if !obj_guard.is_any_kind_of(&[KindOf::Barrier]) {
                    continue;
                }
                let wall_pos = obj_guard.get_position();
                let geom = obj_guard.get_template().get_template_geometry_info();
                let radius = geom.get_bounding_circle_radius();
                let dx = wall_pos.x - pos.x;
                let dy = wall_pos.y - pos.y;
                let dist_sq = dx * dx + dy * dy;
                let allowed = radius + cell_pad;
                if dist_sq <= allowed * allowed {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn get_wall_height() -> Real {
    the_ai()
        .read()
        .ok()
        .and_then(|ai| ai.get_ai_data().read().ok().map(|data| data.wall_height))
        .unwrap_or(0.0)
}

pub(crate) fn resolve_waypoint_by_id(id: WaypointId) -> Option<Arc<Waypoint>> {
    let terrain = get_terrain_logic().read().ok()?;
    let waypoint = terrain.get_waypoint_by_id(id)?;
    Some(Arc::new(Waypoint::from_terrain(waypoint)))
}
