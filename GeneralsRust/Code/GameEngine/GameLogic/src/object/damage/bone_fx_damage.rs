//! BoneFXDamage - damage hook for BoneFXUpdate transitions.

use std::sync::{Arc, Mutex};

use crate::common::{BodyDamageType, ObjectID};
use crate::damage::DamageInfo;
use crate::modules::DamageModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::update::bone_fx_update::{BoneFXUpdate, BoneFXUpdateModule};

/// Damage module that delegates body damage state changes to BoneFXUpdate.
#[derive(Debug)]
pub struct BoneFXDamage {
    object_id: ObjectID,
}

impl BoneFXDamage {
    pub fn new(object_id: ObjectID) -> Self {
        Self { object_id }
    }

    pub fn on_object_created(&self) -> Result<(), String> {
        self.with_bone_fx_update(|_| Ok(()))
    }

    fn with_bone_fx_update<F>(&self, func: F) -> Result<(), String>
    where
        F: FnOnce(&mut BoneFXUpdate) -> Result<(), String>,
    {
        let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return Err(format!("BoneFXDamage: Object {} not found", self.object_id));
        };
        let object_guard = object
            .read()
            .map_err(|_| "BoneFXDamage: Object lock failed")?;
        if let Some(module) = object_guard.find_update_module("BoneFXUpdate") {
            let result = module
                .with_module_downcast::<BoneFXUpdateModule, _, _>(|module| {
                    func(module.behavior_mut())
                })
                .ok_or_else(|| "BoneFXUpdate module type mismatch".to_string())?;
            return result;
        }

        let Some(result) =
            object_guard.with_update_behavior_downcast::<BoneFXUpdate, _, _>("BoneFXUpdate", func)
        else {
            return Err("BoneFXUpdate type mismatch".to_string());
        };
        result
    }
}

impl DamageModuleInterface for BoneFXDamage {
    fn receive_damage(&mut self, _object_id: ObjectID, _damage: &DamageInfo) -> f32 {
        0.0
    }

    fn on_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn on_healing(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.with_bone_fx_update(|bone_fx| {
            bone_fx.change_body_damage_state_simple(old_state, new_state);
            Ok(())
        })
        .map_err(|err| err.into())
    }
}
