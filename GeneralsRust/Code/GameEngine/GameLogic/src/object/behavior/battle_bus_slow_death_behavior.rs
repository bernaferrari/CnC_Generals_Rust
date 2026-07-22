//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/BattleBusSlowDeathBehavior.cpp`.
//!
//! BattleBusSlowDeathBehavior – Rust conversion of the C++ BattleBusSlowDeathBehavior.
//!
//! The classic Battle Bus has a bespoke two-phase slow death: it can fake a death by
//! throwing itself into the air, damaging passengers, and eventually collapsing into
//! the genuine slow-death sequence. The full animation stack depends on the generic
//! SlowDeathBehavior implementation plus FX/OCL assets. Those systems are still under
//! active porting, so this module currently wires the data model, module-factory glue,
//! and high-level state machine while deferring the missing engine hooks until dependent systems land.

use log::warn;
use std::any::Any;
use std::sync::{Arc, Mutex, RwLock};

use crate::common::{
    AsciiString, Bool, Coord3D, DisabledType, Int, ModelConditionFlags, ModuleData, ObjectID, Real,
    UnsignedInt, Xfer, XferVersion,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheFXListStore, TheGameLogic, TheObjectCreationListStore};
use crate::modules::{
    BehaviorModuleInterface, DieModuleInterface, SlowDeathBehaviorInterface, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::behavior::slow_death_behavior::{
    parse_death_types, parse_destruction_altitude, parse_destruction_delay,
    parse_destruction_delay_variance, parse_exempt_status, parse_fling_force,
    parse_fling_force_variance, parse_fling_pitch, parse_fling_pitch_variance, parse_fx,
    parse_modifier_bonus_per_overkill_percent, parse_ocl, parse_probability_modifier,
    parse_required_status, parse_sink_delay, parse_sink_delay_variance, parse_sink_rate,
    parse_veterancy_levels, parse_weapon, SlowDeathBehavior, SlowDeathBehaviorModuleData,
    SlowDeathPhaseType,
};
use crate::object::{
    registry::OBJECT_REGISTRY, Object as GameObject, INVALID_ID as OBJECT_INVALID_ID,
};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as ThingModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};

use crate::effects::{FXList, ObjectCreationList};

// -------------------------------------------------------------------------------------------------
// INI helpers
// -------------------------------------------------------------------------------------------------

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_real(value: &str) -> Result<Real, INIError> {
    INI::parse_real(value)
}

fn parse_percent_to_real(value: &str) -> Result<Real, INIError> {
    INI::parse_percent_to_real(value)
}

fn parse_duration_frames(value: &str) -> Result<UnsignedInt, INIError> {
    INI::parse_duration_unsigned_int(value)
}

fn parse_fx_ref(
    _ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
    assign: impl Fn(&mut BattleBusSlowDeathBehaviorModuleData, Option<Arc<FXList>>),
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if value.eq_ignore_ascii_case("NONE") {
        assign(data, None);
    } else {
        let fx = TheFXListStore::lookup_fx_list(value);
        if fx.is_none() {
            log::warn!("BattleBusSlowDeathBehavior: unresolved FXList '{}'", value);
        }
        assign(data, fx);
    }
    Ok(())
}

fn parse_ocl_ref(
    _ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
    assign: impl Fn(&mut BattleBusSlowDeathBehaviorModuleData, Option<Arc<ObjectCreationList>>),
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if value.eq_ignore_ascii_case("NONE") {
        assign(data, None);
    } else {
        let ocl = TheObjectCreationListStore::find_object_creation_list(value);
        if ocl.is_none() {
            log::warn!("BattleBusSlowDeathBehavior: unresolved OCL '{}'", value);
        }
        assign(data, ocl);
    }
    Ok(())
}

fn parse_fx_start_undeath_field(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_ref(ini, data, tokens, |d, fx| d.fx_start_undeath = fx)
}

