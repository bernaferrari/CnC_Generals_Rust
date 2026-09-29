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
    ) -> bool {
        let (start_pos, is_aircraft, quick, surfaces, is_crusher) = match self.objects.get(&object_id) {
            Some(obj) => {
                let surfaces = if obj.locomotor_surfaces != 0 {
                    obj.locomotor_surfaces
                } else {
                    Object::default_locomotor_surfaces_for_template(&obj.thing.template)
                };
                let quick = (surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) != 0
                    && !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(obj);
                let aircraft = quick
                    && (obj.is_kind_of(KindOf::Aircraft)
                        || obj.object_type == crate::game_logic::ObjectType::Aircraft)
                    && !obj.is_kind_of(KindOf::Projectile);
                (
                    obj.get_position(),
                    aircraft,
                    quick,
                    surfaces,
                    obj.crusher_level > 0,
                )
            }
            None => return false,
        };

        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        let apply_state = |logic: &mut Self, state: AIState| {
            let already = logic
                .objects
                .get(&object_id)
                .is_some_and(|obj| obj.ai_state == state);
            if already {
                return;
            }
            if decision_auth {
                let ordinal =
                    crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
            }
            if let Some(obj) = logic.objects.get_mut(&object_id) {
                obj.set_ai_state(state);
            }
        };

        if quick && self.try_install_flying_quick_path(object_id, target_position) {
            apply_state(self, ai_state_override.unwrap_or(AIState::Moving));
            return true;
        }
        let loco = if surfaces != 0 {
            surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        let saved_adjust = self.pathfinding_system.adjusts_goal();
        if let Some(unit) = self.objects.get(&object_id) {
            let projectile = unit.is_kind_of(KindOf::Projectile);
            self.pathfinding_system.set_adjust_goal(
                unit.is_final_goal && unit.adjust_destinations && !projectile,
            );
        }
        let path = self.pathfinding_system.find_path_ex_surfaces(
            start_pos,
            target_position,
            &self.objects,
            is_aircraft,
            loco,
            is_crusher,
            Some(object_id),
        );
        self.pathfinding_system.set_adjust_goal(saved_adjust);
        

        let mut need_closest = path.is_none();
        let mut state_to_apply: Option<AIState> = None;
        let fallback_state = ai_state_override.unwrap_or(AIState::Moving);
        let mut accepted_goal: Option<Vec3> = None;
        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ FollowPath onExit clears canPathThroughUnits. A new
            // computePath replaces the factory-exit tunnel. A failed search
            // does not.
            if let Some(waypoints) = path {
                if waypoints.len() >= 2 {
                    obj.can_path_through_units = false;
                    obj.movement.path = waypoints;
                    accepted_goal = obj.movement.path.last().copied();
                    obj.record_host_movement();
                    obj.movement.current_path_index = 1; // skip start node
                    obj.movement.target_position = Some(obj.movement.path[1]);
                    obj.is_attack_path = false;
                    obj.is_exact_path = false;
                    obj.waiting_for_path = false;
                    obj.num_frames_blocked = 0;
                    obj.is_blocked_and_stuck = false;
                    obj.path_timestamp = self.frame;
                    obj.refresh_follow_path_extra_distance();
                    obj.start_move();
                    obj.set_status_moving(true);
                    crate::game_logic::host_move_log::record(
                        object_id,
                        Some([target_position.x, target_position.y, target_position.z]),
                    );
                    state_to_apply = Some(fallback_state.clone());
                } else {
                    obj.can_path_through_units = false;
                    // A* found the goal cell (often start==goal after snap).
                    let dest = waypoints.last().copied().unwrap_or(target_position);
                    obj.movement.path = vec![start_pos, dest];
                    accepted_goal = Some(dest);
                    obj.record_host_movement();
                    obj.movement.current_path_index = 1;
                    obj.movement.target_position = Some(dest);
                    obj.is_attack_path = false;
                    obj.is_exact_path = false;
                    obj.waiting_for_path = false;
                    obj.num_frames_blocked = 0;
                    obj.is_blocked_and_stuck = false;
                    obj.path_timestamp = self.frame;
                    obj.refresh_follow_path_extra_distance();
                    obj.start_move();
                    obj.set_status_moving(true);
                    crate::game_logic::host_move_log::record(
                        object_id,
                        Some([dest.x, dest.y, dest.z]),
                    );
                    state_to_apply = Some(fallback_state.clone());
                }
            } else {
                need_closest = true;
            }
        }
        let closest_installed = need_closest
            && self.try_closest_path_when_none(
                object_id,
                start_pos,
                target_position,
                loco,
                is_crusher,
            );
        if closest_installed {
            state_to_apply = Some(fallback_state);
        } else if need_closest {
            self.note_compute_path_failed(object_id);
            log::debug!(
                "No path found for {:?} to {:?}; refuse fail-open march",
                object_id,
                target_position
            );
        }
        if let Some(last) = accepted_goal {
            self.register_ground_path_goal(object_id, last);
        }
        let installed = state_to_apply.is_some();
        if let Some(state) = state_to_apply {
            apply_state(self, state);
            if !closest_installed {
                self.scoot_allies_off_mover_path(object_id);
            }
        }
        installed
    }

    /// C++ `Pathfinder::moveAllies` after `computePath` installs a route.
    pub(in super::super) fn scoot_allies_off_mover_path(&mut self, object_id: ObjectId) {
        let Some((path, mover_radius)) = self.objects.get(&object_id).and_then(|obj| {
            if obj.is_kind_of(KindOf::NoCollide) {
                None
            } else {
                Some((obj.movement.path.clone(), obj.selection_radius))
            }
        }) else {
            return;
        };
        let nudge_allies =
            self.pathfinding_system
                .allies_to_nudge_off_path(object_id, &path, &self.objects);
        let mover_path = path;
        for ally in nudge_allies {
            let prev_path = self
                .objects
                .get(&ally)
                .and_then(|o| o.move_away_from)
                .and_then(|id| {
                    self.objects.get(&id).and_then(|prev| {
                        (prev.movement.path.len() >= 2).then(|| prev.movement.path.clone())
                    })
                });
            let Some(obj) = self.objects.get(&ally) else {
                continue;
            };
            let from = obj.get_position();
            let surfaces = if obj.locomotor_surfaces != 0 {
                obj.locomotor_surfaces
            } else {
                gamelogic::ai::pathfind_complete::SURFACE_GROUND
            };
            let is_crusher = obj.crusher_level > 0;
            let unit_radius = obj.selection_radius;
            let seeker_player = obj.owner_player_id.or(Some(obj.team as u32));
            let crusher_level = obj.crusher_level;
            let can_tunnel = obj.can_path_through_units;
            let mut yield_path = self.pathfinding_system.get_move_away_from_path(
                from,
                &mover_path,
                prev_path.as_deref(),
                surfaces,
                is_crusher,
                unit_radius,
                mover_radius,
                seeker_player,
                crusher_level,
                false,
            );
            if yield_path.is_none() && !can_tunnel {
                yield_path = self.pathfinding_system.get_move_away_from_path(
                    from,
                    &mover_path,
                    prev_path.as_deref(),
                    surfaces,
                    is_crusher,
                    unit_radius,
                    mover_radius,
                    seeker_player,
                    crusher_level,
                    true,
                );
                if let Some(obj) = self.objects.get_mut(&ally) {
                    obj.can_path_through_units = true;
                }
            }
            if let Some(path) = yield_path {
                if let Some(obj) = self.objects.get_mut(&ally) {
                    let _installed = obj.apply_move_away_path(object_id, &path);
                    if obj.ignore_collisions_until_frame > 0
                        && obj.ignore_collisions_until_frame < 100_000
                    {
                        obj.ignore_collisions_until_frame = self.frame.saturating_add(60);
                    }
                }
            }
        }
    }

    /// C++ `AIInternalMoveToState::update` projectile cheap repath (AIStates.cpp:1845-1849).
    /// Airborne projectiles only. A rebuild stamps the frame; a goal already
    /// within 0.25 keeps the existing clock.
    fn repath_airborne_projectiles(&mut self, object_ids: &[ObjectId]) {
        let goals: Vec<(ObjectId, Vec3)> = object_ids
            .iter()
            .filter_map(|&id| {
                let obj = self.objects.get(&id)?;
                if obj.is_disabled() || obj.host_skip_dead_locomotor() {
                    return None;
                }
                if !obj.is_kind_of(crate::game_logic::KindOf::Projectile)
                    || !matches!(obj.ai_state, crate::game_logic::AIState::Moving)
                {
                    return None;
                }
                let surfaces = if obj.locomotor_surfaces != 0 {
                    obj.locomotor_surfaces
                } else {
                    gamelogic::ai::pathfind_complete::SURFACE_GROUND
                };
                if (surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) == 0
                    || crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(obj)
                {
                    return None;
                }
                obj.requested_destination.map(|goal| (id, goal))
            })
            .collect();
        for (id, goal) in goals {
            if self.try_install_flying_quick_path(id, goal) {
                if let Some(obj) = self.objects.get_mut(&id) {
                    if !obj.movement.path.is_empty() {
                        obj.set_locomotor_goal_position_on_path();
                    }
                }
            }
        }
    }

    /// C++ `AIInternalMoveToState::update` goal-moved repath. `MIN_REPATH_TIME` is 10.
    fn repath_if_move_goal_moved(&mut self, object_ids: &[ObjectId]) {
        const MIN_REPATH_TIME: u32 = 10;
        let due: Vec<(ObjectId, Vec3)> = object_ids
            .iter()
            .filter_map(|&id| {
                let obj = self.objects.get(&id)?;
                if obj.is_disabled() || obj.host_skip_dead_locomotor() {
                    return None;
                }
                if !matches!(obj.ai_state, crate::game_logic::AIState::Moving)
                    && obj.temporary_move_frames == 0
                {
                    return None;
                }
                if obj.waiting_for_path || obj.queue_for_path_frames > 0 {
                    return None;
                }
                let surfaces = if obj.locomotor_surfaces != 0 {
                    obj.locomotor_surfaces
                } else {
                    gamelogic::ai::pathfind_complete::SURFACE_GROUND
                };
                if obj.is_kind_of(crate::game_logic::KindOf::Projectile)
                    && (surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) != 0
                    && !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(obj)
                {
                    return None;
                }
                if self.frame.saturating_sub(obj.path_timestamp) <= MIN_REPATH_TIME {
                    return None;
                }
                let cur = obj.requested_destination?;
                let prev = obj.path_goal_position?;
                if crate::game_logic::is_same_position_residual(obj.get_position(), prev, cur) {
                    return None;
                }
                Some((id, cur))
            })
            .collect();
        for (id, goal) in due {
            if self.assign_unit_path(id, goal, &[]) {
                if let Some(obj) = self.objects.get_mut(&id) {
                    if !obj.movement.path.is_empty() {
                        obj.set_locomotor_goal_position_on_path();
                    }
                }
            }
        }
    }

    /// Update movement for all objects
    pub(in super::super) fn update_movement(&mut self, object_ids: &[ObjectId], dt: f32) {
        self.repath_airborne_projectiles(object_ids);
        self.repath_if_move_goal_moved(object_ids);
        // GameWorld movement authority: path integrate + pose last-write runs in
        // shadow_session_after_host_tick via GameWorld::step_movement. Host still
        // owns path *commands* (move_to / attack-move logs) earlier in the frame.
        // Wave 875: movement authority early-return honesty — GW sole integrate.
        if crate::gameworld_shadow::gameworld_movement_authority_live() {
            // Collide/friction still run on the host; C++ applyMotiveForce(0)
            // flags the object as locomotor-driven even when GW integrates pose.
            self.arm_march_motive_flags(object_ids);
            // Contain exit is not a locomotor integrate — still stream riders.
            self.drain_pending_transport_exits();
            // C++ doLocomotor still stamps AIRBORNE_TARGET after the frame.
            self.stamp_airborne_targets_from_locomotor(object_ids);
            return;
        }

        // C++ m_isBlockedAndStuck → patchPath; requestSafePath → findSafePath Dijkstra.
        let mut repaths: Vec<(ObjectId, Vec<Vec3>, bool)> = Vec::new();
        let mut concessions: Vec<(ObjectId, bool, u32, bool, f32)> = Vec::new();
        let mut yield_stuck: Vec<ObjectId> = Vec::new();
        let mut drop_paths: Vec<ObjectId> = Vec::new();
        let mut quick_miss: Vec<ObjectId> = Vec::new();
        let mut quick_retry: Vec<(ObjectId, bool)> = Vec::new();
        let mut closest_paths: Vec<ObjectId> = Vec::new();
        for &id in object_ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if obj.is_disabled()
                || obj.host_skip_dead_locomotor()
                || obj.is_kind_of(KindOf::Immobile)
            {
                continue;
            }
            let surfaces = if obj.locomotor_surfaces != 0 {
                obj.locomotor_surfaces
            } else {
                gamelogic::ai::pathfind_complete::SURFACE_GROUND
            };
            let is_crusher = obj.crusher_level > 0;
            if obj.is_safe_path {
                if let Some(rep) = obj.requested_victim_id {
                    let from = obj.get_position();
                    let vision = obj.vision_range.max(50.0);
                    let rep2 = obj.safe_path_repulsor2;
                    let fallback = obj.move_away_destination.unwrap_or(from);
                    let is_human = obj
                        .owner_player_id
                        .and_then(|pid| self.players.get(&pid))
                        .map(|p| p.is_local)
                        .unwrap_or(true);
                    let rep_pos = self
                        .objects
                        .get(&rep)
                        .map(|r| r.get_position())
                        .unwrap_or(fallback);
                    let rep2_pos = rep2
                        .and_then(|rid| self.objects.get(&rid).map(|r| r.get_position()))
                        .unwrap_or(rep_pos);
                    if let Some(path) = self.pathfinding_system.find_safe_path_from(
                        from, rep_pos, rep2_pos, vision, surfaces, is_crusher, is_human,
                    ) {
                        repaths.push((id, path, false));
                    }
                }
            } else if obj.move_away_frames > 0 {
                if obj.is_blocked_and_stuck {
                    yield_stuck.push(id);
                }
            } else if obj.queue_for_path_frames == 0
                && !obj.waiting_for_path
                && ((obj.movement.path.is_empty()
                    && obj.requested_destination.is_some()
                    && obj.status.moving)
                    || ((obj.is_blocked_and_stuck || obj.num_frames_blocked > 60)
                        && obj.movement.path.len() >= 2))
            {
                let from = obj.get_position();
                let original = obj.movement.path.clone();
                let ignore = obj.ignored_obstacle_id;
                let stuck = obj.is_blocked_and_stuck;
                let aircraft = obj.is_kind_of(KindOf::Aircraft)
                    || obj.object_type == crate::game_logic::ObjectType::Aircraft;
                let goal = obj
                    .requested_destination
                    .unwrap_or_else(|| *original.last().unwrap());
                self.pathfinding_system.set_ignore_obstacle(ignore);
                if (surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) != 0
                    && !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(obj)
                {
                    let already_there = stuck
                        && obj.movement.path.last().is_some_and(|last| {
                            let dx = goal.x - last.x;
                            let dy = goal.y - last.y;
                            let dz = goal.z - last.z;
                            dx * dx + dy * dy + dz * dz < 0.25
                        });
                    if !already_there {
                    let projectile = obj.is_kind_of(KindOf::Projectile);
                    if aircraft && !projectile {
                        match self.pathfinding_system.find_path_ex_surfaces(
                            from,
                            goal,
                            &self.objects,
                            true,
                            surfaces,
                            is_crusher,
                            Some(id),
                        ) {
                            Some(path) if path.len() >= 2 => {
                                repaths.push((id, path, false));
                                quick_retry.push((id, obj.retry_path));
                            }
                            _ => quick_miss.push(id),
                        }
                    } else {
                        let path = crate::game_logic::pathfinding::PathfindingSystem::leftover_compute_quick_path_nodes(
                            from,
                            goal,
                        );
                        repaths.push((id, path, false));
                        quick_retry.push((id, obj.retry_path));
                    }
                    }
                } else if self
                    .pathfinding_system
                    .leftover_should_force_direct_path_for_off_map_start(from, goal)
                {
                    let path = crate::game_logic::pathfinding::PathfindingSystem::leftover_compute_quick_path_nodes(
                        from,
                        goal,
                    );
                    repaths.push((id, path, false));
                } else if self
                    .pathfinding_system
                    .leftover_should_use_direct_path_for_line_passable_non_final_goal(
                        obj.is_final_goal,
                        from,
                        goal,
                        surfaces,
                        ignore,
                    )
                {
                    let path = crate::game_logic::pathfinding::PathfindingSystem::leftover_compute_quick_path_nodes(
                        from,
                        goal,
                    );
                    repaths.push((id, path, false));
                } else if stuck
                    && !self.pathfinding_system.grid.valid_movement_position(
                        self.pathfinding_system.grid.world_to_grid(goal),
                        self.pathfinding_system.grid.layer_for_destination(goal),
                        surfaces,
                        is_crusher,
                        ignore.map(|oid| oid.0).unwrap_or(0),
                    )
                {
                    if original.is_empty() {
                        let is_human = obj
                            .owner_player_id
                            .and_then(|pid| self.players.get(&pid))
                            .map(|player| player.is_local)
                            .unwrap_or(true);
                        if let Some(path) = self.pathfinding_system.find_closest_path(
                            from,
                            goal,
                            surfaces,
                            is_crusher,
                            is_human,
                            0.0,
                        ) {
                            repaths.push((id, path, false));
                            closest_paths.push(id);
                        } else {
                            drop_paths.push(id);
                        }
                    } else {
                        concessions.push((id, true, surfaces, is_crusher, obj.selection_radius));
                    }
                } else if stuck {
                    match self.pathfinding_system.patch_path(
                        from,
                        &original,
                        surfaces,
                        is_crusher,
                        &self.objects,
                        Some(id),
                    ) {
                        Some(path) => repaths.push((id, path, true)),
                        None if original.is_empty() => {
                            let is_human = obj
                                .owner_player_id
                                .and_then(|pid| self.players.get(&pid))
                                .map(|player| player.is_local)
                                .unwrap_or(true);
                            if let Some(path) = self.pathfinding_system.find_closest_path(
                                from,
                                goal,
                                surfaces,
                                is_crusher,
                                is_human,
                                0.0,
                            ) {
                                repaths.push((id, path, false));
                                closest_paths.push(id);
                            } else {
                                drop_paths.push(id);
                            }
                        }
                        None => concessions.push((
                            id,
                            true,
                            surfaces,
                            is_crusher,
                            obj.selection_radius,
                        )),
                    }
                } else {
                    let cell = self.pathfinding_system.grid.world_to_grid(goal);
                    let ignore_id = ignore.map(|oid| oid.0).unwrap_or(0);
                    let dest_ok = self.pathfinding_system.grid.valid_movement_position(
                        cell,
                        self.pathfinding_system.grid.layer_for_destination(goal),
                        surfaces,
                        is_crusher,
                        ignore_id,
                    );
                    let found = if dest_ok {
                        self.pathfinding_system.find_path_ex_surfaces(
                            from,
                            goal,
                            &self.objects,
                            aircraft,
                            surfaces,
                            is_crusher,
                            Some(id),
                        )
                    } else {
                        None
                    };
                    match found {
                        Some(path) => repaths.push((id, path, false)),
                        None => {
                            let is_human = obj
                                .owner_player_id
                                .and_then(|pid| self.players.get(&pid))
                                .map(|player| player.is_local)
                                .unwrap_or(true);
                            if let Some(path) = self.pathfinding_system.find_closest_path(
                                from,
                                goal,
                                surfaces,
                                is_crusher,
                                is_human,
                                0.0,
                            ) {
                                repaths.push((id, path, false));
                                closest_paths.push(id);
                            } else {
                                drop_paths.push(id);
                            }
                        }
                    }
                }
                self.pathfinding_system.set_ignore_obstacle(None);
            }
        }
        for id in yield_stuck {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.can_path_through_units = true;
            }
        }
        for id in quick_miss {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.movement.path.clear();
                obj.movement.current_path_index = 0;
                obj.movement.target_position = None;
                obj.waiting_for_path = false;
                obj.is_attack_path = false;
                obj.set_locomotor_goal_none();
                obj.num_frames_blocked = 0;
                obj.is_blocked_and_stuck = false;
                obj.path_timestamp = self.frame;
            }
        }
        for id in drop_paths {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.movement.path.clear();
                obj.movement.current_path_index = 0;
                obj.movement.target_position = None;
                obj.waiting_for_path = false;
                obj.is_attack_path = false;
                obj.set_locomotor_goal_none();
                obj.num_frames_blocked = 0;
                obj.is_blocked_and_stuck = false;
                obj.path_timestamp = self.frame;
                obj.retry_path = true;
                obj.set_status_moving(false);
            }
        }
        for (id, path, _was_stuck) in repaths {
            if let Some(obj) = self.objects.get_mut(&id) {
                if path.len() >= 2 {
                    let mut path = path.clone();
                    if obj.ultra_accurate {
                        if let Some(dest) = obj.requested_destination {
                            if let Some(last) = path.last_mut() {
                                *last = dest;
                            }
                        }
                    }
                    obj.movement.path = path;
                    obj.set_locomotor_goal_position_on_path();
                    obj.movement.current_path_index = 1;
                    obj.movement.target_position = Some(obj.movement.path[1]);
                    obj.is_attack_path = false;
                    obj.is_exact_path = false;
                    obj.can_path_through_units = false;
                    obj.waiting_for_path = false;
                    obj.queue_for_path_frames = 0;
                    obj.refresh_follow_path_extra_distance();
                    obj.is_blocked_and_stuck = false;
                    // C++ computePath always clears these after patchPath
                    // (AIUpdate.cpp:1753-1754), including a successful patch.
                    obj.num_frames_blocked = 0;
                    obj.retry_path = false;
                    obj.path_timestamp = self.frame;
                    obj.set_status_moving(true);
                    obj.record_host_movement();
                }
            }
            if path.len() >= 2 {
                let stamp = self.objects.get(&id).is_some_and(|obj| {
                    obj.is_final_goal
                        && crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(obj)
                });
                if stamp {
                    if let Some(last) = self
                        .objects
                        .get(&id)
                        .and_then(|obj| obj.movement.path.last().copied())
                    {
                        self.register_ground_path_goal(id, last);
                    }
                }
            }
            if path.len() >= 2
                && self.objects.get(&id).is_some_and(|obj| {
                    crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(obj)
                })
            {
                self.scoot_allies_off_mover_path(id);
            }
        }
        for id in closest_paths {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.retry_path = true;
            }
        }
        for (id, kept) in quick_retry {
            if let Some(obj) = self.objects.get_mut(&id) {
                obj.retry_path = kept;
            }
        }
        for (id, was_jammed, surfaces, is_crusher, radius) in concessions {
            if !was_jammed {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.num_frames_blocked = 0;
                    obj.is_blocked_and_stuck = false;
                    obj.path_timestamp = self.frame;
                }
                continue;
            }
            let keep_path = self.objects.get(&id).is_some_and(|obj| {
                obj.movement.path.is_empty() || obj.move_away_frames > 0
            });
            if keep_path {
                continue;
            }
            // C++ AIUpdate.cpp:1731-1748 concede: patchPath failed while
            // blocked-and-stuck → destroyPath, snap the final position,
            // locomotor goal none, queue for path in 1s, reset blocked flags.
            let (pos, center) = self
                .objects
                .get(&id)
                .map(|o| {
                    let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(
                        o.selection_radius,
                        self.pathfinding_system.grid.grid_size(),
                    );
                    (o.get_position(), center)
                })
                .unwrap_or((Vec3::ZERO, true));
            let goal_pos = self.pathfinding_system.grid.snap_position(pos, center);
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            obj.movement.path.clear();
            obj.movement.current_path_index = 0;
            obj.movement.target_position = None;
            obj.waiting_for_path = true;
            obj.is_attack_path = false;
            obj.final_position = goal_pos;
            obj.do_final_position = false;
            obj.set_locomotor_goal_none();
            obj.queue_for_path_frames =
                crate::game_logic::host_ai_path_combat_residual_wave105::LOGIC_FRAMES_PER_SECOND_RESIDUAL;
            obj.num_frames_blocked = 0;
            obj.path_timestamp = self.frame;
            obj.is_blocked = false;
            obj.is_blocked_and_stuck = false;
            obj.retry_path = false;
            obj.set_status_moving(false);
            obj.record_host_movement();

        }

        for &id in object_ids {
            let (ground_y, surface_y, climber_ahead_y, cell_type, underwater) = {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
                if obj.is_kind_of(KindOf::Immobile) {
                    continue;
                }
                let pos = obj.get_position();
                let gy = self.terrain_height_at(pos).unwrap_or(obj.ground_height);
                let sy = self.surface_ht_at(pos).unwrap_or(gy);
                let sy = if matches!(
                    obj.loco_behavior_z,
                    LocomotorBehaviorZ::RelativeToGroundAndBuildings
                ) {
                    self.ground_or_structure_height_at(pos, gy)
                } else if matches!(
                    obj.loco_behavior_z,
                    LocomotorBehaviorZ::SmoothRelativeToHighestLayer
                ) {
                    obj.highest_layer_surface_ht(sy)
                } else {
                    sy
                };
                let ahead_y = if matches!(obj.loco_appearance, LocomotorAppearance::Climber) {
                    if let Some(tgt) = obj.movement.target_position {
                        let dx = tgt.x - pos.x;
                        let dz = tgt.z - pos.z;
                        let dlen = (dx * dx + dz * dz).sqrt();
                        let ahead = if dlen > 0.001 {
                            Vec3::new(pos.x + dx / dlen, pos.y, pos.z + dz / dlen)
                        } else {
                            pos
                        };
                        self.terrain_height_at(ahead).unwrap_or(pos.y)
                    } else {
                        pos.y
                    }
                } else {
                    pos.y
                };
                let layer = obj.pathfind_layer;
                let cell_type = self.pathfinding_system.grid.locomotor_cell_type(pos, layer);
                let underwater = self
                    .terrain
                    .as_ref()
                    .is_some_and(|t| t.is_underwater_at_world(pos));
                (gy, sy, ahead_y, cell_type, underwater)
            };
            // C++ Locomotor.cpp:1000-1003. DECK_HEIGHT_OFFSET lowers the
            // airborne sample by the producer's landing deck before both tests.
            let deck_drop = {
                let pid = self.objects.get(&id).and_then(|obj| {
                    if obj.has_object_status_bit("DECK_HEIGHT_OFFSET") {
                        obj.producer_id
                    } else {
                        None
                    }
                });
                pid.and_then(|pid| {
                    self.objects.get(&pid).and_then(|producer| {
                        producer
                            .thing
                            .template
                            .parking_place
                            .as_ref()
                            .map(|pp| pp.landing_deck_height_offset)
                    })
                })
                .unwrap_or(0.0)
            };
            // C++ setFinalPosition(goalPosition): capture the path's last node
            // before stop_moving() clears the path.
            let mut plant_goal: Option<Vec3> = None;
            let mut blocked_out = false;
            let mut restamp_after_move = false;
            'unit: {
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.landing_splat_done = false;
                    if obj.is_disabled() {
                        if obj.status.disabled_freefall {
                            Self::stamp_object_airborne_target(obj, ground_y);
                        } else {
                            obj.movement.velocity = Vec3::ZERO;
                            obj.record_host_movement();
                        }
                        break 'unit;
                    }
                    // C++ Locomotor.cpp:954-958 getIsStunned — no motive walk.
                    // Leave velocity for PhysicsBehavior tumble / shock tick.
                    if obj.is_shock_stunned() {
                        Self::stamp_object_airborne_target(obj, ground_y);
                        break 'unit;
                    }
                    // C++ locoUpdate_moveTowardsPosition always applyMotiveForce(0)
                    // so collide/friction treat the unit as driven (Locomotor.cpp:1010-1014).
                    if obj.locomotor_goal_type == LocoGoalType::Angle {
                        obj.choose_good_locomotor_from_current_set(cell_type);
                        obj.set_locomotor_physics_options();
                        obj.tick_do_locomotor_blocked_frames();
                        // C++ doLocomotor ANGLE: locoUpdate_moveTowardsAngle, not path.
                        obj.do_final_position = false;
                        if obj.face_loco_frame != self.frame || self.frame == 0 {
                            obj.loco_update_move_towards_angle(obj.locomotor_goal_angle, dt);
                            obj.face_loco_frame = self.frame;
                        }
                        // Leftover unused `handle_behavior_z_for` via leftover
                        // `get_surface_ht_at_pt`. Single Z — never pose-Y then double.
                        let sy = obj.leftover_surface_ht(surface_y);
                        Self::apply_live_handle_behavior_z(obj, sy, None);
                        Self::stamp_object_airborne_target(obj, ground_y);
                        obj.cur_max_blocked_speed = 999_999.0;
                        break 'unit;
                    }

                    let has_move_goal =
                        obj.movement.target_position.is_some() || !obj.movement.path.is_empty();
                    // C++ POSITION/ANGLE goals clear m_doFinalPosition.
                    // NONE is the only case that may keep the slide.
                    if has_move_goal && obj.locomotor_goal_type != LocoGoalType::None {
                        obj.do_final_position = false;
                    }
                    let skip_loco_move = obj.waiting_for_path
                        && obj.movement.path.is_empty()
                        && obj.locomotor_goal_type == LocoGoalType::PositionOnPath;
                    // C++ Locomotor.cpp:1055 — treatAsAirborne skips appearance
                    // 2D motive only. handleBehaviorZ, IS_BRAKING, braking cheat,
                    // path advance, hover OVER_WATER, and arrival still run
                    // (hq-hq4t8).
                    let allow_2d_motive = obj.allow_motive_force_while_airborne
                        || !Object::height_treats_as_airborne(
                            obj.get_position().y - ground_y - deck_drop,
                        );
                    if obj.is_rappelling() {
                        // C++ AIRappelState owns Z; handleBehaviorZ must not snap to Y=0.
                        Self::stamp_object_airborne_target(obj, ground_y);
                        break 'unit;
                    }

                    // C++ doLocomotor: chooseGoodLocomotorFromCurrentSet then blocked bookkeeping.
                    obj.choose_good_locomotor_from_current_set(cell_type);
                    obj.set_locomotor_physics_options();
                    obj.tick_do_locomotor_blocked_frames();
                    obj.is_blocked = false;
                    if obj.waiting_for_path
                        && obj.movement.path.is_empty()
                        && obj.locomotor_goal_type == LocoGoalType::PositionOnPath
                    {
                        // C++ returns UPDATE_SLEEP_FOREVER before the airborne
                        // stamp and the blocked-speed reset (AIUpdate.cpp:2161).
                        break 'unit;
                    }
                    blocked_out = obj.num_frames_blocked > 0;
                    if has_move_goal && !skip_loco_move {
                        obj.apply_motive_force(glam::Vec3::ZERO);
                    }
                    let here = obj.get_position();
                    let cell = self.pathfinding_system.grid.world_to_grid(here);
                    let unpinched_cliff = cell_type
                        == gamelogic::ai::pathfind_astar::PathfindCellType::Cliff
                        && !self.pathfinding_system.grid.is_pinched(cell);
                    let in_move = obj.status.moving || has_move_goal;
                    if in_move && unpinched_cliff {
                        let climb =
                            1u128 << crate::game_logic::host_enum_table_residual::climbing_model_bit();
                        let rappel = 1u128
                            << crate::game_logic::host_enum_table_residual::rappelling_model_bit();
                        if obj.moving_backwards {
                            obj.model_condition_bits &= !climb;
                        } else {
                            obj.model_condition_bits &= !rappel;
                        }
                    }
                    if obj.num_frames_blocked > 7 {
                        // > 1/4 s blocked clears MODELCONDITION_MOVING only.
                        obj.clear_moving_model_bits();
                    } else if in_move {
                        obj.stamp_internal_move_cliff_model(unpinched_cliff);
                    }
                    if in_move
                        && !obj.movement.path.is_empty()
                        && obj.locomotor_goal_type != LocoGoalType::PositionExplicit
                        && obj.locomotor_goal_type != LocoGoalType::Angle
                    {
                        obj.set_locomotor_goal_position_on_path();
                    }
                    obj.apply_hover_over_water(underwater);
                    // C++ locoUpdate_moveTowardsPosition:968-977 — non-air invalid
                    // terrain runs fixInvalidPosition and returns (no 2D motive).
                    if has_move_goal && !skip_loco_move {
                        let surfaces = if obj.locomotor_surfaces != 0 {
                            obj.locomotor_surfaces
                        } else {
                            gamelogic::ai::pathfind_complete::SURFACE_GROUND
                        };
                        let air = (surfaces & gamelogic::ai::pathfind_complete::SURFACE_AIR) != 0;
                        if !air && !obj.allow_invalid_position {
                            let pos = obj.get_position();
                            if !valid_movement_terrain_at(
                                &self.pathfinding_system.grid,
                                surfaces,
                                pos,
                                obj.pathfind_layer,
                            ) && try_fix_invalid_position_3x3(
                                obj,
                                &self.pathfinding_system.grid,
                                surfaces,
                            ) {
                                Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                Self::stamp_object_airborne_target(obj, ground_y);
                                if !blocked_out && obj.num_frames_blocked > 1 {
                                    obj.num_frames_blocked = 1;
                                }
                                obj.cur_max_blocked_speed = 999_999.0;
                                break 'unit;
                            }
                        }
                    }
                    if obj.host_skip_dead_locomotor() {
                        if !blocked_out && obj.num_frames_blocked > 1 {
                            obj.num_frames_blocked = 1;
                        }
                        obj.cur_max_blocked_speed = 999_999.0;
                        Self::apply_live_handle_behavior_z(obj, surface_y, None);
                        Self::stamp_object_airborne_target(obj, ground_y);
                        break 'unit;
                    }

                    // Horizontal (XZ) distance — path grid / terrain height use Y separately,
                    // and 3D distance falsely stalls waypoint advance when |ΔY| is large.
                    let horiz = |a: Vec3, b: Vec3| {
                        let dx = a.x - b.x;
                        let dz = a.z - b.z;
                        (dx * dx + dz * dz).sqrt()
                    };
                    let z_motive = matches!(
                        obj.loco_behavior_z,
                        LocomotorBehaviorZ::SurfaceRelativeHeight
                            | LocomotorBehaviorZ::SmoothRelativeToHighestLayer
                            | LocomotorBehaviorZ::AbsoluteHeight
                            | LocomotorBehaviorZ::FixedSurfaceRelativeHeight
                            | LocomotorBehaviorZ::FixedAbsoluteHeight
                            | LocomotorBehaviorZ::RelativeToGroundAndBuildings
                    ) || matches!(
                        obj.loco_appearance,
                        LocomotorAppearance::Hover | LocomotorAppearance::Wings
                    );
                    // C++ moveTowardsPositionClimb latches FLAG_CLIMBING on real
                    // goal Z (host Y). Flattening that Y made dz==0 so CLIMBER
                    // never slowed or reversed (Locomotor.cpp:1711-1739).
                    let keep_goal_y = z_motive
                        || matches!(obj.loco_appearance, LocomotorAppearance::Climber)
                        || obj.host_uses_close_enough_dist_3d();
                    let close_enough = host_close_enough_dist(obj);
                    let close_enough_sanity =
                        4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;

                    // C++ advances the waypoint at most once per frame; the
                    // post-integration advance below is suppressed when the
                    // top-of-loop projection advance already consumed
                    // waypoints.
                    let mut advanced_index_this_frame = false;
                    // C++ moveTowardsPositionLegs compares goalPos.z before the
                    // XZ march flattens it (Locomotor.cpp:1596-1598).
                    let mut raw_goal_y: Option<f32> = None;
                    if !obj.movement.path.is_empty()
                        && obj.movement.current_path_index < obj.movement.path.len()
                    {
                        let current_pos = obj.get_position();
                        // C++ Path::computePointOnPath (AIPathfind.cpp:769-860)
                        // advances the closest segment by forward projection —
                        // the lead points past node[i], so gating the index on
                        // a node radius stalled corner-cutting units in
                        // stop-go loops. Advance while the projection is past
                        // node[i]; arrival is a separate final-goal check.
                        let advanced = Self::advance_path_index_by_projection(
                            &obj.movement.path,
                            obj.movement.current_path_index,
                            current_pos,
                        );
                        if advanced != obj.movement.current_path_index {
                            obj.movement.current_path_index = advanced;
                            obj.refresh_follow_path_extra_distance();
                            advanced_index_this_frame = true;
                        }

                        // Arrival is only the final goal node: locomotor
                        // close-enough distance to the last node, with the C++
                        // ground sanity refusing to plant when 2D to it exceeds
                        // 4*PATHFIND_CELL_SIZE (AIStates.cpp:1887-1904).
                        if obj.movement.current_path_index + 1 >= obj.movement.path.len() {
                            let last = *obj.movement.path.last().unwrap();
                            let plant_ok =
                                z_motive || horiz(current_pos, last) <= close_enough_sanity;
                            if obj.host_locomotor_distance_to_goal(current_pos, last)
                                < close_enough
                                && plant_ok
                            {
                                obj.movement.current_path_index += 1;
                                advanced_index_this_frame = true;
                                let do_evac = obj.pending_evacuate_on_stop;
                                let and_exit = obj.pending_exit_after_evacuate;
                                if obj.holds_air_position_when_idle() {
                                    obj.movement.path.clear();
                                    obj.movement.current_path_index = 0;
                                    obj.movement.target_position = None;
                                    obj.maintain_pos_valid = false;
                                    obj.can_path_through_units = false;
                                    obj.ignored_obstacle_id = None;
                                    obj.queue_for_path_frames = 0;
                                    obj.set_precise_z_pos(false);
                                    obj.set_status_moving(false);
                                    if adjusts_destination_now(obj) {
                                        obj.set_locomotor_goal_none();
                                    }
                                    let _ = obj.loco_maintain_current_position(surface_y, dt);
                                } else {
                                    plant_goal = obj.movement.path.last().copied();
                                    if matches!(obj.ai_state, AIState::AttackMoving) {
                                        obj.movement.path.clear();
                                        obj.movement.current_path_index = 0;
                                        obj.movement.target_position = None;
                                        let crate_leg = obj.requested_victim_id.is_some_and(|id| {
                                            self.host_money_crates.get(id).is_some()
                                        });
                                        if crate_leg {
                                            obj.requested_victim_id = None;
                                        }
                                        let resume = if crate_leg { obj.requested_destination } else { None };
                                        if let Some(dest) = resume {
                                            obj.movement.target_position = Some(dest);
                                            obj.set_status_moving(true);
                                        } else {
                                            obj.set_status_moving(false);
                                            if adjusts_destination_now(obj) {
                                                obj.set_locomotor_goal_none();
                                            }
                                        }
                                    } else {
                                        obj.can_path_through_units = false;
                                        obj.set_precise_z_pos(false);
                                        obj.ignored_obstacle_id = None;
                                        obj.queue_for_path_frames = 0;
                                        obj.stop_moving();
                                        if adjusts_destination_now(obj) {
                                            obj.set_locomotor_goal_none();
                                        }
                                    }
                                }
                                if do_evac {
                                    obj.pending_evacuate_on_stop = true;
                                    obj.pending_exit_after_evacuate = and_exit;
                                }
                                Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                Self::stamp_object_airborne_target(obj, ground_y);
                                break 'unit;
                            }
                        }

                        // C++ computePointOnPath: always try lead; take it only
                        // when isLinePassable (AIPathfind.cpp:910-950).
                        let surfaces = if obj.locomotor_surfaces != 0 {
                            obj.locomotor_surfaces
                        } else {
                            gamelogic::ai::pathfind_complete::SURFACE_GROUND
                        };
                        let is_crusher = obj.crusher_level > 0;
                        let path_tail = obj.movement.path
                            [obj.movement.current_path_index.saturating_sub(1)..]
                            .to_vec();
                        let lead = crate::game_logic::PathfindingSystem::compute_point_on_path_for(
                            current_pos,
                            &path_tail,
                            Some(&self.pathfinding_system.grid),
                            surfaces,
                            is_crusher,
                            obj.owner_player_id,
                            obj.crusher_level,
                            obj.ignored_obstacle_id.map(|id| id.0),
                        );
                        let mut target = lead;
                        raw_goal_y = Some(lead.y);
                        // Ground locos keep XZ march; Z-motive / Climber keep lead Y
                        // so preferredHeight can rise and climb can latch on dz.
                        if !keep_goal_y {
                            target.y = current_pos.y;
                        }
                        obj.movement.target_position = Some(target);
                    }

                    if let Some(target_pos) = obj.movement.target_position {
                        let current_pos = obj.get_position();
                        // XZ heading only — do not dive to Y=0 path cells. Height is
                        // handleBehaviorZ (preferredHeight + surface), not path Y.
                        let mut flat_target = target_pos;
                        flat_target.y = current_pos.y;
                        if matches!(obj.loco_appearance, LocomotorAppearance::Climber) {
                            let dx = flat_target.x - current_pos.x;
                            let dz = flat_target.z - current_pos.z;
                            if dx * dx + dz * dz < 1.0e-4 {
                                if let Some(node) = obj
                                    .movement
                                    .path
                                    .get(obj.movement.current_path_index)
                                    .copied()
                                {
                                    flat_target.x = node.x;
                                    flat_target.z = node.z;
                                }
                            }
                        }
                        let direction = (flat_target - current_pos).normalize_or_zero();

                        if direction.length() > 0.0 {
                            // C++ locoUpdate_moveTowardsPosition clears this first
                            // (Locomotor.cpp:932). Maintain-hover is the zero-delta arm.
                            obj.maintain_pos_valid = false;
                            let mut desired_angle = (-direction.z).atan2(direction.x);
                            // C++ legs wander (Locomotor.cpp:1618). Climb never does.
                            let wander_enabled = !matches!(
                                obj.loco_appearance,
                                LocomotorAppearance::Climber
                            ) && (obj.wander_width_factor != 0.0
                                || matches!(obj.loco_appearance, LocomotorAppearance::LegsTwo));
                            if wander_enabled {
                                let actual = obj.movement.velocity.length();
                                desired_angle += obj.tick_wander_angle_offset(actual);
                            }
                            // C++ AIUpdate.cpp:2145-2148. FAST_AS_POSSIBLE stays at max.
                            // Climb flags live in moveTowardsPositionClimb, after the blocked return.
                            let mut speed = obj.effective_max_speed();
                            if obj.desired_speed < speed {
                                speed = obj.desired_speed.max(0.0);
                            }
                            // C++ POSITION_EXPLICIT skips only the bump-speed cap
                            // (AIUpdate.cpp:2197). It still calls locoUpdate with
                            // &blocked ( :2149 ), so the scrub and pivot run.
                            if obj.locomotor_goal_type != LocoGoalType::PositionExplicit {
                                let scaled = speed * obj.group_speed_factor.clamp(0.0, 1.0);
                                // AIUpdate.cpp:2208 else. Do not clear when the cap branch ran.
                                if blocked_out && scaled <= obj.cur_max_blocked_speed {
                                    blocked_out = false;
                                }
                                speed = obj.apply_do_locomotor_blocked_speed(speed);
                            }
                            let mut loco_blocked = blocked_out;
                            if loco_blocked {
                                if speed > obj.movement.velocity.length() {
                                    loco_blocked = false;
                                }
                                let air = (obj.locomotor_surfaces
                                    & gamelogic::ai::pathfind_complete::SURFACE_AIR)
                                    != 0;
                                if air
                                    && Object::height_treats_as_airborne(current_pos.y - ground_y - deck_drop)
                                {
                                    loco_blocked = false;
                                }
                                // AIUpdate.cpp:2270 clamps whenever this frame's
                                // blocked out-param is false, including the
                                // desiredSpeed > velocity clear at Locomotor.cpp:1018.
                                if !loco_blocked {
                                    blocked_out = false;
                                    if obj.num_frames_blocked > 1 {
                                        obj.num_frames_blocked = 1;
                                    }
                                }
                            }
                            if loco_blocked {
                                obj.scrub_velocity_2d(speed);
                                if obj.wander_width_factor == 0.0 {
                                    // C++ Locomotor.cpp:1035: blocked stays set only
                                    // while the pivot yaw is not TURN_NONE. AIUpdate.cpp:2270
                                    // then clamps m_blockedFrames back to 1.
                                    let (turning, _) = obj.rotate_obj_around_loco_pivot(
                                        flat_target,
                                        obj.effective_turn_rate() * dt,
                                    );
                                    if turning == crate::game_logic::PhysicsTurningType::TurnNone
                                    {
                                        blocked_out = false;
                                        if obj.num_frames_blocked > 1 {
                                            obj.num_frames_blocked = 1;
                                        }
                                    }
                                }
                                obj.record_host_movement();
                                Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                Self::stamp_object_airborne_target(obj, ground_y);
                                obj.cur_max_blocked_speed = 999_999.0;
                                break 'unit;
                            }
                            // C++ moveTowardsPositionClimb (Locomotor.cpp:1704-1739) only if the blocked check did not return.
                            if matches!(obj.loco_appearance, LocomotorAppearance::Climber) {
                                // A blocked line-check returns the segment start,
                                // whose Y is the unit. C++ dz uses goalPos.z
                                // (Locomotor.cpp:1710), the path node.
                                let climb_goal = obj
                                    .movement
                                    .path
                                    .get(obj.movement.current_path_index)
                                    .copied()
                                    .unwrap_or(target_pos);
                                let backwards = obj.update_climber_flags(
                                    current_pos,
                                    climb_goal,
                                    climber_ahead_y,
                                );
                                if backwards {
                                    desired_angle += std::f32::consts::PI;
                                }
                                speed *=
                                    obj.climber_slope_speed_scale(current_pos.y, climber_ahead_y);
                            }

                            let current_angle = obj.get_orientation();
                            let mut delta = desired_angle - current_angle;
                            while delta > std::f32::consts::PI {
                                delta -= std::f32::consts::TAU;
                            }
                            while delta < -std::f32::consts::PI {
                                delta += std::f32::consts::TAU;
                            }
                            let dist = horiz(current_pos, flat_target);
                            // C++ Path::computePointOnPath distAlongPath (AIPathfind.cpp:997)
                            // then locoUpdate_moveTowardsPosition raise (Locomotor.cpp:980-992).
                            // doLocomotor uses flight only when !isDoingGroundMovement
                            // (AIUpdate.cpp:2173-2176). Hover stays on distAlongPath.
                            let path_for_dist = if obj.movement.path.is_empty() {
                                None
                            } else {
                                Some(
                                    &obj.movement.path
                                        [obj.movement.current_path_index.saturating_sub(1)..],
                                )
                            };
                            let airborne = !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(obj);
                            let mut on_path_dist = if obj.host_uses_close_enough_dist_3d() {
                                // Leftover unused `get_locomotor_distance_to_goal`
                                // FROM_CENTER_3D to last node (AIUpdate.cpp:2448-2456).
                                let dest = obj.movement.path.last().copied().unwrap_or(target_pos);
                                obj.host_locomotor_distance_to_goal(current_pos, dest)
                            } else {
                                path_for_dist
                                .map(|wps| {
                                    if airborne {
                                        crate::game_logic::PathfindingSystem::compute_flight_dist_to_goal(
                                            current_pos,
                                            wps,
                                        )
                                    } else {
                                        crate::game_logic::PathfindingSystem::dist_along_path(
                                            current_pos,
                                            wps,
                                        )
                                    }
                                })
                                .unwrap_or(dist)
                            };
                            // C++ passes onPathDistToGoal + getPathExtraDistance()
                            // into locoUpdate, so the clear and the 2× raise both
                            // see it (AIUpdate.cpp:2219, Locomotor.cpp:941 and :985).
                            on_path_dist += obj.path_extra_distance.max(0.0);
                            if obj.locomotor_goal_type == LocoGoalType::PositionExplicit {
                                // C++ AIUpdate.cpp:2150. Explicit goals pass 0, not the path.
                                on_path_dist = 0.0;
                            }
                            // C++ reads OBJECT_STATUS_BRAKING before setStatus.
                            // Far-clear and the 2× raise update the locomotor flag
                            // only; the pose cheat must keep this earlier value.
                            let was_braking = obj.is_braking;
                            // C++ Locomotor.cpp:941-946 — far-from-goal IS_BRAKING
                            // clear is unconditional (NO_SLOW_DOWN only skips the
                            // appearance approach-brake, not this un-latch).
                            let braking = obj.braking;
                            if braking > 0.0 {
                                let max_speed = obj.effective_max_speed();
                                let dist_to_stop = (max_speed / braking) * max_speed / 2.0;
                                let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
                                if on_path_dist > cell && on_path_dist > dist_to_stop {
                                    obj.is_braking = false;
                                    obj.braking_factor = 1.0;
                                }
                            }
                            on_path_dist = obj.raise_on_path_dist_to_goal(dist, on_path_dist);
                            // C++ Locomotor.cpp:1047-1049 is after the :989 2× latch.
                            // Wings clear wins, including when the unit is close.
                            // was_braking already sampled the pose bit.
                            if matches!(obj.loco_appearance, LocomotorAppearance::Wings) {
                                obj.is_braking = false;
                            }
                            // C++ moveTowardsPositionLegs returns without scrubbing
                            // velocity. Wheels, treads, and other do not check this.
                            let goal_y = raw_goal_y.unwrap_or(target_pos.y);
                            if matches!(obj.loco_appearance, LocomotorAppearance::LegsTwo)
                                && obj.downhill_only_blocks_goal(current_pos.y, goal_y)
                            {
                                obj.record_host_movement();
                                Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                Self::stamp_object_airborne_target(obj, ground_y);
                                break 'unit;
                            }
                            // C++ locoUpdate_moveTowardsPosition LOCO_THRUST
                            // → moveTowardsPositionThrust (Locomotor.cpp:1104-1107).
                            // Live `move_towards_thrust` already ports the 3D mover
                            // (hq-sw06m); production march must dispatch it (hq-zx7lx).
                            if matches!(obj.loco_appearance, LocomotorAppearance::Thrust) {
                                obj.move_towards_thrust(target_pos, on_path_dist, speed, dt);
                                obj.notify_terrain_trees_on_unit_move();
                                Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                Self::stamp_object_airborne_target(obj, ground_y);
                                let mut reached_target = obj
                                    .host_locomotor_distance_to_goal(current_pos, target_pos)
                                    < close_enough;
                                if reached_target {
                                    let finishing = obj.movement.path.is_empty()
                                        || obj.movement.current_path_index + 1
                                            >= obj.movement.path.len();
                                    if finishing {
                                        if let Some(last) = obj.movement.path.last().copied() {
                                            if !z_motive
                                                && horiz(current_pos, last) > close_enough_sanity
                                            {
                                                reached_target = false;
                                            }
                                        }
                                    }
                                }
                                if reached_target {
                                    if obj.movement.path.is_empty()
                                        || obj.movement.current_path_index + 1
                                            >= obj.movement.path.len()
                                    {
                                        if obj.holds_air_position_when_idle() {
                                            obj.movement.path.clear();
                                            obj.movement.current_path_index = 0;
                                            obj.movement.target_position = None;
                                            obj.maintain_pos_valid = false;
                                            obj.can_path_through_units = false;
                                            obj.ignored_obstacle_id = None;
                                            obj.queue_for_path_frames = 0;
                                            obj.set_precise_z_pos(false);
                                            obj.set_status_moving(false);
                                            if adjusts_destination_now(obj) {
                                                obj.set_locomotor_goal_none();
                                            }
                                            let _ =
                                                obj.loco_maintain_current_position(surface_y, dt);
                                        } else {
                                            plant_goal =
                                                obj.movement.path.last().copied();
                                            if matches!(obj.ai_state, AIState::AttackMoving) {
                                                obj.movement.path.clear();
                                                obj.movement.current_path_index = 0;
                                                obj.movement.target_position = None;
                                                let crate_leg = obj.requested_victim_id.is_some_and(|id| {
                                                    self.host_money_crates.get(id).is_some()
                                                });
                                                if crate_leg {
                                                    obj.requested_victim_id = None;
                                                }
                                                let resume = if crate_leg { obj.requested_destination } else { None };
                                                if let Some(dest) = resume {
                                                    obj.movement.target_position = Some(dest);
                                                    obj.set_status_moving(true);
                                                } else {
                                                    obj.set_status_moving(false);
                                                    if adjusts_destination_now(obj) {
                                                        obj.set_locomotor_goal_none();
                                                    }
                                                }
                                            } else {
                                                obj.can_path_through_units = false;
                                                obj.set_precise_z_pos(false);
                                                obj.ignored_obstacle_id = None;
                                                obj.queue_for_path_frames = 0;
                                                obj.stop_moving();
                                                if adjusts_destination_now(obj) {
                                                    obj.set_locomotor_goal_none();
                                                }
                                            }
                                        }
                                    } else {
                                        obj.movement.current_path_index += 1;
                                        obj.refresh_follow_path_extra_distance();
                                        let mut next =
                                            obj.movement.path[obj.movement.current_path_index];
                                        if !keep_goal_y {
                                            next.y = obj.get_position().y;
                                        }
                                        obj.movement.target_position = Some(next);
                                    }
                                }
                                break 'unit;
                            }
                            // C++ moveTowardsPositionTreads/Legs/Climb angleCoeff
                            // (Locomotor.cpp:1170-1180, 1638-1646, 1760-1767).
                            if matches!(
                                obj.loco_appearance,
                                LocomotorAppearance::Treads
                                    | LocomotorAppearance::LegsTwo
                                    | LocomotorAppearance::Climber
                            ) {
                                let mut angle_coeff = delta.abs() / std::f32::consts::FRAC_PI_4;
                                if angle_coeff > 1.0 {
                                    angle_coeff = 1.0;
                                }
                                speed = (1.0 - angle_coeff) * speed;
                                // Treads-only near-goal tight pivot (Locomotor.cpp:1190-1192).
                                if matches!(obj.loco_appearance, LocomotorAppearance::Treads) {
                                    let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
                                    if dist < 2.0 * cell && angle_coeff > 0.05 {
                                        speed = obj.forward_speed_2d() * 0.6;
                                    }
                                }
                            }
                            let wheeled = matches!(
                                obj.loco_appearance,
                                LocomotorAppearance::WheelsFour | LocomotorAppearance::Motorcycle
                            );
                            // Climber descent already set obj.moving_backwards
                            // (update_climber_flags). Leftover dir_sign=-1: face
                            // away and drive reverse toward the goal.
                            let mut reverse_aim: Option<Vec3> = None;
                            let mut move_backwards =
                                matches!(obj.loco_appearance, LocomotorAppearance::Climber)
                                    && obj.moving_backwards;
                            if wheeled {
                                // C++ Locomotor.cpp:1292-1323 reverse / 3pt + turn-speed cap.
                                let actual_stopped = obj.movement.velocity.x.abs() < 1e-4
                                    && obj.movement.velocity.z.abs() < 1e-4;
                                let major = if obj.thing.template.geometry_info.authored {
                                    obj.thing.template.geometry_info.major_radius
                                } else {
                                    obj.selection_radius.max(1.0)
                                };
                                let on_path = on_path_dist;
                                if actual_stopped {
                                    obj.moving_backwards = false;
                                    if obj.can_move_backward
                                        && delta.abs() > std::f32::consts::FRAC_PI_2
                                    {
                                        obj.moving_backwards = true;
                                        obj.doing_three_point_turn = on_path > 5.0 * major;
                                        obj.record_host_locomotor();
                                    }
                                }
                                if obj.moving_backwards {
                                    if delta.abs() < std::f32::consts::FRAC_PI_2 {
                                        obj.moving_backwards = false;
                                        obj.record_host_locomotor();
                                    } else {
                                        move_backwards = true;
                                        // C++ Locomotor.cpp:1306-1310. Far goals keep
                                        // facing the dest; nearby reverse mirrors it.
                                        obj.doing_three_point_turn = on_path > 5.0 * major;
                                        if !obj.doing_three_point_turn {
                                            // Speed cap uses center heading + pi
                                            // (Locomotor.cpp:1277, :1309). The reflected
                                            // point is only the later pivot rotate.
                                            reverse_aim = Some(Vec3::new(
                                                current_pos.x - (flat_target.x - current_pos.x),
                                                current_pos.y,
                                                current_pos.z - (flat_target.z - current_pos.z),
                                            ));
                                            desired_angle += std::f32::consts::PI;
                                            delta = desired_angle - current_angle;
                                            while delta > std::f32::consts::PI {
                                                delta -= std::f32::consts::TAU;
                                            }
                                            while delta < -std::f32::consts::PI {
                                                delta += std::f32::consts::TAU;
                                            }
                                        }
                                    }
                                }
                                // C++ Locomotor.cpp:1316-1323 SMALL_TURN cap on
                                // desiredSpeed BEFORE approach-brake (:1393-1430).
                                // Once IS_BRAKING latches, goalSpeed is actual-braking
                                // and must not recap to turnSpeed (hq-7soel).
                                let turn_speed = obj.wheeled_turn_speed_floor();
                                if delta.abs() > std::f32::consts::PI / 20.0 && speed > turn_speed {
                                    speed = turn_speed;
                                }
                                // C++ Locomotor.cpp:1340-1389 — 15° half-second
                                // validMovementTerrain probe. Rotate-only + zero
                                // motive when the projected arc is impassable.
                                let frames =
                                    game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
                                let mut actual = obj.forward_speed_2d();
                                if move_backwards {
                                    actual = -actual;
                                }
                                let loco_pos =
                                    Vec3::new(current_pos.x, -current_pos.z, current_pos.y);
                                let surfaces = if obj.locomotor_surfaces != 0 {
                                    obj.locomotor_surfaces
                                } else {
                                    gamelogic::ai::pathfind_complete::SURFACE_GROUND
                                };
                                let grid = &self.pathfinding_system.grid;
                                if gamelogic::locomotor::Locomotor::wheels_look_ahead_blocked(
                                    loco_pos,
                                    current_angle,
                                    delta,
                                    speed / frames,
                                    actual / frames,
                                    turn_speed / frames,
                                    obj.effective_turn_rate() / frames,
                                    |pos| {
                                        let host = Vec3::new(pos.x, pos.z, -pos.y);
                                        valid_movement_terrain_at(grid, surfaces, host, obj.pathfind_layer)
                                    },
                                ) {
                                    // C++ rotateTowardsPosition (full maxTurnRate,
                                    // no wheeled turnFactor) + applyMotiveForce(0).
                                    let _ = obj.rotate_obj_around_loco_pivot(
                                        flat_target,
                                        obj.effective_turn_rate() * dt,
                                    );
                                    obj.record_host_movement();
                                    Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                    Self::stamp_object_airborne_target(obj, ground_y);
                                    break 'unit;
                                }
                            }
                            // C++ moveTowardsPositionOther tests the slide against
                            // desiredSpeed before calcSlowDownDist lowers goalSpeed.
                            let slide_goal_speed = speed;
                            if !obj.no_slow_down_as_approaching_dest {
                                // C++ getForwardSpeed2D stays signed. Wheels and climber
                                // negate it when MOVING_BACKWARDS (Locomotor.cpp:1326, :1771).
                                let mut actual_speed = obj.forward_speed_2d();
                                if move_backwards {
                                    actual_speed = -actual_speed;
                                }
                                speed = obj.apply_cpp_approach_brake(
                                    on_path_dist,
                                    actual_speed,
                                    speed,
                                    self.frame,
                                );
                            }
                            // C++ Locomotor.cpp:2344-2361 ULTRA_ACCURATE slide-into-place.
                            // Appearance 2D (rotate + motive Euler) is skipped when
                            // treatAsAirborne && !AllowAirborneMotiveForce (hq-hq4t8).
                            let march_from = obj.get_position();
                            let mut new_position = march_from;
                            // C++ Locomotor.cpp:1054, before the airborne-motive gate at :1055.
                            obj.physics_turning = crate::game_logic::PhysicsTurningType::TurnNone;
                            if allow_2d_motive {
                                // Leftover Other/Hover (move_ground.rs:511-531):
                                // threshold = per-frame goalSpeed * parse_duration_real.
                                let slide_other_or_hover = matches!(
                                    obj.loco_appearance,
                                    LocomotorAppearance::Other | LocomotorAppearance::Hover
                                );
                                let frames =
                                    game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32;
                                let slide_thresh =
                                    (slide_goal_speed / frames) * obj.ultra_accurate_slide_factor;
                                let sliding = slide_other_or_hover
                                    && obj.ultra_accurate
                                    && obj.ultra_accurate_slide_factor > 0.0
                                    && (flat_target.x - current_pos.x).abs() <= slide_thresh
                                    && (flat_target.z - current_pos.z).abs() <= slide_thresh;
                                let new_angle = if sliding {
                                    current_angle
                                } else {
                                    // Wheels pivot around flat_target, or the reflected
                                    // point on a nearby reverse (Locomotor.cpp:1447-1454).
                                    // Legs and climbers keep a far point on the biased
                                    // heading so wander and the climb π flip survive.
                                    let rotate_goal = if wheeled {
                                        reverse_aim.unwrap_or(flat_target)
                                    } else {
                                        glam::Vec3::new(
                                            current_pos.x + desired_angle.cos() * 1000.0,
                                            current_pos.y,
                                            current_pos.z + (-desired_angle.sin()) * 1000.0,
                                        )
                                    };
                                    let (_turning, _rel) =
                                        obj.rotate_towards_position(rotate_goal, dt);
                                    obj.get_orientation()
                                };

                                let signed_speed = if move_backwards { -speed } else { speed };
                                let heading = if sliding {
                                    glam::Vec3::new(direction.x, 0.0, direction.z)
                                } else {
                                    glam::Vec3::new(new_angle.cos(), 0.0, -new_angle.sin())
                                };
                                let accel = obj.effective_acceleration();
                                // Host speed is dist/sec. C++ speedDelta is dist/frame
                                // and m_vel += m_accel has no extra dt, so one host
                                // frame adds min(A*dt, gap) along the force direction.
                                let forward = obj.movement.velocity.dot(heading);
                                let speed_delta = signed_speed - forward;
                                if speed_delta != 0.0 {
                                    let brake_accel = if matches!(
                                        obj.loco_appearance,
                                        LocomotorAppearance::Treads
                                            | LocomotorAppearance::WheelsFour
                                            | LocomotorAppearance::Motorcycle
                                    ) {
                                        obj.braking_factor * obj.braking
                                    } else {
                                        obj.braking
                                    };
                                    // Wheels (Locomotor.cpp:1462-1472) and climb
                                    // (:1786-1796): speedDelta < 0 speeds up
                                    // backward with -maxAcceleration. Braking
                                    // only slows a reverse that is already too
                                    // fast. Climb uses plain braking, not
                                    // brakingFactor. Treads do not flip.
                                    let reverse_rate = move_backwards
                                        && (wheeled
                                            || matches!(
                                                obj.loco_appearance,
                                                LocomotorAppearance::Climber
                                            ));
                                    let speeding_up = if reverse_rate {
                                        speed_delta < 0.0
                                    } else {
                                        speed_delta > 0.0
                                    };
                                    let rate = if speeding_up { accel } else { brake_accel };
                                    let step = rate * dt;
                                    let applied = if step > speed_delta.abs() {
                                        speed_delta
                                    } else if speed_delta > 0.0 {
                                        step
                                    } else {
                                        -step
                                    };
                                    obj.movement.velocity += heading * applied;
                                }
                                obj.invalidate_velocity_magnitude();
                                obj.record_host_movement();

                                // Keep the rotated pose. Physics integrates once,
                                // after the cheat and the projectile force-on.
                                new_position = obj.get_position();
                            }
                            let mut reached_target = obj
                                .host_locomotor_distance_to_goal(current_pos, target_pos)
                                < close_enough;
                            if reached_target {
                                let finishing = obj.movement.path.is_empty()
                                    || obj.movement.current_path_index + 1
                                        >= obj.movement.path.len();
                                if finishing {
                                    if let Some(last) = obj.movement.path.last().copied() {
                                        if !z_motive
                                            && horiz(current_pos, last) > close_enough_sanity
                                        {
                                            reached_target = false;
                                        }
                                    }
                                }
                            }

                            obj.set_position(new_position);
                            // C++ Object.cpp:2580-2583 notifyTerrainObjectMoved →
                            // W3DTreeBuffer::unitMoved (topple/push). set_position
                            // also notifies on integer XY change for GameWorld writeback.
                            obj.notify_terrain_trees_on_unit_move();
                            Self::apply_live_handle_behavior_z(obj, surface_y, None);
                            if was_braking {
                                // C++ :981 dx/dy/dz are not recomputed. :1096 reads
                                // the post-handleBehaviorZ position and adds that
                                // same entry delta (Locomotor.cpp:1102-1135).
                                let posed = obj.get_position();
                                let entry_target = posed + (target_pos - current_pos);
                                let cheated = obj.braking_cheat_step(posed, entry_target, dt);
                                obj.set_position(cheated);
                                if obj.is_kind_of(KindOf::Projectile)
                                    || obj.object_type == crate::game_logic::ObjectType::Projectile
                                {
                                    obj.is_braking = true;
                                }
                            }
                            if allow_2d_motive {
                                // C++ PhysicsUpdate.cpp:649 after setStatus. One step.
                                let posed = obj.get_position();
                                let projectile = obj.is_kind_of(KindOf::Projectile)
                                    || obj.object_type
                                        == crate::game_logic::ObjectType::Projectile;
                                let stepped = if obj.is_braking && projectile {
                                    posed
                                } else if obj.is_braking {
                                    posed + Vec3::new(0.0, obj.movement.velocity.y * dt, 0.0)
                                } else {
                                    posed + obj.movement.velocity * dt
                                };
                                obj.set_position(stepped);
                            }
                            if matches!(obj.loco_appearance, LocomotorAppearance::Hover) {
                                // C++ moveTowardsPositionHover checks water after the
                                // 2D step (Locomotor.cpp:1869), not the pre-move cell.
                                let p = obj.get_position();
                                let under = self
                                    .terrain
                                    .as_ref()
                                    .is_some_and(|t| t.is_underwater_at_world(p));
                                obj.apply_hover_over_water(under);
                            }
                            if reached_target {
                                // Only stop when there is no further path waypoint.
                                // Mid-path "reached" is handled by index advance above.
                                if obj.movement.path.is_empty()
                                    || obj.movement.current_path_index + 1
                                        >= obj.movement.path.len()
                                {
                                    if obj.holds_air_position_when_idle() {
                                        obj.end_temporary_move_overlay();
                                        obj.maintain_pos_valid = false;
                                        obj.can_path_through_units = false;
                                        obj.ignored_obstacle_id = None;
                                        obj.set_precise_z_pos(false);
                                        obj.set_locomotor_goal_none();
                                        let _ = obj.loco_maintain_current_position(surface_y, dt);
                                    } else if obj.temporary_move_frames > 0
                                        && !matches!(obj.ai_state, AIState::Moving)
                                    {
                                        plant_goal = Some(obj.get_position());
                                        obj.end_temporary_move_overlay();
                                        obj.ignored_obstacle_id = None;
                                        obj.set_locomotor_goal_none();
                                    } else {
                                        plant_goal =
                                            obj.movement.path.last().copied();
                                        if matches!(obj.ai_state, AIState::AttackMoving) {
                                            obj.movement.path.clear();
                                            obj.movement.current_path_index = 0;
                                            obj.movement.target_position = None;
                                            let crate_leg = obj.requested_victim_id.is_some_and(|id| {
                                                self.host_money_crates.get(id).is_some()
                                            });
                                            if crate_leg {
                                                obj.requested_victim_id = None;
                                            }
                                            let resume = if crate_leg { obj.requested_destination } else { None };
                                            if let Some(dest) = resume {
                                                obj.movement.target_position = Some(dest);
                                                obj.set_status_moving(true);
                                            } else {
                                                obj.set_status_moving(false);
                                                if adjusts_destination_now(obj) {
                                                    obj.set_locomotor_goal_none();
                                                }
                                            }
                                        } else {
                                            obj.can_path_through_units = false;
                                            obj.set_precise_z_pos(false);
                                            obj.ignored_obstacle_id = None;
                                            obj.queue_for_path_frames = 0;
                                            obj.stop_moving();
                                            if adjusts_destination_now(obj) {
                                                obj.set_locomotor_goal_none();
                                            }
                                        }
                                    }
                                } else if !advanced_index_this_frame {
                                    // Same projection advance as the
                                    // top-of-loop; reaching the lead's vicinity
                                    // counts as at least the current node.
                                    let reached_pos = obj.get_position();
                                    obj.movement.current_path_index = Self::advance_path_index_by_projection(
                                        &obj.movement.path,
                                        obj.movement.current_path_index,
                                        reached_pos,
                                    )
                                    .max(obj.movement.current_path_index + 1);
                                    obj.refresh_follow_path_extra_distance();
                                    let mut next =
                                        obj.movement.path[obj.movement.current_path_index];
                                    if !keep_goal_y {
                                        next.y = obj.get_position().y;
                                    }
                                    obj.movement.target_position = Some(next);
                                }
                            }
                        } else {
                            // Already on target (zero horizontal delta) — still hold height.
                            // C++ locoUpdate_maintainCurrentPosition: appearance
                            // then handleBehaviorZ (Locomotor.cpp:2433-2474).
                            if matches!(obj.loco_appearance, LocomotorAppearance::Wings) {
                                let _ = obj.loco_maintain_current_position(surface_y, dt);
                                let sy = obj.leftover_surface_ht(surface_y);
                                Self::apply_live_handle_behavior_z(
                                    obj,
                                    sy,
                                    obj.maintain_pos.map(|p| p.y),
                                );
                            } else {
                                Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                if matches!(
                                    obj.loco_appearance,
                                    LocomotorAppearance::Hover | LocomotorAppearance::Thrust
                                ) {
                                    let _ = obj.loco_maintain_current_position(surface_y, dt);
                                } else {
                                    let _ = obj.loco_maintain_current_position(surface_y, dt);
                                }
                            }
                            // C++ friend_endingMove only runs from the move state.
                            // Idle maintain (goal still coincident) must not clear
                            // the queue or the ignored obstacle.
                            if (matches!(obj.ai_state, AIState::Moving | AIState::AttackMoving)
                                || obj.temporary_move_frames > 0)
                                && (obj.movement.path.is_empty()
                                    || obj.movement.current_path_index + 1
                                        >= obj.movement.path.len())
                            {
                                if obj.holds_air_position_when_idle() {
                                    obj.end_temporary_move_overlay();
                                    obj.can_path_through_units = false;
                                    obj.ignored_obstacle_id = None;
                                    obj.set_precise_z_pos(false);
                                    obj.set_locomotor_goal_none();
                                } else if obj.temporary_move_frames > 0
                                    && !matches!(obj.ai_state, AIState::Moving)
                                {
                                    plant_goal = Some(obj.get_position());
                                    obj.end_temporary_move_overlay();
                                    obj.ignored_obstacle_id = None;
                                    obj.set_locomotor_goal_none();
                                } else {
                                    plant_goal = obj.movement.path.last().copied();
                                    if matches!(obj.ai_state, AIState::AttackMoving) {
                                        obj.movement.path.clear();
                                        obj.movement.current_path_index = 0;
                                        obj.movement.target_position = None;
                                        let crate_leg = obj.requested_victim_id.is_some_and(|id| {
                                            self.host_money_crates.get(id).is_some()
                                        });
                                        if crate_leg {
                                            obj.requested_victim_id = None;
                                        }
                                        let resume = if crate_leg { obj.requested_destination } else { None };
                                        if let Some(dest) = resume {
                                            obj.movement.target_position = Some(dest);
                                            obj.set_status_moving(true);
                                        } else {
                                            obj.set_status_moving(false);
                                            if adjusts_destination_now(obj) {
                                                obj.set_locomotor_goal_none();
                                            }
                                        }
                                    } else {
                                        obj.can_path_through_units = false;
                                        obj.set_precise_z_pos(false);
                                        obj.ignored_obstacle_id = None;
                                        obj.queue_for_path_frames = 0;
                                        obj.stop_moving();
                                        if adjusts_destination_now(obj) {
                                            obj.set_locomotor_goal_none();
                                        }
                                    }
                                }
                            }

                        }
                    } else {
                        leftover_settle_final_position_on_object(obj);
                        // Idle hover / wings: C++ appearance then handleBehaviorZ.
                        if matches!(obj.loco_appearance, LocomotorAppearance::Wings) {
                            let _ = obj.loco_maintain_current_position(surface_y, dt);
                            let sy = obj.leftover_surface_ht(surface_y);
                            Self::apply_live_handle_behavior_z(
                                obj,
                                sy,
                                obj.maintain_pos.map(|p| p.y),
                            );
                        } else {
                            Self::apply_live_handle_behavior_z(obj, surface_y, None);
                            if matches!(
                                obj.loco_appearance,
                                LocomotorAppearance::Hover | LocomotorAppearance::Thrust
                            ) {
                                let _ = obj.loco_maintain_current_position(surface_y, dt);
                            } else {
                                let _ = obj.loco_maintain_current_position(surface_y, dt);
                            }
                        }
                    }
                    // C++ AIUpdate.cpp:2270. Clamp from this frame's locomotor
                    // `blocked` local, not the collision flag cleared at :2125.
                    if !blocked_out && obj.num_frames_blocked > 1 {
                        obj.num_frames_blocked = 1;
                    }
                    // C++ AIUpdate.cpp:2281. The cap is only for this frame's collision.
                    obj.cur_max_blocked_speed = 999_999.0;
                    restamp_after_move = true;
                }
            }
            if restamp_after_move {
                // C++ PhysicsUpdate.cpp:739-743 samples getLayerHeight on the
                // post-integrate position and adds the carrier deck.
                let sample = self.objects.get(&id).map(|obj| {
                    (
                        obj.get_position(),
                        obj.ground_height,
                        obj.has_object_status_bit("DECK_HEIGHT_OFFSET"),
                        obj.producer_id,
                    )
                });
                if let Some((pos, fallback, deck, producer)) = sample {
                    let mut gy = self.terrain_height_at(pos).unwrap_or(fallback);
                    if deck {
                        if let Some(pid) = producer {
                            if let Some(extra) = self.objects.get(&pid).and_then(|carrier| {
                                carrier
                                    .thing
                                    .template
                                    .parking_place
                                    .as_ref()
                                    .map(|pp| pp.landing_deck_height_offset)
                            }) {
                                gy += extra;
                            }
                        }
                    }
                    if let Some(obj) = self.objects.get_mut(&id) {
                        Self::stamp_object_airborne_target(obj, gy);
                    }
                }
            }
            if let Some(goal) = plant_goal {
                self.apply_arrival_goal_snap(id, Some(goal));
            }
        }

        self.drain_pending_transport_exits();
    }
    /// Closest-segment projection on the host XZ polyline — the same loop as
    /// `PathfindingSystem::compute_point_on_path_for`
    /// (pathfinding/system_routes.rs; C++ AIPathfind.cpp:769-851). Returns the
    /// closest segment's start-node index and clamped projection parameter so
    /// the waypoint advance and the lead-point computation agree.
    fn closest_segment_projection(pos: Vec3, waypoints: &[Vec3]) -> (usize, f32) {
        let mut best_d2 = f32::MAX;
        let mut best_seg = 0usize;
        let mut best_t = 0.0f32;
        for i in 0..waypoints.len().saturating_sub(1) {
            let a = &waypoints[i];
            let b = &waypoints[i + 1];
            let sx = b.x - a.x;
            let sz = b.z - a.z;
            let len_sqr = sx * sx + sz * sz;
            let t = if len_sqr <= 1.0e-8 {
                0.0
            } else {
                let tx = pos.x - a.x;
                let tz = pos.z - a.z;
                ((tx * sx + tz * sz) / len_sqr).clamp(0.0, 1.0)
            };
            let px = a.x + sx * t;
            let pz = a.z + sz * t;
            let dx = pos.x - px;
            let dz = pos.z - pz;
            let d2 = dx * dx + dz * dz;
            if d2 < best_d2 {
                best_d2 = d2;
                best_seg = i;
                best_t = t;
            }
        }
        (best_seg, best_t)
    }

    /// C++ tracks the path by closest segment with forward projection
    /// (AIPathfind.cpp:769-860): intermediate nodes have no arrival radius —
    /// only the final goal uses closeEnoughDist (AIStates.cpp:1885-1904).
    /// Returns the first node index whose projection the unit has not passed.
    /// node[i] counts as passed when the closest segment starts at i or later,
    /// or is the segment ending at i with t clamped at 1.0.
    fn advance_path_index_by_projection(path: &[Vec3], current: usize, pos: Vec3) -> usize {
        if current + 1 >= path.len() {
            return current;
        }
        let start = current.saturating_sub(1);
        let (best_seg, best_t) = Self::closest_segment_projection(pos, &path[start..]);
        let seg_start = start + best_seg;
        let mut i = current;
        while i + 1 < path.len() && (seg_start >= i || (seg_start + 1 == i && best_t >= 1.0)) {
            i += 1;
        }
        i
    }
    /// C++ `Pathfinder::updateGoal` on a ground path accept. Last node, not the click.
    pub(crate) fn register_ground_path_goal(&mut self, unit_id: ObjectId, last: Vec3) {
        let Some((uid, player, radius, old)) = self.objects.get(&unit_id).and_then(|unit| {
            if unit.is_kind_of(KindOf::Immobile)
                || unit.is_safe_path
                || !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit)
            {
                return None;
            }
            Some((
                unit.id.0,
                unit.owner_player_id.unwrap_or(unit.team as u32),
                unit.selection_radius,
                unit.pathfind_goal_cell,
            ))
        }) else {
            return;
        };
        let new_cell = self.pathfinding_system.grid.update_ground_goal_cell(
            uid, player, radius, false, old, last,
        );
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.pathfind_goal_cell = new_cell;
        }
    }

    /// C++ `requestPath(&goal, getAdjustsDestination())` (AIUpdate.cpp:470-475,
    /// AIStates.cpp:1560-1583). Not the aircraft hover/wings adjust.
    pub(crate) fn note_move_to_request_path(&mut self, unit_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        unit.is_attack_path = false;
        unit.is_approach_path = false;
        unit.is_safe_path = false;
        unit.requested_victim_id = None;
        let landed_chinook = unit.chinook_ai.as_ref().is_some_and(|ai| {
            ai.flight_status
                == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
        });
        unit.adjust_destinations = !unit.ultra_accurate;
        let adjusts = !unit.is_parachuting()
            && !landed_chinook
            && !(unit.chinook_ai.is_some() && unit.allow_invalid_position)
            && unit.adjust_destinations;
        unit.is_final_goal = adjusts;
        unit.num_frames_blocked = 0;
        unit.is_blocked_and_stuck = false;
        unit.set_status_moving(true);
        adjusts
    }


    pub(crate) fn apply_arrival_goal_snap(&mut self, id: ObjectId, goal: Option<Vec3>) {
        let _ = goal;
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        if obj.holds_air_position_when_idle() {
            return;
        }
        if !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(obj) {
            return;
        }
        // C++ goalPosition reads m_pathfindGoalCell. No registered cell skips
        // setFinalPosition. It does not world_to_grid the raw waypoint.
        let stored = obj.pathfind_goal_cell;
        if stored.0 < 0 || stored.1 < 0 {
            return;
        }
        let pos = obj.get_position();
        let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
        let selection_radius = obj.selection_radius;
        let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(
            selection_radius,
            self.pathfinding_system.grid.grid_size(),
        );
        let goal_cell = crate::game_logic::pathfinding::GridPos::new(stored.0, stored.1);
        let adjusted = self
            .pathfinding_system
            .grid
            .adjust_coord_to_ground_cell(goal_cell, center);
        let dx = pos.x - adjusted.x;
        let dz = pos.z - adjusted.z;
        let beyond_one_cell = dx * dx + dz * dz >= cell * cell;
        let final_pos = if beyond_one_cell {
            self.pathfinding_system.grid.snap_position(pos, center)
        } else {
            adjusted
        };
        let uid = obj.id.0;
        let player = obj.owner_player_id.unwrap_or(obj.team as u32);
        let immobile = obj.is_kind_of(KindOf::Immobile);
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.final_position = final_pos;
            obj.do_final_position = false;
        }
        let new_cell = self.pathfinding_system.grid.update_ground_goal_cell(
            uid, player, selection_radius, immobile, stored, final_pos,
        );
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.pathfind_goal_cell = new_cell;
        }
    }


    /// C++ `applyMotiveForce(0)` at locoUpdate_moveTowardsPosition entry.
    /// Host collide/friction need the motive window even when GW owns pose.
    fn arm_march_motive_flags(&mut self, object_ids: &[ObjectId]) {
        for &id in object_ids {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            if obj.is_disabled() || obj.host_skip_dead_locomotor() || obj.is_shock_stunned() {
                continue;
            }
            if obj.waiting_for_path {
                continue;
            }
            let has_move_goal =
                obj.movement.target_position.is_some() || !obj.movement.path.is_empty();
            if has_move_goal {
                obj.apply_motive_force(glam::Vec3::ZERO);
            }
        }
    }

    /// C++ `AIUpdate.cpp:2276-2279` after movement for the frame.
    fn stamp_airborne_targets_from_locomotor(&mut self, object_ids: &[ObjectId]) {
        for &id in object_ids {
            let sample = self.objects.get(&id).and_then(|obj| {
                if obj.is_disabled() && !obj.status.disabled_freefall {
                    None
                } else {
                    Some((obj.get_position(), obj.ground_height))
                }
            });
            let Some((pos, fallback)) = sample else {
                continue;
            };
            let gy = self.terrain_height_at(pos).unwrap_or(fallback);
            if let Some(obj) = self.objects.get_mut(&id) {
                Self::stamp_object_airborne_target(obj, gy);
            }
        }
    }

    fn stamp_object_airborne_target(obj: &mut Object, ground_y: f32) {
        obj.ground_height = ground_y;
        let mut pos = obj.get_position();
        // C++ PhysicsUpdate.cpp:748-760. Every unit at or below the layer
        // is lifted. Excess upward speed is removed. ALLOW_TO_FALL clears.
        if pos.y <= ground_y {
            let impact_vy = obj.movement.velocity.y;
            let was_airborne = obj.was_airborne_last_frame;
            let old_y = pos.y;
            let dz = ground_y - pos.y;
            obj.movement.velocity.y += dz;
            if obj.movement.velocity.y > 0.0 {
                obj.movement.velocity.y = 0.0;
            }
            obj.invalidate_velocity_magnitude();
            pos.y = ground_y;
            obj.set_position(pos);
            obj.allow_to_fall = false;
            // C++ PhysicsUpdate.cpp:822-831. Bounce sound uses the pre-land height.
            if was_airborne && !obj.immune_to_falling_damage {
                obj.record_bounce_land(old_y);
                obj.pending_ground_collide = true;
                let _ = obj.apply_shock_fall_damage(impact_vy);
                obj.landing_splat_done = true;
            }
            if obj.velocity_is_very_small() {
                let _ = obj.maybe_kill_when_resting_on_ground();
            }
            obj.was_airborne_last_frame = false;
            obj.is_in_freefall = false;
            obj.set_status_disabled_freefall(false);
            obj.model_condition_bits &=
                !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL);
            // C++ PhysicsUpdate.cpp:765-769. First ground hit while stunned
            // swaps STUNNED_FLAILING for STUNNED.
            if obj.shock_stun_frames > 0 {
                obj.shock_grounded_once = true;
                obj.model_condition_bits &=
                    !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_STUNNED_FLAILING);
                obj.model_condition_bits |=
                    1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_STUNNED;
            }
        } else if obj.is_in_freefall {
            // C++ PhysicsUpdate.cpp:774-777. Physics IS_IN_FREEFALL, not parachuting.
            obj.set_status_disabled_freefall(true);
            obj.model_condition_bits |=
                1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_FREEFALL;
        } else if obj.stick_to_ground && !obj.allow_to_fall {
            // C++ PhysicsUpdate.cpp:779-781. STICK_TO_GROUND, not ALLOW_TO_FALL,
            // already above the layer. Not a path-blocked unit.
            pos.y = ground_y;
            obj.set_position(pos);
        }
        obj.stamp_airborne_target_from_locomotor();
    }

    /// C++ AIExitState::update polls isExitBusy / getAiFreeToExit every frame
    /// with no hull-stop requirement. move-to-and-evacuate still waits for
    /// arrival (`pending_stream_exit` stays false until the first dump).
    fn drain_pending_transport_exits(&mut self) {
        let mut evac_now: Vec<(ObjectId, bool)> = Vec::new();
        for (id, obj) in &self.objects {
            if !obj.pending_evacuate_on_stop {
                continue;
            }
            let stopped = obj.movement.path.is_empty() && !obj.status.moving;
            let stream = obj.pending_stream_exit
                && !(obj.transport_delay_exit_in_air() && obj.is_above_terrain_for_exit());
            if stopped || stream {
                evac_now.push((*id, obj.pending_exit_after_evacuate));
            }
        }
        for (id, and_exit) in evac_now {
            let _ = self.evacuate_container_now(id, and_exit);
        }
    }

    #[cfg(test)]
    pub fn drain_pending_transport_exits_for_test(&mut self) {
        self.drain_pending_transport_exits();
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
        crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
        crate::game_logic::combat::drain_pending_projectiles(
            &mut self.combat_system,
            &self.objects,
        );
        crate::game_logic::combat::apply_ready_projectileless_delayed_damage(
            &mut self.combat_system,
            &mut self.objects,
            self.frame,
            Some(&self.players),
        );
        self.execute_pending_weapon_fire_ocls();
    }

    /// Hit-only projectile pass after GameWorld flight integrate writeback.
    pub(crate) fn resolve_projectiles_hits_only(&mut self) -> Vec<ObjectId> {
        self.combat_system.refresh_homing_aims(&self.objects);
        let hits = self.combat_system.update_projectiles_with_relationships(
            0.0,
            &mut self.objects,
            Some(&mut self.countermeasures),
            self.frame,
            Some(&self.players),
        );
        self.flush_projectile_impact_fx();
        hits
    }
}

