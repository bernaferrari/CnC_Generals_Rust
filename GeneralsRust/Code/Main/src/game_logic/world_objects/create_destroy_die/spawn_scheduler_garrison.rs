use super::*;

#[cfg(test)]
#[path = "spawn_scheduler_garrison_tests.rs"]
mod tests;

impl GameLogic {
    // OpenContain's inherited reserveDoorForExit always returns DOOR_1.
    pub(super) fn reserve_garrison_spawn_exit(
        &mut self, owner: ObjectId, _effects: &mut ScriptPresentationEffects<'_>,
    ) -> bool {
        self.objects.get(&owner).is_some_and(|object| {
            object.is_garrison_contain()
                && (!object.is_enclosing_garrison_container()
                    || matches!(object.garrison_evac_disposition(), 1 | 2)
                    || self.terrain_height_at(object.get_position()).is_some())
        })
    }

    pub(super) fn exit_garrison_spawn(
        &mut self, owner: ObjectId, child: ObjectId, effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if owner == child { return Ok(false); }
        let Some(parent) = self.objects.get(&owner) else { return Ok(false); };
        if !parent.is_garrison_contain() { return Ok(false); }
        let parent_birth = parent.visual_object_generation;
        let epoch = self.visual_world_epoch;
        let origin = parent.get_position();
        let yaw = parent.get_orientation();
        let geometry = *parent.geometry_info();
        let major = geometry.major_radius;
        let minor = geometry.minor_radius;
        let enclosing = parent.is_enclosing_garrison_container();
        let disposition = parent.garrison_evac_disposition();
        let Some(unit) = self.objects.get(&child) else { return Ok(false); };
        let child_birth = unit.visual_object_generation;
        // The freshly spawned object was never admitted to the contain list.
        if unit.contained_by == Some(owner) { return Ok(false); }
        let has_ai = unit.has_ai_update_interface();
        let surfaces = unit.locomotor_surfaces;
        let has_locomotor = unit.cur_locomotor_name.is_some();
        let (sin, cos) = yaw.sin_cos();
        let mut start = origin;
        if has_ai && has_locomotor {
            use gamelogic::ai::pathfind_astar::PathfindCellType;
            let grid = &self.pathfinding_system.grid;
            let valid = |position| {
                let cell = grid.world_to_grid(position);
                if !grid.is_valid_pos(cell) { return false; }
                let ty = grid.cell_type(cell);
                matches!(ty, PathfindCellType::Obstacle | PathfindCellType::Impassable)
                    || surfaces & crate::game_logic::locomotor_bootstrap::valid_locomotor_surfaces_for_cell_type(ty) != 0
            };
            if !valid(start) {
                start.x -= major * cos;
                start.z -= major * sin;
                if !valid(start) {
                    start.x += 2.0 * major * cos;
                    start.z += 2.0 * major * sin;
                    if !valid(start) { start = origin; }
                }
            }
        }
        let mut end = start;
        let side = disposition == 1 || disposition == 2;
        if side {
            let scalar = if disposition == 1 { 1.0 } else { -1.0 };
            let door_x = self.logic_random.next_real(-major / 4.0, major / 4.0);
            let door_y = self.logic_random.next_real(minor / 2.0, minor * 2.0) * scalar;
            let walk_x = self.logic_random.next_real(-major, major);
            let walk_y = minor * 10.0 * scalar;
            let transform = |x: f32, y: f32| Vec3::new(
                origin.x + x * cos - y * sin, origin.y, origin.z + x * sin + y * cos,
            );
            start = transform(door_x, door_y);
            end = transform(walk_x, walk_y);
        }
        if side || enclosing {
            if !self.set_owned_object_position_with_effects(child, start, self.frame, effects) {
                return Ok(true);
            }
            if !self.spawn_identity_matches(owner, parent_birth, epoch) || !self.spawn_identity_matches(child, child_birth, epoch) {
                return Ok(true);
            }
            if !side {
                let Some(height) = self.terrain_height_at(start) else { return Ok(false); };
                start.y = height;
                if !self.set_owned_object_position_with_effects(child, start, self.frame, effects) {
                    return Ok(true);
                }
            }
        }
        if !self.spawn_identity_matches(owner, parent_birth, epoch) || !self.spawn_identity_matches(child, child_birth, epoch) {
            return Ok(true);
        }
        if let Some(unit) = self.objects.get_mut(&child) {
            unit.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
        }
        self.update_owned_pathfinder_position(child);
        if has_ai {
            let _ = self.adjust_to_possible_destination(child, &mut end);
            if let Some(unit) = self.objects.get_mut(&child) {
                unit.movement.path.clear();
                unit.movement.path.push(end);
                unit.movement.current_path_index = 0;
                unit.is_attack_path = false;
                unit.can_path_through_units = false;
                unit.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
                unit.refresh_follow_path_extra_distance();
                unit.set_adjust_destinations(!unit.ultra_accurate);
                unit.set_destination_with_operation(end, &mut self.movement_events.writer(self.frame),
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events), self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager));
                unit.set_ai_state(AIState::Moving, self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events));
                unit.status.moving = true;
            }
            self.register_path_goal(child, end);
        }
        if self.spawn_identity_matches(owner, parent_birth, epoch) && self.spawn_identity_matches(child, child_birth, epoch) {
            self.recalc_garrison_apparent_controller_with_effects(owner, effects)?;
        }
        Ok(true)
    }
}
