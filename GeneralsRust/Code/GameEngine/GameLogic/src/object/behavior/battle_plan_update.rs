//! BattlePlanUpdate - Handle building states and battle plan execution & changes
//! Author: Kris Morness, September 2002 (C++ version) | Rust conversion: 2025

use crate::common::{
    kindof_from_name, AsciiString, CommandSourceType, Coord3D, DisabledType, KindOf, KindOfMask,
    ModelConditionFlag, ModuleData, ObjectID, SpecialPowerTemplateId, TurretType, UnsignedInt,
    ALL_KIND_OF,
};
use crate::helpers::TheGameLogic;
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, SpecialPowerCommandOptions,
    SpecialPowerUpdateInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::body::body_module::MaxHealthChangeType;
use crate::object::special_power_template::{
    find_or_create_special_power_template, SpecialPowerTemplate,
};
use crate::object::Object as GameObject;
use crate::player::{BattlePlanType, PlayerArcExt};
use crate::waypoint::Waypoint;
use crate::weapon::{WeaponLockType, WeaponSetType, WeaponSlotType};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattlePlanStatus {
    None = 0,
    Bombardment = 1,
    HoldTheLine = 2,
    SearchAndDestroy = 3,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionStatus {
    Idle = 0,
    Unpacking = 1,
    Active = 2,
    Packing = 3,
}

#[derive(Debug, Clone)]
pub struct BattlePlanBonuses {
    pub armor_scalar: f32,
    pub sight_range_scalar: f32,
    pub bombardment: i32,
    pub search_and_destroy: i32,
    pub hold_the_line: i32,
    pub valid_kind_of: KindOfMask,
    pub invalid_kind_of: KindOfMask,
}

#[derive(Debug, Clone)]
pub struct BattlePlanUpdateModuleData {
    pub base: BehaviorModuleData,
    pub special_power_template: Option<SpecialPowerTemplateId>,
    pub bombardment_plan_animation_frames: u32,
    pub hold_the_line_plan_animation_frames: u32,
    pub search_and_destroy_plan_animation_frames: u32,
    pub transition_idle_frames: u32,
    pub battle_plan_paralyze_frames: u32,
    pub hold_the_line_armor_damage_scalar: f32,
    pub search_and_destroy_sight_range_scalar: f32,
    pub strategy_center_search_and_destroy_sight_range_scalar: f32,
    pub strategy_center_search_and_destroy_detects_stealth: bool,
    pub strategy_center_hold_the_line_max_health_scalar: f32,
    pub strategy_center_hold_the_line_max_health_change_type: MaxHealthChangeType,
    pub valid_member_kind_of: KindOfMask,
    pub invalid_member_kind_of: KindOfMask,
    pub vision_object_name: String,
    pub bombardment_unpack_name: String,
    pub bombardment_pack_name: String,
    pub bombardment_message_label: String,
    pub bombardment_announcement_name: String,
    pub search_and_destroy_unpack_name: String,
    pub search_and_destroy_idle_name: String,
    pub search_and_destroy_pack_name: String,
    pub search_and_destroy_message_label: String,
    pub search_and_destroy_announcement_name: String,
    pub hold_the_line_unpack_name: String,
    pub hold_the_line_pack_name: String,
    pub hold_the_line_message_label: String,
    pub hold_the_line_announcement_name: String,
}

impl Default for BattlePlanUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            special_power_template: None,
            bombardment_plan_animation_frames: 0,
            hold_the_line_plan_animation_frames: 0,
            search_and_destroy_plan_animation_frames: 0,
            transition_idle_frames: 0,
            battle_plan_paralyze_frames: 0,
            hold_the_line_armor_damage_scalar: 1.0,
            search_and_destroy_sight_range_scalar: 1.0,
            strategy_center_search_and_destroy_sight_range_scalar: 1.0,
            strategy_center_search_and_destroy_detects_stealth: true,
            strategy_center_hold_the_line_max_health_scalar: 1.0,
            strategy_center_hold_the_line_max_health_change_type:
                MaxHealthChangeType::PreserveRatio,
            valid_member_kind_of: 0u64,
            invalid_member_kind_of: 0u64,
            vision_object_name: String::new(),
            bombardment_unpack_name: String::new(),
            bombardment_pack_name: String::new(),
            bombardment_message_label: String::new(),
            bombardment_announcement_name: String::new(),
            search_and_destroy_unpack_name: String::new(),
            search_and_destroy_idle_name: String::new(),
            search_and_destroy_pack_name: String::new(),
            search_and_destroy_message_label: String::new(),
            search_and_destroy_announcement_name: String::new(),
            hold_the_line_unpack_name: String::new(),
            hold_the_line_pack_name: String::new(),
            hold_the_line_message_label: String::new(),
            hold_the_line_announcement_name: String::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(BattlePlanUpdateModuleData, base);

impl BattlePlanUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, BATTLE_PLAN_UPDATE_FIELDS)
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_kind_of_mask(tokens: &[&str]) -> Result<KindOfMask, INIError> {
    let mut mask = 0;
    for token in tokens.iter().copied().filter(|token| *token != "=") {
        for name in token.split(['|', '+', ',']) {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let kind = kindof_from_name(name).ok_or(INIError::InvalidData)?;
            mask |= kindof_bit(kind).ok_or(INIError::InvalidData)?;
        }
    }
    Ok(mask)
}

