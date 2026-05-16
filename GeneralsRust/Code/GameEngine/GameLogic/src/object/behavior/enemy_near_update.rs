//! EnemyNearUpdate - Rust conversion of C++ EnemyNearUpdate
//!
//! Reacts when an enemy is within vision range by toggling the ENEMYNEAR model condition.

use crate::ai::{search_qualifiers, THE_AI};
use crate::common::{
    AsciiString, Bool, ModuleData, UnsignedInt, XferVersion, LOGICFRAMES_PER_SECOND,
    MODELCONDITION_ENEMYNEAR,
};
use crate::helpers::get_game_logic_random_value;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct EnemyNearUpdateModuleData {
    pub base: BehaviorModuleData,
    pub enemy_scan_delay_time: UnsignedInt,
}

impl Default for EnemyNearUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            enemy_scan_delay_time: LOGICFRAMES_PER_SECOND,
        }
    }
}

crate::impl_behavior_module_data_via_base!(EnemyNearUpdateModuleData, base);

impl EnemyNearUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, ENEMY_NEAR_UPDATE_FIELDS)
    }
}

pub struct EnemyNearUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<EnemyNearUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    enemy_near: Bool,
    enemy_scan_delay: UnsignedInt,
}

impl EnemyNearUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = module_data
            .as_ref()
            .downcast_ref::<EnemyNearUpdateModuleData>()
            .ok_or("Invalid module data for EnemyNearUpdate")?;

        let mut enemy_scan_delay = 0;
        if data.enemy_scan_delay_time > 0 {
            enemy_scan_delay +=
                get_game_logic_random_value(0, data.enemy_scan_delay_time as i32) as u32;
        }

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(data.clone()),
            next_call_frame_and_phase: 0,
            enemy_near: false,
            enemy_scan_delay,
        })
    }

    fn check_for_enemies(&mut self) {
        if self.enemy_scan_delay == 0 {
            self.enemy_scan_delay = self.module_data.enemy_scan_delay_time;
            let Some(obj_arc) = self.object.upgrade() else {
                self.enemy_near = false;
                return;
            };
            let Ok(obj) = obj_arc.read() else {
                self.enemy_near = false;
                return;
            };
            let vision_range = obj.get_vision_range();
            let enemy = THE_AI.read().ok().and_then(|ai| {
                ai.find_closest_enemy(
                    obj.get_id(),
                    vision_range,
                    search_qualifiers::CAN_SEE,
                    None,
                    None,
                )
                .ok()
                .flatten()
            });
            self.enemy_near = enemy.is_some();
        } else {
            self.enemy_scan_delay = self.enemy_scan_delay.saturating_sub(1);
        }
    }
}

impl UpdateModuleInterface for EnemyNearUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let enemy_was_near = self.enemy_near;
        self.check_for_enemies();

        let Some(obj_arc) = self.object.upgrade() else {
            return UpdateSleepTime::None;
        };
        let mut obj = match obj_arc.write() {
            Ok(guard) => guard,
            Err(_) => return UpdateSleepTime::None,
        };

        if self.enemy_near && !enemy_was_near {
            obj.set_model_condition_state(MODELCONDITION_ENEMYNEAR);
        } else if !self.enemy_near && enemy_was_near {
            obj.clear_model_condition_state(MODELCONDITION_ENEMYNEAR);
        }

        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for EnemyNearUpdate {
    fn get_module_name(&self) -> &'static str {
        "EnemyNearUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for EnemyNearUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("EnemyNearUpdate xfer version failed: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_unsigned_int(&mut self.enemy_scan_delay)
            .map_err(|e| format!("EnemyNearUpdate xfer enemy_scan_delay failed: {:?}", e))?;
        game_engine::system::Xfer::xfer_bool(xfer, &mut self.enemy_near)
            .map_err(|e| format!("EnemyNearUpdate xfer enemy_near failed: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes EnemyNearUpdate through the common Module trait.
pub struct EnemyNearUpdateModule {
    behavior: EnemyNearUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<EnemyNearUpdateModuleData>,
}

impl EnemyNearUpdateModule {
    pub fn new(
        behavior: EnemyNearUpdate,
        module_name: &AsciiString,
        module_data: Arc<EnemyNearUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut EnemyNearUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for EnemyNearUpdateModule {
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

impl Module for EnemyNearUpdateModule {
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

pub struct EnemyNearUpdateFactory;

impl EnemyNearUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(EnemyNearUpdate::new(thing, module_data)?))
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_scan_delay_time(
    _ini: &mut INI,
    data: &mut EnemyNearUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.enemy_scan_delay_time = parse_duration_frames(tokens)?;
    Ok(())
}

const ENEMY_NEAR_UPDATE_FIELDS: &[FieldParse<EnemyNearUpdateModuleData>] = &[FieldParse {
    token: "ScanDelayTime",
    parse: parse_scan_delay_time,
}];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames(&["1500ms"]).expect("duration"), 45);
        assert_eq!(parse_duration_frames(&["1.5s"]).expect("duration"), 45);
    }
}