fn parse_ocl_start_undeath_field(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_ref(ini, data, tokens, |d, ocl| d.ocl_start_undeath = ocl)
}

fn parse_fx_hit_ground_field(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_fx_ref(ini, data, tokens, |d, fx| d.fx_hit_ground = fx)
}

fn parse_ocl_hit_ground_field(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_ocl_ref(ini, data, tokens, |d, ocl| d.ocl_hit_ground = ocl)
}

fn parse_throw_force_field(
    _ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.throw_force = parse_real(value)?;
    Ok(())
}

fn parse_percent_damage_field(
    _ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.percent_damage_to_passengers = parse_percent_to_real(value)?;
    Ok(())
}

fn parse_empty_hulk_delay_field(
    _ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.empty_hulk_destruction_delay = parse_duration_frames(value)?;
    Ok(())
}

fn with_base_mut<F>(data: &mut BattleBusSlowDeathBehaviorModuleData, f: F) -> Result<(), INIError>
where
    F: FnOnce(&mut SlowDeathBehaviorModuleData) -> Result<(), INIError>,
{
    let base = Arc::get_mut(&mut data.base).ok_or(INIError::InvalidData)?;
    f(base)
}

fn parse_base_sink_rate(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_sink_rate(ini, base, tokens))
}

fn parse_base_probability_modifier(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_probability_modifier(ini, base, tokens))
}

fn parse_base_modifier_bonus_per_overkill_percent(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| {
        parse_modifier_bonus_per_overkill_percent(ini, base, tokens)
    })
}

fn parse_base_sink_delay(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_sink_delay(ini, base, tokens))
}

fn parse_base_sink_delay_variance(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_sink_delay_variance(ini, base, tokens))
}

fn parse_base_destruction_delay(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_destruction_delay(ini, base, tokens))
}

fn parse_base_destruction_delay_variance(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| {
        parse_destruction_delay_variance(ini, base, tokens)
    })
}

fn parse_base_destruction_altitude(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_destruction_altitude(ini, base, tokens))
}

fn parse_base_fx(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_fx(ini, base, tokens))
}

fn parse_base_ocl(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_ocl(ini, base, tokens))
}

fn parse_base_weapon(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_weapon(ini, base, tokens))
}

fn parse_base_fling_force(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_fling_force(ini, base, tokens))
}

fn parse_base_fling_force_variance(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_fling_force_variance(ini, base, tokens))
}

fn parse_base_fling_pitch(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_fling_pitch(ini, base, tokens))
}

fn parse_base_fling_pitch_variance(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_fling_pitch_variance(ini, base, tokens))
}

fn parse_base_death_types(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_death_types(ini, base, tokens))
}

fn parse_base_veterancy_levels(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_veterancy_levels(ini, base, tokens))
}

fn parse_base_exempt_status(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_exempt_status(ini, base, tokens))
}

fn parse_base_required_status(
    ini: &mut INI,
    data: &mut BattleBusSlowDeathBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    with_base_mut(data, |base| parse_required_status(ini, base, tokens))
}

