//! Compatibility shim for legacy Object/FiringTracker naming, plus Object hooks
//! that drive the ctor-helper tracker (update, pre-attack stretch, shockwave pose).

pub use crate::helpers::FiringTracker;

use super::object_impl_imports::*;
use super::*;

impl Object {
    /// Drive the object-owned FiringTracker UpdateModule (C++ ctor helper on `m_behaviors`).
    pub(super) fn update_firing_tracker(&mut self) {
        // Taken out because `update_for_owner` needs `&mut self` (the owner)
        // at the same time; put straight back.
        let mut tracker = match self.firing_tracker.take() {
            Some(tracker) => tracker,
            None => return,
        };
        let _ = tracker.update_for_owner(self);
        self.firing_tracker = Some(tracker);
    }

    /// C++ `adjustModelConditionForWeaponStatus` WSF_PREATTACK loop-duration stretch.
    pub(super) fn stretch_preattack_animation(
        &mut self,
        condition: crate::weapon::WeaponSetConditionType,
        slot: crate::weapon::WeaponSlotType,
    ) {
        if condition != crate::weapon::WeaponSetConditionType::PreAttack {
            return;
        }
        let done = self
            .weapon_set
            .get_weapon_in_slot(slot)
            .map(|weapon| weapon.get_pre_attack_finished_frame())
            .unwrap_or(0);
        let now = crate::helpers::TheGameLogic::get_frame();
        if done > now {
            self.set_animation_loop_duration(done - now);
        }
    }

    /// C++ shockwave eligibility: physics, not airborne, not projectile.
    pub(super) fn shockwave_applies(&self) -> bool {
        self.physics.is_some() && !self.is_airborne_target() && !self.is_kind_of(KindOf::Projectile)
    }

    /// C++ `setModelConditionState(MODELCONDITION_STUNNED_FLAILING)`.
    pub(super) fn set_shockwave_stunned_flailing(&mut self) {
        self.set_model_condition_state(ModelConditionFlags::STUNNED_FLAILING);
    }
}
