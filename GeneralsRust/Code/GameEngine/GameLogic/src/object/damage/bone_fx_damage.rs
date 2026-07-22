//! BoneFXDamage - damage hook for BoneFXUpdate transitions.

use std::sync::Arc;

use crate::common::xfer::XferExt;
use crate::common::{AsciiString, BodyDamageType, ObjectID};
use crate::damage::DamageInfo;
use crate::modules::{BehaviorModuleInterface, DamageModuleInterface};
use crate::object::damage::DamageModuleData;
use crate::object::registry::OBJECT_REGISTRY;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    BoneFxControlInterface, Module, ModuleData as EngineModuleData,
    NameKeyType as EngineNameKeyType,
};

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
        self.with_bone_fx_control(|_| Ok(()))
    }

    fn with_bone_fx_control<F>(&self, func: F) -> Result<(), String>
    where
        F: FnOnce(&mut dyn BoneFxControlInterface) -> Result<(), String>,
    {
        let object_id = self.object_id;
        let Some(result) = OBJECT_REGISTRY.with_object(self.object_id, |object_guard| {
            if let Some(module) = object_guard.find_update_module("BoneFXUpdate") {
                let mut func = Some(func);
                let mut result = None;
                module.with_module(|module| {
                    if let (Some(bone_fx), Some(func)) =
                        (module.get_bone_fx_control_interface(), func.take())
                    {
                        result = Some(func(bone_fx));
                    }
                });
                return result
                    .unwrap_or_else(|| Err("BoneFXUpdate module type mismatch".to_string()));
            }

            let Some(behavior) = object_guard.find_update_behavior("BoneFXUpdate") else {
                return Err("BoneFXUpdate type mismatch".to_string());
            };
            let mut behavior = match behavior.lock() {
                Ok(b) => b,
                Err(_) => return Err("BoneFXDamage: BoneFXUpdate lock failed".to_string()),
            };
            let Some(bone_fx) = behavior.get_bone_fx_control_interface() else {
                return Err("BoneFXUpdate type mismatch".to_string());
            };
            func(bone_fx)
        }) else {
            return Err(format!("BoneFXDamage: Object {} not found", object_id));
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
        self.with_bone_fx_control(|bone_fx| {
            bone_fx.change_body_damage_state(old_state as u32, new_state as u32);
            Ok(())
        })
        .map_err(|err| err.into())
    }
}

impl BehaviorModuleInterface for BoneFXDamage {
    fn get_module_name(&self) -> &str {
        "BoneFXDamage"
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        BoneFXDamage::on_object_created(self).map_err(|err| err.into())
    }
}

impl Snapshotable for BoneFXDamage {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: u32 = 1;
        xfer.xfer_version_write(CURRENT_VERSION);
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: u32 = 1;
        if xfer.is_loading() {
            let version = xfer.xfer_version_read();
            if version > CURRENT_VERSION {
                return Err(format!(
                    "BoneFXDamage version {} > current version {}",
                    version, CURRENT_VERSION
                ));
            }
        } else {
            xfer.xfer_version_write(CURRENT_VERSION);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct BoneFXDamageModule {
    behavior: BoneFXDamage,
    module_name_key: EngineNameKeyType,
    module_data: Arc<DamageModuleData>,
}

impl BoneFXDamageModule {
    pub fn new(
        behavior: BoneFXDamage,
        module_name: &AsciiString,
        module_data: Arc<DamageModuleData>,
    ) -> Self {
        Self {
            behavior,
            module_name_key: NameKeyGenerator::name_to_key(module_name.as_str()),
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut BoneFXDamage {
        &mut self.behavior
    }
}

impl Snapshotable for BoneFXDamageModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for BoneFXDamageModule {
    fn get_module_name_key(&self) -> EngineNameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> EngineNameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        let _ = self.behavior.on_object_created();
    }
}