fn parse_max_health_change_type(token: &str) -> Result<MaxHealthChangeType, INIError> {
    match token.to_ascii_uppercase().as_str() {
        "SAME_CURRENTHEALTH" => Ok(MaxHealthChangeType::SameCurrentHealth),
        "PRESERVE_RATIO" => Ok(MaxHealthChangeType::PreserveRatio),
        "ADD_CURRENT_HEALTH_TOO" => Ok(MaxHealthChangeType::AddCurrentHealthToo),
        "FULLY_HEAL" => Ok(MaxHealthChangeType::FullyHeal),
        _ => Err(INIError::InvalidData),
    }
}

macro_rules! string_field {
    ($token:literal, $field:ident) => {
        FieldParse {
            token: $token,
            parse: |_, data, tokens| {
                data.$field = required_value(tokens)?.to_string();
                Ok(())
            },
        }
    };
}

const BATTLE_PLAN_UPDATE_FIELDS: &[FieldParse<BattlePlanUpdateModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: |_, data, tokens| {
            let name = AsciiString::from(required_value(tokens)?);
            data.special_power_template =
                Some(find_or_create_special_power_template(&name).get_id());
            Ok(())
        },
    },
    FieldParse {
        token: "BombardmentPlanAnimationTime",
        parse: |_, data, tokens| {
            data.bombardment_plan_animation_frames =
                INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "HoldTheLinePlanAnimationTime",
        parse: |_, data, tokens| {
            data.hold_the_line_plan_animation_frames =
                INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "SearchAndDestroyPlanAnimationTime",
        parse: |_, data, tokens| {
            data.search_and_destroy_plan_animation_frames =
                INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "TransitionIdleTime",
        parse: |_, data, tokens| {
            data.transition_idle_frames =
                INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    string_field!("BombardmentPlanUnpackSoundName", bombardment_unpack_name),
    string_field!("BombardmentPlanPackSoundName", bombardment_pack_name),
    string_field!("BombardmentMessageLabel", bombardment_message_label),
    string_field!("BombardmentAnnouncementName", bombardment_announcement_name),
    string_field!(
        "SearchAndDestroyPlanUnpackSoundName",
        search_and_destroy_unpack_name
    ),
    string_field!(
        "SearchAndDestroyPlanIdleLoopSoundName",
        search_and_destroy_idle_name
    ),
    string_field!(
        "SearchAndDestroyPlanPackSoundName",
        search_and_destroy_pack_name
    ),
    string_field!(
        "SearchAndDestroyMessageLabel",
        search_and_destroy_message_label
    ),
    string_field!(
        "SearchAndDestroyAnnouncementName",
        search_and_destroy_announcement_name
    ),
    string_field!("HoldTheLinePlanUnpackSoundName", hold_the_line_unpack_name),
    string_field!("HoldTheLinePlanPackSoundName", hold_the_line_pack_name),
    string_field!("HoldTheLineMessageLabel", hold_the_line_message_label),
    string_field!(
        "HoldTheLineAnnouncementName",
        hold_the_line_announcement_name
    ),
    FieldParse {
        token: "ValidMemberKindOf",
        parse: |_, data, tokens| {
            data.valid_member_kind_of = parse_kind_of_mask(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "InvalidMemberKindOf",
        parse: |_, data, tokens| {
            data.invalid_member_kind_of = parse_kind_of_mask(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "BattlePlanChangeParalyzeTime",
        parse: |_, data, tokens| {
            data.battle_plan_paralyze_frames =
                INI::parse_duration_unsigned_int(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "HoldTheLinePlanArmorDamageScalar",
        parse: |_, data, tokens| {
            data.hold_the_line_armor_damage_scalar = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "SearchAndDestroyPlanSightRangeScalar",
        parse: |_, data, tokens| {
            data.search_and_destroy_sight_range_scalar = INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "StrategyCenterSearchAndDestroySightRangeScalar",
        parse: |_, data, tokens| {
            data.strategy_center_search_and_destroy_sight_range_scalar =
                INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "StrategyCenterSearchAndDestroyDetectsStealth",
        parse: |_, data, tokens| {
            data.strategy_center_search_and_destroy_detects_stealth =
                INI::parse_bool(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "StrategyCenterHoldTheLineMaxHealthScalar",
        parse: |_, data, tokens| {
            data.strategy_center_hold_the_line_max_health_scalar =
                INI::parse_real(required_value(tokens)?)?;
            Ok(())
        },
    },
    FieldParse {
        token: "StrategyCenterHoldTheLineMaxHealthChangeType",
        parse: |_, data, tokens| {
            data.strategy_center_hold_the_line_max_health_change_type =
                parse_max_health_change_type(required_value(tokens)?)?;
            Ok(())
        },
    },
    string_field!("VisionObjectName", vision_object_name),
];

pub struct BattlePlanUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<BattlePlanUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    status: TransitionStatus,
    current_plan: BattlePlanStatus,
    desired_plan: BattlePlanStatus,
    plan_affecting_army: BattlePlanStatus,
    next_ready_frame: u32,
    invalid_settings: bool,
    centering_turret: bool,
    bonuses: BattlePlanBonuses,
    vision_object_id: Option<ObjectID>,
    special_power_module: Option<SpecialPowerTemplateId>,
}

impl BattlePlanUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<BattlePlanUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let bonuses = BattlePlanBonuses {
            armor_scalar: 1.0,
            sight_range_scalar: 1.0,
            bombardment: 0,
            search_and_destroy: 0,
            hold_the_line: 0,
            valid_kind_of: specific_data.valid_member_kind_of,
            invalid_kind_of: specific_data.invalid_member_kind_of,
        };

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            status: TransitionStatus::Idle,
            current_plan: BattlePlanStatus::None,
            desired_plan: BattlePlanStatus::None,
            plan_affecting_army: BattlePlanStatus::None,
            next_ready_frame: 0,
            invalid_settings: false,
            centering_turret: false,
            bonuses,
            vision_object_id: None,
            special_power_module: None,
        })
    }

    fn object_arc(&self) -> Option<Arc<RwLock<GameObject>>> {
        self.object.upgrade()
    }

    fn with_object<R>(&self, func: impl FnOnce(&GameObject) -> R) -> Option<R> {
        let obj = self.object_arc()?;
        let guard = obj.read().ok()?;
        Some(func(&*guard))
    }

    fn with_object_mut<R>(&self, func: impl FnOnce(&mut GameObject) -> R) -> Option<R> {
        let obj = self.object_arc()?;
        let mut guard = obj.write().ok()?;
        Some(func(&mut *guard))
    }

    fn enable_turret(&self, enable: bool) {
        self.with_object(|object| {
            if let Some(ai) = object.get_ai() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_cur_weapon();
                    if turret != TurretType::Invalid {
                        ai_guard.set_turret_enabled(turret, enable);
                    }
                }
            }
        });
    }

    fn recenter_turret(&self) {
        self.with_object(|object| {
            if let Some(ai) = object.get_ai() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_cur_weapon();
                    if turret != TurretType::Invalid {
                        ai_guard.recenter_turret(turret);
                    }
                }
            }
        });
    }

    fn is_turret_in_natural_position(&self) -> bool {
        self.with_object(|object| {
            if let Some(ai) = object.get_ai() {
                if let Ok(ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_cur_weapon();
                    if turret != TurretType::Invalid {
                        return ai_guard.is_turret_in_natural_position(turret);
                    }
                }
            }
            false
        })
        .unwrap_or(false)
    }

    fn set_status(&mut self, new_status: TransitionStatus) {
        if self.status == new_status {
            return;
        }

        let old_status = self.status;
        self.clear_old_status_states(old_status);

        let now = TheGameLogic::get_frame();
        match new_status {
            TransitionStatus::Idle => {
                self.current_plan = BattlePlanStatus::None;
                self.next_ready_frame = now + self.module_data.transition_idle_frames;
            }
            TransitionStatus::Unpacking => {
                self.apply_unpacking_states(now);
            }
            TransitionStatus::Active => {
                self.set_battle_plan(self.current_plan);
                self.apply_active_states();
            }
            TransitionStatus::Packing => {
                self.set_battle_plan(BattlePlanStatus::None);
                self.apply_packing_states(now);
            }
        }

        self.status = new_status;
    }

    fn clear_old_status_states(&self, old_status: TransitionStatus) {
        self.with_object_mut(|object| match old_status {
            TransitionStatus::Unpacking => match self.current_plan {
                BattlePlanStatus::Bombardment => {
                    object.clear_model_condition_state(ModelConditionFlag::Door1Opening);
                }
                BattlePlanStatus::HoldTheLine => {
                    object.clear_model_condition_state(ModelConditionFlag::Door2Opening);
                }
                BattlePlanStatus::SearchAndDestroy => {
                    object.clear_model_condition_state(ModelConditionFlag::Door3Opening);
                }
                _ => {}
            },
            TransitionStatus::Active => match self.current_plan {
                BattlePlanStatus::Bombardment => {
                    object.clear_model_condition_state(ModelConditionFlag::Door1WaitingToClose);
                }
                BattlePlanStatus::HoldTheLine => {
                    object.clear_model_condition_state(ModelConditionFlag::Door2WaitingToClose);
                }
                BattlePlanStatus::SearchAndDestroy => {
                    object.clear_model_condition_state(ModelConditionFlag::Door3WaitingToClose);
                }
                _ => {}
            },
            TransitionStatus::Packing => match self.current_plan {
                BattlePlanStatus::Bombardment => {
                    object.clear_model_condition_state(ModelConditionFlag::Door1Closing);
                }
                BattlePlanStatus::HoldTheLine => {
                    object.clear_model_condition_state(ModelConditionFlag::Door2Closing);
                }
                BattlePlanStatus::SearchAndDestroy => {
                    object.clear_model_condition_state(ModelConditionFlag::Door3Closing);
                }
                _ => {}
            },
            _ => {}
        });
    }

    fn apply_unpacking_states(&mut self, now: u32) {
        let mut next_ready_frame = self.next_ready_frame;
        self.with_object_mut(|object| match self.current_plan {
            BattlePlanStatus::Bombardment => {
                object.set_model_condition_state(ModelConditionFlag::Door1Opening);
                next_ready_frame = now + self.module_data.bombardment_plan_animation_frames;
            }
            BattlePlanStatus::HoldTheLine => {
                object.set_model_condition_state(ModelConditionFlag::Door2Opening);
                next_ready_frame = now + self.module_data.hold_the_line_plan_animation_frames;
            }
            BattlePlanStatus::SearchAndDestroy => {
                object.set_model_condition_state(ModelConditionFlag::Door3Opening);
                next_ready_frame = now + self.module_data.search_and_destroy_plan_animation_frames;
            }
            _ => {}
        });
        self.next_ready_frame = next_ready_frame;
    }

    fn apply_active_states(&self) {
        self.with_object_mut(|object| match self.current_plan {
            BattlePlanStatus::Bombardment => {
                object.set_model_condition_state(ModelConditionFlag::Door1WaitingToClose);
            }
            BattlePlanStatus::HoldTheLine => {
                object.set_model_condition_state(ModelConditionFlag::Door2WaitingToClose);
            }
            BattlePlanStatus::SearchAndDestroy => {
                object.set_model_condition_state(ModelConditionFlag::Door3WaitingToClose);
            }
            _ => {}
        });
    }

    fn apply_packing_states(&mut self, now: u32) {
        let mut next_ready_frame = self.next_ready_frame;
        self.with_object_mut(|object| match self.current_plan {
            BattlePlanStatus::Bombardment => {
                object.set_model_condition_state(ModelConditionFlag::Door1Closing);
                next_ready_frame = now + self.module_data.bombardment_plan_animation_frames;
            }
            BattlePlanStatus::HoldTheLine => {
                object.set_model_condition_state(ModelConditionFlag::Door2Closing);
                next_ready_frame = now + self.module_data.hold_the_line_plan_animation_frames;
            }
            BattlePlanStatus::SearchAndDestroy => {
                object.set_model_condition_state(ModelConditionFlag::Door3Closing);
                next_ready_frame = now + self.module_data.search_and_destroy_plan_animation_frames;
            }
            _ => {}
        });
        self.next_ready_frame = next_ready_frame;
    }

    fn set_battle_plan(&mut self, plan: BattlePlanStatus) {
        let Some(player) = self.with_object(|object| object.get_controlling_player()) else {
            return;
        };
        let Some(player) = player else {
            return;
        };

        // Remove old plan bonuses.
        if self.plan_affecting_army != BattlePlanStatus::None {
            let plan_type = match self.plan_affecting_army {
                BattlePlanStatus::Bombardment => BattlePlanType::Bombard,
                BattlePlanStatus::HoldTheLine => BattlePlanType::HoldTheLine,
                BattlePlanStatus::SearchAndDestroy => BattlePlanType::SearchAndDestroy,
                BattlePlanStatus::None => unreachable!(),
            };
            player.change_battle_plan(plan_type, -1, &self.bonuses);
            self.remove_building_bonuses();
        }

        self.bonuses.armor_scalar = 1.0;
        self.bonuses.sight_range_scalar = 1.0;
        self.bonuses.bombardment = 0;
        self.bonuses.search_and_destroy = 0;
        self.bonuses.hold_the_line = 0;

        match plan {
            BattlePlanStatus::None => {
                let now = TheGameLogic::get_frame();
                let _ = player.iterate_objects(|obj| {
                    if let Ok(mut guard) = obj.write() {
                        let kind_of = guard.get_kind_of();
                        if (kind_of & self.module_data.valid_member_kind_of) != 0
                            && (kind_of & self.module_data.invalid_member_kind_of) == 0
                        {
                            guard.set_disabled_until(
                                DisabledType::Paralyzed,
                                now + self.module_data.battle_plan_paralyze_frames,
                            );
                        }
                    }
                    Ok(())
                });
            }
            BattlePlanStatus::Bombardment => {
                self.bonuses.bombardment = 1;
                player.change_battle_plan(BattlePlanType::Bombard, 1, &self.bonuses);
            }
            BattlePlanStatus::HoldTheLine => {
                self.apply_hold_the_line_bonuses();
                self.bonuses.armor_scalar = self.module_data.hold_the_line_armor_damage_scalar;
                self.bonuses.hold_the_line = 1;
                player.change_battle_plan(BattlePlanType::HoldTheLine, 1, &self.bonuses);
            }
            BattlePlanStatus::SearchAndDestroy => {
                self.apply_search_and_destroy_bonuses();
                self.bonuses.search_and_destroy = 1;
                self.bonuses.sight_range_scalar =
                    self.module_data.search_and_destroy_sight_range_scalar;
                player.change_battle_plan(BattlePlanType::SearchAndDestroy, 1, &self.bonuses);
            }
        }

        self.plan_affecting_army = plan;
    }

    fn apply_hold_the_line_bonuses(&self) {
        if self
            .module_data
            .strategy_center_hold_the_line_max_health_scalar
            == 1.0
        {
            return;
        }

        self.with_object(|object| {
            if let Some(body) = object.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    let current_max = body_guard.get_max_health();
                    let new_max = current_max
                        * self
                            .module_data
                            .strategy_center_hold_the_line_max_health_scalar;
                    if let Err(_err) = body_guard.set_max_health(
                        new_max,
                        self.module_data
                            .strategy_center_hold_the_line_max_health_change_type,
                    ) {
                        // Keep C++ flow (best-effort bonus application) even if body update fails.
                    }
                }
            }
        });
    }

    fn apply_search_and_destroy_bonuses(&self) {
        self.with_object_mut(|object| {
            if self
                .module_data
                .strategy_center_search_and_destroy_sight_range_scalar
                != 1.0
            {
                let scalar = self
                    .module_data
                    .strategy_center_search_and_destroy_sight_range_scalar;
                object.set_vision_range(object.get_vision_range() * scalar);
                object.set_shroud_clearing_range(object.get_shroud_clearing_range() * scalar);
            }

            if self
                .module_data
                .strategy_center_search_and_destroy_detects_stealth
            {
                if let Some(stealth_detector) = object.find_update_module("StealthDetectorUpdate") {
                    stealth_detector.with_module(|module| {
                        if let Some(control) = module.get_stealth_detector_control_interface() {
                            control.set_sd_enabled(true);
                        }
                    });
                }
            }
        });
    }

    fn remove_building_bonuses(&self) {
        self.with_object_mut(|object| match self.plan_affecting_army {
            BattlePlanStatus::HoldTheLine => {
                if self
                    .module_data
                    .strategy_center_hold_the_line_max_health_scalar
                    != 1.0
                {
                    if let Some(body) = object.get_body_module() {
                        if let Ok(mut body_guard) = body.lock() {
                            let current_max = body_guard.get_max_health();
                            let new_max = current_max
                                / self
                                    .module_data
                                    .strategy_center_hold_the_line_max_health_scalar;
                            if let Err(_err) = body_guard.set_max_health(
                                new_max,
                                self.module_data
                                    .strategy_center_hold_the_line_max_health_change_type,
                            ) {
                                // Keep C++ flow (best-effort bonus removal) even if body update fails.
                            }
                        }
                    }
                }
            }
            BattlePlanStatus::SearchAndDestroy => {
                if self
                    .module_data
                    .strategy_center_search_and_destroy_sight_range_scalar
                    != 1.0
                {
                    let scalar = self
                        .module_data
                        .strategy_center_search_and_destroy_sight_range_scalar;
                    object.set_vision_range(object.get_vision_range() / scalar);
                    object.set_shroud_clearing_range(object.get_shroud_clearing_range() / scalar);
                }

                if self
                    .module_data
                    .strategy_center_search_and_destroy_detects_stealth
                {
                    if let Some(stealth_detector) =
                        object.find_update_module("StealthDetectorUpdate")
                    {
                        stealth_detector.with_module(|module| {
                            if let Some(control) = module.get_stealth_detector_control_interface() {
                                control.set_sd_enabled(false);
                            }
                        });
                    }
                }
            }
            _ => {}
        });
    }

    pub fn get_active_battle_plan(&self) -> BattlePlanStatus {
        if self.status == TransitionStatus::Active {
            self.plan_affecting_army
        } else {
            BattlePlanStatus::None
        }
    }
}

impl UpdateModuleInterface for BattlePlanUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.invalid_settings {
            return UpdateSleepTime::None;
        }

        if self.object_arc().is_none() {
            return UpdateSleepTime::Forever;
        }

        let now = TheGameLogic::get_frame();

        if self.next_ready_frame <= now {
            match self.status {
                TransitionStatus::Idle => {
                    if self.desired_plan != BattlePlanStatus::None {
                        self.current_plan = self.desired_plan;
                        self.set_status(TransitionStatus::Unpacking);
                    }
                }
                TransitionStatus::Unpacking => {
                    self.set_status(TransitionStatus::Active);
                    if self.current_plan == BattlePlanStatus::Bombardment {
                        self.enable_turret(true);
                    }
                }
                TransitionStatus::Active => {
                    if self.current_plan != self.desired_plan {
                        if self.current_plan == BattlePlanStatus::Bombardment {
                            let should_pack = self.with_object(|object| {
                                if let Some(ai) = object.get_ai() {
                                    if self.is_turret_in_natural_position() {
                                        true
                                    } else if !self.centering_turret {
                                        ai.ai_idle(CommandSourceType::FromAI);
                                        false
                                    } else {
                                        false
                                    }
                                } else {
                                    true
                                }
                            });

                            if should_pack.unwrap_or(true) {
                                self.set_status(TransitionStatus::Packing);
                                self.centering_turret = false;
                                self.enable_turret(false);
                            } else if !self.centering_turret {
                                self.recenter_turret();
                                self.centering_turret = true;
                            }
                        } else {
                            self.set_status(TransitionStatus::Packing);
                        }
                    }
                }
                TransitionStatus::Packing => {
                    self.set_status(TransitionStatus::Idle);
                }
            }
        }

        UpdateSleepTime::None
    }
}

impl SpecialPowerUpdateInterface for BattlePlanUpdate {
    fn initiate_intent_to_do_special_power(
        &mut self,
        special_power_template: &SpecialPowerTemplate,
        _target_obj: Option<ObjectID>,
        _target_pos: Option<&Coord3D>,
        _waypoint: Option<&Waypoint>,
        command_options: SpecialPowerCommandOptions,
    ) -> bool {
        let Some(expected_template) = self.module_data.special_power_template else {
            return false;
        };
        if special_power_template.get_id() != expected_template {
            return false;
        }

        if command_options.contains(SpecialPowerCommandOptions::OPTION_ONE) {
            self.desired_plan = BattlePlanStatus::Bombardment;
        } else if command_options.contains(SpecialPowerCommandOptions::OPTION_TWO) {
            self.desired_plan = BattlePlanStatus::HoldTheLine;
        } else if command_options.contains(SpecialPowerCommandOptions::OPTION_THREE) {
            self.desired_plan = BattlePlanStatus::SearchAndDestroy;
        } else {
            return false;
        }

        true
    }

    fn is_special_ability(&self) -> bool {
        false
    }

    fn is_special_power(&self) -> bool {
        true
    }

    fn is_active(&self) -> bool {
        self.get_active_battle_plan() != BattlePlanStatus::None
    }

    fn get_command_option(&self) -> crate::modules::SpecialPowerCommandOption {
        crate::modules::SpecialPowerCommandOptions::NONE
    }

    fn does_special_power_have_overridable_destination_active(&self) -> bool {
        false
    }

    fn does_special_power_have_overridable_destination(&self) -> bool {
        false
    }

    fn set_special_power_overridable_destination(&mut self, _location: &Coord3D) {}

    fn is_power_currently_in_use(
        &self,
        _command: Option<&crate::command_button::CommandButton>,
    ) -> bool {
        self.is_active()
    }
}

impl BehaviorModuleInterface for BattlePlanUpdate {
    fn get_module_name(&self) -> &'static str {
        "BattlePlanUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_special_power_update_interface(
        &mut self,
    ) -> Option<&mut dyn SpecialPowerUpdateInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.module_data.special_power_template.is_none() {
            self.invalid_settings = true;
            return Ok(());
        }

        let mut special_power_module = None;
        self.with_object_mut(|object| {
            if let Some(template_id) = self.module_data.special_power_template {
                special_power_module = object.get_special_power_module(template_id);
            }

            object.set_weapon_set_flag(WeaponSetType::Veteran);

            if object.get_ai().is_some() {
                object.set_weapon_lock(WeaponSlotType::Primary, WeaponLockType::LockedTemporarily);
            }
        });
        self.special_power_module = special_power_module;

        self.enable_turret(false);
        Ok(())
    }

    fn on_die(
        &mut self,
        _damage_info: &crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(vision_id) = self.vision_object_id {
            let _ = TheGameLogic::destroy_object_by_id(vision_id);
        }

        if self.plan_affecting_army != BattlePlanStatus::None {
            let Some(player) = self.with_object(|object| object.get_controlling_player()) else {
                return Ok(());
            };
            if let Some(player) = player {
                let plan_type = match self.plan_affecting_army {
                    BattlePlanStatus::Bombardment => BattlePlanType::Bombard,
                    BattlePlanStatus::HoldTheLine => BattlePlanType::HoldTheLine,
                    BattlePlanStatus::SearchAndDestroy => BattlePlanType::SearchAndDestroy,
                    BattlePlanStatus::None => unreachable!(),
                };
                player.change_battle_plan(plan_type, -1, &self.bonuses);
            }
        }

        Ok(())
    }
}

