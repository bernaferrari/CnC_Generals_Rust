//! Native UnitAI path and explicit-position movement phases.
//!
//! This adapter loans the AI's real path and active locomotor, and asks its
//! supplied AiWorld to update the exact owner through admitted Physics.
use super::ai_core::UnitAIUpdate;
use super::ai_helpers::to_locomotor_body_damage_type;
use super::ai_loco::NativePathMovementContext;
use super::imports::*;

/// Same footprint definition as the supplied queued Pathfinder observations.
/// Missing retired obstacles match C++ NULL-ignore semantics; a poisoned live
/// object is an error rather than an unrelated global lookup.
pub(super) fn ignored_obstacle_cells(
    world: &dyn crate::modules::ai_world::AiWorld,
    id: Option<ObjectID>,
) -> Result<Option<std::collections::HashSet<crate::ai::pathfind_astar::GridCoord>>, String> {
    let Some(object) = id
        .filter(|id| *id != INVALID_ID)
        .and_then(|id| world.object(id))
    else {
        return Ok(None);
    };
    let object = object
        .read()
        .map_err(|_| "native ignored path obstacle poisoned")?;
    Ok(
        crate::ai::object_footprint_positions_with_height(&object, |x, y| {
            world.terrain_layer_height(x, y, crate::common::PathfindLayerEnum::Ground)
        })
        .map(|positions| {
            positions
                .into_iter()
                .map(|position| crate::ai::pathfind_astar::GridCoord::from_world(&position))
                .collect()
        }),
    )
}

