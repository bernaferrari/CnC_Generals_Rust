//! UnitAIUpdate inherent pathfinding, queued pathfind, and goal-cell helpers.

#![allow(unused_imports)]

use super::ai_core::UnitAIUpdate;
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{dual_world_registry_unavailable, get_unit_arc};
use super::types::*;

/// C++ `Region3D::isInRegionNoZ` used by leftover `computePath` off-map gate.
pub fn leftover_is_in_region_no_z(region: &Region3D, position: &Coord3D) -> bool {
    position.x >= region.lo.x
        && position.x <= region.hi.x
        && position.y >= region.lo.y
        && position.y <= region.hi.y
}

/// Leftover `UnitAIUpdate::should_force_direct_path_for_off_map_start`
/// (C++ `AIUpdateInterface::computePath` AIUpdate.cpp:1663-1671).
pub fn leftover_should_force_direct_path_for_off_map_start(
    start: &Coord3D,
    destination: &Coord3D,
) -> bool {
    let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
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
    let ai_store = the_ai();let Some(ai) = ai_store.read().ok() else {
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
        let mut snapshot = AiPath::new();
        for pos in path {
            snapshot.append_node(pos, AiPathLayer::Ground);
        }
        self.current_path_snapshot = Some(snapshot);
    }
    pub(super) fn append_current_path_snapshot_goal(&mut self, goal: &Coord3D) {
        match self.current_path_snapshot.as_mut() {
            Some(path) => path.append_node(goal, AiPathLayer::Ground),
            None => self.set_current_path_snapshot_from_coords(&[*goal]),
        }
    }
    pub(super) fn should_force_direct_path_for_off_map_start(&self, destination: &Coord3D) -> bool {
        let Some(unit) = get_unit_arc(self.unit_id) else {
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
        if self.is_final_goal {
            return false;
        }

        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let surfaces = {
            let set_surfaces = guard.locomotor_set.get_valid_surfaces();
            if set_surfaces != 0 {
                set_surfaces
            } else {
                guard.get_locomotor_surface_mask().unwrap_or(0)
            }
        };
        if surfaces == 0 {
            return false;
        }
        let position = guard.get_position();
        drop(guard);

        let ignore = if self.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.ignore_obstacle_id)
        };
        leftover_should_use_direct_path_for_line_passable_non_final_goal(
            self.is_final_goal,
            &position,
            destination,
            surfaces,
            ignore,
        )
    }
    pub(super) fn has_current_path(&self) -> bool {
        if self.current_path_snapshot.is_some() {
            return true;
        }
        get_unit_arc(self.unit_id)
            .and_then(|unit| unit.read().ok().map(|guard| guard.current_path.is_some()))
            .unwrap_or(false)
    }
    pub(super) fn current_locomotor_is_ultra_accurate(&self) -> bool {
        get_unit_arc(self.unit_id)
            .and_then(|unit| {
                unit.read().ok().and_then(|guard| {
                    guard.current_locomotor.as_ref().and_then(|locomotor| {
                        locomotor.lock().ok().map(|loc| loc.is_ultra_accurate())
                    })
                })
            })
            .unwrap_or(false)
    }
    pub(super) fn path_with_cpp_final_node(
        &self,
        path: &[Coord3D],
    ) -> Result<Vec<Coord3D>, String> {
        if path.is_empty() {
            return Err("set_path_from_coords missing path points".to_string());
        }

        let mut installed_path = path.to_vec();
        if self.current_locomotor_is_ultra_accurate() {
            if let Some(last) = installed_path.last_mut() {
                *last = self.requested_destination;
            }
        }
        Ok(installed_path)
    }
    pub(super) fn try_install_closest_path_for_invalid_destination(
        &mut self,
        destination: &Coord3D,
    ) -> Result<bool, String> {
        let request = self.build_classic_path_request(*destination, false)?;
        let locomotor_set = get_unit_arc(self.unit_id)
            .and_then(|unit| unit.read().ok().map(|guard| guard.locomotor_set.clone()))
            .ok_or_else(|| "unit no longer available".to_string())?;
        let ai_store = the_ai();let Some(ai) = ai_store.read().ok() else {
            return Ok(false);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Ok(false);
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return Ok(false);
        };

        if pf_guard.valid_movement_position(
            &locomotor_set,
            request.is_crusher,
            destination,
            request.ignore_obstacle_id,
        ) {
            return Ok(false);
        }

        if self.has_current_path() {
            if self.blocked_and_stuck {
                self.stop_stuck_old_path_after_failed_path()?;
            } else {
                self.path_timestamp = TheGameLogic::get_frame();
                self.blocked_frames = 0;
                self.blocked_and_stuck = false;
            }
            return Ok(true);
        }

        self.retry_path = true;
        let result = pf_guard.find_closest_path_result(request);
        if result.success && !result.waypoints.is_empty() {
            self.set_path_from_coords(&result.waypoints)?;
            Ok(true)
        } else {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.blocked_and_stuck = false;
            // C++ computePath returns failure when findClosestPath also
            // returns NULL. Do not turn an unreachable destination into a
            // successful no-op merely because its cell was invalid.
            Ok(false)
        }
    }
    pub(super) fn stop_stuck_old_path_after_failed_path(&mut self) -> Result<(), String> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let current_pos = unit
            .read()
            .map_err(|_| "unit lock poisoned".to_string())?
            .get_position();

        let ai_store = the_ai();let snapped = ai_store
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
        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked = false;
        self.blocked_and_stuck = false;
        Ok(())
    }
    pub(super) fn do_queued_pathfind_now(&mut self) -> Result<bool, String> {
        if !self.waiting_for_path {
            return Ok(false);
        }

        self.waiting_for_path = false;
        self.set_queue_for_path_time(0);
        self.retry_path = false;
        let mut destination = self.requested_destination;

        if self.is_safe_path {
            return self.do_queued_safe_pathfind_now();
        }

        if self.is_approach_path && !self.is_doing_ground_movement() {
            self.is_approach_path = false;
        }
        if self.is_approach_path {
            return self.do_queued_approach_pathfind_now(destination);
        }

        if self.is_attack_path {
            if self.try_finish_attack_path_if_already_in_range()? {
                return Ok(true);
            }
            self.prepare_queued_attack_path_fallback()?;
            destination = self.requested_destination;
        }

        if self.try_install_closest_path_for_invalid_destination(&destination)? {
            return Ok(true);
        }

        let request = self.build_classic_path_request(destination, false)?;
        let ai_store = the_ai();let path_result =
            ai_store
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder
                        .read()
                        .ok()
                        .map(|pf| pf.find_path_result(request.clone()))
                });

        if let Some(result) = path_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                return Ok(true);
            }
        }

        if self.has_current_path() {
            if self.blocked_and_stuck {
                self.stop_stuck_old_path_after_failed_path()?;
            } else {
                self.path_timestamp = TheGameLogic::get_frame();
                self.blocked_frames = 0;
                self.blocked_and_stuck = false;
            }
            return Ok(true);
        }

        self.retry_path = true;
        let ai_store = the_ai();let closest_result =
            ai_store
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder
                        .read()
                        .ok()
                        .map(|pf| pf.find_closest_path_result(request))
                });
        if let Some(result) = closest_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                return Ok(true);
            }
        }

        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        Ok(false)
    }
    pub(super) fn try_finish_attack_path_if_already_in_range(&mut self) -> Result<bool, String> {
        // Wave 258: empty dual-world → Ok(false).

        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let Some(unit) = get_unit_arc(self.unit_id) else {
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

        let victim = if self.requested_victim_id != INVALID_ID {
            get_legacy_object(self.requested_victim_id)
        } else {
            None
        };
        let target_pos = if let Some(victim) = victim.as_ref() {
            let victim_guard = victim
                .read()
                .map_err(|_| "victim lock poisoned".to_string())?;
            *victim_guard.get_position()
        } else {
            self.requested_destination
        };
        let in_range = if victim.is_some() {
            weapon.is_within_attack_range(owner_id, Some(self.requested_victim_id), None)
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
        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked = false;
        self.blocked_and_stuck = false;
        Ok(true)
    }
    pub(super) fn prepare_queued_attack_path_fallback(&mut self) -> Result<(), String> {
        // Wave 258: empty dual-world → Ok(()).

        if dual_world_registry_unavailable() {
            return Ok(());
        }

        self.is_attack_path = false;
        if self.requested_victim_id == INVALID_ID {
            return Ok(());
        }

        let Some(victim) = get_legacy_object(self.requested_victim_id) else {
            return Ok(());
        };
        let victim_pos = victim
            .read()
            .map_err(|_| "victim lock poisoned".to_string())?
            .get_position()
            .to_owned();
        self.requested_destination = victim_pos;
        let _ = self.ignore_obstacle(victim.read().ok().map(|g| g.get_id()));
        Ok(())
    }
    pub(super) fn do_queued_approach_pathfind_now(
        &mut self,
        destination: Coord3D,
    ) -> Result<bool, String> {
        self.destroy_path();

        let request = self.build_classic_path_request(destination, false)?;
        let ai_store = the_ai();let closest_result =
            ai_store
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder
                        .read()
                        .ok()
                        .map(|pf| pf.find_closest_path_result(request))
                });

        if let Some(result) = closest_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
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

        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        let base_arc = guard.base_arc();
        let obj_guard = base_arc
            .read()
            .map_err(|_| "unit base object lock poisoned".to_string())?;
        let owner_pos = *obj_guard.get_position();
        let owner_vision_range = obj_guard.get_vision_range();
        drop(obj_guard);
        drop(guard);

        let repulsor_pos1 = get_legacy_object(self.repulsor1)
            .and_then(|repulsor| {
                repulsor
                    .read()
                    .ok()
                    .map(|repulsor_guard| *repulsor_guard.get_position())
            })
            .unwrap_or_else(|| Coord3D::new(-1000.0, -1000.0, 0.0));
        let repulsor_pos2 = get_legacy_object(self.repulsor2)
            .and_then(|repulsor| {
                repulsor
                    .read()
                    .ok()
                    .map(|repulsor_guard| *repulsor_guard.get_position())
            })
            .unwrap_or(repulsor_pos1);
        let ai_store = the_ai();let repulsed_distance = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| data.repulsed_distance)
            })
            .unwrap_or(0.0);
        let safe_radius = owner_vision_range + repulsed_distance;
        let request = self.build_classic_path_request(owner_pos, false)?;
        let ai_store = the_ai();let safe_result =
            ai_store
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder.read().ok().map(|pf| {
                        pf.find_safe_path_result(
                            request,
                            &repulsor_pos1,
                            &repulsor_pos2,
                            safe_radius,
                        )
                    })
                });

        if let Some(result) = safe_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                return Ok(true);
            }
        }

        Ok(false)
    }
    pub(super) fn install_direct_path_from_current_position(
        &mut self,
        destination: &Coord3D,
    ) -> bool {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(mut guard) = unit.write() else {
            return false;
        };

        let mut start = guard.get_position();
        start.z = destination.z;
        guard.current_path = Some(vec![
            Coord2D::new(start.x, start.y),
            Coord2D::new(destination.x, destination.y),
        ]);
        guard.path_following_state = None;
        guard.path_index = 0;
        guard.target_position = Some(*destination);
        guard.movement_state = MovementState::Moving;
        guard.current_speed = 0.0;
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.waiting_for_path = false;
        self.path_timestamp = TheGameLogic::get_frame();
        self.movement_complete = false;
        self.locomotor_goal_type = 1;
        self.locomotor_goal_data = Coord3D::ZERO;
        drop(guard);
        self.set_current_path_snapshot_from_coords(&[start, *destination]);
        true
    }
    pub(super) fn build_classic_path_request(
        &self,
        destination: Coord3D,
        allow_partial: bool,
    ) -> Result<crate::ai::pathfind_complete::PathRequest, String> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        let base_arc = guard.base_arc();
        let obj_guard = base_arc
            .read()
            .map_err(|_| "unit base object lock poisoned".to_string())?;
        let surfaces = guard
            .get_locomotor_surface_mask()
            .unwrap_or(crate::locomotor::SURFACE_GROUND);
        Ok(crate::ai::pathfind_complete::PathRequest {
            object_id: obj_guard.get_id(),
            from: *obj_guard.get_position(),
            to: destination,
            surfaces,
            is_crusher: obj_guard.get_crusher_level() > 0,
            unit_radius: obj_guard.get_geometry_info().get_major_radius(),
            allow_partial,
            move_allies: self.can_path_through_units,
            ignore_obstacle_id: if self.ignore_obstacle_id == INVALID_ID {
                None
            } else {
                Some(self.ignore_obstacle_id)
            },
            is_human: false,
        })
    }
    pub(super) fn queue_path_request_now(&self, destination: Coord3D) -> Result<(), String> {
        let request = self.build_classic_path_request(destination, false)?;

        let ai_store = the_ai(); if let Some(ai) = ai_store.read().ok() {
            if let Some(pathfinder) = ai.pathfinder() {
                pathfinder
                    .read()
                    .map_err(|_| "pathfinder lock poisoned".to_string())?
                    .queue_for_path_request(request)
                    .map_err(|err| err.to_string())?;
            }
        }

        Ok(())
    }
    pub(super) fn clip_goal_position(
        &self,
        guard: &Unit,
        mut pos: Coord3D,
        cmd_source: CommandSourceType,
    ) -> Coord3D {
        if cmd_source != CommandSourceType::FromPlayer {
            return pos;
        }

        let mut fudge = PATHFIND_CELL_SIZE_F * 0.5;
        let is_aircraft = guard
            .base_arc()
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Aircraft))
            .unwrap_or(false);
        if is_aircraft {
            let above_terrain = guard
                .base_arc()
                .read()
                .ok()
                .map(|obj| obj.is_significantly_above_terrain())
                .unwrap_or(false);
            if above_terrain {
                let preferred = guard
                    .current_locomotor
                    .as_ref()
                    .and_then(|loc| loc.lock().ok())
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
        let radius = unit
            .base_arc()
            .read()
            .ok()
            .map(|obj| obj.get_geometry_info().get_bounding_circle_radius())
            .unwrap_or(PATHFIND_CELL_SIZE_F * 0.5);
        let mut diameter = 2.0 * radius;
        if diameter > PATHFIND_CELL_SIZE_F && diameter < 2.0 * PATHFIND_CELL_SIZE_F {
            diameter = 2.0 * PATHFIND_CELL_SIZE_F;
        }

        let mut radius = (diameter / PATHFIND_CELL_SIZE_F + 0.3).floor() as i32;
        let mut center_in_cell = false;

        if radius == 0 {
            radius = 1;
        }
        if (radius & 1) != 0 {
            center_in_cell = true;
        }
        radius /= 2;
        if radius > 2 {
            radius = 2;
            center_in_cell = true;
        }

        (radius, center_in_cell)
    }
    pub(super) fn compute_goal_cell(pos: &Coord3D, center_in_cell: bool) -> ICoord2D {
        if center_in_cell {
            ICoord2D::new(
                (pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
            )
        } else {
            ICoord2D::new(
                (0.5 + pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (0.5 + pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
            )
        }
    }
    pub(super) fn remove_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
    ) {
        if self.pathfind_goal_cell.x < 0 || self.pathfind_goal_cell.y < 0 {
            self.pathfind_goal_cell = ICoord2D::new(-1, -1);
            self.pathfind_goal_layer = ClassicPathLayer::Invalid;
            return;
        }

        let clear_ground = true;
        let clear_layer = self.pathfind_goal_layer != ClassicPathLayer::Ground
            && self.pathfind_goal_layer != ClassicPathLayer::Invalid;
        pathfinder.clear_goal_cells(
            unit_id,
            self.pathfind_goal_cell,
            radius,
            center_in_cell,
            self.pathfind_goal_layer,
            clear_ground,
            clear_layer,
        );
        pathfinder.clear_aircraft_goal_cells(
            unit_id,
            self.pathfind_goal_cell,
            radius,
            center_in_cell,
        );

        self.pathfind_goal_cell = ICoord2D::new(-1, -1);
        self.pathfind_goal_layer = ClassicPathLayer::Invalid;
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
        let layer_changed = self.pathfind_goal_layer != layer;
        if !layer_changed
            && self.pathfind_goal_cell.x == new_cell.x
            && self.pathfind_goal_cell.y == new_cell.y
        {
            return;
        }

        self.remove_goal_cells(pathfinder, unit_id, radius, center_in_cell);

        self.pathfind_goal_cell = new_cell;
        self.pathfind_goal_layer = layer;

        let mut do_ground = layer == ClassicPathLayer::Ground;
        let do_layer = layer != ClassicPathLayer::Ground;
        if do_layer && interacts_with_bridge_end {
            do_ground = true;
        }

        pathfinder.set_goal_cells(
            unit_id,
            new_cell,
            radius,
            center_in_cell,
            layer,
            do_ground,
            do_layer,
        );
    }
    pub(super) fn update_aircraft_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.remove_goal_cells(pathfinder, unit_id, radius, center_in_cell);

        if !self.is_aircraft_that_adjusts_destination() {
            return;
        }

        self.pathfind_goal_cell = new_cell;
        self.pathfind_goal_layer = ClassicPathLayer::Ground;

        pathfinder.set_aircraft_goal_cells(unit_id, new_cell, radius, center_in_cell);
    }
    pub(super) fn has_valid_locomotor_surfaces(&self) -> bool {
        get_unit_arc(self.unit_id)
            .and_then(|unit| {
                unit.read()
                    .ok()
                    .and_then(|guard| guard.get_locomotor_surface_mask())
            })
            .map(|surfaces| surfaces != 0)
            .unwrap_or(false)
    }
    pub(super) fn safe_path_search_distance(vision_range: Real, repulsed_distance: Real) -> Real {
        vision_range + repulsed_distance
    }
    pub(super) fn current_path_extra_distance(&self) -> Real {
        get_unit_arc(self.unit_id)
            .and_then(|unit| unit.read().ok().map(|guard| guard.path_extra_distance))
            .unwrap_or(0.0)
    }
    pub(super) fn finish_completed_movement_like_cpp(&mut self) {
        if !self.movement_complete {
            return;
        }

        self.set_queue_for_path_time(0);
        self.destroy_path();
        self.set_locomotor_goal_none();

        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(guard) = unit.read() {
                if let Ok(mut object) = guard.base_arc().write() {
                    object.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
            }
        }

        self.movement_complete = false;
        self.ignore_obstacle_id = INVALID_ID;
    }
}
