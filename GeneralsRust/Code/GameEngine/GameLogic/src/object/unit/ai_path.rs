//! UnitAIUpdate inherent pathfinding, queued pathfind, and goal-cell helpers.

#![allow(unused_imports)]

use super::ai_core::{UnitAIUpdate, UnitAiRuntime};
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{dual_world_registry_unavailable, get_unit_arc};
use super::types::*;

/// C++ `Region3D::isInRegionNoZ` used by leftover `computePath` off-map gate.
pub fn leftover_is_in_region_no_z(region: &Region3D, position: &Coord3D) -> bool {
    region.is_in_region_no_z(position)
}

/// Leftover `UnitAIUpdate::should_force_direct_path_for_off_map_start`
/// (C++ `AIUpdateInterface::computePath` AIUpdate.cpp:1663-1671).
pub fn leftover_should_force_direct_path_for_off_map_start(
    start: &Coord3D,
    destination: &Coord3D,
) -> bool {
    let terrain_owner_handle = crate::terrain::get_terrain_logic();
    let Ok(terrain) = terrain_owner_handle.read() else {
        return false;
    };
    let extent = terrain.get_maximum_pathfind_extent();
    if leftover_is_in_region_no_z(&extent, destination) {
        return false;
    }
    !leftover_is_in_region_no_z(&extent, start)
}

/// Leftover `UnitAIUpdate::should_use_direct_path_for_line_passable_non_final_goal`
/// (C++ `AIUpdateInterface::computePath` AIUpdate.cpp:1691-1694).
pub fn leftover_should_use_direct_path_for_line_passable_non_final_goal(
    is_final_goal: bool,
    start: &Coord3D,
    destination: &Coord3D,
    surfaces: u32,
    ignore_obstacle_id: Option<ObjectID>,
) -> bool {
    if is_final_goal {
        return false;
    }
    if surfaces == 0 {
        return false;
    }
    let ai_store = the_ai();
    let Some(ai) = ai_store.read().ok() else {
        return false;
    };
    let Some(pathfinder) = ai.pathfinder() else {
        return false;
    };
    let Ok(pf_guard) = pathfinder.read() else {
        return false;
    };
    pf_guard.is_line_passable_for_surfaces(start, destination, surfaces, ignore_obstacle_id)
}

/// C++ `AIUpdateInterface::computeQuickPath` two-node start+dest
/// (AIUpdate.cpp:1624-1630). Start Z is lifted to dest Z.
pub fn leftover_compute_quick_path_coords(start: &Coord3D, destination: &Coord3D) -> [Coord3D; 2] {
    let mut pos = *start;
    pos.z = destination.z;
    [pos, *destination]
}