pub struct BattlePlanUpdateFactory;
impl BattlePlanUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(BattlePlanUpdate::new(thing, module_data)?))
    }
}

pub struct BattlePlanUpdateModule {
    behavior: BattlePlanUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<BattlePlanUpdateModuleData>,
}

impl BattlePlanUpdateModule {
    pub fn new(
        behavior: BattlePlanUpdate,
        module_name: &AsciiString,
        module_data: Arc<BattlePlanUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut BattlePlanUpdate {
        &mut self.behavior
    }
}

fn xfer_battle_plan_status(
    xfer: &mut dyn Xfer,
    status: &mut BattlePlanStatus,
) -> Result<(), String> {
    let mut raw = *status as i32;
    unsafe { xfer.xfer_user(&mut raw as *mut i32 as *mut u8, std::mem::size_of::<i32>()) }
        .map_err(|e| format!("BattlePlanStatus xfer failed: {:?}", e))?;

    if xfer.is_reading() {
        *status = match raw {
            0 => BattlePlanStatus::None,
            1 => BattlePlanStatus::Bombardment,
            2 => BattlePlanStatus::HoldTheLine,
            3 => BattlePlanStatus::SearchAndDestroy,
            _ => BattlePlanStatus::None,
        };
    }

    Ok(())
}

fn xfer_transition_status(
    xfer: &mut dyn Xfer,
    status: &mut TransitionStatus,
) -> Result<(), String> {
    let mut raw = *status as i32;
    unsafe { xfer.xfer_user(&mut raw as *mut i32 as *mut u8, std::mem::size_of::<i32>()) }
        .map_err(|e| format!("TransitionStatus xfer failed: {:?}", e))?;

    if xfer.is_reading() {
        *status = match raw {
            0 => TransitionStatus::Idle,
            1 => TransitionStatus::Unpacking,
            2 => TransitionStatus::Active,
            3 => TransitionStatus::Packing,
            _ => TransitionStatus::Idle,
        };
    }

    Ok(())
}

fn kindof_name(kind: KindOf) -> Option<&'static str> {
    match kind {
        KindOf::Selectable => Some("SELECTABLE"),
        KindOf::Unit => Some("UNIT"),
        KindOf::Building => Some("BUILDING"),
        KindOf::Vehicle => Some("VEHICLE"),
        KindOf::Infantry => Some("INFANTRY"),
        KindOf::Aircraft => Some("AIRCRAFT"),
        KindOf::Drone => Some("DRONE"),
        KindOf::CliffJumper => Some("CLIFF_JUMPER"),
        KindOf::Structure => Some("STRUCTURE"),
        KindOf::Weapon => Some("WEAPON"),
        KindOf::Projectile => Some("PROJECTILE"),
        KindOf::CanSeeThrough => Some("CAN_SEE_THROUGH"),
        KindOf::AlwaysSelectable => Some("ALWAYS_SELECTABLE"),
        KindOf::Crate => Some("CRATE"),
        KindOf::ResourceNode => Some("RESOURCE_NODE"),
        KindOf::SupplySourceOnPreview => Some("SUPPLY_SOURCE_ON_PREVIEW"),
        KindOf::SupplySource => Some("SUPPLY_SOURCE"),
        KindOf::TechBuilding => Some("TECH_BUILDING"),
        KindOf::Powered => Some("POWERED"),
        KindOf::ProducedAtHelipad => Some("PRODUCED_AT_HELIPAD"),
        KindOf::Bridge => Some("BRIDGE"),
        KindOf::Barrier => Some("BARRIER"),
        KindOf::Civilian => Some("CIVILIAN"),
        KindOf::Destructible => Some("DESTRUCTIBLE"),
        KindOf::CanCrossBridges => Some("CAN_CROSS_BRIDGES"),
        KindOf::Amphibious => Some("AMPHIBIOUS"),
        KindOf::AmphibiousTransport => Some("AMPHIBIOUS_TRANSPORT"),
        KindOf::Transport => Some("TRANSPORT"),
        KindOf::CanCapture => Some("CAN_CAPTURE"),
        KindOf::Saboteur => Some("SABOTEUR"),
        KindOf::Hacker => Some("HACKER"),
        KindOf::Hero => Some("HERO"),
        KindOf::KeyStructure => Some("KEY_STRUCTURE"),
        KindOf::CommandCenter => Some("COMMAND_CENTER"),
        KindOf::Prison => Some("PRISON"),
        KindOf::CollectsPrisonBounty => Some("COLLECTS_PRISON_BOUNTY"),
        KindOf::PowTruck => Some("POW_TRUCK"),
        KindOf::PowerPlant => Some("POWER_PLANT"),
        KindOf::Refinery => Some("REFINERY"),
        KindOf::Factory => Some("FACTORY"),
        KindOf::Defense => Some("DEFENSE"),
        KindOf::Shrubbery => Some("SHRUBBERY"),
        KindOf::Dozer => Some("DOZER"),
        KindOf::Harvester => Some("HARVESTER"),
        KindOf::Hulk => Some("HULK"),
        KindOf::Salvager => Some("SALVAGER"),
        KindOf::WeaponSalvager => Some("WEAPON_SALVAGER"),
        KindOf::ArmorSalvager => Some("ARMOR_SALVAGER"),
        KindOf::AircraftCarrier => Some("AIRCRAFT_CARRIER"),
        KindOf::FSBarracks => Some("FS_BARRACKS"),
        KindOf::FSWarfactory => Some("FS_WARFACTORY"),
        KindOf::FSAirfield => Some("FS_AIRFIELD"),
        KindOf::FSInternetCenter => Some("FS_INTERNET_CENTER"),
        KindOf::FSPower => Some("FS_POWER"),
        KindOf::FSBaseDefense => Some("FS_BASE_DEFENSE"),
        KindOf::FSSupplyDropzone => Some("FS_SUPPLY_DROPZONE"),
        KindOf::FSSupplyCenter => Some("FS_SUPPLY_CENTER"),
        KindOf::FSSuperweapon => Some("FS_SUPERWEAPON"),
        KindOf::FSStrategyCenter => Some("FS_STRATEGY_CENTER"),
        KindOf::FSFake => Some("FS_FAKE"),
        KindOf::CountsForVictory => Some("COUNTS_FOR_VICTORY"),
        KindOf::Mine => Some("MINE"),
        KindOf::CleanupHazard => Some("CLEANUP_HAZARD"),
        KindOf::HealPad => Some("HEAL_PAD"),
        KindOf::WaveGuide => Some("WAVE_GUIDE"),
        KindOf::BridgeTower => Some("BRIDGE_TOWER"),
        KindOf::Immobile => Some("IMMOBILE"),
        KindOf::BoobyTrap => Some("BOOBY_TRAP"),
        KindOf::Disguiser => Some("DISGUISER"),
        KindOf::PortableStructure => Some("PORTABLE_STRUCTURE"),
        KindOf::CanRappel => Some("CAN_RAPPEL"),
        KindOf::CanBeRepulsed => Some("CAN_BE_REPULSED"),
        KindOf::EmpHardened => Some("EMP_HARDENED"),
        KindOf::SpawnsAreTheWeapons => Some("SPAWNS_ARE_THE_WEAPONS"),
        KindOf::IgnoreDockingBones => Some("IGNORE_DOCKING_BONES"),
        KindOf::CanSurrender => Some("CAN_SURRENDER"),
        KindOf::RepairPad => Some("REPAIR_PAD"),
        KindOf::RejectUnmanned => Some("REJECT_UNMANNED"),
        KindOf::IgnoredInGui => Some("IGNORED_IN_GUI"),
        KindOf::MobNexus => Some("MOB_NEXUS"),
        KindOf::Capturable => Some("CAPTURABLE"),
        KindOf::ImmuneToCapture => Some("IMMUNE_TO_CAPTURE"),
        KindOf::CashGenerator => Some("CASH_GENERATOR"),
        KindOf::RebuildHole => Some("REBUILD_HOLE"),
        KindOf::FSTechnology => Some("FS_TECHNOLOGY"),
        KindOf::NoGarrison => Some("NO_GARRISON"),
        KindOf::Boat => Some("BOAT"),
        KindOf::GarrisonableUntilDestroyed => Some("GARRISONABLE_UNTIL_DESTROYED"),
        KindOf::Obstacle => Some("OBSTACLE"),
        KindOf::CanAttack => Some("CAN_ATTACK"),
        KindOf::StickToTerrainSlope => Some("STICK_TO_TERRAIN_SLOPE"),
        KindOf::CanCastReflections => Some("CAN_CAST_REFLECTIONS"),
        KindOf::HugeVehicle => Some("HUGE_VEHICLE"),
        KindOf::LineBuild => Some("LINEBUILD"),
        KindOf::Preload => Some("PRELOAD"),
        KindOf::NoCollide => Some("NO_COLLIDE"),
        KindOf::StealthGarrison => Some("STEALTH_GARRISON"),
        KindOf::DrawableOnly => Some("DRAWABLE_ONLY"),
        KindOf::Score => Some("SCORE"),
        KindOf::ScoreCreate => Some("SCORE_CREATE"),
        KindOf::ScoreDestroy => Some("SCORE_DESTROY"),
        KindOf::NoHealIcon => Some("NO_HEAL_ICON"),
        KindOf::Parachutable => Some("PARACHUTABLE"),
        KindOf::SmallMissile => Some("SMALL_MISSILE"),
        KindOf::AlwaysVisible => Some("ALWAYS_VISIBLE"),
        KindOf::Unattackable => Some("UNATTACKABLE"),
        KindOf::AttackNeedsLineOfSight => Some("ATTACK_NEEDS_LINE_OF_SIGHT"),
        KindOf::WalkOnTopOfWall => Some("WALK_ON_TOP_OF_WALL"),
        KindOf::DefensiveWall => Some("DEFENSIVE_WALL"),
        KindOf::AircraftPathAround => Some("AIRCRAFT_PATH_AROUND"),
        KindOf::LowOverlappable => Some("LOW_OVERLAPPABLE"),
        KindOf::ForceAttackable => Some("FORCEATTACKABLE"),
        KindOf::AutoRallypoint => Some("AUTO_RALLYPOINT"),
        KindOf::MoneyHacker => Some("MONEY_HACKER"),
        KindOf::BallisticMissile => Some("BALLISTIC_MISSILE"),
        KindOf::ClickThrough => Some("CLICK_THROUGH"),
        KindOf::ShowPortraitWhenControlled => Some("SHOW_PORTRAIT_WHEN_CONTROLLED"),
        KindOf::CannotBuildNearSupplies => Some("CANNOT_BUILD_NEAR_SUPPLIES"),
        KindOf::RevealToAll => Some("REVEAL_TO_ALL"),
        KindOf::IgnoresSelectAll => Some("IGNORES_SELECT_ALL"),
        KindOf::DontAutoCrushInfantry => Some("DONT_AUTO_CRUSH_INFANTRY"),
        KindOf::FsBlackMarket => Some("FS_BLACK_MARKET"),
        KindOf::FsAdvancedTech => Some("FS_ADVANCED_TECH"),
        KindOf::RevealsEnemyPaths => Some("REVEALS_ENEMY_PATHS"),
        KindOf::NoSelect => Some("NO_SELECT"),
        KindOf::CannotRetaliate => Some("CANNOT_RETALIATE"),
        KindOf::TechBaseDefense => Some("TECH_BASE_DEFENSE"),
        KindOf::Demotrap => Some("DEMOTRAP"),
        KindOf::ConservativeBuilding => Some("CONSERVATIVE_BUILDING"),
        KindOf::BlastCrater => Some("BLAST_CRATER"),
        KindOf::Prop => Some("PROP"),
        KindOf::OptimizedTree => Some("OPTIMIZED_TREE"),
        KindOf::LandmarkBridge => Some("LANDMARK_BRIDGE"),
        KindOf::WaveEffect => Some("WAVE_EFFECT"),
        KindOf::ClearedByBuild => Some("CLEARED_BY_BUILD"),
        KindOf::Parachute => Some("PARACHUTE"),
    }
}

