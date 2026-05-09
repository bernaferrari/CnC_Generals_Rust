//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/SlowDeathBehavior.cpp`.
//!
//! SlowDeathBehavior - Rust conversion of C++ SlowDeathBehavior
//!
//! Update that will count down a lifetime and destroy object when it reaches zero
//! Original Author: Colin Day, December 2001
//! Rust conversion: 2025

use crate::common::INVALID_ID;
use crate::common::{
    Bool, Byte, Coord3D, DisabledMaskType, DisabledType, ICoord3D, Int, KindOf, Matrix3D,
    ModuleData, ObjectID, Real, UnsignedInt, Vector3, LOGICFRAMES_PER_SECOND, PI,
    SECONDS_PER_LOGICFRAME_REAL,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

// Forward declarations - assume these exist
use crate::common::{
    GameLogic, GameLogicRandomValue, GameLogicRandomValueReal, ModelConditionFlags,
    PartitionManager, TheFXListStore, TheGameLODManager, TheGameLogic, TheObjectCreationListStore,
    MODELCONDITION_EXPLODED_BOUNCING, MODELCONDITION_EXPLODED_FLAILING, MODELCONDITION_PARACHUTING,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::drawable::Drawable;
use crate::effects::{FXList, ObjectCreationList};
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BehaviorModule, BehaviorModuleInterface,
    BodyModuleInterface, DieModuleInterface, ModuleInterface, PhysicsBehavior, PhysicsBehaviorExt,
    SlavedUpdateInterface, SlowDeathBehaviorInterface as ModuleSlowDeathBehaviorInterface,
    UpdateModule, UpdateModuleInterface, UpdateSleepTime, MODULEINTERFACE_DIE, UPDATE_SLEEP,
    UPDATE_SLEEP_FOREVER, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::weapon::with_weapon_store;
use crate::MAKE_MODELCONDITION_MASK;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
pub type DieMuxData = crate::object::die::DieMuxData;
use crate::helpers::TheWeaponStore;
use crate::object::die::{
    parse_death_type_flags_tokens, parse_object_status_mask_tokens,
    parse_veterancy_level_flags_tokens,
};
use crate::object::drawable::DrawableArcExt;
use crate::object::{Object, ObjectArcExt, ObjectStatusTypes};
use crate::weapon::{WeaponStore, WeaponTemplate};
use game_engine::common::ini::{FieldParse, INIError, INI};

// Constants
const BEGIN_MIDPOINT_RATIO: Real = 0.35;
const END_MIDPOINT_RATIO: Real = 0.65;

/// Slow death phase types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlowDeathPhaseType {
    Initial = 0,
    Midpoint,
    Final,
}

impl SlowDeathPhaseType {
    pub const COUNT: usize = 3;

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(SlowDeathPhaseType::Initial),
            1 => Some(SlowDeathPhaseType::Midpoint),
            2 => Some(SlowDeathPhaseType::Final),
            _ => None,
        }
    }

    pub fn to_index(self) -> usize {
        self as usize
    }
}

/// Module data for SlowDeathBehavior
#[derive(Debug, Clone)]
pub struct SlowDeathBehaviorModuleData {
    pub die_mux_data: DieMuxData,
    pub sink_rate: Real,
    pub probability_modifier: Int,
    pub modifier_bonus_per_overkill_percent: Real,
    pub sink_delay: UnsignedInt,
    pub sink_delay_variance: UnsignedInt,
    pub destruction_altitude: Real,
    pub destruction_delay: UnsignedInt,
    pub destruction_delay_variance: UnsignedInt,
    pub fx: [Vec<Arc<FXList>>; SlowDeathPhaseType::COUNT],
    pub ocls: [Vec<Arc<ObjectCreationList>>; SlowDeathPhaseType::COUNT],
    pub weapons: [Vec<Arc<WeaponTemplate>>; SlowDeathPhaseType::COUNT],
    pub fling_force: Real,
    pub fling_force_variance: Real,
    pub fling_pitch: Real,
    pub fling_pitch_variance: Real,
    pub mask_of_loaded_effects: Byte,
}

impl ModuleData for SlowDeathBehaviorModuleData {}

impl SlowDeathBehaviorModuleData {
    // Effect mask flags
    pub const HAS_FX: u8 = 1;
    pub const HAS_OCL: u8 = 2;
    pub const HAS_WEAPON: u8 = 4;
    pub const HAS_NON_LOD_EFFECTS: u8 = Self::HAS_OCL | Self::HAS_WEAPON;