const BATTLE_BUS_SLOW_DEATH_FIELDS: &[FieldParse<BattleBusSlowDeathBehaviorModuleData>] = &[
    FieldParse {
        token: "SinkRate",
        parse: parse_base_sink_rate,
    },
    FieldParse {
        token: "ProbabilityModifier",
        parse: parse_base_probability_modifier,
    },
    FieldParse {
        token: "ModifierBonusPerOverkillPercent",
        parse: parse_base_modifier_bonus_per_overkill_percent,
    },
    FieldParse {
        token: "SinkDelay",
        parse: parse_base_sink_delay,
    },
    FieldParse {
        token: "SinkDelayVariance",
        parse: parse_base_sink_delay_variance,
    },
    FieldParse {
        token: "DestructionDelay",
        parse: parse_base_destruction_delay,
    },
    FieldParse {
        token: "DestructionDelayVariance",
        parse: parse_base_destruction_delay_variance,
    },
    FieldParse {
        token: "DestructionAltitude",
        parse: parse_base_destruction_altitude,
    },
    FieldParse {
        token: "FX",
        parse: parse_base_fx,
    },
    FieldParse {
        token: "OCL",
        parse: parse_base_ocl,
    },
    FieldParse {
        token: "Weapon",
        parse: parse_base_weapon,
    },
    FieldParse {
        token: "FlingForce",
        parse: parse_base_fling_force,
    },
    FieldParse {
        token: "FlingForceVariance",
        parse: parse_base_fling_force_variance,
    },
    FieldParse {
        token: "FlingPitch",
        parse: parse_base_fling_pitch,
    },
    FieldParse {
        token: "FlingPitchVariance",
        parse: parse_base_fling_pitch_variance,
    },
    FieldParse {
        token: "DeathTypes",
        parse: parse_base_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_base_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_base_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_base_required_status,
    },
    FieldParse {
        token: "FXStartUndeath",
        parse: parse_fx_start_undeath_field,
    },
    FieldParse {
        token: "OCLStartUndeath",
        parse: parse_ocl_start_undeath_field,
    },
    FieldParse {
        token: "FXHitGround",
        parse: parse_fx_hit_ground_field,
    },
    FieldParse {
        token: "OCLHitGround",
        parse: parse_ocl_hit_ground_field,
    },
    FieldParse {
        token: "ThrowForce",
        parse: parse_throw_force_field,
    },
    FieldParse {
        token: "PercentDamageToPassengers",
        parse: parse_percent_damage_field,
    },
    FieldParse {
        token: "EmptyHulkDestructionDelay",
        parse: parse_empty_hulk_delay_field,
    },
];

// -------------------------------------------------------------------------------------------------
// Module data
// -------------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BattleBusSlowDeathBehaviorModuleData {
    module_tag_name_key: NameKeyType,
    pub base: Arc<SlowDeathBehaviorModuleData>,
    pub fx_start_undeath: Option<Arc<FXList>>,
    pub ocl_start_undeath: Option<Arc<ObjectCreationList>>,
    pub fx_hit_ground: Option<Arc<FXList>>,
    pub ocl_hit_ground: Option<Arc<ObjectCreationList>>,
    pub throw_force: Real,
    pub percent_damage_to_passengers: Real,
    pub empty_hulk_destruction_delay: UnsignedInt,
}

impl BattleBusSlowDeathBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            base: Arc::new(SlowDeathBehaviorModuleData::new()),
            fx_start_undeath: None,
            ocl_start_undeath: None,
            fx_hit_ground: None,
            ocl_hit_ground: None,
            throw_force: 1.0,
            percent_damage_to_passengers: 0.0,
            empty_hulk_destruction_delay: 0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, BATTLE_BUS_SLOW_DEATH_FIELDS)
    }
}

impl Default for BattleBusSlowDeathBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ThingModuleData for BattleBusSlowDeathBehaviorModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for BattleBusSlowDeathBehaviorModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u8 = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
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

// -------------------------------------------------------------------------------------------------
// Behavior implementation
// -------------------------------------------------------------------------------------------------

const GROUND_CHECK_DELAY: UnsignedInt = 10;
const EMPTY_HULK_CHECK_DELAY: UnsignedInt = 15;

pub struct BattleBusSlowDeathBehavior {
    module_data: Arc<BattleBusSlowDeathBehaviorModuleData>,
    object_id: ObjectID,
    base_behavior: Option<SlowDeathBehavior>,
    last_damage_info: Option<DamageInfo>,
    is_real_death: Bool,
    is_in_first_death: Bool,
    ground_check_frame: UnsignedInt,
    penalty_death_frame: UnsignedInt,
}

