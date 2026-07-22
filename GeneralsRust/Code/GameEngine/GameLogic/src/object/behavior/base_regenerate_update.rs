//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BaseRegenerateUpdate.cpp`.
//!
//! BaseRegenerateUpdate - Building self-repair
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::{
    AsciiString, DamageInfo, DisabledMaskType, ModuleData, ObjectID, Real, UnsignedInt,
    XferVersion, LOGICFRAMES_PER_SECOND,
};
use crate::damage::{DamageInfoInput, DamageType, DeathType};
use crate::helpers::{TheGameLogic, TheGlobalData};
use crate::modules::{
    BehaviorModuleInterface, DamageModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct BaseRegenerateUpdateModuleData {
    pub base: BehaviorModuleData,
}

impl Default for BaseRegenerateUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(BaseRegenerateUpdateModuleData, base);

impl BaseRegenerateUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, BASE_REGENERATE_UPDATE_FIELDS)
    }
}

#[allow(dead_code)]
pub struct BaseRegenerateUpdate {
    object_id: ObjectID,
    module_data: Arc<BaseRegenerateUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
}

impl BaseRegenerateUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<BaseRegenerateUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
        })
    }
}

impl UpdateModuleInterface for BaseRegenerateUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(global_data) = TheGlobalData::get() else {
            return UpdateSleepTime::Forever;
        };

        if global_data.get_base_regen_health_percent_per_second() <= 0.0 {
            return UpdateSleepTime::Forever;
        }

        let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return UpdateSleepTime::Forever;
        };
        let obj = match object_arc.write() {
            Ok(guard) => guard,
            Err(_) => return UpdateSleepTime::Forever,
        };

        if obj.test_status(crate::common::ObjectStatusTypes::UnderConstruction) {
            return UpdateSleepTime::None;
        }

        if obj.test_status(crate::common::ObjectStatusTypes::Sold) {
            return UpdateSleepTime::Forever;
        }

        let body = obj.get_body_module();
        let Some(body) = body else {
            return UpdateSleepTime::Forever;
        };
        let body_guard = match body.lock() {
            Ok(guard) => guard,
            Err(_) => return UpdateSleepTime::Forever,
        };

        if body_guard.get_max_health() == body_guard.get_health() {
            return UpdateSleepTime::Forever;
        }

        const HEAL_RATE: UnsignedInt = 3;
        let amount = HEAL_RATE as Real
            * (body_guard.get_max_health()
                * global_data.get_base_regen_health_percent_per_second())
            / LOGICFRAMES_PER_SECOND as Real;
        drop(body_guard);

        let source_id = obj.get_id();
        let mut healing_info = DamageInfo {
            input: DamageInfoInput {
                damage_type: DamageType::Healing,
                death_type: DeathType::None,
                source_id,
                amount,
                ..Default::default()
            },
            ..Default::default()
        };
        healing_info.sync_from_input();

        if let Ok(mut body_guard) = body.lock() {
            let _ = body_guard.attempt_healing(&mut healing_info);
        }
        UpdateSleepTime::Frames(HEAL_RATE)
    }

    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::DISABLED_UNDERPOWERED
    }
}

impl BehaviorModuleInterface for BaseRegenerateUpdate {
    fn get_module_name(&self) -> &'static str {
        "BaseRegenerateUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return Ok(());
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(());
        };
        let sleep = TheGlobalData::get()
            .map(|g| {
                if g.get_base_regen_health_percent_per_second() == 0.0 {
                    UpdateSleepTime::Forever
                } else {
                    UpdateSleepTime::None
                }
            })
            .unwrap_or(UpdateSleepTime::Forever);
        TheGameLogic::set_wake_frame(obj.get_id(), sleep);
        Ok(())
    }
}

impl DamageModuleInterface for BaseRegenerateUpdate {
    fn receive_damage(
        &mut self,
        _object_id: crate::common::ObjectID,
        _damage: &DamageInfo,
    ) -> Real {
        0.0
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(global_data) = TheGlobalData::get() else {
            return Ok(());
        };
        if global_data.get_base_regen_health_percent_per_second() <= 0.0
            || damage_info.input.damage_type == crate::damage::DamageType::Healing
        {
            if let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            }) {
                if let Ok(obj) = obj_arc.read() {
                    TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::Forever);
                }
            }
            return Ok(());
        }

        if let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(obj) = obj_arc.read() {
                let delay = global_data.get_base_regen_delay();
                TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::from_u32(delay));
            }
        }
        Ok(())
    }
}

impl Snapshotable for BaseRegenerateUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("BaseRegenerateUpdate xfer version failed: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes BaseRegenerateUpdate through the common Module trait.
pub struct BaseRegenerateUpdateModule {
    behavior: BaseRegenerateUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<BaseRegenerateUpdateModuleData>,
}

impl BaseRegenerateUpdateModule {
    pub fn new(
        behavior: BaseRegenerateUpdate,
        module_name: &AsciiString,
        module_data: Arc<BaseRegenerateUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut BaseRegenerateUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for BaseRegenerateUpdateModule {
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

impl Module for BaseRegenerateUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

pub struct BaseRegenerateUpdateFactory;
impl BaseRegenerateUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(BaseRegenerateUpdate::new(thing, module_data)?))
    }
}

const BASE_REGENERATE_UPDATE_FIELDS: &[FieldParse<BaseRegenerateUpdateModuleData>] = &[];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processes_while_underpowered_like_cpp() {
        let object = Arc::new(RwLock::new(GameObject::new_test(9601, 100.0)));
        let data: Arc<dyn ModuleData> = Arc::new(BaseRegenerateUpdateModuleData::default());
        let update = BaseRegenerateUpdate::new(object, data).unwrap();

        assert_eq!(
            update.get_disabled_types_to_process(),
            DisabledMaskType::DISABLED_UNDERPOWERED
        );
    }
}
