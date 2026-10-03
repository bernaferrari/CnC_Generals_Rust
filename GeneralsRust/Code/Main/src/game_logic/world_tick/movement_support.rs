//! Host tick `impl GameLogic` — `movement` locomotor pass and path helpers.
//!
//! LOC-ratchet split from `movement.rs` (pure code motion):
//! `GameLogic::update_movement` keeps its repath preamble and the GameWorld
//! movement-authority early-return there (the shipped integrate text is
//! scanned by `gameworld_shadow/tests/authority_writeback.rs` and the
//! wave469 residual); everything after that gate lives here as
//! `update_movement_locomotor_pass`.
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
        let (start_pos, is_aircraft, quick, surfaces, is_crusher) =
            match self.objects.get(&object_id) {
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
            self.pathfinding_system
                .set_adjust_goal(unit.is_final_goal && unit.adjust_destinations && !projectile);
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
    pub(in super::super) fn repath_airborne_projectiles(&mut self, object_ids: &[ObjectId]) {
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
    pub(in super::super) fn repath_if_move_goal_moved(&mut self, object_ids: &[ObjectId]) {
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
    /// C++ doLocomotor pass for every object: repath bookkeeping, then the
    /// per-unit path follow and pose integrate. Body of `update_movement`
    /// after the GameWorld movement-authority early-return (pure code motion;
    /// the authority gate itself lives in `movement.rs`).
    pub(in super::super) fn update_movement_locomotor_pass(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
    ) {
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
                        if let Some(path) = self
                            .pathfinding_system
                            .find_closest_path(from, goal, surfaces, is_crusher, is_human, 0.0)
                        {
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
                            if let Some(path) = self
                                .pathfinding_system
                                .find_closest_path(from, goal, surfaces, is_crusher, is_human, 0.0)
                            {
                                repaths.push((id, path, false));
                                closest_paths.push(id);
                            } else {
                                drop_paths.push(id);
                            }
                        }
                        None => {
                            concessions.push((id, true, surfaces, is_crusher, obj.selection_radius))
                        }
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
                            if let Some(path) = self
                                .pathfinding_system
                                .find_closest_path(from, goal, surfaces, is_crusher, is_human, 0.0)
                            {
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
            let keep_path = self
                .objects
                .get(&id)
                .is_some_and(|obj| obj.movement.path.is_empty() || obj.move_away_frames > 0);
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
                        let climb = 1u128
                            << crate::game_logic::host_enum_table_residual::climbing_model_bit();
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
                            if obj.host_locomotor_distance_to_goal(current_pos, last) < close_enough
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
                                    let _ = obj.loco_maintain_appearance(dt);
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
                                        let resume = if crate_leg {
                                            obj.requested_destination
                                        } else {
                                            None
                                        };
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
                            let wander_enabled =
                                !matches!(obj.loco_appearance, LocomotorAppearance::Climber)
                                    && (obj.wander_width_factor != 0.0
                                        || matches!(
                                            obj.loco_appearance,
                                            LocomotorAppearance::LegsTwo
                                        ));
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
                                    && Object::height_treats_as_airborne(
                                        current_pos.y - ground_y - deck_drop,
                                    )
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
                                    if turning == crate::game_logic::PhysicsTurningType::TurnNone {
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
                            let airborne =
                                !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(
                                    obj,
                                );
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
                                            let _ = obj.loco_maintain_appearance(dt);
                                        } else {
                                            plant_goal = obj.movement.path.last().copied();
                                            if matches!(obj.ai_state, AIState::AttackMoving) {
                                                obj.movement.path.clear();
                                                obj.movement.current_path_index = 0;
                                                obj.movement.target_position = None;
                                                let crate_leg =
                                                    obj.requested_victim_id.is_some_and(|id| {
                                                        self.host_money_crates.get(id).is_some()
                                                    });
                                                if crate_leg {
                                                    obj.requested_victim_id = None;
                                                }
                                                let resume = if crate_leg {
                                                    obj.requested_destination
                                                } else {
                                                    None
                                                };
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
                                        valid_movement_terrain_at(
                                            grid,
                                            surfaces,
                                            host,
                                            obj.pathfind_layer,
                                        )
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
                                    || obj.object_type == crate::game_logic::ObjectType::Projectile;
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
                                        let _ = obj.loco_maintain_appearance(dt);
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
                                            let crate_leg =
                                                obj.requested_victim_id.is_some_and(|id| {
                                                    self.host_money_crates.get(id).is_some()
                                                });
                                            if crate_leg {
                                                obj.requested_victim_id = None;
                                            }
                                            let resume = if crate_leg {
                                                obj.requested_destination
                                            } else {
                                                None
                                            };
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
                                    obj.movement.current_path_index =
                                        Self::advance_path_index_by_projection(
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
                            // Position goals still drive altitude when XZ is coincident.
                            // C++ moveTowardsPosition invalidates maintainPos and
                            // applies handleBehaviorZ(goal) once after appearance.
                            let position_goal = matches!(
                                obj.locomotor_goal_type,
                                LocoGoalType::PositionOnPath | LocoGoalType::PositionExplicit
                            );
                            if position_goal {
                                obj.maintain_pos_valid = false;
                            }
                            let _ = obj.loco_maintain_appearance(dt);
                            let sy = if matches!(obj.loco_appearance, LocomotorAppearance::Wings) {
                                obj.leftover_surface_ht(surface_y)
                            } else {
                                surface_y
                            };
                            let goal_y = if position_goal {
                                Some(target_pos.y)
                            } else {
                                obj.maintain_pos.map(|p| p.y)
                            };
                            Self::apply_live_handle_behavior_z(obj, sy, goal_y);
                            // C++ friend_endingMove only runs from the move state.
                            // Idle maintain (goal still coincident) must not clear
                            // the queue or the ignored obstacle.
                            if obj.locomotor_goal_type != LocoGoalType::PositionExplicit
                                && (matches!(obj.ai_state, AIState::Moving | AIState::AttackMoving)
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
                                        let resume = if crate_leg {
                                            obj.requested_destination
                                        } else {
                                            None
                                        };
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
                        // C++ maintainCurrentPosition: appearance, then one Z update.
                        let _ = obj.loco_maintain_appearance(dt);
                        let sy = if matches!(obj.loco_appearance, LocomotorAppearance::Wings) {
                            obj.leftover_surface_ht(surface_y)
                        } else {
                            surface_y
                        };
                        Self::apply_live_handle_behavior_z(obj, sy, obj.maintain_pos.map(|p| p.y));
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
        let new_cell = self
            .pathfinding_system
            .grid
            .update_ground_goal_cell(uid, player, radius, false, old, last);
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
            uid,
            player,
            selection_radius,
            immobile,
            stored,
            final_pos,
        );
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.pathfind_goal_cell = new_cell;
        }
    }

    /// C++ `applyMotiveForce(0)` at locoUpdate_moveTowardsPosition entry.
    /// Host collide/friction need the motive window even when GW owns pose.
    pub(in super::super) fn arm_march_motive_flags(&mut self, object_ids: &[ObjectId]) {
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
    pub(in super::super) fn stamp_airborne_targets_from_locomotor(
        &mut self,
        object_ids: &[ObjectId],
    ) {
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
                obj.model_condition_bits &= !(1u128
                    << crate::game_logic::host_enum_table_residual::MC_BIT_STUNNED_FLAILING);
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
    pub(in super::super) fn drain_pending_transport_exits(&mut self) {
        let mut evac_now: Vec<(ObjectId, bool)> = Vec::new();
        for (id, obj) in &self.objects {
            if !obj.pending_evacuate_on_stop {
                continue;
            }
            // C++ ChinookAIUpdate::isIdle rejects a saved pending command.
            // A takeoff transition may be idle for one callback before replay;
            // generic exit polling must not auto-land it at the old origin.
            if obj
                .chinook_ai
                .as_ref()
                .is_some_and(|ai| ai.pending_evac_dest.is_some())
            {
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
    /// Update AI behavior for all objects
    /// Enhanced with AI decision system for intelligent behavior

    /// Drain global fire-spawn queue into host CombatSystem (fire-spawn authority apply).
    pub(crate) fn drain_pending_projectiles_into_combat(&mut self) {
        crate::game_logic::host_historic_bonus::set_logic_frame(self.frame);
        crate::game_logic::combat::drain_pending_projectiles(
            &mut self.combat_system,
            &self.objects,
            self.frame,
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
            Some(&self.team_factory),
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
        ai.flight_status == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
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