impl BattleBusSlowDeathBehavior {
    fn construct_with_object(
        object_id: ObjectID,
        module_data: Arc<BattleBusSlowDeathBehaviorModuleData>,
        object: Option<Arc<RwLock<GameObject>>>,
    ) -> Self {
        Self {
            module_data,
            object_id,
            base_behavior: None,
            last_damage_info: None,
            is_real_death: false,
            is_in_first_death: false,
            ground_check_frame: 0,
            penalty_death_frame: 0,
        }
    }

    pub fn new_from_object(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<BattleBusSlowDeathBehaviorModuleData>,
    ) -> Self {
        let object_id = object
            .read()
            .map(|obj| obj.get_id())
            .unwrap_or(OBJECT_INVALID_ID);
        Self::construct_with_object(object_id, module_data, Some(object))
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<BattleBusSlowDeathBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "BattleBusSlowDeathBehavior requires an owning object".to_string())?;

        let object_id = module_object.get_object_id();
        Ok(Self::construct_with_object(object_id, module_data, None))
    }

    fn get_object(
        &self,
    ) -> Result<Arc<RwLock<GameObject>>, Box<dyn std::error::Error + Send + Sync>> {
        if self.object_id == OBJECT_INVALID_ID {
            return Err("BattleBusSlowDeathBehavior missing owning object id".into());
        }
        OBJECT_REGISTRY.get_object(self.object_id).ok_or_else(|| {
            format!(
                "BattleBusSlowDeathBehavior object {} not registered",
                self.object_id
            )
            .into()
        })
    }

    fn get_current_frame(&self) -> UnsignedInt {
        TheGameLogic::get_frame()
    }

    fn ensure_base_behavior(
        &mut self,
    ) -> Result<&mut SlowDeathBehavior, Box<dyn std::error::Error + Send + Sync>> {
        if self.base_behavior.is_none() {
            let object = self.get_object()?;
            let base_data: Arc<dyn ModuleData> = self.module_data.base.clone();
            let mut base = SlowDeathBehavior::new(object.clone(), base_data)?;
            base.set_object(object);
            self.base_behavior = Some(base);
        }

        Ok(self.base_behavior.as_mut().expect("base behavior set"))
    }

    fn begin_base_slow_death(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let base = self.ensure_base_behavior()?;
        base.begin_slow_death(damage_info)
    }

    #[allow(dead_code)]
    fn begin_base_with_last_damage(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let damage_info = self
            .last_damage_info
            .clone()
            .unwrap_or_else(DamageInfo::default);
        self.begin_base_slow_death(&damage_info)
    }

    fn execute_fx_at_object(
        &self,
        fx: &Arc<FXList>,
        obj: &Arc<RwLock<GameObject>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let obj_guard = obj.read().map_err(|_| {
            std::io::Error::other("BattleBusSlowDeathBehavior object read lock poisoned")
        })?;
        let position = *obj_guard.get_position();
        drop(obj_guard);
        fx.do_fx_at_position(&position)?;
        Ok(())
    }

    fn execute_ocl_at_object(
        &self,
        ocl: &Arc<ObjectCreationList>,
        obj: &Arc<RwLock<GameObject>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let obj_guard = obj.read().map_err(|_| {
            std::io::Error::other("BattleBusSlowDeathBehavior object read lock poisoned")
        })?;
        let position = *obj_guard.get_position();
        drop(obj_guard);
        ocl.create_at_position(&position, self.object_id)?;
        Ok(())
    }

    fn damage_passengers(
        &self,
        obj: &Arc<RwLock<GameObject>>,
        damage_percent: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let obj_guard = obj.read().map_err(|_| {
            std::io::Error::other("BattleBusSlowDeathBehavior object read lock poisoned")
        })?;
        if let Some(contain) = obj_guard.get_contain() {
            let contain_guard = contain.lock().map_err(|_| {
                std::io::Error::other("BattleBusSlowDeathBehavior contain lock poisoned")
            })?;
            let passengers = contain_guard.get_contained_objects().to_vec();
            drop(contain_guard);

            for passenger_id in passengers {
                let Some(passenger) = TheGameLogic::find_object_by_id(passenger_id) else {
                    continue;
                };
                let mut passenger_guard = passenger.write().map_err(|_| {
                    std::io::Error::other(
                        "BattleBusSlowDeathBehavior passenger write lock poisoned",
                    )
                })?;
                if let Some(body) = passenger_guard.get_body_module() {
                    let max_health = body
                        .lock()
                        .map_err(|_| {
                            std::io::Error::other("BattleBusSlowDeathBehavior body lock poisoned")
                        })?
                        .get_max_health();
                    let damage_amount = max_health * damage_percent;
                    let mut damage_info = DamageInfo::new();
                    damage_info.input.amount = damage_amount;
                    damage_info.input.damage_type = DamageType::Unresistable;
                    damage_info.input.death_type = DeathType::Normal;
                    damage_info.sync_from_input();
                    passenger_guard.attempt_damage(&mut damage_info)?;
                }
            }
        }
        Ok(())
    }

    fn has_hit_ground(
        &self,
        obj: &Arc<RwLock<GameObject>>,
    ) -> Result<Bool, Box<dyn std::error::Error + Send + Sync>> {
        let obj_guard = obj.read().map_err(|_| {
            std::io::Error::other("BattleBusSlowDeathBehavior object read lock poisoned")
        })?;
        Ok(!obj_guard.is_above_terrain())
    }

    fn finish_first_death(
        &mut self,
        obj: &Arc<RwLock<GameObject>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(fx) = &self.module_data.fx_hit_ground {
            self.execute_fx_at_object(fx, obj)?;
        }
        if let Some(ocl) = &self.module_data.ocl_hit_ground {
            self.execute_ocl_at_object(ocl, obj)?;
        }

        self.is_in_first_death = false;

        {
            let mut obj_guard = obj.write().map_err(|_| {
                std::io::Error::other("BattleBusSlowDeathBehavior object write lock poisoned")
            })?;
            obj_guard.set_model_condition_state(ModelConditionFlags::SECOND_LIFE);
            obj_guard.set_disabled(DisabledType::Held);
        }

        {
            let obj_guard = obj.read().map_err(|_| {
                std::io::Error::other("BattleBusSlowDeathBehavior object read lock poisoned")
            })?;
            if let Some(ai) = obj_guard.get_ai() {
                let mut ai_guard = ai.lock().map_err(|_| {
                    std::io::Error::other("BattleBusSlowDeathBehavior AI lock poisoned")
                })?;
                ai_guard.ai_idle()?;
            }
            if let Some(physics) = obj_guard.get_physics() {
                let mut physics_guard = physics.lock().map_err(|_| {
                    std::io::Error::other("BattleBusSlowDeathBehavior physics lock poisoned")
                })?;
                physics_guard.clear_acceleration();
                physics_guard.scrub_velocity_2d(0.0);
            }
        }

        Ok(())
    }
}