    pub fn new() -> Self {
        Self {
            die_mux_data: DieMuxData::default(),
            sink_rate: 0.0,
            probability_modifier: 10,
            modifier_bonus_per_overkill_percent: 0.0,
            sink_delay: 0,
            sink_delay_variance: 0,
            destruction_delay: 0,
            destruction_delay_variance: 0,
            destruction_altitude: -10.0,
            fx: Default::default(),
            ocls: Default::default(),
            weapons: Default::default(),
            fling_force: 0.0,
            fling_force_variance: 0.0,
            fling_pitch: 0.0,
            fling_pitch_variance: 0.0,
            mask_of_loaded_effects: 0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SLOW_DEATH_BEHAVIOR_FIELDS)
    }

    pub fn has_non_lod_effects(&self) -> bool {
        (self.mask_of_loaded_effects & Self::HAS_NON_LOD_EFFECTS) != 0
    }
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(token)
}

fn parse_slow_death_phase(token: &str) -> Option<SlowDeathPhaseType> {
    match token.to_ascii_uppercase().as_str() {
        "INITIAL" => Some(SlowDeathPhaseType::Initial),
        "MIDPOINT" => Some(SlowDeathPhaseType::Midpoint),
        "FINAL" => Some(SlowDeathPhaseType::Final),
        _ => None,
    }
}