fn adjusts_destination_now(obj: &crate::game_logic::Object) -> bool {
    if obj.is_parachuting() {
        return false;
    }
    let landed_chinook = obj.chinook_ai.as_ref().is_some_and(|ai| {
        ai.flight_status
            == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
    });
    !landed_chinook
        && !(obj.chinook_ai.is_some() && obj.allow_invalid_position)
        && obj.adjust_destinations
}

/// C++ `Pathfinder::validMovementTerrain` (AIPathfind.cpp:4763-4783).
/// Obstacle/Impassable are terrain-present (true). Else locomotor surfaces
/// must intersect the cell's surface mask. Out-of-grid is false (NULL cell).
fn valid_movement_terrain_at(
    grid: &crate::game_logic::PathfindingGrid,
    surfaces: u32,
    world_pos: Vec3,
    layer: u8,
) -> bool {
    use crate::game_logic::locomotor_bootstrap::valid_locomotor_surfaces_for_cell_type;
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let cell = grid.world_to_grid(world_pos);
    if !grid.is_valid_pos(cell) {
        return false;
    }
    // PathfindLayer::getCell returns NULL for CELL_IMPASSABLE
    // (AIPathfind.cpp:3636-3637); Pathfinder::getCell then uses the ground map.
    // BridgeImpassable stays on the layer and is not the early-true case.
    let ty = grid.locomotor_cell_type(world_pos, layer);
    if matches!(
        ty,
        PathfindCellType::Obstacle | PathfindCellType::Impassable
    ) {
        return true;
    }
    (surfaces & valid_locomotor_surfaces_for_cell_type(ty)) != 0
}