impl SlowDeathBehaviorInterface for BattleBusSlowDeathBehavior {
    fn is_slow_death_active(&self) -> bool {
        if self.is_in_first_death {
            return true;
        }

        self.base_behavior
            .as_ref()
            .map(|base| base.is_slow_death_active())
            .unwrap_or(false)
    }

    fn begin_slow_death(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let object = self.get_object()?;
        let data = &self.module_data;
        self.last_damage_info = Some(damage_info.clone());

        if !self.is_real_death {
            // C++ lines 122-157: We can intercept and do our extra slow death
            self.is_in_first_death = true;
            self.ground_check_frame = self.get_current_frame() + GROUND_CHECK_DELAY;
            self.penalty_death_frame = 0;

            // C++ lines 128-130: First do the special effects
            if let Some(fx) = &data.fx_start_undeath {
                self.execute_fx_at_object(fx, &object)?;
            }
            if let Some(ocl) = &data.ocl_start_undeath {
                self.execute_ocl_at_object(ocl, &object)?;
            }

            // C++ lines 132-136: Stop what we were doing (AI idle)
            {
                let obj_guard = object.read().map_err(|_| {
                    std::io::Error::other("BattleBusSlowDeathBehavior object read lock poisoned")
                })?;
                if let Some(ai) = obj_guard.get_ai() {
                    let mut ai_guard = ai.lock().map_err(|_| {
                        std::io::Error::other("BattleBusSlowDeathBehavior AI lock poisoned")
                    })?;
                    ai_guard.ai_idle()?;
                }
            }

            // C++ lines 138-151: Physics - stop and throw into air
            {
                let obj_guard = object.read().map_err(|_| {
                    std::io::Error::other("BattleBusSlowDeathBehavior object read lock poisoned")
                })?;
                if let Some(physics) = obj_guard.get_physics() {
                    let mut physics_guard = physics.lock().map_err(|_| {
                        std::io::Error::other("BattleBusSlowDeathBehavior physics lock poisoned")
                    })?;
                    // C++ line 141: clearAcceleration
                    physics_guard.clear_acceleration();
                    // C++ line 142: scrubVelocity2D(0)
                    physics_guard.scrub_velocity_2d(0.0);
                    // C++ lines 145-149: Apply throw force
                    let throw_velocity = Coord3D::new(0.0, 0.0, data.throw_force);
                    physics_guard.apply_shock(&throw_velocity);
                    // C++ line 150: applyRandomRotation
                    physics_guard.apply_random_rotation();
                }
            }

            // C++ lines 153-155: Hit those inside for some damage
            if data.percent_damage_to_passengers > 0.0 {
                self.damage_passengers(&object, data.percent_damage_to_passengers)?;
            }

            TheGameLogic::set_wake_frame(self.object_id, UpdateSleepTime::None);
        } else {
            // C++ lines 159-163: If a real death, delegate to base SlowDeathBehavior
            self.is_in_first_death = false;
            self.begin_base_slow_death(damage_info)?;
        }

        Ok(())
    }

