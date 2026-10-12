//! One native UnitAI POSITION_ON_PATH movement phase.
//!
//! This adapter loans the AI's real path and active locomotor, and asks its
//! supplied AiWorld to update the exact owner through admitted Physics.
use super::ai_core::UnitAIUpdate;
use super::ai_helpers::to_locomotor_body_damage_type;
use super::ai_loco::NativePathMovementContext;
use super::imports::*;

impl UnitAIUpdate {
    pub(super) fn update_locomotor_path_phase_with_world(
        &mut self,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        self.validate_owned_components(false, false)
            .map_err(|error| error.to_string())?;
        if self.runtime.data.locomotor_goal_type != 1 {
            return Err("native locomotion phase requires POSITION_ON_PATH".into());
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
        world.validate_owned_locomotor_path_step(&owner, active)?;
        let penalty = world.locomotor_movement_penalty()?;
        let ignore_obstacle_id = self.runtime.data.ignore_obstacle_id;
        if ignore_obstacle_id != crate::common::INVALID_ID {
            return Err("native path phase with ignored obstacle cells is not admitted".into());
        }

        let (current, layer, condition) = {
            let object = owner
                .read()
                .map_err(|_| "native locomotion owner poisoned")?;
            if object.get_container_id().is_some() {
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
            return Ok(());
        }
        let path = match self.runtime.data.current_path_snapshot.as_mut() {
            Some(path) => path,
            None if self.runtime.data.waiting_for_path => return Ok(()),
            None => return Err("native POSITION_ON_PATH has no installed path".into()),
        };
        if !path.is_finite_single_layer(crate::ai::pathfind::PathfindLayerEnum::Ground) {
            return Err("native path phase requires finite ground-layer nodes".into());
        }
        let pathfinder = world
            .pathfinder()
            .ok_or("native path phase requires its supplied Pathfinder")?;
        let mut context =
            NativePathMovementContext::borrowed(pathfinder, surfaces, None, layer, None);
        let point = path.compute_point_on_path(&current, &mut context);
        drop(context);
        if point.layer != crate::ai::pathfind::PathfindLayerEnum::Ground {
            return Err("native path phase encountered an unsupported path layer".into());
        }

        let max_speed = active.get_max_speed_with_penalty(condition, penalty);
        let mut speed = self.runtime.data.desired_speed;
        if speed == crate::modules::FAST_AS_POSSIBLE || speed > max_speed {
            speed = max_speed;
        }
        let blocked = self.runtime.data.blocked_frames > 0;
        let (speed, blocked) = self
            .runtime
            .data
            .apply_bump_speed_limit_with_blocked(speed, blocked);
        let active = self
            .runtime
            .data
            .locomotor_set
            .get_active_mut()
            .ok_or("native locomotion member disappeared")?;
        world.move_owned_locomotor_path_step(
            &owner,
            active,
            point.pos_on_path,
            point.dist_along_path + self.runtime.data.path_extra_distance,
            speed,
            blocked,
            condition,
            penalty,
        )?;
        self.runtime.data.do_final_position = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn install_native_path_for_test(&mut self, points: &[Coord3D]) {
        self.runtime.set_current_path_snapshot_from_coords(points);
        self.runtime.data.locomotor_goal_type = 1;
    }
}

/// Read-only evidence from the installed controller; never used for simulation.
#[cfg(test)]
pub(crate) struct NativeWaypointObservation {
    pub(crate) requested_destination: Coord3D,
    pub(crate) waiting_for_path: bool,
    pub(crate) has_path: bool,
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
            requested_destination: self.runtime.data.requested_destination,
            waiting_for_path: self.runtime.data.waiting_for_path,
            has_path: self.runtime.data.current_path_snapshot.is_some(),
            current_waypoint: self.runtime.data.current_waypoint_id,
            completed_waypoint: self.runtime.data.completed_waypoint_id,
            parent_goal: self
                .ai_state_machine
                .as_ref()
                .and_then(|machine| machine.get_goal_position()),
        }
    }
}
