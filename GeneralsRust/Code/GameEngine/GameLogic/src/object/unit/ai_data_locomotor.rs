//! Locomotor access, movement flags, and blocked-speed control.

use super::ai_data::UnitAiData;
use super::imports::*;

impl UnitAiData {
    pub(super) fn get_preferred_height(&self) -> Option<Real> {
        self.locomotor_set
            .get_active()
            .map(|loco| loco.preferred_height)
    }
    pub(super) fn is_waiting_for_path(&self) -> bool {
        // C++ AIUpdate.h:445 reads m_waitingForPath directly. A future queue
        // deadline and the Unit's residual path mirror are separate state.
        self.waiting_for_path
    }
    pub(super) fn set_allow_invalid_position(
        &mut self,
        allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(loco) = self.locomotor_set.get_active_mut() {
            loco.set_allow_invalid_position(allow);
        }
        Ok(())
    }
    pub(super) fn set_allow_chase(&mut self, allowed: bool) {
        self.allow_chase = allowed;
    }
    pub(super) fn set_ultra_accurate(
        &mut self,
        ultra: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(loco) = self.locomotor_set.get_active_mut() {
            loco.set_ultra_accurate(ultra);
        }
        Ok(())
    }
    pub(super) fn set_precise_z_pos(
        &mut self,
        precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(loco) = self.locomotor_set.get_active_mut() {
            loco.set_precise_z_pos(precise);
        }
        Ok(())
    }
    pub(super) fn with_cur_locomotor(&self, f: &mut dyn FnMut(&crate::locomotor::Locomotor)) {
        if let Some(loco) = self.locomotor_set.get_active() {
            f(loco);
        }
    }
    pub(super) fn with_cur_locomotor_mut(
        &mut self,
        f: &mut dyn FnMut(&mut crate::locomotor::Locomotor),
    ) {
        if let Some(loco) = self.locomotor_set.get_active_mut() {
            f(loco);
        }
    }
    pub(super) fn get_locomotor_set_clone(&self) -> Option<crate::locomotor::LocomotorSet> {
        (!self.locomotor_set.is_empty()).then(|| self.locomotor_set.clone())
    }
    pub(super) fn is_aircraft_that_adjusts_destination(&self) -> bool {
        let Some(locomotor) = self.locomotor_set.get_active() else {
            return false;
        };
        matches!(
            locomotor.get_appearance(),
            LocomotorAppearance::Hover | LocomotorAppearance::Wings
        )
    }
    pub(super) fn get_cur_locomotor_set_type(&self) -> LocomotorSetType {
        self.current_locomotor_set
    }
    pub(super) fn get_cur_max_blocked_speed(&self) -> Real {
        self.cur_max_blocked_speed
    }
    pub(super) fn set_cur_max_blocked_speed(&mut self, speed: Real) {
        self.cur_max_blocked_speed = speed;
    }
    pub(super) fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        self.locomotor_goal_type = 3;
        self.locomotor_goal_data.x = angle;
    }
    pub(super) fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        self.locomotor_goal_type = 2;
        self.locomotor_goal_data = pos;
    }
    pub(super) fn apply_bump_speed_limit(
        &mut self,
        mut desired_speed: Real,
        mut blocked: bool,
    ) -> Real {
        if blocked && desired_speed > self.cur_max_blocked_speed {
            desired_speed = self.cur_max_blocked_speed;
            if self.bump_speed_limit > desired_speed {
                self.bump_speed_limit = desired_speed;
            }
            self.bump_speed_limit *= 0.95;
            desired_speed = self.bump_speed_limit;
        } else {
            blocked = false;
            if self.bump_speed_limit < FAST_AS_POSSIBLE {
                let min_limit = desired_speed * 0.2;
                if self.bump_speed_limit < min_limit {
                    self.bump_speed_limit = min_limit;
                }
                self.bump_speed_limit *= 1.05;
            }
            if desired_speed > self.bump_speed_limit {
                desired_speed = self.bump_speed_limit;
            }
        }
        if !blocked && self.blocked_frames > 1 {
            self.blocked_frames = 1;
        }
        desired_speed
    }

    pub(super) fn get_desired_speed(&self) -> Real {
        self.desired_speed
    }

    pub(super) fn set_desired_speed(&mut self, speed: Real) {
        self.desired_speed = speed;
    }

    pub(super) fn get_ignore_collisions_until(&self) -> UnsignedInt {
        self.ignore_collisions_until
    }

    pub(super) fn friend_ending_move(&mut self) {
        self.movement_complete = true;
        self.cpp_is_moving = false;
    }

    pub(super) fn friend_starting_move(&mut self) {
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.movement_complete = false;
        self.cpp_is_moving = true;
    }
}
