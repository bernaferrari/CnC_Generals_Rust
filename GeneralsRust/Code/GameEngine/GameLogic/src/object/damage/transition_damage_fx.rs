//! TransitionDamageFX - damage module that triggers FX on damage state transitions.
//! Ported from GameLogic/Object/Damage/TransitionDamageFX.cpp.

use std::str::FromStr;
use std::sync::{Arc, RwLock, Weak};

use crate::common::{
    game_logic_random_value, AsciiString, Bool, Coord3D, DamageTypeFlags, ModuleData, NameKeyType,
    Real, TheFXListStore, TheObjectCreationListStore, XferExt,
};
use crate::damage::{get_damage_type_flag, BodyDamageType, DamageInfo, DamageType};
use crate::helpers::{TheGameLogic, TheParticleSystemManager};
use crate::modules::{BehaviorModuleInterface, BodyModuleInterfaceExt, DamageModuleInterface};
use crate::object::body::body_module::is_condition_worse;
use crate::object::drawable::Drawable;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object as GameObject;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::object_creation_list::{live_creation_context, ObjectCreationList};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType as EngineNameKeyType, Thing as ModuleThing,
};

const DAMAGE_MODULE_MAX_FX: usize = 12;
const BODY_DAMAGE_TYPE_COUNT: usize = 4;
const INVALID_PARTICLE_SYSTEM_ID: crate::common::ParticleSystemID = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FXDamageLocType {
    Bone,
    Coord,
}

#[derive(Debug, Clone)]
pub struct FXLocInfo {
    pub loc_type: FXDamageLocType,
    pub bone_name: AsciiString,
    pub random_bone: Bool,
    pub loc: Coord3D,
}