impl UnitAIUpdate {
    pub(super) fn update_locomotor_phase_with_world(
        &mut self,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<game_engine::common::thing::update_module::UpdateSleepTime, String> {
        use game_engine::common::thing::update_module::UpdateSleepTime;
        self.validate_owned_components_with_world(
            world,
            self.runtime.data.locomotor_goal_type == 0,
            false,
        )
        .map_err(|error| error.to_string())?;
        if !matches!(self.runtime.data.locomotor_goal_type, 0 | 1 | 2) {
            return Err(
                "native locomotion phase requires NONE, POSITION_ON_PATH or POSITION_EXPLICIT"
                    .into(),
            );
        }
        let owner = self
            .runtime
            .native_owner()
            .ok_or("native locomotion owner is unavailable")?;
        let active = self
            .runtime
            .data
            .locomotor_set
            .get_active()
            .ok_or("native locomotion has no active member")?;
        let surfaces = active.get_legal_surfaces();
        if surfaces & crate::locomotor::SURFACE_GROUND == 0
            || surfaces & crate::locomotor::SURFACE_AIR != 0
        {
            return Err("native path phase admits ordinary ground surfaces only".into());
        }
        if self.runtime.data.locomotor_goal_type == 0 {
            world.validate_owned_maintenance_step(&owner, active)?;
        } else {
            world.validate_owned_locomotor_step(&owner, active)?;
        }
        let (current, layer, condition) = {
            let object = owner
                .read()
                .map_err(|_| "native locomotion owner poisoned")?;
            if self.runtime.data.locomotor_goal_type != 0 && object.get_container_id().is_some() {
                return Err("native path phase does not admit contained owners".into());
            }
            let body = object
                .get_body_module()
                .ok_or("native path phase requires its installed Body module")?;
            let damage = body
                .lock()
                .map_err(|_| "native locomotion Body poisoned")?
                .get_damage_state();
            (
                *object.get_position(),
                crate::ai::pathfind::PathfindLayerEnum::from_u32(object.get_layer() as u32),
                to_locomotor_body_damage_type(damage),
            )
        };
        if layer != crate::ai::pathfind::PathfindLayerEnum::Ground {
            return Err("native path phase admits the ground layer only".into());
        }
        if self.runtime.data.is_ai_in_dead_state() && !active.template.locomotor_works_when_dead {
            world.update_inactive_locomotor_tail(&owner, active)?;
            self.runtime.data.cur_max_blocked_speed = crate::modules::FAST_AS_POSSIBLE;
            return Ok(UpdateSleepTime::None);
        }
        if self.runtime.data.locomotor_goal_type == 0 {
            let final_position = self
                .runtime
                .data
                .do_final_position
                .then_some(self.runtime.data.final_position);
            let active = self
                .runtime
                .data
                .locomotor_set
                .get_active_mut()
                .ok_or("native locomotion member disappeared")?;
            let (position, still_final, requires_constant) =
                world.maintain_owned_locomotor_step(&owner, active, final_position)?;
            if final_position.is_some() && !still_final {
                self.runtime.data.final_position = position;
            }
            self.runtime.data.do_final_position = still_final;
            // CPP NONE leaves blocked unchanged, then restores the next frame's
            // blocked-speed limit after the world's actual AirborneTarget update.
            self.runtime.data.cur_max_blocked_speed = crate::modules::FAST_AS_POSSIBLE;
            return Ok(
                if !still_final
                    && !self.runtime.data.is_blocked
                    && self.runtime.data.blocked_frames == 0
                    && !requires_constant
                {
                    UpdateSleepTime::Forever
                } else {
                    UpdateSleepTime::None
                },
            );
        }
        let penalty = world.locomotor_movement_penalty()?;
        let ignore_obstacle_id = self.runtime.data.ignore_obstacle_id;
        let ignore_obstacle =
            (ignore_obstacle_id != crate::common::INVALID_ID).then_some(ignore_obstacle_id);
        let ignore_cells = ignored_obstacle_cells(world, ignore_obstacle)?;
        let (goal, distance) = if self.runtime.data.locomotor_goal_type == 2 {
            // CPP AIUpdate.cpp:2143–2153 passes an explicit goal directly,
            // with zero on-path distance; no path queue or CPOP is involved.
            let goal = self.runtime.data.locomotor_goal_data;
            if !goal.is_finite() {
                return Err("native explicit goal must be finite".into());
            }
            (goal, 0.0)
        } else {
            let path = match self.runtime.data.current_path_snapshot.as_mut() {
                Some(path) => path,
                None if self.runtime.data.waiting_for_path => return Ok(UpdateSleepTime::None),
                None => return Err("native POSITION_ON_PATH has no installed path".into()),
            };
            if !path.is_finite_single_layer(crate::ai::pathfind::PathfindLayerEnum::Ground) {
                return Err("native path phase requires finite ground-layer nodes".into());
            }
            let pathfinder = world
                .pathfinder()
                .ok_or("native path phase requires its supplied Pathfinder")?;
            let mut context = NativePathMovementContext::borrowed(
                pathfinder,
                surfaces,
                ignore_obstacle,
                layer,
                ignore_cells,
            );
            let point = path.compute_point_on_path(&current, &mut context);
            drop(context);
            if point.layer != crate::ai::pathfind::PathfindLayerEnum::Ground {
                return Err("native path phase encountered an unsupported path layer".into());
            }
            (
                point.pos_on_path,
                point.dist_along_path + self.runtime.data.path_extra_distance,
            )
        };

        let max_speed = active.get_max_speed_with_penalty(condition, penalty);
        let mut speed = self.runtime.data.desired_speed;
        if speed == crate::modules::FAST_AS_POSSIBLE || speed > max_speed {
            speed = max_speed;
        }
        let blocked = self.runtime.data.blocked_frames > 0;
        let (speed, blocked) = if self.runtime.data.locomotor_goal_type == 1 {
            self.runtime
                .data
                .apply_bump_speed_limit_with_blocked(speed, blocked)
        } else {
            (speed, blocked)
        };
        let active = self
            .runtime
            .data
            .locomotor_set
            .get_active_mut()
            .ok_or("native locomotion member disappeared")?;
        world.move_owned_locomotor_step(
            &owner, active, goal, distance, speed, blocked, condition, penalty,
        )?;
        self.runtime.data.do_final_position = false;
        Ok(UpdateSleepTime::None)
    }

    #[cfg(test)]
    pub(crate) fn install_native_path_for_test(&mut self, points: &[Coord3D]) {
        self.runtime.set_current_path_snapshot_from_coords(points);
        self.runtime.data.locomotor_goal_type = 1;
    }
}

/// Read-only evidence from the installed controller; never used for simulation.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct NativeWaypointObservation {
    pub(crate) pathfind_goal_cell: ICoord2D,
    pub(crate) locomotor_goal_type: u32,
    pub(crate) locomotor_goal_data: Coord3D,
    pub(crate) movement_complete: bool,
    pub(crate) temporary_state: Option<u32>,
    pub(crate) requested_destination: Coord3D,
    pub(crate) waiting_for_path: bool,
    pub(crate) has_path: bool,
    pub(crate) do_final_position: bool,
    pub(crate) current_waypoint: Option<crate::waypoint::WaypointId>,
    pub(crate) completed_waypoint: Option<crate::waypoint::WaypointId>,
    pub(crate) parent_goal: Option<Coord3D>,
}
#[cfg(test)]
impl UnitAIUpdate {
    pub(crate) fn native_path_distance_for_test(
        &mut self,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<Real, String> {
        self.validate_world_owner(world)
            .map_err(|error| error.to_string())?;
        let goal = self
            .ai_state_machine
            .as_ref()
            .and_then(|machine| machine.get_goal_position());
        let distance = self
            .runtime
            .get_locomotor_distance_to_goal_with_world(goal, Some(world));
        if distance.is_finite() {
            Ok(distance)
        } else {
            Err("native runtime distance is nonfinite".into())
        }
    }
    pub(crate) fn native_waypoint_observation_for_test(&self) -> NativeWaypointObservation {
        NativeWaypointObservation {
            pathfind_goal_cell: self.runtime.data.pathfind_goal_cell,
            locomotor_goal_type: self.runtime.data.locomotor_goal_type,
            locomotor_goal_data: self.runtime.data.locomotor_goal_data,
            movement_complete: self.runtime.data.movement_complete,
            temporary_state: self
                .ai_state_machine
                .as_ref()
                .and_then(|machine| machine.get_temporary_state()),
            requested_destination: self.runtime.data.requested_destination,
            waiting_for_path: self.runtime.data.waiting_for_path,
            has_path: self.runtime.data.current_path_snapshot.is_some(),
            do_final_position: self.runtime.data.do_final_position,
            current_waypoint: self.runtime.data.current_waypoint_id,
            completed_waypoint: self.runtime.data.completed_waypoint_id,
            parent_goal: self
                .ai_state_machine
                .as_ref()
                .and_then(|machine| machine.get_goal_position()),
        }
    }
}
