use super::*;
use gamelogic::object::contain::OpenContainModuleData;

#[cfg(test)]
#[path = "spawn_scheduler_open_contain_tests.rs"]
mod tests;

// Read the admitted module, not a template-name or unit-kind approximation.
fn open_contain_data(parent: &Object) -> Option<parking_lot::MappedRwLockReadGuard<'_, OpenContainModuleData>> {
    use gamelogic::contain_module_overrides::ContainModuleDataKind;
    use game_engine::common::thing::module::ModuleDataGuard;
    let receiving = parent.get_template().receiving_client_template.as_ref()?;
    receiving.behavior_modules().iter().find_map(|module| {
        let guard = module.data.read();
        ModuleDataGuard::try_map(guard, |data| {
            ContainModuleDataKind::from_module_data(data).map(|kind| kind.open_data())
        })
    })
}

impl GameLogic {
    // Resolve against the model condition that exists after the door pulse.
    // A genuine name miss has Object::getSingleLogicalBonePosition's parent-position fallback.
    fn canonical_open_contain_path(
        &self,
        parent: &Object,
        effects: &ScriptPresentationEffects<'_>,
    ) -> Option<Option<(Vec3, Vec3)>> {
        let data = open_contain_data(parent)?;
        let count = data.number_of_exit_paths;
        if count <= 0 { return Some(None); }
        if count > u8::MAX as i32 { return None; }
        let which = i32::from(parent.which_exit_path.max(1));
        let suffix = if count > 1 { format!("{which:02}") } else { String::new() };
        let (clear, set) = if data.door_open_time != 0 {
            (1u128 << crate::game_logic::host_enum_table_residual::door_1_closing_model_bit(),
             1u128 << crate::game_logic::host_enum_table_residual::door_1_opening_model_bit())
        } else {
            (0, 0)
        };
        drop(data);
        let query = |prefix: &str| {
            let poses = self.query_pristine_world_bone_poses(parent, effects.pristine_bones,
                prefix, 0, 1, clear, set).ok()?;
            Some(poses.into_iter().next().map_or(parent.get_position(), |pose| pose.0))
        };
        Some(Some((query(&format!("ExitStart{suffix}"))?, query(&format!("ExitEnd{suffix}"))?)))
    }

    pub(in crate::game_logic) fn canonical_open_contain_exit_available(
        &self, parent_id: ObjectId, effects: &ScriptPresentationEffects<'_>,
    ) -> bool {
        self.objects.get(&parent_id).is_some_and(|parent| self.canonical_open_contain_path(parent, effects).is_some())
    }

    pub(in crate::game_logic) fn canonical_open_contain_spawn_exit(
        &mut self, parent_id: ObjectId, child_id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if parent_id == child_id { return Ok(false); }
        let Some(parent) = self.objects.get(&parent_id) else { return Ok(false); };
        let Some(path) = self.canonical_open_contain_path(parent, effects) else { return Ok(false); };
        let Some(data) = open_contain_data(parent) else { return Ok(false); };
        let count = data.number_of_exit_paths;
        let door_time = data.door_open_time;
        drop(data);
        let yaw = parent.get_orientation();
        let layer = parent.pathfind_layer;
        let rally = parent.open_contain_rally_point;
        let parent_birth = parent.visual_object_generation;
        let epoch = self.visual_world_epoch;
        let Some(child) = self.objects.get(&child_id) else { return Ok(false); };
        let child_birth = child.visual_object_generation;
        // SpawnBehavior's freshly-created child is not in the contain list, so
        // OpenContain::removeFromContain returns without containment callbacks.
        if child.contained_by == Some(parent_id) { return Ok(false); }
        let parent = self.objects.get_mut(&parent_id).unwrap();
        parent.door_close_countdown = door_time;
        let pulse = gamelogic::object::contain::open_contain::leftover_open_contain_arm_exit_door(door_time);
        super::super::super::world_combat::apply_leftover_open_contain_door_pulse(parent, pulse);
        if count > 1 {
            let current = i32::from(parent.which_exit_path.max(1));
            parent.which_exit_path = ((current % count) + 1) as u8;
        }
        let Some((start, mut end)) = path else {
            self.update_owned_pathfinder_position(child_id);
            return Ok(true);
        };
        if !self.set_owned_object_position_with_effects(child_id, start, self.frame, effects) {
            return Ok(true);
        }
        if !self.spawn_identity_matches(parent_id, parent_birth, epoch)
            || !self.spawn_identity_matches(child_id, child_birth, epoch) {
            return Ok(true);
        }
        self.objects.get_mut(&child_id).unwrap().set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
        self.set_owned_pathfinder_layer(child_id, layer);
        self.update_owned_pathfinder_position(child_id);
        let has_ai = self.objects.get(&child_id).is_some_and(|o| o.has_ai_update_interface());
        if !has_ai { return Ok(true); }
        let refresh_parent = self.objects.get(&parent_id).is_some_and(|parent| {
            parent.is_kind_of(KindOf::Vehicle)
                && parent.has_ai_update_interface()
                && parent.ai_state() == AIState::Idle
        });
        if refresh_parent {
            let parent = self.objects.get_mut(&parent_id).unwrap();
            let radius = parent.geometry_info().bounding_circle_radius();
            if self.pathfinding_system.map_ready {
                self.pathfinding_system.grid.clear_path_position_square(
                    parent_id.0, radius, parent.pathfind_cur_cell, parent.pathfind_layer,
                );
                parent.pathfind_cur_cell = (-1, -1);
            }
            self.pathfinding_system.grid.clear_path_goal_square(
                parent_id.0, radius, parent.pathfind_goal_cell, parent.pathfind_goal_layer,
            );
            parent.pathfind_goal_cell = (-1, -1);
            let position = parent.get_position();
            self.update_owned_pathfinder_position(parent_id);
            self.register_path_goal(parent_id, position);
        }
        if let Some(child) = self.objects.get_mut(&child_id) {
            child.ignored_obstacle_id = None;
            child.ignore_collisions_with = None;
            child.ignore_collisions_until_frame = self.frame.saturating_add(30);
        }
        let _ = self.adjust_to_possible_destination(child_id, &mut end);
        if let Some(child) = self.objects.get_mut(&child_id) {
            let allow_fall = child.allow_to_fall;
            child.allow_to_fall = false;
            child.set_destination_with_operation(end, &mut self.movement_events.writer(self.frame),
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events), self.frame,
                &mut effects.reborrow_for_object(&mut self.partition_manager));
            child.allow_to_fall = allow_fall;
            let mut path = vec![start, end, end];
            if let Some(rally) = rally { path.push(rally); }
            child.movement.path = path;
            child.movement.current_path_index = 1;
            child.movement.target_position = Some(end);
            child.set_ai_state(AIState::Moving, self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events));
            child.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
            child.status.moving = true;
        }
        self.register_path_goal(child_id, end);
        Ok(true)
    }
}
