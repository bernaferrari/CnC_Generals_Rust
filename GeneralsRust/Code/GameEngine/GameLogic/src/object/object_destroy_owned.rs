//! Original destroy notification and final storage release for admitted native owners.
use super::*;
use crate::object::behavior::auto_heal_behavior::AutoHealBehaviorModule;
use crate::object::behavior::slow_death_behavior::SlowDeathBehavior;
use crate::object::die::DieModuleWrapper;
use crate::object::update::AIUpdateInterfaceModule;

pub(crate) struct NativeDestroyPlan {
    modules: Vec<Arc<ModuleEntry>>,
}

impl NativeDestroyPlan {
    pub(crate) fn notify_delete(&self) -> Result<(), String> {
        for entry in &self.modules {
            entry.with_module(|module| {
                if let Some(heal) = module.as_any_mut().downcast_mut::<AutoHealBehaviorModule>() {
                    heal.behavior_mut().delete_owned()
                } else {
                    // Every other admitted concrete class inherits the empty
                    // Module::onDelete. Invoke the actual callback in tag order.
                    module.on_delete();
                    Ok(())
                }
            })?;
        }
        Ok(())
    }
}

impl Object {
    pub(crate) fn scheduled_native_slow_death(
        &self,
        proxy: &UpdateModulePtr,
    ) -> Result<bool, String> {
        let registration = self
            .update_module_registrations
            .iter()
            .find(|entry| Arc::ptr_eq(&entry.module, proxy))
            .ok_or("native scheduled proxy belongs to another object")?;
        let index = registration
            .module_index
            .ok_or("native constructor-helper tick not admitted")?;
        let entry = self
            .modules
            .get(index)
            .ok_or("native update module index invalid")?;
        entry
            .try_with_module(|module| module.as_any().is::<SlowDeathBehavior>())
            .ok_or("native update module already borrowed".into())
    }

    pub(crate) fn native_destroy_plan(&self) -> Result<NativeDestroyPlan, String> {
        if self.ambient_lifecycle_admitted || !self.modules_ready {
            return Err("destroy requires an admitted native object".into());
        }
        if self.contain.is_some()
            || self.get_container_id().is_some()
            || self.drawable.is_some()
            || self.radar_data.is_some()
            || self.group_id.is_some()
            || self.has_any_special_power()
            || self.thing_template.get_energy_production() != 0
        {
            return Err("native destroy callback family requires its owning services".into());
        }
        for entry in &self.modules {
            let supported = entry
                .try_with_module(|module| -> Result<bool, String> {
                    if entry.mask().0 & ModuleInterfaceType::DESTROY.0 != 0 {
                        return Err("native onDestroy interface is not admitted".into());
                    }
                    if let Some(heal) = module.as_any_mut().downcast_mut::<AutoHealBehaviorModule>()
                    {
                        heal.behavior().validate_delete_owned()?;
                        return Ok(true);
                    }
                    if crate::contain_module_overrides::has_owned_empty_body_delete(module)
                        || module.as_any().is::<AIUpdateInterfaceModule>()
                        || module.as_any().is::<SlowDeathBehavior>()
                    {
                        return Ok(true);
                    }
                    if let Some(die) = module.as_any_mut().downcast_mut::<DieModuleWrapper>() {
                        return Ok(die.owned_instant_death().is_some());
                    }
                    Ok(false)
                })
                .ok_or("native delete module unavailable")??;
            if !supported {
                return Err(format!(
                    "native deletion module {} not admitted",
                    entry.name()
                ));
            }
        }
        Ok(NativeDestroyPlan {
            modules: self.modules.clone(),
        })
    }

    pub(crate) fn begin_native_destroy(&mut self) -> bool {
        if self.lifecycle != ObjectLifecycle::Alive || self.is_destroyed() {
            return false;
        }
        self.status.set_status(ObjectStatusTypes::Destroyed);
        true
    }

    pub(crate) fn mark_native_destroy_notified(&mut self) {
        self.lifecycle = ObjectLifecycle::DestroyNotified;
    }

    pub(crate) fn native_destroy_is_finalized(&self) -> bool {
        self.lifecycle == ObjectLifecycle::Finalized
    }

    pub(crate) fn clear_native_team_pin(&mut self) {
        self.team_id = None;
        self.team_pin = None;
    }

    pub(crate) fn finish_native_destroy_storage(&mut self) -> Result<(), String> {
        if self.lifecycle != ObjectLifecycle::DestroyNotified {
            return Err("native final deletion precedes onDelete".into());
        }
        if self.get_team().is_some() {
            return Err("native final deletion precedes team removal".into());
        }
        self.lifecycle = ObjectLifecycle::Finalized;
        self.ambient_lifecycle_admitted = false;
        self.update_module_registrations.clear();
        self.ai = None;
        self.physics = None;
        self.contain = None;
        self.stealth = None;
        self.upgrade_module_handles.clear();
        self.body_module_handles.clear();
        self.die_module_handles.clear();
        self.update_module_handles.clear();
        self.collide_module_handles.clear();
        self.contain_module_handles.clear();
        self.behaviors.clear();
        self.modules.clear();
        self.body = None;
        self.modules_ready = false;
        self.experience_tracker = None;
        self.firing_tracker = None;
        self.status_damage_helper = None;
        self.temp_weapon_bonus_helper = None;
        self.subdual_damage_helper = None;
        self.smc_helper = None;
        self.ws_helper = None;
        self.defection_helper = None;
        self.repulsor_helper = None;
        self.held_helper = None;
        self.partition_data = None;
        self.id = INVALID_ID;
        Ok(())
    }
}
