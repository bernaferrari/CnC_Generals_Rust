//! Host pathfinding, movement, and line-of-sight state.
#![allow(unused_imports, non_snake_case)]
use super::*;

impl GameLogic {
    /// C++ ActiveBody::setIndestructible + TerrainLogic.cpp:181 tower inherit.
    pub fn set_object_indestructible(&mut self, id: ObjectId, indestructible: bool) {
        let is_bridge = if let Some(obj) = self.objects.get_mut(&id) {
            obj.set_indestructible(indestructible);
            obj.is_kind_of(crate::game_logic::KindOf::Bridge)
                || crate::game_logic::host_bridge_behavior::is_bridge_span_template(
                    &obj.template_name,
                )
        } else {
            return;
        };
        if is_bridge {
            self.mirror_indestructible_to_bridge_towers(id, indestructible);
        }
    }

    /// C++ ActiveBody.cpp:1355-1380 KINDOF_BRIDGE mirrors to tower bodies.
    pub fn mirror_indestructible_to_bridge_towers(
        &mut self,
        bridge_id: ObjectId,
        indestructible: bool,
    ) {
        let mut tower_ids = [0u32; 4];
        if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() {
            terrain.for_each_bridge(|bridge| {
                if bridge.get_bridge_info().bridge_object_id == bridge_id.0 {
                    tower_ids = bridge.get_bridge_info().tower_object_id;
                }
            });
        }
        for tid in tower_ids {
            if tid == 0 {
                continue;
            }
            if let Some(tower) = self.objects.get_mut(&ObjectId(tid)) {
                tower.set_indestructible(indestructible);
            }
        }
    }

