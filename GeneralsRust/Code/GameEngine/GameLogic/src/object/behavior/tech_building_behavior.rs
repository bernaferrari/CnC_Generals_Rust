//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/TechBuildingBehavior.cpp`.
//!
//! TechBuildingBehavior - Rust conversion of C++ TechBuildingBehavior
//!
//! Tech building basic behavior
//! Original Author: Colin Day, October 2002
//! Rust conversion: 2025

use crate::common::{Bool, ModuleData, ObjectID, UnsignedInt, XferVersion};
use crate::helpers::{TheFXListStore, TheGameLogic};
use crate::modules::{
    BehaviorModuleInterface, DieModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::any::Any;
use std::sync::{Arc, RwLock, Weak};

use crate::common::ModelConditionFlags;
use crate::damage::DamageInfo;
use crate::effects::FXList;
use crate::player::{player_list, Player};

/// Module data for TechBuildingBehavior
#[derive(Debug, Clone)]
pub struct TechBuildingBehaviorModuleData {
    pub base: BehaviorModuleData,
    /// FXList to play when building is owned and updated
    pub pulse_fx: Option<Arc<FXList>>,
    /// How frequently to play the pulse FX
    pub pulse_fx_rate: UnsignedInt,
}

impl TechBuildingBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            pulse_fx: None,
            pulse_fx_rate: 0,
        }
    }
}

impl Default for TechBuildingBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

crate::impl_behavior_module_data_via_base!(TechBuildingBehaviorModuleData, base);

impl TechBuildingBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, TECH_BUILDING_BEHAVIOR_FIELDS)
    }
}

fn parse_pulse_fx(
    _ini: &mut INI,
    data: &mut TechBuildingBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.pulse_fx = TheFXListStore::find_fx_list(token);
    Ok(())
}

fn parse_pulse_fx_rate(
    _ini: &mut INI,
    data: &mut TechBuildingBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.pulse_fx_rate = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

const TECH_BUILDING_BEHAVIOR_FIELDS: &[FieldParse<TechBuildingBehaviorModuleData>] = &[
    FieldParse {
        token: "PulseFX",
        parse: parse_pulse_fx,
    },
    FieldParse {
        token: "PulseFXRate",
        parse: parse_pulse_fx_rate,
    },
];

/// Main TechBuildingBehavior implementation
#[derive(Debug)]
pub struct TechBuildingBehavior {
    object_id: ObjectID,
    module_data: Arc<TechBuildingBehaviorModuleData>,
    next_call_frame_and_phase: UnsignedInt,
}

impl TechBuildingBehavior {
    pub fn new(
        thing: Arc<RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = {
            let data_ref = module_data
                .as_any()
                .downcast_ref::<TechBuildingBehaviorModuleData>()
                .ok_or("Invalid module data type")?;
            data_ref.clone()
        };

        let object_id = thing.read().map(|guard| guard.get_id()).unwrap_or_default();
        TheGameLogic::set_wake_frame(object_id, UpdateSleepTime::None);

        Ok(Self {
            object_id: thing
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(data),
            next_call_frame_and_phase: 0,
        })
    }

    fn get_object_id(&self) -> crate::common::ObjectID {
        self.object_id
    }

    fn with_object<R>(
        &self,
        f: impl FnOnce(&Object) -> R,
    ) -> Result<R, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return Err("Object not set".into());
        }
        crate::object::registry::OBJECT_REGISTRY
            .with_object(id, f)
            .ok_or_else(|| "Object not found".into())
    }

    fn with_object_mut<R>(
        &self,
        f: impl FnOnce(&mut Object) -> R,
    ) -> Result<R, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return Err("Object not set".into());
        }
        crate::object::registry::OBJECT_REGISTRY
            .with_object_mut(id, f)
            .ok_or_else(|| "Object not found".into())
    }

    fn get_object(&self) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return Err("Object not set".into());
        }
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
            .ok_or_else(|| "Object not found".into())
    }

    /// Handle capture events (when ownership changes)
    pub fn on_capture(
        &mut self,
        _old_owner: Option<&Player>,
        _new_owner: Option<&Player>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wake up next frame so we can re-evaluate our captured status
        TheGameLogic::set_wake_frame(self.object_id, UpdateSleepTime::None);
        Ok(())
    }
}

