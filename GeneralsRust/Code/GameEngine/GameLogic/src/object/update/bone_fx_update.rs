// BoneFXUpdate - Bone-attached effects for damage states
// Ported to Rust from C++ BoneFXUpdate.cpp

use std::str::FromStr;
use std::sync::Arc;

use crate::common::xfer::XferExt;
use crate::common::{name_key_generate, AsciiString};
use crate::damage::{DamageType, DamageTypeFlags};
use crate::helpers::{
    get_fx_list_manager, TheGameLogic, TheObjectCreationListStore, TheParticleSystemManager,
};
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_creation_list::{live_creation_context, nuggets::INVALID_ANGLE};
use crate::prelude::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{BoneFxControlInterface, Module, ModuleData, NameKeyType};

/// Maximum number of bones that can have FX attached (matches C++ BONE_FX_MAX_BONES = 8).
pub const BONE_FX_MAX_BONES: usize = 8;

/// Number of body damage types.
pub const BODY_DAMAGE_TYPE_COUNT: usize = 4;

const STATE_PRISTINE: usize = BodyDamageType::Pristine as usize;
const STATE_DAMAGED: usize = BodyDamageType::Damaged as usize;
const STATE_REALLY_DAMAGED: usize = BodyDamageType::ReallyDamaged as usize;
const STATE_RUBBLE: usize = BodyDamageType::Rubble as usize;

fn body_damage_type_from_index(index: u32) -> Option<BodyDamageType> {
    match index {
        0 => Some(BodyDamageType::Pristine),
        1 => Some(BodyDamageType::Damaged),
        2 => Some(BodyDamageType::ReallyDamaged),
        3 => Some(BodyDamageType::Rubble),
        _ => None,
    }
}

/// Location information for a bone.
#[derive(Debug, Clone)]
pub struct BoneLocInfo {
    pub bone_name: String,
}

impl Default for BoneLocInfo {
    fn default() -> Self {
        Self {
            bone_name: String::new(),
        }
    }
}

/// Base bone list information.
#[derive(Debug, Clone)]
pub struct BaseBoneListInfo {
    pub loc_info: BoneLocInfo,
    pub only_once: bool,
    pub game_logic_delay: RandomVariable,
    pub game_client_delay: RandomVariable,
}

impl Default for BaseBoneListInfo {
    fn default() -> Self {
        Self {
            loc_info: BoneLocInfo::default(),
            only_once: true,
            game_logic_delay: RandomVariable::new(0.0, 0.0),
            game_client_delay: RandomVariable::new(0.0, 0.0),
        }
    }
}

/// FX list information for a bone.
#[derive(Debug, Clone)]
pub struct BoneFXListInfo {
    pub base: BaseBoneListInfo,
    pub fx: Option<FXListId>,
}

impl Default for BoneFXListInfo {
    fn default() -> Self {
        Self {
            base: BaseBoneListInfo::default(),
            fx: None,
        }
    }
}

/// Object creation list information for a bone.
#[derive(Debug, Clone)]
pub struct BoneOCLInfo {
    pub base: BaseBoneListInfo,
    pub ocl: Option<ObjectCreationListId>,
}

impl Default for BoneOCLInfo {
    fn default() -> Self {
        Self {
            base: BaseBoneListInfo::default(),
            ocl: None,
        }
    }
}

/// Particle system information for a bone.
#[derive(Debug, Clone)]
pub struct BoneParticleSystemInfo {
    pub base: BaseBoneListInfo,
    pub particle_sys_template: Option<ParticleSystemTemplateId>,
}

impl Default for BoneParticleSystemInfo {
    fn default() -> Self {
        Self {
            base: BaseBoneListInfo::default(),
            particle_sys_template: None,
        }
    }
}

/// Module data for BoneFXUpdate.
/// Matches C++ BoneFXUpdate.cpp:28-49
#[derive(Debug, Clone)]
pub struct BoneFXUpdateModuleData {
    pub module_tag_name_key: NameKeyType,
    /// FX lists for each damage state and bone.
    pub fx_list: [[BoneFXListInfo; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Object creation lists for each damage state and bone.
    pub ocl: [[BoneOCLInfo; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Particle systems for each damage state and bone.
    pub particle_system: [[BoneParticleSystemInfo; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Damage types that trigger FX.
    pub damage_fx_types: DamageTypeFlags,
    /// Damage types that trigger OCL.
    pub damage_ocl_types: DamageTypeFlags,
    /// Damage types that trigger particle systems.
    pub damage_particle_types: DamageTypeFlags,
}

impl Default for BoneFXUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            fx_list: Default::default(),
            ocl: Default::default(),
            particle_system: Default::default(),
            damage_fx_types: DamageTypeFlags::all_flags(),
            damage_ocl_types: DamageTypeFlags::all_flags(),
            damage_particle_types: DamageTypeFlags::all_flags(),
        }
    }
}

impl BoneFXUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, BONE_FX_UPDATE_FIELDS)
    }
}

