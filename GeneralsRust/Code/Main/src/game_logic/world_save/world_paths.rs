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
        let last_node = full_path.last().copied();
        let (started, entered_move) = if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.waiting_for_path = false;
            unit.queue_for_path_frames = 0;
            unit.movement.current_path_index = 0;
            unit.movement.path = full_path;
            unit.movement.target_position = unit.movement.path.first().copied();
            unit.is_exact_path = true;
            unit.is_attack_path = false;
            unit.set_locomotor_goal_position_on_path();
            unit.num_frames_blocked = 0;
            unit.is_blocked_and_stuck = false;
            unit.path_timestamp = self.frame;
            unit.path_extra_distance = unit.waypoint_link_extra_distance();
            unit.start_move();
            let entered_move = unit.ai_state != AIState::Moving;
            if entered_move {
                unit.set_ai_state(AIState::Moving);
            }
            (true, entered_move)
        } else {
            (false, false)
        };
        if entered_move && crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
        }
        if started {
            let stamp = self
                .objects
                .get(&unit_id)
                .is_some_and(|unit| unit.is_final_goal);
            if stamp {
                if let Some(last) = last_node {
                    self.register_ground_path_goal(unit_id, last);
                }
            }
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
        let restore_adjust = self.pathfinding_system.tighten_restore_adjust;
        self.pathfinding_system.tighten_restore_adjust = false;
        if self.try_install_flying_quick_path(unit_id, destination) {
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.retry_path = false;
            }
            self.settle_try_one_more_repath(unit_id);
            return true;
        }
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
                    // Already Undeploying: begin_undeploy returns false and
                    // does not touch ready_frame. stop_moving only keeps the
                    // empty-path locomotor off while the pack is still running.
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
        
        let delayed = if waypoints.is_empty() {
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                let keep_final = unit.is_final_goal;
                let delayed = !unit.begin_request_move_path(destination, self.frame);
                unit.is_final_goal = keep_final;
                delayed
            } else {
                return false;
            }
        } else {
            false
        };
        if delayed {
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.is_blocked = false;
                unit.num_frames_blocked = 0;
                unit.is_blocked_and_stuck = false;
                unit.try_one_more_repath = true;
                unit.set_status_moving(true);
                unit.start_move();
            }
            self.note_successful_move_path(unit_id);
            self.start_move_sound(unit_id);
            return true;
        }
        let quick_installed = self.objects.get(&unit_id).is_some_and(|unit| {
            unit.path_timestamp == self.frame
                && !unit.waiting_for_path
                && !unit.movement.path.is_empty()
        });
        if quick_installed {
            self.note_successful_move_path(unit_id);
            self.settle_try_one_more_repath(unit_id);
            self.start_move_sound(unit_id);
            return true;
        }
        let (start, can_move, quick_aircraft, surfaces, is_crusher) = match self.objects.get(&unit_id)
        {
            Some(unit) => {
                let surfaces = unit.locomotor_surfaces;
                let quick = (surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) != 0
                    && !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit);
                (
                    unit.get_position(),
                    unit.can_move(),
                    quick
                        && (unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
                            || unit.object_type == crate::game_logic::ObjectType::Aircraft)
                        && !unit.is_kind_of(crate::game_logic::KindOf::Projectile),
                    surfaces,
                    unit.crusher_level > 0,
                )
            }
            None => return false,
        };
        if !can_move {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.try_one_more_repath = true;
            unit.requested_destination = Some(destination);
            unit.set_status_moving(true);
        }

        let defer = self.map_loaded && !compute_now;
        if defer {
            let queued = self
                .pathfinding_system
                .queue_path(super::pathfinding::PendingHostPath {
                    unit_id,
                    start,
                    destination,
                    waypoints: waypoints.to_vec(),
                    aircraft: quick_aircraft,
                    surfaces,
                    is_crusher,
                    ignore_obstacle: self.pathfinding_system.ignore_obstacle(),
                    adjust_destinations: self
                        .objects
                        .get(&unit_id)
                        .is_some_and(|unit| unit.adjust_destinations),
                    restore_adjust_on_install: restore_adjust,
                });
            if !queued {
                return false;
            }
            let mut entered_move = false;
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.waiting_for_path = true;
                unit.movement.target_position = None;
                unit.movement.velocity = glam::Vec3::ZERO;
                unit.start_move();
                entered_move = !matches!(
                    unit.ai_state,
                    AIState::Constructing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::Attacking
                        | AIState::AttackMoving
                        | AIState::Capturing
                        | AIState::Repairing
                        | AIState::SpecialAbility
                        | AIState::GuardingObject
                        | AIState::GuardingArea
                        | AIState::Entering
                        | AIState::Moving
                );
                if entered_move {
                    unit.set_ai_state(AIState::Moving);
                }
                unit.set_status_moving(true);
                unit.record_host_movement();
            }
            crate::game_logic::host_move_log::record(
                unit_id,
                Some([destination.x, destination.y, destination.z]),
            );
            if entered_move && crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
            }
            self.start_move_sound(unit_id);
            return true;
        }


        let Some((full_path, ground_search)) = self.compute_assigned_unit_path(
            unit_id,
            start,
            destination,
            waypoints,
            quick_aircraft,
            surfaces,
            is_crusher,
            None,
        ) else {
            if Self::route_is_already_there(start, destination, waypoints) {
                self.clear_compute_path_blocked(unit_id);
                return false;
            }
            if self.try_closest_path_when_none(unit_id, start, destination, surfaces, is_crusher)
            {
                self.note_successful_move_path(unit_id);
                self.start_move_sound(unit_id);
                self.settle_try_one_more_repath(unit_id);
                return true;
            }
            let closest_ran = self
                .objects
                .get(&unit_id)
                .is_some_and(|unit| unit.movement.path.is_empty());
            self.note_compute_path_failed(unit_id);
            if closest_ran {
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.retry_path = true;
                }
            }
            return false;
        };
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.retry_path = false;
        }
        let ok = self.apply_computed_unit_path(
            unit_id,
            start,
            destination,
            full_path,
            ground_search,
        );
        if ok {
            self.note_successful_move_path(unit_id);
            self.start_move_sound(unit_id);
            self.settle_try_one_more_repath(unit_id);
        }
        ok
    }

    /// C++ `canComputeQuickPath` + non-aircraft `computeQuickPath`.
    pub(in super::super) fn try_install_flying_quick_path(&mut self, unit_id: ObjectId, destination: Vec3) -> bool {
        let Some(unit) = self.objects.get(&unit_id) else {
            return false;
        };
        let air_surface = (unit.locomotor_surfaces
            & crate::game_logic::object::LOCO_SURFACE_AIR)
            != 0;
        if !air_surface || crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit) {
            return false;
        }
        if let Some(last) = unit.movement.path.last() {
            let d = *last - destination;
            if d.length_squared() < 0.25 {
                drop(unit);
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.path_goal_position = Some(destination);
                    unit.waiting_for_path = false;
                    unit.queue_for_path_frames = 0;
                    unit.set_status_moving(true);
                }
                return true;
            }
        }
        if unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
            && !unit.is_kind_of(crate::game_logic::KindOf::Projectile)
        {
            let start = unit.get_position();
            let surfaces = unit.locomotor_surfaces;
            let crusher = unit.crusher_level > 0;
            let saved_adjust = self.pathfinding_system.adjusts_goal();
            self.pathfinding_system.set_adjust_goal(false);
            let path = self
                .pathfinding_system
                .find_path_ex_surfaces(
                    start,
                    destination,
                    &self.objects,
                    true,
                    surfaces,
                    crusher,
                    Some(unit_id),
                )
                .unwrap_or_else(|| {
                    let mut lifted = start;
                    lifted.y = destination.y;
                    vec![lifted, destination]
                });
            self.pathfinding_system.set_adjust_goal(saved_adjust);
            let Some(unit) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            unit.can_path_through_units = false;
            unit.is_attack_path = false;
            unit.is_exact_path = false;
            unit.set_locomotor_goal_position_on_path();
            unit.movement.path = path;
            if unit.movement.path.len() >= 2 {
                unit.movement.current_path_index = 1;
                unit.movement.target_position = Some(unit.movement.path[1]);
            } else {
                unit.movement.current_path_index = 0;
                unit.movement.target_position = Some(destination);
            }
            unit.refresh_follow_path_extra_distance();
            unit.path_goal_position = Some(destination);
            unit.path_timestamp = self.frame;
            unit.waiting_for_path = false;
            unit.queue_for_path_frames = 0;
            unit.num_frames_blocked = 0;
            unit.is_blocked_and_stuck = false;
            unit.set_status_moving(true);
            return true;
        }
        let mut start = unit.get_position();
        start.y = destination.y;
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        unit.can_path_through_units = false;
        unit.is_attack_path = false;
        unit.is_exact_path = false;
        unit.set_locomotor_goal_position_on_path();
        unit.movement.path = vec![start, destination];
        unit.movement.current_path_index = 1;
        unit.movement.target_position = Some(destination);
        unit.refresh_follow_path_extra_distance();
        unit.path_goal_position = Some(destination);
        unit.path_timestamp = self.frame;
        unit.waiting_for_path = false;
        unit.queue_for_path_frames = 0;
        unit.num_frames_blocked = 0;
        unit.is_blocked_and_stuck = false;
        unit.set_status_moving(true);
        true
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
        adjust_snapshot: Option<bool>,
    ) -> Option<(Vec<Vec3>, bool)> {
        let horiz = |a: Vec3, b: Vec3| {
            let dx = a.x - b.x;
            let dz = a.z - b.z;
            (dx * dx + dz * dz).sqrt()
        };

        if !self
            .objects
            .get(&unit_id)
            .is_some_and(|unit| unit.is_blocked_and_stuck)
        {
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.movement.path.clear();
                unit.movement.current_path_index = 0;
                unit.movement.target_position = None;
                unit.is_attack_path = false;
                unit.set_locomotor_goal_none();
                unit.waiting_for_path = false;
            }
        }

        let mut goals: Vec<Vec3> = waypoints.to_vec();
        goals.push(destination);

        let mut full_path: Vec<Vec3> = Vec::new();
        let mut ground_search = false;
        let mut segment_start = start;
        let loco = if surfaces != 0 {
            surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        let request_is_final = self
            .objects
            .get(&unit_id)
            .is_some_and(|u| u.is_final_goal);
        let ignore = self.pathfinding_system.ignore_obstacle();
        let saved_adjust = self.pathfinding_system.adjusts_goal();
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
            let flying = is_aircraft
                && (loco & gamelogic::ai::pathfind_complete::SURFACE_AIR) != 0;
            let hop_is_final = request_is_final && hop_i + 1 == goal_count;
            let leftover_quick = !flying
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
            let (live_adjust, projectile) = self
                .objects
                .get(&unit_id)
                .map(|unit| {
                    (
                        unit.adjust_destinations,
                        unit.is_kind_of(crate::game_logic::KindOf::Projectile),
                    )
                })
                .unwrap_or((false, false));
            let adjusts = adjust_snapshot.unwrap_or(live_adjust);
            self.pathfinding_system
                .set_adjust_goal(hop_is_final && adjusts && !projectile);
            let stuck_path = if flying {
                None
            } else {
                self.objects.get(&unit_id).and_then(|unit| {
                    if unit.is_blocked_and_stuck && unit.movement.path.len() >= 2 {
                        Some(unit.movement.path.clone())
                    } else {
                        None
                    }
                })
            };
            let dest_cell = self.pathfinding_system.grid.world_to_grid(goal);
            let dest_layer = self.pathfinding_system.grid.layer_for_destination(goal);
            let ignore_id = self
                .pathfinding_system
                .ignore_obstacle()
                .map(|id| id.0)
                .unwrap_or(0);
            
            let dest_ok = flying
                || self.pathfinding_system.grid.valid_movement_position(
                    dest_cell,
                    dest_layer,
                    loco,
                    is_crusher,
                    ignore_id,
                );
            let segment = if leftover_quick {
                Some(
                    super::pathfinding::PathfindingSystem::leftover_compute_quick_path_nodes(
                        segment_start,
                        goal,
                    ),
                )
            } else if !dest_ok {
                None
            } else if let Some(original) = stuck_path {
                self.pathfinding_system.patch_path(
                    segment_start,
                    &original,
                    loco,
                    is_crusher,
                    &self.objects,
                    Some(unit_id),
                )
            } else {
                self.pathfinding_system.find_path_ex_surfaces(
                    segment_start,
                    goal,
                    &self.objects,
                    flying,
                    loco,
                    is_crusher,
                    Some(unit_id),
                )
            };

            match segment.filter(|path| !path.is_empty()) {
                Some(mut segment_path) => {
                    ground_search |= !leftover_quick && !flying;
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
                        self.pathfinding_system.set_adjust_goal(saved_adjust);
                        return None;
                    }
                }
            }

            segment_start = goal;
        }

        if full_path.is_empty() {
            // Already at goal (all segments < 0.1) is not a fail-open march.
            self.pathfinding_system.set_adjust_goal(saved_adjust);
            return None;
        }
        // Snapped A* cell stays last so apply can updateLastNode when
        // ultra-accurate. Append the click only for a skipped or quick hop
        // (evacuate / RTB), which never ran findPath.
        if !ground_search {
            let last = full_path.last().copied().unwrap_or(segment_start);
            if horiz(last, destination) >= 0.01 {
                full_path.push(destination);
            }
        }
        self.pathfinding_system.set_adjust_goal(saved_adjust);
        Some((full_path, ground_search))
    }

    pub(in super::super) fn apply_computed_unit_path(
        &mut self,
        unit_id: ObjectId,
        _start: Vec3,
        destination: Vec3,
        full_path: Vec<Vec3>,
        after_ground_search: bool,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return false;
        };
        let no_collide = unit.is_kind_of(crate::game_logic::KindOf::NoCollide);
        let mut full_path = full_path;
        let has_locomotor = unit
            .cur_locomotor_name
            .as_ref()
            .is_some_and(|name| !name.is_empty())
            || unit
                .locomotor_set_names
                .iter()
                .any(|name| !name.is_empty());
        if after_ground_search && unit.ultra_accurate && has_locomotor {
            if let Some(last) = full_path.last_mut() {
                *last = destination;
            }
        }
        unit.waiting_for_path = false;
        unit.queue_for_path_frames = 0;
        if unit.is_safe_path {
            unit.adjust_destinations = false;
        }
        unit.is_exact_path = false;
        // path[0] is the current cell (segment_start). Match the other
        // installer: skip it and aim at the first corner, or the final
        // destination when the path is a single node.
        if full_path.len() >= 2 {
            unit.movement.current_path_index = 1;
            unit.movement.target_position = Some(full_path[1]);
        } else {
            unit.movement.current_path_index = 0;
            unit.movement.target_position = Some(destination);
        }
        unit.movement.path = full_path;
        unit.is_attack_path = false;
        unit.num_frames_blocked = 0;
        unit.is_blocked_and_stuck = false;
        unit.set_locomotor_goal_position_on_path();
        unit.path_timestamp = self.frame;
        unit.refresh_follow_path_extra_distance();
        unit.record_host_movement();
        unit.start_move();
        crate::game_logic::host_move_log::record(
            unit_id,
            Some([destination.x, destination.y, destination.z]),
        );
        // C++ locoUpdate accelerates from the current velocity toward the
        // path lead. Do not stamp max speed at the raw click: a detour
        // would spend the first frames driving into the obstacle.
        let entered_move = !matches!(
            unit.ai_state,
            AIState::Constructing
                | AIState::Gathering
                | AIState::ReturningResources
                | AIState::Attacking
                | AIState::AttackMoving
                | AIState::Capturing
                | AIState::Repairing
                | AIState::SpecialAbility
                | AIState::GuardingObject
                | AIState::GuardingArea
                | AIState::Entering
                | AIState::Moving
        );
        if entered_move {
            unit.set_ai_state(AIState::Moving);
        }
        if entered_move && crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
        }
        unit.set_status_moving(true);
        unit.path_goal_position = Some(destination);
        let last_node = unit.movement.path.last().copied();
        let stamp = unit.is_final_goal;
        drop(unit);
        if stamp {
            if let Some(last) = last_node {
                self.register_ground_path_goal(unit_id, last);
            }
        }
        if after_ground_search && !no_collide {
            self.scoot_allies_off_mover_path(unit_id);
        }
        return true;
    }

    /// C++ `AIGroup::friend_computeGroundPath` + per-member slot:
    /// one A* from the nearest member to `destination`, then each unit
    /// follows that spine with last waypoint = its formation/column goal.
    /// C++ `friend_moveInfantryToPos` / `friend_moveVehicleToPos` per-node
    /// column residual (AIGroup.cpp:897-1008): members march the spine in
    /// laterally offset lanes (lane from the packed column index, corner
    /// normals, alternating ±half-cell rank stagger, `farEnoughSqr` node
    /// thinning) instead of one single-file line.
    pub fn assign_shared_group_paths(
        &mut self,
        goals: &[(ObjectId, Vec3)],
        destination: Vec3,
    ) -> bool {
        if goals.is_empty() {
            return false;
        }
        for &(id, _) in goals {
            let Some(unit) = self.objects.get(&id) else {
                continue;
            };
            let old = unit.pathfind_goal_cell;
            if old.0 < 0 || old.1 < 0 {
                continue;
            }
            let radius = unit.selection_radius;
            let uid = unit.id.0;
            let player = unit.owner_player_id.unwrap_or(unit.team as u32);
            self.pathfinding_system
                .grid
                .clear_ground_goal_square(uid, player, radius, old);
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.pathfind_goal_cell = (-1, -1);
            }
        }
        let mut cx = 0.0f32;
        let mut cz = 0.0f32;
        let mut count = 0.0f32;
        for &(id, _) in goals {
            if let Some(o) = self.objects.get(&id) {
                if o.status.disabled_held || !(o.is_mobile() || o.can_attack()) {
                    continue;
                }
                let p = o.get_position();
                cx += p.x;
                cz += p.z;
                count += 1.0;
            }
        }
        if count <= 0.0 {
            return false;
        }
        cx /= count;
        cz /= count;
        let leader = goals
            .iter()
            .filter_map(|(id, _)| {
                self.objects.get(id).and_then(|o| {
                    let aircraft = o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                        || o.object_type == crate::game_logic::ObjectType::Aircraft;
                    let infantry = o.is_kind_of(crate::game_logic::KindOf::Infantry);
                    let vehicle = o.is_kind_of(crate::game_logic::KindOf::Vehicle);
                    if o.status.disabled_held || !(o.is_mobile() || o.can_attack()) {
                        return None;
                    }
                    if !(infantry || (vehicle && !aircraft)) {
                        return None;
                    }
                    let p = o.get_position();
                    let d = (p.x - cx).hypot(p.z - cz);
                    Some((
                        *id,
                        p,
                        d,
                        o.locomotor_surfaces,
                        aircraft,
                    ))
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
        if let Some(leader) = self.objects.get_mut(&leader_id) {
            let landed_chinook = leader.chinook_ai.as_ref().is_some_and(|ai| {
                ai.flight_status
                    == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
            });
            leader.is_final_goal = !leader.is_parachuting()
                && !landed_chinook
                && !(leader.chinook_ai.is_some() && leader.allow_invalid_position)
                && leader.adjust_destinations;
            leader.num_frames_blocked = 0;
            leader.is_blocked_and_stuck = false;
            leader.set_status_moving(true);
        }
        let Some((spine, mut pass_scoot)) = self.compute_assigned_unit_path(
            leader_id,
            start,
            destination,
            &[],
            aircraft,
            surfaces,
            is_crusher,
            None,
        ) else {
            return false;
        };
        // C++ group march direction = spine start → destination; the lateral
        // lanes live on its normal (AIGroup.cpp:708-716 startVectorNormal).
        let mut dir_vec = Vec3::new(destination.x - start.x, 0.0, destination.z - start.z);
        let dir_len = dir_vec.length();
        let dir = if dir_len > 1e-3 {
            dir_vec /= dir_len;
            dir_vec
        } else {
            Vec3::ZERO
        };
        let lanes = if dir == Vec3::ZERO {
            std::collections::HashMap::new()
        } else {
            Self::group_march_lanes(&self.objects, goals, destination, (dir.x, dir.z))
        };
        let mob_column_blocked = goals.iter().any(|(id, _)| {
            self.objects
                .get(id)
                .is_some_and(|o| o.is_kind_of(crate::game_logic::KindOf::MobNexus))
        });
        let mut any = false;
        for &(unit_id, goal) in goals {
            if mob_column_blocked
                && self
                    .objects
                    .get(&unit_id)
                    .is_some_and(|o| o.is_kind_of(crate::game_logic::KindOf::Infantry))
            {
                continue;
            }
            if self.objects.get(&unit_id).is_some_and(|o| {
                o.status.disabled_held
                    || !(o.is_mobile() || o.can_attack())
                    || o.is_kind_of(crate::game_logic::KindOf::MobNexus)
            }) {
                continue;
            }
            let airborne_aircraft = self.objects.get(&unit_id).is_some_and(|o| {
                let aircraft = o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                    || o.object_type == crate::game_logic::ObjectType::Aircraft;
                o.is_kind_of(crate::game_logic::KindOf::Vehicle)
                    && aircraft
                    && !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(o)
            });
            if airborne_aircraft {
                if self.unit_command_move_free(unit_id, goal, destination) {
                    any = true;
                }
                continue;
            }
            let ground_member = self.objects.get(&unit_id).is_some_and(|o| {
                o.is_kind_of(crate::game_logic::KindOf::Infantry)
                    || o.is_kind_of(crate::game_logic::KindOf::Vehicle)
            });
            if !ground_member {
                continue;
            }
            let Some(unit_start) = self.objects.get(&unit_id).map(|o| o.get_position()) else {
                continue;
            };
            let legacy_spine = || {
                let mut path = spine.clone();
                if let Some(last) = path.last_mut() {
                    *last = goal;
                } else {
                    path.push(goal);
                }
                path
            };
            let path = match lanes.get(&unit_id) {
                Some(&(column_delta, factor, infantry)) => {
                    self.member_column_march_path(
                        &spine,
                        unit_start,
                        goal,
                        column_delta,
                        factor,
                        infantry,
                        (dir.x, dir.z),
                    )
                    .unwrap_or_else(legacy_spine)
                }
                None => legacy_spine(),
            };
            let _ = self.note_move_to_request_path(unit_id);
            if self.apply_computed_unit_path(unit_id, unit_start, goal, path, pass_scoot) {
                any = true;
            }
            pass_scoot = false;
        }
        any
    }

    /// Per-member marching lanes for a shared-spine group march.
    ///
    /// C++ assigns `columnDelta = 1 - curIndex/divisor` after sorting members
    /// on the normal projection FAR_TO_NEAR (AIGroup.cpp:801-803, :1269-1272),
    /// with `divisor = (unitsToPath+1)/numColumns` and lane count from group
    /// size (3 columns infantry / 2 columns vehicles, min group sizes from
    /// AIData.ini). The packed column goals encode the lane as
    /// `destination + n*(delta*width + stagger)`, so descending goal-lateral
    /// order reproduces the pack order and each member recovers the marching
    /// lane its goal was packed into.
    fn group_march_lanes(
        objects: &std::collections::HashMap<ObjectId, Object>,
        goals: &[(ObjectId, Vec3)],
        destination: Vec3,
        dir: (f32, f32),
    ) -> std::collections::HashMap<ObjectId, (i32, i32, bool)> {
        use crate::game_logic::host_ai_path_combat_residual_wave105::{
            MIN_INFANTRY_FOR_GROUP_RESIDUAL, MIN_VEHICLES_FOR_GROUP_RESIDUAL,
        };
        let (nx, nz) = (-dir.1, dir.0);
        let mut infantry: Vec<(ObjectId, f32)> = Vec::new();
        let mut vehicles: Vec<(ObjectId, f32)> = Vec::new();
        for &(id, goal) in goals {
            let Some(o) = objects.get(&id) else {
                continue;
            };
            if o.status.disabled_held || !(o.is_mobile() || o.can_attack()) {
                continue;
            }
            let lateral = (goal.x - destination.x) * nx + (goal.z - destination.z) * nz;
            if o.is_kind_of(crate::game_logic::KindOf::Infantry)
                && !o.is_kind_of(crate::game_logic::KindOf::MobNexus)
            {
                infantry.push((id, lateral));
            } else if o.is_kind_of(crate::game_logic::KindOf::Vehicle)
                && crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(o)
            {
                vehicles.push((id, lateral));
            }
        }
        if goals.iter().any(|(id, _)| {
            objects
                .get(id)
                .is_some_and(|o| o.is_kind_of(crate::game_logic::KindOf::MobNexus))
        }) {
            infantry.clear();
        }
        let mut out = std::collections::HashMap::new();
        for (list, num_columns, min_count) in [
            (&mut infantry, 3_i32, MIN_INFANTRY_FOR_GROUP_RESIDUAL),
            (&mut vehicles, 2_i32, MIN_VEHICLES_FOR_GROUP_RESIDUAL),
        ] {
            let n = list.len() as i32;
            if n < min_count.max(1) {
                // C++ m_minInfantry/m_minVehiclesForGroup gate: too few
                // members of this kind → no column pass for them.
                continue;
            }
            list.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let divisor = ((n + 1) / num_columns).max(1);
            let half = num_columns / 2;
            // Rank stagger counter per lane (C++ columnFactor, :897-907).
            let mut lane_counts = [0_i32; 3];
            for (i, &(id, _)) in list.iter().enumerate() {
                let mut column_delta = 1 - (i as i32 / divisor);
                if num_columns == 2 && column_delta == 0 {
                    // C++ 2-column vehicles keep off the center line
                    // (AIGroup.cpp:1271-1272).
                    column_delta = -1;
                }
                column_delta = column_delta.clamp(-half, half);
                let factor = lane_counts[(column_delta + 1) as usize];
                lane_counts[(column_delta + 1) as usize] += 1;
                out.insert(id, (column_delta, factor, num_columns == 3));
            }
        }
        out
    }

    /// C++ per-node column walk (AIGroup.cpp:897-965): thin the spine at
    /// `farEnoughSqr` = (PATH_DIAMETER_IN_CELLS=6 cells)², offset each kept
    /// node laterally by `columnDelta` lanes along the corner normal
    /// (`PATHFIND_CELL_SIZE_F*2.1/halfNumColumns` infantry, `*1.5` vehicles),
    /// stagger alternate ranks ±half a cell, and keep only nodes that make
    /// forward progress. Returns None when the spine is too short to thin.
    fn member_column_march_path(
        &self,
        spine: &[Vec3],
        unit_start: Vec3,
        goal: Vec3,
        column_delta: i32,
        factor: i32,
        infantry: bool,
        dir: (f32, f32),
    ) -> Option<Vec<Vec3>> {
        const CELL: f32 = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
        let far_enough_sqr = (6.0 * CELL) * (6.0 * CELL);
        let lane_width = if infantry { CELL * 2.1 } else { CELL * 1.5 };
        let dist_sqr = |a: Vec3, b: Vec3| {
            (a.x - b.x) * (a.x - b.x) + (a.z - b.z) * (a.z - b.z)
        };
        if spine.len() < 2 {
            return None;
        }
        let spine_start = *spine.first()?;
        // C++ startNode: first spine node past farEnoughSqr of the start
        // (AIGroup.cpp:673-682).
        let mut node = spine
            .iter()
            .position(|p| dist_sqr(*p, spine_start) > far_enough_sqr)?;
        let (wmin, wmax) = self.world_bounds();
        let clamp_map = |mut p: Vec3| {
            p.x = p.x.clamp(wmin.x + CELL, wmax.x - CELL);
            p.z = p.z.clamp(wmin.z + CELL, wmax.z - CELL);
            p
        };
        let mut prev_idx = 0_usize;
        let mut prev_pos = unit_start;
        let mut path: Vec<Vec3> = Vec::new();
        while node < spine.len() {
            // C++ nextNode: first successor past farEnoughSqr of the current
            // node (:917-924); none → the walk stops (:925).
            let Some(next) = (node + 1..spine.len())
                .find(|&j| dist_sqr(spine[j], spine[node]) > far_enough_sqr)
            else {
                break;
            };
            let corner_x = spine[next].x - spine[prev_idx].x;
            let corner_z = spine[next].z - spine[prev_idx].z;
            let clen = (corner_x * corner_x + corner_z * corner_z).sqrt();
            if clen > 1e-4 {
                // C++ cornerVectorNormal = left normal of the corner vector
                // (:926-929).
                let cn_x = -corner_z / clen;
                let cn_z = corner_x / clen;
                let mut dest = spine[node];
                // Lateral lane offset plus alternating ±half-cell rank
                // stagger (:935-944).
                let lateral = lane_width * column_delta as f32
                    + if factor & 1 == 1 { 0.5 * CELL } else { -0.5 * CELL };
                dest.x += lateral * cn_x;
                dest.z += lateral * cn_z;
                dest = clamp_map(dest);
                // Only keep nodes that make forward progress (:952-955).
                let cx = dest.x - prev_pos.x;
                let cz = dest.z - prev_pos.z;
                if corner_x * cx + corner_z * cz > 0.0 {
                    path.push(dest);
                    prev_pos = dest;
                }
            }
            node += 1; // C++ advances one optimized link (:956).
            // previousNode lags: last node before `node` past farEnoughSqr
            // of it (:958-964).
            if node < spine.len() {
                let mut k = prev_idx + 1;
                while k < node {
                    if dist_sqr(spine[k], spine[node]) > far_enough_sqr {
                        prev_idx = k;
                    }
                    k += 1;
                }
            }
        }
        if path.is_empty() {
            return None;
        }
        // C++ tail trim (:991-1003): drop trailing nodes the goal does not
        // lie ahead of; keep at least one en-route node.
        while path.len() > 1 {
            let last = *path.last().unwrap();
            let gx = goal.x - last.x;
            let gz = goal.z - last.z;
            if dir.0 * gx + dir.1 * gz <= 0.0 {
                path.pop();
            } else {
                break;
            }
        }
        path.push(goal);
        Some(path)
    }

    /// Units that received a move while they could not path retry here.
    /// The stored point is not a locomotor goal. Deployed units are included
    /// even though `can_move` is false: only `assign_unit_path` starts the
    /// pack and clears that bit. A queued path clears the order before A*
    /// runs, so a later wall refusal does not retry every frame.
    pub(crate) fn reissue_pending_moves(&mut self) {
        let ready: Vec<(ObjectId, Vec3, AIState, Option<ObjectId>)> = self
            .objects
            .iter()
            .filter_map(|(id, unit)| {
                if !Self::pending_move_ready(unit) {
                    return None;
                }
                let dest = unit.pending_move?;
                let ignore = match unit.ai_state {
                    AIState::Constructing => unit.dozer_task_build_target.or(unit.target),
                    AIState::Repairing => unit.dozer_task_repair_target.or(unit.target),
                    AIState::Capturing | AIState::Attacking | AIState::AttackMoving => unit.target,
                    AIState::Entering => unit.ignored_obstacle_id.or(unit.target),
                    AIState::SpecialAbility => unit
                        .hacker_disable_channel
                        .as_ref()
                        .map(|channel| channel.target_id)
                        .or(unit.target),
                    _ => None,
                };
                Some((*id, dest, unit.ai_state.clone(), ignore))
            })
            .collect();
        for (id, dest, task, ignore) in ready {
            if matches!(task, AIState::Moving | AIState::AttackMoving | AIState::Idle) {
                if let Some(unit) = self.objects.get_mut(&id) {
                    let landed_chinook = unit.chinook_ai.as_ref().is_some_and(|ai| {
                        ai.flight_status
                            == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
                    });
                    unit.is_final_goal = !unit.is_parachuting()
                        && !landed_chinook
                        && !(unit.chinook_ai.is_some() && unit.allow_invalid_position)
                        && unit.adjust_destinations;
                }
            }
            let ok = if ignore.is_some() {
                self.assign_unit_path_ignoring(id, dest, &[], ignore)
            } else {
                self.assign_unit_path(id, dest, &[])
            };
            if ok {
                let set_state = matches!(
                    task,
                    AIState::Constructing
                        | AIState::Gathering
                        | AIState::ReturningResources
                        | AIState::Repairing
                        | AIState::Capturing
                        | AIState::Attacking
                        | AIState::AttackMoving
                        | AIState::SpecialAbility
                        | AIState::Entering
                );
                if let Some(unit) = self.objects.get_mut(&id) {
                    unit.pending_move = None;
                    if let Some(ignore_id) = ignore {
                        unit.ignored_obstacle_id = Some(ignore_id);
                    }
                }
                if set_state {
                    let already = self.objects.get(&id).is_some_and(|unit| unit.ai_state == task);
                    if !already {
                        self.set_ai_state_decision_aware(id, task);
                    }
                    if let Some(unit) = self.objects.get_mut(&id) {
                        if !unit.movement.path.is_empty() {
                            unit.set_status_moving(true);
                        }
                    }
                }
            }
        }
    }
    fn pending_move_ready(unit: &crate::game_logic::Object) -> bool {
        if unit.pending_move.is_none() || !unit.is_alive() || !unit.is_mobile() || unit.is_disabled()
        {
            return false;
        }
        if unit.shock_stun_frames > 15 {
            return false;
        }
        if !unit.is_parked_at_airfield()
            && matches!(
                unit.ai_state,
                crate::game_logic::AIState::Docked | crate::game_logic::AIState::Garrisoned
            )
        {
            return false;
        }
        true
    }



    /// C++ Pathfinder::processPathfindQueue residual (AI.cpp:332-339).
    pub(crate) fn process_pathfind_queue(&mut self) {
        self.pathfinding_system.begin_pathfind_queue_frame();
        while self.pathfinding_system.pathfind_budget_remaining() {
            let Some(req) = self.pathfinding_system.pop_pending_path() else {
                break;
            };
            let (start, can_move, is_aircraft, surfaces, is_crusher, safe, mut approach, attack, victim) =
                match self.objects.get(&req.unit_id) {
                    Some(unit) if unit.is_alive() => (
                        unit.get_position(),
                        unit.can_move(),
                        req.aircraft,
                        if req.surfaces != 0 {
                            req.surfaces
                        } else {
                            unit.locomotor_surfaces
                        },
                        req.is_crusher || unit.crusher_level > 0,
                        unit.is_safe_path,
                        unit.is_approach_path,
                        unit.is_attack_path,
                        unit.requested_victim_id,
                    ),
                    _ => continue,
                };
            if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                unit.waiting_for_path = false;
                if approach
                    && !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit)
                {
                    unit.is_approach_path = false;
                    approach = false;
                }
            }
            if !can_move {
                continue;
            }
            // C++ queueForPath stores the object id; doPathfind reads
            // getIgnoredObstacleID() live. The queued copy applies only when
            // the unit is already gone.
            let ignore = self
                .objects
                .get(&req.unit_id)
                .map(|unit| unit.ignored_obstacle_id)
                .unwrap_or(req.ignore_obstacle);
            self.pathfinding_system.set_ignore_obstacle(ignore);
            if safe {
                if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                    unit.movement.path.clear();
                    unit.movement.current_path_index = 0;
                    unit.is_attack_path = false;
                    unit.set_locomotor_goal_none();
                    unit.waiting_for_path = false;
                }
                let (p1, p2, radius, is_human) = {
                    let unit = self.objects.get(&req.unit_id);
                    let r1 = unit.and_then(|u| u.requested_victim_id);
                    let r2 = unit.and_then(|u| u.safe_path_repulsor2);
                    let vision = unit.map(|u| u.vision_range).unwrap_or(0.0);
                    let repulsed = gamelogic::ai::the_ai()
                        .read()
                        .ok()
                        .and_then(|ai| {
                            ai.get_ai_data()
                                .read()
                                .ok()
                                .map(|data| data.repulsed_distance)
                        })
                        .unwrap_or(0.0);
                    let radius = vision + repulsed;
                    let missing = glam::Vec3::new(-1000.0, -1000.0, 0.0);
                    let is_human = unit
                        .and_then(|u| u.owner_player_id)
                        .and_then(|pid| self.players.get(&pid))
                        .map(|p| p.is_local)
                        .unwrap_or(true);
                    let p1 = r1
                        .and_then(|i| self.objects.get(&i).map(|o| o.get_position()))
                        .unwrap_or(missing);
                    let p2 = r2
                        .and_then(|i| self.objects.get(&i).map(|o| o.get_position()))
                        .unwrap_or(p1);
                    (p1, p2, radius, is_human)
                };
                if let Some(path) = self.pathfinding_system.find_safe_path_from(
                    start, p1, p2, radius, surfaces, is_crusher, is_human,
                ) {
                    let _ = self.apply_computed_unit_path(req.unit_id, start, req.destination, path, false);
                }
                self.pathfinding_system.set_ignore_obstacle(None);
                continue;
            }
            if approach {
                if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                    unit.movement.path.clear();
                    unit.movement.current_path_index = 0;
                    unit.is_attack_path = false;
                    unit.set_locomotor_goal_none();
                    unit.waiting_for_path = false;
                }
                let is_human = self
                    .objects
                    .get(&req.unit_id)
                    .and_then(|unit| unit.owner_player_id)
                    .and_then(|pid| self.players.get(&pid))
                    .map(|player| player.is_local)
                    .unwrap_or(true);
                if let Some(path) = self.pathfinding_system.find_closest_path(
                    start,
                    req.destination,
                    surfaces,
                    is_crusher,
                    is_human,
                    0.2,
                ) {
                    let last = path.last().copied();
                    let _ = self.apply_computed_unit_path(req.unit_id, start, req.destination, path, false);
                    if let Some(last) = last {
                        if self.objects.get(&req.unit_id).is_some_and(|unit| {
                            crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit)
                        }) {
                            self.register_ground_path_goal(req.unit_id, last);
                        }
                    }
                }
                self.pathfinding_system.set_ignore_obstacle(None);
                continue;
            }
            let mut goal = req.destination;
            if attack {
                if self.assign_unit_attack_path_fallback(
                    req.unit_id,
                    victim,
                    req.destination,
                    false,
                ) {
                    self.pathfinding_system.set_ignore_obstacle(None);
                    continue;
                }
                if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                    unit.is_attack_path = false;
                }
                if let Some(vid) = victim {
                    if let Some(v) = self.objects.get(&vid) {
                        goal = v.get_position();
                    }
                    self.adjust_to_possible_destination(req.unit_id, &mut goal);
                    if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                        unit.ignored_obstacle_id = Some(vid);
                        unit.requested_destination = Some(goal);
                    }
                    self.pathfinding_system.set_ignore_obstacle(Some(vid));
                }
            }
            if self.try_install_flying_quick_path(req.unit_id, goal) {
                if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                    unit.retry_path = false;
                }
                self.settle_try_one_more_repath(req.unit_id);
                self.pathfinding_system.set_ignore_obstacle(None);
                continue;
            }
            match self.compute_assigned_unit_path(
                req.unit_id,
                start,
                goal,
                &req.waypoints,
                is_aircraft,
                surfaces,
                is_crusher,
                Some(req.adjust_destinations),
            ) {
                Some((path, ground_search)) => {
                    if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                        unit.retry_path = false;
                    }
                    let installed = self.apply_computed_unit_path(
                        req.unit_id,
                        start,
                        goal,
                        path,
                        ground_search,
                    );
                    if installed {
                        if req.restore_adjust_on_install {
                            if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                                unit.adjust_destinations = true;
                            }
                        }
                        if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                            unit.num_frames_blocked = 0;
                            unit.is_blocked_and_stuck = false;
                        }
                        self.on_waited_path_arrived(req.unit_id);
                    } else {
                        self.on_waited_path_failed(req.unit_id);
                    }
                }
                None => {
                    if Self::route_is_already_there(start, goal, &req.waypoints) {
                        self.clear_compute_path_blocked(req.unit_id);
                        self.on_waited_path_failed(req.unit_id);
                    } else if self.try_closest_path_when_none(
                        req.unit_id,
                        start,
                        goal,
                        surfaces,
                        is_crusher,
                    ) {
                        if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                            unit.num_frames_blocked = 0;
                            unit.is_blocked_and_stuck = false;
                        }
                        self.on_waited_path_arrived(req.unit_id);
                    } else {
                        let closest_ran = self
                            .objects
                            .get(&req.unit_id)
                            .is_some_and(|unit| unit.movement.path.is_empty());
                        self.note_compute_path_failed(req.unit_id);
                        if closest_ran {
                            if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                                unit.retry_path = true;
                            }
                        }
                        if self
                            .objects
                            .get(&req.unit_id)
                            .is_some_and(|unit| unit.queue_for_path_frames > 0)
                        {
                            if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                                unit.waiting_for_path = true;
                            }
                        } else {
                            self.on_waited_path_failed(req.unit_id);
                        }
                    }
                }
            }
            self.pathfinding_system.set_ignore_obstacle(None);
        }
    }

    /// C++ AIUpdate when `now >= m_queueForPathFrame`: queueForPath and clear the timer.
    pub(crate) fn requeue_expired_path_requests(&mut self, ids: &[ObjectId]) {
        for &id in ids {
            let Some(unit) = self.objects.get(&id) else {
                continue;
            };
            let Some(dest) = unit.requested_destination else {
                continue;
            };
            if !unit.waiting_for_path {
                continue;
            }
            let surfaces = unit.locomotor_surfaces;
            let quick = (surfaces & crate::game_logic::object::LOCO_SURFACE_AIR) != 0
                && !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit);
            let aircraft = quick
                && (unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
                    || unit.object_type == crate::game_logic::ObjectType::Aircraft)
                && !unit.is_kind_of(crate::game_logic::KindOf::Projectile);
            let req = crate::game_logic::pathfinding::PendingHostPath {
                unit_id: id,
                start: unit.get_position(),
                destination: dest,
                waypoints: Vec::new(),
                aircraft,
                surfaces,
                is_crusher: unit.crusher_level > 0,
                ignore_obstacle: unit.ignored_obstacle_id,
                adjust_destinations: unit.adjust_destinations,
                restore_adjust_on_install: false,
            };
            let _ = self.pathfinding_system.queue_path(req);
        }
    }


    fn note_successful_move_path(&mut self, unit_id: ObjectId) {
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return;
        };
        unit.num_frames_blocked = 0;
        unit.is_blocked_and_stuck = false;
        unit.desired_speed = 999_999.0;
    }

    pub(in super::super) fn route_is_already_there(start: Vec3, destination: Vec3, waypoints: &[Vec3]) -> bool {
        let mut prev = start;
        for goal in waypoints.iter().copied().chain(std::iter::once(destination)) {
            let dx = goal.x - prev.x;
            let dz = goal.z - prev.z;
            if (dx * dx + dz * dz).sqrt() >= 0.1 {
                return false;
            }
            prev = goal;
        }
        true
    }

    /// `findClosestPath` once, only when `findPath` returned null and `m_path` is null.
    /// `is_human` is the controlling player. A path that comes back is a ground install,
    /// so allies are still asked to move unless the unit is `NoCollide`.
    pub(in super::super) fn try_closest_path_when_none(
        &mut self,
        unit_id: ObjectId,
        start: Vec3,
        destination: Vec3,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        let owner = {
            let Some(unit) = self.objects.get_mut(&unit_id) else {
                return false;
            };
            if !unit.movement.path.is_empty() {
                return false;
            }
            unit.retry_path = false;
            unit.owner_player_id
        };
        let is_human = match owner {
            Some(pid) => self.player_is_human(pid),
            None => true,
        };
        let loco = if surfaces != 0 {
            surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        let found = self.pathfinding_system.find_closest_path(
            start,
            destination,
            loco,
            is_crusher,
            is_human,
            0.0,
        );
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.retry_path = true;
        }
        let Some(path) = found else {
            return false;
        };
        self.apply_computed_unit_path(unit_id, start, destination, path, true)
    }

    /// Bottom of C++ `computePath`: stamp the clock and clear stuck. No snap.
    fn clear_compute_path_blocked(&mut self, unit_id: ObjectId) {
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return;
        };
        unit.path_timestamp = self.frame;
        unit.num_frames_blocked = 0;
        unit.is_blocked_and_stuck = false;
        unit.retry_path = false;
    }

    /// C++ `computePath` when `findPath` returns null (AIUpdate.cpp:1731-1754).
    pub(in super::super) fn note_compute_path_failed(&mut self, unit_id: ObjectId) {
        let Some(unit) = self.objects.get(&unit_id) else {
            return;
        };
        let stuck_with_path = !unit.movement.path.is_empty() && unit.is_blocked_and_stuck;
        let (pos, center) = if stuck_with_path {
            let (_, center) = crate::game_logic::PathfindingGrid::radius_and_center(
                unit.selection_radius,
                self.pathfinding_system.grid.grid_size(),
            );
            (unit.get_position(), center)
        } else {
            (Vec3::ZERO, true)
        };
        let goal_pos = if stuck_with_path {
            self.pathfinding_system.grid.snap_position(pos, center)
        } else {
            Vec3::ZERO
        };
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return;
        };
        let closest_attempted = !stuck_with_path && unit.movement.path.is_empty();
        if stuck_with_path {
            unit.movement.path.clear();
            unit.movement.current_path_index = 0;
            unit.movement.target_position = None;
            unit.is_attack_path = false;
            unit.final_position = goal_pos;
            unit.do_final_position = false;
            unit.set_locomotor_goal_none();
            unit.queue_for_path_frames =
                crate::game_logic::host_ai_path_combat_residual_wave105::LOGIC_FRAMES_PER_SECOND_RESIDUAL;
            unit.is_blocked = false;
            unit.waiting_for_path = true;
        }
        unit.path_timestamp = self.frame;
        unit.num_frames_blocked = 0;
        unit.is_blocked_and_stuck = false;
        unit.retry_path = closest_attempted;
    }

    pub(crate) fn on_waited_path_failed(&mut self, unit_id: ObjectId) {
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return;
        };
        let overlay = unit.temporary_move_frames > 0
            && !matches!(unit.ai_state, AIState::Moving);
        unit.waiting_for_path = false;
        if overlay {
            unit.end_temporary_move_overlay();
            unit.ignored_obstacle_id = None;
            unit.set_locomotor_goal_none();
        } else if matches!(unit.ai_state, AIState::Moving) {
            unit.stop_moving();
        }
        drop(unit);
        if overlay {
            self.apply_arrival_goal_snap(unit_id, None);
        }
    }


    /// C++ `AIInternalMoveToState::update` when a waited path arrives
    /// (AIStates.cpp:1782-1786). Adjusting units `updateGoal` the last node.
    /// The others `removeGoal`.
    pub(crate) fn on_waited_path_arrived(&mut self, unit_id: ObjectId) {
        let Some(unit) = self.objects.get(&unit_id) else {
            return;
        };
        let goal = unit.requested_destination;
        let landed_chinook = unit.chinook_ai.as_ref().is_some_and(|ai| {
            ai.flight_status
                == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
        });
        let adjusts = !unit.is_parachuting()
            && !landed_chinook
            && !(unit.chinook_ai.is_some() && unit.allow_invalid_position)
            && unit.adjust_destinations;
        let last = unit.movement.path.last().copied();
        if adjusts {
            if let Some(last) = last {
                self.register_ground_path_goal(unit_id, last);
            }
        } else {
            let old = unit.pathfind_goal_cell;
            let radius = unit.selection_radius;
            let id = unit.id.0;
            let player = unit.owner_player_id.unwrap_or(unit.team as u32);
            self.pathfinding_system
                .grid
                .clear_ground_goal_square(id, player, radius, old);
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.pathfind_goal_cell = (-1, -1);
            }
        }
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.path_goal_position = goal.or(unit.path_goal_position);
            unit.waiting_for_path = false;
        }
        self.settle_try_one_more_repath(unit_id);
    }

    fn settle_try_one_more_repath(&mut self, unit_id: ObjectId) {
        let Some(unit) = self.objects.get_mut(&unit_id) else {
            return;
        };
        if !unit.retry_path {
            unit.try_one_more_repath = false;
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
        self.assign_unit_attack_path_fallback(unit_id, target_id, target_pos, true)
    }

    pub(crate) fn assign_unit_attack_path_fallback(
        &mut self,
        unit_id: ObjectId,
        target_id: Option<ObjectId>,
        target_pos: Vec3,
        compute_fallback: bool,
    ) -> bool {
        let (from, range, can_move, contact, is_crusher) = match self.objects.get(&unit_id) {
            Some(u) => {
                let slot = u.selected_weapon_slot();
                let weapon = slot.and_then(|s| u.weapon_slot(s));
                let under = crate::game_logic::weapon_bootstrap::PATHFIND_CELL_SIZE * 0.25;
                let range = weapon
                    .map(|w| (u.effective_weapon_range(w.range) - under).max(0.0))
                    .unwrap_or(50.0);
                let raw_range = weapon.map(|w| w.range).unwrap_or(0.0);
                let wname = slot.and_then(|s| u.weapon_name_for_slot(s));
                let contact = wname
                    .map(crate::game_logic::weapon_bootstrap::host_is_contact_weapon_name)
                    .unwrap_or(false)
                    || crate::game_logic::weapon_bootstrap::is_contact_effective_range(
                        raw_range - under,
                    );
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
        // C++ AIAttackApproachTargetState: contact weapons ignoreObstacle(victim)
        // before requestAttackPath, so the search runs into the target.
        if contact {
            if let Some(tid) = target_id {
                self.pathfinding_system.set_ignore_obstacle(Some(tid));
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.ignored_obstacle_id = Some(tid);
                }
            }
        } else {
            // A leftover contact ignore would open that object for this scan.
            self.pathfinding_system.set_ignore_obstacle(None);
        }
        let owner = self.objects.get(&unit_id).and_then(|unit| unit.owner_player_id);
        let surfaces = self
            .objects
            .get(&unit_id)
            .map(|unit| unit.locomotor_surfaces)
            .unwrap_or(0);
        let is_human = owner
            .and_then(|pid| self.players.get(&pid))
            .map(|player| player.is_local)
            .unwrap_or(true);
        let mut path = if contact {
            self.pathfinding_system.find_closest_path(
                from,
                target_pos,
                surfaces,
                is_crusher,
                is_human,
                0.2,
            )
        } else {
            self.pathfinding_system.find_attack_firing_position(
                from,
                target_pos,
                path_range,
                &self.objects,
                is_crusher,
                Some(unit_id),
            )
        };
        if contact {
            self.pathfinding_system.set_ignore_obstacle(None);
        }
        // LOS_TERRAIN residual: reject firing cell if terrain occludes eye-line.
        // Contact paths go to the victim, not a firing cell.
        if !contact {
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
        }
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        if let Some(mut full_path) = path {
            if full_path.len() >= 2 {
                if contact {
                    let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
                    let three = (cell * 3.0) * (cell * 3.0);
                    let jam = full_path.last().is_some_and(|last| {
                        let dx = last.x - target_pos.x;
                        let dz = last.z - target_pos.z;
                        dx * dx + dz * dz < three
                    });
                    if jam {
                        if let Some(last) = full_path.last_mut() {
                            *last = target_pos;
                        }
                    }
                    let too_short = full_path.last().is_some_and(|last| {
                        let dx = last.x - from.x;
                        let dz = last.z - from.z;
                        dx * dx + dz * dz < cell * cell
                    });
                    if too_short {
                        if let Some(unit) = self.objects.get_mut(&unit_id) {
                            unit.movement.path.clear();
                            unit.movement.target_position = None;
                            unit.movement.current_path_index = 0;
                            unit.set_status_moving(false);
                        }
                        return false;
                    }
                }
                let last_node = full_path.last().copied();
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    unit.movement.path = full_path;
                    unit.record_host_movement();
                    unit.movement.current_path_index = 1;
                    unit.record_host_movement();
                    unit.movement.target_position = Some(unit.movement.path[1]);
                    if contact {
                        unit.is_attack_path = true;
                        unit.path_extra_distance =
                            10.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
                    } else {
                        unit.is_attack_path = false;
                        unit.refresh_follow_path_extra_distance();
                    }
                    unit.set_status_moving(true);
                    if !matches!(unit.ai_state, AIState::AttackMoving | AIState::Patrolling) {
                        unit.set_ai_state(AIState::Attacking);
                        if !unit.movement.path.is_empty() {
                            unit.set_status_moving(true);
                        }
                    }
                    unit.set_status_attacking(true);
                    if !decision_auth {
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
                    crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
                }
                if let Some(last) = last_node {
                    self.register_ground_path_goal(unit_id, last);
                }
                return true;
            }
        }
        let mut dest = target_pos;
        if !contact {
            self.adjust_to_possible_destination(unit_id, &mut dest);
        }
        if let Some(tid) = target_id {
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                unit.ignored_obstacle_id = Some(tid);
            }
        }
        if !compute_fallback {
            return false;
        }
        if self.assign_unit_path_ignoring(unit_id, dest, &[], target_id) {
            if decision_auth {
                if let Some(tid) = target_id {
                    crate::game_logic::host_ai_decision_log::record_attack(unit_id, tid);
                }
            }
            if let Some(unit) = self.objects.get_mut(&unit_id) {
                if contact {
                    unit.is_attack_path = true;
                    unit.path_extra_distance =
                        10.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
                }
                if !matches!(unit.ai_state, AIState::AttackMoving | AIState::Patrolling) {
                    unit.set_ai_state(AIState::Attacking);
                    if !unit.movement.path.is_empty() {
                        unit.set_status_moving(true);
                    }
                }
                unit.set_status_attacking(true);
                if !decision_auth {
                    if let Some(tid) = target_id {
                        unit.target = Some(tid);
                    }
                }
            }
            if decision_auth {
                crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 2);
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
        // C++ computeAttackPath: no obstacle LOS when the victim is
        // significantly above terrain. KindOf::Aircraft alone is not that.
        if let Some(tid) = target_id {
            if let Some(t) = self.objects.get(&tid) {
                if t.is_significantly_above_terrain() {
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
    ) -> bool {
        self.path_approach_with_state_ignoring(object_id, goal, state, None)
    }

    pub(crate) fn path_approach_with_state_ignoring(
        &mut self,
        object_id: ObjectId,
        goal: Vec3,
        state: AIState,
        ignore_obstacle: Option<ObjectId>,
    ) -> bool {
        let state = self.mood_adjusted_move_state(object_id, state);
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        let ordinal = crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
        let attack_moving = matches!(state, AIState::AttackMoving);
        let already = self
            .objects
            .get(&object_id)
            .is_some_and(|obj| obj.ai_state == state);
        let mut assigned = false;
        if self.assign_unit_path_ignoring(object_id, goal, &[], ignore_obstacle) {
            let already = self
                .objects
                .get(&object_id)
                .is_some_and(|obj| obj.ai_state == state);
            if decision_auth {
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
            }
            if let Some(obj) = self.objects.get_mut(&object_id) {
                if !already {
                    obj.set_ai_state(state.clone());
                }
                if !obj.movement.path.is_empty() {
                    obj.set_status_moving(true);
                }
                if let Some(id) = ignore_obstacle {
                    obj.ignored_obstacle_id = Some(id);
                }
            }
            assigned = true;
        } else if decision_auth {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                if !already {
                    obj.set_ai_state(state.clone());
                }
                if obj.is_alive() && !obj.can_move() {
                    obj.pending_move = Some(goal);
                    obj.movement.target_position = None;
                    obj.movement.path.clear();
                }
            }
            if !already {
                crate::game_logic::host_ai_decision_log::record_set_state(object_id, ordinal);
            }
        } else if let Some(obj) = self.objects.get_mut(&object_id) {
            if !already {
                obj.set_ai_state(state);
            }
            if obj.is_alive() && !obj.can_move() {
                obj.pending_move = Some(goal);
                obj.movement.target_position = None;
                obj.movement.path.clear();
            }
        }
        if attack_moving {
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.requested_destination = Some(goal);
            }
        }
        assigned
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
        unit.is_exact_path = false;
        unit.movement.path = appended;
        let last_node = unit.movement.path.last().copied();
        unit.refresh_follow_path_extra_distance();
        unit.movement.target_position = Some(waypoint);
        crate::game_logic::host_move_log::record(
            unit_id,
            Some([waypoint.x, waypoint.y, waypoint.z]),
        );
        let entered_move = unit.ai_state != AIState::Moving;
        if entered_move {
            unit.set_ai_state(AIState::Moving);
        }
        if entered_move && crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            crate::game_logic::host_ai_decision_log::record_set_state(unit_id, 1);
        }
        unit.set_status_moving(true);
        drop(unit);
        if let Some(last) = last_node {
            self.register_ground_path_goal(unit_id, last);
        }
        true
    }

    #[cfg(test)]
    pub fn append_unit_waypoint_for_test(&mut self, unit_id: ObjectId, waypoint: Vec3) -> bool {
        self.append_unit_waypoint(unit_id, waypoint)
    }
}

#[cfg(test)]
mod group_lane_tests {
    use super::*;
    use crate::game_logic::{GameLogic, GridPos, KindOf, Object, ObjectId, Team, ThingTemplate};

    fn ranger(id: u32, pos: Vec3) -> Object {
        let mut tmpl = ThingTemplate::new("Ranger");
        tmpl.add_kind_of(KindOf::Infantry);
        let mut unit = Object::new(tmpl, ObjectId(id), Team::USA);
        unit.set_position(pos);
        unit
    }

    /// C++ `friend_moveInfantryToPos` per-node column offsets
    /// (AIGroup.cpp:897-1008): a 12-infantry group must march the shared
    /// spine in 3 lateral lanes (lane width 2.1 cells, ±half-cell rank
    /// stagger) instead of one single-file line.
    #[test]
    fn shared_group_paths_march_in_lateral_lanes() {
        let mut logic = GameLogic::new();
        let destination = Vec3::new(0.0, 0.0, 420.0);
        // Bend the spine: wall the direct north route so A* detours east.
        for x in -25..10 {
            logic
                .pathfinding_system
                .grid
                .set_blocked(GridPos::new(x, 10), true);
        }
        let mut goals: Vec<(ObjectId, Vec3)> = Vec::new();
        for i in 0..12_i32 {
            let id = ObjectId(8600 + i as u32);
            logic.objects.insert(
                id,
                ranger(8600 + i as u32, Vec3::new((i as f32 - 5.5) * 6.0, 0.0, 5.0)),
            );
            // pack_column_kind-style column goal: destination + n*(delta*2.2
            // cells + stagger), n = (-dir.z, dir.x) for the north march.
            let column_delta = 1 - i / 4; // 4×+1, 4×0, 4×-1
            let stagger = if i % 2 == 1 { 10.0 } else { 0.0 };
            let lateral = column_delta as f32 * 22.0 + stagger;
            goals.push((id, Vec3::new(-lateral, 0.0, 420.0)));
        }
        assert!(logic.assign_shared_group_paths(&goals, destination));

        let firsts: Vec<Vec3> = (8600..8612_u32)
            .filter_map(|i| {
                logic
                    .host_object(ObjectId(i))
                    .map(|o| o.movement.path.first().copied())
                    .flatten()
            })
            .collect();
        assert_eq!(firsts.len(), 12, "every member must receive a path");
        // Within a lane the alternating stagger keeps nodes ≤ 10 apart;
        // adjacent lanes are 2.1 cells (21) apart, so cross-lane pairs are
        // ≥ 11 apart. 3 lanes × C(4,2) = 18 within-lane pairs exactly.
        let mut within_lane = 0_usize;
        for a in 0..firsts.len() {
            for b in (a + 1)..firsts.len() {
                let d = firsts[a].distance(firsts[b]);
                if d <= 10.5 {
                    within_lane += 1;
                } else {
                    assert!(
                        d >= 11.0,
                        "adjacent lanes must differ by ~lane width, d={d}, firsts={firsts:?}"
                    );
                }
            }
        }
        assert_eq!(
            within_lane, 18,
            "12-unit group must occupy exactly 3 lateral lanes, firsts={firsts:?}"
        );
        // The member goal stays the terminal node of its lane path.
        for (id, goal) in &goals {
            let path = &logic.host_object(*id).unwrap().movement.path;
            assert_eq!(path.last().unwrap(), goal);
        }
    }

    fn occupied_click_world(adjust: bool) -> GameLogic {
        let mut logic = GameLogic::new();
        logic.force_map_loaded_for_path_test(true);
        let start = Vec3::new(0.0, 0.0, 0.0);
        let click = Vec3::new(80.0, 0.0, 0.0);
        let mut car_tmpl = ThingTemplate::new("CivilianCar");
        car_tmpl.add_kind_of(KindOf::Vehicle);
        let mut car = Object::new(car_tmpl, ObjectId(8700), Team::GLA);
        car.set_position(click);
        car.crushable_level = 1;
        car.owner_player_id = Some(1);
        logic.objects.insert(ObjectId(8700), car);
        let mut unit = ranger(8701, start);
        unit.is_final_goal = true;
        unit.adjust_destinations = adjust;
        unit.owner_player_id = Some(0);
        logic.objects.insert(ObjectId(8701), unit);
        logic
            .pathfinding_system
            .grid
            .update_dynamic_obstacles(&logic.objects);
        logic
    }

    #[test]
    fn final_hop_spiral_respects_live_adjust_flag() {
        let click = Vec3::new(80.0, 0.0, 0.0);
        let mut adjusting = occupied_click_world(true);
        adjusting.pathfinding_system.set_adjust_goal(false);
        assert!(adjusting.assign_unit_path(ObjectId(8701), click, &[]));
        adjusting.process_pathfind_queue();
        let open_end = adjusting
            .host_object(ObjectId(8701))
            .and_then(|u| u.movement.path.last().copied())
            .expect("adjusting path");
        assert!(!adjusting.pathfinding_system.adjusts_goal());

        let mut pinned = occupied_click_world(false);
        pinned.pathfinding_system.set_adjust_goal(true);
        let _ = pinned.assign_unit_path(ObjectId(8701), click, &[]);
        pinned.process_pathfind_queue();
        assert!(pinned.pathfinding_system.adjusts_goal());
        let pinned_end = pinned
            .host_object(ObjectId(8701))
            .and_then(|u| u.movement.path.last().copied())
            .expect("non-adjusting path");
        assert_ne!(
            adjusting.pathfinding_system.grid.world_to_grid(open_end),
            pinned.pathfinding_system.grid.world_to_grid(pinned_end),
            "the live adjust flag must change the path end"
        );
    }
}
