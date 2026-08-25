//! WanderAIUpdate - AI update logic that issues random moves while idle.
//!
//! Ported from GameLogic/Object/Update/AIUpdate/WanderAIUpdate.cpp.

use std::any::Any;
use std::sync::Arc;

use crate::common::{Coord3D, ObjectID};
use crate::helpers::{TheGameLogic, get_game_logic_random_value};
use crate::modules::AIUpdateInterface;
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use game_engine::common::ini::{INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

#[derive(Debug, Clone)]
pub struct WanderAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
}

impl Default for WanderAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
        }
    }
}

impl WanderAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        // C++ WanderAIUpdate uses inherited AIUpdateModuleData::buildFieldParse.
        self.base.parse_from_ini(ini)
    }
}

impl ModuleData for WanderAIUpdateModuleData {
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

impl Snapshotable for WanderAIUpdateModuleData {
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
pub struct WanderAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<WanderAIUpdateModuleData>,
}

impl WanderAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<WanderAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for WanderAIUpdateModule {
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

impl Snapshotable for WanderAIUpdateModule {
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
pub struct WanderAIUpdate {
    owner_id: ObjectID,
}

impl WanderAIUpdate {
    pub fn new(owner_id: ObjectID) -> Self {
        Self { owner_id }
    }

    pub fn update(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !ai.is_idle() {
            return Ok(());
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };
        let pos = owner_guard.get_position();
        let dx = get_game_logic_random_value(5, 50) as f32;
        let dy = get_game_logic_random_value(5, 50) as f32;
        let dest = Coord3D::new(pos.x + dx, pos.y + dy, pos.z);
        let _ = ai.ai_move_to_position(&dest);

        Ok(())
    }
}
