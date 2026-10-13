//! Original onDie latch and actual installed callback admission, without owner discovery.
use super::*;
use crate::object::die::DieModuleWrapper;

impl Object {
    /// Shared original authored visitation, including containment's DIE callback
    /// even when a Rust template supplies only its CONTAIN interface bit.
    pub(crate) fn death_callback_entries(&self) -> Vec<BehaviorModuleHandle> {
        self.behavior_modules()
            .into_iter()
            .filter(|entry| {
                entry.interface_mask().0 & ModuleInterfaceType::DIE.0 != 0
                    || entry.with_module(|module| {
                        crate::contain_module_overrides::contain_handle_for_module(module).is_some()
                    })
            })
            .collect()
    }
    pub(crate) fn validate_owned_death_callbacks(&self, slow_scale: f32) -> Result<(), String> {
        for entry in self.behavior_modules() {
            if entry.interface_mask().0 & ModuleInterfaceType::DIE.0 == 0 {
                continue;
            }
            entry
                .try_with_module(|module| {
                    if let Some(contain) = crate::contain_module_overrides::contain_handle_for_module(module) {
                        if { let contain = contain.try_lock().map_err(|_| "native containment DIE borrowed")?;
                            contain.supports_owned_lifecycle() || contain.owned_tunnel_registered().is_some() } {
                            return Ok(());
                        }
                        return Err("native containment DIE family not supplied".into());
                    }
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
        if self.drawable.is_some()
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
    /// CPP Object.cpp:1956–1968 removes radar at the effectively-dead edge,
    /// before onDamage/score/onDie. Its later onDie check is then a no-op.
    pub(crate) fn set_effectively_dead_with_radar(
        &mut self,
        dead: bool,
        radar: &mut game_engine::common::system::radar::RadarSystem,
    ) -> Result<(), String> {
        if dead {
            if let Some(data) = self.radar_data.as_deref() {
                if data.object_id != self.id || !radar.contains_object(self.id) {
                    return Err("effectively-dead radar link belongs to another owner".into());
                }
            }
            self.private_status |= ObjectPrivateStatusBits::EffectivelyDead as u8;
            if self.radar_data.is_some() {
                assert!(
                    radar.remove_object(self.id),
                    "admitted effectively-dead radar link"
                );
                self.radar_data = None;
            }
        } else {
            self.private_status &= !(ObjectPrivateStatusBits::EffectivelyDead as u8);
        }
        Ok(())
    }

    pub(crate) fn owned_radar_data(&self) -> Option<&RadarObject> {
        self.radar_data.as_deref()
    }

    /// CPP Radar::deleteFromList removes the object's radar link too.
    pub(crate) fn clear_owned_radar_data(&mut self) {
        self.radar_data = None;
    }

    pub(crate) fn begin_owned_on_die(&mut self) {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            debug_assert!(!self.has_died_already, "onDie called twice");
            self.has_died_already = true;
        }
    }
}