impl Default for FXLocInfo {
    fn default() -> Self {
        Self {
            loc_type: FXDamageLocType::Coord,
            bone_name: AsciiString::new(),
            random_bone: false,
            loc: Coord3D::ZERO,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FXDamageFXListInfo {
    pub fx: Option<Arc<crate::common::FXList>>,
    pub loc_info: FXLocInfo,
}

impl Default for FXDamageFXListInfo {
    fn default() -> Self {
        Self {
            fx: None,
            loc_info: FXLocInfo::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FXDamageOCLInfo {
    pub ocl: Option<Arc<ObjectCreationList>>,
    pub loc_info: FXLocInfo,
}

impl Default for FXDamageOCLInfo {
    fn default() -> Self {
        Self {
            ocl: None,
            loc_info: FXLocInfo::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FXDamageParticleSystemInfo {
    pub particle_system_name: Option<AsciiString>,
    pub loc_info: FXLocInfo,
}

impl Default for FXDamageParticleSystemInfo {
    fn default() -> Self {
        Self {
            particle_system_name: None,
            loc_info: FXLocInfo::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransitionDamageFXModuleData {
    pub base: crate::object::damage::DamageModuleData,
    pub damage_fx_types: DamageTypeFlags,
    pub fx_list: [[FXDamageFXListInfo; DAMAGE_MODULE_MAX_FX]; BODY_DAMAGE_TYPE_COUNT],
    pub damage_ocl_types: DamageTypeFlags,
    pub ocl: [[FXDamageOCLInfo; DAMAGE_MODULE_MAX_FX]; BODY_DAMAGE_TYPE_COUNT],
    pub damage_particle_types: DamageTypeFlags,
    pub particle_system:
        [[FXDamageParticleSystemInfo; DAMAGE_MODULE_MAX_FX]; BODY_DAMAGE_TYPE_COUNT],
}

impl Default for TransitionDamageFXModuleData {
    fn default() -> Self {
        Self {
            base: crate::object::damage::DamageModuleData::default(),
            damage_fx_types: DamageTypeFlags::all_flags(),
            fx_list: std::array::from_fn(|_| {
                std::array::from_fn(|_| FXDamageFXListInfo::default())
            }),
            damage_ocl_types: DamageTypeFlags::all_flags(),
            ocl: std::array::from_fn(|_| std::array::from_fn(|_| FXDamageOCLInfo::default())),
            damage_particle_types: DamageTypeFlags::all_flags(),
            particle_system: std::array::from_fn(|_| {
                std::array::from_fn(|_| FXDamageParticleSystemInfo::default())
            }),
        }
    }
}

impl Snapshotable for TransitionDamageFXModuleData {
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

crate::impl_legacy_module_data_via_base!(TransitionDamageFXModuleData, base);

impl TransitionDamageFXModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, TRANSITION_DAMAGE_FX_FIELDS)
    }
}

fn parse_damage_type_flags(tokens: &[&str]) -> Result<DamageTypeFlags, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut flags = DamageTypeFlags::empty();
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = DamageTypeFlags::empty();
                continue;
            }

            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };

            if let Ok(damage_type) = DamageType::from_str(name) {
                let flag = DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
                if remove {
                    flags.remove(flag);
                } else {
                    flags.insert(flag);
                }
            }
        }
    }

    Ok(flags)
}

fn parse_damage_fx_types(
    _ini: &mut INI,
    data: &mut TransitionDamageFXModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_fx_types = parse_damage_type_flags(tokens)?;
    Ok(())
}

fn parse_damage_ocl_types(
    _ini: &mut INI,
    data: &mut TransitionDamageFXModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_ocl_types = parse_damage_type_flags(tokens)?;
    Ok(())
}

fn parse_damage_particle_types(
    _ini: &mut INI,
    data: &mut TransitionDamageFXModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_particle_types = parse_damage_type_flags(tokens)?;
    Ok(())
}

fn parse_key_value(token: &str) -> (&str, Option<&str>) {
    let mut parts = token.splitn(2, ':');
    let key = parts.next().unwrap_or("");
    let value = parts.next().filter(|value| !value.is_empty());
    (key, value)
}

fn is_coord_token(token: &str) -> bool {
    let key = token.splitn(2, ':').next().unwrap_or("");
    key.eq_ignore_ascii_case("x") || key.eq_ignore_ascii_case("y") || key.eq_ignore_ascii_case("z")
}

fn parse_coord_token<'a, I>(
    token: &'a str,
    iter: &mut std::iter::Peekable<I>,
    x: &mut Option<Real>,
    y: &mut Option<Real>,
    z: &mut Option<Real>,
) -> Result<bool, INIError>
where
    I: Iterator<Item = &'a str>,
{
    let (key, value_opt) = parse_key_value(token);
    let target = if key.eq_ignore_ascii_case("x") {
        x
    } else if key.eq_ignore_ascii_case("y") {
        y
    } else if key.eq_ignore_ascii_case("z") {
        z
    } else {
        return Ok(false);
    };

    if target.is_some() {
        return Ok(true);
    }

    let value = match value_opt {
        Some(val) => val,
        None => iter.next().ok_or(INIError::InvalidData)?,
    };

    *target = Some(INI::parse_real(value)?);
    Ok(true)
}

fn parse_fx_loc_info<'a, I>(
    iter: &mut std::iter::Peekable<I>,
    loc_info: &mut FXLocInfo,
) -> Result<(), INIError>
where
    I: Iterator<Item = &'a str>,
{
    let token = iter.next().ok_or(INIError::InvalidData)?;
    let (key, value_opt) = parse_key_value(token);

    if key.eq_ignore_ascii_case("bone") {
        let bone_name = match value_opt {
            Some(value) => value,
            None => iter.next().ok_or(INIError::InvalidData)?,
        };
        loc_info.loc_type = FXDamageLocType::Bone;
        loc_info.bone_name = AsciiString::from(bone_name);

        let random_token = iter.next().ok_or(INIError::InvalidData)?;
        let (rand_key, rand_value) = parse_key_value(random_token);
        if !rand_key.eq_ignore_ascii_case("randombone") {
            return Err(INIError::InvalidData);
        }
        let rand_value = match rand_value {
            Some(value) => value,
            None => iter.next().ok_or(INIError::InvalidData)?,
        };
        loc_info.random_bone = INI::parse_bool(rand_value)?;
        return Ok(());
    }

    if key.eq_ignore_ascii_case("loc") {
        loc_info.loc_type = FXDamageLocType::Coord;

        let mut x = None;
        let mut y = None;
        let mut z = None;

        if let Some(rest) = value_opt {
            let _ = parse_coord_token(rest, iter, &mut x, &mut y, &mut z)?;
        }

        while let Some(token) = iter.peek().copied() {
            if !is_coord_token(token) {
                break;
            }
            let token = iter.next().ok_or(INIError::InvalidData)?;
            let _ = parse_coord_token(token, iter, &mut x, &mut y, &mut z)?;
        }

        let x = x.ok_or(INIError::InvalidData)?;
        let y = y.ok_or(INIError::InvalidData)?;
        let z = z.ok_or(INIError::InvalidData)?;
        loc_info.loc = Coord3D::new(x, y, z);
        return Ok(());
    }

    Err(INIError::InvalidData)
}

fn parse_tag_value<'a, I>(
    token: &'a str,
    iter: &mut std::iter::Peekable<I>,
    tag: &str,
) -> Result<String, INIError>
where
    I: Iterator<Item = &'a str>,
{
    let (key, value_opt) = parse_key_value(token);
    if !key.eq_ignore_ascii_case(tag) {
        return Err(INIError::InvalidData);
    }
    let value = match value_opt {
        Some(value) => value,
        None => iter.next().ok_or(INIError::InvalidData)?,
    };
    Ok(value.to_string())
}

fn parse_fx_list_entry(tokens: &[&str], info: &mut FXDamageFXListInfo) -> Result<(), INIError> {
    let mut iter = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .peekable();
    parse_fx_loc_info(&mut iter, &mut info.loc_info)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let value = parse_tag_value(token, &mut iter, "fxlist")?;
    if value.eq_ignore_ascii_case("none") {
        info.fx = None;
    } else {
        info.fx = TheFXListStore::lookup_fx_list(&value);
        if info.fx.is_none() {
            log::warn!("TransitionDamageFX: unresolved FXList '{}'", value);
        }
    }
    Ok(())
}

fn parse_ocl_entry(tokens: &[&str], info: &mut FXDamageOCLInfo) -> Result<(), INIError> {
    let mut iter = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .peekable();
    parse_fx_loc_info(&mut iter, &mut info.loc_info)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let value = parse_tag_value(token, &mut iter, "ocl")?;
    if value.eq_ignore_ascii_case("none") {
        info.ocl = None;
    } else {
        info.ocl = TheObjectCreationListStore::find_object_creation_list(&value);
        if info.ocl.is_none() {
            log::warn!("TransitionDamageFX: unresolved OCL '{}'", value);
        }
    }
    Ok(())
}

fn parse_particle_entry(
    tokens: &[&str],
    info: &mut FXDamageParticleSystemInfo,
) -> Result<(), INIError> {
    let mut iter = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .peekable();
    parse_fx_loc_info(&mut iter, &mut info.loc_info)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let value = parse_tag_value(token, &mut iter, "psys")?;
    if value.eq_ignore_ascii_case("none") {
        info.particle_system_name = None;
    } else {
        info.particle_system_name = Some(AsciiString::from(value.as_str()));
    }
    Ok(())
}

const STATE_DAMAGED: usize = BodyDamageType::Damaged as usize;
const STATE_REALLY_DAMAGED: usize = BodyDamageType::ReallyDamaged as usize;
const STATE_RUBBLE: usize = BodyDamageType::Rubble as usize;

macro_rules! fx_list_parser {
    ($name:ident, $state:expr, $index:expr) => {
        fn $name(
            _ini: &mut INI,
            data: &mut TransitionDamageFXModuleData,
            tokens: &[&str],
        ) -> Result<(), INIError> {
            parse_fx_list_entry(tokens, &mut data.fx_list[$state][$index])
        }
    };
}

macro_rules! ocl_parser {
    ($name:ident, $state:expr, $index:expr) => {
        fn $name(
            _ini: &mut INI,
            data: &mut TransitionDamageFXModuleData,
            tokens: &[&str],
        ) -> Result<(), INIError> {
            parse_ocl_entry(tokens, &mut data.ocl[$state][$index])
        }
    };
}

macro_rules! particle_parser {
    ($name:ident, $state:expr, $index:expr) => {
        fn $name(
            _ini: &mut INI,
            data: &mut TransitionDamageFXModuleData,
            tokens: &[&str],
        ) -> Result<(), INIError> {
            parse_particle_entry(tokens, &mut data.particle_system[$state][$index])
        }
    };
}

fx_list_parser!(parse_damaged_fx_list_1, STATE_DAMAGED, 0);
fx_list_parser!(parse_damaged_fx_list_2, STATE_DAMAGED, 1);
fx_list_parser!(parse_damaged_fx_list_3, STATE_DAMAGED, 2);
fx_list_parser!(parse_damaged_fx_list_4, STATE_DAMAGED, 3);
fx_list_parser!(parse_damaged_fx_list_5, STATE_DAMAGED, 4);
fx_list_parser!(parse_damaged_fx_list_6, STATE_DAMAGED, 5);
fx_list_parser!(parse_damaged_fx_list_7, STATE_DAMAGED, 6);
fx_list_parser!(parse_damaged_fx_list_8, STATE_DAMAGED, 7);
fx_list_parser!(parse_damaged_fx_list_9, STATE_DAMAGED, 8);
fx_list_parser!(parse_damaged_fx_list_10, STATE_DAMAGED, 9);
fx_list_parser!(parse_damaged_fx_list_11, STATE_DAMAGED, 10);
fx_list_parser!(parse_damaged_fx_list_12, STATE_DAMAGED, 11);
fx_list_parser!(parse_really_damaged_fx_list_1, STATE_REALLY_DAMAGED, 0);
fx_list_parser!(parse_really_damaged_fx_list_2, STATE_REALLY_DAMAGED, 1);
fx_list_parser!(parse_really_damaged_fx_list_3, STATE_REALLY_DAMAGED, 2);
fx_list_parser!(parse_really_damaged_fx_list_4, STATE_REALLY_DAMAGED, 3);
fx_list_parser!(parse_really_damaged_fx_list_5, STATE_REALLY_DAMAGED, 4);
fx_list_parser!(parse_really_damaged_fx_list_6, STATE_REALLY_DAMAGED, 5);
fx_list_parser!(parse_really_damaged_fx_list_7, STATE_REALLY_DAMAGED, 6);
fx_list_parser!(parse_really_damaged_fx_list_8, STATE_REALLY_DAMAGED, 7);
fx_list_parser!(parse_really_damaged_fx_list_9, STATE_REALLY_DAMAGED, 8);
fx_list_parser!(parse_really_damaged_fx_list_10, STATE_REALLY_DAMAGED, 9);
fx_list_parser!(parse_really_damaged_fx_list_11, STATE_REALLY_DAMAGED, 10);
fx_list_parser!(parse_really_damaged_fx_list_12, STATE_REALLY_DAMAGED, 11);
fx_list_parser!(parse_rubble_fx_list_1, STATE_RUBBLE, 0);
fx_list_parser!(parse_rubble_fx_list_2, STATE_RUBBLE, 1);
fx_list_parser!(parse_rubble_fx_list_3, STATE_RUBBLE, 2);
fx_list_parser!(parse_rubble_fx_list_4, STATE_RUBBLE, 3);
fx_list_parser!(parse_rubble_fx_list_5, STATE_RUBBLE, 4);
fx_list_parser!(parse_rubble_fx_list_6, STATE_RUBBLE, 5);
fx_list_parser!(parse_rubble_fx_list_7, STATE_RUBBLE, 6);
fx_list_parser!(parse_rubble_fx_list_8, STATE_RUBBLE, 7);
fx_list_parser!(parse_rubble_fx_list_9, STATE_RUBBLE, 8);
fx_list_parser!(parse_rubble_fx_list_10, STATE_RUBBLE, 9);
fx_list_parser!(parse_rubble_fx_list_11, STATE_RUBBLE, 10);
fx_list_parser!(parse_rubble_fx_list_12, STATE_RUBBLE, 11);

ocl_parser!(parse_damaged_ocl_1, STATE_DAMAGED, 0);
ocl_parser!(parse_damaged_ocl_2, STATE_DAMAGED, 1);
ocl_parser!(parse_damaged_ocl_3, STATE_DAMAGED, 2);
ocl_parser!(parse_damaged_ocl_4, STATE_DAMAGED, 3);
ocl_parser!(parse_damaged_ocl_5, STATE_DAMAGED, 4);
ocl_parser!(parse_damaged_ocl_6, STATE_DAMAGED, 5);
ocl_parser!(parse_damaged_ocl_7, STATE_DAMAGED, 6);
ocl_parser!(parse_damaged_ocl_8, STATE_DAMAGED, 7);
ocl_parser!(parse_damaged_ocl_9, STATE_DAMAGED, 8);
ocl_parser!(parse_damaged_ocl_10, STATE_DAMAGED, 9);
ocl_parser!(parse_damaged_ocl_11, STATE_DAMAGED, 10);
ocl_parser!(parse_damaged_ocl_12, STATE_DAMAGED, 11);
ocl_parser!(parse_really_damaged_ocl_1, STATE_REALLY_DAMAGED, 0);
ocl_parser!(parse_really_damaged_ocl_2, STATE_REALLY_DAMAGED, 1);
ocl_parser!(parse_really_damaged_ocl_3, STATE_REALLY_DAMAGED, 2);
ocl_parser!(parse_really_damaged_ocl_4, STATE_REALLY_DAMAGED, 3);
ocl_parser!(parse_really_damaged_ocl_5, STATE_REALLY_DAMAGED, 4);
ocl_parser!(parse_really_damaged_ocl_6, STATE_REALLY_DAMAGED, 5);
ocl_parser!(parse_really_damaged_ocl_7, STATE_REALLY_DAMAGED, 6);
ocl_parser!(parse_really_damaged_ocl_8, STATE_REALLY_DAMAGED, 7);
ocl_parser!(parse_really_damaged_ocl_9, STATE_REALLY_DAMAGED, 8);
ocl_parser!(parse_really_damaged_ocl_10, STATE_REALLY_DAMAGED, 9);
ocl_parser!(parse_really_damaged_ocl_11, STATE_REALLY_DAMAGED, 10);
ocl_parser!(parse_really_damaged_ocl_12, STATE_REALLY_DAMAGED, 11);
ocl_parser!(parse_rubble_ocl_1, STATE_RUBBLE, 0);
ocl_parser!(parse_rubble_ocl_2, STATE_RUBBLE, 1);
ocl_parser!(parse_rubble_ocl_3, STATE_RUBBLE, 2);
ocl_parser!(parse_rubble_ocl_4, STATE_RUBBLE, 3);
ocl_parser!(parse_rubble_ocl_5, STATE_RUBBLE, 4);
ocl_parser!(parse_rubble_ocl_6, STATE_RUBBLE, 5);
ocl_parser!(parse_rubble_ocl_7, STATE_RUBBLE, 6);
ocl_parser!(parse_rubble_ocl_8, STATE_RUBBLE, 7);
ocl_parser!(parse_rubble_ocl_9, STATE_RUBBLE, 8);
ocl_parser!(parse_rubble_ocl_10, STATE_RUBBLE, 9);
ocl_parser!(parse_rubble_ocl_11, STATE_RUBBLE, 10);
ocl_parser!(parse_rubble_ocl_12, STATE_RUBBLE, 11);

particle_parser!(parse_damaged_particle_1, STATE_DAMAGED, 0);
particle_parser!(parse_damaged_particle_2, STATE_DAMAGED, 1);
particle_parser!(parse_damaged_particle_3, STATE_DAMAGED, 2);
particle_parser!(parse_damaged_particle_4, STATE_DAMAGED, 3);
particle_parser!(parse_damaged_particle_5, STATE_DAMAGED, 4);
particle_parser!(parse_damaged_particle_6, STATE_DAMAGED, 5);
particle_parser!(parse_damaged_particle_7, STATE_DAMAGED, 6);
particle_parser!(parse_damaged_particle_8, STATE_DAMAGED, 7);
particle_parser!(parse_damaged_particle_9, STATE_DAMAGED, 8);
particle_parser!(parse_damaged_particle_10, STATE_DAMAGED, 9);
particle_parser!(parse_damaged_particle_11, STATE_DAMAGED, 10);
particle_parser!(parse_damaged_particle_12, STATE_DAMAGED, 11);
particle_parser!(parse_really_damaged_particle_1, STATE_REALLY_DAMAGED, 0);
particle_parser!(parse_really_damaged_particle_2, STATE_REALLY_DAMAGED, 1);
particle_parser!(parse_really_damaged_particle_3, STATE_REALLY_DAMAGED, 2);
particle_parser!(parse_really_damaged_particle_4, STATE_REALLY_DAMAGED, 3);
particle_parser!(parse_really_damaged_particle_5, STATE_REALLY_DAMAGED, 4);
particle_parser!(parse_really_damaged_particle_6, STATE_REALLY_DAMAGED, 5);
particle_parser!(parse_really_damaged_particle_7, STATE_REALLY_DAMAGED, 6);
particle_parser!(parse_really_damaged_particle_8, STATE_REALLY_DAMAGED, 7);
particle_parser!(parse_really_damaged_particle_9, STATE_REALLY_DAMAGED, 8);
particle_parser!(parse_really_damaged_particle_10, STATE_REALLY_DAMAGED, 9);
particle_parser!(parse_really_damaged_particle_11, STATE_REALLY_DAMAGED, 10);
particle_parser!(parse_really_damaged_particle_12, STATE_REALLY_DAMAGED, 11);
particle_parser!(parse_rubble_particle_1, STATE_RUBBLE, 0);
particle_parser!(parse_rubble_particle_2, STATE_RUBBLE, 1);
particle_parser!(parse_rubble_particle_3, STATE_RUBBLE, 2);
particle_parser!(parse_rubble_particle_4, STATE_RUBBLE, 3);
particle_parser!(parse_rubble_particle_5, STATE_RUBBLE, 4);
particle_parser!(parse_rubble_particle_6, STATE_RUBBLE, 5);
particle_parser!(parse_rubble_particle_7, STATE_RUBBLE, 6);
particle_parser!(parse_rubble_particle_8, STATE_RUBBLE, 7);
particle_parser!(parse_rubble_particle_9, STATE_RUBBLE, 8);
particle_parser!(parse_rubble_particle_10, STATE_RUBBLE, 9);
particle_parser!(parse_rubble_particle_11, STATE_RUBBLE, 10);
particle_parser!(parse_rubble_particle_12, STATE_RUBBLE, 11);

const TRANSITION_DAMAGE_FX_FIELDS: &[FieldParse<TransitionDamageFXModuleData>] = &[
    FieldParse {
        token: "DamageFXTypes",
        parse: parse_damage_fx_types,
    },
    FieldParse {
        token: "DamageOCLTypes",
        parse: parse_damage_ocl_types,
    },
    FieldParse {
        token: "DamageParticleTypes",
        parse: parse_damage_particle_types,
    },
    FieldParse {
        token: "DamagedFXList1",
        parse: parse_damaged_fx_list_1,
    },
    FieldParse {
        token: "DamagedFXList2",
        parse: parse_damaged_fx_list_2,
    },
    FieldParse {
        token: "DamagedFXList3",
        parse: parse_damaged_fx_list_3,
    },
    FieldParse {
        token: "DamagedFXList4",
        parse: parse_damaged_fx_list_4,
    },
    FieldParse {
        token: "DamagedFXList5",
        parse: parse_damaged_fx_list_5,
    },
    FieldParse {
        token: "DamagedFXList6",
        parse: parse_damaged_fx_list_6,
    },
    FieldParse {
        token: "DamagedFXList7",
        parse: parse_damaged_fx_list_7,
    },
    FieldParse {
        token: "DamagedFXList8",
        parse: parse_damaged_fx_list_8,
    },
    FieldParse {
        token: "DamagedFXList9",
        parse: parse_damaged_fx_list_9,
    },
    FieldParse {
        token: "DamagedFXList10",
        parse: parse_damaged_fx_list_10,
    },
    FieldParse {
        token: "DamagedFXList11",
        parse: parse_damaged_fx_list_11,
    },
    FieldParse {
        token: "DamagedFXList12",
        parse: parse_damaged_fx_list_12,
    },
    FieldParse {
        token: "ReallyDamagedFXList1",
        parse: parse_really_damaged_fx_list_1,
    },
    FieldParse {
        token: "ReallyDamagedFXList2",
        parse: parse_really_damaged_fx_list_2,
    },
    FieldParse {
        token: "ReallyDamagedFXList3",
        parse: parse_really_damaged_fx_list_3,
    },
    FieldParse {
        token: "ReallyDamagedFXList4",
        parse: parse_really_damaged_fx_list_4,
    },
    FieldParse {
        token: "ReallyDamagedFXList5",
        parse: parse_really_damaged_fx_list_5,
    },
    FieldParse {
        token: "ReallyDamagedFXList6",
        parse: parse_really_damaged_fx_list_6,
    },
    FieldParse {
        token: "ReallyDamagedFXList7",
        parse: parse_really_damaged_fx_list_7,
    },
    FieldParse {
        token: "ReallyDamagedFXList8",
        parse: parse_really_damaged_fx_list_8,
    },
    FieldParse {
        token: "ReallyDamagedFXList9",
        parse: parse_really_damaged_fx_list_9,
    },
    FieldParse {
        token: "ReallyDamagedFXList10",
        parse: parse_really_damaged_fx_list_10,
    },
    FieldParse {
        token: "ReallyDamagedFXList11",
        parse: parse_really_damaged_fx_list_11,
    },
    FieldParse {
        token: "ReallyDamagedFXList12",
        parse: parse_really_damaged_fx_list_12,
    },
    FieldParse {
        token: "RubbleFXList1",
        parse: parse_rubble_fx_list_1,
    },
    FieldParse {
        token: "RubbleFXList2",
        parse: parse_rubble_fx_list_2,
    },
    FieldParse {
        token: "RubbleFXList3",
        parse: parse_rubble_fx_list_3,
    },
    FieldParse {
        token: "RubbleFXList4",
        parse: parse_rubble_fx_list_4,
    },
    FieldParse {
        token: "RubbleFXList5",
        parse: parse_rubble_fx_list_5,
    },
    FieldParse {
        token: "RubbleFXList6",
        parse: parse_rubble_fx_list_6,
    },
    FieldParse {
        token: "RubbleFXList7",
        parse: parse_rubble_fx_list_7,
    },
    FieldParse {
        token: "RubbleFXList8",
        parse: parse_rubble_fx_list_8,
    },
    FieldParse {
        token: "RubbleFXList9",
        parse: parse_rubble_fx_list_9,
    },
    FieldParse {
        token: "RubbleFXList10",
        parse: parse_rubble_fx_list_10,
    },
    FieldParse {
        token: "RubbleFXList11",
        parse: parse_rubble_fx_list_11,
    },
    FieldParse {
        token: "RubbleFXList12",
        parse: parse_rubble_fx_list_12,
    },
    FieldParse {
        token: "DamagedOCL1",
        parse: parse_damaged_ocl_1,
    },
    FieldParse {
        token: "DamagedOCL2",
        parse: parse_damaged_ocl_2,
    },
    FieldParse {
        token: "DamagedOCL3",
        parse: parse_damaged_ocl_3,
    },
    FieldParse {
        token: "DamagedOCL4",
        parse: parse_damaged_ocl_4,
    },
    FieldParse {
        token: "DamagedOCL5",
        parse: parse_damaged_ocl_5,
    },
    FieldParse {
        token: "DamagedOCL6",
        parse: parse_damaged_ocl_6,
    },
    FieldParse {
        token: "DamagedOCL7",
        parse: parse_damaged_ocl_7,
    },
    FieldParse {
        token: "DamagedOCL8",
        parse: parse_damaged_ocl_8,
    },
    FieldParse {
        token: "DamagedOCL9",
        parse: parse_damaged_ocl_9,
    },
    FieldParse {
        token: "DamagedOCL10",
        parse: parse_damaged_ocl_10,
    },
    FieldParse {
        token: "DamagedOCL11",
        parse: parse_damaged_ocl_11,
    },
    FieldParse {
        token: "DamagedOCL12",
        parse: parse_damaged_ocl_12,
    },
    FieldParse {
        token: "ReallyDamagedOCL1",
        parse: parse_really_damaged_ocl_1,
    },
    FieldParse {
        token: "ReallyDamagedOCL2",
        parse: parse_really_damaged_ocl_2,
    },
    FieldParse {
        token: "ReallyDamagedOCL3",
        parse: parse_really_damaged_ocl_3,
    },
    FieldParse {
        token: "ReallyDamagedOCL4",
        parse: parse_really_damaged_ocl_4,
    },
    FieldParse {
        token: "ReallyDamagedOCL5",
        parse: parse_really_damaged_ocl_5,
    },
    FieldParse {
        token: "ReallyDamagedOCL6",
        parse: parse_really_damaged_ocl_6,
    },
    FieldParse {
        token: "ReallyDamagedOCL7",
        parse: parse_really_damaged_ocl_7,
    },
    FieldParse {
        token: "ReallyDamagedOCL8",
        parse: parse_really_damaged_ocl_8,
    },
    FieldParse {
        token: "ReallyDamagedOCL9",
        parse: parse_really_damaged_ocl_9,
    },
    FieldParse {
        token: "ReallyDamagedOCL10",
        parse: parse_really_damaged_ocl_10,
    },
    FieldParse {
        token: "ReallyDamagedOCL11",
        parse: parse_really_damaged_ocl_11,
    },
    FieldParse {
        token: "ReallyDamagedOCL12",
        parse: parse_really_damaged_ocl_12,
    },
    FieldParse {
        token: "RubbleOCL1",
        parse: parse_rubble_ocl_1,
    },
    FieldParse {
        token: "RubbleOCL2",
        parse: parse_rubble_ocl_2,
    },
    FieldParse {
        token: "RubbleOCL3",
        parse: parse_rubble_ocl_3,
    },
    FieldParse {
        token: "RubbleOCL4",
        parse: parse_rubble_ocl_4,
    },
    FieldParse {
        token: "RubbleOCL5",
        parse: parse_rubble_ocl_5,
    },
    FieldParse {
        token: "RubbleOCL6",
        parse: parse_rubble_ocl_6,
    },
    FieldParse {
        token: "RubbleOCL7",
        parse: parse_rubble_ocl_7,
    },
    FieldParse {
        token: "RubbleOCL8",
        parse: parse_rubble_ocl_8,
    },
    FieldParse {
        token: "RubbleOCL9",
        parse: parse_rubble_ocl_9,
    },
    FieldParse {
        token: "RubbleOCL10",
        parse: parse_rubble_ocl_10,
    },
    FieldParse {
        token: "RubbleOCL11",
        parse: parse_rubble_ocl_11,
    },
    FieldParse {
        token: "RubbleOCL12",
        parse: parse_rubble_ocl_12,
    },
    FieldParse {
        token: "DamagedParticleSystem1",
        parse: parse_damaged_particle_1,
    },
    FieldParse {
        token: "DamagedParticleSystem2",
        parse: parse_damaged_particle_2,
    },
    FieldParse {
        token: "DamagedParticleSystem3",
        parse: parse_damaged_particle_3,
    },
    FieldParse {
        token: "DamagedParticleSystem4",
        parse: parse_damaged_particle_4,
    },
    FieldParse {
        token: "DamagedParticleSystem5",
        parse: parse_damaged_particle_5,
    },
    FieldParse {
        token: "DamagedParticleSystem6",
        parse: parse_damaged_particle_6,
    },
    FieldParse {
        token: "DamagedParticleSystem7",
        parse: parse_damaged_particle_7,
    },
    FieldParse {
        token: "DamagedParticleSystem8",
        parse: parse_damaged_particle_8,
    },
    FieldParse {
        token: "DamagedParticleSystem9",
        parse: parse_damaged_particle_9,
    },
    FieldParse {
        token: "DamagedParticleSystem10",
        parse: parse_damaged_particle_10,
    },
    FieldParse {
        token: "DamagedParticleSystem11",
        parse: parse_damaged_particle_11,
    },
    FieldParse {
        token: "DamagedParticleSystem12",
        parse: parse_damaged_particle_12,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem1",
        parse: parse_really_damaged_particle_1,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem2",
        parse: parse_really_damaged_particle_2,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem3",
        parse: parse_really_damaged_particle_3,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem4",
        parse: parse_really_damaged_particle_4,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem5",
        parse: parse_really_damaged_particle_5,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem6",
        parse: parse_really_damaged_particle_6,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem7",
        parse: parse_really_damaged_particle_7,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem8",
        parse: parse_really_damaged_particle_8,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem9",
        parse: parse_really_damaged_particle_9,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem10",
        parse: parse_really_damaged_particle_10,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem11",
        parse: parse_really_damaged_particle_11,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem12",
        parse: parse_really_damaged_particle_12,
    },
    FieldParse {
        token: "RubbleParticleSystem1",
        parse: parse_rubble_particle_1,
    },
    FieldParse {
        token: "RubbleParticleSystem2",
        parse: parse_rubble_particle_2,
    },
    FieldParse {
        token: "RubbleParticleSystem3",
        parse: parse_rubble_particle_3,
    },
    FieldParse {
        token: "RubbleParticleSystem4",
        parse: parse_rubble_particle_4,
    },
    FieldParse {
        token: "RubbleParticleSystem5",
        parse: parse_rubble_particle_5,
    },
    FieldParse {
        token: "RubbleParticleSystem6",
        parse: parse_rubble_particle_6,
    },
    FieldParse {
        token: "RubbleParticleSystem7",
        parse: parse_rubble_particle_7,
    },
    FieldParse {
        token: "RubbleParticleSystem8",
        parse: parse_rubble_particle_8,
    },
    FieldParse {
        token: "RubbleParticleSystem9",
        parse: parse_rubble_particle_9,
    },
    FieldParse {
        token: "RubbleParticleSystem10",
        parse: parse_rubble_particle_10,
    },
    FieldParse {
        token: "RubbleParticleSystem11",
        parse: parse_rubble_particle_11,
    },
    FieldParse {
        token: "RubbleParticleSystem12",
        parse: parse_rubble_particle_12,
    },
];

#[derive(Debug)]
pub struct TransitionDamageFX {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<TransitionDamageFXModuleData>,
    particle_system_ids:
        [[crate::common::ParticleSystemID; DAMAGE_MODULE_MAX_FX]; BODY_DAMAGE_TYPE_COUNT],
}

impl TransitionDamageFX {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<TransitionDamageFXModuleData>,
    ) -> Self {
        Self {
            object: Arc::downgrade(&object),
            module_data,
            particle_system_ids: [[INVALID_PARTICLE_SYSTEM_ID; DAMAGE_MODULE_MAX_FX];
                BODY_DAMAGE_TYPE_COUNT],
        }
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<TransitionDamageFXModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "TransitionDamageFX requires an owning object".to_string())?;
        let object_id = module_object.get_object_id();
        let object = OBJECT_REGISTRY
            .get_object(object_id)
            .ok_or_else(|| format!("TransitionDamageFX requires object {object_id}"))?;
        Ok(Self::new(object, module_data))
    }

