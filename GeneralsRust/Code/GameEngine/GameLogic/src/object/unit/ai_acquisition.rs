//! C++ AIUpdate.cpp:4471-4648: native mood-target admission and scan order.
//! The source is the constructor-bound Object. Target/world services still
//! use the active-world adapters and are not instance-isolated by this module.
use super::ai_core::UnitAIUpdate;
use super::imports::*;
use super::registry::{dual_world_registry_unavailable, get_unit_arc};
use crate::ai::{search_qualifiers, vision_factors};
use crate::object::update::ai_update_interface::{
    AUTO_ACQUIRE_IDLE, AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS, AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING,
    AUTO_ACQUIRE_IDLE_STEALTHED,
};

impl UnitAIUpdate {
    pub(super) fn can_auto_acquire(&self) -> bool {
        if self.runtime.owner.is_some() {
            // AIUpdate.h:565 returns the whole mask as Bool, including NO.
            return self.runtime.data.auto_acquire_enemies_when_idle != 0;
        }
        get_unit_arc(self.runtime.unit_id)
            .and_then(|unit| unit.read().ok().map(|guard| guard.auto_acquire_enemies))
            .unwrap_or(false)
    }

    pub(super) fn can_auto_acquire_while_stealthed(&self) -> bool {
        if self.runtime.owner.is_some() {
            let Some(owner) = self.runtime.owner.as_ref().and_then(Weak::upgrade) else {
                return false;
            };
            return owner
                .read()
                .ok()
                .is_some_and(|source| self.can_auto_acquire_while_stealthed_for_source(&source));
        }
        get_unit_arc(self.runtime.unit_id)
            .and_then(|unit| {
                unit.read()
                    .ok()
                    .map(|guard| guard.auto_acquire_while_stealthed)
            })
            .unwrap_or(false)
    }

    fn can_auto_acquire_while_stealthed_for_source(&self, source: &crate::object::Object) -> bool {
        self.runtime
            .can_auto_acquire_while_stealthed_for_source(source)
    }

    pub(super) fn get_next_mood_target_id(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
    ) -> ObjectID {
        self.get_next_mood_target_for_state(called_by_ai, called_during_idle, None)
    }

    /// An in-state caller supplies the current State's real virtual classifier.
    /// External callers obtain it from the machine. No classification is cached.
    /// An in-state caller supplies the current State's real virtual classifier.
    /// External callers obtain it from the machine. No classification is cached.
    pub(super) fn get_next_mood_target_for_state(
        &mut self,
        called_by_ai: bool,
        called_during_idle: bool,
        is_attacking: Option<bool>,
    ) -> ObjectID {
        let machine = self.ai_state_machine.as_ref();
        let owner_exists = self.runtime.owner.is_some();
        let unit_id = self.runtime.unit_id;
        let mut parent_is_attacking =
            || super::ai_runtime::is_attacking_for_runtime(machine, owner_exists, unit_id);
        self.runtime.get_next_mood_target_for_state(
            called_by_ai,
            called_during_idle,
            is_attacking,
            &mut parent_is_attacking,
        )
    }

    // Explicit standalone adapter; native Objects never enter the Unit lookup.
    pub(super) fn get_legacy_mood_target(
        &mut self,
        use_existing_target: bool,
        _ignore_attacked: bool,
    ) -> ObjectID {
        self.runtime
            .get_legacy_mood_target(use_existing_target, _ignore_attacked)
    }
}
