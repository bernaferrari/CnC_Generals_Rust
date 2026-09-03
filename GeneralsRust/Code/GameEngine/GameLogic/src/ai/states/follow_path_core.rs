#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
use super::follow_path::*;
use super::guard::*;
use super::hack::*;
use super::helpers::*;
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

#[derive(Debug)]
pub(crate) struct FollowWaypointPathCore {
    pub(crate) move_as_group: bool,
    pub(crate) is_follow_waypoint_path_state: bool,
    pub(crate) current_waypoint: Option<Arc<Waypoint>>,
    pub(crate) prior_waypoint: Option<Arc<Waypoint>>,
    pub(crate) group_offset: Coord2D,
    pub(crate) angle: Real,
    pub(crate) frames_sleeping: UnsignedInt,
    pub(crate) append_goal_position: bool,
    pub(crate) goal_position: Coord3D,
    pub(crate) goal_layer: PathfindLayerEnum,
}

impl FollowWaypointPathCore {
    pub(crate) fn new(move_as_group: bool, is_follow_waypoint_path_state: bool) -> Self {
        Self {
            move_as_group,
            is_follow_waypoint_path_state,
            current_waypoint: None,
            prior_waypoint: None,
            group_offset: Coord2D::new(0.0, 0.0),
            angle: 0.0,
            frames_sleeping: 0,
            append_goal_position: false,
            goal_position: Coord3D::origin(),
            goal_layer: PathfindLayerEnum::Ground,
        }
    }

    pub(crate) fn has_next_waypoint(&self) -> bool {
        self.current_waypoint
            .as_ref()
            .map(|waypoint| waypoint.get_num_links() > 0)
            .unwrap_or(false)
    }

