//! Native host `AI_PANIC` waypoint-path state.
use super::*;

fn panic_waypoint_extra_distance(terrain: &gamelogic::terrain::TerrainLogic, start_id: u32) -> f32 {
    let mut extra = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL / 10.0;
    let mut current_id = start_id;
    for _ in 0..5 {
        let Some(current) = terrain.get_waypoint_by_id(current_id) else {
            break;
        };
        let Some(next_id) = current.get_link(0) else {
            break;
        };
        let Some(next) = terrain.get_waypoint_by_id(next_id) else {
            break;
        };
        let a = current.get_location();
        let b = next.get_location();
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        extra += (dx * dx + dy * dy).sqrt();
        current_id = next_id;
    }
    extra
}

/// `AIInternalMoveToState::update` succeeds only when the active path's
/// locomotor distance is below close-enough and the ground last-node sanity
/// check passes. `retry_path` controls a later retry; it does not determine
/// arrival. This runs before this frame's locomotor pass, so it observes the
/// path/position produced by the previous pass.
fn panic_path_reached_goal(unit: &crate::game_logic::Object) -> bool {
    if unit.waiting_for_path {
        return false;
    }
    let Some(locomotor_name) = unit.cur_locomotor_name.as_deref() else {
        return false;
    };
    let Some(binding) =
        crate::game_logic::locomotor_bootstrap::resolve_host_locomotor_binding(locomotor_name)
    else {
        return false;
    };
    let Some(last) = unit.movement.path.last().copied() else {
        return false;
    };
    let current = unit.get_position();
    let close_enough = unit
        .close_enough_dist
        .filter(|distance| distance.is_finite() && *distance >= 0.5)
        .unwrap_or(binding.close_enough_dist);
    if !(unit.host_locomotor_distance_to_goal(current, last) < close_enough) {
        return false;
    }
    if crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit) {
        let dx = current.x - last.x;
        let dz = current.z - last.z;
        let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
        if (dx * dx + dz * dz).sqrt() > 4.0 * cell {
            return false;
        }
    }
    true
}

impl GameLogic {
    /// Complete the terminal `AIInternalMoveToState` exit before this frame's
    /// locomotor pass. C++ first exits the state, then consumes
    /// `m_movementComplete` in `AIUpdate::update`: it drops the path, clears
    /// the locomotor goal and ignored obstacle, and reconciles the pathfinder
    /// goal through the ordinary final-position snap.
    fn finish_host_panic_movement(&mut self, id: ObjectId) {
        {
            let Some(unit) = self.objects.get_mut(&id) else {
                return;
            };
            unit.set_ai_state(AIState::Idle);
            unit.set_locomotor_goal_none();
        }

        // This is the same pathfinder goal reconciliation used by other
        // completed movement. Run it after the state exit while the stored
        // pathfind goal cell is still available.
        self.apply_arrival_goal_snap(id, None);

        if let Some(unit) = self.objects.get_mut(&id) {
            unit.movement.path.clear();
            unit.movement.current_path_index = 0;
            unit.movement.target_position = None;
            unit.waiting_for_path = false;
            unit.queue_for_path_frames = 0;
            unit.ignored_obstacle_id = None;
            unit.is_attack_path = false;
            unit.is_panicking = false;
            let panicking_bit = crate::game_logic::host_enum_table_residual::panicking_model_bit();
            unit.model_condition_bits &= !(1u128 << panicking_bit);
            unit.record_host_model_condition();
            unit.set_status_moving(false);
            unit.set_locomotor_goal_none();
        }
    }

    /// Resolve the entry waypoint and the C++ AIFollowWaypointPath lookahead
    /// distance before mutating the unit. Terrain waypoint coordinates use
    /// XY; host Object coordinates use XZ.
    pub(super) fn host_panic_waypoint_from(
        &self,
        path_label: &str,
        from: glam::Vec3,
    ) -> Option<(u32, glam::Vec3, bool, f32)> {
        let from_terrain = gamelogic::common::Coord3D::new(from.x, from.z, from.y);
        let terrain_owner_handle = gamelogic::terrain::get_terrain_logic();
        let terrain = terrain_owner_handle.read().ok()?;
        let start = terrain.get_closest_waypoint_on_path(&from_terrain, path_label)?;
        let waypoint_id = start.get_id();
        let mut current_id = waypoint_id;
        let mut extra = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL / 10.0;
        let mut has_links = false;
        for _ in 0..5 {
            let current = terrain.get_waypoint_by_id(current_id)?;
            if current.get_num_links() == 0 {
                break;
            }
            has_links = true;
            let next_id = current.get_link(0)?;
            let next = terrain.get_waypoint_by_id(next_id)?;
            let a = current.get_location();
            let b = next.get_location();
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            extra += (dx * dx + dy * dy).sqrt();
            current_id = next_id;
        }
        let location = *start.get_location();
        drop(terrain);
        let goal = glam::Vec3::new(location.x, location.z, location.y);
        Some((waypoint_id, goal, has_links, extra))
    }