impl Snapshotable for BoneFXUpdateModuleData {
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

crate::impl_legacy_module_data_with_key_field!(BoneFXUpdateModuleData, module_tag_name_key);

fn parse_key_value(token: &str) -> (&str, Option<&str>) {
    let mut parts = token.splitn(2, ':');
    let key = parts.next().unwrap_or("");
    let value = parts.next().filter(|value| !value.is_empty());
    (key, value)
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

fn parse_bone_loc_info<'a, I>(
    iter: &mut std::iter::Peekable<I>,
    loc_info: &mut BoneLocInfo,
) -> Result<(), INIError>
where
    I: Iterator<Item = &'a str>,
{
    let token = iter.next().ok_or(INIError::InvalidData)?;
    let bone_name = parse_tag_value(token, iter, "bone")?;
    loc_info.bone_name = bone_name;
    Ok(())
}

fn parse_random_delay<'a, I>(iter: &mut I) -> Result<RandomVariable, INIError>
where
    I: Iterator<Item = &'a str>,
{
    let min_token = iter.next().ok_or(INIError::InvalidData)?;
    let max_token = iter.next().ok_or(INIError::InvalidData)?;
    let min = INI::parse_duration_real(min_token)?;
    let max = INI::parse_duration_real(max_token)?;
    Ok(RandomVariable::new(min, max))
}

fn parse_fx_list_entry(tokens: &[&str], info: &mut BoneFXListInfo) -> Result<(), INIError> {
    let mut iter = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .peekable();
    parse_bone_loc_info(&mut iter, &mut info.base.loc_info)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let only_once = parse_tag_value(token, &mut iter, "onlyonce")?;
    info.base.only_once = INI::parse_bool(&only_once)?;

    info.base.game_logic_delay = parse_random_delay(&mut iter)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let value = parse_tag_value(token, &mut iter, "fxlist")?;
    if value.eq_ignore_ascii_case("none") {
        info.fx = None;
    } else {
        if let Some(fx) = crate::helpers::TheFXListStore::lookup_fx_list(&value) {
            info.fx = Some(fx.id());
        } else {
            log::warn!("BoneFXUpdate: unresolved FXList '{}'", value);
            info.fx = None;
        }
    }
    Ok(())
}

fn parse_ocl_entry(tokens: &[&str], info: &mut BoneOCLInfo) -> Result<(), INIError> {
    let mut iter = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .peekable();
    parse_bone_loc_info(&mut iter, &mut info.base.loc_info)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let only_once = parse_tag_value(token, &mut iter, "onlyonce")?;
    info.base.only_once = INI::parse_bool(&only_once)?;

    info.base.game_logic_delay = parse_random_delay(&mut iter)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let value = parse_tag_value(token, &mut iter, "ocl")?;
    if value.eq_ignore_ascii_case("none") {
        info.ocl = None;
    } else {
        if TheObjectCreationListStore::find_object_creation_list(&value).is_some() {
            info.ocl = Some(name_key_generate(&value) as ObjectCreationListId);
        } else {
            log::warn!("BoneFXUpdate: unresolved OCL '{}'", value);
            info.ocl = None;
        }
    }
    Ok(())
}

fn parse_particle_entry(
    tokens: &[&str],
    info: &mut BoneParticleSystemInfo,
) -> Result<(), INIError> {
    let mut iter = tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .peekable();
    parse_bone_loc_info(&mut iter, &mut info.base.loc_info)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let only_once = parse_tag_value(token, &mut iter, "onlyonce")?;
    info.base.only_once = INI::parse_bool(&only_once)?;

    info.base.game_client_delay = parse_random_delay(&mut iter)?;

    let token = iter.next().ok_or(INIError::InvalidData)?;
    let value = parse_tag_value(token, &mut iter, "psys")?;
    if value.eq_ignore_ascii_case("none") {
        info.particle_sys_template = None;
    } else {
        info.particle_sys_template = Some(name_key_generate(&value) as ParticleSystemTemplateId);
    }
    Ok(())
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
    data: &mut BoneFXUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_fx_types = parse_damage_type_flags(tokens)?;
    Ok(())
}

fn parse_damage_ocl_types(
    _ini: &mut INI,
    data: &mut BoneFXUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_ocl_types = parse_damage_type_flags(tokens)?;
    Ok(())
}

fn parse_damage_particle_types(
    _ini: &mut INI,
    data: &mut BoneFXUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_particle_types = parse_damage_type_flags(tokens)?;
    Ok(())
}

macro_rules! fx_list_parser {
    ($fn_name:ident, $state:expr, $idx:expr) => {
        fn $fn_name(
            _ini: &mut INI,
            data: &mut BoneFXUpdateModuleData,
            tokens: &[&str],
        ) -> Result<(), INIError> {
            parse_fx_list_entry(tokens, &mut data.fx_list[$state][$idx])
        }
    };
}

macro_rules! ocl_parser {
    ($fn_name:ident, $state:expr, $idx:expr) => {
        fn $fn_name(
            _ini: &mut INI,
            data: &mut BoneFXUpdateModuleData,
            tokens: &[&str],
        ) -> Result<(), INIError> {
            parse_ocl_entry(tokens, &mut data.ocl[$state][$idx])
        }
    };
}

macro_rules! particle_parser {
    ($fn_name:ident, $state:expr, $idx:expr) => {
        fn $fn_name(
            _ini: &mut INI,
            data: &mut BoneFXUpdateModuleData,
            tokens: &[&str],
        ) -> Result<(), INIError> {
            parse_particle_entry(tokens, &mut data.particle_system[$state][$idx])
        }
    };
}

fx_list_parser!(parse_pristine_fx_list1, STATE_PRISTINE, 0);
fx_list_parser!(parse_pristine_fx_list2, STATE_PRISTINE, 1);
fx_list_parser!(parse_pristine_fx_list3, STATE_PRISTINE, 2);
fx_list_parser!(parse_pristine_fx_list4, STATE_PRISTINE, 3);
fx_list_parser!(parse_pristine_fx_list5, STATE_PRISTINE, 4);
fx_list_parser!(parse_pristine_fx_list6, STATE_PRISTINE, 5);
fx_list_parser!(parse_pristine_fx_list7, STATE_PRISTINE, 6);
fx_list_parser!(parse_pristine_fx_list8, STATE_PRISTINE, 7);

fx_list_parser!(parse_damaged_fx_list1, STATE_DAMAGED, 0);
fx_list_parser!(parse_damaged_fx_list2, STATE_DAMAGED, 1);
fx_list_parser!(parse_damaged_fx_list3, STATE_DAMAGED, 2);
fx_list_parser!(parse_damaged_fx_list4, STATE_DAMAGED, 3);
fx_list_parser!(parse_damaged_fx_list5, STATE_DAMAGED, 4);
fx_list_parser!(parse_damaged_fx_list6, STATE_DAMAGED, 5);
fx_list_parser!(parse_damaged_fx_list7, STATE_DAMAGED, 6);
fx_list_parser!(parse_damaged_fx_list8, STATE_DAMAGED, 7);

fx_list_parser!(parse_really_damaged_fx_list1, STATE_REALLY_DAMAGED, 0);
fx_list_parser!(parse_really_damaged_fx_list2, STATE_REALLY_DAMAGED, 1);
fx_list_parser!(parse_really_damaged_fx_list3, STATE_REALLY_DAMAGED, 2);
fx_list_parser!(parse_really_damaged_fx_list4, STATE_REALLY_DAMAGED, 3);
fx_list_parser!(parse_really_damaged_fx_list5, STATE_REALLY_DAMAGED, 4);
fx_list_parser!(parse_really_damaged_fx_list6, STATE_REALLY_DAMAGED, 5);
fx_list_parser!(parse_really_damaged_fx_list7, STATE_REALLY_DAMAGED, 6);
fx_list_parser!(parse_really_damaged_fx_list8, STATE_REALLY_DAMAGED, 7);

fx_list_parser!(parse_rubble_fx_list1, STATE_RUBBLE, 0);
fx_list_parser!(parse_rubble_fx_list2, STATE_RUBBLE, 1);
fx_list_parser!(parse_rubble_fx_list3, STATE_RUBBLE, 2);
fx_list_parser!(parse_rubble_fx_list4, STATE_RUBBLE, 3);
fx_list_parser!(parse_rubble_fx_list5, STATE_RUBBLE, 4);
fx_list_parser!(parse_rubble_fx_list6, STATE_RUBBLE, 5);
fx_list_parser!(parse_rubble_fx_list7, STATE_RUBBLE, 6);
fx_list_parser!(parse_rubble_fx_list8, STATE_RUBBLE, 7);

ocl_parser!(parse_pristine_ocl1, STATE_PRISTINE, 0);
ocl_parser!(parse_pristine_ocl2, STATE_PRISTINE, 1);
ocl_parser!(parse_pristine_ocl3, STATE_PRISTINE, 2);
ocl_parser!(parse_pristine_ocl4, STATE_PRISTINE, 3);
ocl_parser!(parse_pristine_ocl5, STATE_PRISTINE, 4);
ocl_parser!(parse_pristine_ocl6, STATE_PRISTINE, 5);
ocl_parser!(parse_pristine_ocl7, STATE_PRISTINE, 6);
ocl_parser!(parse_pristine_ocl8, STATE_PRISTINE, 7);

ocl_parser!(parse_damaged_ocl1, STATE_DAMAGED, 0);
ocl_parser!(parse_damaged_ocl2, STATE_DAMAGED, 1);
ocl_parser!(parse_damaged_ocl3, STATE_DAMAGED, 2);
ocl_parser!(parse_damaged_ocl4, STATE_DAMAGED, 3);
ocl_parser!(parse_damaged_ocl5, STATE_DAMAGED, 4);
ocl_parser!(parse_damaged_ocl6, STATE_DAMAGED, 5);
ocl_parser!(parse_damaged_ocl7, STATE_DAMAGED, 6);
ocl_parser!(parse_damaged_ocl8, STATE_DAMAGED, 7);

ocl_parser!(parse_really_damaged_ocl1, STATE_REALLY_DAMAGED, 0);
ocl_parser!(parse_really_damaged_ocl2, STATE_REALLY_DAMAGED, 1);
ocl_parser!(parse_really_damaged_ocl3, STATE_REALLY_DAMAGED, 2);
ocl_parser!(parse_really_damaged_ocl4, STATE_REALLY_DAMAGED, 3);
ocl_parser!(parse_really_damaged_ocl5, STATE_REALLY_DAMAGED, 4);
ocl_parser!(parse_really_damaged_ocl6, STATE_REALLY_DAMAGED, 5);
ocl_parser!(parse_really_damaged_ocl7, STATE_REALLY_DAMAGED, 6);
ocl_parser!(parse_really_damaged_ocl8, STATE_REALLY_DAMAGED, 7);

ocl_parser!(parse_rubble_ocl1, STATE_RUBBLE, 0);
ocl_parser!(parse_rubble_ocl2, STATE_RUBBLE, 1);
ocl_parser!(parse_rubble_ocl3, STATE_RUBBLE, 2);
ocl_parser!(parse_rubble_ocl4, STATE_RUBBLE, 3);
ocl_parser!(parse_rubble_ocl5, STATE_RUBBLE, 4);
ocl_parser!(parse_rubble_ocl6, STATE_RUBBLE, 5);
ocl_parser!(parse_rubble_ocl7, STATE_RUBBLE, 6);
ocl_parser!(parse_rubble_ocl8, STATE_RUBBLE, 7);

particle_parser!(parse_pristine_particle1, STATE_PRISTINE, 0);
particle_parser!(parse_pristine_particle2, STATE_PRISTINE, 1);
particle_parser!(parse_pristine_particle3, STATE_PRISTINE, 2);
particle_parser!(parse_pristine_particle4, STATE_PRISTINE, 3);
particle_parser!(parse_pristine_particle5, STATE_PRISTINE, 4);
particle_parser!(parse_pristine_particle6, STATE_PRISTINE, 5);
particle_parser!(parse_pristine_particle7, STATE_PRISTINE, 6);
particle_parser!(parse_pristine_particle8, STATE_PRISTINE, 7);

particle_parser!(parse_damaged_particle1, STATE_DAMAGED, 0);
particle_parser!(parse_damaged_particle2, STATE_DAMAGED, 1);
particle_parser!(parse_damaged_particle3, STATE_DAMAGED, 2);
particle_parser!(parse_damaged_particle4, STATE_DAMAGED, 3);
particle_parser!(parse_damaged_particle5, STATE_DAMAGED, 4);
particle_parser!(parse_damaged_particle6, STATE_DAMAGED, 5);
particle_parser!(parse_damaged_particle7, STATE_DAMAGED, 6);
particle_parser!(parse_damaged_particle8, STATE_DAMAGED, 7);

particle_parser!(parse_really_damaged_particle1, STATE_REALLY_DAMAGED, 0);
particle_parser!(parse_really_damaged_particle2, STATE_REALLY_DAMAGED, 1);
particle_parser!(parse_really_damaged_particle3, STATE_REALLY_DAMAGED, 2);
particle_parser!(parse_really_damaged_particle4, STATE_REALLY_DAMAGED, 3);
particle_parser!(parse_really_damaged_particle5, STATE_REALLY_DAMAGED, 4);
particle_parser!(parse_really_damaged_particle6, STATE_REALLY_DAMAGED, 5);
particle_parser!(parse_really_damaged_particle7, STATE_REALLY_DAMAGED, 6);
particle_parser!(parse_really_damaged_particle8, STATE_REALLY_DAMAGED, 7);

particle_parser!(parse_rubble_particle1, STATE_RUBBLE, 0);
particle_parser!(parse_rubble_particle2, STATE_RUBBLE, 1);
particle_parser!(parse_rubble_particle3, STATE_RUBBLE, 2);
particle_parser!(parse_rubble_particle4, STATE_RUBBLE, 3);
particle_parser!(parse_rubble_particle5, STATE_RUBBLE, 4);
particle_parser!(parse_rubble_particle6, STATE_RUBBLE, 5);
particle_parser!(parse_rubble_particle7, STATE_RUBBLE, 6);
particle_parser!(parse_rubble_particle8, STATE_RUBBLE, 7);

const BONE_FX_UPDATE_FIELDS: &[FieldParse<BoneFXUpdateModuleData>] = &[
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
        token: "PristineFXList1",
        parse: parse_pristine_fx_list1,
    },
    FieldParse {
        token: "PristineFXList2",
        parse: parse_pristine_fx_list2,
    },
    FieldParse {
        token: "PristineFXList3",
        parse: parse_pristine_fx_list3,
    },
    FieldParse {
        token: "PristineFXList4",
        parse: parse_pristine_fx_list4,
    },
    FieldParse {
        token: "PristineFXList5",
        parse: parse_pristine_fx_list5,
    },
    FieldParse {
        token: "PristineFXList6",
        parse: parse_pristine_fx_list6,
    },
    FieldParse {
        token: "PristineFXList7",
        parse: parse_pristine_fx_list7,
    },
    FieldParse {
        token: "PristineFXList8",
        parse: parse_pristine_fx_list8,
    },
    FieldParse {
        token: "DamagedFXList1",
        parse: parse_damaged_fx_list1,
    },
    FieldParse {
        token: "DamagedFXList2",
        parse: parse_damaged_fx_list2,
    },
    FieldParse {
        token: "DamagedFXList3",
        parse: parse_damaged_fx_list3,
    },
    FieldParse {
        token: "DamagedFXList4",
        parse: parse_damaged_fx_list4,
    },
    FieldParse {
        token: "DamagedFXList5",
        parse: parse_damaged_fx_list5,
    },
    FieldParse {
        token: "DamagedFXList6",
        parse: parse_damaged_fx_list6,
    },
    FieldParse {
        token: "DamagedFXList7",
        parse: parse_damaged_fx_list7,
    },
    FieldParse {
        token: "DamagedFXList8",
        parse: parse_damaged_fx_list8,
    },
    FieldParse {
        token: "ReallyDamagedFXList1",
        parse: parse_really_damaged_fx_list1,
    },
    FieldParse {
        token: "ReallyDamagedFXList2",
        parse: parse_really_damaged_fx_list2,
    },
    FieldParse {
        token: "ReallyDamagedFXList3",
        parse: parse_really_damaged_fx_list3,
    },
    FieldParse {
        token: "ReallyDamagedFXList4",
        parse: parse_really_damaged_fx_list4,
    },
    FieldParse {
        token: "ReallyDamagedFXList5",
        parse: parse_really_damaged_fx_list5,
    },
    FieldParse {
        token: "ReallyDamagedFXList6",
        parse: parse_really_damaged_fx_list6,
    },
    FieldParse {
        token: "ReallyDamagedFXList7",
        parse: parse_really_damaged_fx_list7,
    },
    FieldParse {
        token: "ReallyDamagedFXList8",
        parse: parse_really_damaged_fx_list8,
    },
    FieldParse {
        token: "RubbleFXList1",
        parse: parse_rubble_fx_list1,
    },
    FieldParse {
        token: "RubbleFXList2",
        parse: parse_rubble_fx_list2,
    },
    FieldParse {
        token: "RubbleFXList3",
        parse: parse_rubble_fx_list3,
    },
    FieldParse {
        token: "RubbleFXList4",
        parse: parse_rubble_fx_list4,
    },
    FieldParse {
        token: "RubbleFXList5",
        parse: parse_rubble_fx_list5,
    },
    FieldParse {
        token: "RubbleFXList6",
        parse: parse_rubble_fx_list6,
    },
    FieldParse {
        token: "RubbleFXList7",
        parse: parse_rubble_fx_list7,
    },
    FieldParse {
        token: "RubbleFXList8",
        parse: parse_rubble_fx_list8,
    },
    FieldParse {
        token: "PristineOCL1",
        parse: parse_pristine_ocl1,
    },
    FieldParse {
        token: "PristineOCL2",
        parse: parse_pristine_ocl2,
    },
    FieldParse {
        token: "PristineOCL3",
        parse: parse_pristine_ocl3,
    },
    FieldParse {
        token: "PristineOCL4",
        parse: parse_pristine_ocl4,
    },
    FieldParse {
        token: "PristineOCL5",
        parse: parse_pristine_ocl5,
    },
    FieldParse {
        token: "PristineOCL6",
        parse: parse_pristine_ocl6,
    },
    FieldParse {
        token: "PristineOCL7",
        parse: parse_pristine_ocl7,
    },
    FieldParse {
        token: "PristineOCL8",
        parse: parse_pristine_ocl8,
    },
    FieldParse {
        token: "DamagedOCL1",
        parse: parse_damaged_ocl1,
    },
    FieldParse {
        token: "DamagedOCL2",
        parse: parse_damaged_ocl2,
    },
    FieldParse {
        token: "DamagedOCL3",
        parse: parse_damaged_ocl3,
    },
    FieldParse {
        token: "DamagedOCL4",
        parse: parse_damaged_ocl4,
    },
    FieldParse {
        token: "DamagedOCL5",
        parse: parse_damaged_ocl5,
    },
    FieldParse {
        token: "DamagedOCL6",
        parse: parse_damaged_ocl6,
    },
    FieldParse {
        token: "DamagedOCL7",
        parse: parse_damaged_ocl7,
    },
    FieldParse {
        token: "DamagedOCL8",
        parse: parse_damaged_ocl8,
    },
    FieldParse {
        token: "ReallyDamagedOCL1",
        parse: parse_really_damaged_ocl1,
    },
    FieldParse {
        token: "ReallyDamagedOCL2",
        parse: parse_really_damaged_ocl2,
    },
    FieldParse {
        token: "ReallyDamagedOCL3",
        parse: parse_really_damaged_ocl3,
    },
    FieldParse {
        token: "ReallyDamagedOCL4",
        parse: parse_really_damaged_ocl4,
    },
    FieldParse {
        token: "ReallyDamagedOCL5",
        parse: parse_really_damaged_ocl5,
    },
    FieldParse {
        token: "ReallyDamagedOCL6",
        parse: parse_really_damaged_ocl6,
    },
    FieldParse {
        token: "ReallyDamagedOCL7",
        parse: parse_really_damaged_ocl7,
    },
    FieldParse {
        token: "ReallyDamagedOCL8",
        parse: parse_really_damaged_ocl8,
    },
    FieldParse {
        token: "RubbleOCL1",
        parse: parse_rubble_ocl1,
    },
    FieldParse {
        token: "RubbleOCL2",
        parse: parse_rubble_ocl2,
    },
    FieldParse {
        token: "RubbleOCL3",
        parse: parse_rubble_ocl3,
    },
    FieldParse {
        token: "RubbleOCL4",
        parse: parse_rubble_ocl4,
    },
    FieldParse {
        token: "RubbleOCL5",
        parse: parse_rubble_ocl5,
    },
    FieldParse {
        token: "RubbleOCL6",
        parse: parse_rubble_ocl6,
    },
    FieldParse {
        token: "RubbleOCL7",
        parse: parse_rubble_ocl7,
    },
    FieldParse {
        token: "RubbleOCL8",
        parse: parse_rubble_ocl8,
    },
    FieldParse {
        token: "PristineParticleSystem1",
        parse: parse_pristine_particle1,
    },
    FieldParse {
        token: "PristineParticleSystem2",
        parse: parse_pristine_particle2,
    },
    FieldParse {
        token: "PristineParticleSystem3",
        parse: parse_pristine_particle3,
    },
    FieldParse {
        token: "PristineParticleSystem4",
        parse: parse_pristine_particle4,
    },
    FieldParse {
        token: "PristineParticleSystem5",
        parse: parse_pristine_particle5,
    },
    FieldParse {
        token: "PristineParticleSystem6",
        parse: parse_pristine_particle6,
    },
    FieldParse {
        token: "PristineParticleSystem7",
        parse: parse_pristine_particle7,
    },
    FieldParse {
        token: "PristineParticleSystem8",
        parse: parse_pristine_particle8,
    },
    FieldParse {
        token: "DamagedParticleSystem1",
        parse: parse_damaged_particle1,
    },
    FieldParse {
        token: "DamagedParticleSystem2",
        parse: parse_damaged_particle2,
    },
    FieldParse {
        token: "DamagedParticleSystem3",
        parse: parse_damaged_particle3,
    },
    FieldParse {
        token: "DamagedParticleSystem4",
        parse: parse_damaged_particle4,
    },
    FieldParse {
        token: "DamagedParticleSystem5",
        parse: parse_damaged_particle5,
    },
    FieldParse {
        token: "DamagedParticleSystem6",
        parse: parse_damaged_particle6,
    },
    FieldParse {
        token: "DamagedParticleSystem7",
        parse: parse_damaged_particle7,
    },
    FieldParse {
        token: "DamagedParticleSystem8",
        parse: parse_damaged_particle8,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem1",
        parse: parse_really_damaged_particle1,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem2",
        parse: parse_really_damaged_particle2,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem3",
        parse: parse_really_damaged_particle3,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem4",
        parse: parse_really_damaged_particle4,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem5",
        parse: parse_really_damaged_particle5,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem6",
        parse: parse_really_damaged_particle6,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem7",
        parse: parse_really_damaged_particle7,
    },
    FieldParse {
        token: "ReallyDamagedParticleSystem8",
        parse: parse_really_damaged_particle8,
    },
    FieldParse {
        token: "RubbleParticleSystem1",
        parse: parse_rubble_particle1,
    },
    FieldParse {
        token: "RubbleParticleSystem2",
        parse: parse_rubble_particle2,
    },
    FieldParse {
        token: "RubbleParticleSystem3",
        parse: parse_rubble_particle3,
    },
    FieldParse {
        token: "RubbleParticleSystem4",
        parse: parse_rubble_particle4,
    },
    FieldParse {
        token: "RubbleParticleSystem5",
        parse: parse_rubble_particle5,
    },
    FieldParse {
        token: "RubbleParticleSystem6",
        parse: parse_rubble_particle6,
    },
    FieldParse {
        token: "RubbleParticleSystem7",
        parse: parse_rubble_particle7,
    },
    FieldParse {
        token: "RubbleParticleSystem8",
        parse: parse_rubble_particle8,
    },
];