fn xfer_kind_of_mask(xfer: &mut dyn Xfer, mask: &mut KindOfMask) -> Result<(), String> {
    let mut version = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|e| format!("KindOfMask version xfer failed: {:?}", e))?;

    match xfer.get_xfer_mode() {
        game_engine::system::XferMode::Save => {
            let mut count = ALL_KIND_OF
                .iter()
                .filter(|kind| {
                    kindof_bit(**kind)
                        .map(|bit| (*mask & bit) != 0)
                        .unwrap_or(false)
                })
                .count() as i32;
            xfer.xfer_int(&mut count)
                .map_err(|e| format!("KindOfMask count xfer failed: {:?}", e))?;

            for kind in ALL_KIND_OF {
                let Some(bit) = kindof_bit(*kind) else {
                    continue;
                };
                if (*mask & bit) == 0 {
                    continue;
                }
                let mut name = kindof_name(*kind)
                    .ok_or_else(|| format!("KindOfMask has unnamed bit {}", *kind as u32))?
                    .to_string();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| format!("KindOfMask name xfer failed: {:?}", e))?;
            }
            Ok(())
        }
        game_engine::system::XferMode::Load => {
            *mask = 0;
            let mut count = 0;
            xfer.xfer_int(&mut count)
                .map_err(|e| format!("KindOfMask count xfer failed: {:?}", e))?;

            for _ in 0..count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)
                    .map_err(|e| format!("KindOfMask name xfer failed: {:?}", e))?;
                let kind = kindof_from_name(&name)
                    .ok_or_else(|| format!("KindOfMask unknown kind '{}'", name))?;
                if let Some(bit) = kindof_bit(kind) {
                    *mask |= bit;
                }
            }
            Ok(())
        }
        game_engine::system::XferMode::Crc => unsafe {
            xfer.xfer_user(
                mask as *mut KindOfMask as *mut u8,
                std::mem::size_of::<KindOfMask>(),
            )
            .map_err(|e| format!("KindOfMask crc xfer failed: {:?}", e))
        },
        mode => Err(format!("KindOfMask unsupported xfer mode: {:?}", mode)),
    }
}