/// C++ `Locomotor::getCloseEnoughDist` (default 1.0 at Locomotor.cpp:321).
fn host_close_enough_dist(obj: &crate::game_logic::Object) -> f32 {
    obj.close_enough_dist
        .filter(|d| d.is_finite() && *d >= 0.0)
        .unwrap_or(1.0)
}

/// C++ `Locomotor::fixInvalidPosition` (Locomotor.cpp:1500-1562).
/// Dozer exempt, 3×3 vote, skip if already leaving (dot > 0.25), extra push
/// when velocity-dot < 0.
fn try_fix_invalid_position_3x3(
    obj: &mut crate::game_logic::Object,
    grid: &crate::game_logic::PathfindingGrid,
    surfaces: u32,
) -> bool {
    if obj.is_dozer || obj.is_kind_of(crate::game_logic::KindOf::Dozer) {
        return false;
    }
    let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
    let pos = obj.get_position();
    let mut dx_acc = 0.0f32;
    let mut dz_acc = 0.0f32;
    for j in -1i32..=1 {
        for i in -1i32..=1 {
            let check = Vec3::new(pos.x + (i as f32) * cell, pos.y, pos.z + (j as f32) * cell);
            if !valid_movement_terrain_at(grid, surfaces, check, obj.pathfind_layer) {
                if i < 0 {
                    dx_acc += 1.0;
                }
                if i > 0 {
                    dx_acc -= 1.0;
                }
                if j < 0 {
                    dz_acc += 1.0;
                }
                if j > 0 {
                    dz_acc -= 1.0;
                }
            }
        }
    }
    if dx_acc == 0.0 && dz_acc == 0.0 {
        return false;
    }
    let mass = obj.physics_get_mass();
    let correction = glam::Vec3::new(dx_acc * mass / 5.0, 0.0, dz_acc * mass / 5.0);
    let len = (correction.x * correction.x + correction.z * correction.z).sqrt();
    let (nx, nz) = if len > 0.0001 {
        (correction.x / len, correction.z / len)
    } else {
        (0.0, 0.0)
    };
    let v = obj.movement.velocity;
    let dot = v.x * nx + v.z * nz;
    if dot > 0.25 {
        return false;
    }
    if dot < 0.0 {
        let mag = (-dot).sqrt();
        obj.apply_motive_force(glam::Vec3::new(nx * mag * mass, 0.0, nz * mag * mass));
    }
    obj.apply_motive_force(correction);
    obj.record_host_movement();
    true
}

