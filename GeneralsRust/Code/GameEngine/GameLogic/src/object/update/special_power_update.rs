// FILE: special_power_update.rs
// Port of SpecialPowerUpdateModule.h and SpecialPowerUpdateModule.cpp
// Author: Rust Port
// Desc: Special power update module interface

use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::types::ModuleData;
use crate::common::xfer::{Xfer, XferExt, XferVersion};
use crate::common::{NameKeyGenerator, UnsignedInt};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_module::Waypoint;
use crate::object::Object as GameObject;
use bitflags::bitflags;
use game_engine::common::system::Snapshotable;

// Command option bitflags (matches C++ CommandOption)
bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SpecialPowerCommandOption: u32 {
        const NONE = 0x00000000;
        const NEED_TARGET_ENEMY_OBJECT = 0x00000001;
        const NEED_TARGET_NEUTRAL_OBJECT = 0x00000002;
        const NEED_TARGET_ALLY_OBJECT = 0x00000004;
        const NEED_TARGET_PRISONER = 0x00000008;
        const ALLOW_SHRUBBERY_TARGET = 0x00000010;
        const NEED_TARGET_POS = 0x00000020;
        const NEED_UPGRADE = 0x00000040;
        const NEED_SPECIAL_POWER_SCIENCE = 0x00000080;
        const OK_FOR_MULTI_SELECT = 0x00000100;
        const CONTEXTMODE_COMMAND = 0x00000200;
        const CHECK_LIKE = 0x00000400;
        const ALLOW_MINE_TARGET = 0x00000800;
        const ATTACK_OBJECTS_POSITION = 0x00001000;
        const OPTION_ONE = 0x00002000;
        const OPTION_TWO = 0x00004000;
        const OPTION_THREE = 0x00008000;
        const NOT_QUEUEABLE = 0x00010000;
        const SINGLE_USE_COMMAND = 0x00020000;
        const COMMAND_FIRED_BY_SCRIPT = 0x00040000;
        const SCRIPT_ONLY = 0x00080000;
        const IGNORES_UNDERPOWERED = 0x00100000;
        const USES_MINE_CLEARING_WEAPONSET = 0x00200000;
        const CAN_USE_WAYPOINTS = 0x00400000;
        const MUST_BE_STOPPED = 0x00800000;
    }
}

/// Base special power update module
#[derive(Debug, Clone)]
pub struct SpecialPowerUpdateModule {
    owner_object_id: u32,
    object: std::sync::Weak<std::sync::RwLock<GameObject>>,
    module_data: SpecialPowerUpdateModuleData,
    next_call_frame_and_phase: UnsignedInt,
}

impl SpecialPowerUpdateModule {
    /// Create a new special power update module
    pub fn new(
        owner_object_id: u32,
        object: std::sync::Weak<std::sync::RwLock<GameObject>>,
    ) -> Self {
        Self {
            owner_object_id,
            object,
            module_data: SpecialPowerUpdateModuleData::default(),
            next_call_frame_and_phase: 0,
        }
    }

    /// Get the owner object ID
    pub fn get_owner_object_id(&self) -> u32 {
        self.owner_object_id
    }

    /// Match C++ SpecialPowerUpdateModule::doesSpecialPowerUpdatePassScienceTest.
    pub fn does_special_power_update_pass_science_test(&self) -> bool {
        let extra_required_science = self.get_extra_required_science();
        if extra_required_science == SCIENCE_INVALID {
            return true;
        }

        let obj_arc = self
            .object
            .upgrade()
            .or_else(|| OBJECT_REGISTRY.get_object(self.owner_object_id));
        let Some(obj_arc) = obj_arc else {
            return false;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return false;
        };
        does_special_power_update_pass_science_test_for_object(&obj_guard, extra_required_science)
    }

    /// C++ default: SCIENCE_INVALID (override in derived modules).
    pub fn get_extra_required_science(&self) -> ScienceType {
        self.module_data.extra_required_science
    }

    pub fn set_extra_required_science(&mut self, science: ScienceType) {
        self.module_data.extra_required_science = science;
    }

    pub fn set_module_data(&mut self, data: SpecialPowerUpdateModuleData) {
        self.module_data = data;
    }

    /// Match C++ SpecialPowerUpdateModule::crc (no additional state).
    pub fn crc(&self, xfer: &mut dyn Xfer) {
        xfer.xfer_version_write(1);
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        if let Err(err) = xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase) {
            panic!("SpecialPowerUpdateModule::crc failed to xfer base state: {err}");
        }
    }

    /// Match C++ SpecialPowerUpdateModule::xfer.
    pub fn save(&self, xfer: &mut dyn Xfer) {
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer.xfer_version_write(1);
        if let Err(err) = xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase) {
            panic!("SpecialPowerUpdateModule::save failed to xfer base state: {err}");
        }
    }

    /// Match C++ SpecialPowerUpdateModule::xfer.
    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let _version = xfer.xfer_version_read();
        if let Err(err) = xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase) {
            panic!("SpecialPowerUpdateModule::load failed to xfer base state: {err}");
        }
    }

    /// Match C++ SpecialPowerUpdateModule::loadPostProcess (no-op).
    pub fn load_post_process(&mut self) {}
}