    /// C++ `ScriptActions::doTeamPanic` → `AIUpdateInterface::aiPanic`.
    pub(super) fn host_start_panic(
        &mut self,
        id: ObjectId,
        waypoint_id: u32,
        goal: glam::Vec3,
        has_links: bool,
        extra_distance: f32,
        locomotor_set: &str,
    ) {
        if !self.apply_unit_locomotor_set(id, locomotor_set) {
            return;
        }
        let offset = self
            .objects
            .get(&id)
            .map(|unit| leftover_wander_group_offset(unit.wander_width_factor))
            .unwrap_or(glam::Vec2::ZERO);
        let (goal, append_goal, goal_layer) = self.host_prepare_panic_goal(goal, offset);
        let actual_goal = goal;
        self.unit_command_waypoint_path_prep(id, false);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ai_state(AIState::Panic);
            if unit.ai_state != AIState::Panic {
                return;
            }
            unit.panic_runtime = Some(crate::game_logic::object::PanicRuntime::new(
                waypoint_id,
                offset,
                id.0,
            ));
            if let Some(panic) = unit.panic_runtime.as_mut() {
                panic.append_goal_position = append_goal;
                panic.goal_layer = goal_layer;
            }
            unit.is_panicking = true;
            unit.path_extra_distance = extra_distance;
            unit.adjust_destinations = !append_goal && !has_links;
            if append_goal {
                unit.set_allow_invalid_position(true);
            }
        }
        let path_installed = self.assign_unit_path(id, goal, &[]);
        if let Some(unit) = self.objects.get_mut(&id) {
            // C++ computePath/requestPath queues the movement and returns
            // success even if A* later cannot produce a route. The panic
            // state remains active; InternalMoveTo failure is converted by
            // AIPanicState::update into CONTINUE.
            if !path_installed {
                unit.retry_path = true;
            }
            // assign_unit_path installs the shared canonical Movement path;
            // its state transition preserves Panic as a native movement state.
            unit.path_extra_distance = extra_distance;
            unit.adjust_destinations = !append_goal && !has_links;
            unit.path_goal_position = Some(actual_goal);
            unit.requested_destination = Some(actual_goal);
            if append_goal {
                unit.set_allow_invalid_position(true);
            }
        }
    }

    /// C++ AIFollowWaypointPathState::computeGoal clamps only an offset goal
    /// whose waypoint itself is on-map. A waypoint beyond the pathfind extent
    /// remains the final goal and is appended after the reachable path.
    fn host_prepare_panic_goal(
        &self,
        waypoint_goal: glam::Vec3,
        offset: glam::Vec2,
    ) -> (glam::Vec3, bool, u8) {
        let on_wall = self.pathfinding_system.is_point_on_wall(waypoint_goal);
        let mut base_goal = waypoint_goal;
        if on_wall {
            base_goal.y = self.pathfinding_system.wall_height();
        } else if let Some(height) =
            self.terrain_height_at(glam::Vec3::new(base_goal.x, 0.0, base_goal.z))
        {
            base_goal.y = height;
        }
        let mut goal = leftover_apply_wander_group_offset(base_goal, offset);
        if on_wall {
            if !self.pathfinding_system.is_point_on_wall(goal) {
                goal = base_goal;
            }
        } else if let Some(height) = self.terrain_height_at(glam::Vec3::new(goal.x, 0.0, goal.z)) {
            // C++ computes ground height after applying the group offset.
            goal.y = height;
        }
        let terrain_owner_handle = gamelogic::terrain::get_terrain_logic();
        let Some(extent) = terrain_owner_handle
            .read()
            .ok()
            .map(|terrain| terrain.get_maximum_pathfind_extent())
        else {
            return (goal, false, if on_wall { 15 } else { 1 });
        };
        let to_terrain =
            |point: glam::Vec3| gamelogic::common::Coord3D::new(point.x, point.z, point.y);
        if extent.is_in_region_no_z(&to_terrain(waypoint_goal))
            && !extent.is_in_region_no_z(&to_terrain(goal))
        {
            let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
            goal.x = goal.x.clamp(extent.lo.x + cell, extent.hi.x - cell);
            goal.z = goal.z.clamp(extent.lo.y + cell, extent.hi.y - cell);
        }
        let append_goal = !extent.is_in_region_no_z(&to_terrain(goal));
        (goal, append_goal, if on_wall { 15 } else { 1 })
    }

    /// Update AIPanic before the host locomotor pass, matching C++
    /// AIUpdate::UPDATE before AIUpdate::doLocomotor. The path on Object is
    /// the single canonical path; this method only advances panic's waypoint
    /// continuation after the preceding locomotor pass completed it.
    pub(in super::super::super) fn tick_host_panic_states(&mut self, object_ids: &[ObjectId]) {
        for &id in object_ids {
            let Some((mut panic, can_be_repulsed, vision)) =
                self.objects.get(&id).and_then(|unit| {
                    let panic = unit.panic_runtime.clone()?;
                    (unit.ai_state == AIState::Panic
                        && unit.is_alive()
                        && !unit.status.destroyed
                        && !unit.is_disabled()
                        && !unit.is_kind_of(KindOf::Immobile))
                    .then_some((
                        panic,
                        unit.is_kind_of(KindOf::CanBeRepulsed),
                        unit.vision_range,
                    ))
                })
            else {
                continue;
            };

            // C++ AIInternalMoveToState performs its blocked-path recompute
            // before AIPanicState decrements the repulsor timer. A failed
            // recompute suppresses arrival for this update, but must not skip
            // the timer check that follows it.
            let blocked_repath = self
                .objects
                .get(&id)
                .is_some_and(|unit| unit.is_blocked_and_stuck || unit.num_frames_blocked > 2 * 30);
            let mut blocked_repath_succeeded = None;
            if blocked_repath {
                panic.blocked_repath_timestamp = self.frame;
                let destination = self
                    .objects
                    .get(&id)
                    .and_then(|unit| unit.requested_destination.or(unit.path_goal_position));
                blocked_repath_succeeded = Some(
                    destination
                        .is_some_and(|destination| self.assign_unit_path(id, destination, &[])),
                );
            }
            // Re-read after a possible path recompute, matching the C++
            // success test that follows computePath. A failed recompute is
            // FAILURE (which AIPanic converts to CONTINUE), not arrival.
            let finished = blocked_repath_succeeded != Some(false)
                && self.objects.get(&id).is_some_and(panic_path_reached_goal);

            // AIInternalMoveTo clears the explicit locomotor goal on success
            // before AIPanicState's timer and waypoint selection.
            if finished {
                if let Some(unit) = self.objects.get_mut(&id) {
                    if super::super::movement_support::adjusts_destination_now(unit) {
                        unit.set_locomotor_goal_none();
                    }
                }
            }

            // AIPanicState::update runs the internal move update first, then
            // decrements/queries the repulsor timer, including after failure.
            if can_be_repulsed {
                panic.timer -= 1;
                if panic.timer < 0 {
                    panic.timer = panic.wait_frames;
                    if leftover_wander_has_repulsor(self, id, vision) {
                        self.finish_host_panic_movement(id);
                        let _ = self.host_wander_fail_to_repulse(id);
                        continue;
                    }
                }
            }
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.panic_runtime = Some(panic.clone());
            }
            if panic.append_goal_position {
                let waiting = self
                    .objects
                    .get(&id)
                    .is_some_and(|unit| unit.waiting_for_path);
                if waiting {
                    continue;
                }
                if self
                    .objects
                    .get(&id)
                    .is_some_and(|unit| unit.movement.path.is_empty())
                {
                    // An unavailable appended route makes the internal move
                    // fail, but AIPanic converts that result to CONTINUE.
                    // Keep the panic state and wait for its normal retry path.
                    continue;
                }
                let appended = if let Some(unit) = self.objects.get_mut(&id) {
                    if let Some(goal) = unit.path_goal_position.or(unit.requested_destination) {
                        if unit
                            .movement
                            .path
                            .last()
                            .is_some_and(|last| (*last - goal).length_squared() < 0.01)
                        {
                            true
                        } else {
                            unit.movement.path.push(goal);
                            if unit.movement.target_position.is_none() {
                                unit.movement.current_path_index =
                                    unit.movement.path.len().saturating_sub(2);
                                unit.movement.target_position = Some(goal);
                                unit.set_status_moving(true);
                            }
                            true
                        }
                    } else {
                        false
                    }
                } else {
                    false
                };
                if appended {
                    panic.append_goal_position = false;
                    if let Some(unit) = self.objects.get_mut(&id) {
                        unit.panic_runtime = Some(panic);
                    }
                    continue;
                }
            }
            if !finished {
                continue;
            }

            let next = (|| -> Option<Option<(u32, glam::Vec3, f32, bool)>> {
                let terrain_owner_handle = gamelogic::terrain::get_terrain_logic();
                let Ok(terrain) = terrain_owner_handle.read() else {
                    return None;
                };
                let Some(current) = terrain.get_waypoint_by_id(panic.current_waypoint_id) else {
                    return None;
                };
                let count = current.get_num_links();
                // C++ getNextWaypoint calls RandomValue(0, linkCount - 1)
                // even for zero links. The unsigned range wraps to zero and
                // returns -1 without consuming the stream.
                let index = game_engine::common::random_value::get_game_logic_random_value(
                    0,
                    count as i32 - 1,
                ) as usize;
                if count == 0 {
                    Some(None)
                } else {
                    let next_id = current.get_link(index)?;
                    let next = terrain.get_waypoint_by_id(next_id)?;
                    let loc = *next.get_location();
                    let pos = glam::Vec3::new(loc.x, loc.z, loc.y);
                    let extra = panic_waypoint_extra_distance(&terrain, next_id);
                    Some(Some((next_id, pos, extra, next.get_num_links() > 0)))
                }
            })();

            let Some(next) = next else {
                continue;
            };
            let Some((next_id, goal, extra, has_links)) = next else {
                // C++ getNextWaypoint assigns prior=current before returning
                // null for the terminal waypoint.
                panic.prior_waypoint_id = Some(panic.current_waypoint_id);
                let terrain_owner_handle = gamelogic::terrain::get_terrain_logic();
                let labels = terrain_owner_handle
                    .read()
                    .ok()
                    .and_then(|terrain| {
                        let waypoint = terrain.get_waypoint_by_id(panic.current_waypoint_id)?;
                        Some([
                            waypoint.get_path_label1().to_string(),
                            waypoint.get_path_label2().to_string(),
                            waypoint.get_path_label3().to_string(),
                        ])
                    })
                    .unwrap_or_default();
                if let Some(unit) = self.objects.get_mut(&id) {
                    unit.stamp_pending_waypoint_labels(labels);
                    unit.commit_completed_waypoint_labels();
                }
                self.finish_host_panic_movement(id);
                continue;
            };

            panic.prior_waypoint_id = Some(panic.current_waypoint_id);
            panic.current_waypoint_id = next_id;
            if self
                .objects
                .get(&id)
                .is_some_and(|unit| unit.wander_width_factor > 0.0)
            {
                let width = self
                    .objects
                    .get(&id)
                    .map_or(0.0, |unit| unit.wander_width_factor);
                panic.group_offset = leftover_wander_group_offset(width);
            }
            let (goal, append_goal, goal_layer) =
                self.host_prepare_panic_goal(goal, panic.group_offset);
            let actual_goal = goal;
            panic.append_goal_position = append_goal;
            panic.goal_layer = goal_layer;
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.path_extra_distance = extra;
                unit.adjust_destinations = !append_goal && !has_links;
                unit.panic_runtime = Some(panic.clone());
                if append_goal {
                    unit.set_allow_invalid_position(true);
                }
            }
            let _ = self.assign_unit_path(id, goal, &[]);
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_ai_state(AIState::Panic);
                unit.panic_runtime = Some(panic);
                unit.path_extra_distance = extra;
                unit.adjust_destinations = !append_goal && !has_links;
                unit.path_goal_position = Some(actual_goal);
                unit.requested_destination = Some(actual_goal);
                unit.is_panicking = true;
            }
        }
    }
}