pub(crate) fn parse_fx(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let phase_token = tokens.first().ok_or(INIError::InvalidData)?;
    let Some(phase) = parse_slow_death_phase(phase_token) else {
        return Err(INIError::InvalidData);
    };
    for token in tokens.iter().skip(1) {
        for name in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if let Some(fx) = TheFXListStore::find_fx_list(name) {
                data.fx[phase.to_index()].push(fx);
                data.mask_of_loaded_effects |= SlowDeathBehaviorModuleData::HAS_FX;
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_ocl(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let phase_token = tokens.first().ok_or(INIError::InvalidData)?;
    let Some(phase) = parse_slow_death_phase(phase_token) else {
        return Err(INIError::InvalidData);
    };
    for token in tokens.iter().skip(1) {
        for name in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if let Some(ocl) = TheObjectCreationListStore::find_object_creation_list(name) {
                data.ocls[phase.to_index()].push(ocl);
                data.mask_of_loaded_effects |= SlowDeathBehaviorModuleData::HAS_OCL;
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_weapon(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let phase_token = tokens.first().ok_or(INIError::InvalidData)?;
    let Some(phase) = parse_slow_death_phase(phase_token) else {
        return Err(INIError::InvalidData);
    };
    for token in tokens.iter().skip(1) {
        for name in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            let template = with_weapon_store(|store| store.find_weapon_template(name).cloned())
                .ok()
                .flatten();
            if let Some(weapon) = template {
                data.weapons[phase.to_index()].push(weapon);
                data.mask_of_loaded_effects |= SlowDeathBehaviorModuleData::HAS_WEAPON;
            }
        }
    }
    Ok(())
}

pub(crate) fn parse_sink_rate(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.sink_rate = INI::parse_real(token)?;
    Ok(())
}

pub(crate) fn parse_probability_modifier(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.probability_modifier = INI::parse_int(token)?;
    Ok(())
}

pub(crate) fn parse_modifier_bonus_per_overkill_percent(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.modifier_bonus_per_overkill_percent = INI::parse_percent_to_real(token)?;
    Ok(())
}

pub(crate) fn parse_sink_delay(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.sink_delay = parse_duration_frames(tokens)?;
    Ok(())
}

pub(crate) fn parse_sink_delay_variance(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.sink_delay_variance = parse_duration_frames(tokens)?;
    Ok(())
}

pub(crate) fn parse_destruction_delay(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.destruction_delay = parse_duration_frames(tokens)?;
    Ok(())
}

pub(crate) fn parse_destruction_delay_variance(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.destruction_delay_variance = parse_duration_frames(tokens)?;
    Ok(())
}

pub(crate) fn parse_destruction_altitude(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.destruction_altitude = INI::parse_real(token)?;
    Ok(())
}

pub(crate) fn parse_fling_force(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.fling_force = INI::parse_real(token)?;
    Ok(())
}

pub(crate) fn parse_fling_force_variance(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.fling_force_variance = INI::parse_real(token)?;
    Ok(())
}

pub(crate) fn parse_fling_pitch(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.fling_pitch = INI::parse_angle_real(token)?;
    Ok(())
}

pub(crate) fn parse_fling_pitch_variance(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.fling_pitch_variance = INI::parse_angle_real(token)?;
    Ok(())
}

pub(crate) fn parse_death_types(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.death_types = parse_death_type_flags_tokens(tokens)?;
    Ok(())
}

pub(crate) fn parse_veterancy_levels(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.veterancy_levels = parse_veterancy_level_flags_tokens(tokens)?;
    Ok(())
}

pub(crate) fn parse_exempt_status(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.exempt_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

pub(crate) fn parse_required_status(
    _ini: &mut INI,
    data: &mut SlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.required_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

const SLOW_DEATH_BEHAVIOR_FIELDS: &[FieldParse<SlowDeathBehaviorModuleData>] = &[
    FieldParse {
        token: "SinkRate",
        parse: parse_sink_rate,
    },
    FieldParse {
        token: "ProbabilityModifier",
        parse: parse_probability_modifier,
    },
    FieldParse {
        token: "ModifierBonusPerOverkillPercent",
        parse: parse_modifier_bonus_per_overkill_percent,
    },
    FieldParse {
        token: "SinkDelay",
        parse: parse_sink_delay,
    },
    FieldParse {
        token: "SinkDelayVariance",
        parse: parse_sink_delay_variance,
    },
    FieldParse {
        token: "DestructionDelay",
        parse: parse_destruction_delay,
    },
    FieldParse {
        token: "DestructionDelayVariance",
        parse: parse_destruction_delay_variance,
    },
    FieldParse {
        token: "DestructionAltitude",
        parse: parse_destruction_altitude,
    },
    FieldParse {
        token: "FX",
        parse: parse_fx,
    },
    FieldParse {
        token: "OCL",
        parse: parse_ocl,
    },
    FieldParse {
        token: "Weapon",
        parse: parse_weapon,
    },
    FieldParse {
        token: "FlingForce",
        parse: parse_fling_force,
    },
    FieldParse {
        token: "FlingForceVariance",
        parse: parse_fling_force_variance,
    },
    FieldParse {
        token: "FlingPitch",
        parse: parse_fling_pitch,
    },
    FieldParse {
        token: "FlingPitchVariance",
        parse: parse_fling_pitch_variance,
    },
    FieldParse {
        token: "DeathTypes",
        parse: parse_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_required_status,
    },
];

/// Interface for slow death behavior
pub trait SlowDeathBehaviorInterface: Send + Sync {
    fn begin_slow_death(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn get_probability_modifier(&self, damage_info: &DamageInfo) -> Int;
    fn is_die_applicable(&self, damage_info: &DamageInfo) -> bool;
}

/// Main SlowDeathBehavior implementation
#[derive(Debug)]
pub struct SlowDeathBehavior {
    // Base module data
    object: Option<Arc<RwLock<Object>>>,
    module_data: Arc<SlowDeathBehaviorModuleData>,

    // State tracking
    next_call_frame_and_phase: UnsignedInt,
    flags: UnsignedInt,
    sink_frame: UnsignedInt,
    midpoint_frame: UnsignedInt,
    destruction_frame: UnsignedInt,
    accelerated_time_scale: Real,
}

impl SlowDeathBehavior {
    // Flag bits
    const SLOW_DEATH_ACTIVATED: u32 = 0;
    const MIDPOINT_EXECUTED: u32 = 1;
    const FLUNG_INTO_AIR: u32 = 2;
    const BOUNCED: u32 = 3;

    pub fn new(
        _thing: Arc<RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let data = {
            let data_ref = module_data
                .as_any()
                .downcast_ref::<SlowDeathBehaviorModuleData>()
                .ok_or("Invalid module data type")?;
            data_ref.clone()
        };

        if data.probability_modifier < 1 {
            return Err("ProbabilityModifier must be >= 1".into());
        }

        Ok(Self {
            object: None, // Will be set after creation
            module_data: Arc::new(data),
            next_call_frame_and_phase: 0,
            flags: 0,
            sink_frame: 0,
            midpoint_frame: 0,
            destruction_frame: 0,
            accelerated_time_scale: 1.0,
        })
    }

    pub fn set_object(&mut self, object: Arc<RwLock<Object>>) {
        self.object = Some(object);
    }

    fn get_object(&self) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        self.object.clone().ok_or("Object not set".into())
    }

    fn is_slow_death_activated(&self) -> bool {
        (self.flags & (1 << Self::SLOW_DEATH_ACTIVATED)) != 0
    }

    #[allow(dead_code)]
    fn get_destruction_frame(&self) -> UnsignedInt {
        self.destruction_frame
    }

    /// Execute FX, OCLs, and weapons for a specific death phase
    /// (Matches C++ SlowDeathBehavior::doPhaseStuff at line 315)
    fn do_phase_stuff(
        &mut self,
        phase: SlowDeathPhaseType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = &self.module_data;

        // Early exit if no effects (C++ line 320-321)
        if data.mask_of_loaded_effects == 0 {
            return Ok(());
        }

        let phase_index = phase.to_index();
        let object = self.get_object()?;
        let obj_read = object.read().map_err(|e| format!("Lock error: {}", e))?;

        // Execute FX list if present (C++ lines 323-331)
        let fx_list = &data.fx[phase_index];
        if !fx_list.is_empty() {
            // Pick a random FX from the list (C++ line 326)
            let idx = GameLogicRandomValue(0, fx_list.len() as Int - 1) as usize;
            if let Some(fx) = fx_list.get(idx) {
                // Matches C++ SlowDeathBehavior.cpp:330 - FXList::doFXObj(fxl, getObject(), NULL)
                if let Err(e) = fx.do_fx_obj(&object, None) {
                    log::warn!("Failed to execute FX for phase {:?}: {}", phase, e);
                }
            }
        }

        // Execute OCL if present (C++ lines 333-341)
        let ocl_list = &data.ocls[phase_index];
        if !ocl_list.is_empty() {
            // Pick a random OCL from the list (C++ line 335)
            let idx = GameLogicRandomValue(0, ocl_list.len() as Int - 1) as usize;
            if let Some(ocl) = ocl_list.get(idx) {
                // Matches C++ SlowDeathBehavior.cpp:340 - ObjectCreationList::create(ocl, getObject(), NULL)
                if let Err(e) = ObjectCreationList::create(ocl, &object, None) {
                    log::warn!("Failed to execute OCL for phase {:?}: {}", phase, e);
                }
            }
        }

        // Execute weapon if present (C++ lines 343-354)
        let weapon_list = &data.weapons[phase_index];
        if !weapon_list.is_empty() {
            // Pick a random weapon from the list (C++ line 345)
            let idx = GameLogicRandomValue(0, weapon_list.len() as Int - 1) as usize;
            if let Some(weapon) = weapon_list.get(idx) {
                let position = *obj_read.get_position();
                let object_id = obj_read.get_id();
                drop(obj_read); // Release lock before firing weapon

                // Matches C++ SlowDeathBehavior.cpp:352 - TheWeaponStore->createAndFireTempWeapon(wt, getObject(), getObject()->getPosition())
                if let Some(weapon_store) = TheWeaponStore::get() {
                    if let Err(e) = weapon_store
                        .create_and_fire_temp_weapon_at_pos(weapon, object_id, &position)
                    {
                        log::warn!("Failed to fire weapon for phase {:?}: {}", phase, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Calculate random force for flinging objects
    fn calc_random_force(
        min_mag: Real,
        max_mag: Real,
        min_pitch: Real,
        max_pitch: Real,
    ) -> Coord3D {
        let angle = GameLogicRandomValueReal(-PI, PI);
        let pitch = GameLogicRandomValueReal(min_pitch, max_pitch);
        let magnitude = GameLogicRandomValueReal(min_mag, max_mag);

        let x = magnitude * pitch.cos() * angle.cos();
        let y = magnitude * pitch.cos() * angle.sin();
        let z = magnitude * pitch.sin();

        Coord3D::new(x, y, z)
    }
}

impl UpdateModuleInterface for SlowDeathBehavior {
    /// Update the slow death behavior each frame
    /// (Matches C++ SlowDeathBehavior::update at line 359)
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        // Assert that slow death is activated (C++ line 362)
        if !self.is_slow_death_activated() {
            return Err("SlowDeathBehavior::update called when not activated".into());
        }

        let data = &self.module_data;
        let object = self.get_object()?;

        // Get current time scale from LOD manager (C++ line 367)
        let time_scale = TheGameLODManager::get_slow_death_scale();

        // Check if we need to adjust timing due to LOD changes (C++ lines 370-385)
        if time_scale != 1.0 && self.accelerated_time_scale == 1.0 && !data.has_non_lod_effects() {
            if time_scale == 0.0 {
                // Instant death - destroy immediately (C++ line 377)
                let obj_id = object
                    .read()
                    .map_err(|e| format!("Lock error: {}", e))?
                    .get_id();
                crate::helpers::TheGameLogic::remove_object(obj_id);
                return Ok(UPDATE_SLEEP_NONE);
            }

            // Adjust frame timings for new time scale
            self.sink_frame = ((self.sink_frame as Real) * time_scale) as UnsignedInt;
            self.midpoint_frame = ((self.midpoint_frame as Real) * time_scale) as UnsignedInt;
            self.destruction_frame = ((self.destruction_frame as Real) * time_scale) as UnsignedInt;
            self.accelerated_time_scale = time_scale;
        }

        let now = TheGameLogic::get_frame();

        // Handle flung objects (C++ lines 390-429)
        if (self.flags & (1 << Self::FLUNG_INTO_AIR)) != 0 {
            if (self.flags & (1 << Self::BOUNCED)) == 0 {
                // Keep extending timers while airborne
                self.sink_frame += 1;
                self.midpoint_frame += 1;
                self.destruction_frame += 1;

                let mut obj_write = object.write().map_err(|e| format!("Lock error: {}", e))?;

                if !obj_write.is_above_terrain() {
                    // Object has landed - transition to bouncing
                    obj_write.clear_and_set_model_condition_flags(
                        MAKE_MODELCONDITION_MASK!(MODELCONDITION_EXPLODED_FLAILING),
                        MAKE_MODELCONDITION_MASK!(MODELCONDITION_EXPLODED_BOUNCING),
                    )?;
                    self.flags |= 1 << Self::BOUNCED;
                }

                // Check for collision with trees (C++ lines 406-424)
                if let Some(physics) = obj_write.get_physics() {
                    let tree_id = physics.get_last_collidee();
                    if tree_id != INVALID_ID {
                        if let Some(tree) = TheGameLogic::find_object_by_id(tree_id) {
                            if tree.is_kind_of(KindOf::Shrubbery) {
                                // Caught in tree - disable and sink faster
                                obj_write.set_disabled(DisabledType::Held);
                                obj_write.clear_model_condition_flags(
                                    MAKE_MODELCONDITION_MASK!(MODELCONDITION_EXPLODED_FLAILING),
                                )?;
                                obj_write.clear_model_condition_flags(
                                    MAKE_MODELCONDITION_MASK!(MODELCONDITION_EXPLODED_BOUNCING),
                                )?;
                                obj_write.set_model_condition_flags(MAKE_MODELCONDITION_MASK!(
                                    MODELCONDITION_PARACHUTING
                                ))?;

                                // Sink faster when caught in tree
                                let mut pos = *obj_write.get_position();
                                pos.z -= data.sink_rate * 50.0;
                                obj_write.set_position(&pos)?;

                                if !obj_write.is_above_terrain() {
                                    // Caught in tree and hit ground - destroy object (C++ line 420)
                                    let obj_id = obj_write.get_id();
                                    drop(obj_write);
                                    crate::helpers::TheGameLogic::remove_object(obj_id);
                                    return Ok(UPDATE_SLEEP_NONE);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Handle sinking (C++ lines 431-438)
        if now >= self.sink_frame && data.sink_rate > 0.0 {
            let mut obj_write = object.write().map_err(|e| format!("Lock error: {}", e))?;

            // Disable physics so we can control the sink
            obj_write.set_disabled(DisabledType::Held);

            // Sink the object
            let mut pos = *obj_write.get_position();
            pos.z -= data.sink_rate / self.accelerated_time_scale;
            obj_write.set_position(&pos)?;
        }

        // Handle midpoint effects (C++ lines 440-444)
        if now >= self.midpoint_frame && (self.flags & (1 << Self::MIDPOINT_EXECUTED)) == 0 {
            self.do_phase_stuff(SlowDeathPhaseType::Midpoint)?;
            self.flags |= 1 << Self::MIDPOINT_EXECUTED;
        }

        // Handle final destruction (C++ lines 446-450)
        if now >= self.destruction_frame {
            self.do_phase_stuff(SlowDeathPhaseType::Final)?;
            // Matches C++ line 449: TheGameLogic->destroyObject(obj)
            let obj_id = object
                .read()
                .map_err(|e| format!("Lock error: {}", e))?
                .get_id();
            crate::helpers::TheGameLogic::remove_object(obj_id);
            return Ok(UPDATE_SLEEP_NONE);
        }

        Ok(UPDATE_SLEEP_NONE)
    }
}

impl DieModuleInterface for SlowDeathBehavior {
    /// Called when object dies - selects which slow death behavior to use
    /// (Matches C++ SlowDeathBehavior::onDie at line 456)
    fn on_die(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object = self.get_object()?;
        let obj_write = object.write().map_err(|e| format!("Lock error: {}", e))?;

        // Check if this die module is applicable (C++ lines 460-461)
        if !self.is_die_applicable(damage_info) {
            return Ok(());
        }

        // Check if AI is already in dead state (C++ lines 463-470)
        if let Some(ai) = obj_write.get_ai_update_interface() {
            if ai.is_ai_in_dead_state() {
                return Ok(()); // Another AI already handled death
            }
            ai.mark_as_dead();
        }

        // Deselect this unit for all players (C++ line 473)
        crate::helpers::TheGameLogic::deselect_object(
            &*obj_write,
            crate::common::PLAYERMASK_ALL,
            true,
        )?;

        // Calculate total probability from all applicable slow death behaviors (C++ lines 475-484)
        let mut total_probability: Int = 0;
        let behavior_modules = obj_write.get_behavior_modules();

        for module in behavior_modules.iter() {
            if let Ok(mut module_guard) = module.lock() {
                if let Some(sdu) = module_guard.get_slow_death_behavior_interface() {
                    if sdu.is_die_applicable(damage_info) {
                        total_probability += sdu.get_probability_modifier(damage_info);
                    }
                }
            }
        }

        if total_probability <= 0 {
            return Err("No valid slow death behaviors found".into());
        }

        // Roll dice to select which behavior executes (C++ lines 488)
        let mut roll = GameLogicRandomValue(1, total_probability);

        // Find the selected behavior (C++ lines 490-503)
        for module in behavior_modules.iter() {
            if let Ok(mut module_guard) = module.lock() {
                if let Some(sdu) = module_guard.get_slow_death_behavior_interface() {
                    if sdu.is_die_applicable(damage_info) {
                        roll -= sdu.get_probability_modifier(damage_info);
                        if roll <= 0 {
                            // This behavior was selected - begin slow death
                            drop(obj_write);
                            sdu.begin_slow_death(damage_info)?;
                            return Ok(());
                        }
                    }
                }
            }
        }

        // Should never reach here (C++ line 504)
        Err("Failed to select slow death behavior".into())
    }
}

impl BehaviorModuleInterface for SlowDeathBehavior {
    fn get_interface_mask() -> u32 {
        MODULEINTERFACE_DIE
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }

    fn get_slow_death_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn ModuleSlowDeathBehaviorInterface> {
        Some(self)
    }
}

impl ModuleSlowDeathBehaviorInterface for SlowDeathBehavior {
    fn is_slow_death_active(&self) -> bool {
        self.is_slow_death_activated()
    }

    /// Begin the slow death sequence
    /// (Matches C++ SlowDeathBehavior::beginSlowDeath at line 191)
    fn begin_slow_death(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check if already activated
        if self.is_slow_death_activated() {
            return Ok(());
        }

        let data = &self.module_data;
        let object = self.get_object()?;
        let mut obj_write = object.write().map_err(|e| format!("Lock error: {}", e))?;

        // Handle infantry sinking - disable shadow decals (C++ lines 198-212)
        if data.sink_rate > 0.0 && obj_write.is_kind_of(KindOf::Infantry) {
            if let Some(drawable) = obj_write.get_drawable() {
                drawable.set_shadows_enabled(false);
                drawable.set_terrain_decal_fade_target(0.0, -0.2);
            }
        }

        // Get LOD time scale (C++ line 216)
        let time_scale = TheGameLODManager::get_slow_death_scale();
        self.accelerated_time_scale = 1.0;

        // Check for instant death (C++ lines 219-224)
        if time_scale == 0.0 && !data.has_non_lod_effects() {
            let obj_id = obj_write.get_id();
            drop(obj_write);
            crate::helpers::TheGameLogic::remove_object(obj_id);
            return Ok(());
        }

        let now = TheGameLogic::get_frame();

        // Calculate timing - check for hulk lifetime override (C++ lines 228-243)
        if obj_write.is_kind_of(KindOf::Hulk)
            && TheGameLogic::get_hulk_max_lifetime_override() != -1
        {
            // Scripts want hulks gone quickly
            self.sink_frame = now + 1;
            self.midpoint_frame = now + (LOGICFRAMES_PER_SECOND / 2) + 1;
            self.destruction_frame = now + LOGICFRAMES_PER_SECOND + 1;
            self.accelerated_time_scale = 1.0;
        } else {
            // Normal timing calculation with variance
            let sink_delay = (time_scale
                * (data.sink_delay as Int
                    + GameLogicRandomValue(0, data.sink_delay_variance as Int))
                    as Real) as UnsignedInt;
            let destruction_delay = (time_scale
                * (data.destruction_delay as Int
                    + GameLogicRandomValue(0, data.destruction_delay_variance as Int))
                    as Real) as UnsignedInt;

            self.sink_frame = now + sink_delay;
            self.destruction_frame = now + destruction_delay;
            self.midpoint_frame = now
                + (GameLogicRandomValue(
                    (BEGIN_MIDPOINT_RATIO * destruction_delay as Real) as Int,
                    (END_MIDPOINT_RATIO * destruction_delay as Real) as Int,
                ) as UnsignedInt);
            self.accelerated_time_scale = time_scale;
        }

        // Handle fling force (C++ lines 247-301)
        if data.fling_force > 0.0 {
            // Release held objects (e.g., stinger soldiers) (C++ lines 251-261)
            if obj_write.is_disabled_by_type(DisabledType::Held) {
                // Matches C++ lines 255-260: Find SlavedUpdate and release via onSlaverDie
                let mut handled = false;
                if let Some(module) = obj_write.find_update_module("SlavedUpdate") {
                    handled = module
                        .with_module_downcast::<crate::object::update::slaved_update::SlavedUpdateModule, _, _>(|module| {
                            let _ = module.behavior_mut().on_slaver_die(Some(damage_info));
                        })
                        .is_some();
                }

                if !handled {
                    if let Some(slave_module) = obj_write.find_update_behavior("SlavedUpdate") {
                        if let Ok(mut slave_guard) = slave_module.lock() {
                            if let Some(slaved) = slave_guard.get_slaved_update_interface() {
                                let _ = slaved.on_slaver_die(Some(damage_info));
                            }
                        }
                    }
                }
            }

            if let Some(physics) = obj_write.get_physics_mut() {
                // Ensure minimum altitude (C++ lines 266-274)
                const MIN_ALTITUDE: Real = 1.0;
                let altitude = obj_write.get_height_above_terrain();
                if altitude < MIN_ALTITUDE {
                    let mut pos = *obj_write.get_position();
                    pos.z += MIN_ALTITUDE;
                    obj_write.set_position(&pos)?;
                }

                // Calculate and apply random force (C++ lines 276-281)
                let force = Self::calc_random_force(
                    data.fling_force,
                    data.fling_force + data.fling_force_variance,
                    data.fling_pitch,
                    data.fling_pitch + data.fling_pitch_variance,
                );

                physics.set_allow_to_fall(true);
                physics.apply_force(&force);
                physics.set_extra_bounciness(-1.0); // No bouncing
                physics.set_extra_friction(-3.0 * SECONDS_PER_LOGICFRAME_REAL); // Reduce friction
                physics.set_allow_bouncing(true);

                // Set orientation and model state (C++ lines 284-287)
                let orientation = force.y.atan2(force.x);
                physics.set_angles(orientation, 0.0, 0.0);
                obj_write.set_model_condition_flags(MAKE_MODELCONDITION_MASK!(
                    MODELCONDITION_EXPLODED_FLAILING
                ))?;
                self.flags |= 1 << Self::FLUNG_INTO_AIR;
            }
        }

        // Get object ID before dropping the lock
        let obj_id = obj_write.get_id();
        let when_to_wake = self
            .sink_frame
            .min(self.destruction_frame)
            .min(self.midpoint_frame);

        // Drop the write lock before calling external functions
        drop(obj_write);

        // Wake immediately for physics updates (C++ line 289)
        if (self.flags & (1 << Self::FLUNG_INTO_AIR)) != 0 {
            crate::helpers::TheGameLogic::set_wake_frame(obj_id, crate::modules::UPDATE_SLEEP_NONE);
        } else {
            // Set wake timing if not flung (C++ lines 294-300)
            // C++ line 300: setWakeFrame(obj, UPDATE_SLEEP(whenToWakeTime))
            let sleep_time =
                crate::modules::UpdateSleepTime::Frames(when_to_wake.saturating_sub(now));
            crate::helpers::TheGameLogic::set_wake_frame(obj_id, sleep_time);
        }

        // Mark as activated
        self.flags |= 1 << Self::SLOW_DEATH_ACTIVATED;

        // Execute initial phase effects (C++ line 308)
        self.do_phase_stuff(SlowDeathPhaseType::Initial)?;

        Ok(())
    }

    /// Calculate probability modifier based on overkill
    /// (Matches C++ SlowDeathBehavior::getProbabilityModifier at line 158)
    fn get_probability_modifier(&self, damage_info: &DamageInfo) -> Int {
        let object = match self.get_object() {
            Ok(obj) => obj,
            Err(_) => return self.module_data.probability_modifier,
        };

        let obj_read = match object.read() {
            Ok(o) => o,
            Err(_) => return self.module_data.probability_modifier,
        };

        // Calculate overkill percentage (C++ lines 163-165)
        let overkill_damage =
            damage_info.output.actual_damage_dealt - damage_info.output.actual_damage_clipped;
        let max_health = if let Some(body_arc) = obj_read.get_body_module() {
            if let Ok(body_guard) = body_arc.lock() {
                body_guard.get_max_health()
            } else {
                1.0
            }
        } else {
            1.0
        };

        let overkill_percent = (overkill_damage as Real) / max_health;
        let overkill_modifier =
            (overkill_percent * self.module_data.modifier_bonus_per_overkill_percent) as Int;

        // Return at least 1 (C++ line 167)
        (self.module_data.probability_modifier + overkill_modifier).max(1)
    }

    fn is_die_applicable(&self, damage_info: &DamageInfo) -> bool {
        // Matches C++ SlowDeathBehavior.h:132 - Check die mux data applicability
        let object = match self.get_object() {
            Ok(obj) => obj,
            Err(_) => return false,
        };

        let obj_read = match object.read() {
            Ok(o) => o,
            Err(_) => return false,
        };

        self.module_data
            .die_mux_data
            .is_die_applicable(&*obj_read, damage_info)
    }

    fn get_slow_death_phase(&self) -> u32 {
        // Determine current phase based on timing
        let now = TheGameLogic::get_frame();

        if now >= self.destruction_frame {
            SlowDeathPhaseType::Final as u32
        } else if now >= self.midpoint_frame {
            SlowDeathPhaseType::Midpoint as u32
        } else {
            SlowDeathPhaseType::Initial as u32
        }
    }
}

// Thread safety
unsafe impl Send for SlowDeathBehavior {}
unsafe impl Sync for SlowDeathBehavior {}

impl Snapshotable for SlowDeathBehavior {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SlowDeathBehavior xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_unsigned_int(&mut self.sink_frame)
            .map_err(|e| format!("SlowDeathBehavior xfer sink_frame: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.midpoint_frame)
            .map_err(|e| format!("SlowDeathBehavior xfer midpoint_frame: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.destruction_frame)
            .map_err(|e| format!("SlowDeathBehavior xfer destruction_frame: {:?}", e))?;
        xfer.xfer_real(&mut self.accelerated_time_scale)
            .map_err(|e| format!("SlowDeathBehavior xfer accelerated_time_scale: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.flags)
            .map_err(|e| format!("SlowDeathBehavior xfer flags: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slow_death_phase_type() {
        assert_eq!(
            SlowDeathPhaseType::from_index(0),
            Some(SlowDeathPhaseType::Initial)
        );
        assert_eq!(
            SlowDeathPhaseType::from_index(1),
            Some(SlowDeathPhaseType::Midpoint)
        );
        assert_eq!(
            SlowDeathPhaseType::from_index(2),
            Some(SlowDeathPhaseType::Final)
        );
        assert_eq!(SlowDeathPhaseType::from_index(3), None);

        assert_eq!(SlowDeathPhaseType::Initial.to_index(), 0);
        assert_eq!(SlowDeathPhaseType::Midpoint.to_index(), 1);
        assert_eq!(SlowDeathPhaseType::Final.to_index(), 2);
    }

    #[test]
    fn test_module_data_creation() {
        let data = SlowDeathBehaviorModuleData::new();
        assert_eq!(data.probability_modifier, 10);
        assert_eq!(data.sink_rate, 0.0);
        assert_eq!(data.destruction_altitude, -10.0);
        assert!(!data.has_non_lod_effects());
    }

    #[test]
    fn test_has_non_lod_effects() {
        let mut data = SlowDeathBehaviorModuleData::new();
        assert!(!data.has_non_lod_effects());

        data.mask_of_loaded_effects = SlowDeathBehaviorModuleData::HAS_OCL;
        assert!(data.has_non_lod_effects());

        data.mask_of_loaded_effects = SlowDeathBehaviorModuleData::HAS_WEAPON;
        assert!(data.has_non_lod_effects());

        data.mask_of_loaded_effects = SlowDeathBehaviorModuleData::HAS_FX;
        assert!(!data.has_non_lod_effects());
    }

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames(&["1500ms"]).expect("duration"), 45);
        assert_eq!(parse_duration_frames(&["1.5s"]).expect("duration"), 45);
    }
}