impl UpdateModuleInterface for TechBuildingBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let data = &self.module_data;

        // Update our model condition for the captured status
        let is_playable_side = self
            .with_object(|obj_guard| {
                obj_guard
                    .get_controlling_player()
                    .and_then(|player| player.read().ok().map(|p| p.is_playable_side()))
            })?
            .unwrap_or(false);

        self.with_object_mut(|obj_guard| {
            if is_playable_side {
                obj_guard.set_model_condition_state(ModelConditionFlags::CAPTURED);
            } else {
                obj_guard.clear_model_condition_state(ModelConditionFlags::CAPTURED);
            }
        })?;
        let captured = is_playable_side;

        // If we have a pulse fx, and are owned, sleep only a little while, otherwise sleep forever
        if let Some(pulse_fx) = &data.pulse_fx {
            if data.pulse_fx_rate > 0 && captured {
                // Play the pulse FX
                let object = self.get_object()?;
                pulse_fx.do_fx_obj(&object, None)?;
                return Ok(UpdateSleepTime::from_u32(data.pulse_fx_rate));
            }
        }

        // Now sleep forever
        Ok(UpdateSleepTime::Forever)
    }
}

impl DieModuleInterface for TechBuildingBehavior {
    fn on_die(
        &mut self,
        _damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Put us on the team of the neutral player so no player has any bonus from us
        let neutral_team = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_neutral_player())
            .and_then(|neutral| neutral.read().ok().and_then(|n| n.get_default_team()));

        self.with_object_mut(
            |obj_guard| -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                obj_guard.clear_model_condition_state(ModelConditionFlags::CAPTURED);
                if let Some(team) = neutral_team.clone() {
                    obj_guard.set_team(Some(team))?;
                }
                Ok(())
            },
        )??;

        Ok(())
    }
}

impl BehaviorModuleInterface for TechBuildingBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }

    fn on_capture(
        &mut self,
        _old_owner: Option<&Arc<RwLock<Player>>>,
        _new_owner: Option<&Arc<RwLock<Player>>>,
    ) {
        let _ = self.on_capture(None, None);
    }
}

impl Snapshotable for TechBuildingBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
            .map_err(|e| format!("Failed to xfer update module base state: {}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|e| format!("Failed to xfer update module base state: {}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Module wrapper for TechBuildingBehavior.
pub struct TechBuildingBehaviorModule {
    behavior: TechBuildingBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<TechBuildingBehaviorModuleData>,
}

impl TechBuildingBehaviorModule {
    pub fn new(
        behavior: TechBuildingBehavior,
        module_name: &game_engine::common::rts::AsciiString,
        module_data: Arc<TechBuildingBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut TechBuildingBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for TechBuildingBehaviorModule {
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

impl Module for TechBuildingBehaviorModule {
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

// Thread safety
unsafe impl Send for TechBuildingBehavior {}
unsafe impl Sync for TechBuildingBehavior {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_data_creation() {
        let data = TechBuildingBehaviorModuleData::new();
        assert!(data.pulse_fx.is_none());
        assert_eq!(data.pulse_fx_rate, 0);
    }

    #[test]
    fn test_module_data_default() {
        let data = TechBuildingBehaviorModuleData::default();
        assert!(data.pulse_fx.is_none());
        assert_eq!(data.pulse_fx_rate, 0);
    }

    #[test]
    fn tech_building_fields_use_cpp_ini_token_handling() {
        let mut ini = INI::new();
        let mut data = TechBuildingBehaviorModuleData::default();

        parse_pulse_fx(&mut ini, &mut data, &["=", "TechPulseFX"]).unwrap();
        parse_pulse_fx_rate(&mut ini, &mut data, &["=", "1s"]).unwrap();

        assert!(data.pulse_fx.is_none());
        assert_eq!(data.pulse_fx_rate, 30);
    }

    #[test]
    fn tech_building_rejects_missing_values_like_cpp_parsers() {
        let mut ini = INI::new();
        let mut data = TechBuildingBehaviorModuleData::default();

        assert!(matches!(
            parse_pulse_fx(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
        assert!(matches!(
            parse_pulse_fx_rate(&mut ini, &mut data, &["="]),
            Err(INIError::InvalidData)
        ));
    }
}
