//! ChinookAIUpdate module data + runtime logic.
//!
//! Ported from GameLogic/Module/ChinookAIUpdate.h and
//! GameLogic/Object/Update/AIUpdate/ChinookAIUpdate.cpp.

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock};

use crate::action_manager::{ActionManager, CanEnterType};
use crate::ai::states::AICommandParmsStorage;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{
    AsciiString, Bool, Coord3D, DrawableID, Int, KindOf, Matrix3D, ObjectID, Real, UnsignedInt,
    INVALID_ID, LOGICFRAMES_PER_SECOND,
};
use crate::helpers::{
    get_game_logic_random_value, get_game_logic_random_value_real, TheGameClient, TheGameLogic,
    TheParticleSystemManager, ThePartitionManager, TheTerrainLogic, TheThingFactory,
};
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, ContainModuleInterfaceExt, SupplyTruckAIInterface,
};
use crate::object::draw::draw_module::RGBColor;
use crate::object::drawable::{Drawable, DrawableArcExt};
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use crate::object::Object;
use crate::player::player_list;
use crate::supply_system::{SupplyTruckAIUpdate, SupplyTruckAIUpdateData, SupplyTruckState};
use crate::upgrade::center::get_upgrade_center;
use game_engine::common::global_data;
use game_engine::common::ini::{FieldParse, INIError, INILoadType, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
    "YES",
    "STEALTHED",
    "NO",
    "NOTWHILEATTACKING",
    "ATTACK_BUILDINGS",
];
const INVALID_DRAWABLE_ID: DrawableID = 0;

/// Chinook flight status (matches C++ ChinookFlightStatus).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChinookFlightStatus {
    TakingOff = 0,
    Flying = 1,
    DoingCombatDrop = 2,
    Landing = 3,
    Landed = 4,
}

/// Module data for ChinookAIUpdate (INI-driven).
#[derive(Debug, Clone)]
pub struct ChinookAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub max_boxes_data: Int,
    pub center_delay: UnsignedInt,
    pub warehouse_delay: UnsignedInt,
    pub warehouse_scan_distance: Real,
    pub supplies_depleted_voice: AsciiString,
    pub rope_name: AsciiString,
    pub rotor_wash_particle_system: AsciiString,
    pub rappel_speed: Real,
    pub rope_drop_speed: Real,
    pub rope_width: Real,
    pub rope_final_height: Real,
    pub rope_wobble_len: Real,
    pub rope_wobble_amp: Real,
    pub rope_wobble_rate: Real,
    pub rope_color: RGBColor,
    pub num_ropes: UnsignedInt,
    pub per_rope_delay_min: UnsignedInt,
    pub per_rope_delay_max: UnsignedInt,
    pub min_drop_height: Real,
    pub wait_for_ropes_to_drop: Bool,
    pub upgraded_supply_boost: Int,
}

impl Default for ChinookAIUpdateModuleData {
    fn default() -> Self {
        let gravity = global_data::read_safe()
            .map(|data| data.gravity.abs())
            .unwrap_or(9.81);
        let rappel_speed = gravity * LOGICFRAMES_PER_SECOND as f32 * 0.5;
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            max_boxes_data: 0,
            center_delay: 0,
            warehouse_delay: 0,
            warehouse_scan_distance: 100.0,
            supplies_depleted_voice: AsciiString::new(),
            rope_name: AsciiString::from("GenericRope"),
            rotor_wash_particle_system: AsciiString::new(),
            rappel_speed,
            rope_drop_speed: 1.0e10,
            rope_width: 0.5,
            rope_final_height: 0.0,
            rope_wobble_len: 10.0,
            rope_wobble_amp: 1.0,
            rope_wobble_rate: 0.1,
            rope_color: RGBColor::new(229, 204, 178),
            num_ropes: 4,
            per_rope_delay_min: 0x7fffffff,
            per_rope_delay_max: 0x7fffffff,
            min_drop_height: 30.0,
            wait_for_ropes_to_drop: true,
            upgraded_supply_boost: 0,
        }
    }
}

impl ChinookAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CHINOOK_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for ChinookAIUpdateModuleData {
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

impl Snapshotable for ChinookAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_int(&mut self.max_boxes_data))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.center_delay))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.warehouse_delay))?;
        xfer_io(xfer.xfer_real(&mut self.warehouse_scan_distance))?;
        xfer_io(xfer.xfer_ascii_string(self.supplies_depleted_voice.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_ascii_string(self.rope_name.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_ascii_string(self.rotor_wash_particle_system.as_mut_string_buffer()))?;
        xfer_io(xfer.xfer_real(&mut self.rappel_speed))?;
        xfer_io(xfer.xfer_real(&mut self.rope_drop_speed))?;
        xfer_io(xfer.xfer_real(&mut self.rope_width))?;
        xfer_io(xfer.xfer_real(&mut self.rope_final_height))?;
        xfer_io(xfer.xfer_real(&mut self.rope_wobble_len))?;
        xfer_io(xfer.xfer_real(&mut self.rope_wobble_amp))?;
        xfer_io(xfer.xfer_real(&mut self.rope_wobble_rate))?;
        xfer_io(xfer.xfer_unsigned_byte(&mut self.rope_color.r))?;
        xfer_io(xfer.xfer_unsigned_byte(&mut self.rope_color.g))?;
        xfer_io(xfer.xfer_unsigned_byte(&mut self.rope_color.b))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.num_ropes))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.per_rope_delay_min))?;
        xfer_io(xfer.xfer_unsigned_int(&mut self.per_rope_delay_max))?;
        xfer_io(xfer.xfer_real(&mut self.min_drop_height))?;
        xfer_io(xfer.xfer_bool(&mut self.wait_for_ropes_to_drop))?;
        xfer_io(xfer.xfer_int(&mut self.upgraded_supply_boost))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut ChinookAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    let value = INI::parse_bit_string_32(&values, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    data.base.set_auto_acquire_enemies_when_idle(value);
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> Result<Vec<&'a str>, INIError> {
    let values: Vec<_> = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();
    if values.is_empty() {
        return Err(INIError::InvalidData);
    }
    Ok(values)
}

fn parse_duration_unsigned_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_unsigned_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_unsigned_int(token)?);
    Ok(())
}

#[allow(dead_code)]
fn parse_duration_real_field(
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_real(token)?);
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(Bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(Real), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_velocity_real_field(
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_angular_velocity_real_field(
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_angular_velocity_real(token)?);
    Ok(())
}

fn parse_int_field(setter: &mut dyn FnMut(Int), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_int(token)?);
    Ok(())
}

fn parse_ascii_string_field(
    setter: &mut dyn FnMut(AsciiString),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(AsciiString::from(token));
    Ok(())
}

fn parse_rgb_color_field(
    setter: &mut dyn FnMut(RGBColor),
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    let (r, g, b) = INI::parse_rgb_color(&values)?;
    let to_u8 = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    setter(RGBColor::new(to_u8(r), to_u8(g), to_u8(b)));
    Ok(())
}

fn parse_locomotor_set_field(
    ini: &mut INI,
    data: &mut ChinookAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens)?;
    if values.len() < 2 {
        return Err(INIError::InvalidData);
    }

    let set = match values[0] {
        "SET_NORMAL" => crate::common::LocomotorSetType::Normal,
        "SET_NORMAL_UPGRADED" => crate::common::LocomotorSetType::NormalUpgraded,
        "SET_FREEFALL" => crate::common::LocomotorSetType::Freefall,
        "SET_WANDER" => crate::common::LocomotorSetType::Wander,
        "SET_PANIC" => crate::common::LocomotorSetType::Panic,
        "SET_TAXIING" => crate::common::LocomotorSetType::Taxiing,
        "SET_SUPERSONIC" => crate::common::LocomotorSetType::Supersonic,
        "SET_SLUGGISH" => crate::common::LocomotorSetType::Sluggish,
        _ => return Err(INIError::InvalidData),
    };

    if data.base.has_locomotor_set(set) && ini.get_load_type() != INILoadType::CreateOverrides {
        return Err(INIError::InvalidData);
    }

    let mut entries = Vec::new();
    for token in values.iter().skip(1) {
        if token.is_empty() || token.eq_ignore_ascii_case("None") {
            continue;
        }
        entries.push(AsciiString::from(*token));
    }
    if entries.is_empty() {
        return Err(INIError::InvalidData);
    }
    data.base.set_locomotor_set_entries(set, entries);
    Ok(())
}