    fn get_probability_modifier(&self, damage_info: &DamageInfo) -> Int {
        let object = match self.get_object() {
            Ok(obj) => obj,
            Err(_) => return self.module_data.base.probability_modifier,
        };

        let obj_read = match object.read() {
            Ok(o) => o,
            Err(_) => return self.module_data.base.probability_modifier,
        };

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

        let overkill_percent = if max_health > 0.0 {
            overkill_damage / max_health
        } else {
            0.0
        };
        let overkill_modifier =
            (overkill_percent * self.module_data.base.modifier_bonus_per_overkill_percent) as Int;

        (self.module_data.base.probability_modifier + overkill_modifier).max(1)
    }

    fn is_die_applicable(&self, damage_info: &DamageInfo) -> bool {
        let object = match self.get_object() {
            Ok(obj) => obj,
            Err(_) => return false,
        };

        let obj_read = match object.read() {
            Ok(o) => o,
            Err(_) => return false,
        };

        self.module_data
            .base
            .die_mux_data
            .is_die_applicable(&*obj_read, damage_info)
    }

    fn get_slow_death_phase(&self) -> u32 {
        if self.is_in_first_death {
            return SlowDeathPhaseType::Initial as u32;
        }

        if self.is_real_death {
            if let Some(base) = self.base_behavior.as_ref() {
                return base.get_slow_death_phase();
            }
        }

        SlowDeathPhaseType::Final as u32
    }
}

impl BattleBusSlowDeathBehavior {
    fn current_contain_count(
        &self,
        obj: &Arc<RwLock<GameObject>>,
    ) -> Result<Option<usize>, Box<dyn std::error::Error + Send + Sync>> {
        let obj_guard = obj.read().map_err(|_| {
            std::io::Error::other("BattleBusSlowDeathBehavior object read lock poisoned")
        })?;
        let Some(contain) = obj_guard.get_contain() else {
            return Ok(None);
        };
        let contain_guard = contain.lock().map_err(|_| {
            std::io::Error::other("BattleBusSlowDeathBehavior contain lock poisoned")
        })?;
        Ok(Some(contain_guard.get_contained_count()))
    }
}