    /// C++ AIFollowWaypointPathExact residual — use waypoints as-is (no A* smoothing).
    pub fn assign_unit_path_exact(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> bool {
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            if unit.is_deployed() {
                unit.set_deployed(false);
            }
        }
        let can_move = match self.objects.get(&unit_id) {
            Some(unit) => unit.is_alive() && unit.can_move(),
            None => return false,
        };
        if !can_move {
            return false;
        }
        let mut full_path: Vec<Vec3> = Vec::with_capacity(waypoints.len() + 1);
        for wp in waypoints {
            if !wp.x.is_finite() || !wp.z.is_finite() {
                continue;
            }
            if let Some(last) = full_path.last() {
                let dx = last.x - wp.x;
                let dz = last.z - wp.z;
                if dx * dx + dz * dz < 0.01 {
                    continue;
                }
            }
            full_path.push(*wp);
        }
        if let Some(last) = full_path.last() {
            let dx = last.x - destination.x;
            let dz = last.z - destination.z;
            if dx * dx + dz * dz >= 0.01 {
                full_path.push(destination);
            }
        } else {
            full_path.push(destination);
        }
        if full_path.is_empty() {
            return false;
        }
        let started = if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.waiting_for_path = false;
            unit.movement.current_path_index = 0;
            unit.movement.path = full_path;
            unit.movement.target_position = unit.movement.path.first().copied();
            unit.is_exact_path = true;
            unit.start_move();
            unit.set_ai_state(AIState::Moving);
            true
        } else {
            false
        };
        if started {
            self.start_move_sound(unit_id);
        }
        started
    }

    pub fn assign_unit_path(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> bool {
        self.assign_unit_path_ignoring(unit_id, destination, waypoints, None)
    }

    /// C++ `ignoreObstacle(goalObject)` then `aiMoveToPosition` (DozerAIUpdate.cpp:210-211).
    pub fn assign_unit_path_ignoring(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
        ignore_obstacle: Option<ObjectId>,
    ) -> bool {
        self.pathfinding_system.set_ignore_obstacle(ignore_obstacle);
        let ok = self.assign_unit_path_inner(unit_id, destination, waypoints, false);
        self.pathfinding_system.set_ignore_obstacle(None);
        ok
    }

    #[cfg(test)]
    pub fn force_map_loaded_for_path_test(&mut self, loaded: bool) {
        self.map_loaded = loaded;
    }

    pub(in super::super) fn assign_unit_path_inner(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
        compute_now: bool,
    ) -> bool {
        // C++ DeployStyle: move order packs unit before pathing residual.
        // TurretsMustCenterBeforePacking stays ALIGNING (still DEPLOYED) until
        // the turret is natural; only UNDEPLOY clears OBJECT_STATUS_DEPLOYED.
        let mut started_undeploy = false;
        let mut block_path = false;
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            if unit.deploy_style.is_some() {
                if !unit
                    .deploy_style
                    .as_ref()
                    .is_some_and(|ds| ds.is_ready_to_move())
                {
                    let has_turret = unit.turret_enabled || unit.turret_turn_rate_rad > 0.0;
                    let turret_natural = crate::game_logic::host_deploy_style::leftover_host_turret_is_in_natural_position(
                        unit.status.under_construction,
                        unit.turret_angle_deg,
                        unit.turret_pitch_deg,
                        unit.turret_natural_angle_deg,
                        unit.turret_natural_pitch_deg,
                    );
                    let outcome = unit.deploy_style.as_mut().map(|ds| {
                        let started = ds.begin_undeploy_with_weapon_turret(
                            self.frame,
                            has_turret,
                            turret_natural,
                        );
                        (started, ds.is_aligning_turrets())
                    });
                    if let Some((started, now_aligning)) = outcome {
                        if started && now_aligning {
                            unit.turret_substate =
                                crate::game_logic::object::TurretSubState::Recenter;
                            unit.turret_idle_recentering = true;
                            unit.turret_target_id = None;
                            unit.turret_holding = false;
                            unit.record_host_turret();
                        } else if started && !now_aligning {
                            started_undeploy = true;
                            unit.set_deployed(false);
                        } else if !now_aligning
                            && !unit
                                .deploy_style
                                .as_ref()
                                .is_some_and(|d| d.is_ready_to_move())
                        {
                            unit.set_deployed(false);
                        }
                    }
                    unit.stop_moving();
                    block_path = true;
                }
            } else if unit.is_deployed() {
                unit.set_deployed(false);
            }
            unit.clear_pending_waypoint_labels();
        }
        if started_undeploy {
            self.deploy_style_reg.record_undeploy();
            self.queue_resolved_per_unit_sound(
                unit_id,
                crate::game_logic::host_deploy_style::DEPLOY_STYLE_UNDEPLOY_AUDIO,
                true,
                false,
                None,
                150,
            );
        }
        if block_path {
            self.deploy_style_reg.record_blocked_move();
            // Path blocked until pack completes; re-issue move after ReadyToMove.
            return false;
        }
        let (start, can_move, is_aircraft, surfaces, is_crusher) = match self.objects.get(&unit_id)
        {
            Some(unit) => (
                unit.get_position(),
                unit.can_move(),
                unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
                    || unit.object_type == crate::game_logic::ObjectType::Aircraft,
                unit.locomotor_surfaces,
                unit.crusher_level > 0,
            ),
            None => return false,
        };
        if !can_move {
            return false;
        }

        // C++ Pathfinder::queueForPath: loaded maps wait one frame
        // (AI.cpp:332-339, AIPathfind.h:418). Mapless / test compute now.
        let defer = self.map_loaded && !compute_now;
        if defer {
            let queued = self
                .pathfinding_system
                .queue_path(super::pathfinding::PendingHostPath {
                    unit_id,
                    start,
                    destination,
                    waypoints: waypoints.to_vec(),
                    aircraft: is_aircraft,
                    surfaces,
                    is_crusher,
                    ignore_obstacle: self.pathfinding_system.ignore_obstacle(),
                });
            if !queued {
                // C++ queueForPath full: refuse the newest, keep oldest waiters.
                return false;
            }
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.waiting_for_path = true;
                // C++ queueForPath: sit still until processPathfindQueue installs Path.
                unit.movement.target_position = None;
                unit.movement.velocity = glam::Vec3::ZERO;
                unit.start_move();
                unit.set_ai_state(AIState::Moving);
                unit.set_status_moving(true);
                unit.record_host_movement();
            }
            crate::game_logic::host_move_log::record(
                unit_id,
                Some([destination.x, destination.y, destination.z]),
            );
            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            }
            self.start_move_sound(unit_id);

            return true;
        }

        let Some(full_path) = self.compute_assigned_unit_path(
            unit_id,
            start,
            destination,
            waypoints,
            is_aircraft,
            surfaces,
            is_crusher,
        ) else {
            return false;
        };
        let ok = self.apply_computed_unit_path(unit_id, start, destination, full_path);
        if ok {
            self.start_move_sound(unit_id);
        }
        ok
    }

    pub(in super::super) fn compute_assigned_unit_path(
        &mut self,
        unit_id: ObjectId,
        start: Vec3,
        destination: Vec3,
        waypoints: &[Vec3],
        is_aircraft: bool,
        surfaces: u32,
        is_crusher: bool,
    ) -> Option<Vec<Vec3>> {
        let horiz = |a: Vec3, b: Vec3| {
            let dx = a.x - b.x;
            let dz = a.z - b.z;
            (dx * dx + dz * dz).sqrt()
        };

        let mut goals: Vec<Vec3> = waypoints.to_vec();
        goals.push(destination);

        let mut full_path: Vec<Vec3> = Vec::new();
        let mut segment_start = start;
        let loco = if is_aircraft {
            gamelogic::ai::pathfind_complete::SURFACE_AIR
        } else if surfaces != 0 {
            surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        let request_is_final = match self.objects.get(&unit_id) {
            Some(u) => {
                !u.is_safe_path
                    && u.attack_substate != crate::game_logic::AttackSubState::ChaseTarget
            }
            None => true,
        };
        let ignore = self.pathfinding_system.ignore_obstacle();
        let goal_count = goals.len();
        for (hop_i, goal) in goals.into_iter().enumerate() {
            if horiz(segment_start, goal) < 0.1 {
                // C++ Path always terminates at its goal: an aircraft hop
                // that only changes altitude (dest XY equals start XY, e.g.
                // groupEvacuate descending to terrain height) still lands as
                // a real final waypoint. Dropping it left the path empty, so
                // arrival-gated states (AI_MOVE_AND_EVACUATE) saw a refused
                // move instead of the descent node.
                if is_aircraft && hop_i + 1 == goal_count && full_path.is_empty() {
                    let mut start_at_dest = segment_start;
                    start_at_dest.y = goal.y;
                    full_path.push(start_at_dest);
                }
                segment_start = goal;
                continue;
            }

            // C++ computePath leftover-install: dest-off+start-off or
            // !isFinalGoal && isLinePassable → computeQuickPath two-node.
            let hop_is_final = request_is_final && hop_i + 1 == goal_count;
            let leftover_quick = !is_aircraft
                && (self
                    .pathfinding_system
                    .leftover_should_force_direct_path_for_off_map_start(segment_start, goal)
                    || self
                        .pathfinding_system
                        .leftover_should_use_direct_path_for_line_passable_non_final_goal(
                            hop_is_final,
                            segment_start,
                            goal,
                            loco,
                            ignore,
                        ));
            let straight = horiz(segment_start, goal);
            let segment = if leftover_quick {
                Some(
                    super::pathfinding::PathfindingSystem::leftover_compute_quick_path_nodes(
                        segment_start,
                        goal,
                    ),
                )
            } else {
                // Never fail-open through blocked cells: always ask the pathfinder.
                self.pathfinding_system.find_path_ex_surfaces(
                    segment_start,
                    goal,
                    &self.objects,
                    is_aircraft,
                    loco,
                    is_crusher,
                    Some(unit_id),
                )
            };

            match segment {
                Some(mut segment_path) => {
                    // Keep the found path even if it is long — do not walk through walls.
                    let path_len: f32 = segment_path.windows(2).map(|w| horiz(w[0], w[1])).sum();
                    if straight > 1.0 && path_len > straight * 3.5 {
                        log::debug!(
                            "Path detour {:.0} vs straight {:.0} for {:?}",
                            path_len,
                            straight,
                            unit_id
                        );
                    }
                    {
                        if let Some(first) = segment_path.first_mut() {
                            *first = segment_start;
                        }
                        // C++ Path::optimize / adjustDestination keep the
                        // snapped cell as the last node. Restoring the raw
                        // click (hq-7lrve) walked units into buildings.
                        if !full_path.is_empty()
                            && !segment_path.is_empty()
                            && full_path
                                .last()
                                .is_some_and(|prev| horiz(*prev, segment_path[0]) < 0.01)
                        {
                            segment_path.remove(0);
                        }
                        full_path.extend(segment_path);
                    }
                }
                None => {
                    log::debug!(
                        "No path found for unit {:?} from {:?} to {:?}; refuse fail-open march",
                        unit_id,
                        segment_start,
                        goal
                    );
                    // C++ accepts a direct movement request before a map has
                    // installed its terrain/path graph. Preserve the normal
                    // fail-closed path policy for loaded maps, but keep the
                    // mapless host-authority path usable during startup and
                    // command validation.
                    if !self.map_loaded {
                        if full_path.is_empty() {
                            full_path.push(segment_start);
                        }
                        full_path.push(goal);
                    } else {
                        return None;
                    }
                }
            }

            segment_start = goal;
        }

        if full_path.is_empty() {
            // Already at goal (all segments < 0.1) is not a fail-open march.
            return None;
        }
        // C++ Path always terminates at its goal (Path::appendGoal /
        // adjustDestination keeps the snapped cell as the last node). A
        // skipped short hop or a truncated segment must still deliver the
        // requested destination so arrival-gated states (AI_MOVE_AND_EVACUATE,
        // RTB taxi) observe a real final waypoint instead of an empty path.
        let last = full_path
            .last()
            .copied()
            .unwrap_or(segment_start);
        if horiz(last, destination) >= 0.01 {
            full_path.push(destination);
        }
        Some(full_path)
    }

    pub(in super::super) fn apply_computed_unit_path(
        &mut self,
        unit_id: ObjectId,
        start: Vec3,
        destination: Vec3,
        full_path: Vec<Vec3>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        unit.waiting_for_path = false;
        unit.is_exact_path = false;
        unit.movement.path = full_path;
        unit.record_host_movement();
        unit.movement.current_path_index = 0;
        unit.record_host_movement();
        unit.movement.target_position = Some(destination);
        unit.start_move();
        crate::game_logic::host_move_log::record(
            unit_id,
            Some([destination.x, destination.y, destination.z]),
        );
        // Kick toward destination at full speed so large-map marches do not
        // burn seconds on the acceleration ramp (was a combat_no_teleport residual).
        {
            let mut dir = destination - start;
            dir.y = 0.0;
            let dir = dir.normalize_or_zero();
            unit.movement.velocity = dir * unit.movement.max_speed;
            unit.record_host_movement();
        }
        unit.set_ai_state(AIState::Moving);
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            // Moving
        }
        unit.set_status_moving(true);
        true
    }

    /// C++ `AIGroup::friend_computeGroundPath` + per-member slot:
    /// one A* from the nearest member to `destination`, then each unit
    /// follows that spine with last waypoint = its formation/column goal.
    pub fn assign_shared_group_paths(
        &mut self,
        goals: &[(ObjectId, Vec3)],
        destination: Vec3,
    ) -> bool {
        if goals.is_empty() {
            return false;
        }
        let leader = goals
            .iter()
            .filter_map(|(id, _)| {
                self.objects.get(id).map(|o| {
                    let p = o.get_position();
                    let d = (p.x - destination.x).hypot(p.z - destination.z);
                    (
                        *id,
                        p,
                        d,
                        o.locomotor_surfaces,
                        o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                            || o.object_type == crate::game_logic::ObjectType::Aircraft,
                    )
                })
            })
            .min_by(|a, b| a.2.total_cmp(&b.2));
        let Some((leader_id, start, _, surfaces, aircraft)) = leader else {
            return false;
        };
        let is_crusher = self
            .objects
            .get(&leader_id)
            .is_some_and(|o| o.crusher_level > 0);
        let Some(spine) = self.compute_assigned_unit_path(
            leader_id,
            start,
            destination,
            &[],
            aircraft,
            surfaces,
            is_crusher,
        ) else {
            return false;
        };
        let mut any = false;
        for &(unit_id, goal) in goals {
            let Some(unit_start) = self.objects.get(&unit_id).map(|o| o.get_position()) else {
                continue;
            };
            let mut path = spine.clone();
            if let Some(last) = path.last_mut() {
                *last = goal;
            } else {
                path.push(goal);
            }

            if self.apply_computed_unit_path(unit_id, unit_start, goal, path) {
                any = true;
            }
        }
        any
    }

    /// C++ Pathfinder::processPathfindQueue residual (AI.cpp:332-339).
    pub(crate) fn process_pathfind_queue(&mut self) {
        self.pathfinding_system.begin_pathfind_queue_frame();
        while self.pathfinding_system.pathfind_budget_remaining() {
            let Some(req) = self.pathfinding_system.pop_pending_path() else {
                break;
            };
            let (start, can_move, is_aircraft, surfaces, is_crusher) =
                match self.objects.get(&req.unit_id) {
                    Some(unit) if unit.is_alive() => (
                        unit.get_position(),
                        unit.can_move(),
                        req.aircraft
                            || unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
                            || unit.object_type == crate::game_logic::ObjectType::Aircraft,
                        if req.surfaces != 0 {
                            req.surfaces
                        } else {
                            unit.locomotor_surfaces
                        },
                        req.is_crusher || unit.crusher_level > 0,
                    ),
                    _ => continue,
                };
            if !can_move {
                if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                    unit.waiting_for_path = false;
                }
                continue;
            }
            self.pathfinding_system
                .set_ignore_obstacle(req.ignore_obstacle);
            match self.compute_assigned_unit_path(
                req.unit_id,
                start,
                req.destination,
                &req.waypoints,
                is_aircraft,
                surfaces,
                is_crusher,
            ) {
                Some(path) => {
                    let _ =
                        self.apply_computed_unit_path(req.unit_id, start, req.destination, path);
                }
                None => {
                    if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                        unit.waiting_for_path = false;
                    }
                }
            }
            self.pathfinding_system.set_ignore_obstacle(None);
        }
    }

    #[cfg(test)]
    pub fn assign_unit_path_for_test(
        &mut self,
        unit_id: ObjectId,
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> bool {
        self.assign_unit_path_inner(unit_id, destination, waypoints, true)
    }

    /// Pathfind to goal then set AI state. Falls back to set_destination if A* fails.
    /// C++ Pathfinder::isAttackViewBlockedByObstacle residual for host combat.
    /// Units with AttackNeedsLineOfSight cannot fire through static obstacles.
    /// Aircraft / non-LOS kinds always clear. Fail-closed: not full weapon terrain LOS.
    /// C++ `Pathfinder::adjustToPossibleDestination` for a live unit.
    pub fn adjust_to_possible_destination(&self, unit_id: ObjectId, dest: &mut Vec3) -> bool {
        let Some(obj) = self.objects.get(&unit_id) else {
            return false;
        };
        self.pathfinding_system
            .adjust_to_possible_destination_for(obj, dest)
    }

    /// Path toward a firing position with LOS (C++ findAttackPath residual).
    /// Falls back to path-to-target if no in-range LOS cell is found.
    pub fn assign_unit_attack_path(
        &mut self,
        unit_id: ObjectId,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
    ) -> bool {
        let (from, range, can_move, contact, is_crusher) = match self.objects.get(&unit_id) {
            Some(u) => {
                let range = u
                    .weapon
                    .as_ref()
                    .map(|w| w.range)
                    .or_else(|| u.secondary_weapon.as_ref().map(|w| w.range))
                    .unwrap_or(50.0)
                    * u.battle_plan_range_multiplier();
                let wname = u.thing.template.primary_weapon_name.as_deref().or(u
                    .thing
                    .template
                    .secondary_weapon_name
                    .as_deref());
                let contact = wname
                    .map(crate::game_logic::weapon_bootstrap::host_is_contact_weapon_name)
                    .unwrap_or(false)
                    || crate::game_logic::weapon_bootstrap::is_contact_effective_range(range);
                (
                    u.get_position(),
                    range,
                    u.can_move() && u.is_alive(),
                    contact,
                    u.crusher_level > 0,
                )
            }
            None => return false,
        };
        if !can_move {
            return false;
        }
        // Contact residual: path onto the target cell (C++ approach = victim pos).
        // Non-contact: path to in-range firing cell via find_attack_firing_position.
        // Callers should pass approach-adjusted goal for non-contact when known.
        let path_range = if contact { range.max(1.0) } else { range };
        let _ = contact;
        // Snapshot objects for dynamic occupancy during search.
        let mut path = self.pathfinding_system.find_attack_firing_position(
            from,
            target_pos,
            path_range,
            &self.objects,
            is_crusher,
            Some(unit_id),
        );
        // LOS_TERRAIN residual: reject firing cell if terrain occludes eye-line.
        if let Some(ref full_path) = path {
            if let Some(&goal) = full_path.last() {
                let eye_r = self
                    .objects
                    .get(&unit_id)
                    .map(|o| o.selection_radius.max(5.0) * 0.5)
                    .unwrap_or(5.0);
                let eye_to = target_id
                    .and_then(|tid| self.objects.get(&tid))
                    .map(|o| o.selection_radius.max(5.0) * 0.5)
                    .unwrap_or(5.0);
                let a_eye = Vec3::new(goal.x, goal.y + eye_r, goal.z);
                let b_eye = Vec3::new(target_pos.x, target_pos.y + eye_to, target_pos.z);
                if !self.is_clear_line_of_sight_terrain(a_eye, b_eye) {
                    path = None;
                }
            }
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        if let Some(full_path) = path {
            if full_path.len() >= 2 {
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    // Path integrate stays host (movement authority peels separately).
                    unit.movement.path = full_path;
                    unit.record_host_movement();
                    unit.movement.current_path_index = 1;
                    unit.record_host_movement();
                    unit.movement.target_position = Some(unit.movement.path[1]);
                    unit.set_status_moving(true);
                    if !decision_auth {
                        if !matches!(unit.ai_state, AIState::AttackMoving | AIState::Patrolling) {
                            unit.set_ai_state(AIState::Attacking);
                        }
                        unit.set_status_attacking(true);
                        if let Some(tid) = target_id {
                            unit.target = Some(tid);
                        }
                    }
                    crate::game_logic::host_move_log::record(
                        unit_id,
                        Some([target_pos.x, target_pos.y, target_pos.z]),
                    );
                }
                if decision_auth {
                    if let Some(tid) = target_id {
                        crate::game_logic::host_ai_decision_log::record_attack(unit_id, tid);
                    }
                    // Attacking ordinal = 2
                    crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
                }
                return true;
            }
        }
        // C++ doPathfind attack fail: adjustToPossibleDestination + ignoreObstacle(victim).
        let mut dest = target_pos;
        self.adjust_to_possible_destination(unit_id, &mut dest);
        if self.assign_unit_path_ignoring(unit_id, dest, &[], target_id) {
            if decision_auth {
                if let Some(tid) = target_id {
                    crate::game_logic::host_ai_decision_log::record_attack(unit_id, tid);
                }
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
            } else if let Some(unit) = self.objects.get_mut(&unit_id) {
                if !matches!(unit.ai_state, AIState::AttackMoving | AIState::Patrolling) {
                    unit.set_ai_state(AIState::Attacking);
                }
                unit.set_status_attacking(true);
                if let Some(tid) = target_id {
                    unit.target = Some(tid);
                }
            }
            return true;
        }
        false
    }

    #[cfg(test)]
    pub fn assign_unit_attack_path_for_test(
        &mut self,
        unit_id: ObjectId,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
    ) -> bool {
        self.assign_unit_attack_path(unit_id, target_id, target_pos)
    }

    /// C++ TerrainLogic/PartitionManager isClearLineOfSightTerrain residual.
    /// Samples ground height along the XZ segment; blocked when terrain rises above
    /// the eye-line + clearance. Uses `terrain_height_at` / pathfinding height cache.
    /// Fail-closed: returns true (clear) when no height data is available.
    pub fn is_clear_line_of_sight_terrain(&self, from: Vec3, to: Vec3) -> bool {
        let dx = to.x - from.x;
        let dz = to.z - from.z;
        let dist_xz = (dx * dx + dz * dz).sqrt();
        if dist_xz <= 0.001 {
            return true;
        }
        // Eye height residual: geometry top ~ selection_radius*0.5 fallback + 5.
        // Callers should pass elevated from/to; default add small eye fudge here.
        let from_y = from.y;
        let to_y = to.y;
        let step_len = 10.0_f32;
        let steps = (dist_xz / step_len).ceil().clamp(2.0, 512.0) as u32;
        const CLEARANCE: f32 = 5.0;
        let mut any_sample = false;
        for i in 1..steps {
            let tfrac = i as f32 / steps as f32;
            let x = from.x + dx * tfrac;
            let z = from.z + dz * tfrac;
            let expected_y = from_y + (to_y - from_y) * tfrac;
            let Some(ground) = self.terrain_height_at(Vec3::new(x, 0.0, z)) else {
                continue;
            };
            any_sample = true;
            if ground > expected_y + CLEARANCE {
                return false;
            }
        }
        // No height data along segment → fail-open clear (flat/synthetic maps).
        let _ = any_sample;
        true
    }

    pub fn attack_view_blocked(
        &self,
        attacker_id: ObjectId,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
    ) -> bool {
        let Some(attacker) = self.objects.get(&attacker_id) else {
            return false;
        };
        // C++ KINDOF_ATTACK_NEEDS_LINE_OF_SIGHT gate.
        // Host residual: Infantry/Vehicle default-need LOS unless Immobile structure.
        let needs_los = attacker.is_kind_of(KindOf::AttackNeedsLineOfSight)
            || ((attacker.is_kind_of(KindOf::Infantry) || attacker.is_kind_of(KindOf::Vehicle))
                && !attacker.is_kind_of(KindOf::Structure /* immobile residual */)
                && !attacker.is_kind_of(KindOf::Structure)
                && !attacker.is_kind_of(KindOf::Aircraft));
        if !needs_los {
            return false;
        }
        // Flying victim residual: significantly above terrain → not blocked.
        if let Some(tid) = target_id {
            if let Some(t) = self.objects.get(&tid) {
                if t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target {
                    return false;
                }
            }
        }
        let from = attacker.get_position();
        // Tiny range residual (C++ AIStates close-range skip).
        let dx = from.x - target_pos.x;
        let dz = from.z - target_pos.z;
        if (dx * dx + dz * dz).sqrt() < 15.0 {
            return false;
        }
        // LOS_TERRAIN residual (C++ Weapon::isClearGoalFiringLineOfSightTerrain):
        // immobile attackers skip terrain LOS (cannot path around).
        let immobile = attacker.is_kind_of(KindOf::Structure /* immobile residual */)
            || attacker.is_kind_of(KindOf::Structure);
        if !immobile {
            // Eye-line: lift by geometry height residual (selection_radius as proxy).
            let eye_from = from.y + attacker.selection_radius.max(5.0) * 0.5;
            let eye_to = {
                let th = target_id
                    .and_then(|tid| self.objects.get(&tid))
                    .map(|t| t.selection_radius.max(5.0) * 0.5)
                    .unwrap_or(5.0);
                target_pos.y + th
            };
            let from_eye = Vec3::new(from.x, eye_from, from.z);
            let to_eye = Vec3::new(target_pos.x, eye_to, target_pos.z);
            if !self.is_clear_line_of_sight_terrain(from_eye, to_eye) {
                return true;
            }
        }
        // Structure/static obstacle Bresenham residual.
        self.pathfinding_system
            .is_attack_view_blocked(from, target_pos)
    }

    pub(crate) fn path_approach_with_state(
        &mut self,
        object_id: ObjectId,
        goal: Vec3,
        state: AIState,
    ) {
        self.path_approach_with_state_ignoring(object_id, goal, state, None);
    }

    pub(crate) fn path_approach_with_state_ignoring(
        &mut self,
        object_id: ObjectId,
        goal: Vec3,
        state: AIState,
        ignore_obstacle: Option<ObjectId>,
    ) {
        let state = self.mood_adjusted_move_state(object_id, state);
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        let ordinal = crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
        let attack_moving = matches!(state, AIState::AttackMoving);
        if self.assign_unit_path_ignoring(object_id, goal, &[], ignore_obstacle) {
            if decision_auth {
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
            } else if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.set_ai_state(state.clone());
            }
        } else if decision_auth {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.set_destination(goal);
            }
            crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
        } else if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.set_destination(goal);
            obj.set_ai_state(state);
        }
        if attack_moving {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.is_attack_path = true;
                obj.requested_destination = Some(goal);
            }
        }
    }

    #[cfg(test)]
    pub fn path_approach_with_state_for_test(
        &mut self,
        object_id: ObjectId,
        goal: Vec3,
        state: AIState,
    ) {
        self.path_approach_with_state(object_id, goal, state);
    }

    pub fn append_unit_waypoint(&mut self, unit_id: ObjectId, waypoint: Vec3) -> bool {
        let (unit_pos, current_path, can_move) = match self.objects.get(&unit_id) {
            Some(unit) => (
                unit.get_position(),
                unit.movement.path.clone(),
                unit.can_move(),
            ),
            None => return false,
        };
        if !can_move {
            return false;
        }

        let last_goal = current_path.last().copied().unwrap_or(unit_pos);

        let segment = self
            .pathfinding_system
            .find_path(last_goal, waypoint, &self.objects);

        let mut appended = current_path;
        match segment {
            Some(mut segment_path) => {
                if let Some(first) = segment_path.first_mut() {
                    *first = last_goal;
                }
                if !appended.is_empty()
                    && !segment_path.is_empty()
                    && appended
                        .last()
                        .is_some_and(|prev| prev.distance(segment_path[0]) < 0.01)
                {
                    segment_path.remove(0);
                }
                // C++ Path::appendGoal: the queued waypoint is the real final
                // node. A* ends at the goal CELL center, which would collapse
                // distinct per-unit destinations in the same cell; keep the
                // requested position as the terminal node.
                if segment_path
                    .last()
                    .is_none_or(|last| last.distance(waypoint) >= 0.01)
                {
                    segment_path.push(waypoint);
                }
                appended.extend(segment_path);
            }
            None => {
                log::debug!(
                    "No path found for unit {:?} from {:?} to {:?}; falling back to direct segment",
                    unit_id,
                    last_goal,
                    waypoint
                );
                if appended.is_empty() {
                    appended.push(last_goal);
                }
                appended.push(waypoint);
            }
        }

        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        // C++ privateFollowPathAppend → privateFollowPath:
        // getStateMachine()->clear() exits Attack/Guard so a queued waypoint
        // abandons the latched target. Without this, Moving + leftover target
        // keeps firing / resumes the attack.
        unit.set_guard_position(None);
        unit.set_guard_target(None);
        unit.end_guard_retaliate();
        unit.hunting = false;
        unit.stop_attack();
        unit.is_attack_path = false;
        unit.movement.path = appended;
        unit.movement.target_position = Some(waypoint);
        crate::game_logic::host_move_log::record(
            unit_id,
            Some([waypoint.x, waypoint.y, waypoint.z]),
        );
        unit.set_ai_state(AIState::Moving);
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            // Moving
        }
        unit.set_status_moving(true);
        true
    }

    #[cfg(test)]
    pub fn append_unit_waypoint_for_test(&mut self, unit_id: ObjectId, waypoint: Vec3) -> bool {
        self.append_unit_waypoint(unit_id, waypoint)
    }
}