fn parse_turret_field(
    ini: &mut INI,
    data: &mut ChinookAIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    if data.base.turret_primary().is_some() {
        return Err(INIError::InvalidData);
    }
    let mut turret = crate::object::update::ai_update_interface::TurretAIData::default();
    turret.parse_from_ini(ini)?;
    data.base.set_turret_primary(turret);
    Ok(())
}

fn parse_alt_turret_field(
    ini: &mut INI,
    data: &mut ChinookAIUpdateModuleData,
    _tokens: &[&str],
) -> Result<(), INIError> {
    if data.base.turret_secondary().is_some() {
        return Err(INIError::InvalidData);
    }
    let mut turret = crate::object::update::ai_update_interface::TurretAIData::default();
    turret.parse_from_ini(ini)?;
    data.base.set_turret_secondary(turret);
    Ok(())
}

const CHINOOK_AI_UPDATE_FIELDS: &[FieldParse<ChinookAIUpdateModuleData>] = &[
    FieldParse {
        token: "Turret",
        parse: parse_turret_field,
    },
    FieldParse {
        token: "AltTurret",
        parse: parse_alt_turret_field,
    },
    FieldParse {
        token: "AutoAcquireEnemiesWhenIdle",
        parse: parse_auto_acquire_field,
    },
    FieldParse {
        token: "Locomotor",
        parse: parse_locomotor_set_field,
    },
    FieldParse {
        token: "MoodAttackCheckRate",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(
                &mut |value| data.base.set_mood_attack_check_rate(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "SurrenderDuration",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(
                &mut |value| data.base.set_surrender_duration_frames(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "ForbidPlayerCommands",
        parse: |_, data, tokens| {
            parse_bool_field(
                &mut |value| data.base.set_forbid_player_commands(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "TurretsLinked",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |value| data.base.set_turrets_linked(value), tokens)
        },
    },
    FieldParse {
        token: "MaxBoxes",
        parse: |_, data, tokens| parse_int_field(&mut |value| data.max_boxes_data = value, tokens),
    },
    FieldParse {
        token: "SupplyCenterActionDelay",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |value| data.center_delay = value, tokens)
        },
    },
    FieldParse {
        token: "SupplyWarehouseActionDelay",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |value| data.warehouse_delay = value, tokens)
        },
    },
    FieldParse {
        token: "SupplyWarehouseScanDistance",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.warehouse_scan_distance = value, tokens)
        },
    },
    FieldParse {
        token: "SuppliesDepletedVoice",
        parse: |_, data, tokens| {
            parse_ascii_string_field(&mut |value| data.supplies_depleted_voice = value, tokens)
        },
    },
    FieldParse {
        token: "RappelSpeed",
        parse: |_, data, tokens| {
            parse_velocity_real_field(&mut |value| data.rappel_speed = value, tokens)
        },
    },
    FieldParse {
        token: "RopeDropSpeed",
        parse: |_, data, tokens| {
            parse_velocity_real_field(&mut |value| data.rope_drop_speed = value, tokens)
        },
    },
    FieldParse {
        token: "RopeName",
        parse: |_, data, tokens| {
            parse_ascii_string_field(&mut |value| data.rope_name = value, tokens)
        },
    },
    FieldParse {
        token: "RopeFinalHeight",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.rope_final_height = value, tokens)
        },
    },
    FieldParse {
        token: "RopeWidth",
        parse: |_, data, tokens| parse_real_field(&mut |value| data.rope_width = value, tokens),
    },
    FieldParse {
        token: "RopeWobbleLen",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.rope_wobble_len = value, tokens)
        },
    },
    FieldParse {
        token: "RopeWobbleAmplitude",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.rope_wobble_amp = value, tokens)
        },
    },
    FieldParse {
        token: "RopeWobbleRate",
        parse: |_, data, tokens| {
            parse_angular_velocity_real_field(&mut |value| data.rope_wobble_rate = value, tokens)
        },
    },
    FieldParse {
        token: "RopeColor",
        parse: |_, data, tokens| {
            parse_rgb_color_field(&mut |value| data.rope_color = value, tokens)
        },
    },
    FieldParse {
        token: "NumRopes",
        parse: |_, data, tokens| parse_unsigned_field(&mut |value| data.num_ropes = value, tokens),
    },
    FieldParse {
        token: "PerRopeDelayMin",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |value| data.per_rope_delay_min = value, tokens)
        },
    },
    FieldParse {
        token: "PerRopeDelayMax",
        parse: |_, data, tokens| {
            parse_duration_unsigned_field(&mut |value| data.per_rope_delay_max = value, tokens)
        },
    },
    FieldParse {
        token: "MinDropHeight",
        parse: |_, data, tokens| {
            parse_real_field(&mut |value| data.min_drop_height = value, tokens)
        },
    },
    FieldParse {
        token: "WaitForRopesToDrop",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |value| data.wait_for_ropes_to_drop = value, tokens)
        },
    },
    FieldParse {
        token: "RotorWashParticleSystem",
        parse: |_, data, tokens| {
            parse_ascii_string_field(&mut |value| data.rotor_wash_particle_system = value, tokens)
        },
    },
    FieldParse {
        token: "UpgradedSupplyBoost",
        parse: |_, data, tokens| {
            parse_int_field(&mut |value| data.upgraded_supply_boost = value, tokens)
        },
    },
];

/// Runtime data for Chinook AI.
#[derive(Debug, Clone)]
pub struct ChinookAIUpdateData {
    pub supply: SupplyTruckAIUpdateData,
    pub rope_name: AsciiString,
    pub rotor_wash_particle_system: AsciiString,
    pub rappel_speed: Real,
    pub rope_drop_speed: Real,
    pub rope_width: Real,
    pub rope_final_height: Real,
    pub rope_wobble_len: Real,
    pub rope_wobble_amp: Real,
    pub rope_wobble_rate: Real,
    pub rope_color: RGBColor,
    pub num_ropes: UnsignedInt,
    pub per_rope_delay_min: UnsignedInt,
    pub per_rope_delay_max: UnsignedInt,
    pub min_drop_height: Real,
    pub wait_for_ropes_to_drop: Bool,
    pub upgraded_supply_boost: Int,
}

#[derive(Debug, Clone)]
struct RopeInfo {
    rope_drawable: Option<Arc<RwLock<Drawable>>>,
    rope_drawable_id: DrawableID,
    drop_start_mtx: Matrix3D,
    rope_speed: Real,
    rope_len: Real,
    rope_len_max: Real,
    next_drop_time: UnsignedInt,
    rappeller_ids: Vec<ObjectID>,
}

#[derive(Debug, Default)]
struct ChinookCombatDropState {
    ropes: Vec<RopeInfo>,
}

impl Default for ChinookAIUpdateData {
    fn default() -> Self {
        let module = ChinookAIUpdateModuleData::default();
        Self::from_module(&module)
    }
}

