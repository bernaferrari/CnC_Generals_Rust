//! Unit combat targeting, engagement, and auto-acquire.

#![allow(unused_imports)]

use super::identity::Unit;
use super::imports::*;
use super::types::*;

impl Unit {
    pub(super) fn can_detect_target(&self, target: &Object, distance: Real) -> bool {
        if target.is_detected() {
            return true;
        }

        self.can_detect_target_distance(distance)
    }
    pub(super) fn can_detect_target_distance(&self, distance: Real) -> bool {
        let base_range = self
            .base_arc()
            .read()
            .ok()
            .map(|guard| guard.get_stealth_detection_range() as Real)
            .unwrap_or(0.0);
        let detection_range = self.stealth_detection_range.max(base_range);

        if detection_range <= 0.0 {
            return false;
        }

        distance <= detection_range
    }
    pub(super) fn is_currently_attacking(&self) -> bool {
        matches!(
            self.current_order,
            Some(UnitOrder::Attack { .. }) | Some(UnitOrder::AttackMove { .. })
        ) || self.movement_state == MovementState::Attacking
    }
    pub(super) fn can_auto_acquire_now(&self) -> bool {
        if !self.auto_acquire_enemies {
            return false;
        }

        if self.auto_acquire_not_while_attacking && self.is_currently_attacking() {
            return false;
        }

        if !self.auto_acquire_while_stealthed {
            let stealthed = self
                .base_arc()
                .read()
                .map(|guard| guard.is_stealthed())
                .unwrap_or(false);
            if stealthed {
                return false;
            }
        }

        true
    }
}