fn kindof_bit(kind: KindOf) -> Option<KindOfMask> {
    1u64.checked_shl(kind as u32)
}

impl Snapshotable for BattlePlanUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        let b = &mut self.behavior;

        xfer_update_module_base_state(xfer, &mut b.next_call_frame_and_phase)?;

        xfer_battle_plan_status(xfer, &mut b.current_plan)?;
        xfer_battle_plan_status(xfer, &mut b.desired_plan)?;
        xfer_battle_plan_status(xfer, &mut b.plan_affecting_army)?;
        xfer_transition_status(xfer, &mut b.status)?;

        xfer.xfer_unsigned_int(&mut b.next_ready_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut b.invalid_settings)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut b.centering_turret)
            .map_err(|e| e.to_string())?;

        xfer.xfer_real(&mut b.bonuses.armor_scalar)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut b.bonuses.bombardment)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut b.bonuses.search_and_destroy)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut b.bonuses.hold_the_line)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut b.bonuses.sight_range_scalar)
            .map_err(|e| e.to_string())?;
        xfer_kind_of_mask(xfer, &mut b.bonuses.valid_kind_of)?;
        xfer_kind_of_mask(xfer, &mut b.bonuses.invalid_kind_of)?;

        let mut vision_id = b.vision_object_id.unwrap_or(0u32);
        xfer.xfer_object_id(&mut vision_id)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            b.vision_object_id = if vision_id == 0 {
                None
            } else {
                Some(vision_id)
            };
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Module for BattlePlanUpdateModule {
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