impl ChinookAIUpdateData {
    pub fn from_module(data: &ChinookAIUpdateModuleData) -> Self {
        Self {
            supply: SupplyTruckAIUpdateData {
                max_boxes: data.max_boxes_data,
                warehouse_scan_distance: data.warehouse_scan_distance,
                warehouse_delay: data.warehouse_delay,
                center_delay: data.center_delay,
                supplies_depleted_voice: data.supplies_depleted_voice.to_string(),
            },
            rope_name: data.rope_name.clone(),
            rotor_wash_particle_system: data.rotor_wash_particle_system.clone(),
            rappel_speed: data.rappel_speed,
            rope_drop_speed: data.rope_drop_speed,
            rope_width: data.rope_width,
            rope_final_height: data.rope_final_height,
            rope_wobble_len: data.rope_wobble_len,
            rope_wobble_amp: data.rope_wobble_amp,
            rope_wobble_rate: data.rope_wobble_rate,
            rope_color: data.rope_color,
            num_ropes: data.num_ropes,
            per_rope_delay_min: data.per_rope_delay_min,
            per_rope_delay_max: data.per_rope_delay_max,
            min_drop_height: data.min_drop_height,
            wait_for_ropes_to_drop: data.wait_for_ropes_to_drop,
            upgraded_supply_boost: data.upgraded_supply_boost,
        }
    }
}

/// Chinook AI Update module (matches C++ ChinookAIUpdate).
#[derive(Debug)]
pub struct ChinookAIUpdate {
    data: ChinookAIUpdateData,
    base: SupplyTruckAIUpdate,
    object_id: ObjectID,
    flight_status: ChinookFlightStatus,
    airfield_for_healing: ObjectID,
    original_pos: Coord3D,
    pending_command: Option<AiCommandParams>,
    combat_drop_started: bool,
    combat_drop_target: Option<ObjectID>,
    combat_drop_pos: Coord3D,
    combat_drop_state: Option<ChinookCombatDropState>,
}

impl ChinookAIUpdate {
    pub fn new(data: ChinookAIUpdateData, object_id: ObjectID, player_index: i32) -> Self {
        let base = SupplyTruckAIUpdate::new(data.supply.clone(), object_id, player_index as u32);
        Self {
            data,
            base,
            object_id,
            flight_status: ChinookFlightStatus::Flying,
            airfield_for_healing: INVALID_ID,
            original_pos: Coord3D::ZERO,
            pending_command: None,
            combat_drop_started: false,
            combat_drop_target: None,
            combat_drop_pos: Coord3D::ZERO,
            combat_drop_state: None,
        }
    }

    fn owner_object(&self) -> Option<Arc<RwLock<Object>>> {
        TheGameLogic::find_object_by_id(self.object_id)
    }

    fn get_potential_rappeller(&self) -> Option<Arc<RwLock<Object>>> {
        let owner = self.owner_object()?;
        let owner_guard = owner.read().ok()?;
        let contain = owner_guard.get_contain()?;
        for object_id in contain.get_contained_objects() {
            let Some(obj) = TheGameLogic::find_object_by_id(object_id) else {
                continue;
            };
            let is_rappeller = if let Ok(obj_guard) = obj.read() {
                obj_guard.is_kind_of(KindOf::CanRappel)
            } else {
                false
            };
            if is_rappeller {
                return Some(obj);
            }
        }
        None
    }

    fn init_rope_draw_params(
        drawable: &Arc<RwLock<Drawable>>,
        length: Real,
        width: Real,
        color: RGBColor,
        wobble_len: Real,
        wobble_amp: Real,
        wobble_rate: Real,
    ) {
        drawable.init_rope_draw_params(length, width, color, wobble_len, wobble_amp, wobble_rate);
    }

    fn set_rope_cur_len(drawable: &Arc<RwLock<Drawable>>, length: Real) {
        drawable.set_rope_cur_len(length);
    }

    fn set_rope_speed(
        drawable: &Arc<RwLock<Drawable>>,
        cur_speed: Real,
        max_speed: Real,
        accel: Real,
    ) {
        drawable.set_rope_speed(cur_speed, max_speed, accel);
    }

    fn start_combat_drop(&mut self) -> bool {
        let Some(owner) = self.owner_object() else {
            return false;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return false;
        };
        let Some(drawable) = owner_guard.get_drawable() else {
            return false;
        };
        let Ok(draw_guard) = drawable.read() else {
            return false;
        };

        owner_guard.set_disabled(crate::common::DisabledType::Held);
        while self.base.lose_one_box() {}

        let now = TheGameLogic::get_frame();
        let rope_template = TheThingFactory::find_template(self.data.rope_name.as_str());
        let mut rope_positions = draw_guard.get_pristine_bone_positions("RopeStart", 1, 32);
        let mut drop_transforms = draw_guard.get_pristine_bone_transforms("RopeEnd", 1, 32);

        let mut num_ropes = self.data.num_ropes as usize;
        if num_ropes > rope_positions.len() {
            num_ropes = rope_positions.len();
        }
        if num_ropes > drop_transforms.len() {
            num_ropes = drop_transforms.len();
        }
        if num_ropes == 0 {
            return false;
        }

        rope_positions.truncate(num_ropes);
        drop_transforms.truncate(num_ropes);

        let mut ropes = Vec::with_capacity(num_ropes);
        for i in 0..num_ropes {
            let drop_start_mtx =
                owner_guard.convert_bone_pos_to_world_pos(None, Some(&drop_transforms[i]));

            let (rope_drawable_id, rope_drawable) = if let (Some(template), Some(client)) =
                (rope_template.as_ref(), TheGameClient::get())
            {
                let id = client.create_drawable(template.as_ref());
                let drawable_arc = client.get_drawable_arc(id);
                (id, drawable_arc)
            } else {
                (INVALID_DRAWABLE_ID, None)
            };

            if let Some(rope_drawable) = rope_drawable.as_ref() {
                let rope_world_mtx =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&rope_positions[i]), None);
                if let Ok(mut rope_guard) = rope_drawable.write() {
                    rope_guard.set_transform(rope_world_mtx);
                }
            }

            let mut rope_len_max = 0.0;
            if let Some(terrain) = TheTerrainLogic::get() {
                let rope_world_mtx =
                    owner_guard.convert_bone_pos_to_world_pos(Some(&rope_positions[i]), None);
                let (_, _, translation) = rope_world_mtx.to_scale_rotation_translation();
                let rope_pos = Coord3D::new(translation.x, translation.y, translation.z);
                let layer = terrain.get_highest_layer_for_destination(&rope_pos);
                let ground = terrain.get_layer_height(rope_pos.x, rope_pos.y, layer);
                rope_len_max = rope_pos.z - ground - self.data.rope_final_height;
            }

            if let Some(rope_drawable) = rope_drawable.as_ref() {
                Self::init_rope_draw_params(
                    rope_drawable,
                    rope_len_max,
                    self.data.rope_width,
                    self.data.rope_color,
                    self.data.rope_wobble_len,
                    self.data.rope_wobble_amp,
                    self.data.rope_wobble_rate,
                );
            }

            let next_delay = get_game_logic_random_value(
                self.data.per_rope_delay_min as i32,
                self.data.per_rope_delay_max as i32,
            ) as UnsignedInt;