/// Host Y-up → leftover C++ Z-up.
fn leftover_host_to_cpp(pos: Vec3) -> gamelogic::common::Coord3D {
    gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y)
}

fn leftover_cpp_to_host(pos: gamelogic::common::Coord3D) -> Vec3 {
    Vec3::new(pos.x, pos.z, pos.y)
}

/// Leftover `Locomotor::settle_final_position` (NONE-goal half of
/// `loco_update_when_goal_none`). Live idle already runs maintain.
fn leftover_settle_final_position_on_object(obj: &mut Object) {
    if !obj.do_final_position {
        return;
    }
    let on_ground = !obj.is_above_terrain() && obj.pathfind_layer == 1;
    let (pos, still) = gamelogic::locomotor::Locomotor::settle_final_position(
        leftover_host_to_cpp(obj.get_position()),
        leftover_host_to_cpp(obj.final_position),
        on_ground,
    );
    obj.do_final_position = still;
    obj.set_position(leftover_cpp_to_host(pos));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{
        GameLogic, GridPos, KindOf, LocomotorAppearance, Object, ObjectId, PathfindingGrid, Team,
        ThingTemplate,
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

    /// C++ `validMovementPosition`: crushers enter CELL_RUBBLE without a RUBBLE
    /// locomotor bit (AIPathfind.cpp:4840 / crate `is_passable`). Live host
    /// used to hardcode `is_crusher=false`, so Overlords treated rubble like
    /// infantry.
    #[test]
    fn crusher_paths_rubble_that_blocks_non_crusher() {
        use crate::game_logic::LOCO_SURFACE_GROUND;
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
                .set_cell_type(GridPos::new(wall_x, y), PathfindCellType::Rubble);
        }

        let inf_id = ObjectId(9201);
        let mut ranger = ranger_at(9201, start);
        ranger.locomotor_surfaces = LOCO_SURFACE_GROUND;
        ranger.crusher_level = 0;
        logic.objects.insert(inf_id, ranger);
        logic.move_object_with_pathfinding_for_test(inf_id, goal, None);
        let inf = logic.objects.get(&inf_id).expect("ranger");
        assert!(
            inf.movement.path.is_empty(),
            "non-crusher must not path CELL_RUBBLE without SURFACE_RUBBLE"
        );

        let tank_id = ObjectId(9202);
        let mut tmpl = ThingTemplate::new("Overlord");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(tmpl, tank_id, Team::USA);
        tank.set_position(start);
        tank.locomotor_surfaces = LOCO_SURFACE_GROUND;
        tank.crusher_level = 1;
        logic.objects.insert(tank_id, tank);
        logic.move_object_with_pathfinding_for_test(tank_id, goal, None);
        let tank = logic.objects.get(&tank_id).expect("overlord");
        assert!(
            tank.movement.path.len() >= 2,
            "crusher_level>0 must path CELL_RUBBLE (AIPathfind.cpp:8170)"
        );
        assert!(tank.movement.target_position.is_some());
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
        assert_eq!(
            g.world_to_grid(Vec3::new(20.0, 0.0, 0.0)),
            GridPos::new(2, 0)
        );
        assert_eq!(
            g.world_to_grid(Vec3::new(-19.9, 0.0, -5.1)),
            GridPos::new(-1, 0)
        );
    }

    /// C++ GameLogic.cpp:3677-3718 skips UpdateModules while disabled
    /// (EMP / hack / unmanned / leaflet). Host `update_movement` must halt (hq-psal).
    #[test]
    fn disabled_unit_does_not_advance_in_update_movement() {
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
        let a = logic
            .objects
            .get(&ObjectId(9021))
            .unwrap()
            .get_orientation();
        let b = logic
            .objects
            .get(&ObjectId(9022))
            .unwrap()
            .get_orientation();
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

    /// C++ Object.cpp:2580-2583 notifyTerrainObjectMoved → W3DTreeBuffer::unitMoved.
    #[test]
    fn unit_move_notifies_tree_buffer_topple() {
        let _ = game_client::terrain::terrain_visual::init_terrain_visual();
        let tree_ndx = {
            let mut guard = game_client::terrain::terrain_visual::get_terrain_visual()
                .expect("terrain visual lock");
            let visual = guard.as_mut().expect("terrain visual");
            visual.tree_buffer_mut().clear_all_trees();
            visual
                .tree_buffer_mut()
                .set_bounds(game_client::terrain::TreeRegion2D::new(
                    glam::Vec2::ZERO,
                    glam::Vec2::new(100.0, 100.0),
                ));
            let mut data = game_client::terrain::TreeModuleData::default();
            data.model_name = "Oak".into();
            data.do_topple = true;
            visual
                .tree_buffer_mut()
                .add_tree(
                    77,
                    glam::Vec3::new(10.0, 10.0, 0.0),
                    1.0,
                    0.0,
                    1.0,
                    data,
                    game_client::terrain::TreeSphere {
                        center: glam::Vec3::ZERO,
                        radius: 5.0,
                    },
                )
                .expect("add tree")
        };

        let mut tank_tmpl = ThingTemplate::new("CrusherTank");
        tank_tmpl.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(tank_tmpl, ObjectId(9100), Team::USA);
        tank.set_position(Vec3::ZERO);
        tank.crusher_level = 2;
        tank.selection_radius = 8.0;
        // Integer XY change from (0,0) → (10,10) must notify trees.
        tank.set_position(Vec3::new(10.0, 0.0, 10.0));

        let mut guard = game_client::terrain::terrain_visual::get_terrain_visual()
            .expect("terrain visual lock");
        let visual = guard.as_mut().expect("terrain visual");
        assert_eq!(
            visual.tree_buffer_mut().trees()[tree_ndx].topple_state,
            game_client::terrain::W3DToppleState::Falling,
            "hq-rdyvl: moving crusher must topple map trees"
        );
    }

    /// C++ `Locomotor::handleBehaviorZ` Z_SURFACE_RELATIVE_HEIGHT
    /// (Locomotor.cpp:2288-2316): lift force + Euler, never kinematic snap
    /// to preferredHeight+surface (hq-ygdfb).
    #[test]
    fn hover_surface_relative_follows_preferred_height() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9301);
        let mut tmpl = ThingTemplate::new("Comanche");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, id, Team::USA);
        heli.set_position(Vec3::new(0.0, 0.0, 0.0));
        heli.ground_height = 20.0;
        heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.loco_preferred_height = 10.0;
        heli.loco_preferred_height_damping = 1.0;
        heli.max_lift = 5.0;
        heli.physics_mass = 1.0;
        heli.movement.max_speed = 30.0;
        heli.movement.acceleration = 10_000.0;
        heli.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        logic.objects.insert(id, heli);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("heli");
        let y = obj.get_position().y;
        assert!(
            y > 0.5 && y < 15.0,
            "hover must rise by lift (maxLift=5), not snap to 30; y={}",
            y
        );
    }

    /// Idle hover maintain still applies lift toward preferredHeight
    /// (Locomotor.cpp:2473 / :2288-2316) — no kinematic snap (hq-ygdfb).
    #[test]
    fn idle_hover_maintain_holds_preferred_height() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9302);
        let mut tmpl = ThingTemplate::new("ComancheIdle");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, id, Team::USA);
        heli.set_position(Vec3::new(0.0, 4.0, 0.0));
        heli.ground_height = 8.0;
        heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.loco_preferred_height = 12.0;
        heli.loco_preferred_height_damping = 1.0;
        heli.max_lift = 5.0;
        heli.physics_mass = 1.0;
        logic.objects.insert(id, heli);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("heli");
        let y = obj.get_position().y;
        assert!(
            y > 4.5 && y < 16.0,
            "idle hover must rise by lift, not snap to 20; y={}",
            y
        );
    }

    /// Ground locos still must not dive to Y=0 path cells.
    #[test]
    fn ground_march_does_not_dive_to_path_y() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9303);
        let mut unit = ranger_at(9303, Vec3::new(0.0, 5.0, 0.0));
        unit.ground_height = 5.0;
        unit.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
        unit.movement.max_speed = 30.0;
        unit.movement.acceleration = 10_000.0;
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("ranger");
        assert!(
            (obj.get_position().y - 5.0).abs() < 0.05,
            "ground march must keep Y, got {}",
            obj.get_position().y
        );
    }

    #[test]
    fn wheeled_truck_does_not_spin_in_place() {
        // C++ Locomotor.cpp:1437-1454 turnFactor = |speed|/minTurnSpeed.
        let mut logic = GameLogic::new();
        let id = ObjectId(9401);
        let mut tmpl = ThingTemplate::new("Humvee");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut truck = Object::new(tmpl, id, Team::USA);
        truck.set_position(Vec3::ZERO);
        truck.set_orientation(0.0);
        truck.loco_appearance = LocomotorAppearance::WheelsFour;
        truck.min_turn_speed = 15.0;
        truck.movement.turn_rate = std::f32::consts::PI;
        truck.movement.max_speed = 40.0;
        truck.movement.acceleration = 10_000.0;
        truck.movement.velocity = Vec3::ZERO;
        truck.movement.target_position = Some(Vec3::new(0.0, 0.0, 80.0));
        logic.objects.insert(id, truck);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("truck");
        assert!(
            obj.get_orientation().abs() < 1e-4,
            "stationary wheels must not yaw, got {}",
            obj.get_orientation()
        );
    }

    #[test]
    fn ultra_accurate_doubles_turn_rate() {
        // C++ Locomotor.cpp:796-798 getMaxTurnRate * 2.
        let mut tmpl = ThingTemplate::new("Dozer");
        tmpl.add_kind_of(KindOf::Dozer);
        let mut dozer = Object::new(tmpl, ObjectId(9402), Team::USA);
        dozer.movement.turn_rate = 1.0;
        assert!((dozer.effective_turn_rate() - 1.0).abs() < 1e-5);
        dozer.set_ultra_accurate(true);
        assert!((dozer.effective_turn_rate() - 2.0).abs() < 1e-5);
        dozer.set_ai_state(AIState::Constructing);
        assert!(dozer.ultra_accurate);
    }

    #[test]
    fn downhill_only_refuses_uphill_goal() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ski");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9501), Team::USA);
        unit.set_position(Vec3::new(0.0, 0.0, 0.0));
        unit.loco_appearance = LocomotorAppearance::LegsTwo;
        unit.downhill_only = true;
        unit.movement.max_speed = 30.0;
        unit.movement.target_position = Some(Vec3::new(20.0, 10.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9501), unit);
        logic.update_movement_for_test(&[ObjectId(9501)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9501)).expect("ski");
        assert!(
            obj.movement.velocity.length() < 1e-3,
            "downhill-only must not climb, vel={}",
            obj.movement.velocity
        );
        assert!(
            obj.get_position().x.abs() < 0.1,
            "downhill-only must stay put, pos={:?}",
            obj.get_position()
        );
    }

    #[test]
    fn stick_to_ground_pulls_a_walker_down_to_the_layer() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9510), Team::USA);
        unit.loco_appearance = LocomotorAppearance::LegsTwo;
        unit.stick_to_ground = true;
        unit.allow_to_fall = false;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::new(0.0, 5.0, 0.0));
        unit.movement.max_speed = 30.0;
        unit.movement.target_position = Some(Vec3::new(10.0, 5.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9510), unit);
        logic.update_movement_for_test(&[ObjectId(9510)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9510)).expect("ranger");
        assert!(
            obj.get_position().y.abs() < 0.05,
            "stick-to-ground must drop the walker onto the layer, y={}",
            obj.get_position().y
        );
    }
    #[test]
    fn ground_slam_lifts_a_vehicle_onto_the_cell_just_entered() {
        let mut logic = GameLogic::new();
        logic.pathfinding_system =
            crate::game_logic::pathfinding::PathfindingSystem::new(80.0, 40.0);
        let w = logic.pathfinding_system.grid.width() as u32;
        let h = logic.pathfinding_system.grid.height() as u32;
        let mut heights = vec![0.0f32; (w * h) as usize];
        for y in 0..h {
            for x in 2..w {
                heights[(y * w + x) as usize] = 12.0;
            }
        }
        assert!(logic.restore_terrain_heights_from_grid(w, h, &heights));
        let start = logic
            .pathfinding_system
            .grid
            .grid_to_world(crate::game_logic::pathfinding::GridPos::new(1, 1));
        let goal = logic
            .pathfinding_system
            .grid
            .grid_to_world(crate::game_logic::pathfinding::GridPos::new(5, 1));
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9511), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.stick_to_ground = false;
        unit.allow_to_fall = true;
        unit.allow_invalid_position = true;
        unit.ground_height = 0.0;
        unit.set_position(start);
        unit.movement.max_speed = 900.0;
        unit.movement.acceleration = 1.0e6;
        unit.movement.velocity = Vec3::new(900.0, 0.0, 0.0);
        unit.movement.target_position = Some(goal);
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9511), unit);
        for _ in 0..8 {
            logic.update_movement_for_test(&[ObjectId(9511)], 1.0 / 30.0);
        }
        let obj = logic.objects.get(&ObjectId(9511)).expect("truck");
        let cell = logic.pathfinding_system.grid.world_to_grid(obj.get_position());
        assert!(
            cell.x >= 2,
            "unstunned vehicle should enter the higher cells, cell={cell:?} pos={:?}",
            obj.get_position()
        );
        assert!(
            (obj.get_position().y - 12.0).abs() < 0.05,
            "ground slam must lift onto the cell just entered, y={}",
            obj.get_position().y
        );
        assert!(!obj.shock_grounded_once);
    }

    #[test]
    fn ground_slam_marks_a_stunned_unit_already_on_its_cell() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9512), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.stick_to_ground = false;
        unit.ground_height = 20.0;
        unit.allow_to_fall = true;
        unit.is_in_freefall = true;
        unit.set_status_disabled_freefall(true);
        unit.shock_stun_frames = 40;
        unit.set_position(Vec3::new(0.0, 0.0, 0.0));
        unit.movement.velocity = Vec3::ZERO;
        logic.objects.insert(ObjectId(9512), unit);
        logic.update_movement_for_test(&[ObjectId(9512)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9512)).expect("stunned");
        assert!(obj.shock_grounded_once);
        assert!(!obj.allow_to_fall);
        assert!(!obj.status.disabled_freefall);
        assert!(!obj.is_in_freefall);
        assert!(obj.movement.velocity.y <= 0.0);
    }

    #[test]
    fn airborne_freefall_is_disabled_without_parachuting() {
        use crate::game_logic::host_enum_table_residual::{MC_BIT_FREEFALL, host_model_condition_has};
        let mut logic = GameLogic::new();
        let mut wreck_t = ThingTemplate::new("Wreck");
        wreck_t.add_kind_of(KindOf::Vehicle);
        let mut wreck = Object::new(wreck_t, ObjectId(9513), Team::USA);
        wreck.is_in_freefall = true;
        wreck.stick_to_ground = false;
        wreck.allow_to_fall = true;
        wreck.ground_height = 0.0;
        wreck.set_position(Vec3::new(0.0, 30.0, 0.0));
        logic.objects.insert(ObjectId(9513), wreck);
        let mut chute_t = ThingTemplate::new("AmericaParachute");
        chute_t.add_kind_of(KindOf::Infantry);
        let mut chute = Object::new(chute_t, ObjectId(9514), Team::USA);
        chute.set_status_parachuting(true);
        chute.is_in_freefall = false;
        chute.stick_to_ground = false;
        chute.ground_height = 0.0;
        chute.set_position(Vec3::new(10.0, 30.0, 0.0));
        logic.objects.insert(ObjectId(9514), chute);
        logic.update_movement_for_test(&[ObjectId(9513), ObjectId(9514)], 1.0 / 30.0);
        let wreck = logic.objects.get(&ObjectId(9513)).expect("wreck");
        assert!(wreck.status.disabled_freefall);
        assert!(host_model_condition_has(wreck.model_condition_bits, MC_BIT_FREEFALL));
        assert!(wreck.get_position().y > 1.0);
        let chute = logic.objects.get(&ObjectId(9514)).expect("chute");
        assert!(
            !chute.status.disabled_freefall,
            "parachuting alone is not IS_IN_FREEFALL"
        );
    }

    #[test]
    fn landing_from_last_frame_air_deals_falling_damage() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Faller");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.set_health(200.0);
        let mut unit = Object::new(tmpl, ObjectId(9515), Team::USA);
        unit.was_airborne_last_frame = true;
        unit.immune_to_falling_damage = false;
        unit.stick_to_ground = false;
        unit.ground_height = 0.0;
        // Already on the layer. The bounce correction is zero; damage uses vy.
        unit.set_position(Vec3::ZERO);
        unit.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
        unit.health.current = 200.0;
        logic.objects.insert(ObjectId(9515), unit);
        logic.update_movement_for_test(&[ObjectId(9515)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9515)).expect("faller");
        assert!(
            obj.health.current < 200.0,
            "steep landing must deal falling damage, hp={}",
            obj.health.current
        );
        assert!(!obj.was_airborne_last_frame);
        assert!(obj.pending_ground_collide);
        assert!(obj.bounce_land_events > 0);
        assert!(obj.last_bounce_fall_dy.abs() < 0.05);
    }

    #[test]
    fn unit_still_above_the_layer_does_not_bounce() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Faller");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.set_health(200.0);
        let mut unit = Object::new(tmpl, ObjectId(9518), Team::USA);
        unit.was_airborne_last_frame = true;
        unit.immune_to_falling_damage = false;
        unit.stick_to_ground = false;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::new(0.0, 30.0, 0.0));
        unit.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
        unit.health.current = 200.0;
        logic.objects.insert(ObjectId(9518), unit);
        logic.update_movement_for_test(&[ObjectId(9518)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9518)).expect("faller");
        assert!(
            obj.get_position().y > 1.0,
            "an XZ march must not invent a landing, y={}",
            obj.get_position().y
        );
        assert_eq!(obj.bounce_land_events, 0);
        assert!(!obj.pending_ground_collide);
        assert!((obj.health.current - 200.0).abs() < 0.01);
    }


    #[test]
    fn march_applies_locomotor_physics_options() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("LocoInf");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9520), Team::USA);
        unit.ultra_accurate = true;
        unit.loco_extra_2d_friction = 0.1;
        unit.stick_to_ground = false;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.max_speed = 10.0;
        unit.movement.target_position = Some(Vec3::new(20.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9520), unit);
        logic.update_movement_for_test(&[ObjectId(9520)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9520)).expect("inf");
        assert!((obj.extra_friction - 0.6).abs() < 1e-5);
        assert!(
            !obj.stick_to_ground,
            "the march must not force StickToGround on"
        );
    }

    #[test]
    fn desired_speed_caps_the_march() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("SlowGroup");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9522), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.desired_speed = 5.0;
        unit.movement.max_speed = 40.0;
        unit.movement.acceleration = 1.0e6;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9522), unit);
        logic.update_movement_for_test(&[ObjectId(9522)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9522)).expect("truck");
        let spd = obj.movement.velocity.length();
        assert!(
            spd <= 5.5 && spd > 1.0,
            "desired speed 5 must cap a max of 40, spd={spd}"
        );
    }

    #[test]
    fn unblocked_move_keeps_one_blocked_frame() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9524), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.allow_invalid_position = true;
        unit.is_blocked = true;
        unit.num_frames_blocked = 5;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.max_speed = 20.0;
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9524), unit);
        logic.update_movement_for_test(&[ObjectId(9524)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9524)).expect("truck");
        assert_eq!(obj.num_frames_blocked, 1);
        assert!(!obj.is_blocked);
    }

    #[test]
    fn path_extra_distance_unlatches_braking_before_the_raise() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Tank");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9528), Team::USA);
        unit.loco_appearance = LocomotorAppearance::Treads;
        unit.allow_invalid_position = true;
        unit.is_braking = true;
        unit.braking = 10.0;
        unit.braking_factor = 2.0;
        unit.path_extra_distance = 100.0;
        unit.movement.max_speed = 20.0;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        // Straight distance is ~5, below dist-to-stop (~20). Extra makes it far.
        unit.movement.target_position = Some(Vec3::new(5.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9528), unit);
        logic.update_movement_for_test(&[ObjectId(9528)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9528)).expect("tank");
        assert!(
            !obj.is_braking,
            "path extra distance must count in the far-from-goal unlatch"
        );
        assert!((obj.braking_factor - 1.0).abs() < 1e-4);
    }

    #[test]
    fn explicit_goal_ignores_the_blocked_speed_cap() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9529), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.allow_invalid_position = true;
        unit.locomotor_goal_type = LocoGoalType::PositionExplicit;
        unit.is_blocked = true;
        unit.num_frames_blocked = 5;
        unit.cur_max_blocked_speed = 1.0;
        unit.movement.max_speed = 40.0;
        unit.movement.acceleration = 1.0e6;
        unit.movement.velocity = Vec3::new(1.0, 0.0, 0.0);
        unit.set_orientation(0.0);
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9529), unit);
        logic.update_movement_for_test(&[ObjectId(9529)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9529)).expect("truck");
        let spd = obj.movement.velocity.length();
        assert!(
            spd > 10.0,
            "an explicit goal must not stay at the blocked cap, spd={spd}"
        );
    }

    #[test]
    fn explicit_goal_does_not_use_path_extra_distance() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Tank");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9530), Team::USA);
        unit.loco_appearance = LocomotorAppearance::Treads;
        unit.allow_invalid_position = true;
        unit.locomotor_goal_type = LocoGoalType::PositionExplicit;
        unit.is_braking = false;
        unit.braking = 10.0;
        unit.path_extra_distance = 100.0;
        unit.movement.max_speed = 20.0;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(5.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9530), unit);
        logic.update_movement_for_test(&[ObjectId(9530)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9530)).expect("tank");
        assert!(
            obj.is_braking,
            "explicit distance 0 must still be inside the slow-down, not cleared by path extra"
        );
        assert!(obj.cur_max_blocked_speed > 1_000.0);
    }

    #[test]
    fn idle_hover_keeps_the_first_maintain_point() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Hover");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9531), Team::USA);
        unit.loco_appearance = LocomotorAppearance::Hover;
        unit.maintain_pos_valid = true;
        unit.maintain_pos = Some(Vec3::new(4.0, 6.0, 8.0));
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::ZERO);
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9531), unit);
        logic.update_movement_for_test(&[ObjectId(9531)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9531)).expect("hover");
        assert_eq!(obj.maintain_pos, Some(Vec3::new(4.0, 6.0, 8.0)));
        assert!(obj.maintain_pos_valid);
    }

    #[test]
    fn idle_hover_keeps_its_ignored_obstacle() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Hover");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9532), Team::USA);
        unit.loco_appearance = LocomotorAppearance::Hover;
        unit.ignored_obstacle_id = Some(ObjectId(7));
        unit.ground_height = 0.0;
        unit.movement.target_position = Some(Vec3::ZERO);
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9532), unit);
        logic.update_movement_for_test(&[ObjectId(9532)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9532)).expect("hover");
        assert_eq!(obj.ignored_obstacle_id, Some(ObjectId(7)));
    }

    #[test]
    fn idle_wing_keeps_a_queued_repath() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Jet");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut unit = Object::new(tmpl, ObjectId(9534), Team::USA);
        unit.loco_appearance = LocomotorAppearance::Wings;
        unit.queue_for_path_frames = 15;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::ZERO);
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9534), unit);
        logic.update_movement_for_test(&[ObjectId(9534)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9534)).expect("jet");
        assert_eq!(obj.queue_for_path_frames, 15);
    }

    #[test]
    fn moving_unit_already_on_goal_finishes_once() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Jet");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut unit = Object::new(tmpl, ObjectId(9535), Team::USA);
        unit.loco_appearance = LocomotorAppearance::Wings;
        unit.set_ai_state(AIState::Moving);
        unit.queue_for_path_frames = 15;
        unit.ignored_obstacle_id = Some(ObjectId(7));
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::ZERO);
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9535), unit);
        logic.update_movement_for_test(&[ObjectId(9535)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9535)).expect("jet");
        assert_eq!(obj.queue_for_path_frames, 0);
        assert!(obj.ignored_obstacle_id.is_none());
        assert!(!obj.status.moving);
        assert!(obj.movement.target_position.is_none());
        if let Some(obj) = logic.objects.get_mut(&ObjectId(9535)) {
            obj.queue_for_path_frames = 15;
            obj.ignored_obstacle_id = Some(ObjectId(7));
        }
        logic.update_movement_for_test(&[ObjectId(9535)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9535)).expect("jet");
        assert_eq!(obj.queue_for_path_frames, 15);
        assert_eq!(obj.ignored_obstacle_id, Some(ObjectId(7)));
    }

    #[test]
    fn arrival_without_a_goal_cell_does_not_snap() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9601), Team::USA);
        unit.selection_radius = 8.0;
        unit.pathfind_goal_cell = (-1, -1);
        unit.set_position(Vec3::new(3.0, 0.0, 3.0));
        logic.objects.insert(ObjectId(9601), unit);
        logic.apply_arrival_goal_snap(ObjectId(9601), Some(Vec3::new(80.0, 0.0, 80.0)));
        let obj = logic.objects.get(&ObjectId(9601)).expect("ranger");
        assert!(
            !obj.do_final_position,
            "goalPosition fails when the goal cell was never registered"
        );
    }

    #[test]
    fn arrival_snaps_to_the_stored_goal_cell_not_the_waypoint() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9602), Team::USA);
        unit.selection_radius = 8.0;
        unit.set_position(Vec3::new(3.0, 0.0, 3.0));
        logic.objects.insert(ObjectId(9602), unit);
        let (radius, cell_size) = {
            let obj = logic.objects.get(&ObjectId(9602)).expect("ranger");
            (
                obj.selection_radius,
                logic.pathfinding_system.grid.grid_size(),
            )
        };
        let (_, center) =
            crate::game_logic::PathfindingGrid::radius_and_center(radius, cell_size);
        let cell = logic
            .pathfinding_system
            .grid
            .cell_for_unit_position(Vec3::new(3.0, 0.0, 3.0), center);
        let adjusted = logic
            .pathfinding_system
            .grid
            .adjust_coord_to_ground_cell(cell, center);
        if let Some(obj) = logic.objects.get_mut(&ObjectId(9602)) {
            obj.pathfind_goal_cell = (cell.x, cell.y);
            obj.set_position(adjusted);
        }
        logic.apply_arrival_goal_snap(ObjectId(9602), Some(Vec3::new(80.0, 0.0, 80.0)));
        let obj = logic.objects.get(&ObjectId(9602)).expect("ranger");
        assert!(!obj.do_final_position);
        assert!((obj.final_position.x - adjusted.x).abs() < 0.01);
        assert!((obj.final_position.z - adjusted.z).abs() < 0.01);
        assert!(
            obj.final_position.x < 40.0,
            "the far waypoint must not choose the cell"
        );
    }

    #[test]
    fn installed_path_registers_the_goal_cell() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9603), Team::USA);
        unit.selection_radius = 8.0;
        unit.pathfind_goal_cell = (-1, -1);
        unit.is_final_goal = true;
        unit.set_position(Vec3::new(3.0, 0.0, 3.0));
        logic.objects.insert(ObjectId(9603), unit);
        let dest = Vec3::new(80.0, 0.0, 80.0);
        assert!(logic.apply_computed_unit_path(
            ObjectId(9603),
            Vec3::new(3.0, 0.0, 3.0),
            dest,
            vec![Vec3::new(3.0, 0.0, 3.0), dest],
            false,
        ));
        let obj = logic.objects.get(&ObjectId(9603)).expect("ranger");
        let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(
            obj.selection_radius,
            logic.pathfinding_system.grid.grid_size(),
        );
        let cell = logic
            .pathfinding_system
            .grid
            .cell_for_unit_position(dest, center);
        assert_eq!(obj.pathfind_goal_cell, (cell.x, cell.y));
        assert_ne!(obj.pathfind_goal_cell, (-1, -1));
    }

    #[test]
    fn safe_path_does_not_register_a_goal_cell() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9604), Team::USA);
        unit.selection_radius = 8.0;
        unit.pathfind_goal_cell = (-1, -1);
        unit.is_safe_path = true;
        unit.is_final_goal = false;
        unit.set_position(Vec3::new(3.0, 0.0, 3.0));
        logic.objects.insert(ObjectId(9604), unit);
        let dest = Vec3::new(80.0, 0.0, 80.0);
        assert!(logic.apply_computed_unit_path(
            ObjectId(9604),
            Vec3::new(3.0, 0.0, 3.0),
            dest,
            vec![Vec3::new(3.0, 0.0, 3.0), dest],
            false,
        ));
        let obj = logic.objects.get(&ObjectId(9604)).expect("ranger");
        assert_eq!(
            obj.pathfind_goal_cell,
            (-1, -1),
            "doPathfind returns before updateGoal on a safe path"
        );
    }

    #[test]
    fn non_final_path_does_not_register_a_goal_cell() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9605), Team::USA);
        unit.selection_radius = 8.0;
        unit.pathfind_goal_cell = (-1, -1);
        unit.set_position(Vec3::new(3.0, 0.0, 3.0));
        logic.objects.insert(ObjectId(9605), unit);
        let dest = Vec3::new(80.0, 0.0, 80.0);
        assert!(logic.apply_computed_unit_path(
            ObjectId(9605),
            Vec3::new(3.0, 0.0, 3.0),
            dest,
            vec![Vec3::new(3.0, 0.0, 3.0), dest],
            false,
        ));
        let obj = logic.objects.get(&ObjectId(9605)).expect("ranger");
        assert_eq!(
            obj.pathfind_goal_cell,
            (-1, -1),
            "requestPath(..., false) must not updateGoal"
        );
    }

    #[test]
    fn waited_path_removes_the_goal_when_not_adjusting() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9608), Team::USA);
        unit.selection_radius = 8.0;
        unit.is_final_goal = true;
        unit.pathfind_goal_cell = (-1, -1);
        unit.set_position(Vec3::new(3.0, 0.0, 3.0));
        logic.objects.insert(ObjectId(9608), unit);
        let dest = Vec3::new(80.0, 0.0, 80.0);
        assert!(logic.apply_computed_unit_path(
            ObjectId(9608),
            Vec3::new(3.0, 0.0, 3.0),
            dest,
            vec![Vec3::new(3.0, 0.0, 3.0), dest],
            false,
        ));
        let cell = {
            let obj = logic.objects.get(&ObjectId(9608)).expect("ranger");
            let cell = crate::game_logic::pathfinding::GridPos::new(
                obj.pathfind_goal_cell.0,
                obj.pathfind_goal_cell.1,
            );
            assert_ne!(obj.pathfind_goal_cell, (-1, -1));
            assert_eq!(logic.pathfinding_system.grid.ground_goal_unit(cell), 9608);
            cell
        };
        if let Some(obj) = logic.objects.get_mut(&ObjectId(9608)) {
            obj.set_status_parachuting(true);
        }
        logic.on_waited_path_arrived(ObjectId(9608));
        let obj = logic.objects.get(&ObjectId(9608)).expect("ranger");
        assert_eq!(obj.pathfind_goal_cell, (-1, -1));
        assert_eq!(logic.pathfinding_system.grid.ground_goal_unit(cell), 0);
        assert_eq!(logic.pathfinding_system.grid.ground_goal_mask(cell), 0);
    }

    #[test]
    fn failed_move_path_returns_to_idle() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9609), Team::USA);
        unit.set_ai_state(AIState::Moving);
        unit.waiting_for_path = true;
        logic.objects.insert(ObjectId(9609), unit);
        logic.on_waited_path_failed(ObjectId(9609));
        let obj = logic.objects.get(&ObjectId(9609)).expect("ranger");
        assert!(!obj.waiting_for_path);
        assert_eq!(obj.ai_state, AIState::Idle);

        let mut attacker = Object::new(ThingTemplate::new("Ranger"), ObjectId(9610), Team::USA);
        attacker.set_ai_state(AIState::Attacking);
        attacker.waiting_for_path = true;
        logic.objects.insert(ObjectId(9610), attacker);
        logic.on_waited_path_failed(ObjectId(9610));
        let obj = logic.objects.get(&ObjectId(9610)).expect("attacker");
        assert!(!obj.waiting_for_path);
        assert_eq!(obj.ai_state, AIState::Attacking);
    }

    #[test]
    fn fast_repath_waits_one_logic_second() {
        let mut unit = Object::new(ThingTemplate::new("Ranger"), ObjectId(9620), Team::USA);
        assert!(unit.begin_request_move_path(Vec3::new(10.0, 0.0, 0.0), 100));
        assert_eq!(unit.path_timestamp, 0);
        assert!(unit.begin_request_move_path(Vec3::new(20.0, 0.0, 0.0), 102));
        assert_eq!(unit.queue_for_path_frames, 0);
        unit.path_timestamp = 100;
        assert!(!unit.begin_request_move_path(Vec3::new(20.0, 0.0, 0.0), 102));
        assert_eq!(unit.queue_for_path_frames, 30);
        assert!(unit.waiting_for_path);
        unit.queue_for_path_frames = 0;
        assert!(unit.begin_request_move_path(Vec3::new(30.0, 0.0, 0.0), 103));
        unit.movement.path = vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)];
        unit.is_blocked_and_stuck = true;
        unit.is_blocked = true;
        unit.num_frames_blocked = 12;
        unit.path_timestamp = 200;
        assert!(!unit.begin_request_move_path(Vec3::new(40.0, 0.0, 0.0), 202));
        assert_eq!(unit.queue_for_path_frames, 30);
        assert_eq!(unit.ignore_collisions_until_frame, 262);
        assert_eq!(unit.num_frames_blocked, 0);
        assert!(!unit.is_blocked);
        assert!(!unit.is_blocked_and_stuck);
        assert_eq!(unit.movement.path.len(), 2);
        unit.movement.path.clear();
        unit.is_blocked_and_stuck = true;
        unit.is_blocked = true;
        unit.num_frames_blocked = 8;
        unit.ignore_collisions_until_frame = 0;
        unit.path_timestamp = 300;
        assert!(!unit.begin_request_move_path(Vec3::new(50.0, 0.0, 0.0), 302));
        assert_eq!(unit.ignore_collisions_until_frame, 0);
        assert_eq!(unit.num_frames_blocked, 8);
        assert!(unit.is_blocked);
        assert!(unit.is_blocked_and_stuck);
    }

    #[test]
    fn flying_non_aircraft_gets_a_two_node_quick_path() {
        let mut logic = GameLogic::new();
        logic.frame = 15;
        let id = ObjectId(9641);
        let mut unit = Object::new(ThingTemplate::new("Hover"), id, Team::USA);
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
        unit.movement.max_speed = 20.0;
        unit.num_frames_blocked = 9;
        unit.is_blocked_and_stuck = true;
        unit.set_position(Vec3::new(10.0, 5.0, 10.0));
        logic.objects.insert(id, unit);
        let dest = Vec3::new(80.0, 20.0, 40.0);
        assert!(logic.assign_unit_path(id, dest, &[]));
        let obj = logic.objects.get(&id).expect("hover");
        assert_eq!(obj.movement.path.len(), 2);
        assert_eq!(obj.movement.path[0], Vec3::new(10.0, 20.0, 10.0));
        assert_eq!(obj.movement.path[1], dest);
        assert_eq!(obj.path_timestamp, 15);
        assert_eq!(obj.num_frames_blocked, 0);
        assert!(!obj.is_blocked_and_stuck);
        assert!(!obj.waiting_for_path);
        let mut approach = Object::new(ThingTemplate::new("Ranger"), ObjectId(9642), Team::USA);
        let first = Vec3::new(20.0, 0.0, 0.0);
        assert!(approach.begin_request_approach_path(first, 10));
        let kept = vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)];
        approach.movement.path = kept.clone();
        approach.path_timestamp = 10;
        assert!(!approach.begin_request_approach_path(Vec3::new(40.0, 0.0, 0.0), 12));
        assert_eq!(approach.queue_for_path_frames, 60);
        assert_eq!(approach.movement.path, kept);
        let jet_id = ObjectId(9643);
        let mut jet_tmpl = ThingTemplate::new("Jet");
        jet_tmpl.add_kind_of(KindOf::Aircraft);
        let mut jet = Object::new(jet_tmpl, jet_id, Team::USA);
        jet.loco_appearance = LocomotorAppearance::Wings;
        jet.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
        jet.movement.max_speed = 40.0;
        jet.set_position(Vec3::new(0.0, 30.0, 0.0));
        logic.objects.insert(jet_id, jet);
        let jet_dest = Vec3::new(90.0, 40.0, 20.0);
        assert!(logic.assign_unit_path(jet_id, jet_dest, &[]));
        let jet = logic.objects.get(&jet_id).expect("jet");
        assert!(!jet.waiting_for_path);
        assert!(jet.movement.path.len() >= 2);
        assert!((jet.movement.path[0].y - jet_dest.y).abs() < 0.01);
        assert_eq!(jet.movement.path.last().copied(), Some(jet_dest));
        {
            let jet = logic.objects.get_mut(&jet_id).expect("jet");
            jet.movement.current_path_index = 1;
            jet.num_frames_blocked = 4;
            jet.is_blocked_and_stuck = true;
        }
        let stamp = logic.objects.get(&jet_id).expect("jet").path_timestamp;
        let kept = logic.objects.get(&jet_id).expect("jet").movement.path.clone();
        assert!(logic.assign_unit_path(jet_id, jet_dest, &[]));
        let jet = logic.objects.get(&jet_id).expect("jet");
        assert_eq!(jet.movement.current_path_index, 1);
        assert_eq!(jet.path_timestamp, stamp);
        assert_eq!(jet.num_frames_blocked, 4);
        assert!(jet.is_blocked_and_stuck);
        assert_eq!(jet.movement.path, kept);
    }

    #[test]
    fn parachute_non_final_goal_keeps_the_raw_line() {
        let mut logic = GameLogic::new();
        logic.force_map_loaded_for_path_test(true);
        let id = ObjectId(9644);
        let mut tmpl = ThingTemplate::new("Pilot");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, id, Team::USA);
        unit.set_status_parachuting(true);
        unit.is_final_goal = false;
        unit.movement.max_speed = 20.0;
        unit.set_position(Vec3::new(10.0, 0.0, 10.0));
        logic.objects.insert(id, unit);
        let dest = Vec3::new(83.0, 0.0, 17.0);
        assert!(logic.assign_unit_path(id, dest, &[]));
        logic.process_pathfind_queue();
        let path = logic.objects.get(&id).expect("pilot").movement.path.clone();
        assert_eq!(path.len(), 2, "{path:?}");
        assert_eq!(path.last().copied(), Some(dest));
    }

    #[test]
    fn ultra_accurate_path_ends_on_the_ordered_point() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9701);
        let mut unit = Object::new(ThingTemplate::new("Dozer"), id, Team::USA);
        unit.ultra_accurate = true;
        unit.movement.max_speed = 10.0;
        logic.objects.insert(id, unit);
        let ordered = Vec3::new(43.0, 1.0, 2.0);
        let snapped = Vec3::new(40.0, 0.0, 0.0);
        assert!(logic.apply_computed_unit_path(
            id,
            Vec3::ZERO,
            ordered,
            vec![Vec3::ZERO, snapped],
            true,
        ));
        {
            let unit = logic.objects.get(&id).unwrap();
            assert_eq!(unit.movement.path.last().copied(), Some(ordered));
            assert_eq!(
                unit.locomotor_goal_type,
                crate::game_logic::object::LocoGoalType::PositionOnPath
            );
            assert_eq!(unit.locomotor_goal_angle, 0.0);
        }
        if let Some(unit) = logic.objects.get_mut(&id) {
            unit.set_locomotor_goal_none();
            unit.locomotor_goal_angle = 1.5;
        }
        assert!(logic.apply_computed_unit_path(
            id,
            Vec3::ZERO,
            ordered,
            vec![Vec3::ZERO, snapped],
            false,
        ));
        {
            let unit = logic.objects.get(&id).unwrap();
            assert_eq!(unit.movement.path.last().copied(), Some(snapped));
            assert_eq!(
                unit.locomotor_goal_type,
                crate::game_logic::object::LocoGoalType::None
            );
            assert_eq!(unit.locomotor_goal_angle, 1.5);
        }
    }



    #[test]
    fn stuck_failed_search_snaps_and_waits_one_second() {
        let mut logic = GameLogic::new();
        logic.force_map_loaded_for_path_test(true);
        let id = ObjectId(9710);
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, id, Team::USA);
        unit.movement.max_speed = 20.0;
        unit.is_final_goal = true;
        unit.set_position(Vec3::new(10.0, 0.0, 10.0));
        let kept = vec![Vec3::new(10.0, 0.0, 10.0), Vec3::new(20.0, 0.0, 10.0)];
        unit.movement.path = kept.clone();
        unit.num_frames_blocked = 8;
        unit.path_extra_distance = 100.0;
        unit.desired_speed = 5.0;
        unit.locomotor_goal_type = crate::game_logic::object::LocoGoalType::PositionExplicit;
        logic.objects.insert(id, unit);
        let w = logic.pathfinding_system.grid.width();
        let h = logic.pathfinding_system.grid.height();
        for x in 0..w {
            for y in 0..h {
                logic.pathfinding_system.grid.set_cell_type(
                    crate::game_logic::pathfinding::GridPos::new(x, y),
                    gamelogic::ai::pathfind_astar::PathfindCellType::Impassable,
                );
            }
        }
        let far = Vec3::new(80.0, 0.0, 80.0);
        assert!(!logic.assign_unit_path_for_test(id, far, &[]));
        let unit = logic.objects.get(&id).expect("ranger");
        assert!(unit.movement.path.is_empty());
        assert!(!unit.do_final_position);
        assert_eq!(unit.queue_for_path_frames, 0);
        assert!(!unit.waiting_for_path);
        assert!(!unit.is_blocked_and_stuck);
        assert_eq!(unit.num_frames_blocked, 0);
        assert_eq!(unit.path_extra_distance, 100.0);
        assert_eq!(unit.desired_speed, 5.0);
        assert_eq!(
            unit.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::None
        );
        let mut stuck_tmpl = ThingTemplate::new("Ranger");
        stuck_tmpl.add_kind_of(KindOf::Infantry);
        let mut stuck = Object::new(stuck_tmpl, ObjectId(9714), Team::USA);
        stuck.movement.max_speed = 20.0;
        stuck.is_final_goal = true;
        stuck.set_position(Vec3::new(10.0, 0.0, 10.0));
        stuck.movement.path = kept.clone();
        stuck.is_blocked_and_stuck = true;
        stuck.num_frames_blocked = 8;
        stuck.path_extra_distance = 100.0;
        stuck.desired_speed = 5.0;
        logic.objects.insert(ObjectId(9714), stuck);
        assert!(!logic.assign_unit_path_for_test(ObjectId(9714), far, &[]));
        let stuck = logic.objects.get(&ObjectId(9714)).expect("stuck");
        assert!(stuck.movement.path.is_empty());
        assert!(!stuck.do_final_position);
        assert_eq!(stuck.queue_for_path_frames, 30);
        assert!(stuck.waiting_for_path);
        assert_eq!(stuck.requested_destination, Some(far));
        assert_eq!(stuck.path_extra_distance, 100.0);
        assert_eq!(stuck.desired_speed, 5.0);

        let mut free_tmpl = ThingTemplate::new("Ranger");
        free_tmpl.add_kind_of(KindOf::Infantry);
        let mut free = Object::new(free_tmpl, ObjectId(9711), Team::USA);
        free.movement.max_speed = 20.0;
        free.is_final_goal = true;
        free.set_position(Vec3::new(10.0, 0.0, 10.0));
        free.movement.path = kept.clone();
        logic.objects.insert(ObjectId(9711), free);
        assert!(!logic.assign_unit_path_for_test(ObjectId(9711), far, &[]));
        let free = logic.objects.get(&ObjectId(9711)).expect("free");
        assert!(free.movement.path.is_empty());

        let mut open_tmpl = ThingTemplate::new("Ranger");
        open_tmpl.add_kind_of(KindOf::Infantry);
        let mut open = Object::new(open_tmpl, ObjectId(9712), Team::USA);
        open.movement.max_speed = 20.0;
        open.is_final_goal = true;
        open.set_position(Vec3::new(10.0, 0.0, 10.0));
        open.num_frames_blocked = 6;
        open.is_blocked_and_stuck = true;
        logic.objects.insert(ObjectId(9712), open);
        assert!(!logic.assign_unit_path_for_test(ObjectId(9712), far, &[]));
        let open = logic.objects.get(&ObjectId(9712)).expect("open");
        assert!(open.movement.path.is_empty());
        assert!(!open.do_final_position);
        assert!(!open.retry_path);
        assert_eq!(open.num_frames_blocked, 0);
        assert!(!open.is_blocked_and_stuck);

        let mut here_tmpl = ThingTemplate::new("Ranger");
        here_tmpl.add_kind_of(KindOf::Infantry);
        let mut here = Object::new(here_tmpl, ObjectId(9713), Team::USA);
        here.movement.max_speed = 20.0;
        here.is_final_goal = true;
        here.set_position(Vec3::new(10.0, 0.0, 10.0));
        here.movement.path = kept.clone();
        here.num_frames_blocked = 4;
        here.is_blocked_and_stuck = true;
        logic.objects.insert(ObjectId(9713), here);
        assert!(!logic.assign_unit_path_for_test(
            ObjectId(9713),
            Vec3::new(10.05, 0.0, 10.0),
            &[],
        ));
        let here = logic.objects.get(&ObjectId(9713)).expect("here");
        assert_eq!(here.movement.path, kept);
        assert!(!here.do_final_position);
        assert_eq!(here.num_frames_blocked, 0);
        assert!(!here.is_blocked_and_stuck);
    }

    #[test]
    fn stopping_clears_the_moving_model_condition() {
        use crate::game_logic::host_enum_table_residual::{MC_BIT_MOVING, moving_model_bit};
        let mut unit = Object::new(ThingTemplate::new("Ranger"), ObjectId(9720), Team::USA);
        let mc = 1u128 << MC_BIT_MOVING;
        let dock = 1u128 << moving_model_bit();
        unit.model_condition_bits |= mc | dock;
        unit.set_status_moving(true);
        assert_ne!(unit.model_condition_bits & mc, 0);
        unit.stop_moving();
        assert_eq!(unit.model_condition_bits & mc, 0);
        assert_eq!(unit.model_condition_bits & dock, 0);
        assert!(!unit.status.moving);
    }

    #[test]
    fn unpinched_pathfinder_cliff_sets_climb_or_rappel() {
        use crate::game_logic::host_enum_table_residual::{climbing_model_bit, rappelling_model_bit};
        let mut unit = Object::new(ThingTemplate::new("Infantry"), ObjectId(9721), Team::USA);
        unit.cell_is_cliff = true;
        unit.stamp_internal_move_cliff_model(true);
        let climb = 1u128 << climbing_model_bit();
        let rappel = 1u128 << rappelling_model_bit();
        assert_ne!(unit.model_condition_bits & climb, 0);
        assert_eq!(unit.model_condition_bits & rappel, 0);
        unit.moving_backwards = true;
        unit.stamp_internal_move_cliff_model(true);
        assert_eq!(unit.model_condition_bits & climb, 0);
        assert_ne!(unit.model_condition_bits & rappel, 0);
        unit.stamp_internal_move_cliff_model(false);
        assert_eq!(unit.model_condition_bits & (climb | rappel), 0);
        assert!(!unit.status.moving);
    }

    #[test]
    fn flying_projectile_repaths_to_a_moved_goal_every_frame() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9722);
        let mut tmpl = ThingTemplate::new("Missile");
        tmpl.add_kind_of(KindOf::Projectile);
        let mut unit = Object::new(tmpl, id, Team::USA);
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
        unit.loco_appearance = LocomotorAppearance::Thrust;
        unit.movement.max_speed = 40.0;
        unit.set_position(Vec3::new(0.0, 20.0, 0.0));
        unit.set_ai_state(AIState::Moving);
        unit.movement.path = vec![Vec3::ZERO, Vec3::new(10.0, 20.0, 0.0)];
        let goal = Vec3::new(80.0, 30.0, 40.0);
        unit.requested_destination = Some(goal);
        unit.path_timestamp = 100;


        logic.objects.insert(id, unit);
        logic.frame = 50;
        logic.update_movement(&[id], 1.0 / 30.0);
        let unit = logic.objects.get(&id).expect("missile");
        assert_eq!(unit.path_timestamp, 50);
        assert_eq!(unit.movement.path.last().copied(), Some(goal));
        assert_eq!(
            unit.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::PositionOnPath
        );
        let close_id = ObjectId(9723);
        let mut close_tmpl = ThingTemplate::new("Missile");
        close_tmpl.add_kind_of(KindOf::Projectile);
        let mut close = Object::new(close_tmpl, close_id, Team::USA);
        close.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
        close.loco_appearance = LocomotorAppearance::Thrust;
        close.movement.max_speed = 40.0;
        close.set_position(Vec3::new(0.0, 20.0, 0.0));
        close.set_ai_state(AIState::Moving);
        let close_goal = Vec3::new(10.1, 20.0, 0.0);
        close.movement.path = vec![Vec3::ZERO, close_goal];
        close.requested_destination = Some(close_goal);
        close.path_timestamp = 100;
        logic.objects.insert(close_id, close);
        logic.update_movement(&[close_id], 1.0 / 30.0);
        let close = logic.objects.get(&close_id).expect("close");
        assert_eq!(close.path_timestamp, 100);
        assert_eq!(close.movement.path.last().copied(), Some(close_goal));
        assert_eq!(
            close.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::PositionOnPath
        );
        let attack_id = ObjectId(9724);
        let mut attack_tmpl = ThingTemplate::new("Missile");
        attack_tmpl.add_kind_of(KindOf::Projectile);
        let mut attack = Object::new(attack_tmpl, attack_id, Team::USA);
        attack.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
        attack.loco_appearance = LocomotorAppearance::Thrust;
        attack.movement.max_speed = 40.0;
        attack.set_position(Vec3::new(0.0, 20.0, 0.0));
        attack.set_ai_state(AIState::Attacking);
        let attack_path = vec![Vec3::ZERO, Vec3::new(12.0, 20.0, 0.0)];
        attack.movement.path = attack_path.clone();
        attack.requested_destination = Some(Vec3::new(90.0, 30.0, 10.0));
        attack.set_locomotor_goal_position_explicit(Vec3::new(4.0, 0.0, 0.0));
        attack.path_timestamp = 100;
        logic.objects.insert(attack_id, attack);
        logic.update_movement(&[attack_id], 1.0 / 30.0);
        let attack = logic.objects.get(&attack_id).expect("attack");
        assert_eq!(attack.movement.path, attack_path);
        assert_eq!(attack.path_timestamp, 100);
        assert_eq!(
            attack.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::PositionExplicit
        );
    }
    #[test]
    fn move_state_repaths_after_ten_frames_when_the_goal_moves() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9725);
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, id, Team::USA);
        unit.movement.max_speed = 20.0;
        unit.set_position(Vec3::ZERO);
        unit.set_ai_state(AIState::Moving);
        let old = Vec3::new(30.0, 0.0, 0.0);
        let next = Vec3::new(120.0, 0.0, 40.0);
        unit.movement.path = vec![Vec3::ZERO, old];
        unit.path_goal_position = Some(old);
        unit.requested_destination = Some(next);
        unit.path_timestamp = 0;
        logic.objects.insert(id, unit);
        logic.frame = 5;
        logic.update_movement(&[id], 1.0 / 30.0);
        assert_eq!(
            logic.objects.get(&id).unwrap().movement.path.last().copied(),
            Some(old)
        );
        logic.frame = 20;
        logic.update_movement(&[id], 1.0 / 30.0);
        let unit = logic.objects.get(&id).expect("ranger");
        assert_eq!(unit.path_goal_position, Some(next));
        let last = unit.movement.path.last().copied().unwrap_or(Vec3::ZERO);
        assert!(
            last.distance(next) < 15.0,
            "goal moved, path should follow, got {last:?}"
        );
    }

    #[test]
    fn arrival_clears_locomotor_goal_only_when_adjusting() {
        let mut logic = GameLogic::new();
        let here = Vec3::new(20.0, 0.0, 0.0);
        let explicit = Vec3::new(5.0, 0.0, 0.0);
        let mut keep_tmpl = ThingTemplate::new("Ranger");
        keep_tmpl.add_kind_of(KindOf::Infantry);
        let mut keep = Object::new(keep_tmpl, ObjectId(9726), Team::USA);
        keep.movement.max_speed = 20.0;
        keep.set_position(here);
        keep.set_ai_state(AIState::Moving);
        keep.movement.path = vec![Vec3::ZERO, here];
        keep.movement.current_path_index = 1;
        keep.set_status_parachuting(true);
        keep.is_final_goal = true;
        keep.set_locomotor_goal_position_explicit(explicit);
        logic.objects.insert(ObjectId(9726), keep);
        logic.update_movement(&[ObjectId(9726)], 1.0 / 30.0);
        assert_eq!(
            logic.objects.get(&ObjectId(9726)).unwrap().locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::PositionExplicit
        );
        let mut clear_tmpl = ThingTemplate::new("Ranger");
        clear_tmpl.add_kind_of(KindOf::Infantry);
        let mut clear = Object::new(clear_tmpl, ObjectId(9727), Team::USA);
        clear.movement.max_speed = 20.0;
        clear.set_position(here);
        clear.set_ai_state(AIState::Moving);
        clear.movement.path = vec![Vec3::ZERO, here];
        clear.movement.current_path_index = 1;
        clear.is_final_goal = false;
        clear.set_locomotor_goal_position_explicit(explicit);
        logic.objects.insert(ObjectId(9727), clear);
        logic.update_movement(&[ObjectId(9727)], 1.0 / 30.0);
        assert_eq!(
            logic.objects.get(&ObjectId(9727)).unwrap().locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::None
        );
    }

    #[test]
    fn zero_delta_arrival_keeps_goal_unless_adjusting() {
        let mut logic = GameLogic::new();
        let here = Vec3::new(20.0, 0.0, 0.0);
        let explicit = Vec3::new(5.0, 0.0, 0.0);
        let mut keep_tmpl = ThingTemplate::new("Ranger");
        keep_tmpl.add_kind_of(KindOf::Infantry);
        let mut keep = Object::new(keep_tmpl, ObjectId(9728), Team::USA);
        keep.movement.max_speed = 20.0;
        keep.set_position(here);
        keep.set_ai_state(AIState::Moving);
        keep.movement.path = vec![Vec3::ZERO, here];
        keep.movement.current_path_index = 1;
        keep.is_final_goal = false;
        keep.adjust_destinations = false;
        keep.set_locomotor_goal_position_explicit(explicit);
        logic.objects.insert(ObjectId(9728), keep);
        logic.update_movement(&[ObjectId(9728)], 1.0 / 30.0);
        assert_eq!(
            logic
                .objects
                .get(&ObjectId(9728))
                .unwrap()
                .locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::PositionExplicit
        );
        let mut clear_tmpl = ThingTemplate::new("Ranger");
        clear_tmpl.add_kind_of(KindOf::Infantry);
        let mut clear = Object::new(clear_tmpl, ObjectId(9729), Team::USA);
        clear.movement.max_speed = 20.0;
        clear.set_position(here);
        clear.set_ai_state(AIState::Moving);
        clear.movement.path.clear();
        clear.movement.target_position = Some(here);
        clear.set_status_parachuting(true);
        clear.is_final_goal = true;
        clear.set_locomotor_goal_position_explicit(explicit);
        logic.objects.insert(ObjectId(9729), clear);
        logic.update_movement(&[ObjectId(9729)], 1.0 / 30.0);
        assert_eq!(
            logic.objects.get(&ObjectId(9729)).unwrap().locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::PositionExplicit
        );
    }





    #[test]
    fn entering_move_clears_blocked_frames() {
        let mut unit = Object::new(ThingTemplate::new("Ranger"), ObjectId(9640), Team::USA);
        unit.num_frames_blocked = 12;
        unit.is_blocked_and_stuck = true;
        unit.set_ai_state(AIState::Moving);
        assert_eq!(unit.num_frames_blocked, 0);
        assert!(!unit.is_blocked_and_stuck);
        assert!(unit.status.moving);
        unit.num_frames_blocked = 9;
        unit.is_blocked_and_stuck = true;
        unit.set_ai_state(AIState::Moving);
        assert_eq!(
            unit.num_frames_blocked, 9,
            "staying in Moving is not another onEnter"
        );
        assert!(unit.is_blocked_and_stuck);
    }


    #[test]
    fn fast_attack_approach_and_safe_repath_wait_two_seconds() {
        let mut attack = Object::new(ThingTemplate::new("Ranger"), ObjectId(9621), Team::USA);
        attack.set_locomotor_goal_position_explicit(Vec3::new(5.0, 0.0, 0.0));
        assert!(attack.begin_request_attack_path(None, Vec3::new(10.0, 0.0, 0.0), 100));
        attack.path_timestamp = 100;
        assert!(!attack.begin_request_attack_path(None, Vec3::new(20.0, 0.0, 0.0), 102));
        assert_eq!(attack.queue_for_path_frames, 60);
        assert_eq!(
            attack.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::None
        );

        let mut approach = Object::new(ThingTemplate::new("Ranger"), ObjectId(9622), Team::USA);
        approach.set_locomotor_goal_position_explicit(Vec3::new(5.0, 0.0, 0.0));
        assert!(approach.begin_request_approach_path(Vec3::new(10.0, 0.0, 0.0), 100));
        approach.path_timestamp = 100;
        assert!(!approach.begin_request_approach_path(Vec3::new(20.0, 0.0, 0.0), 102));
        assert_eq!(approach.queue_for_path_frames, 60);
        assert_ne!(
            approach.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::None
        );

        let mut safe = Object::new(ThingTemplate::new("Ranger"), ObjectId(9623), Team::USA);
        assert!(safe.begin_request_safe_path(ObjectId(1), Vec3::new(10.0, 0.0, 0.0), 100));
        safe.path_timestamp = 100;
        assert!(!safe.begin_request_safe_path(ObjectId(1), Vec3::new(20.0, 0.0, 0.0), 102));
        assert_eq!(safe.queue_for_path_frames, 60);
    }

    #[test]
    fn expired_path_delay_is_queued_again() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9624);
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, id, Team::USA);
        let dest = Vec3::new(40.0, 0.0, 0.0);
        assert!(unit.begin_request_move_path(dest, 100));
        unit.path_timestamp = 100;
        assert!(!unit.begin_request_move_path(Vec3::new(50.0, 0.0, 0.0), 102));
        unit.queue_for_path_frames = 1;
        unit.movement.max_speed = 20.0;
        logic.objects.insert(id, unit);
        logic.tick_physics_collisions_all();
        logic.process_pathfind_queue();
        let obj = logic.objects.get(&id).expect("ranger");
        assert_eq!(obj.queue_for_path_frames, 0);
        assert!(!obj.movement.path.is_empty());

        let atk_id = ObjectId(9625);
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut attack = Object::new(tmpl, atk_id, Team::USA);
        attack.movement.max_speed = 20.0;
        assert!(attack.begin_request_attack_path(None, Vec3::new(10.0, 0.0, 0.0), 200));
        attack.path_timestamp = 200;
        assert!(!attack.begin_request_attack_path(None, Vec3::new(30.0, 0.0, 0.0), 202));
        assert!(attack.is_attack_path);
        attack.queue_for_path_frames = 2;
        logic.objects.insert(atk_id, attack);
        logic.tick_physics_collisions_all();
        let mid = logic.objects.get(&atk_id).expect("attack");
        assert_eq!(mid.queue_for_path_frames, 1);
        assert!(mid.waiting_for_path);
        assert!(mid.movement.path.is_empty());
        logic.tick_physics_collisions_all();
        logic.process_pathfind_queue();
        let done = logic.objects.get(&atk_id).expect("attack");
        assert_eq!(done.queue_for_path_frames, 0);
        assert!(done.is_attack_path);
        assert!(!done.movement.path.is_empty());
    }

    #[test]
    fn safe_queue_one_repulsor_clears_the_old_path() {
        struct Restore(f32);
        impl Drop for Restore {
            fn drop(&mut self) {
                if let Ok(ai) = gamelogic::ai::the_ai().write() {
                    if let Ok(mut data) = ai.get_ai_data().write() {
                        data.repulsed_distance = self.0;
                    }
                }
            }
        }
        let previous = gamelogic::ai::the_ai()
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.repulsed_distance))
            .unwrap_or(0.0);
        let _restore = Restore(previous);
        let mut logic = GameLogic::new();
        if let Ok(ai) = gamelogic::ai::the_ai().write() {
            if let Ok(mut data) = ai.get_ai_data().write() {
                data.repulsed_distance = 40.0;
            }
        }
        let seen = gamelogic::ai::the_ai()
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|d| d.repulsed_distance))
            .unwrap_or(-1.0);
        assert_eq!(seen, 40.0, "repulsed_distance must be visible to the queue");
        let id = ObjectId(9631);
        let threat = ObjectId(9632);
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl.clone(), id, Team::USA);
        unit.movement.max_speed = 20.0;
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
        unit.vision_range = 30.0;
        unit.is_safe_path = true;
        unit.requested_victim_id = Some(threat);
        unit.is_attack_path = true;
        unit.set_locomotor_goal_position_explicit(Vec3::new(5.0, 0.0, 0.0));
        unit.safe_path_repulsor2 = None;
        unit.movement.path = vec![Vec3::new(999.0, 0.0, 999.0)];
        unit.set_position(Vec3::new(10.0, 0.0, 0.0));
        logic.objects.insert(id, unit);
        logic
            .objects
            .insert(threat, Object::new(tmpl, threat, Team::GLA));
        if let Some(t) = logic.objects.get_mut(&threat) {
            t.set_position(Vec3::ZERO);
        }
        let _ = logic
            .pathfinding_system
            .queue_path(crate::game_logic::pathfinding::PendingHostPath {
                unit_id: id,
                start: Vec3::new(10.0, 0.0, 0.0),
                destination: Vec3::new(40.0, 0.0, 0.0),
                waypoints: Vec::new(),
                aircraft: false,
                surfaces: 0,
                is_crusher: false,
                ignore_obstacle: None,
                adjust_destinations: true,
                restore_adjust_on_install: false,
            });
        logic.process_pathfind_queue();
        let obj = logic.objects.get(&id).expect("ranger");
        assert!(!obj.waiting_for_path);
        assert!(
            !obj.movement.path.is_empty(),
            "one repulsor must still produce a flee path"
        );
        let radius = 30.0 + 40.0;
        assert!(!obj.waiting_for_path);
        assert!(!obj.is_attack_path);
        assert_eq!(
            obj.locomotor_goal_type,
            crate::game_logic::object::LocoGoalType::None
        );
        let goal = *obj.movement.path.last().expect("flee goal");
        let goal_dist = goal.x.hypot(goal.z);
        assert!(
            goal_dist > radius,
            "flee goal {goal:?} is inside vision+repulsed_distance {radius}"
        );
        assert!(
            goal_dist < radius + 60.0,
            "flee goal {goal_dist} is too far for radius {radius}"
        );
    }

    #[test]
    fn airborne_approach_request_drops_the_approach_flag() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9633);
        let mut tmpl = ThingTemplate::new("Jet");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut unit = Object::new(tmpl, id, Team::USA);
        unit.loco_appearance = LocomotorAppearance::Wings;
        unit.movement.max_speed = 40.0;
        unit.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
        unit.is_approach_path = true;
        unit.movement.path = vec![Vec3::new(999.0, 0.0, 999.0)];
        unit.set_position(Vec3::new(0.0, 20.0, 0.0));
        logic.objects.insert(id, unit);
        let _ = logic
            .pathfinding_system
            .queue_path(crate::game_logic::pathfinding::PendingHostPath {
                unit_id: id,
                start: Vec3::new(0.0, 20.0, 0.0),
                destination: Vec3::new(80.0, 20.0, 0.0),
                waypoints: Vec::new(),
                aircraft: true,
                surfaces: 0,
                is_crusher: false,
                ignore_obstacle: None,
                adjust_destinations: true,
                restore_adjust_on_install: false,
            });
        logic.process_pathfind_queue();
        let obj = logic.objects.get(&id).expect("jet");
        assert!(!obj.is_approach_path, "an airborne approach is not findClosestPath");
        assert!(
            !obj.movement.path.is_empty(),
            "airborne falls through to computePath"
        );
        let dest = Vec3::new(80.0, 20.0, 0.0);
        let goal = *obj.movement.path.last().expect("path");
        assert!(
            goal.distance(dest) < Vec3::new(999.0, 0.0, 999.0).distance(dest),
            "the normal path install replaced the old path"
        );
    }














    #[test]
    fn blocked_speed_cap_resets_after_the_move() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9527), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.allow_invalid_position = true;
        unit.cur_max_blocked_speed = 4.0;
        unit.movement.max_speed = 40.0;
        unit.ground_height = 0.0;

        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9527), unit);
        logic.update_movement_for_test(&[ObjectId(9527)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9527)).expect("truck");
        assert!(
            obj.cur_max_blocked_speed > 1_000.0,
            "blocked speed cap must reset after the move, cap={}",
            obj.cur_max_blocked_speed
        );
    }

    #[test]
    fn emp_unit_does_not_move_its_turret() {
        use crate::game_logic::object::TurretSubState;
        let mut logic = GameLogic::new();
        let id = ObjectId(9630);
        let mut unit = Object::new(ThingTemplate::new("Tank"), id, Team::USA);
        unit.turret_enabled = true;
        unit.turret_substate = TurretSubState::Recenter;
        unit.turret_angle_deg = 40.0;
        unit.turret_natural_angle_deg = 0.0;
        unit.turret_turn_rate_rad = 1.0;
        unit.status.disabled_emp = true;
        logic.objects.insert(id, unit);
        logic.tick_all_turret_state_machines(&[id], 0.0, 1);
        let obj = logic.objects.get(&id).expect("tank");
        assert!(
            (obj.turret_angle_deg - 40.0).abs() < 0.01,
            "DISABLED_EMP must not run the turret"
        );
    }


    #[test]
    fn no_goal_tread_maintains_and_clears_braking() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Tank");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9533), Team::USA);
        unit.loco_appearance = LocomotorAppearance::Treads;
        unit.is_braking = true;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::new(3.0, 0.0, 4.0));
        unit.movement.velocity = Vec3::new(8.0, 0.0, 0.0);
        unit.movement.target_position = None;
        logic.objects.insert(ObjectId(9533), unit);
        logic.update_movement_for_test(&[ObjectId(9533)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9533)).expect("tank");
        assert!(!obj.is_braking);
        assert!(obj.maintain_pos_valid);
        assert_eq!(obj.maintain_pos, Some(Vec3::new(3.0, 0.0, 4.0)));
        assert!(obj.movement.velocity.length() < 1e-3);
    }

    #[test]
    fn blocked_speed_clamp_does_not_drop_the_frame_count() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9525), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.allow_invalid_position = true;
        unit.is_blocked = true;
        unit.num_frames_blocked = 5;
        unit.cur_max_blocked_speed = 1.0;
        unit.movement.max_speed = 40.0;
        unit.movement.acceleration = 1.0e6;
        unit.movement.velocity = Vec3::new(1.0, 0.0, 0.0);
        unit.set_orientation(0.0);
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9525), unit);
        logic.update_movement_for_test(&[ObjectId(9525)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9525)).expect("truck");
        assert!(
            obj.num_frames_blocked > 1,
            "a move still inside the blocked-speed clamp must keep the count, frames={}",
            obj.num_frames_blocked
        );
    }


    #[test]
    fn blocked_rotator_still_accumulates_frames() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Truck");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9526), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.allow_invalid_position = true;
        unit.is_blocked = true;
        unit.num_frames_blocked = 5;
        unit.cur_max_blocked_speed = 1.0;
        unit.movement.max_speed = 40.0;
        unit.movement.velocity = Vec3::new(1.0, 0.0, 0.0);
        // Face away from the goal so needToRotate is true.
        unit.set_orientation(std::f32::consts::PI);
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9526), unit);
        logic.update_movement_for_test(&[ObjectId(9526)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9526)).expect("truck");
        assert_eq!(
            obj.num_frames_blocked, 6,
            "doLocomotor must increment while blocked even if the unit needs to rotate"
        );
    }
    #[test]
    fn move_toward_goal_clears_maintain_position() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Hover");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9521), Team::USA);
        unit.loco_appearance = LocomotorAppearance::Hover;
        unit.maintain_pos_valid = true;
        unit.maintain_pos = Some(Vec3::new(0.0, 8.0, 0.0));
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.max_speed = 20.0;
        unit.movement.target_position = Some(Vec3::new(40.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9521), unit);
        logic.update_movement_for_test(&[ObjectId(9521)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9521)).expect("hover");
        assert!(
            !obj.maintain_pos_valid,
            "a move toward a goal must clear MAINTAIN_POS_IS_VALID"
        );
    }
    #[test]
    fn group_speed_factor_caps_the_live_march() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("HalfForm");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut unit = Object::new(tmpl, ObjectId(9523), Team::USA);
        unit.loco_appearance = LocomotorAppearance::WheelsFour;
        unit.group_speed_factor = 0.5;
        unit.movement.max_speed = 40.0;
        unit.movement.acceleration = 1.0e6;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9523), unit);
        logic.update_movement_for_test(&[ObjectId(9523)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9523)).expect("truck");
        let spd = obj.movement.velocity.length();
        assert!(
            spd <= 20.5 && spd > 15.0,
            "group factor 0.5 must cap a max of 40 once, spd={spd}"
        );
    }
    #[test]
    fn immune_landing_does_not_bounce() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Shell");
        tmpl.add_kind_of(KindOf::Projectile);
        tmpl.set_health(50.0);
        let mut unit = Object::new(tmpl, ObjectId(9517), Team::USA);
        unit.was_airborne_last_frame = true;
        unit.immune_to_falling_damage = true;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
        unit.health.current = 50.0;
        logic.objects.insert(ObjectId(9517), unit);
        logic.update_movement_for_test(&[ObjectId(9517)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9517)).expect("shell");
        assert_eq!(obj.bounce_land_events, 0);
        assert!(!obj.pending_ground_collide);
        assert!((obj.health.current - 50.0).abs() < 0.01);
    }

    #[test]
    fn resting_on_ground_kills_when_the_flag_is_set() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Mine");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.set_health(50.0);
        let mut unit = Object::new(tmpl, ObjectId(9516), Team::USA);
        unit.kill_when_resting_on_ground = true;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.velocity = Vec3::ZERO;
        unit.health.current = 50.0;
        logic.objects.insert(ObjectId(9516), unit);
        logic.update_movement_for_test(&[ObjectId(9516)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9516)).expect("mine");
        assert!(
            obj.status.destroyed || !obj.is_alive(),
            "a unit that comes to rest on the ground must die"
        );
    }

    #[test]
    fn stunned_landing_does_not_splat_twice() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("Faller");
        tmpl.add_kind_of(KindOf::Infantry);
        tmpl.set_health(200.0);
        let mut unit = Object::new(tmpl, ObjectId(9519), Team::USA);
        unit.was_airborne_last_frame = true;
        unit.shock_was_airborne = true;
        unit.shock_stun_frames = 40;
        unit.physics_mass = 1.0;
        unit.fall_height_damage_factor = 1.0;
        unit.ground_height = 0.0;
        unit.set_position(Vec3::ZERO);
        unit.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
        unit.health.current = 200.0;
        logic.objects.insert(ObjectId(9519), unit);
        logic.update_movement_for_test(&[ObjectId(9519)], 1.0 / 30.0);
        logic.tick_shock_stun_all();
        if let Some(o) = logic.objects.get_mut(&ObjectId(9519)) {
            let _ = o.tick_physics_motion_step(0.0);
        }
        let obj = logic.objects.get(&ObjectId(9519)).expect("faller");
        assert!(
            obj.health.current < 200.0 && obj.health.current > 150.0,
            "one landing splat, not two, hp={}",
            obj.health.current
        );
        logic.tick_physics_collisions_all();
        {
            let o = logic.objects.get_mut(&ObjectId(9519)).expect("faller");
            assert!(!o.landing_splat_done);
            o.was_airborne_last_frame = true;
            o.health.current = 200.0;
            o.set_position(Vec3::ZERO);
            o.movement.velocity = Vec3::new(0.0, -40.0, 0.0);
            let _ = o.tick_physics_motion_step(0.0);
        }
        let obj = logic.objects.get(&ObjectId(9519)).expect("faller");
        assert!(
            obj.health.current < 200.0,
            "a later physics-only landing must still splat, hp={}",
            obj.health.current
        );
    }

    #[test]
    fn downhill_only_refuses_an_uphill_path_waypoint() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("SkiPath");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(9503), Team::USA);
        unit.set_position(Vec3::ZERO);
        unit.loco_appearance = LocomotorAppearance::LegsTwo;
        unit.downhill_only = true;
        unit.movement.max_speed = 30.0;
        unit.movement.velocity = Vec3::new(8.0, 0.0, 0.0);
        unit.movement.path = vec![Vec3::ZERO, Vec3::new(20.0, 10.0, 0.0)];
        unit.movement.current_path_index = 0;
        unit.movement.target_position = Some(Vec3::new(20.0, 0.0, 0.0));
        unit.set_status_moving(true);
        logic.objects.insert(ObjectId(9503), unit);
        logic.update_movement_for_test(&[ObjectId(9503)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9503)).expect("ski");
        assert!(
            (obj.movement.velocity.x - 8.0).abs() < 0.5,
            "legs uphill return must not scrub velocity, vel={}",
            obj.movement.velocity
        );
        assert!(
            obj.get_position().x.abs() < 0.1,
            "downhill-only must stay put on an uphill path, pos={:?}",
            obj.get_position()
        );
    }

    #[test]
    fn climber_slows_on_steep_slope() {
        let mut unit = {
            let mut tmpl = ThingTemplate::new("RedGuardClimber");
            tmpl.add_kind_of(KindOf::Infantry);
            Object::new(tmpl, ObjectId(9502), Team::USA)
        };
        unit.loco_appearance = LocomotorAppearance::Climber;
        unit.set_position(Vec3::new(0.0, 20.0, 0.0));
        let goal = Vec3::new(10.0, 0.0, 0.0);
        let _ = unit.update_climber_flags(unit.get_position(), goal, 5.0);
        assert!(unit.is_climbing, " |dz| > cell must set FLAG_CLIMBING");
        let scale = unit.climber_slope_speed_scale(20.0, 5.0);
        assert!(
            scale < 0.05,
            "slope 15 must divide speed by 60, scale={scale}"
        );
    }

    /// hq-tb3v5: path lead must keep goal Y so FLAG_CLIMBING latches.
    #[test]
    fn climber_path_lead_keeps_goal_y_and_latches_climbing() {
        let mut logic = GameLogic::new();
        let id = ObjectId(95021);
        let mut unit = {
            let mut tmpl = ThingTemplate::new("RedGuardClimberPath");
            tmpl.add_kind_of(KindOf::Infantry);
            Object::new(tmpl, id, Team::USA)
        };
        unit.loco_appearance = LocomotorAppearance::Climber;
        unit.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
        unit.set_position(Vec3::new(0.0, 20.0, 0.0));
        unit.ground_height = 20.0;
        unit.movement.max_speed = 30.0;
        unit.movement.acceleration = 10_000.0;
        unit.no_slow_down_as_approaching_dest = true;
        unit.set_status_moving(true);
        let start = Vec3::new(0.0, 20.0, 0.0);
        let goal = Vec3::new(40.0, 0.0, 0.0);
        unit.movement.path = vec![start, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("climber");
        let stored_y = obj
            .movement
            .target_position
            .expect("climber must keep a goal")
            .y;
        assert!(
            stored_y < 1.0,
            "climber path lead must not flatten goal Y, stored_y={stored_y}"
        );
        assert!(
            obj.is_climbing,
            " |dz| > PATHFIND_CELL_SIZE_F must latch FLAG_CLIMBING"
        );
    }

    /// hq-tb3v5: descent while CLIMBING faces away and drives reverse toward goal.
    #[test]
    fn climber_descent_reverses_toward_goal() {
        let mut logic = GameLogic::new();
        let gw = logic.pathfinding_system.grid.width().max(0) as u32;
        let gh = logic.pathfinding_system.grid.height().max(0) as u32;
        assert!(gw > 0 && gh > 0, "host grid must exist for height samples");
        // Sit just before a cell boundary so the 1-unit climb probe crosses
        // into a lower cell (cache is per-cell; leftover samples terrain).
        let start = Vec3::new(3.5, 20.0, 0.0);
        let goal = Vec3::new(43.5, 0.0, 0.0);
        let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
        let mut heights = vec![5.0; gw as usize * gh as usize];
        if start_cell.x >= 0
            && start_cell.y >= 0
            && (start_cell.x as u32) < gw
            && (start_cell.y as u32) < gh
        {
            heights[(start_cell.y as u32 * gw + start_cell.x as u32) as usize] = 20.0;
        }
        assert!(logic.restore_terrain_heights_from_grid(gw, gh, &heights));

        let id = ObjectId(95022);
        let mut unit = {
            let mut tmpl = ThingTemplate::new("RedGuardClimberDescent");
            tmpl.add_kind_of(KindOf::Infantry);
            Object::new(tmpl, id, Team::USA)
        };
        unit.loco_appearance = LocomotorAppearance::Climber;
        unit.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
        // Height-map interpolation at a cell edge can report ground << 20 and
        // trip treatAsAirborne (~0.64wu). Motive still applies on the cliff.
        unit.allow_motive_force_while_airborne = true;
        unit.set_position(start);
        // Already facing away so angleCoeff is 0 and reverse drive is live
        // this frame (Locomotor.cpp:1760-1774).
        unit.set_orientation(std::f32::consts::PI);
        unit.ground_height = 20.0;
        unit.movement.max_speed = 30.0;
        unit.movement.acceleration = 10_000.0;
        unit.movement.turn_rate = 0.0;
        unit.no_slow_down_as_approaching_dest = true;
        unit.set_status_moving(true);
        unit.movement.path = vec![start, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("climber");
        assert!(obj.is_climbing, "descent goal must latch FLAG_CLIMBING");
        assert!(
            obj.moving_backwards,
            "ground 1wu ahead lower must set MOVING_BACKWARDS"
        );
        assert!(
            obj.movement.velocity.x > 0.4 && obj.movement.velocity.x < 1.2,
            "slope cut must be far below max speed 30, vel={:?}",
            obj.movement.velocity
        );
        assert!(
            obj.get_orientation().abs() > 2.5,
            "descent must keep facing away from the goal, yaw={}",
            obj.get_orientation()
        );
    }

    #[test]
    fn legs_brake_uses_min_speed_not_dest_zero_snap() {
        let mut unit = {
            let mut tmpl = ThingTemplate::new("RangerLegs");
            tmpl.add_kind_of(KindOf::Infantry);
            Object::new(tmpl, ObjectId(9503), Team::USA)
        };
        unit.loco_appearance = LocomotorAppearance::LegsTwo;
        unit.min_speed = 4.0;
        unit.braking = 20.0;
        unit.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
        let goal = unit.apply_cpp_approach_brake(2.0, 20.0, 20.0, 0);
        assert!(
            !unit.is_braking,
            "legs must not set IS_BRAKING (Locomotor.cpp:1648-1653)"
        );
        assert!(
            (goal - 4.0).abs() < 1e-4,
            "legs must drop to minSpeed not 0, goal={goal}"
        );

        let mut hover = {
            let mut tmpl = ThingTemplate::new("ComancheHover");
            tmpl.add_kind_of(KindOf::Aircraft);
            Object::new(tmpl, ObjectId(9507), Team::USA)
        };
        hover.loco_appearance = LocomotorAppearance::Hover;
        hover.min_speed = 4.0;
        hover.braking = 20.0;
        hover.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
        let hover_goal = hover.apply_cpp_approach_brake(2.0, 20.0, 20.0, 0);
        assert!(
            !hover.is_braking,
            "hover must not set IS_BRAKING (Locomotor.cpp:2368-2374)"
        );
        assert!((hover_goal - 4.0).abs() < 1e-4);
    }

    #[test]
    fn treads_use_squared_braking_factor() {
        let mut tank = {
            let mut tmpl = ThingTemplate::new("Crusader");
            tmpl.add_kind_of(KindOf::Vehicle);
            Object::new(tmpl, ObjectId(9504), Team::USA)
        };
        tank.loco_appearance = LocomotorAppearance::Treads;
        tank.braking = 10.0;
        tank.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
        let _ = tank.apply_cpp_approach_brake(5.0, 30.0, 30.0, 0);
        assert!(tank.is_braking);
        assert!(
            tank.braking_factor > 1.0,
            "treads must square braking_factor, got {}",
            tank.braking_factor
        );
        assert!(tank.braking_factor <= 5.0);
    }

    #[test]
    fn wheels_donut_forces_brake_and_wings_never_brake() {
        let mut truck = {
            let mut tmpl = ThingTemplate::new("Humvee");
            tmpl.add_kind_of(KindOf::Vehicle);
            Object::new(tmpl, ObjectId(9505), Team::USA)
        };
        truck.loco_appearance = LocomotorAppearance::WheelsFour;
        truck.braking = 10.0;
        truck.donut_timer = 0;
        let _ = truck.apply_cpp_approach_brake(5.0, 10.0, 20.0, 80);
        assert!(
            truck.is_braking,
            "donut timer expired must force IS_BRAKING"
        );
        assert!(
            (truck.braking_factor - 1.0).abs() < 1e-5,
            "wheels overwrite braking_factor to 1.0"
        );

        let mut jet = {
            let mut tmpl = ThingTemplate::new("Raptor");
            tmpl.add_kind_of(KindOf::Aircraft);
            Object::new(tmpl, ObjectId(9506), Team::USA)
        };
        jet.loco_appearance = LocomotorAppearance::Wings;
        jet.is_braking = true;
        jet.min_speed = 10.0;
        jet.braking = 10.0;
        jet.movement.max_speed = 40.0;
        let goal = jet.apply_cpp_approach_brake(1.0, 40.0, 40.0, 0);
        assert!(!jet.is_braking, "wings never latch IS_BRAKING");
        assert!(
            (goal - 10.0).abs() < 1e-5,
            "wings must floor to minSpeed on approach, got {goal}"
        );
        let cruise = jet.apply_cpp_approach_brake(200.0, 40.0, 40.0, 0);
        assert!(
            (cruise - 40.0).abs() < 1e-5,
            "wings keep cruise when outside slowDownDist, got {cruise}"
        );
    }

    #[test]
    fn path_raise_latches_is_braking_at_2x_before_raise() {
        let mut tank = {
            let mut tmpl = ThingTemplate::new("Crusader");
            tmpl.add_kind_of(KindOf::Vehicle);
            Object::new(tmpl, ObjectId(9507), Team::USA)
        };
        tank.loco_appearance = LocomotorAppearance::Treads;
        let raised = tank.raise_on_path_dist_to_goal(80.0, 10.0);
        assert!(
            tank.is_braking,
            "dist 80 > 2*on_path 10 must latch IS_BRAKING (Locomotor.cpp:980-992)"
        );
        assert!((raised - 80.0).abs() < 1e-5);

        let mut proj = {
            let mut tmpl = ThingTemplate::new("Missile");
            tmpl.add_kind_of(KindOf::Projectile);
            Object::new(tmpl, ObjectId(9508), Team::USA)
        };
        proj.object_type = crate::game_logic::ObjectType::Projectile;
        let proj_raised = proj.raise_on_path_dist_to_goal(80.0, 10.0);
        assert!(!proj.is_braking, "projectiles must not 2x-latch IS_BRAKING");
        assert!((proj_raised - 80.0).abs() < 1e-5);
    }

    #[test]
    fn wheeled_min_turn_speed_floor_uses_max_speed() {
        let mut truck = {
            let mut tmpl = ThingTemplate::new("Humvee");
            tmpl.add_kind_of(KindOf::Vehicle);
            Object::new(tmpl, ObjectId(9509), Team::USA)
        };
        truck.loco_appearance = LocomotorAppearance::WheelsFour;
        truck.min_turn_speed = 0.0;
        truck.movement.max_speed = 40.0;
        let floor = truck.wheeled_turn_speed_floor();
        assert!(
            (floor - 10.0).abs() < 1e-5,
            "floor is maxSpeed/4=10, not reduced desiredSpeed/4, got {floor}"
        );
    }

    #[test]
    fn treads_angle_coeff_slows_hard_turns() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9601);
        let mut tmpl = ThingTemplate::new("Crusader");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut tank = Object::new(tmpl, id, Team::USA);
        tank.set_position(Vec3::ZERO);
        tank.set_orientation(0.0);
        tank.loco_appearance = LocomotorAppearance::Treads;
        tank.movement.turn_rate = std::f32::consts::PI;
        tank.movement.max_speed = 40.0;
        tank.movement.acceleration = 10_000.0;
        tank.movement.velocity = Vec3::ZERO;
        tank.no_slow_down_as_approaching_dest = true;
        tank.movement.target_position = Some(Vec3::new(0.0, 0.0, 80.0));
        logic.objects.insert(id, tank);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("tank");
        assert!(
            obj.movement.velocity.length() < 1.0,
            "90° treads turn must zero goalSpeed via angleCoeff, vel={}",
            obj.movement.velocity.length()
        );
        assert!(
            obj.get_orientation().abs() > 1e-4,
            "treads must still yaw toward the goal"
        );
    }

    #[test]
    fn wheeled_can_move_backwards_reverses_nearby_rear_goal() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9602);
        let mut tmpl = ThingTemplate::new("Humvee");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut truck = Object::new(tmpl, id, Team::USA);
        truck.set_position(Vec3::ZERO);
        truck.set_orientation(0.0);
        truck.loco_appearance = LocomotorAppearance::WheelsFour;
        truck.can_move_backward = true;
        truck.min_turn_speed = 15.0;
        truck.movement.max_speed = 40.0;
        truck.movement.turn_rate = std::f32::consts::PI;
        truck.movement.acceleration = 30.0;
        truck.braking = 1.0;
        truck.movement.velocity = Vec3::ZERO;
        truck.no_slow_down_as_approaching_dest = true;
        truck.thing.template.geometry_info.authored = true;
        truck.thing.template.geometry_info.major_radius = 8.0;
        truck.movement.target_position = Some(Vec3::new(-20.0, 0.0, 0.0));
        logic.objects.insert(id, truck);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("truck");
        assert!(
            obj.moving_backwards,
            "CanMoveBackwards + rear goal at rest must set MOVING_BACKWARDS"
        );
        assert!(
            !obj.doing_three_point_turn,
            "goal inside 5*majorRadius must not set DOING_THREE_POINT_TURN"
        );
        assert!(
            (obj.movement.velocity.x + 1.0).abs() < 0.05,
            "reverse startup must take the accel step (30/30), not braking/30, vel={:?}",
            obj.movement.velocity
        );
        assert!(
            obj.get_orientation().abs() < 0.2,
            "nearby reverse must not flip heading, yaw={}",
            obj.get_orientation()
        );
    }

    /// hq-t505c: live march must yaw about TurnPivotOffset, not hull center.
    #[test]
    fn march_yaws_around_turn_pivot_offset() {
        let mut logic = GameLogic::new();
        let mk = |id, offset| {
            let mut tmpl = ThingTemplate::new("CombatBike");
            tmpl.add_kind_of(KindOf::Vehicle);
            let mut bike = Object::new(tmpl, ObjectId(id), Team::USA);
            bike.set_position(Vec3::ZERO);
            bike.set_orientation(0.0);
            bike.loco_appearance = LocomotorAppearance::Motorcycle;
            bike.turn_pivot_offset = offset;
            bike.selection_radius = 10.0;
            bike.min_turn_speed = 5.0;
            bike.movement.turn_rate = std::f32::consts::PI;
            bike.movement.max_speed = 40.0;
            bike.movement.acceleration = 10_000.0;
            bike.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
            bike.no_slow_down_as_approaching_dest = true;
            bike.movement.target_position = Some(Vec3::new(0.0, 0.0, 80.0));
            bike
        };
        logic.objects.insert(ObjectId(9610), mk(9610, 0.0));
        logic.objects.insert(ObjectId(9611), mk(9611, -0.60));
        logic.update_movement_for_test(&[ObjectId(9610), ObjectId(9611)], 1.0 / 30.0);
        let center = logic.objects.get(&ObjectId(9610)).unwrap().get_position();
        let pivoted = logic.objects.get(&ObjectId(9611)).unwrap().get_position();
        let drift = (pivoted.x - center.x).abs() + (pivoted.z - center.z).abs();
        assert!(
            drift > 1e-3,
            "TurnPivotOffset must translate hull vs center yaw, center={center:?} pivoted={pivoted:?}"
        );
    }

    /// hq-py0re: Wings idle hold circles instead of freezing at last waypoint.
    #[test]
    fn wings_idle_hold_circles_at_min_speed() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9701);
        let mut tmpl = ThingTemplate::new("Raptor");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut jet = Object::new(tmpl, id, Team::USA);
        jet.set_position(Vec3::new(0.0, 50.0, 0.0));
        jet.ground_height = 0.0;
        jet.loco_appearance = LocomotorAppearance::Wings;
        jet.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        jet.min_speed = 20.0;
        jet.circling_radius = 40.0;
        jet.movement.max_speed = 80.0;
        jet.movement.acceleration = 10_000.0;
        jet.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
        jet.motive_frames_remaining = 10;
        jet.status.airborne_target = true;
        jet.movement.path = vec![Vec3::new(0.0, 50.0, 0.0)];
        jet.movement.current_path_index = 0;
        jet.movement.target_position = Some(Vec3::new(0.0, 50.0, 0.0));
        let start = jet.get_position();
        logic.objects.insert(id, jet);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("jet");
        assert!(
            obj.maintain_pos_valid,
            "wings hold must install maintain pos"
        );
        assert!(
            obj.movement.velocity.length() > 5.0,
            "wings must keep flying, vel={}",
            obj.movement.velocity.length()
        );
        let moved = obj.get_position().distance(start);
        assert!(moved > 0.05, "wings idle must circle, moved={moved}");
        assert!(
            obj.movement.target_position.is_none(),
            "hold must not keep a grounded move order"
        );
    }

    /// hq-66eos: SET_NORMAL cliff member activates on CELL_CLIFF.
    #[test]
    fn choose_good_locomotor_switches_on_cliff_cell() {
        let _ = crate::game_logic::locomotor_bootstrap::ensure_host_locomotor_store();
        let mut logic = GameLogic::new();
        let id = ObjectId(9702);
        let mut tmpl = ThingTemplate::new("CombatBike");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut bike = Object::new(tmpl, id, Team::USA);
        bike.set_position(Vec3::new(15.0, 0.0, 15.0));
        bike.locomotor_set_names = vec![
            crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_GROUND_LOCOMOTOR.to_string(),
            crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_CLIFF_LOCOMOTOR.to_string(),
        ];
        bike.cur_locomotor_name =
            Some(crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_GROUND_LOCOMOTOR.to_string());
        bike.locomotor_surfaces = crate::game_logic::LOCO_SURFACE_GROUND;
        let cell = logic
            .pathfinding_system
            .grid
            .world_to_grid(bike.get_position());
        logic
            .pathfinding_system
            .grid
            .set_cell_type(cell, gamelogic::ai::pathfind_astar::PathfindCellType::Cliff);
        logic.objects.insert(id, bike);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("bike");
        assert_eq!(
            obj.cur_locomotor_name.as_deref(),
            Some(crate::game_logic::locomotor_bootstrap::COMBAT_BIKE_CLIFF_LOCOMOTOR),
            "cliff cell must pick CombatBikeCliffLocomotor"
        );
        assert_eq!(
            obj.locomotor_surfaces & crate::game_logic::LOCO_SURFACE_CLIFF,
            crate::game_logic::LOCO_SURFACE_CLIFF
        );
    }

    /// hq-66eos: known SET_NORMAL members bind from template name (no manual list).
    #[test]
    fn choose_good_locomotor_fills_burton_set_from_template_name() {
        let _ = crate::game_logic::locomotor_bootstrap::ensure_host_locomotor_store();
        let mut logic = GameLogic::new();
        let id = ObjectId(97021);
        let mut tmpl = ThingTemplate::new("AmericaInfantryColonelBurton");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut burton = Object::new(tmpl, id, Team::USA);
        burton.set_position(Vec3::new(25.0, 0.0, 25.0));
        burton.cur_locomotor_name = Some(
            crate::game_logic::locomotor_bootstrap::COLONEL_BURTON_GROUND_LOCOMOTOR.to_string(),
        );
        burton.locomotor_surfaces = crate::game_logic::LOCO_SURFACE_GROUND;
        let cell = logic
            .pathfinding_system
            .grid
            .world_to_grid(burton.get_position());
        logic
            .pathfinding_system
            .grid
            .set_cell_type(cell, gamelogic::ai::pathfind_astar::PathfindCellType::Cliff);
        logic.objects.insert(id, burton);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("burton");
        assert_eq!(
            obj.cur_locomotor_name.as_deref(),
            Some(crate::game_logic::locomotor_bootstrap::HUMAN_CLIFF_LOCOMOTOR),
            "cliff cell must pick HumanCliffLocomotor"
        );
        assert_eq!(obj.loco_appearance, LocomotorAppearance::Climber);
        assert_eq!(
            obj.locomotor_surfaces & crate::game_logic::LOCO_SURFACE_CLIFF,
            crate::game_logic::LOCO_SURFACE_CLIFF
        );
    }

    /// hq-ene6j: Hover OVER_WATER is sampled from the water table.
    #[test]
    fn hover_sets_over_water_from_water_table() {
        let mut logic = GameLogic::new();
        #[cfg(feature = "game_client")]
        {
            use crate::game_logic::terrain::TerrainData;
            use game_client::terrain::height_map::HeightMap;
            let mut hm = HeightMap::new(8, 8, 100.0, 1.0);
            for h in hm.heights.iter_mut() {
                *h = 0.05;
            }
            let mut terrain = TerrainData::from_heightmap(
                hm,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(70.0, 0.0, 70.0),
                0,
            );
            terrain.water_plane_y = Some(20.0);
            logic.terrain = Some(terrain);
        }
        let id = ObjectId(9703);
        let mut tmpl = ThingTemplate::new("CombatChinook");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, id, Team::USA);
        heli.set_position(Vec3::new(20.0, 5.0, 20.0));
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        heli.over_water = false;
        logic.objects.insert(id, heli);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("heli");
        #[cfg(feature = "game_client")]
        {
            assert!(obj.over_water, "hover over water plane must set OVER_WATER");
            let bit = crate::game_logic::host_enum_table_residual::over_water_model_bit();
            assert_ne!(obj.model_condition_bits & (1u128 << bit), 0);
        }
        #[cfg(not(feature = "game_client"))]
        {
            let mut heli = Object::new(
                {
                    let mut t = ThingTemplate::new("CombatChinook");
                    t.add_kind_of(KindOf::Aircraft);
                    t
                },
                ObjectId(97031),
                Team::USA,
            );
            heli.loco_appearance = LocomotorAppearance::Hover;
            heli.apply_hover_over_water(true);
            assert!(heli.over_water);
        }
    }

    /// hq-89bqp: blocked-wait caps speed before the march via bumpSpeedLimit.
    #[test]
    fn blocked_wait_caps_speed_before_march() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9704);
        let mut unit = ranger_at(9704, Vec3::ZERO);
        unit.set_orientation(0.0);
        unit.movement.max_speed = 40.0;
        unit.movement.acceleration = 10_000.0;
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.is_blocked = true;
        unit.cur_max_blocked_speed = 4.0;
        unit.bump_speed_limit = 4.0;
        unit.no_slow_down_as_approaching_dest = true;
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.movement.velocity.length() < 5.0,
            "blocked march must use bumpSpeedLimit, vel={}",
            obj.movement.velocity.length()
        );
        assert!(
            obj.bump_speed_limit < 4.0,
            "blocked must decay bumpSpeedLimit * 0.95, bump={}",
            obj.bump_speed_limit
        );
        assert_eq!(obj.num_frames_blocked, 1);
    }

    #[test]
    fn blocked_and_stuck_when_other_stopped() {
        let mut self_u = ranger_at(9705, Vec3::ZERO);
        self_u.set_orientation(0.0);
        self_u.movement.velocity = Vec3::new(10.0, 0.0, 0.0);
        self_u.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        let mut other = ranger_at(9706, Vec3::new(8.0, 0.0, 0.0));
        other.set_orientation(0.0);
        other.movement.velocity = Vec3::ZERO;
        assert!(self_u.ai_process_collision(&other, 1, true, true) == false);
        assert!(self_u.is_blocked);
        assert!(
            self_u.is_blocked_and_stuck,
            "other stopped + facing dest must stick immediately"
        );
    }

    #[test]
    fn march_apply_motive_force_zero_flags_driven() {
        let mut logic = GameLogic::new();
        let mut unit = ranger_at(9801, Vec3::ZERO);
        unit.set_orientation(0.0);
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.motive_frames_remaining = 0;
        logic.objects.insert(ObjectId(9801), unit);
        logic.update_movement_for_test(&[ObjectId(9801)], 1.0 / 30.0);
        {
            let obj = logic.objects.get(&ObjectId(9801)).expect("unit");
            assert_eq!(
                obj.motive_frames_remaining,
                crate::game_logic::MOTIVE_FRAMES_RESIDUAL,
                "C++ applyMotiveForce(0) must arm motive so collide is lateral-only"
            );
        }
        let obj = logic.objects.get_mut(&ObjectId(9801)).expect("unit");
        obj.apply_physics_force(Vec3::new(10.0, 0.0, 0.0));
        assert!(
            obj.physics_accel.x.abs() < 1e-4,
            "motive march must reject forward collide force, accel.x={}",
            obj.physics_accel.x
        );
    }

    #[test]
    fn start_move_resets_donut_so_short_order_does_not_instant_brake() {
        let mut truck = {
            let mut tmpl = ThingTemplate::new("Humvee");
            tmpl.add_kind_of(KindOf::Vehicle);
            Object::new(tmpl, ObjectId(9810), Team::USA)
        };
        truck.loco_appearance = LocomotorAppearance::WheelsFour;
        truck.braking = 10.0;
        truck.movement.max_speed = 20.0;
        truck.donut_timer = 0;
        truck.start_move();
        assert_eq!(truck.donut_timer, u32::MAX);
        let _ = truck.apply_cpp_approach_brake(35.0, 10.0, 20.0, 0);
        assert!(
            !truck.is_braking,
            "startMove must open a 2.5s donut window (Locomotor.cpp:761-765)"
        );
    }

    #[test]
    fn maintain_resets_donut_and_clears_braking() {
        let mut truck = {
            let mut tmpl = ThingTemplate::new("Humvee");
            tmpl.add_kind_of(KindOf::Vehicle);
            Object::new(tmpl, ObjectId(9811), Team::USA)
        };
        truck.loco_appearance = LocomotorAppearance::WheelsFour;
        truck.is_braking = true;
        truck.donut_timer = 0;
        let _ = truck.loco_maintain_current_position(0.0, 1.0 / 30.0);
        assert!(!truck.is_braking, "maintain must clear IS_BRAKING");
        assert_eq!(
            truck.donut_timer,
            u32::MAX,
            "maintain must reset donut timer (Locomotor.cpp:2420-2421)"
        );
    }

    #[test]
    fn hover_maintain_bleeds_speed_not_scrub() {
        let mut hover = {
            let mut tmpl = ThingTemplate::new("Comanche");
            tmpl.add_kind_of(KindOf::Aircraft);
            Object::new(tmpl, ObjectId(9812), Team::USA)
        };
        hover.loco_appearance = LocomotorAppearance::Hover;
        hover.min_speed = 0.0;
        hover.braking = 5.0;
        hover.movement.acceleration = 5.0;
        hover.set_orientation(0.0);
        let dir = hover.unit_direction_vector_2d();
        hover.movement.velocity = Vec3::new(dir.x * 20.0, 0.0, dir.y * 20.0);
        hover.motive_frames_remaining = 3;
        let _ = hover.loco_maintain_current_position(0.0, 1.0 / 30.0);
        let speed = hover.forward_speed_2d();
        assert!(
            speed > 1.0,
            "hover maintain must not scrub vel to 0, speed={speed}"
        );
        assert!(
            speed < 20.0,
            "hover maintain must apply brake force, speed={speed}"
        );
    }

    #[test]
    fn dist_along_path_is_remaining_not_lead() {
        let path = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 100.0),
        ];
        let pos = Vec3::new(50.0, 0.0, 0.0);
        let remaining = crate::game_logic::PathfindingSystem::dist_along_path(pos, &path);
        assert!(
            (remaining - 150.0).abs() < 0.5,
            "closest-point remaining must be 150, got {remaining}"
        );
        let lead = crate::game_logic::PathfindingSystem::compute_point_on_path(pos, &path);
        let lead_d = {
            let dx = pos.x - lead.x;
            let dz = pos.z - lead.z;
            (dx * dx + dz * dz).sqrt()
        };
        assert!(
            remaining > lead_d + 40.0,
            "distAlongPath {remaining} must exceed lead range {lead_d}"
        );
    }

    /// hq-wwtka: hover/air flight-dist is projected remaining, not closest-point winding.
    #[test]
    fn flight_dist_to_goal_does_not_snap_to_later_dogleg() {
        let path = [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 0.0),
            Vec3::new(100.0, 0.0, 100.0),
        ];
        let corner_cut = Vec3::new(50.0, 0.0, 50.0);
        let winding = crate::game_logic::PathfindingSystem::dist_along_path(corner_cut, &path);
        let flight =
            crate::game_logic::PathfindingSystem::compute_flight_dist_to_goal(corner_cut, &path);
        assert!(
            (winding - 150.0).abs() < 0.5,
            "closest-point winding must stay 150, got {winding}"
        );
        assert!(
            (flight - 100.0).abs() < 0.5,
            "computeFlightDistToGoal must be 50+50=100, got {flight}"
        );
        assert!(
            flight + 40.0 < winding,
            "flight remaining {flight} must be shorter than winding {winding}"
        );
    }

    #[test]
    fn approach_brake_does_not_trigger_mid_long_path() {
        let mut tank = {
            let mut tmpl = ThingTemplate::new("Crusader");
            tmpl.add_kind_of(KindOf::Vehicle);
            Object::new(tmpl, ObjectId(9813), Team::USA)
        };
        tank.loco_appearance = LocomotorAppearance::Treads;
        tank.braking = 10.0;
        tank.movement.max_speed = 30.0;
        tank.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
        tank.is_braking = true;
        let _ = tank.apply_cpp_approach_brake(200.0, 30.0, 30.0, 0);
        assert!(
            !tank.is_braking,
            "treads unlatch when on_path > 2*slowDownDist (Locomotor.cpp:1200-1203)"
        );
    }

    /// C++ AIUpdate.cpp:1027-1041 — setFinalPosition arms the path's last
    /// node; occupancy re-snap only when farther than one cell; no teleport.
    #[test]
    fn arrival_arms_final_position_at_path_last_node() {
        let mut logic = GameLogic::new();
        let goal = Vec3::new(80.0, 0.0, 80.0);
        let stop = Vec3::new(79.4, 0.0, 80.0);
        let mut arriver = ranger_at(7002, stop);
        arriver.movement.path = vec![Vec3::new(70.0, 0.0, 80.0), goal];
        arriver.movement.current_path_index = 1;
        arriver.movement.target_position = Some(goal);
        arriver.set_status_moving(true);
        arriver.set_ai_state(AIState::Moving);
        let arriver_id = ObjectId(7002);
        logic.objects.insert(arriver_id, arriver);
        logic.update_movement_for_test(&[arriver_id], 1.0 / 30.0);
        let obj = logic.objects.get(&arriver_id).expect("arriver");
        let pos = obj.get_position();
        assert!(
            (pos.x - stop.x).abs() < 1.0e-3 && (pos.z - stop.z).abs() < 1.0e-3,
            "setFinalPosition must not teleport the unit, pos={pos:?}"
        );
        assert!(!obj.do_final_position, "setFinalPosition stores the point and leaves the slide off");
        assert_eq!(obj.final_position, goal);
    }
    #[test]
    fn leftover_goal_none_settles_final_position() {
        let mut logic = GameLogic::new();
        let id = ObjectId(7003);
        let mut unit = ranger_at(7003, Vec3::ZERO);
        unit.do_final_position = true;
        unit.final_position = Vec3::new(20.0, 0.0, 0.0);
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        let step = 2.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL / 30.0;
        assert!(
            obj.do_final_position,
            "far final position must keep leftover-settling"
        );
        assert!(
            (obj.get_position().x - step).abs() < 1.0e-3,
            "settle steps 2 cells/s, x={} expected {step}",
            obj.get_position().x
        );

        let mut logic = GameLogic::new();
        let mut unit = ranger_at(7004, Vec3::new(20.0, 0.0, 0.0));
        unit.do_final_position = true;
        unit.final_position = Vec3::new(20.1, 0.0, 0.0);
        logic.objects.insert(ObjectId(7004), unit);
        logic.update_movement_for_test(&[ObjectId(7004)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(7004)).expect("close");
        assert!(
            !obj.do_final_position,
            "dSqr < 0.25 snaps and clears do_final_position"
        );
        assert!(
            (obj.get_position().x - 20.1).abs() < 1.0e-4,
            "close settle snaps to leftover final_position, x={}",
            obj.get_position().x
        );
    }

    /// hq-99njb: blocked locoUpdate scrubs 2D motive when already at/above cap.
    #[test]
    fn blocked_loco_update_scrubs_instead_of_marching() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9710);
        let mut unit = ranger_at(9710, Vec3::ZERO);
        unit.set_orientation(0.0);
        unit.movement.max_speed = 40.0;
        unit.movement.acceleration = 10_000.0;
        unit.movement.velocity = Vec3::new(4.0, 0.0, 0.0);
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 0.0));
        unit.is_blocked = true;
        unit.cur_max_blocked_speed = 4.0;
        unit.bump_speed_limit = 4.0;
        unit.no_slow_down_as_approaching_dest = true;
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.get_position().x < 0.15,
            "blocked unit at/above cap must not apply 2D motive, pos={}",
            obj.get_position().x
        );
        assert!(
            obj.movement.velocity.x <= 4.0 + 1e-3,
            "scrubVelocity2D must cap, vel={}",
            obj.movement.velocity.x
        );
    }

    /// hq-g9idj: invalid terrain runs 3×3 fixInvalidPosition shove.
    #[test]
    fn fix_invalid_position_3x3_shoves_off_water() {
        let mut logic = GameLogic::new();
        let water = logic
            .pathfinding_system
            .grid
            .world_to_grid(Vec3::new(50.0, 0.0, 50.0));
        logic.pathfinding_system.grid.set_cell_type(
            water,
            gamelogic::ai::pathfind_astar::PathfindCellType::Water,
        );
        logic.pathfinding_system.grid.set_cell_type(
            GridPos::new(water.x - 1, water.y),
            gamelogic::ai::pathfind_astar::PathfindCellType::Water,
        );
        let id = ObjectId(9711);
        let mut unit = ranger_at(9711, Vec3::new(50.0, 0.0, 50.0));
        unit.locomotor_surfaces = gamelogic::ai::pathfind_complete::SURFACE_GROUND;
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 50.0));
        unit.movement.velocity = Vec3::ZERO;
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.motive_frames_remaining > 0,
            "3x3 shove must applyMotiveForce"
        );
        assert!(
            obj.physics_accel.x != 0.0 || obj.physics_accel.z != 0.0,
            "correction must be non-zero, accel={:?}",
            obj.physics_accel
        );
        assert!(
            (obj.get_position().x - 50.0).abs() < 0.01,
            "locoUpdate returns without 2D march, pos={:?}",
            obj.get_position()
        );
    }

    /// hq-i4tcw: ALLOW_INVALID_POSITION skips fixInvalidPosition 3x3 shove.
    #[test]
    fn allow_invalid_position_skips_3x3_shove() {
        let mut logic = GameLogic::new();
        let water = logic
            .pathfinding_system
            .grid
            .world_to_grid(Vec3::new(50.0, 0.0, 50.0));
        logic.pathfinding_system.grid.set_cell_type(
            water,
            gamelogic::ai::pathfind_astar::PathfindCellType::Water,
        );
        logic.pathfinding_system.grid.set_cell_type(
            GridPos::new(water.x - 1, water.y),
            gamelogic::ai::pathfind_astar::PathfindCellType::Water,
        );
        let id = ObjectId(9721);
        let mut unit = ranger_at(9721, Vec3::new(50.0, 0.0, 50.0));
        unit.locomotor_surfaces = gamelogic::ai::pathfind_complete::SURFACE_GROUND;
        unit.movement.target_position = Some(Vec3::new(80.0, 0.0, 50.0));
        unit.movement.velocity = Vec3::ZERO;
        unit.set_allow_invalid_position(true);
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.get_position().x > 50.01 || obj.movement.velocity.x > 0.0,
            "ALLOW_INVALID_POSITION must continue 2D motive, not 3x3-return, pos={:?} vel={:?}",
            obj.get_position(),
            obj.movement.velocity
        );
    }

    /// hq-i4tcw: AIEnterState sets ALLOW_INVALID_POSITION; exit clears it.
    #[test]
    fn enter_state_sets_allow_invalid_position() {
        let mut unit = ranger_at(9722, Vec3::ZERO);
        assert!(!unit.allow_invalid_position);
        unit.set_ai_state(AIState::Entering);
        assert!(
            unit.allow_invalid_position,
            "AIEnterState::onEnter setAllowInvalidPosition(true)"
        );
        unit.set_ai_state(AIState::Idle);
        assert!(
            !unit.allow_invalid_position,
            "AIEnterState::onExit setAllowInvalidPosition(false)"
        );
    }

    /// hq-qdgxx: stamped CloseEnoughDist plants before the 1wu default.
    #[test]
    fn close_enough_dist_plants_before_default_one() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9712);
        let start = Vec3::new(0.0, 0.0, 0.0);
        let goal = Vec3::new(20.0, 0.0, 0.0);
        let mut unit = ranger_at(9712, start);
        unit.close_enough_dist = Some(25.0);
        unit.movement.path = vec![start, goal];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(goal);
        unit.set_status_moving(true);
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.movement.path.is_empty() || !obj.status.moving,
            "SET_STOPPING_DISTANCE 25 must plant at 20wu"
        );
        assert!(
            obj.get_position().x.abs() < 1.0,
            "must plant in place, not walk to 1wu, pos={:?}",
            obj.get_position()
        );
    }

    /// hq-qdgxx: ground sanity refuses to finish if last node is > 4 cells away.
    #[test]
    fn close_enough_ground_sanity_refuses_far_last_node() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9713);
        let start = Vec3::new(0.0, 0.0, 0.0);
        let last = Vec3::new(80.0, 0.0, 0.0);
        let mut unit = ranger_at(9713, start);
        unit.close_enough_dist = Some(25.0);
        unit.movement.path = vec![start, last];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(last);
        unit.movement.max_speed = 1.0;
        unit.set_status_moving(true);
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            !obj.movement.path.is_empty(),
            "4*cell sanity must refuse to plant 80wu out"
        );
        assert!(obj.status.moving, "must keep marching toward last node");
    }

    /// hq-i9ywj: treatAsAirborne is -(3*3)*gravity (~0.64wu), not 9.0.
    #[test]
    fn treat_as_airborne_uses_three_frame_gravity() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9814);
        let mut unit = ranger_at(9814, Vec3::new(0.0, 2.0, 0.0));
        unit.ground_height = 0.0;
        unit.allow_motive_force_while_airborne = false;
        unit.movement.max_speed = 40.0;
        unit.movement.acceleration = 10_000.0;
        unit.movement.velocity = Vec3::ZERO;
        unit.movement.target_position = Some(Vec3::new(80.0, 2.0, 0.0));
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.get_position().x.abs() < 0.15,
            "2wu hop must skip 2D motive (treatAsAirborne -(3*3)*g), pos.x={}",
            obj.get_position().x
        );
    }

    /// hq-7f4ct: NoSlowDown still runs the far-from-goal IS_BRAKING clear.
    #[test]
    fn no_slow_down_does_not_latch_is_braking_after_path_raise() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9815);
        let mut tmpl = ThingTemplate::new("Aurora");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut jet = Object::new(tmpl, id, Team::USA);
        jet.set_position(Vec3::ZERO);
        jet.ground_height = 0.0;
        jet.allow_motive_force_while_airborne = true;
        jet.no_slow_down_as_approaching_dest = true;
        jet.is_braking = true;
        jet.braking_factor = 5.0;
        jet.braking = 10.0;
        jet.movement.max_speed = 30.0;
        jet.movement.acceleration = 10_000.0;
        jet.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
        jet.movement.path = vec![Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0)];
        jet.movement.current_path_index = 1;
        jet.movement.target_position = Some(Vec3::new(200.0, 0.0, 0.0));
        logic.objects.insert(id, jet);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("jet");
        assert!(
            !obj.is_braking,
            "NoSlowDown must un-latch IS_BRAKING when far from dest (Locomotor.cpp:941-946)"
        );
        assert!(
            (obj.braking_factor - 1.0).abs() < 1e-5,
            "far-from-goal clear resets braking_factor, got {}",
            obj.braking_factor
        );
    }

    /// hq-ryf26: lift uses goal Y only when PRECISE_Z_POS.
    /// C++ calcLiftToUseAtPt only allows negative lift in ULTRA_ACCURATE.
    #[test]
    fn lift_ignores_goal_y_without_precise_z_pos() {
        let mut tmpl = ThingTemplate::new("ComancheHill");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, ObjectId(9816), Team::USA);
        heli.set_position(Vec3::new(0.0, 80.0, 0.0));
        heli.ground_height = 40.0;
        heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.loco_preferred_height = 10.0;
        heli.loco_preferred_height_damping = 1.0;
        heli.precise_z_pos = false;
        heli.max_lift = 20.0;
        heli.ultra_accurate = true;
        heli.physics_mass = 1.0;
        heli.physics_accel = Vec3::ZERO;
        let _ = heli.handle_behavior_z(40.0, Some(80.0));
        assert!(
            heli.physics_accel.y < -1.0,
            "without PRECISE_Z_POS lift must seek preferred+surface (50), not hold 80; accel.y={}",
            heli.physics_accel.y
        );

        heli.physics_accel = Vec3::ZERO;
        heli.precise_z_pos = true;
        let _ = heli.handle_behavior_z(40.0, Some(80.0));
        assert!(
            heli.physics_accel.y > -1.0,
            "PRECISE_Z_POS may hold goal_y=80; accel.y={}",
            heli.physics_accel.y
        );
    }

    /// hq-2e10h: PRECISE_Z_POS lift seeks runway/pad goal Y, not cruise PreferredHeight.
    #[test]
    fn landing_lift_tracks_runway_goal_y() {
        let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut jet = Object::new(tmpl, ObjectId(9818), Team::USA);
        jet.set_position(Vec3::new(0.0, 80.0, 0.0));
        jet.ground_height = 0.0;
        jet.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        jet.loco_appearance = LocomotorAppearance::Wings;
        jet.loco_preferred_height = 50.0;
        jet.loco_preferred_height_damping = 1.0;
        jet.max_lift = 20.0;
        jet.physics_mass = 1.0;
        jet.physics_accel = Vec3::ZERO;
        jet.movement.target_position = Some(Vec3::new(0.0, 5.0, 0.0));
        jet.set_precise_z_and_ultra_accurate(true);
        GameLogic::apply_live_handle_behavior_z_for_test(&mut jet, 0.0, None);
        assert!(
            jet.physics_accel.y < -1.0 || jet.get_position().y < 79.0,
            "landing PRECISE_Z_POS must seek runway Y=5 not cruise 50; y={} accel.y={}",
            jet.get_position().y,
            jet.physics_accel.y
        );
    }

    /// hq-hq4t8: treatAsAirborne must not freeze path advance / arrival plant.
    #[test]
    fn treat_as_airborne_still_plants_near_goal() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9817);
        let start = Vec3::new(0.0, 2.0, 0.0);
        let last = Vec3::new(0.2, 2.0, 0.0);
        let mut unit = ranger_at(9817, start);
        unit.ground_height = 0.0;
        unit.allow_motive_force_while_airborne = false;
        unit.close_enough_dist = Some(1.0);
        unit.movement.path = vec![start, last];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(last);
        unit.movement.max_speed = 40.0;
        unit.set_status_moving(true);
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.movement.path.is_empty() || !obj.status.moving,
            "airborne hop must still plant when inside CloseEnoughDist"
        );
        assert!(
            obj.get_position().x.abs() < 6.0,
            "plant must not apply 2D walk motive, x={}",
            obj.get_position().x
        );
    }

    /// hq-hq4t8: IS_BRAKING pose cheat still runs when treatAsAirborne.
    #[test]
    fn treat_as_airborne_still_applies_braking_cheat() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9818);
        let mut unit = ranger_at(9818, Vec3::new(0.0, 2.0, 0.0));
        unit.ground_height = 0.0;
        unit.allow_motive_force_while_airborne = false;
        unit.is_braking = true;
        unit.braking = 10.0;
        unit.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
        unit.movement.max_speed = 30.0;
        unit.movement.acceleration = 10_000.0;
        unit.movement.target_position = Some(Vec3::new(20.0, 2.0, 0.0));
        logic.objects.insert(id, unit);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.get_position().x > 0.2,
            "airborne IS_BRAKING must still cheat toward dest, x={}",
            obj.get_position().x
        );
    }

    /// hq-ygdfb: SurfaceRelative Y is lift+Euler, not preferred+surface snap.
    #[test]
    fn surface_relative_is_lift_not_kinematic_snap() {
        let mut tmpl = ThingTemplate::new("ComancheSnap");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, ObjectId(9819), Team::USA);
        heli.set_position(Vec3::new(0.0, 0.0, 0.0));
        heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.loco_preferred_height = 10.0;
        heli.loco_preferred_height_damping = 1.0;
        heli.max_lift = 5.0;
        heli.physics_mass = 1.0;
        heli.physics_accel = Vec3::ZERO;
        GameLogic::apply_live_handle_behavior_z_for_test(&mut heli, 20.0, None);
        let y = heli.get_position().y;
        assert!(
            (y - 30.0).abs() > 1.0,
            "must not teleport to preferred+surface=30; y={}",
            y
        );
        assert!(
            y > 0.5 && y <= 5.5,
            "one Euler step is lift-limited (maxLift=5), y={}",
            y
        );
    }

    /// hq-0rri4: AbsoluteHeight Y is lift+Euler, not preferred-height snap.
    #[test]
    fn absolute_height_is_lift_not_kinematic_snap() {
        let mut tmpl = ThingTemplate::new("ComancheAbs");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, ObjectId(9821), Team::USA);
        heli.set_position(Vec3::new(0.0, 0.0, 0.0));
        heli.loco_behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
        heli.loco_appearance = LocomotorAppearance::Wings;
        heli.loco_preferred_height = 10.0;
        heli.loco_preferred_height_damping = 1.0;
        heli.max_lift = 5.0;
        heli.physics_mass = 1.0;
        heli.physics_accel = Vec3::ZERO;
        GameLogic::apply_live_handle_behavior_z_for_test(&mut heli, 20.0, None);
        let y = heli.get_position().y;
        assert!(
            (y - 10.0).abs() > 1.0,
            "must not teleport to preferredHeight=10; y={}",
            y
        );
        assert!(
            y > 0.5 && y <= 5.5,
            "one Euler step is lift-limited (maxLift=5), y={}",
            y
        );
    }

    /// hq-g8oig: leftover lift is desiredAccel - gravity; Y Euler must add
    /// leftover gravity so hover/wings hold preferred height instead of climb.
    #[test]
    fn hover_at_preferred_height_does_not_climb() {
        let mut tmpl = ThingTemplate::new("ComancheHold");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, ObjectId(9822), Team::USA);
        heli.set_position(Vec3::new(0.0, 30.0, 0.0));
        heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.loco_preferred_height = 10.0;
        heli.loco_preferred_height_damping = 1.0;
        heli.max_lift = 5.0;
        heli.physics_mass = 1.0;
        heli.physics_accel = Vec3::ZERO;
        heli.movement.velocity = Vec3::ZERO;
        GameLogic::apply_live_handle_behavior_z_for_test(&mut heli, 20.0, None);
        let y = heli.get_position().y;
        assert!(
            (y - 30.0).abs() < 0.02,
            "hover at preferred must hold, not climb by leftover lift; y={}",
            y
        );
        assert!(
            heli.movement.velocity.y.abs() < 0.02,
            "hover hold net accel is 0; vel.y={}",
            heli.movement.velocity.y
        );
    }

    /// hq-si460: leftover Other/Hover slide keeps yaw when ULTRA_ACCURATE
    /// and inside parse_duration_real(SlideIntoPlaceTime) * per-frame speed.
    #[test]
    fn hover_ultra_accurate_slides_without_yaw() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9820);
        let mut tmpl = ThingTemplate::new("ChinookSlide");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, id, Team::USA);
        heli.set_position(Vec3::new(0.0, 0.0, 0.0));
        heli.set_orientation(0.0);
        heli.ground_height = 0.0;
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.allow_motive_force_while_airborne = true;
        heli.ultra_accurate = true;
        heli.ultra_accurate_slide_factor = 3.0;
        heli.movement.max_speed = 150.0;
        heli.movement.acceleration = 10_000.0;
        // Per-frame speed 5 * leftover 3 frames = 15 wu window. Goal at +10 Z
        // is inside the box; facing +X so yaw would change without slide.
        heli.movement.target_position = Some(Vec3::new(0.0, 0.0, 10.0));
        logic.objects.insert(id, heli);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("heli");
        assert!(
            obj.get_orientation().abs() < 0.01,
            "Hover ULTRA_ACCURATE must slide (TURN_NONE), yaw={}",
            obj.get_orientation()
        );
        assert!(
            obj.get_position().z > 0.2,
            "slide must still translate toward goal, z={}",
            obj.get_position().z
        );

        // Braking would drop goal speed to minSpeed and shrink the window to 0.
        // C++ already decided the slide from the pre-brake desired speed.
        let mut braking = Object::new(
            {
                let mut t = ThingTemplate::new("ChinookBrakeSlide");
                t.add_kind_of(KindOf::Aircraft);
                t
            },
            ObjectId(9822),
            Team::USA,
        );
        braking.set_position(Vec3::ZERO);
        braking.set_orientation(0.0);
        braking.ground_height = 0.0;
        braking.loco_appearance = LocomotorAppearance::Hover;
        braking.allow_motive_force_while_airborne = true;
        braking.ultra_accurate = true;
        braking.ultra_accurate_slide_factor = 3.0;
        braking.movement.max_speed = 150.0;
        braking.movement.acceleration = 10_000.0;
        braking.min_speed = 0.0;
        braking.braking = 5.0;
        braking.movement.velocity = Vec3::new(40.0, 0.0, 0.0);
        // 150/30*3 = 15. Goal at 12 is inside that box and outside a zeroed brake window.
        braking.movement.target_position = Some(Vec3::new(0.0, 0.0, 12.0));
        logic.objects.insert(ObjectId(9822), braking);
        logic.update_movement_for_test(&[ObjectId(9822)], 1.0 / 30.0);
        let obj = logic.objects.get(&ObjectId(9822)).expect("braking heli");
        assert!(
            obj.get_orientation().abs() < 0.01,
            "pre-brake slide window must hold while slowing, yaw={}",
            obj.get_orientation()
        );

        let mut wheels = Object::new(
            {
                let mut t = ThingTemplate::new("TruckNoSlide");
                t.add_kind_of(KindOf::Vehicle);
                t
            },
            ObjectId(9821),
            Team::USA,
        );
        wheels.set_position(Vec3::new(0.0, 0.0, 0.0));
        wheels.set_orientation(0.0);
        wheels.ground_height = 0.0;
        wheels.loco_appearance = LocomotorAppearance::WheelsFour;
        wheels.ultra_accurate = true;
        wheels.ultra_accurate_slide_factor = 3.0;
        wheels.movement.max_speed = 150.0;
        wheels.movement.acceleration = 10_000.0;
        wheels.movement.turn_rate = 10.0;
        wheels.min_turn_speed = 1.0;
        wheels.movement.velocity = Vec3::new(20.0, 0.0, 0.0);
        wheels.movement.target_position = Some(Vec3::new(0.0, 0.0, 10.0));
        let mut logic2 = GameLogic::new();
        logic2.objects.insert(ObjectId(9821), wheels);
        logic2.update_movement_for_test(&[ObjectId(9821)], 1.0 / 30.0);
        let truck = logic2.objects.get(&ObjectId(9821)).expect("truck");
        assert!(
            truck.get_orientation().abs() > 0.01,
            "Wheels must still yaw; leftover slide is Other/Hover only, yaw={}",
            truck.get_orientation()
        );
    }

    /// hq-zx7lx: production march must dispatch Thrust to the 3D mover
    /// (orient-to-velocity), not the generic yaw-at-goal hover-car path.
    #[test]
    fn thrust_march_orients_to_velocity_not_goal() {
        let mut logic = GameLogic::new();
        let id = ObjectId(9830);
        let mut tmpl = ThingTemplate::new("ComancheThrust");
        tmpl.add_kind_of(KindOf::Aircraft);
        let mut heli = Object::new(tmpl, id, Team::USA);
        heli.set_position(Vec3::new(0.0, 10.0, 0.0));
        // Nose already along current velocity (−Z). Generic march would yaw
        // toward the +X goal; thrust keeps the nose on the velocity vector.
        heli.set_orientation(std::f32::consts::FRAC_PI_2);
        heli.ground_height = 0.0;
        heli.loco_appearance = LocomotorAppearance::Thrust;
        heli.allow_motive_force_while_airborne = true;
        heli.min_speed = 5.0;
        heli.movement.max_speed = 50.0;
        heli.movement.acceleration = 100.0;
        heli.movement.turn_rate = 10.0;
        heli.max_thrust_angle = std::f32::consts::FRAC_PI_2;
        heli.movement.velocity = Vec3::new(0.0, 0.0, -20.0);
        heli.movement.target_position = Some(Vec3::new(100.0, 10.0, 0.0));
        logic.objects.insert(id, heli);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("heli");
        let yaw = obj.get_orientation();
        assert!(
            (yaw - std::f32::consts::FRAC_PI_2).abs() < 0.35,
            "Thrust must orient to velocity (−Z), not snap yaw to +X goal, yaw={yaw}"
        );
        assert!(
            obj.get_position().distance(Vec3::new(0.0, 10.0, 0.0)) > 1e-3
                || obj.movement.velocity.length() > 1.0,
            "Thrust march must apply 3D motive"
        );
    }

    /// hq-7soel: SMALL_TURN must not recap goalSpeed after IS_BRAKING overwrite.
    #[test]
    fn wheeled_small_turn_does_not_recap_after_approach_brake() {
        let mut logic = GameLogic::new();
        let id = ObjectId(98231);
        let mut truck = {
            let mut tmpl = ThingTemplate::new("HumveeSmallTurnBrake");
            tmpl.add_kind_of(KindOf::Vehicle);
            Object::new(tmpl, id, Team::USA)
        };
        truck.set_position(Vec3::ZERO);
        truck.set_orientation(0.0);
        truck.loco_appearance = LocomotorAppearance::WheelsFour;
        truck.min_turn_speed = 0.0;
        truck.movement.max_speed = 40.0;
        truck.movement.acceleration = 10_000.0;
        truck.movement.turn_rate = std::f32::consts::PI;
        truck.movement.velocity = Vec3::new(30.0, 0.0, 0.0);
        truck.braking = 5.0;
        truck.is_braking = true;
        truck.donut_timer = u32::MAX;
        // ~12° heading error: SMALL_TURN (9°) applies, 15° look-ahead does not.
        truck.movement.target_position = Some(Vec3::new(50.0, 0.0, -10.63));
        truck.set_status_moving(true);
        logic.objects.insert(id, truck);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("truck");
        let speed = obj.movement.velocity.length();
        let turn_floor = 10.0; // maxSpeed/4
        assert!(
            obj.is_braking,
            "close-in wheeled approach must keep IS_BRAKING"
        );
        assert!(
            speed > turn_floor + 5.0,
            "braking goalSpeed (actual-braking≈25) must not recap to turnSpeed=10, speed={speed}"
        );
    }

    /// hq-xlays: idle Wings lift off terrain, not own altitude.
    #[test]
    fn wings_idle_maintain_z_uses_terrain_not_own_altitude() {
        let mut logic = GameLogic::new();
        let id = ObjectId(98232);
        let mut jet = {
            let mut tmpl = ThingTemplate::new("AmericaJetRaptorIdleZ");
            tmpl.add_kind_of(KindOf::Aircraft);
            Object::new(tmpl, id, Team::USA)
        };
        jet.set_position(Vec3::new(0.0, 50.0, 0.0));
        jet.ground_height = 0.0;
        jet.loco_appearance = LocomotorAppearance::Wings;
        jet.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        jet.loco_preferred_height = 20.0;
        jet.loco_preferred_height_damping = 1.0;
        jet.max_lift = 20.0;
        jet.physics_mass = 1.0;
        jet.min_speed = 10.0;
        jet.circling_radius = 40.0;
        jet.motive_frames_remaining = 4;
        jet.movement.max_speed = 40.0;
        // Idle: no target. C++ maintain then handleBehaviorZ(terrain).
        logic.objects.insert(id, jet);
        for _ in 0..8 {
            logic.update_movement_for_test(&[id], 1.0 / 30.0);
        }
        let obj = logic.objects.get(&id).expect("jet");
        let y = obj.get_position().y;
        assert!(
            y < 50.0,
            "idle Wings must descend toward PreferredHeight+terrain=20, not climb via own_y, y={y}"
        );
        assert!(y > 5.0, "must not slam to ground; y={y}");
    }

    /// hq-jg55x: FACE leftover handleBehaviorZ is leftover-terrain, not pose-Y.
    /// Hover at preferred must not climb via preferredHeight+currentY.
    #[test]
    fn face_angle_does_not_double_lift_off_own_altitude() {
        let mut logic = GameLogic::new();
        let id = ObjectId(98240);
        let mut heli = {
            let mut tmpl = ThingTemplate::new("ComancheFaceZ");
            tmpl.add_kind_of(KindOf::Aircraft);
            Object::new(tmpl, id, Team::USA)
        };
        heli.set_position(Vec3::new(0.0, 30.0, 0.0));
        heli.ground_height = 20.0;
        heli.loco_behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        heli.loco_appearance = LocomotorAppearance::Hover;
        heli.loco_preferred_height = 10.0;
        heli.loco_preferred_height_damping = 1.0;
        heli.max_lift = 5.0;
        heli.physics_mass = 1.0;
        heli.physics_accel = Vec3::ZERO;
        heli.movement.velocity = Vec3::ZERO;
        heli.min_speed = 0.0;
        heli.locomotor_goal_type = LocoGoalType::Angle;
        heli.locomotor_goal_angle = std::f32::consts::FRAC_PI_2;
        heli.face_loco_frame = 0;
        logic.objects.insert(id, heli);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("heli");
        let y = obj.get_position().y;
        assert!(
            (y - 30.0).abs() < 0.5,
            "FACE leftover Z must hold preferred+terrain, not climb via pose-Y; y={y}"
        );
    }

    /// hq-v9inf / hq-ij10w: leftover CloseEnoughDist3D keep-Z + 3D remaining.
    #[test]
    fn close_enough_dist_3d_does_not_plant_while_high() {
        let mut logic = GameLogic::new();
        let id = ObjectId(98241);
        let mut dive = {
            let mut tmpl = ThingTemplate::new("ScudDiveMarch");
            tmpl.add_kind_of(KindOf::Projectile);
            Object::new(tmpl, id, Team::USA)
        };
        dive.set_position(Vec3::new(0.0, 40.0, 0.0));
        dive.ground_height = 0.0;
        dive.close_enough_dist_3d = true;
        dive.close_enough_dist = Some(2.0);
        dive.loco_behavior_z = LocomotorBehaviorZ::NoZMotiveForce;
        dive.loco_appearance = LocomotorAppearance::Thrust;
        dive.movement.max_speed = 0.0;
        dive.movement.velocity = Vec3::ZERO;
        dive.movement.path = vec![Vec3::new(0.0, 40.0, 0.0), Vec3::new(1.0, 0.0, 0.0)];
        dive.movement.current_path_index = 1;
        dive.movement.target_position = Some(Vec3::new(1.0, 0.0, 0.0));
        logic.objects.insert(id, dive);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("dive");
        assert!(
            obj.movement.target_position.is_some(),
            "CloseEnoughDist3D leftover unused must not plant on 2D when 3D > arrive"
        );
        assert!(
            obj.host_locomotor_distance_to_goal(obj.get_position(), Vec3::new(1.0, 0.0, 0.0)) > 9.0,
            "3D remaining must stay large while high"
        );
    }

    /// C++ AIUpdate.cpp:1731-1748: patchPath failing while blocked-and-stuck
    /// must concede (destroyPath, snap final position, locomotor goal none,
    /// queue-for-path 1s, reset blocked flags) instead of ordering another
    /// A* every frame; AIStates.cpp:2143-2148 arms the canPathThroughUnits
    /// tunnel for the jam.
    #[test]
    fn stuck_unit_with_failing_patch_concedes_and_backs_off() {
        let mut logic = GameLogic::new();
        let id = ObjectId(98301);
        let mut unit = ranger_at(98301, Vec3::new(5.0, 0.0, 5.0));
        unit.movement.path = vec![
            Vec3::new(5.0, 0.0, 5.0),
            Vec3::new(35.0, 0.0, 5.0),
            Vec3::new(95.0, 0.0, 5.0),
        ];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(Vec3::new(35.0, 0.0, 5.0));
        unit.is_blocked = true;
        unit.is_blocked_and_stuck = true;
        unit.num_frames_blocked = 90;
        logic.objects.insert(id, unit);
        // Seal the goal node's column: patchPath's reverse walk stops at the
        // first blocked suffix node and returns None before any A* splice.
        let start_cell = logic.pathfinding_system.grid.world_to_grid(Vec3::new(5.0, 0.0, 5.0));
        seal_column(&mut logic, start_cell.x + 9);

        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.movement.path.is_empty(),
            "concede must destroy the path instead of hammering A*"
        );
        assert_eq!(
            obj.queue_for_path_frames, 30,
            "concede queues for path 1s (LOGICFRAMES_PER_SECOND)"
        );
        assert!(obj.movement.target_position.is_none());
        assert_eq!(obj.locomotor_goal_type, LocoGoalType::None);
        assert!(!obj.is_blocked && !obj.is_blocked_and_stuck);
        assert_eq!(obj.num_frames_blocked, 0);
        assert!(!obj.do_final_position, "concede setFinalPosition leaves the slide off");
        assert!(
            obj.final_position.x.is_finite() && obj.final_position.z.is_finite(),
            "final position must be a snapped cell"
        );
        assert!(
            obj.can_path_through_units,
            "blocked-and-stuck jam must arm the can_path_through_units tunnel"
        );

        // No repath before the deadline: reinstall a path (as the AI state
        // machine would), keep the deadline, reopen the wall — the stuck
        // branch must stay gated.
        {
            let obj = logic.objects.get_mut(&id).unwrap();
            obj.movement.path = vec![
                Vec3::new(5.0, 0.0, 5.0),
                Vec3::new(35.0, 0.0, 5.0),
                Vec3::new(95.0, 0.0, 5.0),
            ];
            obj.movement.current_path_index = 1;
            obj.movement.target_position = Some(Vec3::new(35.0, 0.0, 5.0));
            obj.is_blocked_and_stuck = true;
        }
        for y in -8..80 {
            logic
                .pathfinding_system
                .grid
                .set_blocked(GridPos::new(start_cell.x + 9, y), false);
        }
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            obj.is_blocked_and_stuck,
            "queue deadline must gate the stuck repath (no A* before it)"
        );
        assert_eq!(obj.movement.path.len(), 3, "path must be untouched");

        // After the deadline expires, a stuck unit's successful patch
        // installs and grants 2s of ignore-collision (AIUpdate.cpp:486-495).
        logic.frame = 80;
        logic.objects.get_mut(&id).unwrap().queue_for_path_frames = 0;
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(
            !obj.is_blocked_and_stuck,
            "successful patch must install and reset the stuck flag"
        );
        assert!(obj.movement.path.len() >= 2, "patched path must install");
        assert_eq!(obj.path_timestamp, 80);
        assert_eq!(
            obj.ignore_collisions_until_frame, 0,
            "computePath does not grant ignore-collision"
        );
    }

    /// num_frames_blocked > 60 without the hard jam concedes but does not
    /// arm the tunnel (C++ concede only; AIMoveOutOfTheWayState arms it).
    #[test]
    fn blocked_not_jammed_unit_concedes_without_tunnel() {
        let mut logic = GameLogic::new();
        let id = ObjectId(98302);
        let mut unit = ranger_at(98302, Vec3::new(5.0, 0.0, 5.0));
        unit.movement.path = vec![
            Vec3::new(5.0, 0.0, 5.0),
            Vec3::new(35.0, 0.0, 5.0),
            Vec3::new(95.0, 0.0, 5.0),
        ];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(Vec3::new(35.0, 0.0, 5.0));
        unit.num_frames_blocked = 90;
        logic.objects.insert(id, unit);
        // Goal-node column seal: patchPath's reverse walk hits the blocked
        // suffix node and returns None before any A* splice.
        let start_cell = logic.pathfinding_system.grid.world_to_grid(Vec3::new(5.0, 0.0, 5.0));
        seal_column(&mut logic, start_cell.x + 9);

        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("unit");
        assert!(obj.movement.path.is_empty(), "slow-blocked unit still concedes");
        assert_eq!(obj.queue_for_path_frames, 30);
        assert!(
            !obj.can_path_through_units,
            "tunnel arms only for blocked-and-stuck jams"
        );
    }
}
