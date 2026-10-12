//! The same installed InstantDeath callback with an explicit driving owner.
use super::*;
impl InstantDeathBehaviorModuleData {
    pub(crate) fn validate_owned_effects(&self) -> Result<(), String> {
        if !self.fx.is_empty() || !self.ocls.is_empty() || !self.weapons.is_empty() {
            return Err("InstantDeath authored effects require supplied native services".into());
        }
        Ok(())
    }
}
impl InstantDeathBehavior {
    pub(crate) fn new_for_object_id(
        id: crate::common::ObjectID,
        data: Arc<InstantDeathBehaviorModuleData>,
    ) -> Self {
        Self {
            base: DieModule::new_for_object_id(id, data),
        }
    }
    pub(crate) fn validate_owned_die(&self, owner: &Object) -> Result<(), String> {
        if owner.get_id() != self.base.object_id {
            return Err("InstantDeath owner identity mismatch".into());
        }
        self.base.module_data.validate_owned_effects()
    }
    /// Original predicate precedes the cached-AI dead latch. The caller admits
    /// the returned destruction at this point after releasing this module loan.
    pub(crate) fn destroy_on_die_owned(
        &mut self,
        owner: &Object,
        damage: &DamageInfo,
        ai_already_dead: bool,
    ) -> Result<bool, String> {
        self.validate_owned_die(owner)?;
        Ok(self
            .base
            .module_data
            .base
            .die_mux_data
            .is_die_applicable(owner, damage)
            && !ai_already_dead)
    }
}
