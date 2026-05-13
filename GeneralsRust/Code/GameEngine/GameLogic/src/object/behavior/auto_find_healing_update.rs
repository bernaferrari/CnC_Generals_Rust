//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/AutoFindHealingUpdate.cpp`.
//!
//! AutoFindHealingUpdate - Automatically finds and moves to healers when damaged
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{
    AsciiString, KindOf, ModuleData, ObjectID, Real, UnsignedInt, XferVersion, FROM_CENTER_2D,
};
use crate::helpers::ThePartitionManager;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use crate::player::PlayerType;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct AutoFindHealingUpdateModuleData {
    pub base: BehaviorModuleData,
    pub scan_frames: UnsignedInt,
    pub scan_range: Real,
    pub never_heal: Real,
    pub always_heal: Real,
}

impl Default for AutoFindHealingUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            scan_frames: 0,
            scan_range: 0.0,
            never_heal: 0.95,
            always_heal: 0.25,
        }
    }
}

crate::impl_behavior_module_data_via_base!(AutoFindHealingUpdateModuleData, base);

impl AutoFindHealingUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, AUTO_FIND_HEALING_UPDATE_FIELDS)
    }
}

pub struct AutoFindHealingUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<AutoFindHealingUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    next_scan_frames: i32,
}

impl AutoFindHealingUpdate {
    pub fn new_typed(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<AutoFindHealingUpdateModuleData>,
    ) -> Self {
        Self {
            object: Arc::downgrade(&object),
            module_data,
            next_call_frame_and_phase: 0,
            next_scan_frames: 0,
        }
    }

    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<AutoFindHealingUpdateModuleData>()
            .ok_or("Invalid AutoFindHealingUpdate module data")?;

        Ok(Self::new_typed(object, Arc::new(specific_data.clone())))
    }
}

impl UpdateModuleInterface for AutoFindHealingUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let Some(object_arc) = self.object.upgrade() else {
            return UpdateSleepTime::None;
        };
        let Ok(obj) = object_arc.read() else {
            return UpdateSleepTime::None;
        };

        let is_human_player = if let Some(player) = obj.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                player_guard.get_player_type() == PlayerType::Human
            } else {
                false
            }
        } else {
            false
        };
        if is_human_player {
            return UpdateSleepTime::None;
        }

        if self.next_scan_frames > 0 {
            self.next_scan_frames -= 1;
            return UpdateSleepTime::None;
        }
        self.next_scan_frames = self.module_data.scan_frames as i32;

        let Some(ai) = obj.get_ai_update_interface() else {
            return UpdateSleepTime::None;
        };

        let Some(body) = obj.get_body_module() else {
            return UpdateSleepTime::None;
        };
        let Ok(body_guard) = body.lock() else {
            return UpdateSleepTime::None;
        };

        if body_guard.get_health() > body_guard.get_max_health() * self.module_data.never_heal {
            return UpdateSleepTime::None;
        }

        let Ok(mut ai_guard) = ai.lock() else {
            return UpdateSleepTime::None;
        };
        if !ai_guard.is_idle() {
            return UpdateSleepTime::None;
        }

        let target = self.scan_closest_target(&*obj);
        if let Some(target) = target {
            let mut params =
                AiCommandParams::new(AiCommandType::GetHealed, CommandSourceType::FromAi);
            params.obj = Some(target);
            let _ = ai_guard.execute_command(&params);
        }

        UpdateSleepTime::None
    }
}

