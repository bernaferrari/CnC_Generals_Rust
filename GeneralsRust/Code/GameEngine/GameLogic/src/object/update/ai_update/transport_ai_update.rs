//! TransportAIUpdate - AI update logic for transports that relay attack orders to passengers.
//!
//! Ported from GameLogic/Object/Update/AIUpdate/TransportAIUpdate.cpp.

use std::any::Any;
use std::sync::{Arc, Mutex};

use crate::ai::CommandSourceType;
use crate::common::{DisabledType, KindOf, ObjectID};
use crate::helpers::TheGameLogic;
use crate::modules::{AIUpdateInterface, AIUpdateInterfaceExt};
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

#[derive(Debug, Clone)]
pub struct TransportAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
}

impl Default for TransportAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
        }
    }
}

impl TransportAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        // C++ TransportAIUpdate uses inherited AIUpdateModuleData::buildFieldParse.
        self.base.parse_from_ini(ini)
    }
}

impl ModuleData for TransportAIUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn is_ai_module_data(&self) -> bool {
        true
    }
}

impl Snapshotable for TransportAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.base.xfer(xfer)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct TransportAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<TransportAIUpdateModuleData>,
}

impl TransportAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<TransportAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for TransportAIUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for TransportAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TransportAIUpdate {
    owner_id: ObjectID,
}

impl TransportAIUpdate {
    pub fn new(owner_id: ObjectID) -> Self {
        Self { owner_id }
    }

    pub fn private_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if !matches!(
            cmd_source,
            CommandSourceType::FromPlayer | CommandSourceType::FromScript
        ) {
            return;
        }

        if !crate::object::registry::OBJECT_REGISTRY.contains(victim_id) {
            return;
        }
        self.relay_attack_to_passengers(|ai| {
            ai.ai_attack_object(
                victim_id,
                max_shots_to_fire,
                cmd_source,
            );
        });
    }

    pub fn private_force_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if !matches!(
            cmd_source,
            CommandSourceType::FromPlayer | CommandSourceType::FromScript
        ) {
            return;
        }

        if !crate::object::registry::OBJECT_REGISTRY.contains(victim_id) {
            return;
        }
        self.relay_attack_to_passengers(|ai| {
            ai.ai_force_attack_object(
                victim_id,
                max_shots_to_fire,
                cmd_source,
            );
        });
    }

    pub fn private_attack_position(
        &self,
        pos: &crate::common::Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if !matches!(
            cmd_source,
            CommandSourceType::FromPlayer | CommandSourceType::FromScript
        ) {
            return;
        }

        self.relay_attack_to_passengers(|ai| {
            ai.ai_attack_position(pos, max_shots_to_fire, cmd_source);
        });
    }

    fn relay_attack_to_passengers<F>(&self, mut action: F)
    where
        F: FnMut(&Arc<Mutex<dyn AIUpdateInterface>>),
    {
        let Some(contain) = crate::object::registry::OBJECT_REGISTRY
            .with_object(self.owner_id, |transport_guard| transport_guard.get_contain())
            .flatten()
        else {
            return;
        };
        let Ok(contain_guard) = contain.lock() else {
            return;
        };
        if !contain_guard.is_passenger_allowed_to_fire(None) {
            return;
        }

        for passenger_id in contain_guard.get_contained_objects().iter().copied() {
            let disabled = crate::object::registry::OBJECT_REGISTRY.with_object(passenger_id, |passenger_guard| {
                passenger_guard.is_kind_of(KindOf::PortableStructure)
                    && (passenger_guard.is_disabled_by_type(DisabledType::DisabledHacked)
                        || passenger_guard.is_disabled_by_type(DisabledType::DisabledEmp)
                        || passenger_guard.is_disabled_by_type(DisabledType::DisabledSubdued)
                        || passenger_guard.is_disabled_by_type(DisabledType::Paralyzed))
            });
            if disabled.unwrap_or(true) {
                continue;
            }
            let Some(ai) = crate::object::registry::OBJECT_REGISTRY
                .with_object(passenger_id, |passenger_guard| passenger_guard.get_ai_update_interface())
                .flatten()
            else {
                continue;
            };
            action(&ai);
        }
    }
}