    pub(crate) fn get_next_waypoint(&mut self, state: &State) -> Option<Arc<Waypoint>> {
        let current = self.current_waypoint.as_ref()?;
        let link_count = current.get_num_links();
        if link_count == 0 {
            self.prior_waypoint = self.current_waypoint.clone();
            return None;
        }

        let which = get_game_logic_random_value(0, (link_count - 1) as i32) as usize;
        let next_id = current.get_link(which)?;
        self.prior_waypoint = self.current_waypoint.clone();
        if let Ok(machine) = state.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(current.position);
            }
        }
        resolve_waypoint_by_id(next_id)
    }

    pub(crate) fn calc_extra_path_distance(&self) -> Real {
        let mut extra = PATHFIND_CELL_SIZE_F / 10.0;
        let mut cur = self.current_waypoint.clone();
        let mut limit = 5;
        while let Some(way) = cur.take() {
            if limit == 0 {
                break;
            }
            limit -= 1;
            if way.get_num_links() == 0 {
                break;
            }
            let next_id = way.get_link(0);
            let Some(next_id) = next_id else {
                break;
            };
            let next = resolve_waypoint_by_id(next_id);
            let Some(next_way) = next else {
                break;
            };
            let dx = next_way.position.x - way.position.x;
            let dy = next_way.position.y - way.position.y;
            extra += (dx * dx + dy * dy).sqrt();
            cur = Some(next_way);
        }
        extra
    }

    pub(crate) fn compute_goal(
        &mut self,
        state: &State,
        owner: &Object,
        ai: &mut dyn AIUpdateInterface,
        use_group_offsets: bool,
    ) -> Result<(), String> {
        let Some(current_waypoint) = self.current_waypoint.as_ref() else {
            return Ok(());
        };

        let mut dest = current_waypoint.position;
        self.goal_layer = PathfindLayerEnum::Ground;
        if is_point_on_wall(&dest) {
            dest.z = get_wall_height();
            self.goal_layer = PathfindLayerEnum::Wall;
        }
        self.goal_position = dest;

        if use_group_offsets {
            self.goal_position.x += self.group_offset.x;
            self.goal_position.y += self.group_offset.y;
        }

        if let Ok(terrain_guard) = get_terrain_logic().read() {
            if self.goal_layer == PathfindLayerEnum::Wall {
                if !is_point_on_wall(&self.goal_position) {
                    self.goal_position = dest;
                }
                self.goal_position.z = get_wall_height();
            } else {
                self.goal_layer = PathfindLayerEnum::Ground;
                self.goal_position.z = terrain_guard.get_ground_height(
                    self.goal_position.x,
                    self.goal_position.y,
                    None,
                );
            }
            let extent = terrain_guard.get_maximum_pathfind_extent();
            if is_in_region_no_z(&extent, &dest) && !is_in_region_no_z(&extent, &self.goal_position)
            {
                if self.goal_position.x < extent.lo.x + PATHFIND_CELL_SIZE_F {
                    self.goal_position.x = extent.lo.x + PATHFIND_CELL_SIZE_F;
                }
                if self.goal_position.y < extent.lo.y + PATHFIND_CELL_SIZE_F {
                    self.goal_position.y = extent.lo.y + PATHFIND_CELL_SIZE_F;
                }
                if self.goal_position.x > extent.hi.x - PATHFIND_CELL_SIZE_F {
                    self.goal_position.x = extent.hi.x - PATHFIND_CELL_SIZE_F;
                }
                if self.goal_position.y > extent.hi.y - PATHFIND_CELL_SIZE_F {
                    self.goal_position.y = extent.hi.y - PATHFIND_CELL_SIZE_F;
                }
            }
            if !is_in_region_no_z(&extent, &self.goal_position) {
                ai.set_adjusts_destination(false);
                let _ = ai.set_allow_invalid_position(true);
                self.append_goal_position = true;
            }
        }

        if self.has_next_waypoint() {
            ai.set_adjusts_destination(false);
        } else {
            ai.set_adjusts_destination(true);
            if owner.is_kind_of(KindOf::Projectile) {
                if let Some(locomotor) = ai.get_cur_locomotor() {
                    if let Ok(mut guard) = locomotor.lock() {
                        guard.set_precise_z_pos(true);
                    }
                }
            }
        }

        ai.set_path_extra_distance(self.calc_extra_path_distance())
            .map_err(|e| e.to_string())?;
        if let Ok(machine) = state.get_machine() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_position(self.goal_position);
            }
        }
        let _dest = self.goal_position;
        Ok(())
    }

    pub(crate) fn compute_path(&mut self, ai: &mut dyn AIUpdateInterface) -> Result<(), String> {
        ai.set_movement_target(&self.goal_position).map_err(|err| {
            format!(
                "FollowWaypointPathState set_movement_target failed: {}",
                err
            )
        })
    }

    pub(crate) fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut group_offset_x = self.group_offset.x;
        let mut group_offset_y = self.group_offset.y;
        let mut angle = self.angle;
        let mut frames_sleeping = self.frames_sleeping;
        let mut append_goal_position = self.append_goal_position;
        let mut goal_position_x = self.goal_position.x;
        let mut goal_position_y = self.goal_position.y;
        let mut goal_position_z = self.goal_position.z;
        let mut current_id: WaypointId = self
            .current_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        let mut prior_id: WaypointId = self
            .prior_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_real(&mut group_offset_x)
            .map_err(|e| format!("Failed to crc group_offset.x: {:?}", e))?;
        xfer.xfer_real(&mut group_offset_y)
            .map_err(|e| format!("Failed to crc group_offset.y: {:?}", e))?;
        xfer.xfer_real(&mut angle)
            .map_err(|e| format!("Failed to crc angle: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut frames_sleeping)
            .map_err(|e| format!("Failed to crc frames_sleeping: {:?}", e))?;
        xfer.xfer_bool(&mut append_goal_position)
            .map_err(|e| format!("Failed to crc append_goal_position: {:?}", e))?;
        xfer.xfer_real(&mut goal_position_x)
            .map_err(|e| format!("Failed to crc goal_position.x: {:?}", e))?;
        xfer.xfer_real(&mut goal_position_y)
            .map_err(|e| format!("Failed to crc goal_position.y: {:?}", e))?;
        xfer.xfer_real(&mut goal_position_z)
            .map_err(|e| format!("Failed to crc goal_position.z: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut current_id)
            .map_err(|e| format!("Failed to crc current waypoint id: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut prior_id)
            .map_err(|e| format!("Failed to crc prior waypoint id: {:?}", e))?;
        Ok(())
    }

    pub(crate) fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        xfer.xfer_real(&mut self.group_offset.x)
            .map_err(|e| format!("Failed to xfer group_offset.x: {:?}", e))?;
        xfer.xfer_real(&mut self.group_offset.y)
            .map_err(|e| format!("Failed to xfer group_offset.y: {:?}", e))?;
        xfer.xfer_real(&mut self.angle)
            .map_err(|e| format!("Failed to xfer angle: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.frames_sleeping)
            .map_err(|e| format!("Failed to xfer frames_sleeping: {:?}", e))?;
        xfer.xfer_bool(&mut self.append_goal_position)
            .map_err(|e| format!("Failed to xfer append_goal_position: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.x)
            .map_err(|e| format!("Failed to xfer goal_position.x: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.y)
            .map_err(|e| format!("Failed to xfer goal_position.y: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.z)
            .map_err(|e| format!("Failed to xfer goal_position.z: {:?}", e))?;

        let mut current_id: WaypointId = self
            .current_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut current_id)
            .map_err(|e| format!("Failed to xfer current waypoint id: {:?}", e))?;
        if xfer.is_loading() {
            self.current_waypoint = if current_id == INVALID_ID {
                None
            } else {
                resolve_waypoint_by_id(current_id)
            };
        }

        let mut prior_id: WaypointId = self
            .prior_waypoint
            .as_ref()
            .map(|w| w.id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_unsigned_int(&mut prior_id)
            .map_err(|e| format!("Failed to xfer prior waypoint id: {:?}", e))?;
        if xfer.is_loading() {
            self.prior_waypoint = if prior_id == INVALID_ID {
                None
            } else {
                resolve_waypoint_by_id(prior_id)
            };
        }

        Ok(())
    }
}
