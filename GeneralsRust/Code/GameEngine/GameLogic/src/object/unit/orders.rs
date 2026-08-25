//! Unit order processing and public order-issue helpers.

#![allow(unused_imports)]

use super::identity::Unit;
use super::imports::*;
use super::registry::dual_world_registry_unavailable;
use super::types::*;

impl Unit {
    /// Update unit logic for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Process current order
        self.process_current_order(delta_time)?;

        // Update movement
        self.update_movement(delta_time)?;

        // Update combat behavior
        self.update_combat(delta_time)?;

        // Update facing direction
        self.update_facing(delta_time)?;

        // Check for state changes
        self.check_status_effects(delta_time)?;

        // Update animation state
        self.update_animation_state()?;

        // Update per-unit AI module (matches C++ AIUpdateInterface::update call per frame).
        if let Ok(base_guard) = self.base_arc().read() {
            if let Some(ai) = base_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let _ = ai_guard.update();
                }
            }
        }

        Ok(())
    }
    /// Process the current order
    pub(super) fn process_current_order(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.advance_order_queue();
        let order = self.current_order.take();
        match order {
            None => {
                // No current order, check for auto-behaviors
                if self.auto_acquire_enemies {
                    self.look_for_enemies()?;
                }

                if self.return_to_formation {
                    self.return_to_formation_position()?;
                }
            }
            Some(current_order) => {
                if !matches!(current_order, UnitOrder::AttackMove { .. }) {
                    self.attack_move_active = false;
                }

                match current_order {
                    UnitOrder::Stop => {
                        self.stop_movement();
                        // Don't restore — order is consumed
                        self.advance_order_queue();
                    }

                    UnitOrder::Move {
                        destination,
                        use_formation,
                        waypoints,
                    } => {
                        if self.movement_state == MovementState::Idle
                            && self.target_position.is_none()
                            && self.waypoint_queue.is_empty()
                        {
                            let delta = self.get_position() - destination;
                            if (delta.x * delta.x + delta.y * delta.y).sqrt() <= 1.0 {
                                // Don't restore — order completed
                                self.advance_order_queue();
                                return Ok(());
                            }
                        }
                        self.process_move_order(destination, use_formation, &waypoints)?;
                        // Restore — move order continues across frames
                        self.current_order = Some(UnitOrder::Move {
                            destination,
                            use_formation,
                            waypoints,
                        });
                    }

                    UnitOrder::Attack { target, pursue } => {
                        self.process_attack_order(target, pursue)?;
                        self.current_order = Some(UnitOrder::Attack { target, pursue });
                    }

                    UnitOrder::AttackMove {
                        destination,
                        engage_enemies,
                    } => {
                        self.process_attack_move_order(destination, engage_enemies)?;
                        self.current_order = Some(UnitOrder::AttackMove {
                            destination,
                            engage_enemies,
                        });
                    }

                    UnitOrder::Guard {
                        position,
                        area_radius,
                    } => {
                        self.process_guard_order(position, area_radius)?;
                        self.current_order = Some(UnitOrder::Guard {
                            position,
                            area_radius,
                        });
                    }

                    UnitOrder::Follow { target, distance } => {
                        self.process_follow_order(target, distance)?;
                        self.current_order = Some(UnitOrder::Follow { target, distance });
                    }

                    UnitOrder::Patrol {
                        waypoints,
                        loop_patrol,
                    } => {
                        self.process_patrol_order(&waypoints, loop_patrol)?;
                        self.current_order = Some(UnitOrder::Patrol {
                            waypoints,
                            loop_patrol,
                        });
                    }

                    UnitOrder::Garrison { building } => {
                        self.process_garrison_order(building)?;
                        self.current_order = Some(UnitOrder::Garrison { building });
                    }

                    UnitOrder::Ungarrison { exit_position } => {
                        self.process_ungarrison_order(exit_position)?;
                        self.current_order = Some(UnitOrder::Ungarrison { exit_position });
                    }

                    UnitOrder::Capture { building } => {
                        self.process_capture_order(building)?;
                        self.current_order = Some(UnitOrder::Capture { building });
                    }

                    UnitOrder::Retreat {
                        safe_position,
                        organized,
                    } => {
                        self.process_retreat_order(safe_position, organized)?;
                        self.current_order = Some(UnitOrder::Retreat {
                            safe_position,
                            organized,
                        });
                    }

                    other => {
                        // Restore unhandled order types
                        self.current_order = Some(other);
                    }
                }
            }
        }

        Ok(())
    }
    pub(super) fn advance_order_queue(&mut self) {
        if self.current_order.is_none() && !self.order_queue.is_empty() {
            self.current_order = Some(self.order_queue.remove(0));
        }
    }
    /// Issue a move order to the unit
    pub fn give_move_order(
        &mut self,
        destination: Coord3D,
        waypoints: Vec<Waypoint>,
        use_formation: bool,
        queue_order: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let order = UnitOrder::Move {
            destination,
            use_formation,
            waypoints,
        };

        if queue_order {
            self.order_queue.push(order);
        } else {
            self.current_order = Some(order);
            self.order_queue.clear();
        }

        Ok(())
    }
    /// Issue an attack order to the unit
    pub fn give_attack_order(
        &mut self,
        target: ObjectID,
        pursue: bool,
        queue_order: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let order = UnitOrder::Attack { target, pursue };

        if queue_order {
            self.order_queue.push(order);
        } else {
            self.current_order = Some(order);
            self.order_queue.clear();
        }

        self.attack_target = Some(target);

        Ok(())
    }
    /// Issue a capture building order to the unit.
    pub fn give_capture_order(
        &mut self,
        building: ObjectID,
        queue_order: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let order = UnitOrder::Capture { building };

        if queue_order {
            self.order_queue.push(order);
        } else {
            self.current_order = Some(order);
            self.order_queue.clear();
        }

        Ok(())
    }
    /// Set combat mode
    pub fn set_combat_mode(&mut self, mode: CombatMode) {
        self.combat_mode = mode;

        // Clear attack target if switching to hold fire
        if mode == CombatMode::HoldFire {
            self.attack_target = None;
        }
    }
    /// Check if unit can move
    pub fn can_move(&self) -> bool {
        !self.is_stunned
            && !self.is_pinned
            && !self.is_garrisoned
            && self.current_locomotor.is_some()
    }
    /// Check if unit can attack
    pub fn can_attack(&self) -> bool {
        !self.is_stunned
            && !self.is_suppressed
            && self.combat_mode != CombatMode::HoldFire
            && self.has_weapons()
    }
    /// Get current position
    pub fn get_position(&self) -> Coord3D {
        if let Ok(obj_guard) = self.base_arc().read() {
            *obj_guard.get_position()
        } else {
            Coord3D::new(0.0, 0.0, 0.0)
        }
    }
    /// Get current health percentage
    pub fn get_health_percentage(&self) -> Real {
        if let Ok(obj_guard) = self.base_arc().read() {
            let current = obj_guard.get_health();
            let max = obj_guard.get_max_health();
            if max > 0.0 { current / max } else { 0.0 }
        } else {
            0.0
        }
    }
    /// Check if unit has weapons
    pub fn has_weapons(&self) -> bool {
        if let Ok(obj_guard) = self.base_arc().read() {
            obj_guard.has_any_weapon()
        } else {
            false
        }
    }
    /// Private helper methods
    pub(super) fn process_move_order(
        &mut self,
        destination: Coord3D,
        use_formation: bool,
        waypoints: &[Waypoint],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_move() {
            let should_repath = self
                .target_position
                .map(|pos| (pos - destination).length() > 0.1 || !self.is_movement_active())
                .unwrap_or(true);
            if should_repath {
                self.move_to_position(destination, use_formation)?;
                // Set up waypoint queue
                self.waypoint_queue = waypoints.to_vec();
            }
        }
        Ok(())
    }
    pub(super) fn process_attack_order(
        &mut self,
        target: ObjectID,
        _pursue: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_attack() {
            self.attack_target = Some(target);
            // Additional attack logic would go here
        }
        Ok(())
    }
    pub(super) fn process_attack_move_order(
        &mut self,
        destination: Coord3D,
        engage_enemies: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_move() {
            let should_repath = self
                .target_position
                .map(|pos| (pos - destination).length() > 0.1 || !self.is_movement_active())
                .unwrap_or(true);

            if should_repath {
                self.move_to_position(destination, false)?;
            }
        }

        self.attack_target = None;
        self.auto_acquire_enemies = engage_enemies;
        if engage_enemies {
            self.combat_mode = CombatMode::Aggressive;
        }
        self.attack_move_active = true;
        Ok(())
    }
    pub(super) fn process_guard_order(
        &mut self,
        position: Coord3D,
        area_radius: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_move() {
            let should_repath = self
                .target_position
                .map(|pos| (pos - position).length() > 0.1 || !self.is_movement_active())
                .unwrap_or(true);

            if should_repath {
                self.move_to_position(position, false)?;
            }
        }

        self.guard_position = Some(position);
        self.guard_radius = area_radius;
        self.combat_mode = CombatMode::GuardArea;
        self.auto_acquire_enemies = true;
        Ok(())
    }
    pub(super) fn process_follow_order(
        &mut self,
        target: ObjectID,
        distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 258: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        self.follow_target = Some(target);
        self.follow_distance = distance.max(0.0);
        let Some(target_pos) =
            crate::object::registry::OBJECT_REGISTRY.with_object(target, |g| *g.get_position())
        else {
            self.follow_target = None;
            self.current_order = None;
            self.advance_order_queue();
            return Ok(());
        };
        let current_pos = self.get_position();
        let dx = target_pos.x - current_pos.x;
        let dy = target_pos.y - current_pos.y;
        let distance_to_target = (dx * dx + dy * dy).sqrt();
        if distance_to_target > self.follow_distance + 1.0 {
            let mut desired = target_pos;
            if distance_to_target > 0.001 {
                let scale =
                    (distance_to_target - self.follow_distance).max(0.0) / distance_to_target;
                desired.x = current_pos.x + dx * scale;
                desired.y = current_pos.y + dy * scale;
            }
            if self.can_move() {
                let should_repath = self
                    .target_position
                    .map(|pos| (pos - desired).length() > 0.1 || !self.is_movement_active())
                    .unwrap_or(true);
                if should_repath {
                    self.move_to_position(desired, false)?;
                    self.movement_state = MovementState::Following;
                }
            }
        } else if matches!(
            self.movement_state,
            MovementState::Moving | MovementState::Following
        ) {
            self.movement_state = MovementState::Idle;
            self.target_position = None;
        }
        Ok(())
    }
    pub(super) fn process_patrol_order(
        &mut self,
        waypoints: &[Coord3D],
        loop_patrol: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let is_same_path = !self.patrol_points.is_empty() && self.patrol_points == waypoints;
        if !is_same_path {
            self.patrol_points = waypoints.to_vec();
            self.current_patrol_index = 0;
            self.patrol_loop = loop_patrol;
        } else {
            self.patrol_loop = loop_patrol;
        }
        if self.patrol_points.is_empty() {
            self.current_order = None;
            self.advance_order_queue();
            return Ok(());
        }
        self.combat_mode = CombatMode::Aggressive;
        self.auto_acquire_enemies = true;
        self.attack_move_active = true;
        if self.can_move()
            && self.movement_state == MovementState::Idle
            && self.target_position.is_none()
        {
            if self.current_patrol_index >= self.patrol_points.len() {
                if self.patrol_loop {
                    self.current_patrol_index = 0;
                } else {
                    self.current_order = None;
                    self.advance_order_queue();
                    return Ok(());
                }
            }
            let dest = self.patrol_points[self.current_patrol_index];
            self.current_patrol_index = self.current_patrol_index.saturating_add(1);
            self.move_to_position(dest, false)?;
            self.movement_state = MovementState::Patrolling;
        }
        Ok(())
    }
    pub(super) fn process_garrison_order(
        &mut self,
        building: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_garrisoned {
            self.current_order = None;
            self.advance_order_queue();
            return Ok(());
        }
        self.garrison_building = Some(building);
        let base = self.base_arc();
        let enter_command = base.read().ok().and_then(|obj_guard| {
            let ai = obj_guard.get_ai_update_interface()?;
            let cmd_source = ai
                .lock()
                .ok()
                .map(|ai_guard| ai_guard.get_last_command_source())
                .unwrap_or(CommandSourceType::FromPlayer);
            Some((ai, cmd_source))
        });
        if let Some((ai, cmd_source)) = enter_command {
            ai.ai_enter(building, cmd_source);
            self.current_order = None;
            self.advance_order_queue();
            return Ok(());
        }
        if let Some(container) = TheGameLogic::find_object_by_id(building) {
            if let Ok(container_guard) = container.read() {
                if let Some(contain) = container_guard.get_contain() {
                    if let Ok(mut contain_guard) = contain.lock() {
                        if let Ok(base_guard) = self.base_arc().read() {
                            let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                &*base_guard,
                                crate::modules::ContainWant::WantsToEnter,
                            );
                        }
                    }
                }
                if self.can_move() && self.movement_state == MovementState::Idle {
                    self.move_to_position(*container_guard.get_position(), false)?;
                }
            }
        }
        Ok(())
    }
    pub(super) fn process_ungarrison_order(
        &mut self,
        exit_position: Option<Coord3D>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let container_id = self
            .base_arc()
            .read()
            .ok()
            .and_then(|guard| guard.get_contained_by());
        if let Ok(obj_guard) = self.base_arc().read() {
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                let cmd_source = ai
                    .lock()
                    .ok()
                    .map(|ai_guard| ai_guard.get_last_command_source())
                    .unwrap_or(CommandSourceType::FromPlayer);
                let mut params =
                    crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Exit, cmd_source);
                params.obj = container_id;
                let _ = ai
                    .lock()
                    .ok()
                    .map(|mut guard| guard.execute_command(&params));
            }
        }
        if let Some(container_id) = container_id {
            if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                if let Ok(container_guard) = container.read() {
                    if let Some(contain) = container_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            if let Ok(base_guard) = self.base_arc().read() {
                                let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                    &*base_guard,
                                    crate::modules::ContainWant::WantsToExit,
                                );
                            }
                        }
                    }
                }
            }
        }
        self.is_garrisoned = false;
        self.garrison_building = None;
        if let Some(pos) = exit_position {
            self.order_queue.insert(
                0,
                UnitOrder::Move {
                    destination: pos,
                    use_formation: false,
                    waypoints: Vec::new(),
                },
            );
        }
        self.current_order = None;
        self.advance_order_queue();
        Ok(())
    }
    pub(super) fn process_capture_order(
        &mut self,
        building: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_capture_buildings {
            return Ok(());
        }

        let Some(building_arc) = TheGameLogic::find_object_by_id(building) else {
            return Ok(());
        };

        let (unit_pos, unit_radius, unit_player_id) = {
            let base = self.base_arc();
            let Ok(unit_guard) = base.read() else {
                return Ok(());
            };
            let player_id = unit_guard.get_player_id();
            let radius = unit_guard.get_geometry_info().get_bounding_circle_radius();
            (*unit_guard.get_position(), radius, player_id)
        };

        let (building_pos, building_radius, can_capture) = {
            let Ok(building_guard) = building_arc.read() else {
                return Ok(());
            };
            let radius = building_guard
                .get_geometry_info()
                .get_bounding_circle_radius();
            let can_capture = TheActionManager::can_capture_building(
                &*self.base_arc().read().map_err(|_| "Unit lock poisoned")?,
                &*building_guard,
                CommandSourceType::FromAi,
            );
            (*building_guard.get_position(), radius, can_capture)
        };

        if !can_capture {
            return Ok(());
        }

        let dx = unit_pos.x - building_pos.x;
        let dy = unit_pos.y - building_pos.y;
        let dist_sq = dx * dx + dy * dy;
        let capture_range = unit_radius + building_radius + PATHFIND_CLOSE_ENOUGH;

        if dist_sq > capture_range * capture_range {
            self.move_to_position(building_pos, false)?;
            return Ok(());
        }

        let Some(player_id) = unit_player_id else {
            return Ok(());
        };

        if let Ok(mut factory) = get_object_factory().write() {
            if let Some(GameObjectInstance::Structure(structure)) = factory.get_object_mut(building)
            {
                let _ = structure.mark_capture_activity(player_id);
            }
        }
        Ok(())
    }
    pub(super) fn process_retreat_order(
        &mut self,
        safe_position: Coord3D,
        _organized: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.attack_target = None;
        self.attack_move_active = false;
        self.auto_acquire_enemies = false;

        if self.can_move() {
            let should_repath = self
                .target_position
                .map(|pos| (pos - safe_position).length() > 0.1 || !self.is_movement_active())
                .unwrap_or(true);
            if should_repath {
                self.move_to_position(safe_position, false)?;
                self.movement_state = MovementState::Retreating;
            }
        }
        Ok(())
    }
}