impl crate::modules::SpecialPowerUpdateInterface for SpecialPowerUpdateModule {
    fn does_special_power_update_pass_science_test(&self) -> bool {
        SpecialPowerUpdateModule::does_special_power_update_pass_science_test(self)
    }

    fn get_extra_required_science(&self) -> ScienceType {
        SpecialPowerUpdateModule::get_extra_required_science(self)
    }

    fn initiate_intent_to_do_special_power(
        &mut self,
        _special_power_template: &crate::object::SpecialPowerTemplate,
        _target_obj: Option<crate::common::ObjectID>,
        _target_pos: Option<&crate::common::Coord3D>,
        _waypoint: Option<&Waypoint>,
        _command_options: SpecialPowerCommandOption,
    ) -> bool {
        // Base implementation does nothing, returns false
        false
    }

    fn is_special_ability(&self) -> bool {
        false
    }

    fn is_special_power(&self) -> bool {
        true
    }

    fn is_active(&self) -> bool {
        false
    }

    fn get_command_option(&self) -> SpecialPowerCommandOption {
        SpecialPowerCommandOption::NONE
    }

    fn does_special_power_have_overridable_destination_active(&self) -> bool {
        false
    }

    fn does_special_power_have_overridable_destination(&self) -> bool {
        false
    }

    fn set_special_power_overridable_destination(&mut self, _location: &crate::common::Coord3D) {
        // Base implementation does nothing
    }

    fn is_power_currently_in_use(
        &self,
        _command: Option<&crate::command_button::CommandButton>,
    ) -> bool {
        false
    }
}

/// Module data for SpecialPowerUpdateModule
#[derive(Debug, Clone)]
pub struct SpecialPowerUpdateModuleData {
    pub base: BehaviorModuleData,
    pub extra_required_science: ScienceType,
}

impl Default for SpecialPowerUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            extra_required_science: SCIENCE_INVALID,
        }
    }
}

crate::impl_behavior_module_data_via_base!(SpecialPowerUpdateModuleData, base);

/// Factory for SpecialPowerUpdateModule
pub struct SpecialPowerUpdateModuleFactory;

impl SpecialPowerUpdateModuleFactory {
    pub fn create_module(
        &self,
        object: std::sync::Weak<std::sync::RwLock<GameObject>>,
        module_data: std::sync::Arc<dyn ModuleData>,
    ) -> Box<dyn crate::modules::BehaviorModuleInterface> {
        let object_id = object
            .upgrade()
            .and_then(|obj| obj.read().ok().map(|guard| guard.id))
            .unwrap_or(crate::common::types::INVALID_ID);

        let mut module = SpecialPowerUpdateModule::new(object_id, object.clone());

        // Init from data
        if let Some(data) = module_data
            .as_any()
            .downcast_ref::<SpecialPowerUpdateModuleData>()
        {
            module.set_module_data(data.clone());
        }

        Box::new(module)
    }
}

impl crate::modules::UpdateModuleInterface for SpecialPowerUpdateModule {
    fn update(
        &mut self,
    ) -> Result<crate::modules::UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        Ok(crate::modules::UPDATE_SLEEP_FOREVER)
    }
}

impl crate::modules::BehaviorModuleInterface for SpecialPowerUpdateModule {
    fn get_module_name(&self) -> &'static str {
        "SpecialPowerUpdateModule"
    }

    fn get_update(&mut self) -> Option<&mut dyn crate::modules::UpdateModuleInterface> {
        Some(self)
    }

    fn get_special_power_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::SpecialPowerUpdateInterface> {
        Some(self)
    }
}

impl Snapshotable for SpecialPowerUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl game_engine::common::thing::module::Module for SpecialPowerUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        NameKeyGenerator::name_to_key("SpecialPowerUpdate")
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(&self.module_data)
    }

    fn get_module_data(&self) -> &dyn game_engine::common::thing::module::ModuleData {
        &self.module_data
    }
}

/// Shared helper for SpecialPowerUpdateModule science checks (C++ SpecialPowerUpdateModule::doesSpecialPowerUpdatePassScienceTest).
pub fn does_special_power_update_pass_science_test_for_object(
    object: &GameObject,
    extra_required_science: ScienceType,
) -> bool {
    if extra_required_science == SCIENCE_INVALID {
        return true;
    }

    let Some(player_arc) = object.get_controlling_player() else {
        return false;
    };
    let Ok(player_guard) = player_arc.read() else {
        return false;
    };
    player_guard.has_science(extra_required_science)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_power_update_module_creation() {
        let module = SpecialPowerUpdateModule::new(1, std::sync::Weak::new());

        assert_eq!(module.get_owner_object_id(), 1);
    }

    #[test]
    fn test_science_test() {
        let module = SpecialPowerUpdateModule::new(1, std::sync::Weak::new());

        // With no extra science requirement, should pass
        assert!(module.does_special_power_update_pass_science_test());
    }

    #[test]
    fn test_extra_required_science_setter() {
        let mut module = SpecialPowerUpdateModule::new(1, std::sync::Weak::new());
        module.set_extra_required_science(42);
        assert_eq!(module.get_extra_required_science(), 42);
    }
}