impl AutoFindHealingUpdate {
    fn scan_closest_target(&self, me: &GameObject) -> Option<ObjectID> {
        let Some(partition) = ThePartitionManager::get() else {
            return None;
        };
        let candidates =
            partition.get_objects_in_range(me.get_position(), self.module_data.scan_range);
        let mut best_target = None;
        let mut closest_dist = 0.0;

        for other_id in candidates {
            let Some(other_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(other_id)
            else {
                continue;
            };
            let Ok(other_guard) = other_arc.read() else {
                continue;
            };
            if !other_guard.is_kind_of(KindOf::HealPad) {
                continue;
            }
            let dist = ThePartitionManager::get_distance_squared(me, &*other_guard, FROM_CENTER_2D);
            if best_target.is_none() {
                best_target = Some(other_guard.get_id());
                closest_dist = dist;
                continue;
            }
            if dist < closest_dist {
                best_target = Some(other_guard.get_id());
                closest_dist = dist;
            }
        }

        best_target
    }
}

impl BehaviorModuleInterface for AutoFindHealingUpdate {
    fn get_module_name(&self) -> &'static str {
        "AutoFindHealingUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

pub struct AutoFindHealingUpdateFactory;
impl AutoFindHealingUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(AutoFindHealingUpdate::new(thing, module_data)?))
    }
}

impl Snapshotable for AutoFindHealingUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AutoFindHealingUpdate xfer version failed: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        let mut next_scan_frames = self.next_scan_frames;
        xfer.xfer_int(&mut next_scan_frames).map_err(|e| {
            format!(
                "AutoFindHealingUpdate xfer next_scan_frames failed: {:?}",
                e
            )
        })?;
        self.next_scan_frames = next_scan_frames;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes AutoFindHealingUpdate through the common Module trait.
pub struct AutoFindHealingUpdateModule {
    behavior: AutoFindHealingUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<AutoFindHealingUpdateModuleData>,
}

impl AutoFindHealingUpdateModule {
    pub fn new(
        behavior: AutoFindHealingUpdate,
        module_name: &AsciiString,
        module_data: Arc<AutoFindHealingUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut AutoFindHealingUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for AutoFindHealingUpdateModule {
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

impl Module for AutoFindHealingUpdateModule {
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

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = required_value(tokens)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_scan_rate(
    _ini: &mut INI,
    data: &mut AutoFindHealingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.scan_frames = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_scan_range(
    _ini: &mut INI,
    data: &mut AutoFindHealingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.scan_range = INI::parse_real(required_value(tokens)?)?;
    Ok(())
}

fn parse_never_heal(
    _ini: &mut INI,
    data: &mut AutoFindHealingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.never_heal = INI::parse_real(required_value(tokens)?)?;
    Ok(())
}

fn parse_always_heal(
    _ini: &mut INI,
    data: &mut AutoFindHealingUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.always_heal = INI::parse_real(required_value(tokens)?)?;
    Ok(())
}

const AUTO_FIND_HEALING_UPDATE_FIELDS: &[FieldParse<AutoFindHealingUpdateModuleData>] = &[
    FieldParse {
        token: "ScanRate",
        parse: parse_scan_rate,
    },
    FieldParse {
        token: "ScanRange",
        parse: parse_scan_range,
    },
    FieldParse {
        token: "NeverHeal",
        parse: parse_never_heal,
    },
    FieldParse {
        token: "AlwaysHeal",
        parse: parse_always_heal,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames(&["1500ms"]).expect("duration"), 45);
        assert_eq!(parse_duration_frames(&["1.5s"]).expect("duration"), 45);
    }

    #[test]
    fn parse_fields_accept_ini_equals_token() {
        let mut ini = INI::new();
        let mut data = AutoFindHealingUpdateModuleData::default();

        parse_scan_rate(&mut ini, &mut data, &["=", "1500ms"]).expect("scan rate");
        parse_scan_range(&mut ini, &mut data, &["=", "125.5"]).expect("scan range");
        parse_never_heal(&mut ini, &mut data, &["=", "0.8"]).expect("never heal");
        parse_always_heal(&mut ini, &mut data, &["=", "0.2"]).expect("always heal");

        assert_eq!(data.scan_frames, 45);
        assert!((data.scan_range - 125.5).abs() < f32::EPSILON);
        assert!((data.never_heal - 0.8).abs() < f32::EPSILON);
        assert!((data.always_heal - 0.2).abs() < f32::EPSILON);
    }
}
