// FILE: special_ability_update.rs
// Port of SpecialAbilityUpdate.h and SpecialAbilityUpdate.cpp
// Author: Rust Port
// Desc: Handles processing of unit special abilities.

use crate::common::audio::AudioEventRts;
use crate::common::{AsciiString, Coord3D, ParticleSystemTemplate};
// use game_engine::ini::IniField;
use crate::common::science::{ScienceType, SCIENCE_INVALID};
use crate::common::types::SpecialPowerType as CommonSpecialPowerType;
use crate::common::types::{
    ModelConditionFlags, ModuleData, ObjectID, Real, UnsignedInt, INVALID_ID, PI,
};
use crate::object::special_power_module::Waypoint;
use crate::object::update::special_power_update::SpecialPowerCommandOption;
type UpdateSleepTime = crate::modules::UpdateSleepTime;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = crate::modules::UPDATE_SLEEP_FOREVER;
const UPDATE_SLEEP_NONE: UpdateSleepTime = crate::modules::UPDATE_SLEEP_NONE;

use crate::common::xfer::Xfer;
use crate::modules::{
    BehaviorModuleInterface, PhysicsBehaviorExt, SpecialPowerCommandOptions,
    SpecialPowerModuleInterface, SpecialPowerUpdateInterface, UpdateModuleInterface,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
// use crate::object::behavior::ObjectBehavior;
use crate::command_button::CommandButton;
use crate::helpers::TheThingFactory;
use crate::helpers::{
    get_game_logic_random_value_real, TheAudio, TheGameLogic, TheGameText, TheInGameUI,
    TheParticleSystemManager, ThePartitionManager, TheRadar,
};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::update::special_power_update::SpecialPowerUpdateModule;
use crate::player::CMD_FROM_AI;
use crate::weapon::{WeaponLockType, WeaponSlotType};
// use crate::object::update::UpdateModule;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::{Object, SpecialPowerTemplate};
// use game_engine::thing::ThingFactory;
use crate::common::Color;
use crate::common::ObjectStatusTypes;
use crate::common::Relationship;
use crate::path::PATHFIND_CELL_SIZE_F;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use log::warn;
use std::sync::RwLock;
use std::sync::{Arc, Weak};

const SPECIAL_ABILITY_HUGE_DISTANCE: Real = 10000000.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackingState {
    None,
    Packing,
    Unpacking,
    Packed,
    Unpacked,
}

#[derive(Debug, Clone)]
pub struct SpecialAbilityUpdateModuleData {
    pub base: BehaviorModuleData,
    pub special_object_name: AsciiString,
    pub special_object_attach_to_bone_name: AsciiString,
    pub special_power_template: Option<Arc<SpecialPowerTemplate>>,
    pub start_ability_range: Real,
    pub ability_abort_range: Real,
    pub pack_unpack_variation_factor: Real,
    pub flee_range_after_completion: Real,
    pub effect_value: i32,
    pub award_xp_for_triggering: i32,
    pub skill_points_for_triggering: i32,
    pub preparation_frames: u32,
    pub persistent_prep_frames: u32,
    pub effect_duration: u32,
    pub max_special_objects: u32,
    pub pack_time: u32,
    pub unpack_time: u32,
    pub pre_trigger_unstealth_frames: u32,
    pub skip_packing_with_no_target: bool,
    pub special_objects_persistent: bool,
    pub unique_special_object_targets: bool,
    pub special_objects_persist_when_owner_dies: bool,
    pub flip_object_after_packing: bool,
    pub flip_object_after_unpacking: bool,
    pub always_validate_special_objects: bool,
    pub do_capture_fx: bool,
    pub lose_stealth_on_trigger: bool,
    pub approach_requires_los: bool,
    pub need_to_face_target: bool,
    pub persistence_requires_recharge: bool,
    pub pack_sound: Option<AudioEventRts>,
    pub unpack_sound: Option<AudioEventRts>,
    pub prep_sound_loop: Option<AudioEventRts>,
    pub trigger_sound: Option<AudioEventRts>,
    pub disable_fx_particle_system: Option<Arc<ParticleSystemTemplate>>,
}

crate::impl_behavior_module_data_via_base!(SpecialAbilityUpdateModuleData, base);

impl Default for SpecialAbilityUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            special_object_name: AsciiString::new(),
            special_object_attach_to_bone_name: AsciiString::new(),
            special_power_template: None,
            start_ability_range: SPECIAL_ABILITY_HUGE_DISTANCE,
            ability_abort_range: SPECIAL_ABILITY_HUGE_DISTANCE,
            pack_unpack_variation_factor: 0.0,
            flee_range_after_completion: 0.0,
            effect_value: 1,
            award_xp_for_triggering: 0,
            skill_points_for_triggering: -1,
            preparation_frames: 0,
            persistent_prep_frames: 0,
            effect_duration: 0,
            max_special_objects: 1,
            pack_time: 0,
            unpack_time: 0,
            pre_trigger_unstealth_frames: 0,
            skip_packing_with_no_target: false,
            special_objects_persistent: false,
            unique_special_object_targets: false,
            special_objects_persist_when_owner_dies: false,
            flip_object_after_packing: false,
            flip_object_after_unpacking: false,
            always_validate_special_objects: false,
            do_capture_fx: false,
            lose_stealth_on_trigger: false,
            approach_requires_los: true,
            need_to_face_target: true,
            persistence_requires_recharge: false,
            pack_sound: None,
            unpack_sound: None,
            prep_sound_loop: None,
            trigger_sound: None,
            disable_fx_particle_system: None,
        }
    }
}

impl SpecialAbilityUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPECIAL_ABILITY_UPDATE_FIELDS)
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens
        .iter()
        .skip_while(|t| **t == "=")
        .find(|t| !t.is_empty())
        .copied()
}

fn parse_special_power_template(
    _ini: &mut INI,
    data: &mut SpecialAbilityUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    let name = AsciiString::from(value);
    data.special_power_template = Some(find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_real(value)?);
    Ok(())
}

fn parse_int_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(i32),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_int(value)?);
    Ok(())
}

fn parse_duration_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_duration_unsigned_int(value)?);
    Ok(())
}

fn parse_unsigned_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_unsigned_int(value)?);
    Ok(())
}

fn parse_bool_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(bool),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(INI::parse_bool(value)?);
    Ok(())
}

fn parse_ascii_string_field(
    _ini: &mut INI,
    target: &mut AsciiString,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    *target = AsciiString::from(value);
    Ok(())
}

fn parse_audio_event_field(
    _ini: &mut INI,
    target: &mut Option<AudioEventRts>,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if value.eq_ignore_ascii_case("none") {
        *target = None;
    } else {
        *target = Some(AudioEventRts::new(value));
    }
    Ok(())
}

fn parse_particle_system_template_field(
    _ini: &mut INI,
    target: &mut Option<Arc<ParticleSystemTemplate>>,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if value.eq_ignore_ascii_case("none") {
        *target = None;
        return Ok(());
    }
    let name = AsciiString::from(value);
    *target = Some(Arc::new(ParticleSystemTemplate::new(name)));
    Ok(())
}

