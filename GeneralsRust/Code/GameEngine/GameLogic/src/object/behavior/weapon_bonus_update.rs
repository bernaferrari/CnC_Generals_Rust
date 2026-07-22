//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/WeaponBonusUpdate.cpp`.
//!
//! WeaponBonusUpdate - Weapon damage and range bonuses
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::WeaponBonusConditionType;
use crate::common::{
    AsciiString, KindOfMaskType, ModuleData, ObjectID, Real, UnsignedInt, XferVersion,
    KIND_OF_MASK_NONE,
};
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::auto_heal_behavior::parse_kind_of_mask;
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct WeaponBonusUpdateModuleData {
    pub base: BehaviorModuleData,
    pub required_affect_kind_of: KindOfMaskType,
    pub forbidden_affect_kind_of: KindOfMaskType,
    pub bonus_duration: UnsignedInt,
    pub bonus_delay: UnsignedInt,
    pub bonus_range: Real,
    pub bonus_condition_type: WeaponBonusConditionType,
}

impl Default for WeaponBonusUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            required_affect_kind_of: KIND_OF_MASK_NONE,
            forbidden_affect_kind_of: KIND_OF_MASK_NONE,
            bonus_duration: 0,
            bonus_delay: 0,
            bonus_range: 0.0,
            bonus_condition_type: WeaponBonusConditionType::Invalid,
        }
    }
}

impl WeaponBonusUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, WEAPON_BONUS_UPDATE_FIELDS)
    }
}

impl Snapshotable for WeaponBonusUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

crate::impl_legacy_module_data_via_base!(WeaponBonusUpdateModuleData, base);

#[derive(Debug)]
pub struct WeaponBonusUpdate {
    object_id: ObjectID,
    module_data: Arc<WeaponBonusUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
}

impl WeaponBonusUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<WeaponBonusUpdateModuleData>()
            .ok_or("Invalid module data")?;

        if let Ok(obj) = object.read() {
            TheGameLogic::set_wake_frame(obj.get_id(), UpdateSleepTime::None);
        }

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

impl UpdateModuleInterface for WeaponBonusUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let Some(obj_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) else {
            return Ok(UpdateSleepTime::Forever);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(UpdateSleepTime::None);
        };

        let Some(partition) = ThePartitionManager::get() else {
            return Ok(UpdateSleepTime::from_u32(self.module_data.bonus_delay));
        };

        let candidates =
            partition.get_objects_in_range(obj.get_position(), self.module_data.bonus_range);
        let same_map_status = obj.is_off_map();
        for id in candidates {
            let required = self.module_data.required_affect_kind_of;
            let forbidden = self.module_data.forbidden_affect_kind_of;
            let condition = self.module_data.bonus_condition_type;
            let duration = self.module_data.bonus_duration;
            let _ = OBJECT_REGISTRY.with_object_mut(id, |target| {
                if target.is_effectively_dead() {
                    return;
                }

                let relationship = obj.relationship_to(target);
                if !matches!(relationship, crate::common::Relationship::Allies) {
                    return;
                }

                if target.is_off_map() != same_map_status {
                    return;
                }

                if target.is_kind_of_multi(required, forbidden) {
                    target.do_temp_weapon_bonus(condition, duration);
                }

                if let Some(contain) = target.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        for contained_id in contain_guard.get_contained_objects() {
                            let _ = OBJECT_REGISTRY.with_object_mut(*contained_id, |contained| {
                                if contained.is_kind_of_multi(required, forbidden) {
                                    contained.do_temp_weapon_bonus(condition, duration);
                                }
                            });
                        }
                    }
                }
            });
        }

        Ok(UpdateSleepTime::from_u32(self.module_data.bonus_delay))
    }
}