impl UnitAIUpdate {
    pub(super) fn set_current_path_snapshot_from_coords(&mut self, path: &[Coord3D]) {
        self.runtime.set_current_path_snapshot_from_coords(path)
    }
    pub(super) fn set_path_from_coords_with_pathfinder(
        &mut self,
        path: &[Coord3D],
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<(), String> {
        self.runtime
            .set_path_from_coords_with_pathfinder(path, pathfinder)
    }
    pub(super) fn remember_result_layers(&mut self, waypoints: &[Coord3D], layers: &[u8]) {
        self.runtime.data.remember_result_layers(waypoints, layers)
    }
    pub(super) fn apply_final_ground_path_layer(
        &mut self,
        waypoints: &[Coord3D],
    ) -> Result<(), String> {
        if !(self.runtime.data.is_final_goal && self.is_doing_ground_movement()) {
            return Ok(());
        }
        let Some(ordinal) = self.runtime.data.installed_path_layers.last().copied() else {
            return Ok(());
        };
        let installed = self.path_with_cpp_final_node(waypoints)?;
        let Some(last) = installed.last().copied() else {
            return Ok(());
        };
        self.update_goal_position(
            &last,
            crate::common::PathfindLayerEnum::from_u32(u32::from(ordinal)),
        )
    }
    pub(super) fn append_current_path_snapshot_goal(&mut self, goal: &Coord3D) {
        self.runtime.append_current_path_snapshot_goal(goal)
    }
    pub(super) fn should_force_direct_path_for_off_map_start(&self, destination: &Coord3D) -> bool {
        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        leftover_should_force_direct_path_for_off_map_start(&guard.get_position(), destination)
    }
    pub(super) fn is_in_region_no_z(region: &Region3D, position: &Coord3D) -> bool {
        leftover_is_in_region_no_z(region, position)
    }
    pub(super) fn should_use_direct_path_for_line_passable_non_final_goal(
        &self,
        destination: &Coord3D,
    ) -> bool {
        if self.runtime.data.is_final_goal {
            return false;
        }

        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let surfaces = {
            let set_surfaces = self.runtime.data.locomotor_set.get_valid_surfaces();
            if set_surfaces != 0 {
                set_surfaces
            } else {
                self.runtime
                    .data
                    .locomotor_set
                    .get_active()
                    .map(|loco| loco.get_legal_surfaces())
                    .unwrap_or(0)
            }
        };
        if surfaces == 0 {
            return false;
        }
        let position = guard.get_position();
        drop(guard);

        let ignore = if self.runtime.data.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.runtime.data.ignore_obstacle_id)
        };
        leftover_should_use_direct_path_for_line_passable_non_final_goal(
            self.runtime.data.is_final_goal,
            &position,
            destination,
            surfaces,
            ignore,
        )
    }
    pub(super) fn has_current_path(&self) -> bool {
        self.runtime.data.current_path_snapshot.is_some()
    }
    pub(super) fn current_locomotor_is_ultra_accurate(&self) -> bool {
        self.runtime.data.current_locomotor_is_ultra_accurate()
    }
    pub(super) fn path_with_cpp_final_node(
        &self,
        path: &[Coord3D],
    ) -> Result<Vec<Coord3D>, String> {
        self.runtime.path_with_cpp_final_node(path)
    }
    pub(super) fn try_install_closest_path_for_invalid_destination(
        &mut self,
        destination: &Coord3D,
    ) -> Result<bool, String> {
        let request = self.build_classic_path_request(*destination, false)?;
        let pathfinder = {
            let ai_store = the_ai();
            let Some(ai) = ai_store.read().ok() else {
                return Ok(false);
            };
            let Some(pathfinder) = ai.pathfinder() else {
                return Ok(false);
            };
            pathfinder
        };
        // Fallback and path installation update goals through the pathfinder.
        // Return the search result before invoking those callbacks.
        let result = {
            let Ok(mut pf_guard) = pathfinder.write() else {
                return Ok(false);
            };
            if pf_guard.valid_movement_position(
                &self.runtime.data.locomotor_set,
                request.is_crusher,
                destination,
                request.ignore_obstacle_id,
            ) {
                return Ok(false);
            }
            if self.has_current_path() {
                None
            } else {
                self.runtime.data.retry_path = true;
                Some(pf_guard.find_closest_path_result(request))
            }
        };
        let Some(result) = result else {
            if self.runtime.data.blocked_and_stuck {
                self.stop_stuck_old_path_after_failed_path()?;
            } else {
                self.runtime.data.path_timestamp = TheGameLogic::get_frame();
                self.runtime.data.blocked_frames = 0;
                self.runtime.data.blocked_and_stuck = false;
            }
            return Ok(true);
        };
        if result.success && !result.waypoints.is_empty() {
            self.set_path_from_coords(&result.waypoints)?;
            self.remember_result_layers(
                &result.waypoints,
                &result
                    .layers
                    .iter()
                    .map(|layer| *layer as u8)
                    .collect::<Vec<_>>(),
            );
            self.apply_final_ground_path_layer(&result.waypoints)?;
            Ok(true)
        } else {
            self.runtime.data.path_timestamp = TheGameLogic::get_frame();
            self.runtime.data.blocked_frames = 0;
            self.runtime.data.blocked_and_stuck = false;
            // C++ computePath returns failure when findClosestPath also
            // returns NULL. Do not turn an unreachable destination into a
            // successful no-op merely because its cell was invalid.
            Ok(false)
        }
    }
    pub(super) fn stop_stuck_old_path_after_failed_path(&mut self) -> Result<(), String> {
        let unit = get_unit_arc(self.runtime.unit_id)
            .ok_or_else(|| "unit no longer available".to_string())?;
        let current_pos = unit
            .read()
            .map_err(|_| "unit lock poisoned".to_string())?
            .get_position();

        let ai_store = the_ai();
        let snapped = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pathfinder| {
                pathfinder
                    .read()
                    .ok()
                    .map(|pf| pf.snap_position(&current_pos))
            })
            .unwrap_or(current_pos);

        self.destroy_path();
        self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
        {
            let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
            guard.target_position = Some(snapped);
            guard.path_index = 0;
            guard.current_speed = 0.0;
            guard.movement_state = MovementState::Idle;
        }
        self.set_locomotor_goal_none();
        self.runtime.data.path_timestamp = TheGameLogic::get_frame();
        self.runtime.data.blocked_frames = 0;
        self.runtime.data.is_blocked = false;
        self.runtime.data.blocked_and_stuck = false;
        Ok(())
    }
    pub(super) fn do_queued_pathfind_now(&mut self) -> Result<bool, String> {
        // Native factory-owned AIs use the same borrowed-pathfinder kernel as
        // the live queue callback. This avoids reacquiring TheAI's Pathfinder
        // while process_queue already holds its write loan. The ownerless
        // compatibility boundary below retains the former standalone path.
        if !self.runtime.data.waiting_for_path {
            return Ok(false);
        }
        if self.runtime.owner.is_some() {
            let pathfinder = the_ai()
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .ok_or_else(|| "pathfinder unavailable for native AI".to_string())?;
            let mut pathfinder = pathfinder
                .write()
                .map_err(|_| "pathfinder lock poisoned".to_string())?;
            return self.do_queued_pathfind_with_pathfinder(pathfinder.pathfinding_system_mut());
        }

        if !self.runtime.data.waiting_for_path {
            return Ok(false);
        }

        self.runtime.data.waiting_for_path = false;
        self.set_queue_for_path_time(0);
        self.runtime.data.retry_path = false;
        let mut destination = self.runtime.data.requested_destination;

        if self.runtime.data.is_safe_path {
            return self.do_queued_safe_pathfind_now();
        }

        if self.runtime.data.is_approach_path && !self.is_doing_ground_movement() {
            self.runtime.data.is_approach_path = false;
        }
        if self.runtime.data.is_approach_path {
            return self.do_queued_approach_pathfind_now(destination);
        }

        if self.runtime.data.is_attack_path {
            if self.try_finish_attack_path_if_already_in_range()? {
                return Ok(true);
            }
            self.prepare_queued_attack_path_fallback()?;
            destination = self.runtime.data.requested_destination;
        }

        // C++ AIUpdate.cpp:1648-1696: ground shortcuts belong to computePath,
        // after the request reaches the pathfind queue, never requestPath.
        if self.should_force_direct_path_for_off_map_start(&destination)
            && self.install_direct_path_from_current_position(&destination)
        {
            return Ok(true);
        }
        if (self.get_current_state_id() == Some(u32::from(AIStateType::FollowExitProductionPath))
            || self.runtime.data.current_command
                == Some(crate::ai::AiCommandType::FollowExitProductionPath))
            && self.runtime.data.can_path_through_units
            && self.install_direct_path_from_current_position(&destination)
        {
            let _ = self.set_can_path_through_units(false);
            return Ok(true);
        }
        if self.should_use_direct_path_for_line_passable_non_final_goal(&destination)
            && self.install_direct_path_from_current_position(&destination)
        {
            return Ok(true);
        }
        if self.try_install_closest_path_for_invalid_destination(&destination)? {
            return Ok(true);
        }

        let request = self.build_classic_path_request(destination, false)?;
        let ai_store = the_ai();
        let path_result = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pathfinder| {
                pathfinder
                    .write()
                    .ok()
                    .map(|mut pf| pf.find_path_result(request.clone()))
            });

        if let Some(result) = path_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                self.remember_result_layers(
                    &result.waypoints,
                    &result
                        .layers
                        .iter()
                        .map(|layer| *layer as u8)
                        .collect::<Vec<_>>(),
                );
                self.apply_final_ground_path_layer(&result.waypoints)?;
                return Ok(true);
            }
        }

        if self.has_current_path() {
            if self.runtime.data.blocked_and_stuck {
                self.stop_stuck_old_path_after_failed_path()?;
            } else {
                self.runtime.data.path_timestamp = TheGameLogic::get_frame();
                self.runtime.data.blocked_frames = 0;
                self.runtime.data.blocked_and_stuck = false;
            }
            return Ok(true);
        }

        self.runtime.data.retry_path = true;
        let ai_store = the_ai();
        let closest_result = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pathfinder| {
                pathfinder
                    .write()
                    .ok()
                    .map(|mut pf| pf.find_closest_path_result(request))
            });
        if let Some(result) = closest_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                self.remember_result_layers(
                    &result.waypoints,
                    &result
                        .layers
                        .iter()
                        .map(|layer| *layer as u8)
                        .collect::<Vec<_>>(),
                );
                self.apply_final_ground_path_layer(&result.waypoints)?;
                return Ok(true);
            }
        }

        self.runtime.data.path_timestamp = TheGameLogic::get_frame();
        self.runtime.data.blocked_frames = 0;
        self.runtime.data.blocked_and_stuck = false;
        Ok(false)
    }
    fn native_path_movement_policy(&self) -> (bool, bool) {
        // C++ AIPathfind.cpp:6189 uses the entire LocomotorSet policy.
        let downhill_only = self.runtime.data.locomotor_set.is_downhill_only();
        let aircraft_goal_only = self.runtime.data.is_aircraft_that_adjusts_destination();
        (downhill_only, aircraft_goal_only)
    }

    pub(super) fn do_queued_pathfind_with_pathfinder(
        &mut self,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<bool, String> {
        if !self.runtime.data.waiting_for_path {
            return Ok(false);
        }
        self.runtime.data.waiting_for_path = false;
        self.set_queue_for_path_time(0);
        self.runtime.data.retry_path = false;
        let mut destination = self.runtime.data.requested_destination;

        if self.runtime.data.is_safe_path {
            return self.do_queued_safe_pathfind_with_pathfinder(pathfinder);
        }
        if self.runtime.data.is_approach_path && !self.is_doing_ground_movement() {
            self.runtime.data.is_approach_path = false;
        }
        if self.runtime.data.is_approach_path {
            return self.do_queued_approach_pathfind_with_pathfinder(destination, pathfinder);
        }
        if self.runtime.data.is_attack_path {
            if self.try_finish_attack_path_with_pathfinder(pathfinder)? {
                return Ok(true);
            }
            self.prepare_queued_attack_path_fallback()?;
            destination = self.runtime.data.requested_destination;
        }

        // C++ AIUpdate.cpp:1648-1696. Native owner identity is resolved from
        // the constructor-bound weak owner; same-ID legacy Units are irrelevant.
        if self.native_should_force_direct_path_for_off_map_start(&destination)
            && self.install_direct_path_from_current_position(&destination)
        {
            return Ok(true);
        }
        if self.native_follow_exit_direct_path(&destination) {
            self.runtime.data.set_can_path_through_units(false).ok();
            return Ok(true);
        }
        if self.native_line_passable_shortcut(&destination, pathfinder)
            && self.install_direct_path_from_current_position(&destination)
        {
            return Ok(true);
        }
        if self.try_install_closest_path_for_invalid_destination_with_pathfinder(
            &destination,
            pathfinder,
        )? {
            return Ok(true);
        }

        let request = self.build_classic_path_request(destination, false)?;
        let (downhill_only, _) = self.native_path_movement_policy();
        let result =
            pathfinder.find_path_with_movement_policy(request.clone(), Some(downhill_only));
        if result.success && !result.waypoints.is_empty() {
            self.install_path_result_with_pathfinder(&result, pathfinder)?;
            return Ok(true);
        }
        if self.has_current_path() {
            if self.runtime.data.blocked_and_stuck {
                self.stop_stuck_old_path_after_failed_path_with_pathfinder(pathfinder)?;
            } else {
                self.runtime.data.path_timestamp = TheGameLogic::get_frame();
                self.runtime.data.blocked_frames = 0;
                self.runtime.data.blocked_and_stuck = false;
            }
            return Ok(true);
        }
        self.runtime.data.retry_path = true;
        let (downhill_only, aircraft_goal_only) = self.native_path_movement_policy();
        let closest = pathfinder.find_closest_path_with_movement_policy(
            request,
            Some(downhill_only),
            Some(aircraft_goal_only),
        );
        if closest.success && !closest.waypoints.is_empty() {
            self.install_path_result_with_pathfinder(&closest, pathfinder)?;
            return Ok(true);
        }
        self.runtime.data.path_timestamp = TheGameLogic::get_frame();
        self.runtime.data.blocked_frames = 0;
        self.runtime.data.blocked_and_stuck = false;
        Ok(false)
    }

    fn install_path_result_with_pathfinder(
        &mut self,
        result: &crate::ai::pathfind_complete::PathResult,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<(), String> {
        self.runtime
            .set_path_from_coords_with_pathfinder(&result.waypoints, pathfinder)?;
        self.runtime.data.remember_result_layers(
            &result.waypoints,
            &result
                .layers
                .iter()
                .map(|layer| *layer as u8)
                .collect::<Vec<_>>(),
        );
        self.apply_final_ground_path_layer_with_pathfinder(&result.waypoints, pathfinder)
    }

    fn apply_final_ground_path_layer_with_pathfinder(
        &mut self,
        waypoints: &[Coord3D],
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<(), String> {
        if !(self.runtime.data.is_final_goal && self.is_doing_ground_movement()) {
            return Ok(());
        }
        let Some(ordinal) = self.runtime.data.installed_path_layers.last().copied() else {
            return Ok(());
        };
        let installed = self.runtime.path_with_cpp_final_node(waypoints)?;
        let Some(last) = installed.last().copied() else {
            return Ok(());
        };
        let layer = crate::common::PathfindLayerEnum::from_u32(u32::from(ordinal));
        self.runtime
            .update_goal_position_with_pathfinder(&last, layer, pathfinder)
    }

    fn native_should_force_direct_path_for_off_map_start(&self, destination: &Coord3D) -> bool {
        let Some(owner) = self.runtime.native_owner() else {
            return false;
        };
        let Ok(owner) = owner.read() else {
            return false;
        };
        leftover_should_force_direct_path_for_off_map_start(owner.get_position(), destination)
    }

    fn native_follow_exit_direct_path(&mut self, destination: &Coord3D) -> bool {
        (self.get_current_state_id() == Some(u32::from(AIStateType::FollowExitProductionPath))
            || self.runtime.data.current_command
                == Some(crate::ai::AiCommandType::FollowExitProductionPath))
            && self.runtime.data.can_path_through_units
            && self
                .runtime
                .install_direct_path_from_current_position(destination)
    }

    fn native_line_passable_shortcut(
        &self,
        destination: &Coord3D,
        pathfinder: &crate::ai::pathfind_complete::PathfindingSystem,
    ) -> bool {
        if self.runtime.data.is_final_goal {
            return false;
        }
        let Some(owner) = self.runtime.native_owner() else {
            return false;
        };
        let Ok(owner) = owner.read() else {
            return false;
        };
        let surfaces = {
            let valid = self.runtime.data.locomotor_set.get_valid_surfaces();
            if valid != 0 {
                valid
            } else {
                self.runtime
                    .data
                    .locomotor_set
                    .get_active()
                    .map(|loco| loco.get_legal_surfaces())
                    .unwrap_or(0)
            }
        };
        if surfaces == 0 {
            return false;
        }
        let ignore = (self.runtime.data.ignore_obstacle_id != INVALID_ID)
            .then_some(self.runtime.data.ignore_obstacle_id);
        pathfinder.is_line_passable_for_surfaces(
            owner.get_position(),
            destination,
            surfaces,
            ignore,
        )
    }

    fn try_install_closest_path_for_invalid_destination_with_pathfinder(
        &mut self,
        destination: &Coord3D,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<bool, String> {
        let request = self.build_classic_path_request(*destination, false)?;
        if pathfinder.valid_movement_position(
            request.surfaces,
            request.is_crusher,
            destination,
            request.ignore_obstacle_id,
        ) {
            return Ok(false);
        }
        if self.has_current_path() {
            if self.runtime.data.blocked_and_stuck {
                self.stop_stuck_old_path_after_failed_path_with_pathfinder(pathfinder)?;
            } else {
                self.runtime.data.path_timestamp = TheGameLogic::get_frame();
                self.runtime.data.blocked_frames = 0;
                self.runtime.data.blocked_and_stuck = false;
            }
            return Ok(true);
        }
        self.runtime.data.retry_path = true;
        let (downhill_only, aircraft_goal_only) = self.native_path_movement_policy();
        let result = pathfinder.find_closest_path_with_movement_policy(
            request,
            Some(downhill_only),
            Some(aircraft_goal_only),
        );
        if result.success && !result.waypoints.is_empty() {
            self.install_path_result_with_pathfinder(&result, pathfinder)?;
            return Ok(true);
        }
        self.runtime.data.path_timestamp = TheGameLogic::get_frame();
        self.runtime.data.blocked_frames = 0;
        self.runtime.data.blocked_and_stuck = false;
        Ok(false)
    }

    fn do_queued_approach_pathfind_with_pathfinder(
        &mut self,
        destination: Coord3D,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<bool, String> {
        self.destroy_path();
        let request = self.build_classic_path_request(destination, false)?;
        let (downhill_only, aircraft_goal_only) = self.native_path_movement_policy();
        let result = pathfinder.find_closest_path_with_movement_policy(
            request,
            Some(downhill_only),
            Some(aircraft_goal_only),
        );
        if result.success && !result.waypoints.is_empty() {
            self.install_path_result_with_pathfinder(&result, pathfinder)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn do_queued_safe_pathfind_with_pathfinder(
        &mut self,
        pathfinder: &mut crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<bool, String> {
        if dual_world_registry_unavailable() {
            return Ok(false);
        }
        self.destroy_path();
        let owner = self
            .runtime
            .native_owner()
            .ok_or_else(|| "unit owner no longer available".to_string())?;
        let owner = owner
            .read()
            .map_err(|_| "unit owner lock poisoned".to_string())?;
        let owner_pos = *owner.get_position();
        let vision_range = owner.get_vision_range();
        drop(owner);
        let repulsor_pos1 = get_legacy_object(self.runtime.data.repulsor1)
            .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()))
            .unwrap_or_else(|| Coord3D::new(-1000.0, -1000.0, 0.0));
        let repulsor_pos2 = get_legacy_object(self.runtime.data.repulsor2)
            .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()))
            .unwrap_or(repulsor_pos1);
        let repulsed_distance = the_ai()
            .read()
            .ok()
            .map(|ai| ai.get_ai_data().repulsed_distance)
            .unwrap_or(0.0);
        let request = self.build_classic_path_request(owner_pos, false)?;
        let (downhill_only, _) = self.native_path_movement_policy();
        let result = pathfinder.find_safe_path_with_movement_policy(
            request,
            &repulsor_pos1,
            &repulsor_pos2,
            vision_range + repulsed_distance,
            Some(downhill_only),
        );
        if result.success && !result.waypoints.is_empty() {
            self.install_path_result_with_pathfinder(&result, pathfinder)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn try_finish_attack_path_with_pathfinder(
        &mut self,
        pathfinder: &crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<bool, String> {
        if dual_world_registry_unavailable() {
            return Ok(false);
        }
        let owner = self
            .runtime
            .native_owner()
            .ok_or_else(|| "unit owner no longer available".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "unit owner lock poisoned".to_string())?;
        let owner_id = owner_guard.get_id();
        let Some((weapon, _)) = owner_guard.get_current_weapon() else {
            return Ok(false);
        };
        let victim = (self.runtime.data.requested_victim_id != INVALID_ID)
            .then(|| get_legacy_object(self.runtime.data.requested_victim_id))
            .flatten();
        let target_pos = victim
            .as_ref()
            .and_then(|victim| victim.read().ok().map(|guard| *guard.get_position()))
            .unwrap_or(self.runtime.data.requested_destination);
        let in_range = if victim.is_some() {
            weapon.is_within_attack_range(
                owner_id,
                Some(self.runtime.data.requested_victim_id),
                None,
            )
        } else {
            weapon.is_within_attack_range(owner_id, None, Some(&target_pos))
        };
        if !in_range {
            return Ok(false);
        }
        let blocked = if self.is_doing_ground_movement() {
            let victim_id = victim
                .as_ref()
                .and_then(|v| v.read().ok().map(|g| g.get_id()));
            pathfinder.is_attack_view_blocked_by_obstacle(
                owner_id,
                owner_guard.get_position(),
                victim_id,
                &target_pos,
            )
        } else {
            false
        };
        drop(owner_guard);
        if blocked {
            return Ok(false);
        }
        self.destroy_path();
        self.runtime.data.path_timestamp = TheGameLogic::get_frame();
        self.runtime.data.blocked_frames = 0;
        self.runtime.data.is_blocked = false;
        self.runtime.data.blocked_and_stuck = false;
        Ok(true)
    }

    fn stop_stuck_old_path_after_failed_path_with_pathfinder(
        &mut self,
        pathfinder: &crate::ai::pathfind_complete::PathfindingSystem,
    ) -> Result<(), String> {
        let owner = self
            .runtime
            .native_owner()
            .ok_or_else(|| "unit owner no longer available".to_string())?;
        let owner = owner
            .read()
            .map_err(|_| "unit owner lock poisoned".to_string())?;
        let current_pos = *owner.get_position();
        let snapped = pathfinder.snap_position(&current_pos);
        drop(owner);
        self.destroy_path();
        self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
        self.runtime.set_locomotor_goal_none();
        self.runtime.data.path_timestamp = TheGameLogic::get_frame();
        self.runtime.data.blocked_frames = 0;
        self.runtime.data.is_blocked = false;
        self.runtime.data.blocked_and_stuck = false;
        self.runtime.data.locomotor_goal_type = 0;
        self.runtime.data.locomotor_goal_data = snapped;
        Ok(())
    }

    pub(super) fn try_finish_attack_path_if_already_in_range(&mut self) -> Result<bool, String> {
        // Wave 258: empty dual-world → Ok(false).

        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let Some(unit) = get_unit_arc(self.runtime.unit_id) else {
            return Ok(false);
        };
        let unit_guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        let owner_id = unit_guard.get_id();
        let owner_base = unit_guard.base_arc();
        let Ok(owner_guard) = owner_base.read() else {
            return Ok(false);
        };
        let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
            return Ok(false);
        };

        let victim = if self.runtime.data.requested_victim_id != INVALID_ID {
            get_legacy_object(self.runtime.data.requested_victim_id)
        } else {
            None
        };
        let target_pos = if let Some(victim) = victim.as_ref() {
            let victim_guard = victim
                .read()
                .map_err(|_| "victim lock poisoned".to_string())?;
            *victim_guard.get_position()
        } else {
            self.runtime.data.requested_destination
        };
        let in_range = if victim.is_some() {
            weapon.is_within_attack_range(
                owner_id,
                Some(self.runtime.data.requested_victim_id),
                None,
            )
        } else {
            weapon.is_within_attack_range(owner_id, None, Some(&target_pos))
        };
        if !in_range {
            return Ok(false);
        }

        let view_blocked = if self.is_doing_ground_movement() {
            the_ai()
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder.read().ok().map(|pf| {
                        if let Some(victim) = victim.as_ref() {
                            match victim.read() {
                                Ok(victim_guard) => pf.is_attack_view_blocked_by_obstacle(
                                    &owner_guard,
                                    owner_guard.get_position(),
                                    Some(&victim_guard),
                                    &target_pos,
                                ),
                                Err(_) => false,
                            }
                        } else {
                            pf.is_attack_view_blocked_by_obstacle(
                                &owner_guard,
                                owner_guard.get_position(),
                                None,
                                &target_pos,
                            )
                        }
                    })
                })
                .unwrap_or(false)
        } else {
            false
        };
        if view_blocked {
            return Ok(false);
        }

        drop(owner_guard);
        drop(unit_guard);
        self.destroy_path();
        self.runtime.data.path_timestamp = TheGameLogic::get_frame();
        self.runtime.data.blocked_frames = 0;
        self.runtime.data.is_blocked = false;
        self.runtime.data.blocked_and_stuck = false;
        Ok(true)
    }
    pub(super) fn prepare_queued_attack_path_fallback(&mut self) -> Result<(), String> {
        // Wave 258: empty dual-world → Ok(()).

        if dual_world_registry_unavailable() {
            return Ok(());
        }

        self.runtime.data.is_attack_path = false;
        if self.runtime.data.requested_victim_id == INVALID_ID {
            return Ok(());
        }

        let Some(victim) = get_legacy_object(self.runtime.data.requested_victim_id) else {
            return Ok(());
        };
        let victim_pos = victim
            .read()
            .map_err(|_| "victim lock poisoned".to_string())?
            .get_position()
            .to_owned();
        self.runtime.data.requested_destination = victim_pos;
        let _ = self.ignore_obstacle(victim.read().ok().map(|g| g.get_id()));
        Ok(())
    }
    pub(super) fn do_queued_approach_pathfind_now(
        &mut self,
        destination: Coord3D,
    ) -> Result<bool, String> {
        self.destroy_path();

        let request = self.build_classic_path_request(destination, false)?;
        let ai_store = the_ai();
        let closest_result = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pathfinder| {
                pathfinder
                    .write()
                    .ok()
                    .map(|mut pf| pf.find_closest_path_result(request))
            });

        if let Some(result) = closest_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                self.remember_result_layers(
                    &result.waypoints,
                    &result
                        .layers
                        .iter()
                        .map(|layer| *layer as u8)
                        .collect::<Vec<_>>(),
                );
                self.apply_final_ground_path_layer(&result.waypoints)?;
                return Ok(true);
            }
        }

        Ok(false)
    }
    pub(super) fn do_queued_safe_pathfind_now(&mut self) -> Result<bool, String> {
        // Wave 258: empty dual-world → Ok(false).

        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        self.destroy_path();

        let unit = get_unit_arc(self.runtime.unit_id)
            .ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        let base_arc = guard.base_arc();
        let obj_guard = base_arc
            .read()
            .map_err(|_| "unit base object lock poisoned".to_string())?;
        let owner_pos = *obj_guard.get_position();
        let owner_vision_range = obj_guard.get_vision_range();
        drop(obj_guard);
        drop(guard);

        let repulsor_pos1 = get_legacy_object(self.runtime.data.repulsor1)
            .and_then(|repulsor| {
                repulsor
                    .read()
                    .ok()
                    .map(|repulsor_guard| *repulsor_guard.get_position())
            })
            .unwrap_or_else(|| Coord3D::new(-1000.0, -1000.0, 0.0));
        let repulsor_pos2 = get_legacy_object(self.runtime.data.repulsor2)
            .and_then(|repulsor| {
                repulsor
                    .read()
                    .ok()
                    .map(|repulsor_guard| *repulsor_guard.get_position())
            })
            .unwrap_or(repulsor_pos1);
        let repulsed_distance = the_ai()
            .read()
            .ok()
            .map(|ai| ai.get_ai_data().repulsed_distance)
            .unwrap_or(0.0);
        let safe_radius = owner_vision_range + repulsed_distance;
        let request = self.build_classic_path_request(owner_pos, false)?;
        let ai_store = the_ai();
        let safe_result = ai_store
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pathfinder| {
                pathfinder.write().ok().map(|mut pf| {
                    pf.find_safe_path_result(request, &repulsor_pos1, &repulsor_pos2, safe_radius)
                })
            });

        if let Some(result) = safe_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                self.remember_result_layers(
                    &result.waypoints,
                    &result
                        .layers
                        .iter()
                        .map(|layer| *layer as u8)
                        .collect::<Vec<_>>(),
                );
                self.apply_final_ground_path_layer(&result.waypoints)?;
                return Ok(true);
            }
        }

        Ok(false)
    }
    pub(super) fn install_direct_path_from_current_position(
        &mut self,
        destination: &Coord3D,
    ) -> bool {
        self.runtime
            .install_direct_path_from_current_position(destination)
    }
    pub(super) fn build_classic_path_request(
        &self,
        destination: Coord3D,
        allow_partial: bool,
    ) -> Result<crate::ai::pathfind_complete::PathRequest, String> {
        self.runtime
            .build_classic_path_request(destination, allow_partial)
    }
    pub(super) fn queue_path_request_now(&self, destination: Coord3D) -> Result<(), String> {
        self.runtime.queue_path_request_now(destination)
    }
    pub(super) fn clip_goal_position(
        &self,
        owner: &Arc<RwLock<crate::object::Object>>,
        mut pos: Coord3D,
        cmd_source: CommandSourceType,
    ) -> Coord3D {
        if cmd_source != CommandSourceType::FromPlayer {
            return pos;
        }

        let mut fudge = PATHFIND_CELL_SIZE_F * 0.5;
        if let Ok(object) = owner.read() {
            if object.is_kind_of(KindOf::Aircraft) && object.is_significantly_above_terrain() {
                let preferred = self
                    .runtime
                    .data
                    .locomotor_set
                    .get_active()
                    .map(|loc| loc.preferred_height)
                    .unwrap_or(0.0);
                if preferred > fudge {
                    fudge = preferred;
                }
            }
        }

        if let Ok(terrain_guard) = crate::terrain::get_terrain_logic().read() {
            let extent = terrain_guard.get_maximum_pathfind_extent();
            let min_x = extent.lo.x + fudge;
            let max_x = extent.hi.x - fudge;
            let min_y = extent.lo.y + fudge;
            let max_y = extent.hi.y - fudge;
            pos.x = pos.x.clamp(min_x, max_x);
            pos.y = pos.y.clamp(min_y, max_y);
        }

        pos
    }
    pub(super) fn compute_pathfind_radius_and_center(unit: &Unit) -> (i32, bool) {
        UnitAiRuntime::compute_pathfind_radius_and_center(unit)
    }
    pub(super) fn compute_goal_cell(pos: &Coord3D, center_in_cell: bool) -> ICoord2D {
        UnitAiRuntime::compute_goal_cell(pos, center_in_cell)
    }
    pub(super) fn remove_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.runtime
            .remove_goal_cells(pathfinder, unit_id, radius, center_in_cell)
    }
    pub(super) fn remove_stored_pathfinder_goal(&mut self) {
        self.runtime.remove_stored_pathfinder_goal()
    }
    pub(super) fn update_ground_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        layer: ClassicPathLayer,
        radius: i32,
        center_in_cell: bool,
        interacts_with_bridge_end: bool,
    ) {
        self.runtime.update_ground_goal_cells(
            pathfinder,
            unit_id,
            new_cell,
            layer,
            radius,
            center_in_cell,
            interacts_with_bridge_end,
        )
    }
    pub(super) fn update_aircraft_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.runtime.update_aircraft_goal_cells(
            pathfinder,
            unit_id,
            new_cell,
            radius,
            center_in_cell,
        )
    }
    pub(super) fn has_valid_locomotor_surfaces(&self) -> bool {
        self.runtime.has_valid_locomotor_surfaces()
    }
    pub(super) fn safe_path_search_distance(vision_range: Real, repulsed_distance: Real) -> Real {
        vision_range + repulsed_distance
    }
    pub(super) fn current_path_extra_distance(&self) -> Real {
        self.runtime.data.path_extra_distance
    }
    pub(super) fn finish_completed_movement_like_cpp(&mut self) {
        if !self.runtime.data.movement_complete {
            return;
        }

        self.set_queue_for_path_time(0);
        self.destroy_path();
        self.set_locomotor_goal_none();

        // C++ friend_endingMove clears m_isMoving (AIUpdate.cpp:2030-2033),
        // and update consumes completion after clearing path/goal (1018-1044).
        // Keep the Rust movement companion inactive too: idle/movement queries
        // read it. Do not stop_movement(), which also discards future waypoints.
        let base = get_unit_arc(self.runtime.unit_id).and_then(|unit| {
            let mut guard = unit.write().ok()?;
            if guard.is_movement_active() {
                guard.movement_state = MovementState::Idle;
            }
            guard.target_position = None;
            guard.current_speed = 0.0;
            Some(guard.base_arc())
        });
        if let Some(base) = base {
            if let Ok(mut object) = base.write() {
                object.clear_model_condition_state(ModelConditionFlags::MOVING);
            }
        }

        if let Some((pos, radius, layer, id)) =
            get_unit_arc(self.runtime.unit_id).and_then(|unit| {
                let guard = unit.read().ok()?;
                let base = guard.base_arc();
                let object = base.read().ok()?;
                Some((
                    *object.get_position(),
                    object.get_geometry_info().get_bounding_circle_radius(),
                    object.get_layer(),
                    object.get_id(),
                ))
            })
        {
            let mut goal = Coord3D::new(0.0, 0.0, 0.0);
            let found = the_ai().read().ok().and_then(|ai| {
                let pf = ai.pathfinder()?;
                let pf = pf.read().ok()?;
                if !pf.goal_position_for_unit(id, radius, &mut goal) {
                    return None;
                }
                let dx = goal.x - pos.x;
                let dy = goal.y - pos.y;
                let cell = crate::ai::pathfind_astar::PATHFIND_CELL_SIZE_F;
                if dx * dx + dy * dy >= cell * cell {
                    goal = pf.snap_position_for_radius(&pos, radius);
                }
                Some(goal)
            });
            if let Some(goal) = found {
                self.runtime.data.final_position = goal;
                self.runtime.data.do_final_position = false;
                let _ = crate::ai::pathfind::update_goal_for_object(
                    id,
                    &goal,
                    crate::ai::pathfind::PathfindLayerEnum::from_u32(layer as u32),
                );
            }
        }

        self.runtime.data.movement_complete = false;
        self.runtime.data.ignore_obstacle_id = INVALID_ID;
    }
}