            ropes.push(RopeInfo {
                rope_drawable,
                rope_drawable_id,
                drop_start_mtx,
                rope_speed: 0.0,
                rope_len: 1.0,
                rope_len_max,
                next_drop_time: now + next_delay - self.data.per_rope_delay_min,
                rappeller_ids: Vec::new(),
            });
        }

        self.combat_drop_state = Some(ChinookCombatDropState { ropes });
        self.combat_drop_started = true;
        true
    }

    fn update_combat_drop(&mut self) -> bool {
        let Some(mut state) = self.combat_drop_state.take() else {
            return true;
        };
        let Some(owner) = self.owner_object() else {
            return true;
        };
        let Ok(owner_guard) = owner.read() else {
            return true;
        };
        let Some(_contain) = owner_guard.get_contain() else {
            return true;
        };

        // remove done rappellers
        for rope in &mut state.ropes {
            rope.rappeller_ids.retain(|id| {
                if let Some(rappeller) = TheGameLogic::find_object_by_id(*id) {
                    if let Ok(rappeller_guard) = rappeller.read() {
                        return !rappeller_guard.is_effectively_dead()
                            && rappeller_guard.is_above_terrain();
                    }
                }
                false
            });
        }

        let now = TheGameLogic::get_frame();
        let gravity = global_data::read_safe()
            .map(|data| data.gravity.abs())
            .unwrap_or(9.81);

        let mut ropes_in_use = 0;
        for rope in &mut state.ropes {
            if rope.rope_len < rope.rope_len_max {
                rope.rope_speed += gravity;
                if rope.rope_speed > self.data.rope_drop_speed {
                    rope.rope_speed = self.data.rope_drop_speed;
                }
                rope.rope_len += rope.rope_speed;
                if let Some(rope_drawable) = rope.rope_drawable.as_ref() {
                    Self::set_rope_cur_len(rope_drawable, rope.rope_len);
                }
                if self.data.wait_for_ropes_to_drop {
                    rope.next_drop_time = rope.next_drop_time.saturating_add(1);
                    continue;
                }
            }

            if now >= rope.next_drop_time {
                if let Some(rappeller) = self.get_potential_rappeller() {
                    if let Ok(rappeller_guard) = rappeller.read() {
                        let exit_interface = owner_guard.get_object_exit_interface();
                        let exit_door = exit_interface
                            .as_ref()
                            .and_then(|exit| {
                                exit.lock().ok().map(|mut guard| {
                                    guard.reserve_door_for_exit(
                                        Some(&*owner_guard),
                                        Some(&*rappeller_guard),
                                    )
                                })
                            })
                            .unwrap_or(crate::modules::DOOR_NONE_AVAILABLE);

                        if exit_door != crate::modules::DOOR_NONE_AVAILABLE {
                            if let Some(exit) = exit_interface {
                                let _ = exit.lock().ok().map(|mut guard| {
                                    guard.exit_object_via_door(
                                        rappeller.read().map(|g| g.get_id()).unwrap_or(0),
                                        exit_door,
                                    )
                                });
                            }
                        }
                    }

                    if let Ok(mut rappeller_guard) = rappeller.write() {
                        rappeller_guard.set_transform_matrix(&rope.drop_start_mtx);
                    }

                    if let Ok(rappeller_guard) = rappeller.read() {
                        if let Some(ai) = rappeller_guard.get_ai_update_interface() {
                            let mut params = AiCommandParams::new(
                                AiCommandType::RappelInto,
                                CommandSourceType::FromAi,
                            );
                            params.obj = self.combat_drop_target;
                            params.pos = self.combat_drop_pos;
                            let _ = ai.execute_command(&params);
                        }
                    }

                    if let Ok(rappeller_guard) = rappeller.read() {
                        rope.rappeller_ids.push(rappeller_guard.get_id());
                    }

                    let next_delay = get_game_logic_random_value(
                        self.data.per_rope_delay_min as i32,
                        self.data.per_rope_delay_max as i32,
                    )
                    .max(self.data.per_rope_delay_min as i32)
                        as UnsignedInt;
                    rope.next_drop_time = now + next_delay;
                }
            }

            if !rope.rappeller_ids.is_empty() {
                ropes_in_use += 1;
            }
        }

        let done = ropes_in_use == 0 && self.get_potential_rappeller().is_none();
        if !done {
            self.combat_drop_state = Some(state);
        }
        done
    }

    fn finish_combat_drop(&mut self, owner_dead: bool) {
        let Some(owner) = self.owner_object() else {
            self.combat_drop_state = None;
            self.combat_drop_started = false;
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            self.combat_drop_state = None;
            self.combat_drop_started = false;
            return;
        };

        owner_guard.clear_disabled(crate::common::DisabledType::Held);
        self.flight_status = ChinookFlightStatus::Flying;

        if owner_dead {
            if let Some(state) = self.combat_drop_state.as_ref() {
                for rope in &state.ropes {
                    for rappeller_id in &rope.rappeller_ids {
                        if let Some(rappeller) = TheGameLogic::find_object_by_id(*rappeller_id) {
                            if let Ok(rappeller_guard) = rappeller.read() {
                                if let Some(ai) = rappeller_guard.get_ai_update_interface() {
                                    ai.ai_idle(CommandSourceType::FromAi);
                                }
                            }
                        }
                    }
                }
            }
        }

        let now = TheGameLogic::get_frame();
        let gravity = global_data::read_safe()
            .map(|data| data.gravity.abs())
            .unwrap_or(9.81);
        if let Some(state) = self.combat_drop_state.take() {
            for rope in state.ropes {
                if let Some(rope_drawable) = rope.rope_drawable.as_ref() {
                    let initial_speed = gravity * 30.0;
                    Self::set_rope_speed(
                        rope_drawable,
                        initial_speed,
                        self.data.rope_drop_speed,
                        gravity,
                    );
                }
                if rope.rope_drawable_id != INVALID_DRAWABLE_ID {
                    if let Some(client) = TheGameClient::get() {
                        let expiration = LOGICFRAMES_PER_SECOND * 5;
                        client
                            .set_drawable_expiration_date(rope.rope_drawable_id, now + expiration);
                    }
                }
            }
        }
        self.combat_drop_started = false;
    }

    pub fn record_original_position(&mut self, pos: Coord3D) {
        self.original_pos = pos;
    }

    pub fn get_original_position(&self) -> Coord3D {
        self.original_pos
    }

    pub fn set_airfield_for_healing(&mut self, id: ObjectID) {
        if self.airfield_for_healing != INVALID_ID && self.airfield_for_healing != id {
            if let (Some(airfield), Some(owner)) = (
                TheGameLogic::find_object_by_id(self.airfield_for_healing),
                self.owner_object(),
            ) {
                if let Ok(guard) = airfield.read() {
                    let _ = guard.with_parking_place_behavior(|pp| {
                        pp.set_healee(Some(owner.clone()), false);
                    });
                }
            }
        }
        self.airfield_for_healing = id;
    }

    fn set_flight_status(&mut self, status: ChinookFlightStatus, ai: &mut dyn AIUpdateInterface) {
        self.flight_status = status;
        match status {
            ChinookFlightStatus::Landed => {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Taxiing);
            }
            ChinookFlightStatus::TakingOff | ChinookFlightStatus::Landing => {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let _ = ai.set_allow_invalid_position(false);
                if let Some(locomotor) = ai.get_cur_locomotor() {
                    if let Ok(mut guard) = locomotor.lock() {
                        guard.set_precise_z_pos(true);
                        guard.set_ultra_accurate(true);
                    }
                }
            }
            ChinookFlightStatus::Flying => {
                if let Some(locomotor) = ai.get_cur_locomotor() {
                    if let Ok(mut guard) = locomotor.lock() {
                        guard.set_precise_z_pos(false);
                        guard.set_ultra_accurate(false);
                    }
                }
            }
            _ => {}
        }
    }

    pub fn is_idle(&self) -> bool {
        if self.pending_command.is_some() {
            return false;
        }
        let mut result = self.base.get_state() == SupplyTruckState::Idle;
        if result && self.flight_status == ChinookFlightStatus::Landed {
            if let Some(owner) = self.owner_object() {
                if let Ok(guard) = owner.read() {
                    if let Some(contain) = guard.get_contain() {
                        if contain.has_objects_wanting_to_enter_or_exit() {
                            result = false;
                        }
                    }
                }
            }
        }
        result
    }

    pub fn is_currently_ferrying_supplies(&self) -> bool {
        self.base.is_currently_ferrying_supplies()
    }

    pub fn is_available_for_supplying(&self) -> bool {
        if !self.base.is_available_for_supplying() {
            return false;
        }
        let Some(owner) = self.owner_object() else {
            return false;
        };
        let Ok(guard) = owner.read() else {
            return false;
        };
        let Some(contain) = guard.get_contain() else {
            return false;
        };
        if contain.has_objects_wanting_to_enter_or_exit() {
            return false;
        }
        if contain.get_contained_count() > 0 {
            return false;
        }
        if contain.is_special_overlord_style_container() {
            return false;
        }
        true
    }

    pub fn is_allowed_to_adjust_destination(&self) -> bool {
        self.flight_status != ChinookFlightStatus::Landed
    }

    pub fn get_ai_free_to_exit(
        &self,
        exiter: &Object,
    ) -> crate::object::production::AIFreeToExitType {
        if self.flight_status == ChinookFlightStatus::Landed
            || (self.flight_status == ChinookFlightStatus::DoingCombatDrop
                && exiter.is_kind_of(KindOf::CanRappel))
        {
            crate::object::production::AIFreeToExitType::FreeToExit
        } else {
            crate::object::production::AIFreeToExitType::WaitToExit
        }
    }

    pub fn get_upgraded_supply_boost(&self) -> u32 {
        let Some(owner) = self.owner_object() else {
            return 0;
        };
        let Ok(owner_guard) = owner.read() else {
            return 0;
        };
        let Some(player_id) = owner_guard.get_controlling_player_id() else {
            return 0;
        };
        let upgrade = get_upgrade_center()
            .read()
            .ok()
            .and_then(|center| center.find_upgrade("Upgrade_AmericaSupplyLines"));
        if let Some(upgrade) = upgrade {
            let player_has = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(player_id as i32).cloned())
                .and_then(|player| {
                    let guard = player.read().ok()?;
                    Some(guard.has_upgrade_complete(&upgrade))
                })
                .unwrap_or(false);
            if player_has {
                return self.data.upgraded_supply_boost.max(0) as u32;
            }
        }
        0
    }

    pub fn handle_command(
        &mut self,
        params: &AiCommandParams,
        ai: &mut dyn AIUpdateInterface,
    ) -> bool {
        self.set_airfield_for_healing(INVALID_ID);

        if matches!(
            self.flight_status,
            ChinookFlightStatus::TakingOff
                | ChinookFlightStatus::Landing
                | ChinookFlightStatus::DoingCombatDrop
        ) {
            self.pending_command = Some(params.clone());
            return true;
        }

        match params.cmd {
            AiCommandType::MoveToPositionAndEvacuate
            | AiCommandType::MoveToPositionAndEvacuateAndExit => {
                let Some(owner) = self.owner_object() else {
                    return true;
                };
                let Ok(owner_guard) = owner.read() else {
                    return true;
                };
                let delta = *owner_guard.get_position() - params.pos;
                let dist_sqr = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
                let thresh = 3.0;
                if dist_sqr > thresh * thresh && self.flight_status == ChinookFlightStatus::Landed {
                    self.pending_command = Some(params.clone());
                    self.set_flight_status(ChinookFlightStatus::TakingOff, ai);
                    return true;
                }
                false
            }
            AiCommandType::Exit | AiCommandType::Evacuate => {
                if self.flight_status != ChinookFlightStatus::Landed {
                    self.pending_command = Some(params.clone());
                    self.set_flight_status(ChinookFlightStatus::Landing, ai);
                    return true;
                }
                false
            }
            _ => {
                if self.flight_status != ChinookFlightStatus::Flying {
                    self.pending_command = Some(params.clone());
                    self.set_flight_status(ChinookFlightStatus::TakingOff, ai);
                    return true;
                }
                false
            }
        }
    }

    pub fn private_idle(&mut self, cmd_source: CommandSourceType) {
        if let Some(owner) = self.owner_object() {
            if let Ok(guard) = owner.read() {
                if let Some(contain) = guard.get_contain() {
                    if let Some(rider_id) = contain.friend_get_rider() {
                        if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                            if let Ok(rider_guard) = rider.read() {
                                if let Some(ai) = rider_guard.get_ai_update_interface() {
                                    if let Ok(mut ai_guard) = ai.lock() {
                                        let _ = ai_guard.ai_idle();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        self.base.private_idle(cmd_source);
    }

    pub fn private_dock(&mut self, dock_id: Option<ObjectID>, cmd_source: CommandSourceType) {
        self.base.private_dock(dock_id, cmd_source);
    }

    pub fn private_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if let Some(owner) = self.owner_object() {
            if let Ok(guard) = owner.read() {
                if !guard.test_status(crate::common::ObjectStatusTypes::CanAttack) {
                    return;
                }
                if let Some(contain) = guard.get_contain() {
                    if matches!(
                        cmd_source,
                        CommandSourceType::FromPlayer | CommandSourceType::FromScript
                    ) {
                        let passengers = contain.get_contained_objects().to_vec();
                        for passenger_id in passengers {
                            if !contain.is_passenger_allowed_to_fire(Some(passenger_id)) {
                                continue;
                            }
                            let Some(passenger) = TheGameLogic::find_object_by_id(passenger_id)
                            else {
                                continue;
                            };
                            let Ok(pass_guard) = passenger.read() else {
                                continue;
                            };
                            if !pass_guard.is_kind_of(KindOf::Infantry) {
                                continue;
                            }
                            if pass_guard.is_kind_of(KindOf::PortableStructure)
                                && (pass_guard.is_disabled_by_type(
                                    crate::common::DisabledType::DisabledHacked,
                                ) || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::DisabledEmp)
                                    || pass_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledSubdued,
                                    )
                                    || pass_guard.is_disabled_by_type(
                                        crate::common::DisabledType::Paralyzed,
                                    ))
                            {
                                continue;
                            }
                            if let Some(ai) = pass_guard.get_ai_update_interface() {
                                ai.ai_attack_object_id(victim_id, max_shots_to_fire, cmd_source);
                            }
                        }
                        self.tell_portable_structure_to_attack_with_me(
                            victim_id,
                            max_shots_to_fire,
                            cmd_source,
                        );
                    }
                }
            }
        }
    }

    pub fn private_force_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        let victim = TheGameLogic::find_object_by_id(victim_id);
        if victim.is_none() {
            return;
        }
        if let Some(owner) = self.owner_object() {
            if let Ok(guard) = owner.read() {
                if !guard.test_status(crate::common::ObjectStatusTypes::CanAttack) {
                    return;
                }
                if let Some(contain) = guard.get_contain() {
                    if matches!(
                        cmd_source,
                        CommandSourceType::FromPlayer | CommandSourceType::FromScript
                    ) {
                        let passengers = contain.get_contained_objects().to_vec();
                        for passenger_id in passengers {
                            if !contain.is_passenger_allowed_to_fire(Some(passenger_id)) {
                                continue;
                            }
                            let Some(passenger) = TheGameLogic::find_object_by_id(passenger_id)
                            else {
                                continue;
                            };
                            let Ok(pass_guard) = passenger.read() else {
                                continue;
                            };
                            if !pass_guard.is_kind_of(KindOf::Infantry) {
                                continue;
                            }
                            if pass_guard.is_kind_of(KindOf::PortableStructure)
                                && (pass_guard.is_disabled_by_type(
                                    crate::common::DisabledType::DisabledHacked,
                                ) || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::DisabledEmp)
                                    || pass_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledSubdued,
                                    )
                                    || pass_guard.is_disabled_by_type(
                                        crate::common::DisabledType::Paralyzed,
                                    ))
                            {
                                continue;
                            }
                            if let Some(ai) = pass_guard.get_ai_update_interface() {
                                if let Some(victim_arc) = victim.as_ref() {
                                    ai.ai_force_attack_object(
                                        victim_arc,
                                        max_shots_to_fire,
                                        cmd_source,
                                    );
                                }
                            }
                        }
                    }
                    if let Some(rider_id) = contain.friend_get_rider() {
                        if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                            if let Ok(rider_guard) = rider.read() {
                                if rider_guard.is_kind_of(KindOf::PortableStructure)
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledHacked,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledEmp,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledSubdued,
                                    )
                                    && !rider_guard
                                        .is_disabled_by_type(crate::common::DisabledType::Paralyzed)
                                {
                                    if let Some(ai) = rider_guard.get_ai_update_interface() {
                                        if let Some(victim_arc) = victim.as_ref() {
                                            ai.ai_force_attack_object(
                                                victim_arc,
                                                max_shots_to_fire,
                                                cmd_source,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn private_attack_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if let Some(owner) = self.owner_object() {
            if let Ok(guard) = owner.read() {
                if !guard.test_status(crate::common::ObjectStatusTypes::CanAttack) {
                    return;
                }
                if let Some(contain) = guard.get_contain() {
                    if matches!(
                        cmd_source,
                        CommandSourceType::FromPlayer | CommandSourceType::FromScript
                    ) {
                        let passengers = contain.get_contained_objects().to_vec();
                        for passenger_id in passengers {
                            if !contain.is_passenger_allowed_to_fire(Some(passenger_id)) {
                                continue;
                            }
                            let Some(passenger) = TheGameLogic::find_object_by_id(passenger_id)
                            else {
                                continue;
                            };
                            let Ok(pass_guard) = passenger.read() else {
                                continue;
                            };
                            if !pass_guard.is_kind_of(KindOf::Infantry) {
                                continue;
                            }
                            if pass_guard.is_kind_of(KindOf::PortableStructure)
                                && (pass_guard.is_disabled_by_type(
                                    crate::common::DisabledType::DisabledHacked,
                                ) || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::DisabledEmp)
                                    || pass_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledSubdued,
                                    )
                                    || pass_guard.is_disabled_by_type(
                                        crate::common::DisabledType::Paralyzed,
                                    ))
                            {
                                continue;
                            }
                            if let Some(ai) = pass_guard.get_ai_update_interface() {
                                ai.ai_attack_position(pos, max_shots_to_fire, cmd_source);
                            }
                        }
                    }
                    if let Some(rider_id) = contain.friend_get_rider() {
                        if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                            if let Ok(rider_guard) = rider.read() {
                                if rider_guard.is_kind_of(KindOf::PortableStructure)
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledHacked,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledEmp,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledSubdued,
                                    )
                                    && !rider_guard
                                        .is_disabled_by_type(crate::common::DisabledType::Paralyzed)
                                {
                                    if let Some(ai) = rider_guard.get_ai_update_interface() {
                                        ai.ai_attack_position(pos, max_shots_to_fire, cmd_source);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn tell_portable_structure_to_attack_with_me(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        if let Some(owner) = self.owner_object() {
            if let Ok(guard) = owner.read() {
                if let Some(contain) = guard.get_contain() {
                    if let Some(rider_id) = contain.friend_get_rider() {
                        if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                            if let Ok(rider_guard) = rider.read() {
                                if rider_guard.is_kind_of(KindOf::PortableStructure)
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledHacked,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledEmp,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledSubdued,
                                    )
                                    && !rider_guard
                                        .is_disabled_by_type(crate::common::DisabledType::Paralyzed)
                                {
                                    if let Some(ai) = rider_guard.get_ai_update_interface() {
                                        ai.ai_attack_object_id(
                                            victim_id,
                                            max_shots_to_fire,
                                            cmd_source,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn private_get_repaired(
        &mut self,
        repair_depot_id: ObjectID,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) {
        if matches!(
            self.flight_status,
            ChinookFlightStatus::Landing | ChinookFlightStatus::Landed
        ) {
            return;
        }
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Some(repair_depot) = TheGameLogic::find_object_by_id(repair_depot_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(repair_guard)) = (owner.read(), repair_depot.read()) else {
            return;
        };
        if !ActionManager::can_get_repaired_at(&*owner_guard, &*repair_guard, cmd_source) {
            return;
        }

        self.set_airfield_for_healing(repair_depot_id);
        let mut pos = *repair_guard.get_position();
        let mut tmp = pos;
        let mut options = crate::helpers::FindPositionOptions::default();
        options.max_radius = repair_guard
            .get_geometry_info()
            .get_bounding_circle_radius()
            * 100.0;
        if let Some(partition) = ThePartitionManager::get() {
            if partition.find_position_around_with_options(&pos, &options, &mut tmp) {
                pos = tmp;
            }
        }
        let _ = ai.set_movement_target(&pos);
        self.set_flight_status(ChinookFlightStatus::Landing, ai);
    }

    pub fn private_combat_drop(
        &mut self,
        target_id: Option<ObjectID>,
        pos: Coord3D,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) {
        let target = target_id.and_then(TheGameLogic::find_object_by_id);
        if let Some(target_obj) = target.as_ref() {
            if cmd_source == CommandSourceType::FromPlayer {
                if let (Some(owner), Ok(target_guard)) = (self.owner_object(), target_obj.read()) {
                    if let Ok(owner_guard) = owner.read() {
                        if !ActionManager::can_enter_object(
                            &*owner_guard,
                            &*target_guard,
                            cmd_source,
                            CanEnterType::CombatDropInto,
                        ) {
                            return;
                        }
                    }
                }
            }
        }

        let mut local_pos = pos;
        if target.is_none() {
            let mut tmp = local_pos;
            let mut options = crate::helpers::FindPositionOptions::default();
            if let Some(owner) = self.owner_object() {
                if let Ok(owner_guard) = owner.read() {
                    options.max_radius =
                        owner_guard.get_geometry_info().get_bounding_circle_radius() * 100.0;
                }
            }
            if let Some(partition) = ThePartitionManager::get() {
                if partition.find_position_around_with_options(&local_pos, &options, &mut tmp) {
                    local_pos = tmp;
                }
            }
        }

        let _ = ai.set_movement_target(&local_pos);
        self.flight_status = ChinookFlightStatus::DoingCombatDrop;
        self.combat_drop_started = false;
        self.combat_drop_state = None;
        self.combat_drop_target = target_id;
        self.combat_drop_pos = local_pos;
    }

    pub fn is_doing_combat_drop(&self) -> bool {
        self.flight_status == ChinookFlightStatus::DoingCombatDrop
            || self.combat_drop_started
            || self.combat_drop_state.is_some()
    }

    fn update_rotor_wash(&self) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let local_index = player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(-1);
        if local_index < 0 {
            return;
        }
        if owner_guard.get_shrouded_status(local_index) != crate::common::ObjectShroudStatus::Clear
        {
            return;
        }
        if !matches!(
            self.flight_status,
            ChinookFlightStatus::Landing
                | ChinookFlightStatus::TakingOff
                | ChinookFlightStatus::Landed
        ) {
            return;
        }
        let mut pos = *owner_guard.get_position();
        let Some(terrain) = TheTerrainLogic::get() else {
            return;
        };
        let ground = terrain.get_ground_height(pos.x, pos.y, None);
        pos.z = ground + 3.0;
        let chopper_elevation = owner_guard.get_position().z - pos.z;
        if get_game_logic_random_value_real(0.0, chopper_elevation) < 5.0 {
            if let Some(ps_manager) = TheParticleSystemManager::get() {
                let template = if self.data.rotor_wash_particle_system.is_empty() {
                    None
                } else {
                    Some(self.data.rotor_wash_particle_system.as_str())
                };
                if let Some(id) = ps_manager.create_particle_system(template) {
                    ps_manager.set_particle_system_position(id, &pos);
                }
            }
        }
    }

    pub fn update(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.update_flight_status(ai);

        if self.airfield_for_healing != INVALID_ID {
            if let (Some(airfield), Some(owner)) = (
                TheGameLogic::find_object_by_id(self.airfield_for_healing),
                self.owner_object(),
            ) {
                if let (Ok(airfield_guard), Ok(owner_guard)) = (airfield.read(), owner.read()) {
                    let mut healed = false;
                    if self.flight_status == ChinookFlightStatus::Landed
                        && self.pending_command.is_none()
                    {
                        if let Some(body) = owner_guard.get_body_module() {
                            if let Ok(body_guard) = body.lock() {
                                if body_guard.get_health() >= body_guard.get_max_health() {
                                    healed = true;
                                }
                            }
                        }
                    }
                    if healed {
                        let _ = airfield_guard.with_parking_place_behavior(|pp| {
                            pp.set_healee(Some(owner.clone()), false);
                        });
                        self.set_flight_status(ChinookFlightStatus::TakingOff, ai);
                    } else {
                        let landed = self.flight_status == ChinookFlightStatus::Landed;
                        let _ = airfield_guard.with_parking_place_behavior(|pp| {
                            pp.set_healee(Some(owner.clone()), landed);
                        });
                    }
                }
            } else {
                self.set_airfield_for_healing(INVALID_ID);
            }
        }

        if let Some(owner) = self.owner_object() {
            if let Ok(guard) = owner.read() {
                if let Some(contain) = guard.get_contain() {
                    if self.base.get_state() == SupplyTruckState::Idle {
                        let waiting = contain.has_objects_wanting_to_enter_or_exit();
                        if let Some(command) = self.pending_command.take() {
                            let _ = ai.execute_command(&command);
                        } else if waiting && self.flight_status != ChinookFlightStatus::Landed {
                            self.set_flight_status(ChinookFlightStatus::Landing, ai);
                        } else if !waiting
                            && self.flight_status == ChinookFlightStatus::Landed
                            && self.airfield_for_healing == INVALID_ID
                        {
                            self.set_flight_status(ChinookFlightStatus::TakingOff, ai);
                        }
                    }

                    if TheGameLogic::get_frame() % 10 == 1 {
                        if let Some(ai_update) = guard.get_ai_update_interface() {
                            if let Ok(ai_guard) = ai_update.lock() {
                                if let Some(victim_id) = ai_guard.get_current_victim() {
                                    if contain.is_passenger_allowed_to_fire(None) {
                                        let passengers = contain.get_contained_objects().to_vec();
                                        for passenger_id in passengers {
                                            if let Some(passenger) =
                                                TheGameLogic::find_object_by_id(passenger_id)
                                            {
                                                if let Ok(pass_guard) = passenger.read() {
                                                    if let Some(pass_ai) =
                                                        pass_guard.get_ai_update_interface()
                                                    {
                                                        pass_ai.ai_attack_object_id(
                                                            victim_id,
                                                            999,
                                                            CommandSourceType::FromAi,
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if self.flight_status == ChinookFlightStatus::DoingCombatDrop {
                        if !self.combat_drop_started {
                            if !self.start_combat_drop() {
                                self.flight_status = ChinookFlightStatus::Flying;
                            }
                        }

                        let owner_dead = guard.is_effectively_dead();
                        if owner_dead {
                            self.finish_combat_drop(true);
                        } else if self.combat_drop_state.is_some() {
                            if self.update_combat_drop() {
                                self.finish_combat_drop(false);
                            }
                        }
                    }
                }
            }
        }

        self.update_rotor_wash();

        self.base.update();
        Ok(())
    }

    fn update_flight_status(&mut self, ai: &mut dyn AIUpdateInterface) {
        let Some(owner) = self.owner_object() else {
            return;
        };
        let Ok(guard) = owner.read() else {
            return;
        };
        let Some(terrain) = TheTerrainLogic::get() else {
            return;
        };
        let pos = *guard.get_position();
        let ground = terrain.get_ground_height(pos.x, pos.y, None);
        let height = pos.z - ground;
        let preferred = ai.get_preferred_height().unwrap_or(0.0);
        match self.flight_status {
            ChinookFlightStatus::TakingOff => {
                if height >= preferred - 1.0 {
                    self.set_flight_status(ChinookFlightStatus::Flying, ai);
                }
            }
            ChinookFlightStatus::Landing => {
                if height <= 1.0 {
                    self.set_flight_status(ChinookFlightStatus::Landed, ai);
                }
            }
            _ => {}
        }
    }
}

impl SupplyTruckAIInterface for ChinookAIUpdate {
    fn get_supplies_count(&self) -> Result<i32, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.base.get_number_boxes())
    }

    fn get_number_boxes(&self) -> i32 {
        self.base.get_number_boxes()
    }

    fn get_action_delay_for_dock(
        &self,
        dock: &Arc<RwLock<Object>>,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        SupplyTruckAIInterface::get_action_delay_for_dock(&self.base, dock)
    }

    fn set_force_wanting_state(&mut self, enabled: bool) {
        self.base.set_force_wanting_state(enabled);
    }

    fn is_forced_into_wanting_state(&self) -> bool {
        self.base.is_forced_into_wanting_state()
    }

    fn set_force_busy_state(&mut self, enabled: bool) {
        self.base.set_force_busy_state(enabled);
    }

    fn is_forced_into_busy_state(&self) -> bool {
        self.base.is_forced_into_busy_state()
    }

    fn get_preferred_dock_id(&self) -> Option<ObjectID> {
        self.base.get_preferred_dock()
    }

    fn get_warehouse_scan_distance(&self, is_ai_player: bool) -> Option<Real> {
        Some(self.base.get_warehouse_scan_distance(is_ai_player))
    }

    fn is_available_for_supplying(&self) -> bool {
        self.is_available_for_supplying()
    }

    fn is_currently_ferrying_supplies(&self) -> bool {
        self.is_currently_ferrying_supplies()
    }

    fn lose_one_box(&mut self) -> bool {
        self.base.lose_one_box()
    }

    fn gain_one_box(&mut self, remaining_stock: i32) -> bool {
        self.base.gain_one_box(remaining_stock)
    }

    fn get_upgraded_supply_boost(&self) -> u32 {
        self.get_upgraded_supply_boost()
    }
}

impl Snapshotable for ChinookAIUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());

        let mut version: u8 = 2;
        xfer_io(xfer.xfer_version(&mut version, 2))?;

        self.base.xfer(xfer)?;

        let mut has_pending_command = self.pending_command.is_some();
        xfer_io(xfer.xfer_bool(&mut has_pending_command))?;
        if has_pending_command {
            let mut storage = self
                .pending_command
                .as_ref()
                .map(chinook_command_storage_from_params)
                .unwrap_or_else(chinook_default_command_storage);
            storage.do_xfer(xfer)?;
            if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
                self.pending_command = Some(chinook_command_params_from_storage(&storage));
            }
        } else if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
            self.pending_command = None;
        }

        let mut flight_status = self.flight_status as i32;
        xfer_io(xfer.xfer_int(&mut flight_status))?;
        if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
            self.flight_status = chinook_flight_status_from_i32(flight_status);
        }

        xfer_io(xfer.xfer_unsigned_int(&mut self.airfield_for_healing))?;

        if version >= 2 {
            xfer_io(xfer.xfer_real(&mut self.original_pos.x))?;
            xfer_io(xfer.xfer_real(&mut self.original_pos.y))?;
            xfer_io(xfer.xfer_real(&mut self.original_pos.z))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn chinook_flight_status_from_i32(value: i32) -> ChinookFlightStatus {
    match value {
        0 => ChinookFlightStatus::TakingOff,
        2 => ChinookFlightStatus::DoingCombatDrop,
        3 => ChinookFlightStatus::Landing,
        4 => ChinookFlightStatus::Landed,
        _ => ChinookFlightStatus::Flying,
    }
}

fn chinook_default_command_storage() -> AICommandParmsStorage {
    AICommandParmsStorage {
        cmd: AiCommandType::NoCommand,
        cmd_source: CommandSourceType::FromAi,
        pos: Coord3D::ZERO,
        obj: INVALID_ID,
        other_obj: INVALID_ID,
        team_name: String::new(),
        coords: Vec::new(),
        waypoint: None,
        polygon: None,
        int_value: 0,
        damage: crate::damage::DamageInfo::new(),
        command_button: None,
        command_button_name: String::new(),
        path: None,
    }
}

fn chinook_command_storage_from_params(params: &AiCommandParams) -> AICommandParmsStorage {
    let mut storage = chinook_default_command_storage();
    storage.cmd = params.cmd;
    storage.cmd_source = params.cmd_source;
    storage.pos = params.pos;
    storage.obj = params.obj.unwrap_or(INVALID_ID);
    storage.other_obj = params.other_obj.unwrap_or(INVALID_ID);
    storage.team_name = params.team.clone().unwrap_or_default();
    storage.coords = params.coords.clone();
    storage.int_value = params.int_value;
    storage
}

fn chinook_command_params_from_storage(storage: &AICommandParmsStorage) -> AiCommandParams {
    let mut params = AiCommandParams::new(storage.cmd, storage.cmd_source);
    params.pos = storage.pos;
    if storage.obj != INVALID_ID {
        params.obj = Some(storage.obj);
    }
    if storage.other_obj != INVALID_ID {
        params.other_obj = Some(storage.other_obj);
    }
    if !storage.team_name.is_empty() {
        params.team = Some(storage.team_name.clone());
    }
    params.coords = storage.coords.clone();
    params.waypoint = storage.waypoint.as_ref().map(|waypoint| waypoint.id);
    params.polygon = storage.polygon.as_ref().map(|polygon| polygon.get_id());
    params.int_value = storage.int_value;
    params
}

/// Module wrapper for ChinookAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct ChinookAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<ChinookAIUpdateModuleData>,
}

impl ChinookAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<ChinookAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

impl Module for ChinookAIUpdateModule {
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

impl Snapshotable for ChinookAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.data).xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::LocomotorSetType;
    use game_engine::system::xfer_load::XferLoad;
    use game_engine::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn parse_field(data: &mut ChinookAIUpdateModuleData, token: &str, values: &[&str]) {
        let field = CHINOOK_AI_UPDATE_FIELDS
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, data, values).expect("field parses");
    }

    #[test]
    fn chinook_fields_accept_ini_equals_token() {
        let mut data = ChinookAIUpdateModuleData::default();

        parse_field(
            &mut data,
            "AutoAcquireEnemiesWhenIdle",
            &["=", "YES", "ATTACK_BUILDINGS"],
        );
        parse_field(
            &mut data,
            "Locomotor",
            &["=", "SET_NORMAL", "ChinookLocomotor"],
        );
        parse_field(&mut data, "MoodAttackCheckRate", &["=", "2000"]);
        parse_field(&mut data, "SurrenderDuration", &["=", "3000"]);
        parse_field(&mut data, "ForbidPlayerCommands", &["=", "Yes"]);
        parse_field(&mut data, "TurretsLinked", &["=", "Yes"]);
        parse_field(&mut data, "MaxBoxes", &["=", "6"]);
        parse_field(&mut data, "SupplyCenterActionDelay", &["=", "1200"]);
        parse_field(&mut data, "SupplyWarehouseActionDelay", &["=", "900"]);
        parse_field(&mut data, "SupplyWarehouseScanDistance", &["=", "275.5"]);
        parse_field(
            &mut data,
            "SuppliesDepletedVoice",
            &["=", "ChinookSupplyEmpty"],
        );
        parse_field(&mut data, "RappelSpeed", &["=", "55.5"]);
        parse_field(&mut data, "RopeDropSpeed", &["=", "12.5"]);
        parse_field(&mut data, "RopeName", &["=", "CombatDropRope"]);
        parse_field(&mut data, "RopeFinalHeight", &["=", "35.0"]);
        parse_field(&mut data, "RopeWidth", &["=", "0.75"]);
        parse_field(&mut data, "RopeWobbleLen", &["=", "16.0"]);
        parse_field(&mut data, "RopeWobbleAmplitude", &["=", "2.5"]);
        parse_field(&mut data, "RopeWobbleRate", &["=", "0.35"]);
        parse_field(&mut data, "RopeColor", &["=", "R:51", "G:102", "B:153"]);
        parse_field(&mut data, "NumRopes", &["=", "5"]);
        parse_field(&mut data, "PerRopeDelayMin", &["=", "600"]);
        parse_field(&mut data, "PerRopeDelayMax", &["=", "900"]);
        parse_field(&mut data, "MinDropHeight", &["=", "40.0"]);
        parse_field(&mut data, "WaitForRopesToDrop", &["=", "No"]);
        parse_field(&mut data, "RotorWashParticleSystem", &["=", "ChinookDust"]);
        parse_field(&mut data, "UpgradedSupplyBoost", &["=", "4"]);

        assert_ne!(data.base.auto_acquire_enemies_when_idle(), 0);
        assert!(data.base.has_locomotor_set(LocomotorSetType::Normal));
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert_eq!(data.max_boxes_data, 6);
        assert_eq!(data.center_delay, 36);
        assert_eq!(data.warehouse_delay, 27);
        assert_eq!(data.warehouse_scan_distance, 275.5);
        assert_eq!(data.supplies_depleted_voice.as_str(), "ChinookSupplyEmpty");
        assert_eq!(data.rappel_speed, 55.5);
        assert_eq!(data.rope_drop_speed, 12.5);
        assert_eq!(data.rope_name.as_str(), "CombatDropRope");
        assert_eq!(data.rope_final_height, 35.0);
        assert_eq!(data.rope_width, 0.75);
        assert_eq!(data.rope_wobble_len, 16.0);
        assert_eq!(data.rope_wobble_amp, 2.5);
        assert_eq!(
            data.rope_wobble_rate,
            INI::parse_angular_velocity_real("0.35").unwrap()
        );
        assert_eq!(data.rope_color.r, 51);
        assert_eq!(data.rope_color.g, 102);
        assert_eq!(data.rope_color.b, 153);
        assert_eq!(data.num_ropes, 5);
        assert_eq!(data.per_rope_delay_min, 18);
        assert_eq!(data.per_rope_delay_max, 27);
        assert_eq!(data.min_drop_height, 40.0);
        assert!(!data.wait_for_ropes_to_drop);
        assert_eq!(data.rotor_wash_particle_system.as_str(), "ChinookDust");
        assert_eq!(data.upgraded_supply_boost, 4);
    }

    #[test]
    fn chinook_xfer_preserves_cpp_runtime_fields() {
        let data = ChinookAIUpdateData::default();
        let mut original = ChinookAIUpdate::new(data.clone(), 77, 1);
        original.base.set_preferred_dock(1234);
        original.base.set_force_wanting_state(true);
        original.flight_status = ChinookFlightStatus::Landing;
        original.airfield_for_healing = 5678;
        original.original_pos = Coord3D::new(10.0, 20.0, 30.0);

        let mut command =
            AiCommandParams::new(AiCommandType::CombatDrop, CommandSourceType::FromAi);
        command.pos = Coord3D::new(40.0, 50.0, 60.0);
        command.obj = Some(4321);
        command.other_obj = Some(8765);
        command.team = Some("TeamAlpha".to_string());
        command.coords = vec![Coord3D::new(1.0, 2.0, 3.0)];
        command.int_value = 99;
        original.pending_command = Some(command);

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            original.xfer(&mut save).unwrap();
        }

        let mut loaded = ChinookAIUpdate::new(data, 77, 1);
        {
            let cursor = Cursor::new(bytes.as_slice());
            let mut load = XferLoad::new(cursor, 1);
            loaded.xfer(&mut load).unwrap();
        }

        assert_eq!(loaded.base.get_preferred_dock(), Some(1234));
        assert!(loaded.base.is_forced_into_wanting_state());
        assert_eq!(loaded.flight_status, ChinookFlightStatus::Landing);
        assert_eq!(loaded.airfield_for_healing, 5678);
        assert_eq!(loaded.original_pos, Coord3D::new(10.0, 20.0, 30.0));

        let pending = loaded.pending_command.expect("pending command restored");
        assert_eq!(pending.cmd, AiCommandType::CombatDrop);
        assert_eq!(pending.cmd_source, CommandSourceType::FromAi);
        assert_eq!(pending.pos, Coord3D::new(40.0, 50.0, 60.0));
        assert_eq!(pending.obj, Some(4321));
        assert_eq!(pending.other_obj, Some(8765));
        assert_eq!(pending.team.as_deref(), Some("TeamAlpha"));
        assert_eq!(pending.coords, vec![Coord3D::new(1.0, 2.0, 3.0)]);
        assert_eq!(pending.int_value, 99);
    }
}