const SPECIAL_ABILITY_UPDATE_FIELDS: &[FieldParse<SpecialAbilityUpdateModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template,
    },
    FieldParse {
        token: "StartAbilityRange",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.start_ability_range = v, tokens)
        },
    },
    FieldParse {
        token: "AbilityAbortRange",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.ability_abort_range = v, tokens)
        },
    },
    FieldParse {
        token: "PreparationTime",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.preparation_frames = v, tokens)
        },
    },
    FieldParse {
        token: "PersistentPrepTime",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.persistent_prep_frames = v, tokens)
        },
    },
    FieldParse {
        token: "PackTime",
        parse: |ini, data, tokens| parse_duration_field(ini, &mut |v| data.pack_time = v, tokens),
    },
    FieldParse {
        token: "UnpackTime",
        parse: |ini, data, tokens| parse_duration_field(ini, &mut |v| data.unpack_time = v, tokens),
    },
    FieldParse {
        token: "PreTriggerUnstealthTime",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.pre_trigger_unstealth_frames = v, tokens)
        },
    },
    FieldParse {
        token: "SkipPackingWithNoTarget",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.skip_packing_with_no_target = v, tokens)
        },
    },
    FieldParse {
        token: "PackUnpackVariationFactor",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.pack_unpack_variation_factor = v, tokens)
        },
    },
    FieldParse {
        token: "SpecialObject",
        parse: |ini, data, tokens| {
            parse_ascii_string_field(ini, &mut data.special_object_name, tokens)
        },
    },
    FieldParse {
        token: "SpecialObjectAttachToBone",
        parse: |ini, data, tokens| {
            parse_ascii_string_field(ini, &mut data.special_object_attach_to_bone_name, tokens)
        },
    },
    FieldParse {
        token: "MaxSpecialObjects",
        parse: |ini, data, tokens| {
            parse_unsigned_field(ini, &mut |v| data.max_special_objects = v, tokens)
        },
    },
    FieldParse {
        token: "SpecialObjectsPersistent",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.special_objects_persistent = v, tokens)
        },
    },
    FieldParse {
        token: "EffectDuration",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.effect_duration = v, tokens)
        },
    },
    FieldParse {
        token: "EffectValue",
        parse: |ini, data, tokens| parse_int_field(ini, &mut |v| data.effect_value = v, tokens),
    },
    FieldParse {
        token: "UniqueSpecialObjectTargets",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.unique_special_object_targets = v, tokens)
        },
    },
    FieldParse {
        token: "SpecialObjectsPersistWhenOwnerDies",
        parse: |ini, data, tokens| {
            parse_bool_field(
                ini,
                &mut |v| data.special_objects_persist_when_owner_dies = v,
                tokens,
            )
        },
    },
    FieldParse {
        token: "AlwaysValidateSpecialObjects",
        parse: |ini, data, tokens| {
            parse_bool_field(
                ini,
                &mut |v| data.always_validate_special_objects = v,
                tokens,
            )
        },
    },
    FieldParse {
        token: "FlipOwnerAfterPacking",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.flip_object_after_packing = v, tokens)
        },
    },
    FieldParse {
        token: "FlipOwnerAfterUnpacking",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.flip_object_after_unpacking = v, tokens)
        },
    },
    FieldParse {
        token: "FleeRangeAfterCompletion",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.flee_range_after_completion = v, tokens)
        },
    },
    FieldParse {
        token: "DisableFXParticleSystem",
        parse: |ini, data, tokens| {
            parse_particle_system_template_field(ini, &mut data.disable_fx_particle_system, tokens)
        },
    },
    FieldParse {
        token: "DoCaptureFX",
        parse: |ini, data, tokens| parse_bool_field(ini, &mut |v| data.do_capture_fx = v, tokens),
    },
    FieldParse {
        token: "PackSound",
        parse: |ini, data, tokens| parse_audio_event_field(ini, &mut data.pack_sound, tokens),
    },
    FieldParse {
        token: "UnpackSound",
        parse: |ini, data, tokens| parse_audio_event_field(ini, &mut data.unpack_sound, tokens),
    },
    FieldParse {
        token: "PrepSoundLoop",
        parse: |ini, data, tokens| parse_audio_event_field(ini, &mut data.prep_sound_loop, tokens),
    },
    FieldParse {
        token: "TriggerSound",
        parse: |ini, data, tokens| parse_audio_event_field(ini, &mut data.trigger_sound, tokens),
    },
    FieldParse {
        token: "LoseStealthOnTrigger",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.lose_stealth_on_trigger = v, tokens)
        },
    },
    FieldParse {
        token: "AwardXPForTriggering",
        parse: |ini, data, tokens| {
            parse_int_field(ini, &mut |v| data.award_xp_for_triggering = v, tokens)
        },
    },
    FieldParse {
        token: "SkillPointsForTriggering",
        parse: |ini, data, tokens| {
            parse_int_field(ini, &mut |v| data.skill_points_for_triggering = v, tokens)
        },
    },
    FieldParse {
        token: "ApproachRequiresLOS",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.approach_requires_los = v, tokens)
        },
    },
    FieldParse {
        token: "NeedToFaceTarget",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.need_to_face_target = v, tokens)
        },
    },
    FieldParse {
        token: "PersistenceRequiresRecharge",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.persistence_requires_recharge = v, tokens)
        },
    },
];

#[derive(Debug)]
pub struct SpecialAbilityUpdate {
    base: SpecialPowerUpdateModule,
    module_data: Arc<SpecialAbilityUpdateModuleData>,
    this_module_data: Option<Arc<dyn ModuleData>>, // Keep reference to raw module data if needed
    active: bool,
    prep_frames: u32,
    anim_frames: u32,
    target_id: ObjectID,
    target_pos: Coord3D,
    location_count: i32,
    special_object_id_list: Vec<ObjectID>,
    packing_state: PackingState,
    no_target_command: bool,
    facing_initiated: bool,
    facing_complete: bool,
    within_start_ability_range: bool,
    do_disable_fx_particles: bool,
    capture_flash_phase: Real,
    prep_sound_loop: Option<AudioEventRts>,
    object_ptr: Weak<RwLock<Object>>,
}

impl SpecialAbilityUpdate {
    pub fn new(object_ptr: Weak<RwLock<Object>>, module_data: Arc<dyn ModuleData>) -> Self {
        let object_id = object_ptr
            .upgrade()
            .and_then(|obj| obj.read().ok().map(|guard| guard.id))
            .unwrap_or(INVALID_ID);

        let sa_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<SpecialAbilityUpdateModuleData>()
            .expect("Invalid ModuleData for SpecialAbilityUpdate")
            .clone();

        let behavior = Self {
            base: SpecialPowerUpdateModule::new(object_id, object_ptr.clone()),
            module_data: Arc::new(sa_data),
            this_module_data: Some(module_data),
            active: false,
            prep_frames: 0,
            anim_frames: 0,
            target_id: INVALID_ID,
            target_pos: Coord3D::default(),
            location_count: 0,
            special_object_id_list: Vec::new(),
            packing_state: PackingState::None,
            no_target_command: false,
            facing_initiated: false,
            facing_complete: false,
            within_start_ability_range: false,
            do_disable_fx_particles: true,
            capture_flash_phase: 0.0,
            prep_sound_loop: None,
            object_ptr,
        };

        if object_id != INVALID_ID {
            TheGameLogic::set_wake_frame(object_id, UPDATE_SLEEP_FOREVER);
        }

        behavior
    }

    fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.object_ptr.upgrade()
    }

    fn calc_sleep_time(&self) -> UpdateSleepTime {
        if self.active || self.module_data.always_validate_special_objects {
            UPDATE_SLEEP_NONE
        } else {
            UPDATE_SLEEP_FOREVER
        }
    }

    pub fn get_special_power_type(&self) -> Option<CommonSpecialPowerType> {
        self.module_data
            .special_power_template
            .as_ref()
            .and_then(|template| {
                CommonSpecialPowerType::from_u32(template.get_special_power_type() as u32)
            })
    }

    pub fn get_special_object_count(&self) -> UnsignedInt {
        self.special_object_id_list.len() as UnsignedInt
    }

    pub fn get_special_object_max(&self) -> UnsignedInt {
        self.module_data.max_special_objects
    }

    fn get_template(&self) -> Option<Arc<SpecialPowerTemplate>> {
        self.module_data.special_power_template.clone()
    }

    fn with_spm<F, R>(&self, func: F) -> Option<R>
    where
        F: FnOnce(&mut dyn SpecialPowerModuleInterface) -> R,
    {
        let obj = self.get_object()?;
        let obj_guard = obj.read().ok()?;
        let template = self.module_data.special_power_template.as_ref()?;
        obj_guard.with_special_power_module_mut_by_name(template.get_name(), func)
    }

    fn is_preparation_complete(&self) -> bool {
        self.prep_frames == 0
    }

    fn reset_preparation(&mut self) {
        self.prep_frames = self.module_data.persistent_prep_frames;
    }

    fn is_persistent_ability(&self) -> bool {
        self.module_data.persistent_prep_frames > 0
    }

    fn on_exit(&mut self, cleanup: bool) {
        if let Some(obj) = self.get_object() {
            if let Ok(mut obj_guard) = obj.write() {
                obj_guard
                    .clear_model_condition_flags(
                        ModelConditionFlags::Unpacking
                            | ModelConditionFlags::Packing
                            | ModelConditionFlags::FiringA,
                    )
                    .unwrap_or_else(|err| {
                        log::debug!(
                            "SpecialAbilityUpdate::on_exit clear_model_condition_flags: {err}"
                        )
                    });
                obj_guard.clear_status(crate::common::ObjectStatusMaskType::IS_USING_ABILITY);
            }
        }

        // remove audio event...
        self.end_preparation();

        if !self.module_data.special_objects_persistent
            || (cleanup && !self.module_data.special_objects_persist_when_owner_dies)
        {
            self.kill_special_objects();
        }

        self.active = false;
        self.within_start_ability_range = false;
        self.packing_state = PackingState::None;
    }

    fn end_preparation(&mut self) {
        if let Some(obj) = self.get_object() {
            if let Ok(mut obj_guard) = obj.write() {
                obj_guard.clear_status(crate::common::ObjectStatusMaskType::IS_USING_ABILITY);
            }
        }

        if let Some(sound) = self.prep_sound_loop.take() {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(sound.get_playing_handle());
            }
        }

        if let Some(template) = self.get_template() {
            match template.get_special_power_type() {
                crate::object::special_power_types::SpecialPowerType::TankHunterTntAttack
                | crate::object::special_power_types::SpecialPowerType::TimedCharges
                | crate::object::special_power_types::SpecialPowerType::BoobyTrap
                | crate::object::special_power_types::SpecialPowerType::RemoteCharges
                | crate::object::special_power_types::SpecialPowerType::DisguiseAsVehicle
                | crate::object::special_power_types::SpecialPowerType::HelixNapalmBomb => {}
                crate::object::special_power_types::SpecialPowerType::MissileDefenderLaserGuidedMissiles
                | crate::object::special_power_types::SpecialPowerType::HackerDisableBuilding
                | crate::object::special_power_types::SpecialPowerType::BlackLotusDisableVehicleHack
                | crate::object::special_power_types::SpecialPowerType::BlackLotusCaptureBuilding
                | crate::object::special_power_types::SpecialPowerType::BlackLotusStealCashHack
                | crate::object::special_power_types::SpecialPowerType::InfantryCaptureBuilding => {
                    self.kill_special_objects();
                }
                _ => {}
            }
        }
    }

    fn init_laser(
        &mut self,
        special_object: &Arc<RwLock<Object>>,
        target: Option<&Arc<RwLock<Object>>>,
    ) -> bool {
        let Some(owner) = self.get_object() else {
            self.kill_special_objects();
            return false;
        };

        let owner_guard = match owner.read() {
            Ok(guard) => guard,
            Err(_) => {
                self.kill_special_objects();
                return false;
            }
        };
        let (found, start_pos, _mat) = owner_guard.get_single_logical_bone_position(
            self.module_data.special_object_attach_to_bone_name.as_str(),
        );
        let start_pos = if found {
            start_pos
        } else {
            *owner_guard.get_position()
        };

        let target_guard = target.and_then(|t| t.read().ok());
        let end_pos = target_guard
            .as_ref()
            .map(|guard| {
                guard
                    .get_geometry_info()
                    .get_center_position(guard.get_position())
            })
            .unwrap_or(start_pos);

        let client_modules = {
            let Ok(guard) = special_object.read() else {
                self.kill_special_objects();
                return false;
            };
            guard.client_update_modules()
        };

        for module in client_modules {
            let _ = module.with_module_downcast::<crate::object::update::laser_update::LaserUpdateModule, _, _>(
                |laser_update| {
                    laser_update.update_mut().init_laser(
                        Some(&*owner_guard),
                        target_guard.as_deref(),
                        Some(&start_pos),
                        Some(&end_pos),
                        self.module_data.special_object_attach_to_bone_name.as_str().to_string(),
                        0,
                    );
                },
            );
        }

        true
    }

    fn is_within_start_ability_range(&self) -> bool {
        let Some(obj) = self.get_object() else {
            return false;
        };
        let obj_guard = match obj.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        let mut range = self.module_data.start_ability_range;
        let undersize = PATHFIND_CELL_SIZE_F * 0.25;
        range = (range - undersize).max(0.0);

        let mut dist_sq = 0.0;
        let mut target_arc: Option<Arc<RwLock<Object>>> = None;

        if self.target_id != INVALID_ID {
            if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                if let Ok(target_guard) = target.read() {
                    dist_sq = ThePartitionManager::get_distance_squared(
                        &obj_guard,
                        &target_guard,
                        crate::common::FROM_BOUNDING_SPHERE_2D,
                    );
                }
                target_arc = Some(target);
            }
        } else if self.target_pos.x != 0.0 || self.target_pos.y != 0.0 || self.target_pos.z != 0.0 {
            dist_sq = ThePartitionManager::get_distance_squared_to_pos(
                &obj_guard,
                &self.target_pos,
                crate::common::FROM_BOUNDING_SPHERE_2D,
            );
        } else {
            return true;
        }

        if dist_sq > range * range {
            return false;
        }

        if range == 0.0 && self.target_id != INVALID_ID {
            return dist_sq <= 0.0;
        }

        if self.module_data.approach_requires_los {
            if let Some(target) = target_arc {
                if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
                    if let Ok(target_guard) = target.read() {
                        let src_pos = obj_guard.get_position();
                        let tgt_pos = target_guard.get_position();
                        return terrain.is_clear_line_of_sight(src_pos, tgt_pos);
                    }
                }
            }
        }

        true
    }

    fn is_within_ability_abort_range(&self) -> bool {
        let Some(obj) = self.get_object() else {
            return false;
        };
        let obj_guard = match obj.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        let mut range = self.module_data.start_ability_range;
        let undersize = PATHFIND_CELL_SIZE_F * 0.25;
        range = (range - undersize).max(0.0);

        let mut dist_sq = 0.0;
        let mut target_arc: Option<Arc<RwLock<Object>>> = None;

        if self.target_id != INVALID_ID {
            if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                if let Ok(target_guard) = target.read() {
                    dist_sq = ThePartitionManager::get_distance_squared(
                        &obj_guard,
                        &target_guard,
                        crate::common::FROM_BOUNDING_SPHERE_2D,
                    );
                }
                target_arc = Some(target);
            }
        } else if self.target_pos.x != 0.0 || self.target_pos.y != 0.0 || self.target_pos.z != 0.0 {
            dist_sq = ThePartitionManager::get_distance_squared_to_pos(
                &obj_guard,
                &self.target_pos,
                crate::common::FROM_BOUNDING_SPHERE_2D,
            );
        } else {
            return true;
        }

        if dist_sq > self.module_data.ability_abort_range * self.module_data.ability_abort_range {
            return false;
        }

        if range == 0.0 && self.target_id != INVALID_ID {
            return dist_sq <= 0.0;
        }

        if self.module_data.approach_requires_los {
            if let Some(target) = target_arc {
                if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
                    if let Ok(target_guard) = target.read() {
                        let src_pos = obj_guard.get_position();
                        let tgt_pos = target_guard.get_position();
                        return terrain.is_clear_line_of_sight(src_pos, tgt_pos);
                    }
                }
            }
        }

        true
    }

    fn approach_target(&mut self) -> bool {
        let Some(obj) = self.get_object() else {
            return false;
        };
        let obj_guard = match obj.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        if self.target_id != INVALID_ID {
            if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(target_guard) = target.read() {
                        let _ = ai
                            .lock()
                            .map(|mut ai_guard| ai_guard.ignore_obstacle(Some(&target)));
                    }
                    ai.ai_move_to_object(self.target_id, CMD_FROM_AI);
                    return true;
                }
            }
        } else if self.target_pos.x != 0.0 || self.target_pos.y != 0.0 || self.target_pos.z != 0.0 {
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                ai.ai_move_to_position(&self.target_pos, false, CMD_FROM_AI);
                return true;
            }
        }
        false
    }

    fn start_preparation(&mut self) {
        self.prep_frames = self.module_data.preparation_frames;

        let template = match self.get_template() {
            Some(t) => t,
            None => return,
        };

        match template.get_special_power_type() {
            crate::object::special_power_types::SpecialPowerType::MissileDefenderLaserGuidedMissiles => {
                if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                    if let Some(special_object) = self.create_special_object() {
                        let _ = self.init_laser(&special_object, Some(&target));
                    }
                }
            }
            crate::object::special_power_types::SpecialPowerType::InfantryCaptureBuilding => {
                self.capture_flash_phase = 0.0;
                if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                    if let (Ok(target_guard), Some(owner)) =
                        (target.read(), self.get_object())
                    {
                        if owner.read().ok().map(|o| o.relationship_to(&target_guard))
                            == Some(Relationship::Allies)
                        {
                            return;
                        }
                        if target_guard.check_and_detonate_booby_trap(owner.read().ok().as_deref()) {
                            if target_guard.is_effectively_dead()
                                || owner.read().ok().map(|o| o.is_effectively_dead()).unwrap_or(false)
                            {
                                return;
                            }
                        }
                    }
                }

                if let Some(obj) = self.get_object() {
                    if let Ok(mut obj_guard) = obj.write() {
                        obj_guard.clear_and_set_model_condition_flags(
                            ModelConditionFlags::Unpacking,
                            ModelConditionFlags::empty(),
                        )
                        .unwrap_or_else(|err| {
                            log::debug!(
                                "SpecialAbilityUpdate::process_mode_specific clear_and_set flags: {err}"
                            )
                        });
                        if self.prep_frames > 0 {
                            obj_guard.set_animation_completion_time(self.prep_frames);
                        }
                    }
                }

                if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                    let _ = TheRadar::try_infiltration_event(target);
                }
            }
            crate::object::special_power_types::SpecialPowerType::HackerDisableBuilding
            | crate::object::special_power_types::SpecialPowerType::BlackLotusCaptureBuilding
            | crate::object::special_power_types::SpecialPowerType::BlackLotusDisableVehicleHack
            | crate::object::special_power_types::SpecialPowerType::BlackLotusStealCashHack => {
                if template.get_special_power_type()
                    == crate::object::special_power_types::SpecialPowerType::BlackLotusCaptureBuilding
                {
                    self.capture_flash_phase = 0.0;
                }
                if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                    if let (Ok(target_guard), Some(owner)) =
                        (target.read(), self.get_object())
                    {
                        if owner.read().ok().map(|o| o.relationship_to(&target_guard))
                            == Some(Relationship::Allies)
                        {
                            return;
                        }
                    }
                    if let Some(special_object) = self.create_special_object() {
                        let _ = self.init_laser(&special_object, Some(&target));
                        if let Some(obj) = self.get_object() {
                            if let Ok(mut obj_guard) = obj.write() {
                                obj_guard.clear_and_set_model_condition_flags(
                                    ModelConditionFlags::Unpacking,
                                    ModelConditionFlags::FiringA,
                                )
                                .unwrap_or_else(|err| {
                                    log::debug!(
                                        "SpecialAbilityUpdate::process_mode_specific firing flags: {err}"
                                    )
                                });
                            }
                        }
                    }
                    let _ = TheRadar::try_infiltration_event(target);
                }
            }
            _ => {}
        }

        let _ = self.with_spm(|spm| {
            spm.mark_special_power_triggered(None);
        });

        if let Some(obj) = self.get_object() {
            if let Ok(mut obj_guard) = obj.write() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    ai.ai_idle(CMD_FROM_AI);
                }
                obj_guard.set_status(crate::common::ObjectStatusMaskType::IS_USING_ABILITY, true);
            }
        }

        if let Some(sound) = self.module_data.prep_sound_loop.as_ref() {
            if let Some(audio) = TheAudio::get() {
                if let Some(obj) = self.get_object() {
                    if let Ok(obj_guard) = obj.read() {
                        let mut event = sound.clone();
                        event.set_object_id(obj_guard.get_id());
                        let handle = audio.add_audio_event(&event);
                        event.set_playing_handle(handle);
                        self.prep_sound_loop = Some(event);
                    }
                }
            }
        }
    }

    fn continue_preparation(&mut self) -> bool {
        if self.module_data.ability_abort_range < SPECIAL_ABILITY_HUGE_DISTANCE {
            if !self.is_within_ability_abort_range() {
                return false;
            }
        }

        let template = match self.get_template() {
            Some(t) => t,
            None => return false,
        };

        match template.get_special_power_type() {
            crate::object::special_power_types::SpecialPowerType::MissileDefenderLaserGuidedMissiles
            | crate::object::special_power_types::SpecialPowerType::BlackLotusDisableVehicleHack => {
                let target = match TheGameLogic::find_object_by_id(self.target_id) {
                    Some(target) => target,
                    None => return false,
                };

                if let (Ok(target_guard), Some(owner)) =
                    (target.read(), self.get_object())
                {
                    if owner.read().ok().map(|o| o.relationship_to(&target_guard))
                        == Some(Relationship::Allies)
                    {
                        return false;
                    }
                }

                let special_ids = self.special_object_id_list.clone();
                for id in special_ids {
                    if let Some(special_object) = TheGameLogic::find_object_by_id(id) {
                        if !self.init_laser(&special_object, Some(&target)) {
                            return false;
                        }
                    }
                }
            }
            crate::object::special_power_types::SpecialPowerType::InfantryCaptureBuilding
            | crate::object::special_power_types::SpecialPowerType::BlackLotusCaptureBuilding => {
                let target = match TheGameLogic::find_object_by_id(self.target_id) {
                    Some(target) => target,
                    None => return false,
                };
                if let (Ok(target_guard), Some(owner)) =
                    (target.read(), self.get_object())
                {
                    if owner.read().ok().map(|o| o.relationship_to(&target_guard))
                        == Some(Relationship::Allies)
                    {
                        return false;
                    }
                }

                if self.module_data.do_capture_fx {
                    if let Ok(target_guard) = target.read() {
                        if let Some(drawable) = target_guard.get_drawable() {
                            let last_phase = (self.capture_flash_phase as i32) & 1;
                            let denom = self.module_data.preparation_frames.max(1) as Real;
                            let increment = 1.0 - (self.prep_frames as Real / denom);
                            self.capture_flash_phase += increment / 3.0;
                            let this_phase = (self.capture_flash_phase as i32) & 1;
                            if last_phase == 1 && this_phase == 0 {
                                if let Ok(mut draw_guard) = drawable.write() {
                                    draw_guard.flash_as_selected();
                                }
                            }
                        }
                    }
                }

                let _ = self.with_spm(|spm| {
                    if template.get_special_power_type()
                        == crate::object::special_power_types::SpecialPowerType::InfantryCaptureBuilding
                    {
                        let _ = spm.start_power_recharge();
                    }
                });
            }
            _ => {}
        }

        true
    }

    fn trigger_ability_effect(&mut self) {
        let template = match self.get_template() {
            Some(t) => t,
            None => return,
        };

        if let Some(obj) = self.get_object() {
            if let Ok(obj_guard) = obj.read() {
                if self.module_data.award_xp_for_triggering > 0 {
                    if let Some(tracker) = obj_guard.get_experience_tracker() {
                        let _ = tracker.lock().map(|mut t| {
                            t.add_experience_points(
                                self.module_data.award_xp_for_triggering,
                                false,
                                &crate::experience::ExperienceTracker::DEFAULT_EXPERIENCE_REQUIRED,
                            );
                        });
                    }
                }
                let skill_points = if self.module_data.skill_points_for_triggering != -1 {
                    self.module_data.skill_points_for_triggering
                } else {
                    self.module_data.award_xp_for_triggering
                };
                if skill_points > 0 {
                    if let Some(player) = obj_guard.get_controlling_player() {
                        let _ = player.write().map(|mut p| {
                            p.add_skill_points(skill_points);
                        });
                    }
                }
            }
        }

        if let Some(sound) = self.module_data.trigger_sound.as_ref() {
            if let Some(audio) = TheAudio::get() {
                if let Some(obj) = self.get_object() {
                    if let Ok(obj_guard) = obj.read() {
                        let mut event = sound.clone();
                        event.set_object_id(obj_guard.get_id());
                        audio.add_audio_event(&event);
                    }
                }
            }
        }

        let mut ok_to_lose_stealth = true;

        match template.get_special_power_type() {
            crate::object::special_power_types::SpecialPowerType::MissileDefenderLaserGuidedMissiles => {
                if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                    if let Some(obj) = self.get_object() {
                        if let Ok(mut obj_guard) = obj.write() {
                            obj_guard.set_weapon_lock(
                                WeaponSlotType::Secondary,
                                WeaponLockType::LockedTemporarily,
                            );
                            if let Some(ai) = obj_guard.get_ai_update_interface() {
                                ai.ai_attack_object(&target, crate::weapon::NO_MAX_SHOTS_LIMIT, CMD_FROM_AI);
                            }
                        }
                    }
                    drop(target);
                }
            }
            crate::object::special_power_types::SpecialPowerType::HelixNapalmBomb => {
                let _ = self.create_special_object();
            }
            crate::object::special_power_types::SpecialPowerType::TankHunterTntAttack
            | crate::object::special_power_types::SpecialPowerType::TimedCharges
            | crate::object::special_power_types::SpecialPowerType::BoobyTrap => {
                let target = match TheGameLogic::find_object_by_id(self.target_id) {
                    Some(target) => target,
                    None => return,
                };
                if let Some(owner) = self.get_object() {
                    if target.read().ok().map(|t| t.check_and_detonate_booby_trap(owner.read().ok().as_deref())).unwrap_or(false) {
                        if target.read().ok().map(|t| t.is_effectively_dead()).unwrap_or(false)
                            || owner.read().ok().map(|o| o.is_effectively_dead()).unwrap_or(false)
                        {
                            return;
                        }
                    }
                }

                if template.get_special_power_type()
                    == crate::object::special_power_types::SpecialPowerType::BoobyTrap
                {
                    if target
                        .read()
                        .ok()
                        .map(|t| t.test_status(ObjectStatusTypes::BoobyTrapped))
                        .unwrap_or(false)
                    {
                        return;
                    }
                }

                if let Some(charge) = self.create_special_object() {
                    let module = match charge.read() {
                        Ok(guard) => guard.find_update_module("StickyBombUpdate"),
                        Err(_) => None,
                    };
                    if let Some(module) = module {
                        let _ = module.with_module_downcast::<crate::object::behavior::sticky_bomb_update::StickyBombUpdateModule, _, _>(
                            |module| {
                                let update = module.behavior_mut();
                                let Ok(target_guard) = target.read() else {
                                    return;
                                };
                                let Some(owner_obj) = self.get_object() else {
                                    return;
                                };
                                let Ok(owner_guard) = owner_obj.read() else {
                                    return;
                                };
                                update.init_sticky_bomb(Some(&*target_guard), Some(&*owner_guard), None);
                            },
                        );
                    } else {
                        self.kill_special_objects();
                        return;
                    }
                }
            }
            crate::object::special_power_types::SpecialPowerType::HackerDisableBuilding
            | crate::object::special_power_types::SpecialPowerType::BlackLotusDisableVehicleHack => {
                let target = match TheGameLogic::find_object_by_id(self.target_id) {
                    Some(target) => target,
                    None => return,
                };
                if let Some(obj) = self.get_object() {
                    if let Ok(obj_guard) = obj.read() {
                        if obj_guard.relationship_to(&target.read().unwrap()) == Relationship::Allies {
                            return;
                        }
                    }
                }
                if let Ok(mut target_guard) = target.write() {
                    target_guard.set_disabled_until(
                        crate::common::DisabledType::DisabledHacked,
                        TheGameLogic::get_frame() + self.module_data.effect_duration,
                    );
                }

                if let Some(manager) = TheParticleSystemManager::get() {
                    if let Some(tmpl) = self.module_data.disable_fx_particle_system.as_ref() {
                        if let Some(system_id) =
                            manager.create_particle_system(Some(tmpl.name.as_str()))
                        {
                            if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                                if let Ok(target_guard) = target.read() {
                                    manager.attach_particle_system_to_object(
                                        system_id,
                                        target_guard.get_id(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            crate::object::special_power_types::SpecialPowerType::InfantryCaptureBuilding
            | crate::object::special_power_types::SpecialPowerType::BlackLotusCaptureBuilding => {
                let target = match TheGameLogic::find_object_by_id(self.target_id) {
                    Some(target) => target,
                    None => return,
                };
                let owner = match self.get_object() {
                    Some(owner) => owner,
                    None => return,
                };
                let Ok(mut target_guard) = target.write() else {
                    return;
                };
                let Ok(owner_guard) = owner.read() else {
                    return;
                };
                if target_guard.check_and_detonate_booby_trap(Some(&owner_guard)) {
                    if target_guard.is_effectively_dead() || owner_guard.is_effectively_dead() {
                        return;
                    }
                }
                if owner_guard.relationship_to(&target_guard) == Relationship::Allies {
                    return;
                }

                if let Some(contain) = target_guard.get_contain() {
                    if let Ok(mut contain_guard) = contain.lock() {
                        if contain_guard.is_garrisonable() {
                            let _ = contain_guard.remove_all_contained(true);
                            return;
                        }
                    }
                }

                target_guard.defect(owner_guard.get_team(), 1);
            }
            crate::object::special_power_types::SpecialPowerType::BlackLotusStealCashHack => {
                let target = match TheGameLogic::find_object_by_id(self.target_id) {
                    Some(target) => target,
                    None => return,
                };
                let owner = match self.get_object() {
                    Some(owner) => owner,
                    None => return,
                };
                let (mut target_guard, mut owner_guard) =
                    match (target.write(), owner.write()) {
                        (Ok(t), Ok(o)) => (t, o),
                        _ => return,
                    };
                let cash = target_guard
                    .get_controlling_player()
                    .and_then(|player| player.read().ok().map(|guard| guard.get_money().get_money()))
                    .map(|money| money.clamp(0, 1000) as u32)
                    .unwrap_or(0);
                if cash > 0 {
                    if let Some(target_player) = target_guard.get_controlling_player() {
                        let _ = target_player
                            .write()
                            .map(|mut p| p.get_money_mut().withdraw(cash));
                    }
                    if let Some(owner_player) = owner_guard.get_controlling_player() {
                        let _ = owner_player
                            .write()
                            .map(|mut p| p.get_money_mut().deposit(cash));
                    }

                    let mut pos = *owner_guard.get_position();
                    pos.z += 20.0;
                    let text = TheGameText::fetch("GUI:AddCash");
                    let _ = TheInGameUI::add_floating_text(
                        &format!("{} {}", text, cash),
                        &pos,
                        Color::rgb(0, 255, 0),
                    );

                    let mut tpos = *target_guard.get_position();
                    tpos.z += 30.0;
                    let text = TheGameText::fetch("GUI:LoseCash");
                    let _ = TheInGameUI::add_floating_text(
                        &format!("{} {}", text, cash),
                        &tpos,
                        Color::rgb(255, 0, 0),
                    );
                }
            }
            crate::object::special_power_types::SpecialPowerType::RemoteCharges => {
                if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                    if let Some(owner) = self.get_object() {
                        if target
                            .read()
                            .ok()
                            .map(|t| t.check_and_detonate_booby_trap(owner.read().ok().as_deref()))
                            .unwrap_or(false)
                        {
                            if target.read().ok().map(|t| t.is_effectively_dead()).unwrap_or(false)
                                || owner.read().ok().map(|o| o.is_effectively_dead()).unwrap_or(false)
                            {
                                return;
                            }
                        }
                    }
                }
                if self.target_id == INVALID_ID
                    && self.target_pos.x == 0.0
                    && self.target_pos.y == 0.0
                    && self.target_pos.z == 0.0
                {
                    for id in &self.special_object_id_list {
                        if let Some(special_object) = TheGameLogic::find_object_by_id(*id) {
                            if let Ok(guard) = special_object.read() {
                                if let Some(module) = guard.find_update_module("StickyBombUpdate") {
                                    let _ = module.with_module_downcast::<crate::object::behavior::sticky_bomb_update::StickyBombUpdateModule, _, _>(
                                        |module| {
                                            module.behavior_mut().detonate();
                                        },
                                    );
                                    ok_to_lose_stealth = false;
                                }
                            }
                        }
                    }
                } else {
                    let target = match TheGameLogic::find_object_by_id(self.target_id) {
                        Some(target) => target,
                        None => return,
                    };
                    if let Some(charge) = self.create_special_object() {
                        let module = match charge.read() {
                            Ok(guard) => guard.find_update_module("StickyBombUpdate"),
                            Err(_) => None,
                        };
                        if let Some(module) = module {
                            let _ = module.with_module_downcast::<crate::object::behavior::sticky_bomb_update::StickyBombUpdateModule, _, _>(
                                |module| {
                                    let update = module.behavior_mut();
                                    let Ok(target_guard) = target.read() else {
                                        return;
                                    };
                                    let Some(owner_obj) = self.get_object() else {
                                        return;
                                    };
                                    let Ok(owner_guard) = owner_obj.read() else {
                                        return;
                                    };
                                    update.init_sticky_bomb(Some(&*target_guard), Some(&*owner_guard), None);
                                },
                            );
                        } else {
                            self.kill_special_objects();
                            return;
                        }
                    }
                }
            }
            crate::object::special_power_types::SpecialPowerType::DisguiseAsVehicle => {
                let target = match TheGameLogic::find_object_by_id(self.target_id) {
                    Some(target) => target,
                    None => return,
                };
                let Some(obj) = self.get_object() else {
                    return;
                };
                let template_name = target
                    .read()
                    .ok()
                    .map(|g| g.get_template().get_name().to_string());
                if let Some(template_name) = template_name {
                    if let Ok(obj_guard) = obj.read() {
                        if let Some(module) = obj_guard.find_update_module("StealthUpdate") {
                            let _ = module.with_module_downcast::<crate::object::update::stealth_update::StealthUpdate, _, _>(
                                |module| {
                                    let controller_arc = module.get_controller();
                                    let controller_lock = controller_arc.lock();
                                    if let Ok(mut controller) = controller_lock {
                                        controller.disguise_as_object(
                                            Some(template_name),
                                            TheGameLogic::get_frame(),
                                        );
                                    }
                                },
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        if self.module_data.lose_stealth_on_trigger && ok_to_lose_stealth {
            if let Some(obj) = self.get_object() {
                if let Ok(obj_guard) = obj.read() {
                    if let Some(stealth) = obj_guard.get_stealth() {
                        let _ = stealth.lock().map(|mut s| s.mark_as_detected());
                    }
                }
            }
        }
    }

    fn create_special_object(&mut self) -> Option<Arc<RwLock<Object>>> {
        if self.special_object_id_list.len() as UnsignedInt == self.module_data.max_special_objects
        {
            if self.module_data.special_objects_persistent {
                return None;
            }
            self.kill_special_objects();
        }

        let template =
            TheThingFactory::find_template(self.module_data.special_object_name.as_str())?;
        let factory = TheThingFactory::get().ok()?;
        let owner = self.get_object()?;
        let team = owner.read().ok().and_then(|o| o.get_team());

        let new_object = if let Some(team_arc) = team {
            if let Ok(team_guard) = team_arc.read() {
                factory.new_object(template, &*team_guard).ok()?
            } else {
                factory.new_object_optional_team(template, None).ok()?
            }
        } else {
            factory.new_object_optional_team(template, None).ok()?
        };

        let owner_guard = owner.read().ok();
        if let Ok(mut new_guard) = new_object.write() {
            if let Some(owner_guard) = owner_guard.as_ref() {
                let _ = new_guard.set_position(owner_guard.get_position());
                let _ = new_guard.set_orientation(owner_guard.get_orientation());
            }
            if let Some(tracker) = new_guard.get_experience_tracker() {
                if let Some(owner_guard) = owner_guard.as_ref() {
                    let _ = tracker
                        .lock()
                        .map(|mut t| t.set_experience_sink(owner_guard.get_id()));
                }
            }
            if let Some(physics) = new_guard.get_physics() {
                physics.set_pitch_rate(0.0);
                physics.set_allow_airborne_friction(false);
            }
        }

        if let Ok(new_guard) = new_object.read() {
            self.special_object_id_list.push(new_guard.get_id());
        }

        Some(new_object)
    }

    fn is_facing(&mut self) -> bool {
        let Some(obj) = self.get_object() else {
            return true;
        };
        let ai = obj.read().ok().and_then(|g| g.get_ai_update_interface());
        let Some(ai) = ai else {
            return true;
        };

        if !self.facing_complete && self.facing_initiated {
            if ai.lock().map(|a| a.is_idle()).unwrap_or(false) {
                self.facing_complete = true;
                return false;
            }
            return true;
        }
        false
    }

    fn need_to_face(&self) -> bool {
        let Some(obj) = self.get_object() else {
            return false;
        };
        let ai = obj.read().ok().and_then(|g| g.get_ai_update_interface());
        if ai.is_none() {
            return false;
        }
        if !self.module_data.need_to_face_target {
            return false;
        }
        !self.facing_initiated || !self.facing_complete
    }

    fn start_facing(&mut self) {
        let Some(obj) = self.get_object() else {
            return;
        };
        let ai = obj.read().ok().and_then(|g| g.get_ai_update_interface());
        let Some(ai) = ai else {
            return;
        };

        ai.ai_idle(CMD_FROM_AI);
        if let Ok(obj_guard) = obj.read() {
            if let Some(physics) = obj_guard.get_physics() {
                physics.reset_dynamic_physics();
            }
        }

        self.facing_initiated = true;
        let target_pos = if self.target_id != INVALID_ID {
            TheGameLogic::find_object_by_id(self.target_id)
                .and_then(|t| t.read().ok().map(|g| *g.get_position()))
        } else if self.target_pos.x != 0.0 || self.target_pos.y != 0.0 || self.target_pos.z != 0.0 {
            Some(self.target_pos)
        } else {
            None
        };

        if let (Some(target_pos), Ok(mut obj_guard)) = (target_pos, obj.write()) {
            let dx = target_pos.x - obj_guard.get_position().x;
            let dy = target_pos.y - obj_guard.get_position().y;
            let angle = dy.atan2(dx);
            let _ = obj_guard.set_orientation(angle);
            if let Ok(mut ai_guard) = ai.lock() {
                ai_guard.set_locomotor_goal_orientation(angle);
            }
        }

        self.facing_complete = true;
    }

    fn handle_packing_processing(&mut self) -> bool {
        if self.anim_frames == 0 {
            return false;
        }

        self.anim_frames = self.anim_frames.saturating_sub(1);
        if self.anim_frames == 0 {
            if let Some(obj) = self.get_object() {
                if let Ok(mut obj_guard) = obj.write() {
                    obj_guard
                        .clear_model_condition_flags(
                            ModelConditionFlags::Unpacking | ModelConditionFlags::Packing,
                        )
                        .unwrap_or_else(|err| {
                            log::debug!(
                            "SpecialAbilityUpdate::handle_packing_processing clear flags: {err}"
                        )
                        });
                    match self.packing_state {
                        PackingState::Unpacking => {
                            if self.module_data.flip_object_after_unpacking {
                                let orientation = obj_guard.get_orientation();
                                let _ = obj_guard.set_orientation(orientation + PI);
                            }
                            self.packing_state = PackingState::Unpacked;
                        }
                        PackingState::Packing => {
                            if self.module_data.flip_object_after_packing {
                                let orientation = obj_guard.get_orientation();
                                let _ = obj_guard.set_orientation(orientation + PI);
                            }
                            self.packing_state = PackingState::Packed;
                            self.finish_ability();
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            return false;
        }

        if self.module_data.lose_stealth_on_trigger
            && self.anim_frames < self.module_data.pre_trigger_unstealth_frames
        {
            if let Some(obj) = self.get_object() {
                if let Ok(obj_guard) = obj.read() {
                    if let Some(stealth) = obj_guard.get_stealth() {
                        if let Ok(mut stealth_guard) = stealth.lock() {
                            stealth_guard.mark_as_detected();
                        }
                    }
                }
            }
        }

        true
    }

    fn need_to_pack(&self) -> bool {
        if self.packing_state != PackingState::Unpacked {
            return false;
        }
        if self.module_data.skip_packing_with_no_target && self.no_target_command {
            return false;
        }
        self.module_data.pack_time > 0
    }

    fn need_to_unpack(&self) -> bool {
        if self.packing_state != PackingState::Packed {
            return false;
        }
        if self.module_data.skip_packing_with_no_target && self.no_target_command {
            return false;
        }
        self.module_data.unpack_time > 0
    }

    fn start_packing(&mut self, _success: bool) {
        let variation = if self.module_data.pack_unpack_variation_factor > 0.0 {
            get_game_logic_random_value_real(
                1.0 - self.module_data.pack_unpack_variation_factor,
                1.0 + self.module_data.pack_unpack_variation_factor,
            )
        } else {
            1.0
        };

        self.packing_state = PackingState::Packing;
        self.anim_frames = (self.module_data.pack_time as Real * variation) as u32;

        if let Some(obj) = self.get_object() {
            if let Ok(mut obj_guard) = obj.write() {
                obj_guard
                    .clear_and_set_model_condition_flags(
                        ModelConditionFlags::Unpacking,
                        ModelConditionFlags::Packing,
                    )
                    .unwrap_or_else(|err| {
                        log::debug!("SpecialAbilityUpdate::start_packing set flags: {err}")
                    });

                if let Some(sound) = self.module_data.pack_sound.as_ref() {
                    if let Some(audio) = TheAudio::get() {
                        let mut event = sound.clone();
                        event.set_object_id(obj_guard.get_id());
                        let handle = audio.add_audio_event(&event);
                        event.set_playing_handle(handle);
                    }
                }

                if self.anim_frames > 0 {
                    obj_guard.set_animation_completion_time(self.anim_frames);
                }

                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    let _ = ai.lock().map(|mut ai_guard| ai_guard.ai_busy(CMD_FROM_AI));
                }
            }
        }
    }

    fn start_unpacking(&mut self) {
        let variation = if self.module_data.pack_unpack_variation_factor > 0.0 {
            get_game_logic_random_value_real(
                1.0 - self.module_data.pack_unpack_variation_factor,
                1.0 + self.module_data.pack_unpack_variation_factor,
            )
        } else {
            1.0
        };

        self.packing_state = PackingState::Unpacking;
        self.anim_frames = (self.module_data.unpack_time as Real * variation) as u32;

        if let Some(obj) = self.get_object() {
            if let Ok(mut obj_guard) = obj.write() {
                obj_guard
                    .clear_and_set_model_condition_flags(
                        ModelConditionFlags::Packing,
                        ModelConditionFlags::Unpacking,
                    )
                    .unwrap_or_else(|err| {
                        log::debug!("SpecialAbilityUpdate::start_unpacking set flags: {err}")
                    });

                if let Some(sound) = self.module_data.unpack_sound.as_ref() {
                    if let Some(audio) = TheAudio::get() {
                        let mut event = sound.clone();
                        event.set_object_id(obj_guard.get_id());
                        let handle = audio.add_audio_event(&event);
                        event.set_playing_handle(handle);
                    }
                }

                if self.anim_frames > 0 {
                    obj_guard.set_animation_completion_time(self.anim_frames);
                }

                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    let _ = ai.lock().map(|mut ai_guard| ai_guard.ai_busy(CMD_FROM_AI));
                }
            }
        }
    }

    fn finish_ability(&mut self) {
        self.within_start_ability_range = false;
        self.packing_state = PackingState::None;

        let valid_target = self.target_id != INVALID_ID
            || self.target_pos.x != 0.0
            || self.target_pos.y != 0.0
            || self.target_pos.z != 0.0;

        if self.module_data.flee_range_after_completion > 0.0 && valid_target {
            if let Some(obj) = self.get_object() {
                if let Ok(obj_guard) = obj.read() {
                    let (dir_x, dir_y) = obj_guard.get_unit_direction_vector_2d();
                    let mut pos = *obj_guard.get_position();
                    let scale = self.module_data.flee_range_after_completion;
                    if self.module_data.flip_object_after_unpacking
                        || self.module_data.flip_object_after_packing
                    {
                        pos.x += dir_x * scale;
                        pos.y += dir_y * scale;
                    } else {
                        pos.x -= dir_x * scale;
                        pos.y -= dir_y * scale;
                    }

                    if let Some(ai) = obj_guard.get_ai_update_interface() {
                        if let Some(physics) = obj_guard.get_physics() {
                            physics.apply_motive_force(&Coord3D::ZERO);
                        }
                        ai.ai_move_to_position(&pos, false, CMD_FROM_AI);
                        if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                            if let Ok(target_guard) = target.read() {
                                let _ = ai.lock().map(|mut guard| {
                                    let _ = guard.ignore_obstacle(Some(&target));
                                });
                            }
                        }
                    }
                }
            }
        } else if let Some(obj) = self.get_object() {
            if let Ok(obj_guard) = obj.read() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    ai.ai_idle(CMD_FROM_AI);
                }
            }
        }

        self.on_exit(false);
    }

    fn validate_special_objects(&mut self) {
        self.special_object_id_list
            .retain(|id| TheGameLogic::find_object_by_id(*id).is_some());
    }

    fn kill_special_objects(&mut self) {
        for id in self.special_object_id_list.drain(..) {
            if let Err(err) = TheGameLogic::destroy_object_by_id(id) {
                warn!("Failed to destroy special object {}: {}", id, err);
            }
        }

        if let Some(template) = self.module_data.special_power_template.as_ref() {
            if template.get_special_power_type()
                == crate::object::special_power_types::SpecialPowerType::MissileDefenderLaserGuidedMissiles
            {
                if let Some(obj) = self.get_object() {
                    if let Ok(mut obj_guard) = obj.write() {
                        obj_guard.set_weapon_lock(
                            WeaponSlotType::Primary,
                            WeaponLockType::LockedTemporarily,
                        );
                    }
                }
            }
        }
    }
}

impl UpdateModuleInterface for SpecialAbilityUpdate {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.validate_special_objects();

        let Some(obj) = self.get_object() else {
            return Ok(UPDATE_SLEEP_FOREVER);
        };

        if let Ok(obj_guard) = obj.read() {
            if obj_guard.is_effectively_dead() {
                self.on_exit(true);
                return Ok(self.calc_sleep_time());
            }
        }

        if !self.active {
            return Ok(self.calc_sleep_time());
        }

        let ai = {
            let Ok(obj_guard) = obj.read() else {
                self.on_exit(false);
                return Ok(self.calc_sleep_time());
            };
            obj_guard.get_ai_update_interface()
        };

        let Some(ai) = ai else {
            self.on_exit(false);
            return Ok(self.calc_sleep_time());
        };

        if let Ok(ai_guard) = ai.lock() {
            if ai_guard.get_last_command_source() != CMD_FROM_AI {
                self.on_exit(false);
                return Ok(self.calc_sleep_time());
            }

            if ai_guard.is_moving()
                && self.is_power_currently_in_use(None)
                && !self.facing_initiated
            {
                match self.get_special_power_type() {
                    Some(CommonSpecialPowerType::SpecialInfantryCaptureBuilding)
                    | Some(CommonSpecialPowerType::SpecialBlackLotusCaptureBuilding) => {
                        self.on_exit(false);
                        return Ok(self.calc_sleep_time());
                    }
                    _ => {}
                }
            }
        }

        if self.handle_packing_processing() {
            return Ok(self.calc_sleep_time());
        }

        let mut should_abort = false;
        if self.target_id != INVALID_ID {
            if let Some(target) = TheGameLogic::find_object_by_id(self.target_id) {
                if let Ok(target_guard) = target.read() {
                    if target_guard.is_effectively_dead() {
                        should_abort = true;
                    } else if let Some(sp_type) = self.get_special_power_type() {
                        match sp_type {
                            CommonSpecialPowerType::SpecialInfantryCaptureBuilding
                            | CommonSpecialPowerType::SpecialBlackLotusCaptureBuilding
                            | CommonSpecialPowerType::SpecialHackerDisableBuilding => {
                                if let Some(obj) = self.get_object() {
                                    if let Ok(obj_guard) = obj.read() {
                                        if obj_guard.relationship_to(&target_guard)
                                            == Relationship::Allies
                                        {
                                            should_abort = true;
                                        }
                                    }
                                }
                                if target_guard.test_status(ObjectStatusTypes::Stealthed)
                                    && !target_guard.test_status(ObjectStatusTypes::Detected)
                                    && !self.is_preparation_complete()
                                {
                                    should_abort = true;
                                }
                            }
                            CommonSpecialPowerType::SpecialBlackLotusStealCashHack
                            | CommonSpecialPowerType::SpecialBoobyTrap => {
                                if target_guard.test_status(ObjectStatusTypes::Stealthed)
                                    && !target_guard.test_status(ObjectStatusTypes::Detected)
                                    && !self.is_preparation_complete()
                                {
                                    should_abort = true;
                                }
                            }
                            CommonSpecialPowerType::SpecialRemoteCharges
                            | CommonSpecialPowerType::SpecialTimedCharges => {
                                if !self.need_to_unpack()
                                    && target_guard.test_status(ObjectStatusTypes::Stealthed)
                                    && !target_guard.test_status(ObjectStatusTypes::Detected)
                                    && !self.is_preparation_complete()
                                {
                                    should_abort = true;
                                }
                            }
                            CommonSpecialPowerType::SpecialMissileDefenderLaserGuidedMissiles => {
                                if target_guard.is_kind_of(crate::common::KindOf::Structure) {
                                    should_abort = true;
                                }
                            }
                            CommonSpecialPowerType::SpecialBlackLotusDisableVehicleHack => {
                                if target_guard.test_status(ObjectStatusTypes::Stealthed)
                                    && !target_guard.test_status(ObjectStatusTypes::Detected)
                                {
                                    should_abort = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        let spm_exists = self.with_spm(|_| ()).is_some();
        if should_abort || self.get_template().is_none() || !spm_exists {
            if let Some(obj) = self.get_object() {
                if let Ok(obj_guard) = obj.read() {
                    if let Some(ai) = obj_guard.get_ai_update_interface() {
                        ai.ai_idle(CMD_FROM_AI);
                    }
                }
            }
            self.on_exit(false);
            return Ok(self.calc_sleep_time());
        }

        let mut spm_ready = true;
        if self.is_persistent_ability() && self.module_data.persistence_requires_recharge {
            let ready = self.with_spm(|spm| {
                spm.is_ready() && spm.get_ready_frame() < TheGameLogic::get_frame()
            });
            spm_ready = ready.unwrap_or(true);
        }

        if !self.is_preparation_complete() {
            if spm_ready {
                self.prep_frames = self.prep_frames.saturating_sub(1);
            }
            if self.is_preparation_complete() {
                self.trigger_ability_effect();
                if self.is_persistent_ability() {
                    self.reset_preparation();
                    if self.module_data.persistence_requires_recharge {
                        let _ = self.with_spm(|spm| spm.start_power_recharge());
                    }
                } else {
                    self.end_preparation();
                    if self.need_to_pack() {
                        self.start_packing(true);
                    } else {
                        self.finish_ability();
                    }
                }
            } else {
                if !self.continue_preparation() {
                    self.end_preparation();
                    if self.need_to_pack() {
                        self.start_packing(false);
                    } else {
                        self.finish_ability();
                    }
                }
            }
        } else if self.is_within_start_ability_range() {
            self.within_start_ability_range = true;
            if !self.is_facing() && self.need_to_face() {
                self.start_facing();
                return Ok(self.calc_sleep_time());
            }

            if self.need_to_unpack() {
                self.start_unpacking();
                return Ok(self.calc_sleep_time());
            }

            if self.packing_state == PackingState::Unpacked {
                self.start_preparation();
                if self.is_preparation_complete() {
                    self.trigger_ability_effect();

                    if self.is_persistent_ability()
                        && self.module_data.persistence_requires_recharge
                    {
                        self.reset_preparation();
                        let _ = self.with_spm(|spm| spm.start_power_recharge());
                        return Ok(self.calc_sleep_time());
                    } else {
                        self.end_preparation();
                    }

                    if self.need_to_pack() {
                        self.start_packing(true);
                    } else {
                        self.finish_ability();
                    }
                }
            }
        } else if ai.lock().map(|g| g.is_idle()).unwrap_or(false) {
            self.approach_target();
        }

        Ok(self.calc_sleep_time())
    }
}

impl BehaviorModuleInterface for SpecialAbilityUpdate {
    // get_module_data, crc, save, load removed as they are not part of BehaviorModuleInterface in this codebase
}

impl SpecialPowerUpdateInterface for SpecialAbilityUpdate {
    fn does_special_power_update_pass_science_test(&self) -> bool {
        self.base.does_special_power_update_pass_science_test()
    }

    fn get_extra_required_science(&self) -> ScienceType {
        self.base.get_extra_required_science()
    }

    fn initiate_intent_to_do_special_power(
        &mut self,
        special_power_template: &SpecialPowerTemplate,
        target_obj: Option<ObjectID>,
        target_pos: Option<&Coord3D>,
        _waypoint: Option<&Waypoint>,
        _command_options: SpecialPowerCommandOptions,
    ) -> bool {
        // Verify template matches
        if let Some(ref my_template) = self.module_data.special_power_template {
            if my_template.get_name() != special_power_template.get_name() {
                return false;
            }
        } else {
            return false;
        }

        self.target_id = target_obj.unwrap_or(INVALID_ID);
        self.target_pos = target_pos.cloned().unwrap_or_default();
        self.location_count = 0;
        self.prep_frames = 0;
        self.anim_frames = 0;
        self.packing_state = PackingState::Packed;
        self.facing_initiated = false;
        self.facing_complete = false;
        self.within_start_ability_range = false;

        // Clear model conditions
        if let Some(obj) = self.get_object() {
            // obj.write().clear_model_condition_flags(...)
            // Clear AI
            // obj.write().get_ai_update_interface().ai_idle(CMD_FROM_AI);
        }

        self.no_target_command = target_obj.is_none() && target_pos.is_none();

        if self.module_data.unpack_time == 0
            || (self.no_target_command && self.module_data.skip_packing_with_no_target)
        {
            self.packing_state = PackingState::Unpacked;
        }

        self.active = true;
        if let Some(obj) = self.get_object() {
            let obj_id = obj.read().map(|guard| guard.id).unwrap_or(INVALID_ID);
            if obj_id != INVALID_ID {
                TheGameLogic::set_wake_frame(obj_id, UPDATE_SLEEP_NONE);
            }
        }
        true
    }

    fn is_special_ability(&self) -> bool {
        true
    }

    fn is_special_power(&self) -> bool {
        false
    }

    fn is_active(&self) -> bool {
        self.active
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

    fn set_special_power_overridable_destination(&mut self, location: &Coord3D) {
        self.target_pos = *location;
    }

    fn update_special_power(
        &mut self,
        _frame_time: f32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn is_power_ready(&self) -> bool {
        true
    }

    fn is_power_currently_in_use(&self, command: Option<&CommandButton>) -> bool {
        if let Some(command) = command {
            if let Some(template) = command.get_special_power_template() {
                if template.get_special_power_type()
                    == crate::object::special_power_types::SpecialPowerType::RemoteCharges
                    && !command.is_context_command()
                {
                    return self.special_object_id_list.is_empty();
                }
            }
        }

        if self.packing_state != PackingState::None {
            if let Some(command) = command {
                if let Some(template) = command.get_special_power_template() {
                    if (self.packing_state == PackingState::Packing
                        || self.packing_state == PackingState::Packed)
                        && template.get_reload_time() == 0
                    {
                        return false;
                    }
                }
            }

            if self.within_start_ability_range {
                return true;
            }
        }
        false
    }
}

pub struct SpecialAbilityUpdateFactory;

impl SpecialAbilityUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        let object_ptr = Arc::downgrade(&thing);
        Ok(Box::new(SpecialAbilityUpdate::new(object_ptr, module_data)))
    }
}

pub struct SpecialAbilityUpdateModule {
    behavior: SpecialAbilityUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<SpecialAbilityUpdateModuleData>,
}

impl SpecialAbilityUpdateModule {
    pub fn new(
        behavior: SpecialAbilityUpdate,
        module_name: &AsciiString,
        module_data: Arc<SpecialAbilityUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut SpecialAbilityUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for SpecialAbilityUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Module for SpecialAbilityUpdateModule {
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