// -------------------------------------------------------------------------------------------------
// Update / module plumbing
// -------------------------------------------------------------------------------------------------

impl UpdateModuleInterface for BattleBusSlowDeathBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let object = self.get_object()?;
        let empty_hulk_destruction_delay = self.module_data.empty_hulk_destruction_delay;
        let now = self.get_current_frame();

        if self.is_in_first_death {
            if now > self.ground_check_frame && self.has_hit_ground(&object)? {
                self.finish_first_death(&object)?;
                if empty_hulk_destruction_delay == 0 {
                    return Ok(crate::modules::UPDATE_SLEEP_FOREVER);
                }
                return Ok(UpdateSleepTime::None);
            }
            return Ok(UpdateSleepTime::None);
        }

        if self.is_real_death {
            let base = self.ensure_base_behavior()?;
            return UpdateModuleInterface::update(base);
        }

        let Some(contain_count) = self.current_contain_count(&object)? else {
            return Ok(crate::modules::UPDATE_SLEEP_FOREVER);
        };

        if self.penalty_death_frame != 0 {
            if contain_count > 0 {
                self.penalty_death_frame = 0;
                return Ok(UpdateSleepTime::Frames(EMPTY_HULK_CHECK_DELAY));
            }
            if now > self.penalty_death_frame {
                let mut obj_guard = object.write().map_err(|_| {
                    std::io::Error::other("BattleBusSlowDeathBehavior object write lock poisoned")
                })?;
                obj_guard.kill(Some(DamageType::Penalty), Some(DeathType::Extra4));
                return Ok(crate::modules::UPDATE_SLEEP_FOREVER);
            }
            return Ok(UpdateSleepTime::Frames(EMPTY_HULK_CHECK_DELAY));
        }

        if contain_count == 0 {
            self.penalty_death_frame = now + empty_hulk_destruction_delay;
        }

        Ok(UpdateSleepTime::Frames(EMPTY_HULK_CHECK_DELAY))
    }
}

impl BehaviorModuleInterface for BattleBusSlowDeathBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_slow_death_behavior_interface(&mut self) -> Option<&mut dyn SlowDeathBehaviorInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for BattleBusSlowDeathBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        // Base SlowDeathBehavior CRC is not available via &self, but we still feed own state
        let mut is_real_death = self.is_real_death;
        xfer.xfer_bool(&mut is_real_death)
            .map_err(|e| e.to_string())?;
        let mut is_in_first_death = self.is_in_first_death;
        xfer.xfer_bool(&mut is_in_first_death)
            .map_err(|e| e.to_string())?;
        let mut ground_check_frame = self.ground_check_frame;
        xfer.xfer_unsigned_int(&mut ground_check_frame)
            .map_err(|e| e.to_string())?;
        let mut penalty_death_frame = self.penalty_death_frame;
        xfer.xfer_unsigned_int(&mut penalty_death_frame)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.ensure_base_behavior()
            .map_err(|e| e.to_string())?
            .xfer(xfer)?;

        xfer.xfer_bool(&mut self.is_real_death)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_in_first_death)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.ground_check_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.penalty_death_frame)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl DieModuleInterface for BattleBusSlowDeathBehavior {
    fn on_die(
        &mut self,
        damage_info: &DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.is_real_death = true;
        self.is_in_first_death = false;
        let base = self.ensure_base_behavior()?;
        DieModuleInterface::on_die(base, damage_info)
    }
}

// -------------------------------------------------------------------------------------------------
// Module wrapper
// -------------------------------------------------------------------------------------------------

