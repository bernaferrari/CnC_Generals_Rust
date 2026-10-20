//! Object-owned path request admission and execution.
//! C++ AIUpdateInterface::requestAttackPath, doPathfind and deferred requeue.
use super::*;

impl GameLogic {
    /// C++ AIUpdateInterface::doPathfind (378–452): consume an existing
    /// request, never initialize a new move while resolving attack fallback.
    pub(in crate::game_logic) fn resolve_initialized_path_request(
        &mut self,
        mut req: crate::game_logic::pathfinding::PendingHostPath,
    ) -> bool {
        let (
            start,
            can_move,
            is_aircraft,
            surfaces,
            is_crusher,
            safe,
            mut approach,
            attack,
            victim,
        ) = match self.objects.get(&req.unit_id) {
            Some(unit) if unit.is_alive() && unit.waiting_for_path => (
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
            _ => return false,
        };
        if let Some(unit) = self.objects.get_mut(&req.unit_id) {
            req.destination = unit.requested_destination.unwrap_or(req.destination);
            unit.waiting_for_path = false;
            if approach && !crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit)
            {
                unit.is_approach_path = false;
                approach = false;
            }
        }
        if !can_move {
            return false;
        }
        // C++ queueForPath stores the object id; doPathfind reads the
        // current ignored obstacle from this same world's object.
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
                let repulsed = self.pathfind_ai_rules().repulsed_distance;
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
            if let Some(path) = ({
                let ground_owner = self.world_services.terrain();
                let ground_loan = ground_owner
                    .read()
                    .unwrap_or_else(|error| error.into_inner());
                let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                    &ground_loan,
                    self.map_loaded,
                    self.terrain.as_ref(),
                    self.pathfinding_height_samples.as_ref(),
                    &self.pathfinding_system.grid,
                );
                self.pathfinding_system.find_safe_path_from(
                    start,
                    p1,
                    p2,
                    radius,
                    surfaces,
                    is_crusher,
                    is_human,
                    &ground_source,
                )
            }) {
                let _ =
                    self.apply_computed_unit_path(req.unit_id, start, req.destination, path, false);
            }
            self.pathfinding_system.set_ignore_obstacle(None);
            return self
                .objects
                .get(&req.unit_id)
                .is_some_and(|unit| !unit.movement.path.is_empty());
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
            if let Some(path) = ({
                let ground_owner = self.world_services.terrain();
                let ground_loan = ground_owner
                    .read()
                    .unwrap_or_else(|error| error.into_inner());
                let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                    &ground_loan,
                    self.map_loaded,
                    self.terrain.as_ref(),
                    self.pathfinding_height_samples.as_ref(),
                    &self.pathfinding_system.grid,
                );
                self.pathfinding_system.find_closest_path(
                    start,
                    req.destination,
                    surfaces,
                    is_crusher,
                    is_human,
                    0.2,
                    &ground_source,
                )
            }) {
                let last = path.last().copied();
                let _ =
                    self.apply_computed_unit_path(req.unit_id, start, req.destination, path, false);
                if let Some(last) = last {
                    if self.objects.get(&req.unit_id).is_some_and(|unit| {
                        crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(unit)
                    }) {
                        self.register_ground_path_goal(req.unit_id, last);
                    }
                }
            }
            self.pathfinding_system.set_ignore_obstacle(None);
            return self
                .objects
                .get(&req.unit_id)
                .is_some_and(|unit| !unit.movement.path.is_empty());
        }
        let mut goal = self
            .objects
            .get(&req.unit_id)
            .and_then(|unit| unit.requested_destination)
            .unwrap_or(req.destination);
        if attack {
            let live_victim = victim.filter(|id| self.objects.contains_key(id));
            let search_position = live_victim
                .and_then(|id| self.objects.get(&id).map(|victim| victim.get_position()))
                .unwrap_or(goal);
            if self.assign_unit_attack_path_fallback(
                req.unit_id,
                live_victim,
                search_position,
                false,
            ) {
                if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                    // computeAttackPath may clear this flag while installing.
                    // C++ doPathfind restores it only on attack-search success.
                    unit.is_attack_path = true;
                }
                self.pathfinding_system.set_ignore_obstacle(None);
                return true;
            }
            if let Some(unit) = self.objects.get_mut(&req.unit_id) {
                unit.is_attack_path = false;
            }
            if let Some((vid, position)) =
                victim.and_then(|vid| self.objects.get(&vid).map(|v| (vid, v.get_position())))
            {
                goal = position;
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
            return self
                .objects
                .get(&req.unit_id)
                .is_some_and(|unit| !unit.movement.path.is_empty());
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
                let installed =
                    self.apply_computed_unit_path(req.unit_id, start, goal, path, ground_search);
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
        self.objects
            .get(&req.unit_id)
            .is_some_and(|unit| !unit.movement.path.is_empty())
    }

    /// C++ AIUpdate when `now >= m_queueForPathFrame`: queueForPath and clear the timer.
    pub(in crate::game_logic) fn requeue_expired_path_requests(&mut self, ids: &[ObjectId]) {
        for &id in ids {
            let _ = self.queue_initialized_path_request(id);
        }
    }

    /// Queue only the object-owned request metadata; no order/state/audio effects.
    pub(in crate::game_logic) fn queue_initialized_path_request(&mut self, id: ObjectId) -> bool {
        let Some(req) = self.initialized_path_request(id) else {
            return false;
        };
        self.pathfinding_system.queue_path(req)
    }

    pub(in crate::game_logic) fn initialized_path_request(
        &self,
        id: ObjectId,
    ) -> Option<crate::game_logic::pathfinding::PendingHostPath> {
        let Some(unit) = self.objects.get(&id) else {
            return None;
        };
        let Some(dest) = unit.requested_destination else {
            return None;
        };
        if !unit.waiting_for_path {
            return None;
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
        Some(req)
    }
}

#[cfg(test)]
#[path = "attack_path_request_tests.rs"]
mod tests;
