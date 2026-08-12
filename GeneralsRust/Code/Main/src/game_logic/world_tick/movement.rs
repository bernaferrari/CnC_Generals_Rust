//! Host tick `impl GameLogic` — `movement`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// Move an object to a target position using pathfinding.
    /// Falls back to direct movement if no path is found.
    /// If `ai_state_override` is provided, sets that AI state after moving.
    pub(in super::super) fn move_object_with_pathfinding(
        &mut self,
        object_id: ObjectId,
        target_position: Vec3,
        ai_state_override: Option<AIState>,
    ) {
        let start_pos = self.objects.get(&object_id).map(|obj| obj.get_position());

        let start_pos = match start_pos {
            Some(pos) => pos,
            None => return,
        };

        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        let apply_state = |logic: &mut Self, state: AIState| {
            if decision_auth {
                let ordinal =
                    crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
            } else if let Some(obj) = logic.objects.get_mut(&object_id) {
                obj.set_ai_state(state);
            }
        };

        // Short distance — skip pathfinding overhead and go direct.
        if start_pos.distance(target_position) < 20.0 {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.move_to(target_position);
            }
            if let Some(state) = ai_state_override {
                apply_state(self, state);
            }
            return;
        }

        // Attempt A* pathfinding.
        let path = self
            .pathfinding_system
            .find_path(start_pos, target_position, &self.objects);

        let mut state_to_apply: Option<AIState> = None;
        if let Some(obj) = self.objects.get_mut(&object_id) {
            if let Some(waypoints) = path {
                if waypoints.len() >= 2 {
                    obj.movement.path = waypoints;
                    obj.record_host_movement();
                    obj.movement.current_path_index = 1; // skip start node
                                                         // target_position will be set to path[1] by update_movement
                    obj.movement.target_position = Some(obj.movement.path[1]);
                    obj.set_status_moving(true);
                    // Final destination for shadow move channel (not intermediate waypoint).
                    crate::game_logic::host_move_log::record(
                        object_id,
                        Some([target_position.x, target_position.y, target_position.z]),
                    );
                    state_to_apply = Some(ai_state_override.unwrap_or(AIState::Moving));
                } else {
                    obj.move_to(target_position);
                    state_to_apply = ai_state_override;
                }
            } else {
                // No path found — fall back to direct movement.
                obj.move_to(target_position);
                state_to_apply = ai_state_override;
            }
        }
        if let Some(state) = state_to_apply {
            apply_state(self, state);
        }
    }

    /// Update movement for all objects
    pub(in super::super) fn update_movement(&mut self, object_ids: &[ObjectId], dt: f32) {
        // GameWorld movement authority: path integrate + pose last-write runs in
        // shadow_session_after_host_tick via GameWorld::step_movement. Host still
        // owns path *commands* (move_to / attack-move logs) earlier in the frame.
        // Wave 875: movement authority early-return honesty — GW sole integrate.
        if crate::gameworld_shadow::gameworld_movement_authority_live() {
            let _ = (object_ids, dt);
            return;
        }
        for &id in object_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                // Horizontal (XZ) distance — path grid / terrain height use Y separately,
                // and 3D distance falsely stalls waypoint advance when |ΔY| is large.
                let horiz = |a: Vec3, b: Vec3| {
                    let dx = a.x - b.x;
                    let dz = a.z - b.z;
                    (dx * dx + dz * dz).sqrt()
                };

                if !obj.movement.path.is_empty()
                    && obj.movement.current_path_index < obj.movement.path.len()
                {
                    let current_pos = obj.get_position();
                    let waypoint = obj.movement.path[obj.movement.current_path_index];
                    if horiz(current_pos, waypoint) < 5.0 {
                        obj.movement.current_path_index += 1;
                        if obj.movement.current_path_index >= obj.movement.path.len() {
                            let do_evac = obj.pending_evacuate_on_stop;
                            let and_exit = obj.pending_exit_after_evacuate;
                            obj.stop_moving();
                            if do_evac {
                                // Defer mut borrow: process after this get_mut ends.
                                // Use a side channel via pending flags re-set for post-pass.
                                obj.pending_evacuate_on_stop = true;
                                obj.pending_exit_after_evacuate = and_exit;
                            }
                            continue;
                        }
                    }

                    let waypoint = obj.movement.path[obj.movement.current_path_index];
                    // Keep unit height; path cells often sit at Y=0 from the grid.
                    let mut target = waypoint;
                    target.y = current_pos.y;
                    obj.movement.target_position = Some(target);
                }

                if let Some(target_pos) = obj.movement.target_position {
                    let current_pos = obj.get_position();
                    // March in the XZ plane; do not dive to Y=0 path cells.
                    let mut flat_target = target_pos;
                    flat_target.y = current_pos.y;
                    let direction = (flat_target - current_pos).normalize_or_zero();

                    if direction.length() > 0.0 {
                        // Calculate new position and orientation
                        let target_velocity = direction * obj.movement.max_speed;
                        let velocity_diff = target_velocity - obj.movement.velocity;
                        let max_accel = obj.movement.acceleration * dt;

                        let new_velocity = if velocity_diff.length() <= max_accel {
                            target_velocity
                        } else {
                            obj.movement.velocity + velocity_diff.normalize() * max_accel
                        };

                        // Persist velocity — without this, every frame restarts from 0 and
                        // units crawl at ~accel*dt per frame (pure-march combat stalls OOR).
                        obj.movement.velocity = new_velocity;
                        obj.record_host_movement();

                        let new_position = current_pos + new_velocity * dt;
                        let desired_angle = (-new_velocity.z).atan2(new_velocity.x);
                        let reached_target = horiz(current_pos, flat_target) < 2.0;

                        obj.set_position(new_position);
                        obj.set_orientation(desired_angle);
                        if reached_target {
                            // Only stop when there is no further path waypoint.
                            // Mid-path "reached" is handled by index advance above.
                            if obj.movement.path.is_empty()
                                || obj.movement.current_path_index + 1 >= obj.movement.path.len()
                            {
                                obj.stop_moving();
                            } else {
                                obj.movement.current_path_index += 1;
                                let mut next = obj.movement.path[obj.movement.current_path_index];
                                next.y = new_position.y;
                                obj.movement.target_position = Some(next);
                            }
                        }
                    } else {
                        // Already on target (zero horizontal delta).
                        obj.movement.velocity = Vec3::ZERO;
                        obj.record_host_movement();
                        if obj.movement.path.is_empty()
                            || obj.movement.current_path_index + 1 >= obj.movement.path.len()
                        {
                            obj.stop_moving();
                        }
                    }
                }
            }
        }

        // C++ AICMD_MOVE_TO_POSITION_AND_EVACUATE arrival residual.
        let mut evac_now: Vec<(ObjectId, bool)> = Vec::new();
        for &id in object_ids {
            if let Some(obj) = self.objects.get(&id) {
                if obj.pending_evacuate_on_stop
                    && obj.movement.path.is_empty()
                    && !obj.status.moving
                {
                    evac_now.push((id, obj.pending_exit_after_evacuate));
                }
            }
        }
        for (id, and_exit) in evac_now {
            let _ = self.evacuate_container_now(id, and_exit);
        }
    }

    #[cfg(test)]
    pub fn update_movement_for_test(&mut self, object_ids: &[ObjectId], dt: f32) {
        self.update_movement(object_ids, dt);
    }

    /// Update AI behavior for all objects
    /// Enhanced with AI decision system for intelligent behavior

    /// Drain global fire-spawn queue into host CombatSystem (fire-spawn authority apply).
    pub(crate) fn drain_pending_projectiles_into_combat(&mut self) {
        crate::game_logic::combat::drain_pending_projectiles(
            &mut self.combat_system,
            &self.objects,
        );
        self.execute_pending_weapon_fire_ocls();
    }

    /// Hit-only projectile pass after GameWorld flight integrate writeback.
    pub(crate) fn resolve_projectiles_hits_only(&mut self) -> Vec<ObjectId> {
        self.combat_system.refresh_homing_aims(&self.objects);
        let hits = self.combat_system.update_projectiles_with_countermeasures(
            0.0,
            &mut self.objects,
            Some(&mut self.countermeasures),
            self.frame,
        );
        self.flush_projectile_impact_fx();
        hits
    }
}