pub struct BattleBusSlowDeathBehaviorModule {
    behavior: BattleBusSlowDeathBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<BattleBusSlowDeathBehaviorModuleData>,
}

impl BattleBusSlowDeathBehaviorModule {
    pub fn new(
        behavior: BattleBusSlowDeathBehavior,
        module_name: &AsciiString,
        module_data: Arc<BattleBusSlowDeathBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut BattleBusSlowDeathBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for BattleBusSlowDeathBehaviorModule {
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

impl EngineModule for BattleBusSlowDeathBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ThingModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {}

    fn on_delete(&mut self) {}
}

// -------------------------------------------------------------------------------------------------
// Factory helpers
// -------------------------------------------------------------------------------------------------

pub fn battle_bus_slow_death_data_factory(ini: Option<&mut INI>) -> Box<dyn ThingModuleData> {
    let mut data = BattleBusSlowDeathBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse BattleBusSlowDeathBehavior data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

pub fn battle_bus_slow_death_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ThingModuleData>,
) -> Box<dyn EngineModule> {
    let typed = module_data
        .as_any()
        .downcast_ref::<BattleBusSlowDeathBehaviorModuleData>()
        .expect("BattleBusSlowDeathBehaviorModuleData expected");

    let shared_data = Arc::new(typed.clone());
    let behavior = BattleBusSlowDeathBehavior::from_module_thing(thing, Arc::clone(&shared_data))
        .expect("BattleBusSlowDeathBehavior requires an owning object");

    let module_name = AsciiString::from("BattleBusSlowDeathBehavior");
    Box::new(BattleBusSlowDeathBehaviorModule::new(
        behavior,
        &module_name,
        shared_data,
    ))
}

// -------------------------------------------------------------------------------------------------
// Basic smoke test (module override wiring)
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::thing::module::ModuleData as ThingModuleDataTrait;

    #[derive(Debug, Clone)]
    struct StubThing;

    impl ModuleObject for StubThing {
        fn get_object_id(&self) -> ObjectID {
            0
        }

        fn upgrade_handle(&self) -> Option<Arc<RwLock<dyn ModuleObject>>> {
            None
        }
    }

    impl ModuleThing for StubThing {
        fn as_object(&self) -> Option<&dyn ModuleObject> {
            Some(self)
        }

        fn as_drawable(&self) -> Option<&dyn game_engine::common::thing::module::Drawable> {
            None
        }
    }

    #[test]
    fn data_factory_produces_defaults() {
        let data = battle_bus_slow_death_data_factory(None);
        let typed = data
            .as_ref()
            .downcast_ref::<BattleBusSlowDeathBehaviorModuleData>()
            .expect("battle bus slow death data");
        assert_eq!(typed.throw_force, 1.0);
        assert!(typed.fx_start_undeath.is_none());
    }

    #[test]
    fn module_factory_downcasts_data() {
        let data =
            Arc::new(BattleBusSlowDeathBehaviorModuleData::default()) as Arc<dyn ThingModuleData>;
        let thing: Arc<dyn ModuleThing> = Arc::new(StubThing);
        let module = battle_bus_slow_death_module_factory(thing, data);
        assert!(module
            .get_module_data()
            .as_any()
            .downcast_ref::<BattleBusSlowDeathBehaviorModuleData>()
            .is_some());
    }

    #[test]
    fn parse_fx_ref_keeps_missing_reference_none() {
        let mut ini = INI::new();
        let mut data = BattleBusSlowDeathBehaviorModuleData::default();
        parse_fx_ref(
            &mut ini,
            &mut data,
            &["MissingBattleBusFx_ParityTest_20260302"],
            |d, fx| d.fx_hit_ground = fx,
        )
        .expect("parse should succeed");
        assert!(data.fx_hit_ground.is_none());
    }

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames("1500ms").expect("duration"), 45);
        assert_eq!(parse_duration_frames("1.5s").expect("duration"), 45);
    }
}