    fn get_local_effect_pos(loc_info: &FXLocInfo, drawable: Option<&Drawable>) -> Coord3D {
        if loc_info.loc_type == FXDamageLocType::Bone {
            if let Some(drawable) = drawable {
                if !loc_info.random_bone {
                    let positions =
                        drawable.get_pristine_bone_positions(loc_info.bone_name.as_str(), 0, 1);
                    if let Some(pos) = positions.first() {
                        return *pos;
                    }
                    return loc_info.loc;
                }

                const MAX_BONES: usize = 32;
                let positions =
                    drawable.get_pristine_bone_positions(loc_info.bone_name.as_str(), 1, MAX_BONES);
                if positions.is_empty() {
                    return loc_info.loc;
                }
                let pick = game_logic_random_value(0, positions.len() as u32 - 1) as usize;
                return positions[pick];
            }
        }

        loc_info.loc
    }

    fn resolve_damage_source_pos(damage_info: &DamageInfo) -> Option<Coord3D> {
        if damage_info.input.source_id == crate::common::INVALID_ID {
            return None;
        }
        TheGameLogic::find_object_by_id(damage_info.input.source_id)
            .and_then(|source| source.read().ok().map(|guard| *guard.get_position()))
    }

    fn should_play_for_damage_type(mask: DamageTypeFlags, last_damage: Option<DamageInfo>) -> bool {
        match last_damage {
            Some(info) => get_damage_type_flag(mask, info.input.damage_type),
            None => true,
        }
    }

