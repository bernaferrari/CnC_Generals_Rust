//! Host tick `impl GameLogic` — `movement`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// Move an object to a target position using pathfinding.
    ///
    /// C++ `AIInternalMoveToState::computePath` never installs a straight-line
    /// fallback through blocked cells (AIStates.cpp:1577-1585). A null path
    /// leaves the unit halted (`update` returns `STATE_FAILURE` at
    /// AIStates.cpp:1771-1778).
    /// If `ai_state_override` is provided, sets that AI state after a real path.
    pub(in super::super) fn move_object_with_pathfinding(
        &mut self,
        object_id: ObjectId,
        target_position: Vec3,
        ai_state_override: Option<AIState>,
    ) {
        let (start_pos, is_aircraft, surfaces) = match self.objects.get(&object_id) {
            Some(obj) => {
                let surfaces = if obj.locomotor_surfaces != 0 {
                    obj.locomotor_surfaces
                } else {
                    Object::default_locomotor_surfaces_for_template(&obj.thing.template)
                };
                (
                    obj.get_position(),
                    obj.is_kind_of(KindOf::Aircraft)
                        || obj.object_type == crate::game_logic::ObjectType::Aircraft,
                    surfaces,
                )
            }
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

        // C++ Pathfinder uses the mover's legal surfaces (AIPathfind.cpp:4779-4782).
        // Aircraft use getAircraftPath (AIPathfind.cpp:5781-5782), not the ground grid.
        let loco = if is_aircraft {
            gamelogic::ai::pathfind_complete::SURFACE_AIR
        } else if surfaces != 0 {
            surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        let path = self.pathfinding_system.find_path_ex_surfaces(
            start_pos,
            target_position,
            &self.objects,
            is_aircraft,
            loco,
            false,
        );

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
                    // A* found the goal cell (often start==goal after snap).
                    // Walk the remaining intra-cell offset; this is not a
                    // through-obstacle fallback (AIStates.cpp:1577-1585).
                    obj.movement.path = vec![start_pos, target_position];
                    obj.record_host_movement();
                    obj.movement.current_path_index = 1;
                    obj.movement.target_position = Some(target_position);
                    obj.set_status_moving(true);
                    crate::game_logic::host_move_log::record(
                        object_id,
                        Some([target_position.x, target_position.y, target_position.z]),
                    );
                    state_to_apply = Some(ai_state_override.unwrap_or(AIState::Moving));
                }
            } else {
                // C++ null path → STATE_FAILURE (AIStates.cpp:1771-1778).
                // Leave the unit without a through-obstacle march.
                log::debug!(
                    "No path found for {:?} to {:?}; refuse fail-open march",
                    object_id,
                    target_position
                );
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
                // C++ GameLogic.cpp:3677-3718: UpdateModules (including AI/locomotor
                // movement) are skipped while any disabled flag is set and does not
                // intersect getDisabledTypesToProcess (AIUpdate default is NONE).
                // EMP / hack / unmanned / paralyzed / subdued / held-as-is_disabled.
                if obj.is_disabled() {
                    obj.movement.velocity = Vec3::ZERO;
                    obj.record_host_movement();
                    continue;
                }
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
                        let mut desired_angle = (-direction.z).atan2(direction.x);
                        // C++ Locomotor.cpp:1618-1637 legs wander weave.
                        let wander_enabled = obj.wander_width_factor != 0.0
                            || matches!(
                                obj.loco_appearance,
                                LocomotorAppearance::LegsTwo | LocomotorAppearance::Climber
                            );
                        if wander_enabled {
                            let actual = obj.movement.velocity.length();
                            desired_angle += obj.tick_wander_angle_offset(actual);
                        }
                        let current_angle = obj.get_orientation();
                        let mut delta = desired_angle - current_angle;
                        while delta > std::f32::consts::PI {
                            delta -= std::f32::consts::TAU;
                        }
                        while delta < -std::f32::consts::PI {
                            delta += std::f32::consts::TAU;
                        }
                        let max_turn = obj.effective_turn_rate() * dt;
                        let applied = delta.clamp(-max_turn, max_turn);
                        let new_angle = current_angle + applied;
                        obj.set_orientation(new_angle);

                        let dist = horiz(current_pos, flat_target);
                        let mut speed = obj.effective_max_speed();
                        if !obj.no_slow_down_as_approaching_dest {
                            let slow = crate::game_logic::calc_slow_down_dist(
                                obj.movement.velocity.length(),
                                0.0,
                                obj.braking.max(1.0e-3),
                            );
                            if dist < slow {
                                obj.is_braking = true;
                                speed = speed.min(dist / dt.max(1.0e-3));
                            } else {
                                obj.is_braking = false;
                            }
                        }
                        let heading = glam::Vec3::new(new_angle.cos(), 0.0, -new_angle.sin());
                        let target_velocity = heading * speed;
                        let velocity_diff = target_velocity - obj.movement.velocity;
                        let accel = obj.effective_acceleration();
                        let max_accel = if obj.is_braking {
                            obj.braking.max(accel) * dt
                        } else {
                            accel * dt
                        };

                        let new_velocity = if velocity_diff.length() <= max_accel {
                            target_velocity
                        } else {
                            obj.movement.velocity
                                + velocity_diff.normalize_or_zero() * max_accel
                        };

                        obj.movement.velocity = new_velocity;
                        obj.record_host_movement();

                        let new_position = current_pos + new_velocity * dt;
                        let reached_target = dist < 2.0;

                        obj.set_position(new_position);
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

    #[cfg(test)]
    pub fn move_object_with_pathfinding_for_test(
        &mut self,
        object_id: ObjectId,
        target_position: Vec3,
        ai_state_override: Option<AIState>,
    ) {
        self.move_object_with_pathfinding(object_id, target_position, ai_state_override);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{
        GameLogic, GridPos, KindOf, Object, ObjectId, PathfindingGrid, Team, ThingTemplate,
    };
    use glam::Vec3;

    fn ranger_at(id: u32, pos: Vec3) -> Object {
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(id), Team::USA);
        unit.set_position(pos);
        unit
    }

    fn seal_column(logic: &mut GameLogic, cell_x: i32) {
        // Cover the whole host grid (GameLogic world is 512/10 cells).
        for y in -8..80 {
            logic
                .pathfinding_system
                .grid
                .set_blocked(GridPos::new(cell_x, y), true);
        }
    }

    /// C++ `AIInternalMoveToState::update`: `thePath==NULL` → `STATE_FAILURE`
    /// (AIStates.cpp:1771-1778). Host must not `move_to` through a sealed wall,
    /// including the former `distance < 20` skip (hq-3plv).
    #[test]
    fn blocked_astar_does_not_install_direct_through_obstacle_move() {
        let mut logic = GameLogic::new();
        // distance 15 < 20: pre-fix skipped A* and marched through the wall.
        let start = Vec3::new(0.0, 0.0, 0.0);
        let goal = Vec3::new(15.0, 0.0, 0.0);
        let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
        let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
        assert_ne!(start_cell, goal_cell, "short move must span two cells");
        let wall_x = if start_cell.x < goal_cell.x {
            start_cell.x + 1
        } else {
            start_cell.x - 1
        };
        seal_column(&mut logic, wall_x);
        assert!(
            logic
                .pathfinding_system
                .find_path(start, goal, &logic.objects)
                .is_none(),
            "sealed wall must make A* fail"
        );

        let id = ObjectId(9002);
        logic.objects.insert(id, ranger_at(9002, start));
        logic.move_object_with_pathfinding_for_test(id, goal, None);

        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.movement.path.is_empty(),
            "null A* must not install a through-obstacle path"
        );
        assert!(
            obj.movement.target_position.is_none(),
            "null A* must not fail-open to direct move_to"
        );
        assert!(!obj.status.moving);
        assert_ne!(obj.ai_state, AIState::Moving);
        assert_eq!(obj.get_position(), start);
    }

    /// Same contract beyond the old 20-unit skip (AIStates.cpp:1577-1585).
    #[test]
    fn blocked_astar_long_range_does_not_fail_open() {
        let mut logic = GameLogic::new();
        let start = Vec3::new(0.0, 0.0, 0.0);
        let goal = Vec3::new(100.0, 0.0, 0.0);
        let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
        let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
        let wall_x = (start_cell.x + goal_cell.x) / 2;
        seal_column(&mut logic, wall_x);
        assert!(
            logic
                .pathfinding_system
                .find_path(start, goal, &logic.objects)
                .is_none()
        );

        let id = ObjectId(9003);
        logic.objects.insert(id, ranger_at(9003, start));
        logic.move_object_with_pathfinding_for_test(id, goal, None);

        let obj = logic.objects.get(&id).expect("unit");
        assert!(obj.movement.path.is_empty());
        assert!(obj.movement.target_position.is_none());
        assert!(!obj.status.moving);
    }

    #[test]
    fn open_field_path_still_installs_waypoints() {
        let mut logic = GameLogic::new();
        let start = Vec3::new(0.0, 0.0, 0.0);
        let goal = Vec3::new(80.0, 0.0, 0.0);
        let id = ObjectId(9004);
        logic.objects.insert(id, ranger_at(9004, start));
        logic.move_object_with_pathfinding_for_test(id, goal, None);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.movement.path.len() >= 2,
            "open field must still get an A* path"
        );
        assert!(obj.movement.target_position.is_some());
        assert!(obj.status.moving);
    }

    /// C++ Pathfinder::validMovementTerrain uses locomotor surfaces
    /// (AIPathfind.cpp:4779-4782). Water is WATER|AIR only, so a ground
    /// infantry right-click must fail A* across a water wall while an
    /// amphibious unit with SURFACE_WATER succeeds.
    #[test]
    fn right_click_move_uses_unit_locomotor_surfaces() {
        use crate::game_logic::{LOCO_SURFACE_GROUND, LOCO_SURFACE_WATER};
        use gamelogic::ai::pathfind_astar::PathfindCellType;
        let mut logic = GameLogic::new();
        let start = Vec3::new(10.0, 0.0, 10.0);
        let goal = Vec3::new(80.0, 0.0, 10.0);
        let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
        let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
        let wall_x = (start_cell.x + goal_cell.x) / 2;
        for y in -8..80 {
            logic
                .pathfinding_system
                .grid
                .set_cell_type(GridPos::new(wall_x, y), PathfindCellType::Water);
        }

        let ground_id = ObjectId(9101);
        let mut ranger = ranger_at(9101, start);
        ranger.locomotor_surfaces = LOCO_SURFACE_GROUND;
        logic.objects.insert(ground_id, ranger);
        logic.move_object_with_pathfinding_for_test(ground_id, goal, None);
        let ground = logic.objects.get(&ground_id).expect("ranger");
        assert!(
            ground.movement.path.is_empty(),
            "ground-only locomotor must not path through WATER cells"
        );

        let amph_id = ObjectId(9102);
        let mut tmpl = ThingTemplate::new("AmphibHover");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut hover = Object::new(tmpl, amph_id, Team::USA);
        hover.set_position(start);
        hover.locomotor_surfaces = LOCO_SURFACE_GROUND | LOCO_SURFACE_WATER;
        logic.objects.insert(amph_id, hover);
        logic.move_object_with_pathfinding_for_test(amph_id, goal, None);
        let hover = logic.objects.get(&amph_id).expect("hover");
        assert!(
            hover.movement.path.len() >= 2,
            "amphibious locomotor must path WATER cells (AIPathfind.cpp:4750)"
        );
        assert!(hover.movement.target_position.is_some());
    }

    #[test]
    fn live_march_turns_at_turn_rate_not_snap() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9010);
        let mut unit = ranger_at(9010, Vec3::ZERO);
        unit.set_orientation(0.0);
        unit.movement.turn_rate = 1.0; // rad/sec
        unit.movement.max_speed = 10.0;
        unit.movement.acceleration = 100.0;
        unit.movement.target_position = Some(Vec3::new(0.0, 0.0, 20.0));
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        let yaw = obj.get_orientation();
        assert!(
            yaw.abs() > 1e-3 && yaw.abs() < 0.2,
            "must rotate a fraction of a right-angle per frame, yaw={yaw}"
        );
    }

    /// C++ `Pathfinder::worldToGrid` uses `REAL_TO_INT` truncate-toward-zero
    /// (AIPathfind.h:856-858, BaseType.h:213). Host must not round (hq-i1ut).
    #[test]
    fn host_world_to_grid_truncates_like_cpp_real_to_int() {
        let g = PathfindingGrid::new(200.0, 200.0, 10.0);
        assert_eq!(
            g.world_to_grid(Vec3::new(19.9, 0.0, 5.0)),
            GridPos::new(1, 0),
            "19.9/10=1.99 and 5/10=0.5 must truncate, not round"
        );
        assert_eq!(g.world_to_grid(Vec3::new(20.0, 0.0, 0.0)), GridPos::new(2, 0));
        assert_eq!(
            g.world_to_grid(Vec3::new(-19.9, 0.0, -5.1)),
            GridPos::new(-1, 0)
        );
    }

    /// C++ GameLogic.cpp:3677-3718 skips UpdateModules while disabled
    /// (EMP / hack / unmanned / leaflet). Host `update_movement` must halt (hq-psal).
    #[test]
    fn disabled_unit_does_not_advance_in_update_movement() {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
        let mut logic = GameLogic::new();
        let start = Vec3::new(0.0, 0.0, 0.0);
        let id = ObjectId(9010);
        let mut unit = ranger_at(9010, start);
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.movement.max_speed = 30.0;
        unit.movement.acceleration = 10_000.0;
        unit.status.disabled_emp = true;
        logic.objects.insert(id, unit);

        logic.update_movement_for_test(&[id], 1.0 / 30.0);

        let obj = logic.objects.get(&id).expect("unit");
        assert_eq!(obj.get_position(), start, "EMP unit must not integrate");
        assert_eq!(obj.movement.velocity, Vec3::ZERO);
    }

    /// hq-vpocc: ReallyDamaged uses SpeedDamaged, not pristine max.
    #[test]
    fn really_damaged_unit_uses_speed_damaged() {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
        let mut logic = GameLogic::new();
        let id = ObjectId(9020);
        let mut unit = ranger_at(9020, Vec3::ZERO);
        unit.movement.max_speed = 40.0;
        unit.movement.max_speed_damaged = 10.0;
        unit.movement.acceleration = 10_000.0;
        unit.movement.acceleration_damaged = 10_000.0;
        unit.body_damage_state =
            crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged;
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.set_orientation(0.0);
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        let speed = obj.movement.velocity.length();
        assert!(
            speed < 15.0,
            "ReallyDamaged must cap at SpeedDamaged 10, got {speed}"
        );
        assert!(speed > 1.0, "must still move, got {speed}");
    }

    /// hq-fll0r: wander weave offsets heading so two units diverge.
    #[test]
    fn legs_wander_offsets_heading() {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
        let mut logic = GameLogic::new();
        let mut make = |id: u32, inc: f32, increasing: bool| {
            let mut unit = ranger_at(id, Vec3::ZERO);
            unit.movement.max_speed = 30.0;
            unit.movement.acceleration = 10_000.0;
            unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
            unit.set_orientation(0.0);
            unit.loco_appearance = LocomotorAppearance::LegsTwo;
            unit.wander_width_factor = 1.0;
            unit.wander_angle_offset = 0.0;
            unit.wander_offset_increment = inc;
            unit.wander_offset_increasing = increasing;
            unit
        };
        logic.objects.insert(ObjectId(9021), make(9021, 0.2, true));
        logic.objects.insert(ObjectId(9022), make(9022, 0.2, false));
        logic.update_movement_for_test(&[ObjectId(9021), ObjectId(9022)], 1.0 / 30.0);
        let a = logic.objects.get(&ObjectId(9021)).unwrap().get_orientation();
        let b = logic.objects.get(&ObjectId(9022)).unwrap().get_orientation();
        assert!(
            (a - b).abs() > 1e-3,
            "wander phase must split heading, {a} vs {b}"
        );
    }

    /// hq-hh1mu: default braking is BIGNUM, not 50.
    #[test]
    fn object_default_braking_is_bignum() {
        let unit = ranger_at(9023, Vec3::ZERO);
        assert!(
            (unit.braking - 99999.0).abs() < 0.5,
            "C++ BIGNUM default, got {}",
            unit.braking
        );
    }
}