/// BoneFXUpdate - Manages effects attached to bones for different damage states.
/// Matches C++ BoneFXUpdate.cpp:53-530
#[derive(Debug, Clone)]
pub struct BoneFXUpdate {
    object_id: ObjectID,
    module_data: Arc<BoneFXUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    /// Next frame to trigger FX for each damage state and bone.
    next_fx_frame: [[i32; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Next frame to trigger OCL for each damage state and bone.
    next_ocl_frame: [[i32; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Next frame to trigger particle system for each damage state and bone.
    next_particle_system_frame: [[i32; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Bone positions for FX.
    fx_bone_positions: [[Coord3D; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Bone positions for OCL.
    ocl_bone_positions: [[Coord3D; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Bone positions for particle systems.
    ps_bone_positions: [[Coord3D; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
    /// Whether bones have been resolved for each damage state.
    bones_resolved: [bool; BODY_DAMAGE_TYPE_COUNT],
    /// Active particle system IDs.
    particle_system_ids: Vec<ParticleSystemId>,
    /// Whether the module is active.
    active: bool,
    /// Current body damage state.
    cur_body_state: BodyDamageType,
}

impl BoneFXUpdate {
    /// Create new BoneFXUpdate module.
    /// Matches C++ BoneFXUpdate.cpp:53-72
    pub fn new(object_id: ObjectID, module_data: Arc<BoneFXUpdateModuleData>) -> Self {
        Self {
            object_id,
            module_data,
            next_call_frame_and_phase: 0,
            next_fx_frame: [[-1; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            next_ocl_frame: [[-1; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            next_particle_system_frame: [[-1; BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            fx_bone_positions: [[Coord3D::origin(); BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            ocl_bone_positions: [[Coord3D::origin(); BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            ps_bone_positions: [[Coord3D::origin(); BONE_FX_MAX_BONES]; BODY_DAMAGE_TYPE_COUNT],
            bones_resolved: [false; BODY_DAMAGE_TYPE_COUNT],
            particle_system_ids: Vec::new(),
            active: false,
            cur_body_state: BodyDamageType::Pristine,
        }
    }

    /// Change body damage state.
    /// Matches C++ BoneFXUpdate.cpp:351-356
    pub fn change_body_damage_state(
        &mut self,
        _old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) {
        self.cur_body_state = new_state;
        self.kill_running_particle_systems();
        let now = TheGameLogic::get_frame() as i32;
        self.init_times(now);
    }

    /// Change body damage state without external context (legacy call sites).
    pub fn change_body_damage_state_simple(
        &mut self,
        old_state: BodyDamageType,
        new_state: BodyDamageType,
    ) {
        self.change_body_damage_state(old_state, new_state);
    }

    /// Stop all bone FX.
    /// Matches C++ BoneFXUpdate.cpp:520-530
    pub fn stop_all_bone_fx_simple(&mut self) {
        for i in 0..BODY_DAMAGE_TYPE_COUNT {
            for j in 0..BONE_FX_MAX_BONES {
                self.next_fx_frame[i][j] = -1;
                self.next_ocl_frame[i][j] = -1;
                self.next_particle_system_frame[i][j] = -1;
            }
        }
        self.kill_running_particle_systems();
    }

    fn update_internal(&mut self) -> UpdateSleepTime {
        let now = TheGameLogic::get_frame() as i32;

        if !self.active {
            self.init_times(now);
            self.active = true;
        }

        let state_idx = self.cur_body_state as usize;
        for i in 0..BONE_FX_MAX_BONES {
            if self.next_fx_frame[state_idx][i] != -1 && self.next_fx_frame[state_idx][i] <= now {
                if let Some(fx) = self.module_data.fx_list[state_idx][i].fx {
                    let bone_pos = self.fx_bone_positions[state_idx][i];
                    self.do_fx_list_at_bone(fx, &bone_pos);
                }
                let base_info = &self.module_data.fx_list[state_idx][i].base;
                let mut next_frame = self.next_fx_frame[state_idx][i];
                self.compute_next_logic_fx_time(base_info, &mut next_frame);
                self.next_fx_frame[state_idx][i] = next_frame;
            }

            if self.next_ocl_frame[state_idx][i] != -1 && self.next_ocl_frame[state_idx][i] <= now {
                if let Some(ocl) = self.module_data.ocl[state_idx][i].ocl {
                    let bone_pos = self.ocl_bone_positions[state_idx][i];
                    self.do_ocl_at_bone(ocl, &bone_pos);
                }
                let base_info = &self.module_data.ocl[state_idx][i].base;
                let mut next_frame = self.next_ocl_frame[state_idx][i];
                self.compute_next_logic_fx_time(base_info, &mut next_frame);
                self.next_ocl_frame[state_idx][i] = next_frame;
            }

            if self.next_particle_system_frame[state_idx][i] != -1
                && self.next_particle_system_frame[state_idx][i] <= now
            {
                if let Some(ps) =
                    self.module_data.particle_system[state_idx][i].particle_sys_template
                {
                    let bone_pos = self.ps_bone_positions[state_idx][i];
                    self.do_particle_system_at_bone(ps, &bone_pos);
                }
                let base_info = &self.module_data.particle_system[state_idx][i].base;
                let mut next_frame = self.next_particle_system_frame[state_idx][i];
                self.compute_next_client_fx_time(base_info, &mut next_frame);
                self.next_particle_system_frame[state_idx][i] = next_frame;
            }
        }

        UpdateSleepTime::None
    }

    /// Initialize timing for all FX.
    /// Matches C++ BoneFXUpdate.cpp:297-319
    fn init_times(&mut self, now: i32) {
        let state_idx = self.cur_body_state as usize;
        for i in 0..BONE_FX_MAX_BONES {
            if !self.module_data.fx_list[state_idx][i]
                .base
                .loc_info
                .bone_name
                .is_empty()
            {
                let delay = self.module_data.fx_list[state_idx][i]
                    .base
                    .game_logic_delay
                    .get_value() as i32;
                self.next_fx_frame[state_idx][i] = now + delay;
            } else {
                self.next_fx_frame[state_idx][i] = -1;
            }

            if !self.module_data.ocl[state_idx][i]
                .base
                .loc_info
                .bone_name
                .is_empty()
            {
                let delay = self.module_data.ocl[state_idx][i]
                    .base
                    .game_logic_delay
                    .get_value() as i32;
                self.next_ocl_frame[state_idx][i] = now + delay;
            } else {
                self.next_ocl_frame[state_idx][i] = -1;
            }

            if !self.module_data.particle_system[state_idx][i]
                .base
                .loc_info
                .bone_name
                .is_empty()
            {
                let delay = self.module_data.particle_system[state_idx][i]
                    .base
                    .game_client_delay
                    .get_value() as i32;
                self.next_particle_system_frame[state_idx][i] = now + delay;
            } else {
                self.next_particle_system_frame[state_idx][i] = -1;
            }
        }
    }

    fn resolve_bone_locations(&mut self) {
        let state_idx = self.cur_body_state as usize;
        let Some(object_arc) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return;
        };
        let Ok(object_guard) = object_arc.read() else {
            return;
        };
        let Some(drawable) = object_guard.get_drawable() else {
            return;
        };
        let Ok(draw_guard) = drawable.read() else {
            return;
        };

        for i in 0..BONE_FX_MAX_BONES {
            if !self.module_data.fx_list[state_idx][i]
                .base
                .loc_info
                .bone_name
                .is_empty()
            {
                let bone_name = self.module_data.fx_list[state_idx][i]
                    .base
                    .loc_info
                    .bone_name
                    .as_str();
                let positions = draw_guard.get_pristine_bone_positions(bone_name, 0, 1);
                if let Some(pos) = positions.first() {
                    self.fx_bone_positions[state_idx][i] = *pos;
                }
            }

            if !self.module_data.ocl[state_idx][i]
                .base
                .loc_info
                .bone_name
                .is_empty()
            {
                let bone_name = self.module_data.ocl[state_idx][i]
                    .base
                    .loc_info
                    .bone_name
                    .as_str();
                let positions = draw_guard.get_pristine_bone_positions(bone_name, 0, 1);
                if let Some(pos) = positions.first() {
                    self.ocl_bone_positions[state_idx][i] = *pos;
                }
            }

            if !self.module_data.particle_system[state_idx][i]
                .base
                .loc_info
                .bone_name
                .is_empty()
            {
                let bone_name = self.module_data.particle_system[state_idx][i]
                    .base
                    .loc_info
                    .bone_name
                    .as_str();
                let positions = draw_guard.get_pristine_bone_positions(bone_name, 0, 1);
                if let Some(pos) = positions.first() {
                    self.ps_bone_positions[state_idx][i] = *pos;
                }
            }
        }

        self.bones_resolved[state_idx] = true;
    }

    /// Execute FX list at a bone position.
    /// Matches C++ BoneFXUpdate.cpp:360-383
    fn do_fx_list_at_bone(&mut self, fx_list: FXListId, bone_position: &Coord3D) {
        let state_idx = self.cur_body_state as usize;
        if !self.bones_resolved[state_idx] {
            self.resolve_bone_locations();
        }

        let Some(object_arc) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return;
        };
        let Ok(object_guard) = object_arc.read() else {
            return;
        };

        if let Some(body_module) = object_guard.get_body_module() {
            if let Some(last_damage_info) = body_module.get_last_damage_info() {
                if !self
                    .module_data
                    .damage_fx_types
                    .contains_damage_type(last_damage_info.input.damage_type)
                {
                    return;
                }
            }
        }

        let world_transform = object_guard.convert_bone_pos_to_world_pos(Some(bone_position), None);
        let translation = world_transform.w_axis;
        let new_pos = Coord3D {
            x: translation.x,
            y: translation.y,
            z: translation.z,
        };

        if let Some(manager) = get_fx_list_manager() {
            manager.do_fx_pos(fx_list, &new_pos, None);
        }
    }

    /// Execute OCL at a bone position.
    /// Matches C++ BoneFXUpdate.cpp:387-408
    fn do_ocl_at_bone(&mut self, ocl: ObjectCreationListId, bone_position: &Coord3D) {
        let state_idx = self.cur_body_state as usize;
        if !self.bones_resolved[state_idx] {
            self.resolve_bone_locations();
        }

        let Some(object_arc) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return;
        };
        let Ok(object_guard) = object_arc.read() else {
            return;
        };

        if let Some(body_module) = object_guard.get_body_module() {
            if let Some(last_damage_info) = body_module.get_last_damage_info() {
                if !self
                    .module_data
                    .damage_ocl_types
                    .contains_damage_type(last_damage_info.input.damage_type)
                {
                    return;
                }
            }
        }

        let world_transform = object_guard.convert_bone_pos_to_world_pos(Some(bone_position), None);
        let translation = world_transform.w_axis;
        let new_pos = Coord3D {
            x: translation.x,
            y: translation.y,
            z: translation.z,
        };

        let Some(ocl_name) = NameKeyGenerator::key_to_name(ocl as NameKeyType) else {
            return;
        };
        let Some(ocl_handle) =
            TheObjectCreationListStore::find_object_creation_list(ocl_name.as_str())
        else {
            return;
        };
        let ctx = live_creation_context();
        let _ = ocl_handle.create_with_angle(
            &ctx,
            Some(&*object_guard),
            &new_pos,
            &new_pos,
            INVALID_ANGLE,
            0,
        );
    }

    /// Execute particle system at a bone position.
    /// Matches C++ BoneFXUpdate.cpp:412-438
    fn do_particle_system_at_bone(
        &mut self,
        particle_system_template: ParticleSystemTemplateId,
        bone_position: &Coord3D,
    ) {
        let state_idx = self.cur_body_state as usize;
        if !self.bones_resolved[state_idx] {
            self.resolve_bone_locations();
        }

        let Some(object_arc) = OBJECT_REGISTRY.get_object(self.object_id) else {
            return;
        };
        let Ok(object_guard) = object_arc.read() else {
            return;
        };

        if let Some(body_module) = object_guard.get_body_module() {
            if let Some(last_damage_info) = body_module.get_last_damage_info() {
                if !self
                    .module_data
                    .damage_particle_types
                    .contains_damage_type(last_damage_info.input.damage_type)
                {
                    return;
                }
            }
        }

        let world_transform = object_guard.convert_bone_pos_to_world_pos(Some(bone_position), None);
        let translation = world_transform.w_axis;
        let new_pos = Coord3D {
            x: translation.x,
            y: translation.y,
            z: translation.z,
        };

        let Some(template_name) =
            NameKeyGenerator::key_to_name(particle_system_template as NameKeyType)
        else {
            return;
        };
        let Some(ps_manager) = TheParticleSystemManager::get() else {
            return;
        };

        let Some(psys_id) = ps_manager.create_particle_system(Some(template_name.as_str())) else {
            return;
        };

        if let Some(drawable) = object_guard.get_drawable() {
            if let Ok(draw_guard) = drawable.read() {
                if draw_guard.is_drawable_effectively_hidden() {
                    // Best-effort replacement for C++ ParticleSystem::stop().
                    ps_manager.destroy_particle_system(psys_id);
                    return;
                }
            }
        }

        self.particle_system_ids.push(psys_id);
        ps_manager.set_particle_system_position(psys_id, &new_pos);
        ps_manager.attach_particle_system_to_object(psys_id, object_guard.get_id());
    }

    /// Compute next client FX time.
    /// Matches C++ BoneFXUpdate.cpp:442-449
    fn compute_next_client_fx_time(&self, info: &BaseBoneListInfo, next_frame: &mut i32) {
        if info.only_once {
            *next_frame = -1;
            return;
        }
        *next_frame = TheGameLogic::get_frame() as i32 + info.game_client_delay.get_value() as i32;
    }

    /// Compute next logic FX time.
    /// Matches C++ BoneFXUpdate.cpp:453-460
    fn compute_next_logic_fx_time(&self, info: &BaseBoneListInfo, next_frame: &mut i32) {
        if info.only_once {
            *next_frame = -1;
            return;
        }
        *next_frame = TheGameLogic::get_frame() as i32 + info.game_logic_delay.get_value() as i32;
    }

    /// Kill all running particle systems.
    /// Matches C++ BoneFXUpdate.cpp:464-473
    fn kill_running_particle_systems(&mut self) {
        if let Some(ps_manager) = TheParticleSystemManager::get() {
            for system_id in &self.particle_system_ids {
                ps_manager.destroy_particle_system(*system_id);
            }
        }
        self.particle_system_ids.clear();
    }

    /// Save state to xfer.
    /// Matches C++ BoneFXUpdate.cpp:548-629
    pub fn save(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("BoneFXUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_u16(&mut (self.particle_system_ids.len() as u16));
        for id in &self.particle_system_ids {
            let mut id_copy = *id;
            xfer.xfer_particle_system_id(&mut id_copy);
        }

        for i in 0..BODY_DAMAGE_TYPE_COUNT {
            for j in 0..BONE_FX_MAX_BONES {
                let mut next_fx_frame = self.next_fx_frame[i][j];
                xfer_io(xfer.xfer_i32(&mut next_fx_frame), "next_fx_frame");
            }
        }

        for i in 0..BODY_DAMAGE_TYPE_COUNT {
            for j in 0..BONE_FX_MAX_BONES {
                let mut next_ocl_frame = self.next_ocl_frame[i][j];
                xfer_io(xfer.xfer_i32(&mut next_ocl_frame), "next_ocl_frame");
            }
        }

        for i in 0..BODY_DAMAGE_TYPE_COUNT {
            for j in 0..BONE_FX_MAX_BONES {
                let mut next_ps_frame = self.next_particle_system_frame[i][j];
                xfer_io(
                    xfer.xfer_i32(&mut next_ps_frame),
                    "next_particle_system_frame",
                );
            }
        }

        for i in 0..BODY_DAMAGE_TYPE_COUNT {
            for j in 0..BONE_FX_MAX_BONES {
                xfer.xfer_coord3d(&mut self.fx_bone_positions[i][j].clone());
            }
        }

        for i in 0..BODY_DAMAGE_TYPE_COUNT {
            for j in 0..BONE_FX_MAX_BONES {
                xfer.xfer_coord3d(&mut self.ocl_bone_positions[i][j].clone());
            }
        }

        for i in 0..BODY_DAMAGE_TYPE_COUNT {
            for j in 0..BONE_FX_MAX_BONES {
                xfer.xfer_coord3d(&mut self.ps_bone_positions[i][j].clone());
            }
        }

        let mut cur_body_state = self.cur_body_state as i32;
        xfer_io(xfer.xfer_i32(&mut cur_body_state), "cur_body_state");

        for resolved in &self.bones_resolved {
            let mut value = *resolved;
            xfer_io(xfer.xfer_bool(&mut value), "bones_resolved");
        }

        let mut active = self.active;
        xfer_io(xfer.xfer_bool(&mut active), "active");

        Ok(())
    }

    /// Load state from xfer.
    /// Matches C++ BoneFXUpdate.cpp:548-629
    pub fn load(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("BoneFXUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

            let mut count: u16 = 0;
            xfer.xfer_u16(&mut count);
            if !self.particle_system_ids.is_empty() {
                self.particle_system_ids.clear();
            }
            for _ in 0..count {
                let mut id = ParticleSystemId::default();
                xfer.xfer_particle_system_id(&mut id);
                self.particle_system_ids.push(id);
            }

            for i in 0..BODY_DAMAGE_TYPE_COUNT {
                for j in 0..BONE_FX_MAX_BONES {
                    xfer_io(
                        xfer.xfer_i32(&mut self.next_fx_frame[i][j]),
                        "next_fx_frame",
                    );
                }
            }

            for i in 0..BODY_DAMAGE_TYPE_COUNT {
                for j in 0..BONE_FX_MAX_BONES {
                    xfer_io(
                        xfer.xfer_i32(&mut self.next_ocl_frame[i][j]),
                        "next_ocl_frame",
                    );
                }
            }

            for i in 0..BODY_DAMAGE_TYPE_COUNT {
                for j in 0..BONE_FX_MAX_BONES {
                    xfer_io(
                        xfer.xfer_i32(&mut self.next_particle_system_frame[i][j]),
                        "next_particle_system_frame",
                    );
                }
            }

            for i in 0..BODY_DAMAGE_TYPE_COUNT {
                for j in 0..BONE_FX_MAX_BONES {
                    xfer.xfer_coord3d(&mut self.fx_bone_positions[i][j]);
                }
            }

            for i in 0..BODY_DAMAGE_TYPE_COUNT {
                for j in 0..BONE_FX_MAX_BONES {
                    xfer.xfer_coord3d(&mut self.ocl_bone_positions[i][j]);
                }
            }

            for i in 0..BODY_DAMAGE_TYPE_COUNT {
                for j in 0..BONE_FX_MAX_BONES {
                    xfer.xfer_coord3d(&mut self.ps_bone_positions[i][j]);
                }
            }

            let mut state: i32 = 0;
            xfer_io(xfer.xfer_i32(&mut state), "cur_body_state");
            self.cur_body_state = match state {
                0 => BodyDamageType::Pristine,
                1 => BodyDamageType::Damaged,
                2 => BodyDamageType::ReallyDamaged,
                3 => BodyDamageType::Rubble,
                _ => BodyDamageType::Pristine,
            };

            for resolved in &mut self.bones_resolved {
                xfer_io(xfer.xfer_bool(resolved), "bones_resolved");
            }

            xfer_io(xfer.xfer_bool(&mut self.active), "active");
        }
        Ok(())
    }
}

impl UpdateModuleInterface for BoneFXUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        self.update_internal()
    }
}

impl BehaviorModuleInterface for BoneFXUpdate {
    fn get_module_name(&self) -> &'static str {
        "BoneFXUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_bone_fx_control_interface(&mut self) -> Option<&mut dyn BoneFxControlInterface> {
        Some(self)
    }
}

impl BoneFxControlInterface for BoneFXUpdate {
    fn change_body_damage_state(&mut self, old_state: u32, new_state: u32) {
        let Some(old_state) = body_damage_type_from_index(old_state) else {
            return;
        };
        let Some(new_state) = body_damage_type_from_index(new_state) else {
            return;
        };
        self.change_body_damage_state_simple(old_state, new_state);
    }

    fn stop_all_bone_fx(&mut self) {
        self.stop_all_bone_fx_simple();
    }
}

impl Snapshotable for BoneFXUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if xfer.is_writing() {
            self.save(xfer)?;
        } else {
            self.load(xfer)?;
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes BoneFXUpdate through the common Module trait.
pub struct BoneFXUpdateModule {
    behavior: BoneFXUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<BoneFXUpdateModuleData>,
}

impl BoneFXUpdateModule {
    pub fn new(
        behavior: BoneFXUpdate,
        module_name: &AsciiString,
        module_data: Arc<BoneFXUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut BoneFXUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for BoneFXUpdateModule {
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

impl Module for BoneFXUpdateModule {
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
        ModuleData::get_module_tag_name_key(self.module_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn get_bone_fx_control_interface(&mut self) -> Option<&mut dyn BoneFxControlInterface> {
        Some(self)
    }

    fn on_delete(&mut self) {
        self.behavior.kill_running_particle_systems();
    }
}

impl BoneFxControlInterface for BoneFXUpdateModule {
    fn change_body_damage_state(&mut self, old_state: u32, new_state: u32) {
        BoneFxControlInterface::change_body_damage_state(&mut self.behavior, old_state, new_state);
    }

    fn stop_all_bone_fx(&mut self) {
        self.behavior.stop_all_bone_fx();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fx_entry_keeps_missing_reference_none() {
        let mut info = BoneFXListInfo::default();
        parse_fx_list_entry(
            &[
                "Bone:Root",
                "OnlyOnce:true",
                "0",
                "0",
                "FXList:MissingBoneFx_ParityTest_20260302",
            ],
            &mut info,
        )
        .expect("parse should succeed");
        assert!(info.fx.is_none());
    }

    #[test]
    fn bone_fx_update_exposes_typed_control_interface() {
        let data = Arc::new(BoneFXUpdateModuleData::default());
        let mut module = BoneFXUpdateModule::new(
            BoneFXUpdate::new(1, data.clone()),
            &AsciiString::from("BoneFXUpdate"),
            data,
        );

        let control = module
            .get_bone_fx_control_interface()
            .expect("BoneFXUpdate should expose BoneFxControlInterface");
        control.change_body_damage_state(
            BodyDamageType::Pristine as u32,
            BodyDamageType::Damaged as u32,
        );
        assert_eq!(module.behavior.cur_body_state, BodyDamageType::Damaged);

        let control = module
            .get_bone_fx_control_interface()
            .expect("BoneFXUpdate should expose BoneFxControlInterface");
        control.stop_all_bone_fx();
        assert!(module
            .behavior
            .next_fx_frame
            .iter()
            .flatten()
            .all(|frame| *frame == -1));
    }
}
