//! Unit order processing and public order-issue helpers.

#![allow(unused_imports)]

use super::identity::Unit;
use super::imports::*;
use super::types::*;

impl Unit {
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
            && self.locomotor_set.get_active().is_some()
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
}
