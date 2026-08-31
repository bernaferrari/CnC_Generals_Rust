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
        let (start_pos, is_aircraft, surfaces, is_crusher) = match self.objects.get(&object_id) {
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
                    // C++ Pathfinder: `isCrusher = obj ? obj->getCrusherLevel() > 0 : false`
                    // (AIPathfind.cpp:8170). Hardcoding false made tanks halt at fences/rubble.
                    obj.crusher_level > 0,
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
            is_crusher,
            Some(object_id),
        );

        let mut state_to_apply: Option<AIState> = None;
        let mut nudge_allies: Vec<ObjectId> = Vec::new();
        if let Some(obj) = self.objects.get_mut(&object_id) {
            // C++ FollowPath onExit clears canPathThroughUnits. A new
            // computePath replaces the factory-exit tunnel.
            obj.can_path_through_units = false;
            if let Some(waypoints) = path {
                if waypoints.len() >= 2 {
                    obj.movement.path = waypoints;
                    obj.record_host_movement();
                    obj.movement.current_path_index = 1; // skip start node
                    obj.movement.target_position = Some(obj.movement.path[1]);
                    obj.start_move();
                    obj.set_status_moving(true);
                    crate::game_logic::host_move_log::record(
                        object_id,
                        Some([target_position.x, target_position.y, target_position.z]),
                    );
                    state_to_apply = Some(ai_state_override.unwrap_or(AIState::Moving));
                } else {
                    // A* found the goal cell (often start==goal after snap).
                    let dest = waypoints.last().copied().unwrap_or(target_position);
                    obj.movement.path = vec![start_pos, dest];
                    obj.record_host_movement();
                    obj.movement.current_path_index = 1;
                    obj.movement.target_position = Some(dest);
                    obj.start_move();
                    obj.set_status_moving(true);
                    crate::game_logic::host_move_log::record(
                        object_id,
                        Some([dest.x, dest.y, dest.z]),
                    );
                    state_to_apply = Some(ai_state_override.unwrap_or(AIState::Moving));
                }
            } else {
                log::debug!(
                    "No path found for {:?} to {:?}; refuse fail-open march",
                    object_id,
                    target_position
                );
            }
        }
        if state_to_apply.is_some() {
            if let Some(obj) = self.objects.get(&object_id) {
                let path = obj.movement.path.clone();
                nudge_allies = self.pathfinding_system.allies_to_nudge_off_path(
                    object_id,
                    &path,
                    &self.objects,
                );
            }
        }
        if let Some(state) = state_to_apply {
            apply_state(self, state);
        }
        let mover_radius = self
            .objects
            .get(&object_id)
            .map(|o| o.selection_radius)
            .unwrap_or(0.0);
        let mover_path = self
            .objects
            .get(&object_id)
            .map(|o| o.movement.path.clone())
            .unwrap_or_default();
        for ally in nudge_allies {
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
                None,
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
                    None,
                    surfaces,
                    is_crusher,
                    unit_radius,
                    mover_radius,
                    seeker_player,
                    crusher_level,
                    true,
                );
            }
            if let Some(path) = yield_path {
                if let Some(obj) = self.objects.get_mut(&ally) {
                    obj.apply_move_away_path(object_id, &path);
                }
            }
        }
    }

    /// Update movement for all objects
    pub(in super::super) fn update_movement(&mut self, object_ids: &[ObjectId], dt: f32) {
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
        let mut repaths: Vec<(ObjectId, Vec<Vec3>)> = Vec::new();
        for &id in object_ids {
            let Some(obj) = self.objects.get(&id) else {
                continue;
            };
            if obj.is_disabled() || obj.host_skip_dead_locomotor() {
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
                        repaths.push((id, path));
                    }
                }
            } else if (obj.is_blocked_and_stuck || obj.num_frames_blocked > 60)
                && obj.movement.path.len() >= 2
            {
                let from = obj.get_position();
                let original = obj.movement.path.clone();
                if let Some(path) = self.pathfinding_system.patch_path(
                    from,
                    &original,
                    surfaces,
                    is_crusher,
                    &self.objects,
                    Some(id),
                ) {
                    repaths.push((id, path));
                }
            }
        }
        for (id, path) in repaths {
            if let Some(obj) = self.objects.get_mut(&id) {
                if path.len() >= 2 {
                    obj.movement.path = path;
                    obj.movement.current_path_index = 1;
                    obj.movement.target_position = Some(obj.movement.path[1]);
                    obj.is_blocked_and_stuck = false;
                    obj.set_status_moving(true);
                    obj.record_host_movement();
                }
            }
        }

        for &id in object_ids {
            let (ground_y, surface_y, climber_ahead_y, cell_type, underwater) = {
                let Some(obj) = self.objects.get(&id) else {
                    continue;
                };
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
                let cell = self.pathfinding_system.grid.world_to_grid(pos);
                let cell_type = self.pathfinding_system.grid.cell_type(cell);
                let underwater = self
                    .terrain
                    .as_ref()
                    .is_some_and(|t| t.is_underwater_at_world(pos));
                (gy, sy, ahead_y, cell_type, underwater)
            };
            let mut plant_snap = false;
            'unit: {
                if let Some(obj) = self.objects.get_mut(&id) {
                    // C++ GameLogic.cpp:3677-3718: UpdateModules (including AI/locomotor
                    // movement) are skipped while any disabled flag is set and does not
                    // intersect getDisabledTypesToProcess (AIUpdate default is NONE).
                    // EMP / hack / unmanned / paralyzed / subdued / held-as-is_disabled.
                    if obj.is_disabled() {
                        obj.movement.velocity = Vec3::ZERO;
                        obj.record_host_movement();
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
                        break 'unit;
                    }

                    let has_move_goal =
                        obj.movement.target_position.is_some() || !obj.movement.path.is_empty();
                    // C++ POSITION/ANGLE goals clear m_doFinalPosition (AIUpdate.cpp:2151).
                    if has_move_goal {
                        obj.do_final_position = false;
                    }
                    let skip_loco_move = obj.waiting_for_path;
                    if has_move_goal && !skip_loco_move {
                        obj.apply_motive_force(glam::Vec3::ZERO);
                    }
                    // C++ Locomotor.cpp:1055 — treatAsAirborne skips appearance
                    // 2D motive only. handleBehaviorZ, IS_BRAKING, braking cheat,
                    // path advance, hover OVER_WATER, and arrival still run
                    // (hq-hq4t8).
                    let allow_2d_motive = obj.allow_motive_force_while_airborne
                        || !Object::height_treats_as_airborne(obj.get_position().y - ground_y);
                    if obj.is_rappelling() {
                        // C++ AIRappelState owns Z; handleBehaviorZ must not snap to Y=0.
                        Self::stamp_object_airborne_target(obj, ground_y);
                        break 'unit;
                    }
                    if obj.waiting_for_path {
                        // C++ queueForPath: locomotor does not integrate until Path is installed.
                        obj.movement.velocity = Vec3::ZERO;
                        obj.record_host_movement();
                        Self::apply_live_handle_behavior_z(obj, surface_y, None);
                        Self::stamp_object_airborne_target(obj, ground_y);
                        break 'unit;
                    }
                    if matches!(obj.ai_state, AIState::AttackMoving) && obj.target.is_some() {
                        // C++ AIAttackMoveToState::update: setLocomotorGoalNone while
                        // the nested attack machine is not idle. A replaced chase
                        // path (findAttackPath) is the nested locomotor goal.
                        let dest_walk = obj.requested_destination.map(|dest| {
                            let near = |p: Vec3| {
                                let dx = p.x - dest.x;
                                let dz = p.z - dest.z;
                                dx * dx + dz * dz < 16.0
                            };
                            obj.movement.path.last().copied().is_some_and(near)
                                || obj.movement.target_position.is_some_and(near)
                        });
                        if dest_walk.unwrap_or(true) {
                            obj.movement.velocity = Vec3::ZERO;
                            obj.set_status_moving(false);
                            Self::apply_live_handle_behavior_z(obj, surface_y, None);
                            Self::stamp_object_airborne_target(obj, ground_y);
                            break 'unit;
                        }
                    }

                    // C++ doLocomotor: chooseGoodLocomotorFromCurrentSet then blocked bookkeeping.
                    obj.choose_good_locomotor_from_current_set(cell_type);
                    obj.tick_do_locomotor_blocked_frames();
                    if obj.num_frames_blocked > 7 {
                        // AIInternalMoveToState: > 1/4 s blocked clears MODELCONDITION_MOVING.
                        obj.set_status_moving(false);
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
                            ) && try_fix_invalid_position_3x3(
                                obj,
                                &self.pathfinding_system.grid,
                                surfaces,
                            ) {
                                Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                Self::stamp_object_airborne_target(obj, ground_y);
                                break 'unit;
                            }
                        }
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

                    if !obj.movement.path.is_empty()
                        && obj.movement.current_path_index < obj.movement.path.len()
                    {
                        let current_pos = obj.get_position();
                        let waypoint = obj.movement.path[obj.movement.current_path_index];
                        if obj.host_locomotor_distance_to_goal(current_pos, waypoint) < close_enough
                        {
                            let finishing =
                                obj.movement.current_path_index + 1 >= obj.movement.path.len();
                            let last = *obj.movement.path.last().unwrap_or(&waypoint);
                            // C++ AIStates.cpp:1889-1904 — ground sanity refuses to
                            // plant if 2D to last node > 4*PATHFIND_CELL_SIZE.
                            if finishing
                                && !z_motive
                                && horiz(current_pos, last) > close_enough_sanity
                            {
                                // Keep marching toward the last node.
                            } else {
                                obj.movement.current_path_index += 1;
                                if obj.movement.current_path_index >= obj.movement.path.len() {
                                    let do_evac = obj.pending_evacuate_on_stop;
                                    let and_exit = obj.pending_exit_after_evacuate;
                                    if obj.holds_air_position_when_idle() {
                                        obj.movement.path.clear();
                                        obj.movement.current_path_index = 0;
                                        obj.movement.target_position = None;
                                        obj.maintain_pos_valid = false;
                                        obj.can_path_through_units = false;
                                        let _ = obj.loco_maintain_current_position(surface_y, dt);
                                    } else {
                                        obj.stop_moving();
                                        plant_snap = true;
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
                        );
                        let mut target = lead;
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
                            // C++ moveTowardsPositionClimb (Locomotor.cpp:1690-1739).
                            let mut speed = obj.effective_max_speed();
                            if matches!(obj.loco_appearance, LocomotorAppearance::Climber) {
                                let backwards = obj.update_climber_flags(
                                    current_pos,
                                    target_pos,
                                    climber_ahead_y,
                                );
                                speed *=
                                    obj.climber_slope_speed_scale(current_pos.y, climber_ahead_y);
                                if backwards {
                                    desired_angle += std::f32::consts::PI;
                                }
                            }
                            speed = obj.apply_do_locomotor_blocked_speed(speed);
                            // C++ Locomotor.cpp:1016-1040 — blocked frame scrubs 2D
                            // motive and only rotates / handleBehaviorZ.
                            let mut loco_blocked = obj.num_frames_blocked > 0;
                            if loco_blocked {
                                if speed > obj.movement.velocity.length() {
                                    loco_blocked = false;
                                }
                                let air = (obj.locomotor_surfaces
                                    & gamelogic::ai::pathfind_complete::SURFACE_AIR)
                                    != 0;
                                if air
                                    && Object::height_treats_as_airborne(current_pos.y - ground_y)
                                {
                                    loco_blocked = false;
                                }
                            }
                            if loco_blocked {
                                obj.scrub_velocity_2d(speed);
                                if obj.wander_width_factor == 0.0 {
                                    let _ = obj.rotate_obj_around_loco_pivot(
                                        flat_target,
                                        obj.effective_turn_rate() * dt,
                                    );
                                }
                                obj.record_host_movement();
                                Self::apply_live_handle_behavior_z(obj, surface_y, None);
                                Self::stamp_object_airborne_target(obj, ground_y);
                                break 'unit;
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
                            // Hover / !ground / aircraft use computeFlightDistToGoal
                            // (AIUpdate.cpp:2412-2466) so dogleg winding does not
                            // brake Comanche/Chinook/Helix early.
                            let path_for_dist = if obj.movement.path.is_empty() {
                                None
                            } else {
                                Some(
                                    &obj.movement.path
                                        [obj.movement.current_path_index.saturating_sub(1)..],
                                )
                            };
                            let treat_as_aircraft =
                                !crate::game_logic::PathfindingGrid::is_doing_ground_movement(obj)
                                    || matches!(obj.loco_appearance, LocomotorAppearance::Hover);
                            let mut on_path_dist = if obj.host_uses_close_enough_dist_3d() {
                                // Leftover unused `get_locomotor_distance_to_goal`
                                // FROM_CENTER_3D to last node (AIUpdate.cpp:2448-2456).
                                let dest = obj.movement.path.last().copied().unwrap_or(target_pos);
                                obj.host_locomotor_distance_to_goal(current_pos, dest)
                            } else {
                                path_for_dist
                                .map(|wps| {
                                    if treat_as_aircraft {
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
                            // C++ getIsDownhillOnly: refuse uphill goals (Locomotor.cpp:1596-1598).
                            if obj.downhill_only_blocks_goal(current_pos.y, target_pos.y) {
                                obj.movement.velocity = Vec3::ZERO;
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
                                            let _ =
                                                obj.loco_maintain_current_position(surface_y, dt);
                                        } else {
                                            obj.stop_moving();
                                            plant_snap = true;
                                        }
                                    } else {
                                        obj.movement.current_path_index += 1;
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
                                        speed = obj.movement.velocity.length() * 0.6;
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
                                        obj.record_host_locomotor();
                                    }
                                }
                                if obj.moving_backwards {
                                    if delta.abs() < std::f32::consts::FRAC_PI_2 {
                                        obj.moving_backwards = false;
                                        obj.record_host_locomotor();
                                    } else {
                                        move_backwards = true;
                                        // Far goals keep facing the dest (3-point); nearby
                                        // reverse mirrors desiredAngle (Locomotor.cpp:1307-1310).
                                        if on_path <= 5.0 * major {
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
                                let mut actual = obj.movement.velocity.length();
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
                                        valid_movement_terrain_at(grid, surfaces, host)
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
                            if !obj.no_slow_down_as_approaching_dest {
                                speed = obj.apply_cpp_approach_brake(
                                    on_path_dist,
                                    obj.movement.velocity.length(),
                                    speed,
                                    self.frame,
                                );
                            }
                            // C++ Locomotor.cpp:2344-2361 ULTRA_ACCURATE slide-into-place.
                            // Appearance 2D (rotate + motive Euler) is skipped when
                            // treatAsAirborne && !AllowAirborneMotiveForce (hq-hq4t8).
                            let march_from = obj.get_position();
                            let mut new_position = march_from;
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
                                    (speed / frames) * obj.ultra_accurate_slide_factor;
                                let sliding = slide_other_or_hover
                                    && obj.ultra_accurate
                                    && obj.ultra_accurate_slide_factor > 0.0
                                    && (flat_target.x - current_pos.x).abs() <= slide_thresh
                                    && (flat_target.z - current_pos.z).abs() <= slide_thresh;
                                let new_angle = if sliding {
                                    current_angle
                                } else {
                                    // C++ rotateTowardsPosition always calls
                                    // rotateObjAroundLocoPivot (Locomotor.cpp:901-907,
                                    // 2113-2187). TurnPivotOffset != 0 (combat bikes
                                    // seed -0.60) yaws around the rear axle so the
                                    // hull center translates. Aim at a far point on
                                    // the already-biased desired heading so wander /
                                    // climber reverse / nearby reverse survive.
                                    let rotate_goal = glam::Vec3::new(
                                        current_pos.x + desired_angle.cos() * 1000.0,
                                        current_pos.y,
                                        current_pos.z + (-desired_angle.sin()) * 1000.0,
                                    );
                                    let (_turning, _rel) =
                                        obj.rotate_towards_position(rotate_goal, dt);
                                    obj.get_orientation()
                                };

                                let heading = if sliding {
                                    glam::Vec3::new(direction.x, 0.0, direction.z)
                                } else {
                                    glam::Vec3::new(new_angle.cos(), 0.0, -new_angle.sin())
                                };
                                let signed_speed = if move_backwards { -speed } else { speed };
                                let target_velocity = heading * signed_speed;
                                let velocity_diff = target_velocity - obj.movement.velocity;
                                let accel = obj.effective_acceleration();
                                let max_accel = if obj.is_braking {
                                    obj.braking_factor.max(1.0) * obj.braking.max(accel) * dt
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

                                // Pivot rotate already translated the hull; integrate
                                // from that pose (C++ setTransformMatrix then physics).
                                new_position = march_from + new_velocity * dt;
                            }
                            if obj.is_braking {
                                // C++ OBJECT_STATUS_BRAKING pose cheat (Locomotor.cpp:1092-1138).
                                new_position = obj.braking_cheat_step(march_from, flat_target, dt);
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
                            if reached_target {
                                // Only stop when there is no further path waypoint.
                                // Mid-path "reached" is handled by index advance above.
                                if obj.movement.path.is_empty()
                                    || obj.movement.current_path_index + 1
                                        >= obj.movement.path.len()
                                {
                                    if obj.holds_air_position_when_idle() {
                                        obj.movement.path.clear();
                                        obj.movement.current_path_index = 0;
                                        obj.movement.target_position = None;
                                        obj.maintain_pos_valid = false;
                                        let _ = obj.loco_maintain_current_position(surface_y, dt);
                                    } else {
                                        obj.stop_moving();
                                        plant_snap = true;
                                    }
                                } else {
                                    obj.movement.current_path_index += 1;
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
                                if obj.holds_air_position_when_idle() {
                                    obj.maintain_pos_valid = false;
                                }
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
                                    if obj.holds_air_position_when_idle() {
                                        obj.maintain_pos_valid = false;
                                    }
                                    let _ = obj.loco_maintain_current_position(surface_y, dt);
                                } else {
                                    obj.movement.velocity = Vec3::ZERO;
                                    obj.record_host_movement();
                                }
                            }
                            if obj.movement.path.is_empty()
                                || obj.movement.current_path_index + 1 >= obj.movement.path.len()
                            {
                                if obj.holds_air_position_when_idle() {
                                    obj.movement.path.clear();
                                    obj.movement.current_path_index = 0;
                                    obj.movement.target_position = None;
                                } else {
                                    obj.stop_moving();
                                    plant_snap = true;
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
                            }
                        }
                    }
                    Self::stamp_object_airborne_target(obj, ground_y);
                }
            }
            if plant_snap {
                self.apply_arrival_goal_snap(id);
            }
        }

        self.drain_pending_transport_exits();
    }

    /// C++ `AIUpdateInterface::update` movement-complete `setFinalPosition`
    /// then NONE-goal leftover settle (AIUpdate.cpp:1039-1041, 2234-2262).
    /// Snap computes the plant cell; leftover marches 2 cells/s — no teleport.
    fn apply_arrival_goal_snap(&mut self, id: ObjectId) {
        let Some(obj) = self.objects.get(&id) else {
            return;
        };
        if obj.holds_air_position_when_idle() {
            return;
        }
        if obj.is_kind_of(KindOf::Aircraft)
            || obj.object_type == crate::game_logic::ObjectType::Aircraft
        {
            return;
        }
        let pos = obj.get_position();
        let surfaces = if obj.locomotor_surfaces != 0 {
            obj.locomotor_surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };
        let is_crusher = obj.crusher_level > 0;
        let radius = obj.selection_radius;
        let player = obj.owner_player_id.or(Some(obj.team as u32));
        let snapped = self.pathfinding_system.snap_plant_goal(
            pos,
            surfaces,
            is_crusher,
            radius,
            id,
            player,
            &self.objects,
        );
        if let Some(obj) = self.objects.get_mut(&id) {
            // Leftover `set_final_position` matches the C++ header (`= false`)
            // but settle only runs when the flag is armed. Live arms it.
            obj.final_position = snapped;
            obj.do_final_position = true;
            leftover_settle_final_position_on_object(obj);
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
            if let Some(obj) = self.objects.get_mut(&id) {
                if !obj.is_disabled() {
                    obj.stamp_airborne_target_from_locomotor();
                }
            }
        }
    }

    fn stamp_object_airborne_target(obj: &mut Object, ground_y: f32) {
        obj.ground_height = ground_y;
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

/// C++ `Pathfinder::validMovementTerrain` (AIPathfind.cpp:4763-4783).
/// Obstacle/Impassable are terrain-present (true). Else locomotor surfaces
/// must intersect the cell's surface mask. Out-of-grid is false (NULL cell).
fn valid_movement_terrain_at(
    grid: &crate::game_logic::PathfindingGrid,
    surfaces: u32,
    world_pos: Vec3,
) -> bool {
    use crate::game_logic::locomotor_bootstrap::valid_locomotor_surfaces_for_cell_type;
    use gamelogic::ai::pathfind_astar::PathfindCellType;
    let cell = grid.world_to_grid(world_pos);
    if !grid.is_valid_pos(cell) {
        return false;
    }
    let ty = grid.cell_type(cell);
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
            if !valid_movement_terrain_at(grid, surfaces, check) {
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
    let on_ground = !obj.is_above_terrain();
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
            obj.movement.velocity.x > 1.0,
            "reverse drive must back toward +X goal, vel={:?}",
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        truck.movement.turn_rate = std::f32::consts::PI;
        truck.movement.max_speed = 40.0;
        truck.movement.acceleration = 10_000.0;
        truck.movement.velocity = Vec3::ZERO;
        truck.no_slow_down_as_approaching_dest = true;
        truck.thing.template.geometry_info.authored = true;
        truck.thing.template.geometry_info.major_radius = 8.0;
        // Behind, closer than 5*majorRadius → reverse, not 3-point.
        truck.movement.target_position = Some(Vec3::new(-20.0, 0.0, 0.0));
        logic.objects.insert(id, truck);
        logic.update_movement_for_test(&[id], 1.0 / 30.0);
        let obj = logic.objects.get(&id).expect("truck");
        assert!(
            obj.moving_backwards,
            "CanMoveBackwards + rear goal at rest must set MOVING_BACKWARDS"
        );
        assert!(
            obj.movement.velocity.x < -1.0,
            "nearby reverse must accelerate backward, vel={:?}",
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        assert!(self_u.ai_process_collision(&other, 1, true) == false);
        assert!(self_u.is_blocked);
        assert!(
            self_u.is_blocked_and_stuck,
            "other stopped + facing dest must stick immediately"
        );
    }

    #[test]
    fn march_apply_motive_force_zero_flags_driven() {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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

    /// hq-xg2ym: arrival leftover-marches to the plant cell (no teleport).
    #[test]
    fn arrival_snap_offsets_off_occupied_pad() {
        let mut logic = GameLogic::new();
        let pad = Vec3::new(80.0, 0.0, 80.0);
        let parked_id = ObjectId(7001);
        let arriver_id = ObjectId(7002);
        logic.objects.insert(parked_id, ranger_at(7001, pad));
        let mut arriver = ranger_at(7002, pad);
        arriver.movement.path = vec![Vec3::new(70.0, 0.0, 80.0), pad];
        arriver.movement.current_path_index = 1;
        arriver.movement.target_position = Some(pad);
        arriver.set_status_moving(true);
        arriver.set_ai_state(AIState::Moving);
        logic.objects.insert(arriver_id, arriver);
        logic.update_movement_for_test(&[parked_id, arriver_id], 1.0 / 30.0);
        {
            let obj = logic.objects.get(&arriver_id).expect("arriver");
            let pos = obj.get_position();
            let dx = pos.x - pad.x;
            let dz = pos.z - pad.z;
            let dist = (dx * dx + dz * dz).sqrt();
            assert!(
                obj.do_final_position,
                "arrival must arm leftover do_final_position, pos={pos:?}"
            );
            assert!(
                dist > 0.1 && dist < crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL,
                "first settle step must leftover-march, not teleport, pos={pos:?} dist={dist}"
            );
        }
        for _ in 0..20 {
            logic.update_movement_for_test(&[parked_id, arriver_id], 1.0 / 30.0);
        }
        let pos = logic
            .objects
            .get(&arriver_id)
            .expect("arriver")
            .get_position();
        let dx = pos.x - pad.x;
        let dz = pos.z - pad.z;
        let dist = (dx * dx + dz * dz).sqrt();
        assert!(
            dist > 1.0,
            "leftover settle must finish off the occupied pad, pos={pos:?}"
        );
    }

    /// hq-xg2ym: NONE-goal leftover settle is 2 cells/s then DARN_CLOSE snap.
    #[test]
    fn leftover_goal_none_settles_final_position() {
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
        crate::env_compat::set_var("GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY", "0");
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
}