impl BehaviorModuleInterface for WeaponBonusUpdate {
    fn get_module_name(&self) -> &'static str {
        "WeaponBonusUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for WeaponBonusUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
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

/// Glue that exposes WeaponBonusUpdate through the common Module trait.
pub struct WeaponBonusUpdateModule {
    behavior: WeaponBonusUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<WeaponBonusUpdateModuleData>,
}

impl WeaponBonusUpdateModule {
    pub fn new(
        behavior: WeaponBonusUpdate,
        module_name: &AsciiString,
        module_data: Arc<WeaponBonusUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut WeaponBonusUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for WeaponBonusUpdateModule {
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

impl Module for WeaponBonusUpdateModule {
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

pub struct WeaponBonusUpdateFactory;
impl WeaponBonusUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(WeaponBonusUpdate::new(thing, module_data)?))
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = required_value(tokens)?;
    INI::parse_duration_unsigned_int(token)
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_bonus_condition(token: &str) -> Result<WeaponBonusConditionType, INIError> {
    let idx = INI::parse_index_list(token, WEAPON_BONUS_NAMES)?;
    WEAPON_BONUS_TYPES
        .get(idx)
        .copied()
        .ok_or(INIError::InvalidData)
}

fn parse_required_affect_kind_of(
    _ini: &mut INI,
    data: &mut WeaponBonusUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.required_affect_kind_of = parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_forbidden_affect_kind_of(
    _ini: &mut INI,
    data: &mut WeaponBonusUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.forbidden_affect_kind_of = parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_bonus_duration(
    _ini: &mut INI,
    data: &mut WeaponBonusUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.bonus_duration = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_bonus_delay(
    _ini: &mut INI,
    data: &mut WeaponBonusUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.bonus_delay = parse_duration_frames(tokens)?;
    Ok(())
}

fn parse_bonus_range(
    _ini: &mut INI,
    data: &mut WeaponBonusUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.bonus_range = INI::parse_real(token)?;
    Ok(())
}

fn parse_bonus_condition_type(
    _ini: &mut INI,
    data: &mut WeaponBonusUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.bonus_condition_type = parse_bonus_condition(required_value(tokens)?)?;
    Ok(())
}

const WEAPON_BONUS_UPDATE_FIELDS: &[FieldParse<WeaponBonusUpdateModuleData>] = &[
    FieldParse {
        token: "RequiredAffectKindOf",
        parse: parse_required_affect_kind_of,
    },
    FieldParse {
        token: "ForbiddenAffectKindOf",
        parse: parse_forbidden_affect_kind_of,
    },
    FieldParse {
        token: "BonusDuration",
        parse: parse_bonus_duration,
    },
    FieldParse {
        token: "BonusDelay",
        parse: parse_bonus_delay,
    },
    FieldParse {
        token: "BonusRange",
        parse: parse_bonus_range,
    },
    FieldParse {
        token: "BonusConditionType",
        parse: parse_bonus_condition_type,
    },
];

const WEAPON_BONUS_TYPES: &[WeaponBonusConditionType] = &[
    WeaponBonusConditionType::Garrisoned,
    WeaponBonusConditionType::Horde,
    WeaponBonusConditionType::ContinuousFireMean,
    WeaponBonusConditionType::ContinuousFireFast,
    WeaponBonusConditionType::Nationalism,
    WeaponBonusConditionType::PlayerUpgrade,
    WeaponBonusConditionType::DroneSpotting,
    WeaponBonusConditionType::DemoralizedObsolete,
    WeaponBonusConditionType::Enthusiastic,
    WeaponBonusConditionType::Veteran,
    WeaponBonusConditionType::Elite,
    WeaponBonusConditionType::Hero,
    WeaponBonusConditionType::BattlePlanBombardment,
    WeaponBonusConditionType::BattlePlanHoldTheLine,
    WeaponBonusConditionType::BattlePlanSearchAndDestroy,
    WeaponBonusConditionType::Subliminal,
    WeaponBonusConditionType::SoloHumanEasy,
    WeaponBonusConditionType::SoloHumanNormal,
    WeaponBonusConditionType::SoloHumanHard,
    WeaponBonusConditionType::SoloAiEasy,
    WeaponBonusConditionType::SoloAiNormal,
    WeaponBonusConditionType::SoloAiHard,
    WeaponBonusConditionType::TargetFaerieFire,
    WeaponBonusConditionType::Fanaticism,
    WeaponBonusConditionType::FrenzyOne,
    WeaponBonusConditionType::FrenzyTwo,
    WeaponBonusConditionType::FrenzyThree,
];

const WEAPON_BONUS_NAMES: &[&str] = &[
    "GARRISONED",
    "HORDE",
    "CONTINUOUS_FIRE_MEAN",
    "CONTINUOUS_FIRE_FAST",
    "NATIONALISM",
    "PLAYER_UPGRADE",
    "DRONE_SPOTTING",
    "DEMORALIZED_OBSOLETE",
    "ENTHUSIASTIC",
    "VETERAN",
    "ELITE",
    "HERO",
    "BATTLEPLAN_BOMBARDMENT",
    "BATTLEPLAN_HOLDTHELINE",
    "BATTLEPLAN_SEARCHANDDESTROY",
    "SUBLIMINAL",
    "SOLO_HUMAN_EASY",
    "SOLO_HUMAN_NORMAL",
    "SOLO_HUMAN_HARD",
    "SOLO_AI_EASY",
    "SOLO_AI_NORMAL",
    "SOLO_AI_HARD",
    "TARGET_FAERIE_FIRE",
    "FANATICISM",
    "FRENZY_ONE",
    "FRENZY_TWO",
    "FRENZY_THREE",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weapon_bonus_fields_use_cpp_ini_token_handling() {
        let mut data = WeaponBonusUpdateModuleData::default();
        let mut ini = INI::new();

        parse_bonus_duration(&mut ini, &mut data, &["=", "1500ms"]).expect("duration");
        parse_bonus_delay(&mut ini, &mut data, &["=", "1.5s"]).expect("delay");
        parse_bonus_range(&mut ini, &mut data, &["=", "125.5f"]).expect("range");
        parse_bonus_condition_type(&mut ini, &mut data, &["=", "FRENZY_TWO"]).expect("condition");

        assert_eq!(data.bonus_duration, 45);
        assert_eq!(data.bonus_delay, 45);
        assert!((data.bonus_range - 125.5).abs() < f32::EPSILON);
        assert_eq!(
            data.bonus_condition_type,
            WeaponBonusConditionType::FrenzyTwo
        );
    }

    #[test]
    fn weapon_bonus_condition_rejects_unknown_tokens_like_cpp_parse_index_list() {
        let mut data = WeaponBonusUpdateModuleData::default();
        let mut ini = INI::new();

        let err = parse_bonus_condition_type(&mut ini, &mut data, &["NOT_A_BONUS"])
            .expect_err("invalid bonus condition");

        assert!(matches!(err, INIError::InvalidData));
        assert_eq!(data.bonus_condition_type, WeaponBonusConditionType::Invalid);
    }
}