    fn clear_particle_systems_for_state(&mut self, state: BodyDamageType) {
        let state_index = state as usize;
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            for id in &mut self.particle_system_ids[state_index] {
                if *id != INVALID_PARTICLE_SYSTEM_ID {
                    ps_manager.destroy_particle_system(*id);
                    *id = INVALID_PARTICLE_SYSTEM_ID;
                }
            }
        } else {
            for id in &mut self.particle_system_ids[state_index] {
                *id = INVALID_PARTICLE_SYSTEM_ID;
            }
        }
    }

    fn play_fx_for_state(
        &mut self,
        object: &GameObject,
        drawable: Option<&Drawable>,
        damage_source_pos: Option<Coord3D>,
        last_damage: Option<DamageInfo>,
        new_state: BodyDamageType,
    ) {
        let state_index = new_state as usize;
        for i in 0..DAMAGE_MODULE_MAX_FX {
            if let Some(fx) = &self.module_data.fx_list[state_index][i].fx {
                if Self::should_play_for_damage_type(
                    self.module_data.damage_fx_types,
                    last_damage.clone(),
                ) {
                    let mut pos = Self::get_local_effect_pos(
                        &self.module_data.fx_list[state_index][i].loc_info,
                        drawable,
                    );
                    let world = object.convert_bone_pos_to_world_pos(Some(&pos), None);
                    let translation = world.w_axis;
                    pos = Coord3D::new(translation.x, translation.y, translation.z);
                    let _ = fx.do_fx_at_position(&pos);
                }
            }

            if let Some(ocl) = &self.module_data.ocl[state_index][i].ocl {
                if Self::should_play_for_damage_type(
                    self.module_data.damage_ocl_types,
                    last_damage.clone(),
                ) {
                    let mut pos = Self::get_local_effect_pos(
                        &self.module_data.ocl[state_index][i].loc_info,
                        drawable,
                    );
                    let world = object.convert_bone_pos_to_world_pos(Some(&pos), None);
                    let translation = world.w_axis;
                    pos = Coord3D::new(translation.x, translation.y, translation.z);
                    let secondary = damage_source_pos.unwrap_or(pos);
                    let ctx = live_creation_context();
                    let _ = ocl.create_with_angle(
                        &ctx,
                        Some(object),
                        &pos,
                        &secondary,
                        INVALID_ANGLE,
                        0,
                    );
                }
            }

            if let Some(template_name) =
                &self.module_data.particle_system[state_index][i].particle_system_name
            {
                if Self::should_play_for_damage_type(
                    self.module_data.damage_particle_types,
                    last_damage.clone(),
                ) {
                    if let Some(ps_manager) = TheParticleSystemManager::get() {
                        if let Some(system_id) =
                            ps_manager.create_particle_system(Some(template_name.as_str()))
                        {
                            let pos = Self::get_local_effect_pos(
                                &self.module_data.particle_system[state_index][i].loc_info,
                                drawable,
                            );
                            ps_manager.set_particle_system_position(system_id, &pos);
                            ps_manager.attach_particle_system_to_object(system_id, object.get_id());
                            self.particle_system_ids[state_index][i] = system_id;
                        }
                    }
                }
            }
        }
    }

    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u32 = 1;
        xfer.xfer_version_write(current_version);
        for state in 0..BODY_DAMAGE_TYPE_COUNT {
            for slot in 0..DAMAGE_MODULE_MAX_FX {
                let mut id = self.particle_system_ids[state][slot];
                let _ = game_engine::system::Xfer::xfer_unsigned_int(xfer, &mut id);
            }
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: u32 = 1;
        if xfer.is_loading() {
            let version = xfer.xfer_version_read();
            if version > current_version {
                return Err(format!(
                    "TransitionDamageFX version {} > current version {}",
                    version, current_version
                ));
            }
        } else {
            xfer.xfer_version_write(current_version);
        }

        for state in 0..BODY_DAMAGE_TYPE_COUNT {
            for slot in 0..DAMAGE_MODULE_MAX_FX {
                let mut id = self.particle_system_ids[state][slot];
                let _ = game_engine::system::Xfer::xfer_unsigned_int(xfer, &mut id);
                self.particle_system_ids[state][slot] = id;
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

impl DamageModuleInterface for TransitionDamageFX {
    fn receive_damage(
        &mut self,
        _object_id: crate::common::ObjectID,
        _damage: &DamageInfo,
    ) -> Real {
        0.0
    }

    fn on_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn on_healing(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn on_body_damage_state_change(
        &mut self,
        damage_info: &DamageInfo,
        old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(object_arc) = self.object.upgrade() else {
            return Ok(());
        };
        let Ok(object_guard) = object_arc.read() else {
            return Ok(());
        };

        self.clear_particle_systems_for_state(old_state);

        if !is_condition_worse(new_state, old_state) {
            return Ok(());
        }

        let last_damage = object_guard
            .get_body_module()
            .and_then(|body| body.get_last_damage_info());

        let damage_source_pos = Self::resolve_damage_source_pos(damage_info);

        let drawable = object_guard.get_drawable();
        let drawable_guard = drawable.as_ref().and_then(|d| d.read().ok());

        self.play_fx_for_state(
            &object_guard,
            drawable_guard.as_deref(),
            damage_source_pos,
            last_damage,
            new_state,
        );

        Ok(())
    }
}

impl BehaviorModuleInterface for TransitionDamageFX {
    fn get_module_name(&self) -> &str {
        "TransitionDamageFX"
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }
}

/// Glue module implementing engine Module for TransitionDamageFX.
pub struct TransitionDamageFXModule {
    behavior: TransitionDamageFX,
    module_name_key: NameKeyType,
    module_data: Arc<TransitionDamageFXModuleData>,
}

impl TransitionDamageFXModule {
    pub fn new(
        behavior: TransitionDamageFX,
        module_name: &AsciiString,
        module_data: Arc<TransitionDamageFXModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut TransitionDamageFX {
        &mut self.behavior
    }
}

impl Snapshotable for TransitionDamageFXModule {
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

impl Module for TransitionDamageFXModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> EngineNameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> EngineNameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        let _ = self.behavior.on_object_created();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fx_entry_keeps_missing_reference_none() {
        let mut info = FXDamageFXListInfo::default();
        parse_fx_list_entry(
            &[
                "Loc",
                "X:0",
                "Y:0",
                "Z:0",
                "FXList:MissingTransitionFx_ParityTest_20260302",
            ],
            &mut info,
        )
        .expect("parse should succeed");
        assert!(info.fx.is_none());
    }
}
