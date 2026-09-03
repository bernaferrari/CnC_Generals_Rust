//! AIUpdateInterface locomotor, path request, mood, and command helpers.

#![allow(unused_imports)]

use super::ai_core::UnitAIUpdate;
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{
    dual_world_registry_unavailable, get_unit_arc, with_unit_mut, with_unit_ref,
};
use super::types::*;

impl UnitAIUpdate {
    pub(super) fn get_preferred_height(&self) -> Option<Real> {
        let locomotor = get_unit_arc(self.unit_id).and_then(|unit| {
            unit.read()
                .ok()
                .and_then(|guard| guard.current_locomotor.as_ref().cloned())
        })?;
        locomotor.lock().ok().map(|loc| loc.preferred_height)
    }
    pub(super) fn is_allowed_to_adjust_destination(&self) -> bool {
        if let Some(chinook_ai) = self.chinook_ai.as_ref() {
            let invalid_allowed = get_unit_arc(self.unit_id)
                .and_then(|unit| {
                    let guard = unit.read().ok()?;
                    let locomotor = guard.current_locomotor.as_ref()?.clone();
                    drop(guard);
                    let loc_guard = locomotor.lock().ok()?;
                    Some(loc_guard.is_allowing_invalid_positions())
                })
                .unwrap_or(false);
            if invalid_allowed {
                return false;
            }
            chinook_ai.is_allowed_to_adjust_destination()
        } else {
            true
        }
    }
    pub(super) fn get_ai_free_to_exit(
        &self,
        exiter: &Object,
    ) -> crate::object::production::AIFreeToExitType {
        if let Some(chinook_ai) = self.chinook_ai.as_ref() {
            chinook_ai.get_ai_free_to_exit(exiter)
        } else {
            crate::object::production::AIFreeToExitType::FreeToExit
        }
    }
    pub(super) fn set_path_extra_distance(
        &mut self,
        distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.path_extra_distance = distance;
            }
        }
        Ok(())
    }
    pub(super) fn set_path_from_waypoint(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        group_offset: &Coord2D,
    ) -> Result<(), String> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let start_pos = unit
            .read()
            .map_err(|_| "unit lock poisoned".to_string())?
            .get_position();

        let terrain = crate::terrain::get_terrain_logic()
            .read()
            .map_err(|_| "terrain lock poisoned".to_string())?;

        self.destroy_path();

        // Build a chain following link 0 to match the classic path order.
        let mut visited = std::collections::HashSet::new();
        let mut waypoints = Vec::new();
        let mut path_coords = vec![start_pos];
        let mut current = waypoint.clone();
        for _ in 0..=WAYPOINT_PATH_LIMIT {
            if !visited.insert(current.id) {
                break;
            }
            let next_id = current.get_link(0);
            let mut adjusted = current.clone();
            adjusted.position.x += group_offset.x;
            adjusted.position.y += group_offset.y;
            adjusted.position.z =
                terrain.get_ground_height(adjusted.position.x, adjusted.position.y, None);
            if next_id.is_none() {
                adjusted.position = the_ai()
                    .read()
                    .ok()
                    .and_then(|ai| ai.pathfinder())
                    .and_then(|pathfinder| {
                        pathfinder
                            .read()
                            .ok()
                            .map(|pf| pf.snap_position(&adjusted.position))
                    })
                    .unwrap_or(adjusted.position);
            }
            path_coords.push(adjusted.position);
            waypoints.push(adjusted);

            let Some(next_id) = next_id else {
                break;
            };
            let Some(next) = terrain.get_waypoint_by_id(next_id) else {
                break;
            };
            current = crate::waypoint::Waypoint::from_terrain(next);
        }

        if waypoints.is_empty() {
            return Ok(());
        }

        let last = waypoints
            .last()
            .map(|waypoint| waypoint.position)
            .expect("waypoints is not empty");
        if let Ok(mut guard) = unit.write() {
            guard.target_position = Some(last);
            guard.movement_state = MovementState::Moving;
            guard.current_speed = 0.0;
            guard.path_index = 0;
            guard.path_following_state = None;
            guard.current_path = Some(
                path_coords
                    .iter()
                    .map(|pos| Coord2D::new(pos.x, pos.y))
                    .collect(),
            );
            guard.waypoint_queue.clear();
        }
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.waiting_for_path = false;
        self.queue_for_path_frame = 0;
        self.path_timestamp = TheGameLogic::get_frame();
        self.set_current_path_snapshot_from_coords(&path_coords);

        Ok(())
    }
    pub(super) fn is_waypoint_queue_empty(&self) -> bool {
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(guard) = unit.read() {
                return guard.waypoint_queue.is_empty();
            }
        }
        true
    }
    pub(super) fn do_pathfind(&mut self) {
        // C++ AIUpdateInterface::doPathfind — process queued path request.
        if let Err(e) = self.do_queued_pathfind_now() {
            log::trace!("UnitAIUpdate::do_pathfind: {e}");
        }
    }
    pub(super) fn is_waiting_for_path(&self) -> bool {
        if self.waiting_for_path {
            return true;
        }
        if self.queue_for_path_frame > TheGameLogic::get_frame() {
            return true;
        }
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(guard) = unit.read() {
                return guard
                    .path_following_state
                    .as_ref()
                    .map(|state| state.waiting_for_path)
                    .unwrap_or(false);
            }
        }
        false
    }
    pub(super) fn queue_waypoint(&mut self, pos: &Coord3D) {
        if (self.planning_waypoint_count as usize) < AI_UPDATE_MAX_WAYPOINTS {
            self.planning_waypoint_queue[self.planning_waypoint_count as usize] = *pos;
            self.planning_waypoint_count += 1;
            if let Some(unit) = get_unit_arc(self.unit_id) {
                if let Ok(mut guard) = unit.write() {
                    guard
                        .waypoint_queue
                        .push(Waypoint::new(0, *pos, String::new()));
                }
            }
        }
    }
    pub(super) fn clear_waypoint_queue(&mut self) {
        self.planning_waypoint_count = 0;
        self.planning_waypoint_index = 0;
        self.executing_waypoint_queue = false;
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.waypoint_queue.clear();
            }
        }
    }
    pub(super) fn execute_waypoint_queue(&mut self) {
        if self.planning_waypoint_count > 0 {
            self.planning_waypoint_index = 0;
            self.executing_waypoint_queue = true;
        }
        let first_pos = {
            let unit = match get_unit_arc(self.unit_id) {
                Some(u) => u,
                None => return,
            };
            let mut guard = match unit.write() {
                Ok(g) => g,
                Err(_) => return,
            };
            if guard.waypoint_queue.is_empty() {
                return;
            }
            let first = guard.waypoint_queue.remove(0);
            first.position
        };
        if let Err(e) = self.ai_move_to_position(&first_pos) {
            log::warn!("execute_waypoint_queue failed: {}", e);
        }
    }
    pub(super) fn append_goal_position_to_path(&mut self, goal: &Coord3D) -> Result<(), String> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;

        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                if let Some(active_path) = loc_guard.active_path.as_mut() {
                    active_path.append_waypoint(*goal);
                    self.append_current_path_snapshot_goal(goal);
                    return Ok(());
                }
            }
        }

        if let Some(path) = guard.current_path.as_mut() {
            path.push(Coord2D::new(goal.x, goal.y));
            self.append_current_path_snapshot_goal(goal);
            return Ok(());
        }

        Ok(())
    }
    pub(super) fn set_path_from_coords(&mut self, path: &[Coord3D]) -> Result<(), String> {
        let installed_path = self.path_with_cpp_final_node(path)?;
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;

        let last = *installed_path.last().unwrap();
        guard.target_position = Some(last);
        guard.movement_state = MovementState::Moving;
        guard.current_speed = 0.0;
        guard.path_index = 0;
        guard.path_following_state = None;
        guard.current_path = Some({
            let mut v = Vec::with_capacity(installed_path.len());
            v.extend(installed_path.iter().map(|pos| Coord2D::new(pos.x, pos.y)));
            v
        });
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.queue_for_path_frame = 0;
        self.path_timestamp = TheGameLogic::get_frame();
        self.movement_complete = false;
        self.locomotor_goal_type = 1;
        self.locomotor_goal_data = Coord3D::ZERO;

        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                loc_guard.clear_path();
            }
        }
        drop(guard);
        self.set_current_path_snapshot_from_coords(&installed_path);
        if self.is_final_goal && self.is_doing_ground_movement() {
            let layer = TheTerrainLogic::get()
                .map(|terrain| terrain.get_layer_for_destination(&last))
                .unwrap_or(crate::common::PathfindLayerEnum::Ground);
            self.update_goal_position(&last, layer)?;
        }

        Ok(())
    }
    pub(super) fn request_safe_path(&mut self, repulsor_id: ObjectID) -> Result<bool, String> {
        self.is_final_goal = false;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = false;
        self.is_safe_path = true;
        self.waiting_for_path = true;
        if repulsor_id != self.repulsor1 {
            self.repulsor2 = self.repulsor1;
        }
        self.repulsor1 = repulsor_id;
        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return Ok(false);
        }
        self.set_queue_for_path_time(0);
        self.path_timestamp = now;
        Ok(true)
    }
    pub(super) fn is_doing_ground_movement(&self) -> bool {
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            return jet_ai.is_doing_ground_movement();
        }
        let unit = get_unit_arc(self.unit_id);
        let Some(unit) = unit else {
            return true;
        };
        let Ok(guard) = unit.read() else {
            return true;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return true;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return true;
        };

        !matches!(
            loc_guard.get_appearance(),
            LocomotorAppearance::Hover | LocomotorAppearance::Thrust | LocomotorAppearance::Wings
        )
    }
    pub(super) fn is_allowed_to_move_away_from_unit(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_allowed_to_move_away_from_unit())
            .unwrap_or(true)
    }
    pub(super) fn get_sneaky_targeting_offset(&self, offset: &mut Coord3D) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.get_sneaky_targeting_offset(offset))
            .unwrap_or(false)
    }
    pub(super) fn is_temporarily_preventing_aim_success(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_temporarily_preventing_aim_success())
            .unwrap_or(false)
    }
    pub(super) fn add_targeter(&mut self, id: ObjectID, add: bool) {
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            jet_ai.add_targeter(id, add);
        }
    }
    pub(super) fn are_turrets_linked(&self) -> Bool {
        self.turrets_linked
    }
    pub(super) fn set_turret_target_object(
        &mut self,
        turret: TurretType,
        target_id: Option<ObjectID>,
        force_attacking: bool,
    ) {
        if let Some(machine) = self.ensure_turret_machine(turret) {
            if let Some(turret_ai) = machine.get_turret_ai() {
                if let Ok(mut guard) = turret_ai.lock() {
                    guard.set_current_target_with_force(target_id, force_attacking);
                }
            }
        }
    }
    pub(super) fn set_turret_target_position(&mut self, turret: TurretType, pos: &Coord3D) {
        if let Some(machine) = self.ensure_turret_machine(turret) {
            if let Some(turret_ai) = machine.get_turret_ai() {
                if let Ok(mut guard) = turret_ai.lock() {
                    guard.set_target_position(Some(*pos));
                }
            }
        }
    }
    pub(super) fn is_out_of_special_reload_ammo(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_out_of_special_reload_ammo())
            .unwrap_or(false)
    }
    pub(super) fn get_treat_as_aircraft_for_loco_dist_to_goal(&self) -> bool {
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            return jet_ai.get_treat_as_aircraft_for_loco_dist_to_goal();
        }
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return true;
        };
        let Ok(guard) = unit.read() else {
            return true;
        };

        let mut treat_as_aircraft = !self.is_doing_ground_movement();
        if guard.path_extra_distance > PATHFIND_CLOSE_ENOUGH {
            treat_as_aircraft = true;
        }
        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(loc_guard) = locomotor.lock() {
                if loc_guard.get_appearance() == LocomotorAppearance::Hover {
                    treat_as_aircraft = true;
                }
            }
        }
        treat_as_aircraft
    }
    pub(super) fn update_goal_position(
        &mut self,
        goal: &Coord3D,
        layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        let is_ground_movement = self.is_doing_ground_movement();
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;

        let owner_id = guard
            .base_arc()
            .read()
            .ok()
            .map(|obj| obj.get_id())
            .unwrap_or(INVALID_ID);
        let mut adjusted = *goal;
        let mut interacts_with_bridge_end = false;
        let terrain_layer = match layer {
            crate::common::PathfindLayerEnum::Invalid => crate::path::PathfindLayerEnum::Invalid,
            crate::common::PathfindLayerEnum::Ground => crate::path::PathfindLayerEnum::Ground,
            crate::common::PathfindLayerEnum::Wall => crate::path::PathfindLayerEnum::Wall,
            crate::common::PathfindLayerEnum::Tunnel
            | crate::common::PathfindLayerEnum::Water
            | crate::common::PathfindLayerEnum::Air
            | crate::common::PathfindLayerEnum::Last => crate::path::PathfindLayerEnum::Ground,
            other => crate::path::PathfindLayerEnum::from_u32(other as u32),
        };
        if let Ok(terrain) = crate::terrain::get_terrain_logic().read() {
            if layer == crate::common::PathfindLayerEnum::Wall {
                adjusted.z = crate::ai::the_ai()
                    .read()
                    .ok()
                    .and_then(|ai| ai.get_ai_data().read().ok().map(|data| data.wall_height))
                    .unwrap_or(adjusted.z);
            } else {
                adjusted.z =
                    terrain.get_layer_height(adjusted.x, adjusted.y, terrain_layer, None, true);
            }

            let mut dest_layer = layer;
            if layer != crate::common::PathfindLayerEnum::Ground {
                if let Ok(obj_guard) = guard.base_arc().read() {
                    interacts_with_bridge_end =
                        terrain.object_interacts_with_bridge_layer(&obj_guard, terrain_layer, true);
                }
            }
            if layer != crate::common::PathfindLayerEnum::Ground && !interacts_with_bridge_end {
                dest_layer = crate::common::PathfindLayerEnum::Ground;
            }
            if let Ok(mut obj_guard) = guard.base_arc().write() {
                obj_guard.set_destination_layer(dest_layer);
            }
        }

        guard.target_position = Some(adjusted);
        if let Some(state) = guard.path_following_state.as_mut() {
            state.goal_position = adjusted;
            state.path_goal_position = adjusted;
        }

        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                if let Some(active_path) = loc_guard.active_path.as_mut() {
                    active_path.set_last_waypoint(adjusted);
                }
            }
        }

        let is_immobile = guard
            .base_arc()
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Immobile))
            .unwrap_or(false);
        if is_immobile {
            return Ok(());
        }

        let path_layer = match layer {
            crate::common::PathfindLayerEnum::Ground => ClassicPathLayer::Ground,
            _ => ClassicPathLayer::Top,
        };
        let (radius, center_in_cell) = Self::compute_pathfind_radius_and_center(&guard);
        let new_cell = Self::compute_goal_cell(&adjusted, center_in_cell);
        let is_unmanned_heli = guard
            .base_arc()
            .read()
            .ok()
            .map(|obj| {
                obj.is_kind_of(KindOf::ProducedAtHelipad)
                    && obj.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned)
            })
            .unwrap_or(false);

        let ai_store = the_ai(); if let Ok(ai_lock) = ai_store.read() {
            if let Some(pathfinder) = ai_lock.pathfinder() {
                if let Ok(mut pf_guard) = pathfinder.write() {
                    if !is_ground_movement && !is_unmanned_heli {
                        self.update_aircraft_goal_cells(
                            &mut pf_guard,
                            owner_id,
                            new_cell,
                            radius,
                            center_in_cell,
                        );
                    } else {
                        self.update_ground_goal_cells(
                            &mut pf_guard,
                            owner_id,
                            new_cell,
                            path_layer,
                            radius,
                            center_in_cell,
                            interacts_with_bridge_end,
                        );
                    }
                }
            }
        }

        Ok(())
    }
    pub(super) fn adjust_destination(&mut self, goal: &mut Coord3D) -> bool {
        let unit = get_unit_arc(self.unit_id);
        let Some(unit) = unit else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        if guard.current_locomotor.is_none() {
            return false;
        }
        // C++ Pathfinder::adjustDestination performs the 400-cell spiral and
        // checkForAdjust path-existence gate. The former host route consulted
        // the legacy terrain-sampled grid and then fell back to
        // findClosestPath, which could report a nearby path as destination
        // adjustment success even when the requested adjustment itself failed.
        // Use the canonical Pathfinder wrapper so bridge/layer, footprint,
        // occupancy, and off-map validation remain in one implementation.
        let base_arc = guard.base_arc();
        let Some(base) = base_arc.read().ok() else {
            return false;
        };
        let ai_store = the_ai();let Some(pathfinder_arc) = ai_store.read().ok().and_then(|ai| ai.pathfinder()) else {
            return false;
        };
        let Some(pathfinder) = pathfinder_arc.read().ok() else {
            return false;
        };
        let mut candidate = *goal;
        if !pathfinder.adjust_destination(&base, &guard.locomotor_set, &mut candidate) {
            return false;
        }
        *goal = candidate;
        true
    }
    pub(super) fn set_adjusts_destination(&mut self, adjust: bool) {
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.path_adjusts_destination = adjust;
                if let Some(state) = guard.path_following_state.as_mut() {
                    state.adjusts_destination = adjust;
                }
            }
        }
    }
    pub(super) fn set_allow_invalid_position(
        &mut self,
        allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                loc_guard.set_allow_invalid_position(allow);
            }
        }
        Ok(())
    }
    pub(super) fn set_allow_chase(&mut self, allowed: bool) {
        self.allow_chase = allowed;
    }
    pub(super) fn set_locomotor_upgrade(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.locomotor_upgraded = enabled;
        if matches!(
            self.current_locomotor_set,
            LocomotorSetType::Normal | LocomotorSetType::NormalUpgraded
        ) {
            let _ = self.choose_locomotor_set(LocomotorSetType::Normal);
        }
        Ok(())
    }
    pub(super) fn choose_locomotor_set(
        &mut self,
        set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut target_set = set;
        if target_set == LocomotorSetType::Normal && self.locomotor_upgraded {
            target_set = LocomotorSetType::NormalUpgraded;
        }
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if let Some(desired) = jet_ai.desired_locomotor_set() {
                target_set = desired;
            }
        }

        if target_set == self.current_locomotor_set {
            return Ok(());
        }

        let Some(unit) = get_unit_arc(self.unit_id) else {
            return Ok(());
        };
        let Some(locomotors) = self.locomotor_sets.get(&target_set) else {
            return Ok(());
        };

        self.current_locomotor_set = target_set;

        let mut new_set = LocomotorSet::new();
        for locomotor_name in locomotors {
            if let Some(template) =
                crate::locomotor::LOCOMOTOR_STORE.get_template(locomotor_name.as_str())
            {
                let loco = Arc::new(Mutex::new(Locomotor::new(template)));
                new_set.add_locomotor(locomotor_name.as_str().to_string(), loco);
            } else {
                log::warn!("Locomotor template '{}' not found", locomotor_name.as_str());
            }
        }

        let mut guard = unit.write().map_err(|_| "unit lock poisoned")?;
        let prev_locomotor = guard.current_locomotor.as_ref().cloned();
        guard.locomotor_set = new_set;
        guard.current_locomotor = guard.locomotor_set.get_default_locomotor();

        if let (Some(prev), Some(current)) = (prev_locomotor, guard.current_locomotor.as_ref()) {
            if !Arc::ptr_eq(&prev, current) {
                if let Ok(mut loco_guard) = current.lock() {
                    loco_guard.set_precise_z_pos(false);
                    loco_guard.set_no_slow_down(false);
                    loco_guard.set_ultra_accurate(false);
                }
            }
        }

        Ok(())
    }
    pub(super) fn set_ultra_accurate(
        &mut self,
        ultra: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                loc_guard.set_ultra_accurate(ultra);
            }
        }
        Ok(())
    }
    pub(super) fn set_precise_z_pos(
        &mut self,
        precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                loc_guard.set_precise_z_pos(precise);
            }
        }
        Ok(())
    }
    pub(super) fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        get_unit_arc(self.unit_id).and_then(|unit| {
            unit.read()
                .ok()
                .and_then(|guard| guard.current_locomotor.as_ref().cloned())
        })
    }
    pub(super) fn get_locomotor_set_clone(&self) -> Option<crate::locomotor::LocomotorSet> {
        let unit = get_unit_arc(self.unit_id)?;
        let guard = unit.read().ok()?;
        if guard.locomotor_set.is_empty() {
            return None;
        }
        Some(guard.locomotor_set.clone())
    }
    pub(super) fn get_path_destination(&self) -> Option<Coord3D> {
        let unit = get_unit_arc(self.unit_id)?;
        let guard = unit.read().ok()?;
        if let Some(state) = guard.path_following_state.as_ref() {
            return Some(state.goal_position);
        }
        if let Some(path) = guard.current_path.as_ref() {
            let last = path.last()?;
            let z = guard
                .target_position
                .map(|pos| pos.z)
                .unwrap_or_else(|| guard.get_position().z);
            return Some(Coord3D::new(last.x, last.y, z));
        }
        None
    }
    pub(super) fn peek_cached_point_on_path(&self) -> Option<Coord3D> {
        let unit = get_unit_arc(self.unit_id)?;
        let guard = unit.read().ok()?;
        let pos = guard.get_position();
        if let Some(path) = guard.current_path.as_ref() {
            if !path.is_empty() {
                let waypoints: Vec<Coord3D> =
                    path.iter().map(|p| Coord3D::new(p.x, p.y, pos.z)).collect();
                return Some(
                    crate::ai::pathfind_complete::peek_point_on_path_from_waypoints(
                        &pos, &waypoints,
                    ),
                );
            }
        }
        self.get_path_destination()
    }

    pub(super) fn get_locomotor_distance_to_goal(&self) -> Real {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return 0.0;
        };
        let Ok(guard) = unit.read() else {
            return 0.0;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return 0.0;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return 0.0;
        };

        let obj_pos = guard.get_position();
        let is_projectile = guard
            .base_arc()
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Projectile))
            .unwrap_or(false);
        let mut treat_as_aircraft = guard.path_extra_distance > PATHFIND_CLOSE_ENOUGH
            || loc_guard.get_appearance() == LocomotorAppearance::Hover;
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            treat_as_aircraft = jet_ai.get_treat_as_aircraft_for_loco_dist_to_goal();
        }

        if let Some(active_path) = loc_guard.active_path.as_ref() {
            let last_waypoint = active_path.waypoints.last().copied();
            let goal_pos = last_waypoint
                .or(guard.target_position)
                .or_else(|| {
                    guard
                        .path_following_state
                        .as_ref()
                        .map(|state| state.goal_position)
                })
                .unwrap_or(obj_pos);

            if loc_guard.is_close_enough_dist_3d() || is_projectile {
                return (goal_pos - obj_pos).length();
            }

            if treat_as_aircraft {
                let delta = goal_pos - obj_pos;
                let dist = delta.length();
                let dist_sqr = delta.x * delta.x + delta.y * delta.y;
                if dist * dist > dist_sqr {
                    return dist_sqr.sqrt();
                }
                return dist;
            }

            let dist_remaining = active_path.distance_remaining().max(0.0);
            let dist = if let Some(current_target) = active_path.current_target() {
                let delta = current_target - obj_pos;
                (delta.x * delta.x + delta.y * delta.y).sqrt() + dist_remaining
            } else {
                dist_remaining
            };

            let dx = goal_pos.x - obj_pos.x;
            let dy = goal_pos.y - obj_pos.y;
            let dist_sqr = dx * dx + dy * dy;
            if dist < PATHFIND_CELL_SIZE_F || dist * dist < dist_sqr {
                return dist_sqr.sqrt();
            }
            return dist;
        }

        if let Some(state) = guard.path_following_state.as_ref() {
            let delta = state.goal_position - obj_pos;
            return (delta.x * delta.x + delta.y * delta.y).sqrt();
        }

        0.0
    }
    pub(super) fn get_speed(&self) -> f32 {
        get_unit_arc(self.unit_id)
            .and_then(|unit| unit.read().ok().map(|guard| guard.current_speed))
            .unwrap_or(0.0)
    }
    pub(super) fn get_last_command_source(&self) -> CommandSourceType {
        self.last_command_source
    }
    pub(super) fn set_last_command_source(&mut self, source: CommandSourceType) {
        self.last_command_source = source;
    }
    pub(super) fn get_current_command(&self) -> Option<crate::ai::AiCommandType> {
        self.current_command
    }
    pub(super) fn get_pending_command_type(&self) -> Option<crate::ai::AiCommandType> {
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if let Some(cmd) = jet_ai.pending_command_type() {
                return Some(cmd);
            }
        }
        self.pending_command
    }
    pub(super) fn purge_pending_command(&mut self) {
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            jet_ai.set_has_pending_command(false);
        }
        self.pending_command = None;
    }
    pub(super) fn is_taxiing_to_parking(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_taxiing_to_parking())
            .unwrap_or(false)
    }
    pub(super) fn is_reloading(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_reloading())
            .unwrap_or(false)
    }
    pub(super) fn is_clearing_mines(&self) -> bool {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let obj = guard.base_arc();
        let Ok(obj_guard) = obj.read() else {
            return false;
        };
        if !obj_guard.test_status(ObjectStatusTypes::OBJECT_STATUS_IS_ATTACKING) {
            return false;
        }
        let Some((weapon, _slot)) = obj_guard.get_current_weapon() else {
            return false;
        };
        (weapon.get_anti_mask() & WeaponAntiMask::MINE) != 0
    }
    pub(super) fn is_takeoff_or_landing_in_progress(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_takeoff_or_landing_in_progress())
            .unwrap_or(false)
    }
    pub(super) fn get_current_state_id(&self) -> Option<u32> {
        self.ai_state_machine.as_ref().and_then(|machine| {
            machine
                .lock()
                .ok()
                .and_then(|guard| guard.get_current_state_id())
        })
    }
    pub(super) fn get_parking_offset(&self) -> Real {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.parking_offset())
            .unwrap_or(0.0)
    }
    pub(super) fn keeps_parking_space_when_airborne(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.keeps_parking_space_when_airborne())
            .unwrap_or(true)
    }
    pub(super) fn get_desired_speed(&self) -> Real {
        self.desired_speed
    }
    pub(super) fn set_desired_speed(&mut self, speed: Real) {
        self.desired_speed = speed;
    }
    pub(super) fn is_in_rappel_state(&self) -> bool {
        self.rappel_state.is_some()
    }
    pub(super) fn is_doing_combat_drop(&self) -> bool {
        self.chinook_ai
            .as_ref()
            .map(|ai| ai.is_doing_combat_drop())
            .unwrap_or(false)
    }
    pub(super) fn is_aircraft_that_adjusts_destination(&self) -> bool {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return false;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return false;
        };
        matches!(
            loc_guard.get_appearance(),
            LocomotorAppearance::Hover | LocomotorAppearance::Wings
        )
    }
    pub(super) fn is_moving_away_from(&self, obj_id: ObjectID) -> bool {
        let is_temp_move_out = self
            .ai_state_machine
            .as_ref()
            .and_then(|machine| machine.lock().ok())
            .map(|guard| guard.get_temporary_state() == Some(AIStateType::MoveOutOfTheWay as u32))
            .unwrap_or(false);
        if !is_temp_move_out {
            return false;
        }
        self.move_out_of_way_1 == obj_id || self.move_out_of_way_2 == obj_id
    }
    pub(super) fn set_ignore_collision_time(&mut self, duration_frames: UnsignedInt) {
        self.ignore_collisions_until = TheGameLogic::get_frame().saturating_add(duration_frames);
    }
    pub(super) fn get_ignore_collisions_until(&self) -> UnsignedInt {
        self.ignore_collisions_until
    }
    pub(super) fn set_queue_for_path_time(&mut self, frames: UnsignedInt) {
        self.queue_for_path_frame = if frames == 0 {
            0
        } else {
            TheGameLogic::get_frame().saturating_add(frames)
        };
    }
    pub(super) fn ignore_obstacle(
        &mut self,
        obj_id: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.ignore_obstacle_id = obj_id.unwrap_or(INVALID_ID);
        Ok(())
    }
    pub(super) fn ignore_obstacle_id(
        &mut self,
        id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.ignore_obstacle_id = id;
        Ok(())
    }
    pub(super) fn get_ignored_obstacle_id(&self) -> ObjectID {
        self.ignore_obstacle_id
    }
    pub(super) fn is_ai_in_dead_state(&self) -> bool {
        self.ai_dead
    }
    pub(super) fn mark_as_dead(&mut self) {
        self.ai_dead = true;
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(unit_guard) = unit.read() {
                if let Ok(mut object_guard) = unit_guard.base_arc().write() {
                    object_guard.set_effectively_dead(true);
                }
            }
        }
        self.wake_up_now();
    }
    pub(super) fn set_is_recruitable(&mut self, recruitable: Bool) {
        self.is_recruitable = recruitable;
    }
    pub(super) fn get_goal_object_id(&self) -> ObjectID {
        let Some(machine) = self.ai_state_machine.as_ref() else {
            return INVALID_ID;
        };
        let Ok(guard) = machine.lock() else {
            return INVALID_ID;
        };
        guard.get_goal_object_id()
    }
    pub(super) fn set_goal_object(&mut self, obj_id: Option<ObjectID>) {
        let Some(machine) = self.ai_state_machine.as_ref() else {
            return;
        };
        let Ok(mut guard) = machine.lock() else {
            return;
        };
        let was_locked = guard.is_locked();
        guard.unlock();
        guard.set_goal_object(obj_id.unwrap_or(INVALID_ID));
        if was_locked {
            guard.lock();
        }
    }
    pub(super) fn get_goal_position(&self) -> Option<Coord3D> {
        let machine = self.ai_state_machine.as_ref()?;
        let guard = machine.lock().ok()?;
        guard.get_goal_position()
    }
    pub(super) fn set_goal_position(&mut self, pos: Option<Coord3D>) {
        let Some(pos) = pos else {
            return;
        };
        let Some(machine) = self.ai_state_machine.as_ref() else {
            return;
        };
        let Ok(mut guard) = machine.lock() else {
            return;
        };
        guard.set_goal_position(pos);
    }
    /// C++ `AIUpdateInterface::joinTeam` (AIUpdate.cpp).
    ///
    /// After `clear()`, C++ `getCurrentStateID()` is `INVALID_STATE_ID` (NULL
    /// current state). `setState(INVALID)` then falls through to the default
    /// state. Port that literally — C++ does not read the teammate's state id.
    pub(super) fn join_team(&mut self) {
        // Wave 258: empty dual-world → no factory object walks.
        if dual_world_registry_unavailable() {
            panic!("dual-world registry unavailable in test helper");
        }

        if self.is_ai_in_dead_state() {
            return;
        }
        let Some(unit_arc) = get_unit_arc(self.unit_id) else {
            return;
        };
        let (mobile, self_id, team) = {
            let Ok(g) = unit_arc.read() else {
                return;
            };
            let self_id = g.get_id();
            let base = g.base_arc();
            let Ok(obj) = base.read() else {
                return;
            };
            (obj.is_mobile(), self_id, obj.get_team())
        };
        if !mobile {
            return;
        }

        let _ = self.choose_locomotor_set(LocomotorSetType::Normal);

        // getStateMachine()->clear(); setGoalWaypoint(NULL);
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                guard.clear();
                guard.set_goal_waypoint(None);
            }
        }

        let mut other_pos = None;
        let mut other_idle = false;
        let mut other_goal_id = INVALID_ID;
        let mut other_goal_pos: Option<Coord3D> = None;
        let mut found_other = false;

        if let Some(team_arc) = team {
            let members = team_arc
                .read()
                .ok()
                .map(|tg| tg.get_members().to_vec())
                .unwrap_or_default();
            for mid in members {
                if mid == self_id {
                    continue;
                }
                let Some((pos, ai)) = crate::object::registry::OBJECT_REGISTRY
                    .with_object(mid, |og| {
                        let Some(oai) = og.get_ai_update_interface() else {
                            return None;
                        };
                        if og.is_disabled_by_type(crate::common::types::DisabledType::Held) {
                            return None;
                        }
                        Some((*og.get_position(), oai))
                    })
                    .flatten()
                else {
                    continue;
                };
                other_pos = Some(pos);
                if let Ok(aig) = ai.try_lock() {
                    other_idle = aig.is_idle();
                    other_goal_id = aig.get_goal_object_id();
                    other_goal_pos = aig.get_goal_position();
                }
                found_other = true;
                break;
            }
        }

        if !found_other {
            return;
        }
        let Some(pos) = other_pos else {
            return;
        };

        if other_idle {
            self.last_command_source = CommandSourceType::FromAi;
            let _ = self.ai_move_to_position(&pos);
            return;
        }

        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                if other_goal_id != INVALID_ID {
                    guard.set_goal_object(other_goal_id);
                } else if let Some(gp) = other_goal_pos {
                    guard.set_goal_position(gp);
                }
            }
        }

        // C++ after clear: getCurrentStateID() == INVALID_STATE_ID → default state.
        self.last_command_source = CommandSourceType::FromAi;
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                let _ = guard.set_state(crate::state_machine::INVALID_STATE_ID);
            }
        }
    }
    pub(super) fn is_path_available(&self, destination: &Coord3D) -> bool {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let ai_store = the_ai();let Some(ai) = ai_store.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        let pos = guard.get_position();
        let ignore = if self.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.ignore_obstacle_id)
        };
        pf_guard.client_safe_quick_does_path_exist_with_ignore(
            &guard.locomotor_set,
            &pos,
            destination,
            ignore,
        )
    }
    pub(super) fn request_path(
        &mut self,
        destination: &Coord3D,
        _is_final_goal: bool,
    ) -> Result<(), String> {
        self.requested_destination = *destination;
        self.is_final_goal = _is_final_goal;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = false;
        self.is_safe_path = false;
        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        let _ = self.ignore_obstacle(None);
        if self.can_compute_quick_path() {
            self.compute_quick_path(destination);
            return Ok(());
        }
        self.retry_path = false;
        if self.should_force_direct_path_for_off_map_start(destination)
            && self.install_direct_path_from_current_position(destination)
        {
            return Ok(());
        }
        if (self.get_current_state_id() == Some(u32::from(AIStateType::FollowExitProductionPath))
            || self.current_command == Some(crate::ai::AiCommandType::FollowExitProductionPath))
            && self.can_path_through_units
            && self.install_direct_path_from_current_position(destination)
        {
            let _ = self.set_can_path_through_units(false);
            return Ok(());
        }
        if self.should_use_direct_path_for_line_passable_non_final_goal(destination)
            && self.install_direct_path_from_current_position(destination)
        {
            return Ok(());
        }
        self.waiting_for_path = true;
        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
            if self.blocked_and_stuck {
                self.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
                self.blocked_frames = 0;
                self.is_blocked = false;
                self.blocked_and_stuck = false;
            }
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*destination);
        self.path_timestamp = now;
        Ok(())
    }
    pub(super) fn request_attack_path(
        &mut self,
        victim_id: ObjectID,
        victim_pos: &Coord3D,
    ) -> Result<(), String> {
        // Wave 258: empty dual-world → Ok(()).

        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        self.requested_destination = *victim_pos;
        self.requested_victim_id = victim_id;
        self.is_attack_path = true;
        self.is_approach_path = false;
        self.is_safe_path = false;
        self.waiting_for_path = true;
        let victim = get_legacy_object(victim_id);
        let _ = self.set_goal_object(
            victim
                .as_ref()
                .and_then(|a| a.read().ok().map(|g| g.get_id())),
        );
        let _ = self.ignore_obstacle(
            victim
                .as_ref()
                .and_then(|a| a.read().ok().map(|g| g.get_id())),
        );
        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            self.set_locomotor_goal_none();
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*victim_pos);
        self.path_timestamp = now;
        Ok(())
    }
    pub(super) fn request_approach_path(&mut self, destination: &Coord3D) -> Result<(), String> {
        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        self.requested_destination = *destination;
        self.is_final_goal = true;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = true;
        self.is_safe_path = false;
        self.waiting_for_path = true;
        let _ = self.ignore_obstacle(None);
        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*destination);
        self.path_timestamp = now;
        Ok(())
    }
    pub(super) fn can_compute_quick_path(&self) -> bool {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let locomotor = guard
            .current_locomotor
            .as_ref()
            .cloned()
            .or_else(|| guard.locomotor_set.get_default_locomotor());
        let Some(locomotor) = locomotor else {
            return false;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return false;
        };
        let surfaces = loc_guard.get_legal_surfaces();
        drop(loc_guard);
        drop(guard);
        let land_bound = (surfaces & SURFACE_AIR) == 0;
        if land_bound {
            return false;
        }
        !self.is_doing_ground_movement()
    }
    pub(super) fn compute_quick_path(&mut self, destination: &Coord3D) -> bool {
        if !self.can_compute_quick_path() {
            return false;
        }
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(guard) = unit.read() {
                if let Some(path) = guard.current_path.as_ref() {
                    if let Some(last) = path.last() {
                        let dx = destination.x - last.x;
                        let dy = destination.y - last.y;
                        let path_goal_z = guard
                            .target_position
                            .unwrap_or_else(|| guard.get_position())
                            .z;
                        let dz = destination.z - path_goal_z;
                        if dx * dx + dy * dy + dz * dz < 0.25 {
                            return true;
                        }
                    }
                }
            }
        }

        self.install_direct_path_from_current_position(destination)
    }
    pub(super) fn is_quick_path_available(&self, destination: &Coord3D) -> bool {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let ai_store = the_ai();let Some(ai) = ai_store.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        let pos = guard.get_position();
        pf_guard.client_safe_quick_does_path_exist_for_ui(&guard.locomotor_set, &pos, destination)
    }
    pub(super) fn is_valid_locomotor_position(&self, pos: &Coord3D) -> bool {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let ai_store = the_ai();let Some(ai) = ai_store.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        pf_guard.valid_movement_position(
            &guard.locomotor_set,
            guard.get_crusher_level() > 0,
            pos,
            if self.ignore_obstacle_id == INVALID_ID {
                None
            } else {
                Some(self.ignore_obstacle_id)
            },
        )
    }
    pub(super) fn need_to_rotate(&self) -> bool {
        if self.is_waiting_for_path() {
            return true;
        }
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return false;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return false;
        };
        if loc_guard.template.wander_width_factor > 0.0 {
            return false;
        }
        let Some(active_path) = loc_guard.active_path.as_ref() else {
            return false;
        };
        let Some(target) = active_path.current_target() else {
            return false;
        };
        let pos = guard.get_position();
        let mut path_point = target;
        if active_path.current_waypoint + 1 < active_path.waypoints.len() {
            let start = active_path.waypoints[active_path.current_waypoint];
            let end = active_path.waypoints[active_path.current_waypoint + 1];
            let seg = Coord3D::new(end.x - start.x, end.y - start.y, 0.0);
            let seg_len_sqr = seg.x * seg.x + seg.y * seg.y;
            if seg_len_sqr > f32::EPSILON {
                let to_pos = Coord3D::new(pos.x - start.x, pos.y - start.y, 0.0);
                let mut t = (to_pos.x * seg.x + to_pos.y * seg.y) / seg_len_sqr;
                if t < 0.0 {
                    t = 0.0;
                } else if t > 1.0 {
                    t = 1.0;
                }
                path_point = Coord3D::new(start.x + seg.x * t, start.y + seg.y * t, pos.z);
            }
        }
        let delta = path_point - pos;
        if delta.length_squared() < f32::EPSILON {
            return false;
        }
        let desired_angle = delta.y.atan2(delta.x);
        let current_angle = guard.get_orientation();
        let mut delta_angle = desired_angle - current_angle;
        while delta_angle > std::f32::consts::PI {
            delta_angle -= std::f32::consts::PI * 2.0;
        }
        while delta_angle < -std::f32::consts::PI {
            delta_angle += std::f32::consts::PI * 2.0;
        }
        delta_angle.abs() > (std::f32::consts::PI / 30.0)
    }
    pub(super) fn get_cur_locomotor_set_type(&self) -> LocomotorSetType {
        self.current_locomotor_set
    }
    pub(super) fn has_locomotor_for_surface(
        &self,
        surface: crate::common::LocomotorSurfaceTypeMask,
    ) -> bool {
        let Some(entries) = self.locomotor_sets.get(&self.current_locomotor_set) else {
            return false;
        };
        for name in entries {
            if let Some(template) = crate::locomotor::LOCOMOTOR_STORE.get_template(name.as_str()) {
                if (template.surfaces & surface) != 0 {
                    return true;
                }
            }
        }
        false
    }
    pub(super) fn get_cur_locomotor_speed(&self) -> Real {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return 0.0;
        };
        let Ok(guard) = unit.read() else {
            return 0.0;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return 0.0;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return 0.0;
        };
        let body_state = guard
            .base_arc()
            .read()
            .ok()
            .and_then(|obj| obj.get_body_module())
            .and_then(|body| {
                body.lock()
                    .ok()
                    .map(|b| to_locomotor_body_damage_type(b.get_damage_state()))
            })
            .unwrap_or(BodyDamageType::Pristine);
        loc_guard.get_max_speed_for_condition(body_state)
    }
    pub(super) fn get_cur_max_blocked_speed(&self) -> Real {
        self.cur_max_blocked_speed
    }
    pub(super) fn set_cur_max_blocked_speed(&mut self, speed: Real) {
        self.cur_max_blocked_speed = speed;
    }
    pub(super) fn set_locomotor_goal_none(&mut self) {
        self.locomotor_goal_type = 0;
        self.locomotor_goal_data = Coord3D::ZERO;
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if jet_ai.is_takeoff_or_landing_in_progress()
                && jet_ai.allow_air_loco()
                && !jet_ai.allow_circling()
            {
                if let Some(unit) = get_unit_arc(self.unit_id) {
                    if let Ok(guard) = unit.read() {
                        let (dir_x, dir_y) = guard.get_unit_direction_vector_2d();
                        let mut desired = guard.get_position();
                        desired.x += dir_x * 1000.0;
                        desired.y += dir_y * 1000.0;
                        let _ = self.set_movement_target(&desired);
                        return;
                    }
                }
            }
        }

        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.stop_movement();
            }
        }
    }
    pub(super) fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        self.locomotor_goal_type = 3;
        self.locomotor_goal_data = Coord3D::new(angle, 0.0, 0.0);
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                let _ = guard.set_orientation(angle);
            }
        }
    }
    pub(super) fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        self.locomotor_goal_type = 2;
        self.locomotor_goal_data = pos;
        let _ = self.set_movement_target(&pos);
    }
    pub(super) fn friend_ending_move(&mut self) {
        self.queue_for_path_frame = 0;
        self.ignore_obstacle_id = INVALID_ID;
        self.movement_complete = true;
        self.locomotor_goal_type = 0;
        self.locomotor_goal_data = Coord3D::ZERO;
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.stop_movement();
            }
        }
    }
    pub(super) fn friend_starting_move(&mut self) {
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.movement_complete = false;
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.movement_state = MovementState::Moving;
                if let Some(loco) = guard.current_locomotor.as_ref() {
                    if let Ok(mut loco_guard) = loco.lock() {
                        loco_guard.start_move();
                    }
                }
            }
        }
    }
    pub(super) fn evaluate_morale_bonus(&mut self) {
        let Some(unit_arc) = get_unit_arc(self.unit_id) else {
            return;
        };
        let base_object = match unit_arc.read() {
            Ok(guard) => guard.base_arc(),
            Err(_) => return,
        };
        let Ok(mut obj_guard) = base_object.write() else {
            return;
        };

        let mut nationalism = false;
        let mut fanaticism = false;
        if let Some(player) = obj_guard.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                if let Ok(center) = get_upgrade_center().read() {
                    if let Some(upgrade) = center.find_upgrade("Upgrade_Nationalism") {
                        if player_guard.has_upgrade_complete(&upgrade) {
                            nationalism = true;
                        }
                    }
                    if let Some(upgrade) = center.find_upgrade("Upgrade_Fanaticism") {
                        if player_guard.has_upgrade_complete(&upgrade) {
                            fanaticism = true;
                        }
                    }
                }
            }
        }

        let mut horde = false;
        let mut allow_nationalism = true;
        obj_guard.with_horde_update_interface(|hui| {
            if hui.is_in_horde() {
                horde = true;
                if !hui.is_allowed_nationalism() {
                    allow_nationalism = false;
                }
            }
        });

        if !allow_nationalism {
            nationalism = false;
            fanaticism = false;
        }

        let demoralized = self.demoralized_frames_left > 0;

        if !demoralized {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Demoralized);
        }

        if horde {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Horde);
        } else {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Horde);
        }

        if nationalism {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            if fanaticism {
                obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
            } else {
                obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
            }
        } else {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
        }

        if demoralized {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Demoralized);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Horde);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);

            if !obj_guard.is_kind_of(KindOf::PortableStructure) {
                if let Some(drawable) = obj_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        draw_guard.set_terrain_decal(TerrainDecalType::Demoralized);
                    }
                }
            }
        }
    }
    pub(super) fn set_surrendered(&mut self, to_object_id: Option<ObjectID>, surrendered: bool) {
        // Wave 258: empty dual-world → no factory object walks.

        if dual_world_registry_unavailable() {
            panic!("dual-world registry unavailable in test helper");
        }

        if surrendered {
            self.surrendered_frames_left = self.surrender_duration_frames;
            self.surrendered_player_index = to_object_id.and_then(|id| {
                let obj = crate::helpers::TheGameLogic::find_object_by_id(id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))?;
                let guard = obj.read().ok()?;
                guard
                    .get_controlling_player_id()
                    .map(|idx| idx as PlayerIndex)
            });
        } else {
            self.surrendered_frames_left = 0;
            self.surrendered_player_index = None;
        }
    }
    pub(super) fn transfer_attack(&mut self, from_id: ObjectID, to_id: ObjectID) {
        use crate::helpers::TheGameLogic;

        let new_target = TheGameLogic::find_object_by_id(to_id);

        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                if guard.attack_target == Some(from_id) {
                    guard.attack_target = Some(to_id);
                }
            }
        }

        if self.get_goal_object_id() == from_id {
            self.set_goal_object(
                new_target
                    .as_ref()
                    .and_then(|a| a.read().ok().map(|g| g.get_id())),
            );
        }

        for turret in [TurretType::Primary, TurretType::Secondary] {
            let turret_ai = match turret {
                TurretType::Primary => self
                    .turret_primary_machine
                    .as_ref()
                    .and_then(|m| m.get_turret_ai()),
                TurretType::Secondary => self
                    .turret_secondary_machine
                    .as_ref()
                    .and_then(|m| m.get_turret_ai()),
                _ => continue,
            };
            let Some(turret_ai) = turret_ai else {
                continue;
            };
            let needs_transfer = if let Ok(ai_guard) = turret_ai.lock() {
                if let Some(target_obj) = ai_guard.get_current_target() {
                    if let Ok(tg) = target_obj.read() {
                        tg.get_id() == from_id
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
            if needs_transfer {
                self.set_turret_target_object(
                    turret,
                    new_target
                        .as_ref()
                        .and_then(|a| a.read().ok().map(|g| g.get_id())),
                    true,
                );
            }
        }
    }
    pub(super) fn is_surrendered(&self) -> bool {
        self.surrendered_frames_left > 0
    }
    pub(super) fn get_surrendered_player_index(&self) -> Option<PlayerIndex> {
        self.surrendered_player_index
    }
    pub(super) fn ai_move_to_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.give_move_order(*pos, Vec::new(), false, false)?;
        Ok(())
    }
    pub(super) fn ai_idle(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(state_machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut machine) = state_machine.lock() {
                machine.clear();
                let params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                );
                let _ = machine.ai_do_command(&params);
                return Ok(());
            }
        }
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.stop_movement();
            }
        }
        Ok(())
    }
    pub(super) fn ai_busy(
        &mut self,
        cmd_source: crate::ai::CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Busy, cmd_source);
        self.execute_command(&params)
    }
    pub(super) fn ai_attack_object(
        &mut self,
        target_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.give_attack_order(target_id, true, false)?;
        Ok(())
    }
    pub(super) fn ai_guard_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        self.push_guard_target_type(GuardTargetType::Location);
        self.location_to_guard = *pos;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.current_order = Some(UnitOrder::Guard {
            position: *pos,
            area_radius: guard.engagement_range,
        });
        guard.order_queue.clear();
        Ok(())
    }
    pub(super) fn get_crate_id(&self) -> ObjectID {
        self.crate_created
            .lock()
            .map(|id| *id)
            .unwrap_or(crate::common::INVALID_ID)
    }
    pub(super) fn get_current_victim(&self) -> Option<ObjectID> {
        let unit = get_unit_arc(self.unit_id)?;
        let guard = unit.read().ok()?;
        guard.attack_target
    }
    pub(super) fn set_current_victim(&mut self, victim: Option<ObjectID>) {
        let unit = match get_unit_arc(self.unit_id) {
            Some(u) => u,
            None => return,
        };
        let mut guard = match unit.write() {
            Ok(g) => g,
            Err(_) => return,
        };

        if victim.is_none() && guard.attack_target.is_some() {
            let old_id = guard.attack_target.unwrap();
            if let Some(old_victim) = crate::helpers::TheGameLogic::find_object_by_id(old_id) {
                if let Ok(old_guard) = old_victim.read() {
                    if let Some(ai) = old_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            if let Ok(self_guard) = unit.read() {
                                ai_guard.add_targeter(self_guard.get_id(), false);
                            }
                        }
                    }
                }
            }
        }

        guard.attack_target = victim;
    }
    pub(super) fn check_for_crate_to_pickup_id(&self) -> ObjectID {
        let Ok(mut guard) = self.crate_created.lock() else {
            return INVALID_ID;
        };
        if *guard == crate::common::INVALID_ID {
            return INVALID_ID;
        }
        // C++ clears m_crateCreated before the lookup, so the processed marker
        // does not yield a crate object from this path.
        *guard = crate::common::INVALID_ID;
        INVALID_ID
    }
    pub(super) fn get_next_mood_target_id(
        &mut self,
        use_existing_target: bool,
        _ignore_attacked: bool,
    ) -> ObjectID {
        // Wave 258: empty dual-world → invalid id.

        if dual_world_registry_unavailable() {
            return INVALID_ID;
        }

        let Some(unit) = get_unit_arc(self.unit_id) else {
            return INVALID_ID;
        };
        let Ok(guard) = unit.read() else {
            return INVALID_ID;
        };
        if !guard.can_auto_acquire_now() {
            return INVALID_ID;
        }

        let max_range = guard.engagement_range;
        if use_existing_target {
            if let Some(existing_id) = guard.attack_target {
                if let Some(existing_arc) =
                    crate::object::registry::OBJECT_REGISTRY.get_object(existing_id)
                {
                    if let Ok(existing_guard) = existing_arc.read() {
                        let relationship = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|base| base.relationship_to(&existing_guard))
                            .unwrap_or(Relationship::Neutral);
                        if relationship == Relationship::Enemies {
                            let target_pos = *existing_guard.get_position();
                            let self_pos = guard.get_position();
                            let dx = target_pos.x - self_pos.x;
                            let dy = target_pos.y - self_pos.y;
                            let dist = (dx * dx + dy * dy).sqrt();
                            if dist <= max_range && guard.can_detect_target(&existing_guard, dist) {
                                return existing_id;
                            }
                        }
                    }
                }
            }
        }

        let ai_store = the_ai();let Ok(ai) = ai_store.read() else {
            return INVALID_ID;
        };
        let ai_data = ai.get_ai_data();
        let Ok(ai_data_guard) = ai_data.read() else {
            return INVALID_ID;
        };

        let mut qualifiers = search_qualifiers::CAN_ATTACK;
        if ai_data_guard.attack_uses_line_of_sight {
            qualifiers |= search_qualifiers::CAN_SEE;
        }
        if ai_data_guard.attack_ignore_insignificant_buildings {
            qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
        }
        if guard.auto_acquire_attack_buildings {
            qualifiers |= search_qualifiers::ATTACK_BUILDINGS;
        }

        ai.find_closest_enemy(guard.get_id(), max_range, qualifiers, None, None)
            .ok()
            .flatten()
            .unwrap_or(INVALID_ID)
    }
    pub(super) fn get_next_mood_check_time(&self) -> u32 {
        let unit = get_unit_arc(self.unit_id);
        let Some(unit) = unit else {
            return TheGameLogic::get_frame();
        };
        let Ok(guard) = unit.read() else {
            return TheGameLogic::get_frame();
        };
        let interval = guard.mood_attack_check_rate_frames.max(1);
        guard.last_target_scan_frame.saturating_add(interval)
    }
    pub(super) fn reset_next_mood_check_time(&mut self) {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return;
        };
        let Ok(mut guard) = unit.write() else {
            return;
        };
        guard.last_target_scan_frame = TheGameLogic::get_frame();
    }
    pub(super) fn set_next_mood_check_time(&mut self, frame: u32) {
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return;
        };
        let Ok(mut guard) = unit.write() else {
            return;
        };
        let interval = guard.mood_attack_check_rate_frames.max(1);
        guard.last_target_scan_frame = frame.saturating_sub(interval);
    }
    pub(super) fn get_mood_matrix_value(&self) -> u32 {
        if self.ai_state_machine.is_none() {
            return 0;
        }

        let Some(unit_arc) = get_unit_arc(self.unit_id) else {
            return 0;
        };
        let Ok(unit_guard) = unit_arc.read() else {
            return 0;
        };
        let owner_arc = unit_guard.base_arc();
        let Ok(owner_guard) = owner_arc.read() else {
            return 0;
        };
        let Some(player_arc) = owner_guard.get_controlling_player() else {
            return 0;
        };
        let Ok(player_guard) = player_arc.read() else {
            return 0;
        };

        let mut value = 0u32;
        if player_guard.get_player_type() == crate::player::PlayerType::Human {
            value |= mood_matrix_parameters::CONTROLLER_PLAYER;
        } else {
            value |= mood_matrix_parameters::CONTROLLER_AI;
            value |= match self.attitude {
                AIAttitudeType::Passive => mood_matrix_parameters::MOOD_PASSIVE,
                AIAttitudeType::Defensive => mood_matrix_parameters::MOOD_ALERT,
                AIAttitudeType::Aggressive => mood_matrix_parameters::MOOD_AGGRESSIVE,
                AIAttitudeType::Sleep => mood_matrix_parameters::MOOD_SLEEP,
                AIAttitudeType::Normal => mood_matrix_parameters::MOOD_NORMAL,
            };
        }

        let is_air = unit_guard
            .get_locomotor_surface_mask()
            .map(|surfaces| (surfaces & SURFACE_AIR) != 0)
            .unwrap_or(false);
        if is_air {
            value |= mood_matrix_parameters::UNITTYPE_AIR;
        } else if self.turret_primary_machine.is_some() {
            value |= mood_matrix_parameters::UNITTYPE_TURRETED;
        } else {
            value |= mood_matrix_parameters::UNITTYPE_NON_TURRETED;
        }

        value
    }
    pub(super) fn get_mood_matrix_action_adjustment(&mut self, action: MoodMatrixAction) -> u32 {
        let Some(unit_arc) = get_unit_arc(self.unit_id) else {
            return mood_matrix_adjustment::ACTION_OK;
        };
        let Ok(unit_guard) = unit_arc.read() else {
            return mood_matrix_adjustment::ACTION_OK;
        };
        let owner_arc = unit_guard.base_arc();
        let Ok(owner_guard) = owner_arc.read() else {
            return mood_matrix_adjustment::ACTION_OK;
        };

        // Mirror C++ mob-member special case that ignores mood conversions.
        if owner_guard.is_kind_of(KindOf::Infantry) && owner_guard.is_kind_of(KindOf::IgnoredInGui)
        {
            return mood_matrix_adjustment::ACTION_OK;
        }

        let mood_matrix = self.get_mood_matrix_value();
        if (mood_matrix & mood_matrix_parameters::CONTROLLER_PLAYER) != 0 {
            return mood_matrix_adjustment::ACTION_OK;
        }

        match action {
            MoodMatrixAction::Idle => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                mood_matrix_parameters::MOOD_PASSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_WAIT_FOR_ATTACK
                }
                mood_matrix_parameters::MOOD_ALERT => {
                    mood_matrix_adjustment::ACTION_OK | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                }
                mood_matrix_parameters::MOOD_AGGRESSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::Move => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_TO_IDLE
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                mood_matrix_parameters::MOOD_PASSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_WAIT_FOR_ATTACK
                }
                mood_matrix_parameters::MOOD_ALERT => {
                    mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE
                        | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                }
                mood_matrix_parameters::MOOD_AGGRESSIVE => {
                    mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE
                        | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::Attack => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_TO_IDLE
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::AttackMove => {
                match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                    mood_matrix_parameters::MOOD_SLEEP => {
                        mood_matrix_adjustment::ACTION_TO_IDLE
                            | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                    }
                    mood_matrix_parameters::MOOD_ALERT => {
                        mood_matrix_adjustment::ACTION_OK
                            | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                    }
                    mood_matrix_parameters::MOOD_AGGRESSIVE => {
                        mood_matrix_adjustment::ACTION_OK
                            | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                    }
                    _ => mood_matrix_adjustment::ACTION_OK,
                }
            }
        }
    }
    pub(super) fn notify_fired(&mut self) {}
    pub(super) fn notify_new_victim_chosen(&mut self, victim: ObjectID) {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_object(victim);
            }
        }
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut unit_guard) = unit.write() {
                unit_guard.attack_target = Some(victim);
            }
        }
    }
    pub(super) fn is_weapon_slot_ok_to_fire(&self, _wslot: WeaponSlotType) -> Bool {
        if self.turrets_linked {
            return true;
        }

        let has_primary = self.turret_primary_machine.is_some();
        let has_secondary = self.turret_secondary_machine.is_some();
        if !has_primary && !has_secondary {
            return true;
        }

        match _wslot {
            WeaponSlotType::Primary => has_primary && self.turret_primary_enabled,
            WeaponSlotType::Secondary => has_secondary && self.turret_secondary_enabled,
            WeaponSlotType::Tertiary => !has_primary && !has_secondary,
        }
    }
    pub(super) fn get_original_victim_pos(&self) -> Option<Coord3D> {
        self.original_victim_pos
    }
    pub(super) fn set_original_victim_pos(&mut self, pos: Option<Coord3D>) {
        self.original_victim_pos = pos;
    }
    pub(super) fn is_in_attack_state(&self) -> bool {
        self.ai_state_machine
            .as_ref()
            .and_then(|machine| machine.lock().ok().map(|guard| guard.is_in_attack_state()))
            .unwrap_or(false)
    }
    pub(super) fn is_in_guard_idle_state(&self) -> bool {
        self.ai_state_machine
            .as_ref()
            .and_then(|machine| {
                machine
                    .lock()
                    .ok()
                    .map(|guard| guard.is_in_guard_idle_state())
            })
            .unwrap_or(false)
    }
    pub(super) fn set_temporary_state(&mut self, state: AIStateType, frame_limit: UnsignedInt) {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                let _ = guard.set_temporary_state(state as u32, frame_limit);
            }
        }
    }
    pub(super) fn notify_crate(&mut self, crate_id: ObjectID) {
        if let Ok(mut guard) = self.crate_created.lock() {
            *guard = crate_id;
        }
    }
    pub(super) fn notify_victim_is_dead(&mut self) {
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            jet_ai.notify_victim_is_dead();
        }
    }
    pub(super) fn set_prior_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.prior_waypoint_id = Some(waypoint_id);
    }
    pub(super) fn set_current_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.current_waypoint_id = Some(waypoint_id);
    }
    pub(super) fn set_completed_waypoint_id(
        &mut self,
        waypoint_id: Option<crate::waypoint::WaypointId>,
    ) {
        self.completed_waypoint_id = waypoint_id;
    }
    pub(super) fn get_completed_waypoint_id(&self) -> Option<crate::waypoint::WaypointId> {
        self.completed_waypoint_id
    }
}
