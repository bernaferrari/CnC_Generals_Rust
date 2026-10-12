//! Original onDie latch and actual installed callback admission, without owner discovery.
use super::*;
use crate::object::die::DieModuleWrapper;

impl Object {
    pub(crate) fn validate_owned_death_callbacks(&self, slow_scale: f32) -> Result<(), String> {
        for entry in self.behavior_modules() {
            if entry.interface_mask().0 & ModuleInterfaceType::DIE.0 == 0 {
                continue;
            }
            entry
                .try_with_module(|module| {
                    if let Some(slow) = module.as_any_mut().downcast_mut::<crate::object::behavior::slow_death_behavior::SlowDeathBehavior>() {
                        return slow.validate_owned_die(self, slow_scale);
                    }
                    let wrapper = module.as_any_mut().downcast_mut::<DieModuleWrapper>()
                        .ok_or("native die callback family not admitted")?;
                    wrapper.owned_instant_death().ok_or("native die callback family not admitted")?.validate_owned_die(self)
                })
                .ok_or("native die callback unavailable")??;
        }
        Ok(())
    }
    pub(crate) fn validate_owned_on_die_tail(&self) -> Result<(), String> {
        if self.radar_data.is_some()
            || self.drawable.is_some()
            || self.producer_id != INVALID_ID
            || self.test_status(ObjectStatusTypes::BoobyTrapped)
            || self.test_status(ObjectStatusTypes::Reconstructing)
        {
            return Err("native onDie effect family not admitted".into());
        }
        if let Some(team) = &self.team_pin {
            if team
                .try_write()
                .map_err(|_| "native dying team unavailable")?
                .has_object_death_script()
            {
                return Err("native team death script delivery not admitted".into());
            }
        }
        Ok(())
    }
    pub(crate) fn begin_owned_on_die(&mut self) {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            debug_assert!(!self.has_died_already, "onDie called twice");
            self.has_died_already = true;
        }
    }
}
