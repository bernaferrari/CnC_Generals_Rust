//! DumbProjectileBehavior – Rust conversion of the C++ DumbProjectileBehavior module.
//!
//! The legacy module creates a simple ballistic arc via Bezier control points and lets the
//! projectile detonate either on impact or when its lifespan expires. The full physics stack,
//! garrison interaction, and weapon detonation pipeline are still being ported, so this file
//! focuses on wiring module data parsing, module-factory integration, and a faithful state
//! machine scaffold. Remaining engine hooks are wired as dependent systems land.

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock, Weak};

use glam::Vec4;

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, Int, KindOf, KindOfMaskType, ModuleData, ObjectID,
    ObjectStatusMaskType, PathfindLayerEnum, Real, UnsignedInt, WeaponBonusConditionFlags,
    XferMode, XferVersion, KIND_OF_MASK_ALL, KIND_OF_MASK_NONE, LOGICFRAMES_PER_SECOND,
    MODELCONDITION_JAMMED, SECONDS_PER_LOGICFRAME_REAL,
};
use crate::effects::FXList;
use crate::helpers::{
    get_game_logic_random_value_real, TheFXListStore, TheGameLogic, ThePartitionManager,
};
use crate::modules::{
    BehaviorModuleInterface, ProjectileUpdateInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::{
    registry::OBJECT_REGISTRY, DrawableArcExt, Object as GameObject,
    INVALID_ID as OBJECT_INVALID_ID,
};
use crate::weapon::WeaponTemplate;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as ThingModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};
use log::warn;

fn terrain_layer_to_logic_layer(layer: crate::path::PathfindLayerEnum) -> PathfindLayerEnum {
    match layer {
        crate::path::PathfindLayerEnum::Ground => PathfindLayerEnum::Ground,
        crate::path::PathfindLayerEnum::Top => PathfindLayerEnum::Top,
        crate::path::PathfindLayerEnum::Bridge1 => PathfindLayerEnum::Bridge1,
        crate::path::PathfindLayerEnum::Bridge2 => PathfindLayerEnum::Bridge2,
        crate::path::PathfindLayerEnum::Bridge3 => PathfindLayerEnum::Bridge3,
        crate::path::PathfindLayerEnum::Bridge4 => PathfindLayerEnum::Bridge4,
        crate::path::PathfindLayerEnum::Wall => PathfindLayerEnum::Wall,
        crate::path::PathfindLayerEnum::Invalid | crate::path::PathfindLayerEnum::Last => {
            PathfindLayerEnum::Ground
        }
    }
}

// -------------------------------------------------------------------------------------------------
// INI parsing helpers
// -------------------------------------------------------------------------------------------------

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_real(value: &str) -> Result<Real, INIError> {
    INI::parse_real(value)
}

fn parse_percent(value: &str) -> Result<Real, INIError> {
    INI::parse_percent_to_real(value)
}

fn parse_unsigned(value: &str) -> Result<UnsignedInt, INIError> {
    INI::parse_unsigned_int(value)
}

fn parse_bool(value: &str) -> Result<Bool, INIError> {
    INI::parse_bool(value)
}

fn parse_duration_frames(value: &str) -> Result<UnsignedInt, INIError> {
    INI::parse_duration_unsigned_int(value)
}

fn parse_velocity_per_frame(value: &str) -> Result<Real, INIError> {
    let per_second = INI::parse_real(value)?;
    Ok(per_second * SECONDS_PER_LOGICFRAME_REAL)
}

fn parse_bool_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Bool),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(parse_bool(value)?);
    Ok(())
}

fn parse_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(parse_real(value)?);
    Ok(())
}

fn parse_percent_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(parse_percent(value)?);
    Ok(())
}

fn parse_unsigned_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(parse_unsigned(value)?);
    Ok(())
}

fn parse_velocity_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    setter(parse_velocity_per_frame(value)?);
    Ok(())
}

fn parse_fx_field(
    _ini: &mut INI,
    data: &mut DumbProjectileBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    if value.eq_ignore_ascii_case("NONE") {
        data.garrison_hit_kill_fx = None;
    } else {
        data.garrison_hit_kill_fx = TheFXListStore::lookup_fx_list(value);
        if data.garrison_hit_kill_fx.is_none() {
            log::warn!("DumbProjectileBehavior: unresolved FXList '{}'", value);
        }
    }
    Ok(())
}

fn parse_kind_of_mask(label: &str, _tokens: &[&str]) -> KindOfMaskType {
    fn parse_kind_of(token: &str) -> Option<KindOf> {
        let token = token.trim().trim_matches(',').trim();
        let token = token.strip_prefix("KINDOF_").unwrap_or(token);
        let token = token.strip_prefix("KINDOF").unwrap_or(token);
        let upper = token.to_ascii_uppercase();

        match upper.as_str() {
            "SELECTABLE" => Some(KindOf::Selectable),
            "UNIT" => Some(KindOf::Unit),
            "BUILDING" => Some(KindOf::Building),
            "VEHICLE" => Some(KindOf::Vehicle),
            "INFANTRY" => Some(KindOf::Infantry),
            "AIRCRAFT" => Some(KindOf::Aircraft),
            "DRONE" => Some(KindOf::Drone),
            "CLIFFJUMPER" | "CLIFF_JUMPER" => Some(KindOf::CliffJumper),
            "STRUCTURE" => Some(KindOf::Structure),
            "WEAPON" => Some(KindOf::Weapon),
            "PROJECTILE" => Some(KindOf::Projectile),
            "CANSEETHROUGH" | "CAN_SEE_THROUGH" => Some(KindOf::CanSeeThrough),
            "ALWAYSSELECTABLE" | "ALWAYS_SELECTABLE" => Some(KindOf::AlwaysSelectable),
            "CRATE" => Some(KindOf::Crate),
            "RESOURCENODE" | "RESOURCE_NODE" => Some(KindOf::ResourceNode),
            "DISGUISER" => Some(KindOf::Disguiser),
            "PORTABLE_STRUCTURE" | "PORTABLESTRUCTURE" => Some(KindOf::PortableStructure),
            "TECHBUILDING" | "TECH_BUILDING" => Some(KindOf::TechBuilding),
            "BRIDGE" => Some(KindOf::Bridge),
            "BARRIER" => Some(KindOf::Barrier),
            "CIVILIAN" => Some(KindOf::Civilian),
            "DESTRUCTIBLE" => Some(KindOf::Destructible),
            "CANCROSSBRIDGES" | "CAN_CROSS_BRIDGES" => Some(KindOf::CanCrossBridges),
            "AMPHIBIOUS" => Some(KindOf::Amphibious),
            "AMPHIBIOUSTRANSPORT" | "AMPHIBIOUS_TRANSPORT" => Some(KindOf::AmphibiousTransport),
            "CAPTURE" | "CAN_CAPTURE" => Some(KindOf::CanCapture),
            "SABOTEUR" => Some(KindOf::Saboteur),
            "HACKER" => Some(KindOf::Hacker),
            "HERO" => Some(KindOf::Hero),
            "KEYSTRUCTURE" | "KEY_STRUCTURE" => Some(KindOf::KeyStructure),
            "COMMANDCENTER" | "COMMAND_CENTER" => Some(KindOf::CommandCenter),
            "POWERPLANT" | "POWER_PLANT" => Some(KindOf::PowerPlant),
            "REFINERY" => Some(KindOf::Refinery),
            "FACTORY" => Some(KindOf::Factory),
            "DEFENSE" => Some(KindOf::Defense),
            "SHRUBBERY" => Some(KindOf::Shrubbery),
            "DOZER" => Some(KindOf::Dozer),
            "HULK" => Some(KindOf::Hulk),
            "SALVAGER" => Some(KindOf::Salvager),
            "WEAPONSALVAGER" | "WEAPON_SALVAGER" => Some(KindOf::WeaponSalvager),
            "ARMORSALVAGER" | "ARMOR_SALVAGER" => Some(KindOf::ArmorSalvager),
            "AIRCRAFTCARRIER" | "AIRCRAFT_CARRIER" => Some(KindOf::AircraftCarrier),
            "FSBARRACKS" | "FS_BARRACKS" => Some(KindOf::FSBarracks),
            "FSWARFACTORY" | "FS_WARFACTORY" => Some(KindOf::FSWarfactory),
            "FSAIRFIELD" | "FS_AIRFIELD" => Some(KindOf::FSAirfield),
            "FSINTERNETCENTER" | "FS_INTERNET_CENTER" => Some(KindOf::FSInternetCenter),
            "FSPOWER" | "FS_POWER" => Some(KindOf::FSPower),
            "FSSUPPLYDROPZONE" | "FS_SUPPLY_DROPZONE" => Some(KindOf::FSSupplyDropzone),
            "FSSUPPLYCENTER" | "FS_SUPPLY_CENTER" => Some(KindOf::FSSupplyCenter),
            "FSSUPERWEAPON" | "FS_SUPERWEAPON" => Some(KindOf::FSSuperweapon),
            "FSSTRATEGYCENTER" | "FS_STRATEGY_CENTER" => Some(KindOf::FSStrategyCenter),
            "COUNTSFORVICTORY" | "COUNTS_FOR_VICTORY" => Some(KindOf::CountsForVictory),
            "MINE" => Some(KindOf::Mine),
            "CAN_BE_REPULSED" | "CANBEREPULSED" => Some(KindOf::CanBeRepulsed),
            "EMP_HARDENED" | "EMPHARDENED" => Some(KindOf::EmpHardened),
            "SPAWNS_ARE_THE_WEAPONS" | "SPAWNSARETHEWEAPONS" => Some(KindOf::SpawnsAreTheWeapons),
            "IGNORE_DOCKING_BONES" | "IGNOREDOCKINGBONES" => Some(KindOf::IgnoreDockingBones),
            _ => None,
        }
    }

    let mut mask: KindOfMaskType = 0;
    for token in _tokens.iter().copied().filter(|t| *t != "=") {
        if token == label {
            continue;
        }
        if token.eq_ignore_ascii_case("ALL") {
            return KIND_OF_MASK_ALL;
        }
        if token.eq_ignore_ascii_case("NONE") {
            continue;
        }
        if let Some(kind) = parse_kind_of(token) {
            mask |= 1u64 << (kind as u32);
        } else {
            warn!(
                "DumbProjectileBehavior.{} unknown KindOf token '{}'",
                label, token
            );
        }
    }

    if mask == 0 {
        KIND_OF_MASK_NONE
    } else {
        mask
    }
}

const DUMB_PROJECTILE_FIELDS: &[FieldParse<DumbProjectileBehaviorModuleData>] = &[
    FieldParse {
        token: "MaxLifespan",
        parse: |_, data, tokens| {
            let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
            data.max_lifespan = parse_duration_frames(value)?;
            Ok(())
        },
    },
    FieldParse {
        token: "TumbleRandomly",
        parse: |ini, data, tokens| parse_bool_field(ini, &mut |v| data.tumble_randomly = v, tokens),
    },
    FieldParse {
        token: "DetonateCallsKill",
        parse: |_ini, data, tokens| {
            parse_bool_field(_ini, &mut |v| data.detonate_calls_kill = v, tokens)
        },
    },
    FieldParse {
        token: "OrientToFlightPath",
        parse: |ini, data, tokens| {
            parse_bool_field(ini, &mut |v| data.orient_to_flight_path = v, tokens)
        },
    },
    FieldParse {
        token: "FirstHeight",
        parse: |ini, data, tokens| parse_real_field(ini, &mut |v| data.first_height = v, tokens),
    },
    FieldParse {
        token: "SecondHeight",
        parse: |ini, data, tokens| parse_real_field(ini, &mut |v| data.second_height = v, tokens),
    },
    FieldParse {
        token: "FirstPercentIndent",
        parse: |ini, data, tokens| {
            parse_percent_field(ini, &mut |v| data.first_percent_indent = v, tokens)
        },
    },
    FieldParse {
        token: "SecondPercentIndent",
        parse: |ini, data, tokens| {
            parse_percent_field(ini, &mut |v| data.second_percent_indent = v, tokens)
        },
    },
    FieldParse {
        token: "GarrisonHitKillCount",
        parse: |_ini, data, tokens| {
            let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
            data.garrison_hit_kill_count =
                value.parse::<Int>().map_err(|_| INIError::InvalidData)?;
            Ok(())
        },
    },
    FieldParse {
        token: "GarrisonHitKillRequiredKindOf",
        parse: |_ini, data, tokens| {
            data.garrison_hit_kill_kindof =
                parse_kind_of_mask("GarrisonHitKillRequiredKindOf", tokens);
            Ok(())
        },
    },
    FieldParse {
        token: "GarrisonHitKillForbiddenKindOf",
        parse: |_ini, data, tokens| {
            data.garrison_hit_kill_kindof_not =
                parse_kind_of_mask("GarrisonHitKillForbiddenKindOf", tokens);
            Ok(())
        },
    },
    FieldParse {
        token: "GarrisonHitKillFX",
        parse: parse_fx_field,
    },
    FieldParse {
        token: "FlightPathAdjustDistPerSecond",
        parse: |ini, data, tokens| {
            parse_velocity_field(
                ini,
                &mut |v| data.flight_path_adjust_dist_per_frame = v,
                tokens,
            )
        },
    },
];

// -------------------------------------------------------------------------------------------------
// Module data definitions
// -------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct UpdateModuleData {
    module_tag_name_key: NameKeyType,
}

impl UpdateModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
        }
    }
}

impl ThingModuleData for UpdateModuleData {
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

impl Snapshotable for UpdateModuleData {
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

#[derive(Clone, Debug)]
pub struct DumbProjectileBehaviorModuleData {
    module_tag_name_key: NameKeyType,
    pub base: UpdateModuleData,
    pub first_height: Real,
    pub second_height: Real,
    pub first_percent_indent: Real,
    pub second_percent_indent: Real,
    pub max_lifespan: UnsignedInt,
    pub tumble_randomly: Bool,
    pub orient_to_flight_path: Bool,
    pub detonate_calls_kill: Bool,
    pub garrison_hit_kill_count: Int,
    pub garrison_hit_kill_kindof: KindOfMaskType,
    pub garrison_hit_kill_kindof_not: KindOfMaskType,
    pub garrison_hit_kill_fx: Option<Arc<FXList>>,
    pub flight_path_adjust_dist_per_frame: Real,
}

impl DumbProjectileBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            module_tag_name_key: 0,
            base: UpdateModuleData::new(),
            first_height: 0.0,
            second_height: 0.0,
            first_percent_indent: 0.0,
            second_percent_indent: 0.0,
            max_lifespan: (10 * LOGICFRAMES_PER_SECOND) as UnsignedInt,
            tumble_randomly: false,
            orient_to_flight_path: true,
            detonate_calls_kill: false,
            garrison_hit_kill_count: 0,
            garrison_hit_kill_kindof: KIND_OF_MASK_NONE,
            garrison_hit_kill_kindof_not: KIND_OF_MASK_NONE,
            garrison_hit_kill_fx: None,
            flight_path_adjust_dist_per_frame: 0.0,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        if let Err(err) = ini.init_from_ini_with_fields(self, DUMB_PROJECTILE_FIELDS) {
            warn!(
                "DumbProjectileBehavior failed to parse module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
        Ok(())
    }
}

impl Default for DumbProjectileBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ThingModuleData for DumbProjectileBehaviorModuleData {
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

impl Snapshotable for DumbProjectileBehaviorModuleData {
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

// -------------------------------------------------------------------------------------------------
// Behavior implementation
// -------------------------------------------------------------------------------------------------

#[derive(Default)]
pub struct DumbProjectileBehavior {
    module_data: Arc<DumbProjectileBehaviorModuleData>,
    object_id: ObjectID,
    object_handle: Mutex<Option<Weak<RwLock<GameObject>>>>,
    launcher_id: ObjectID,
    victim_id: ObjectID,
    detonation_weapon: Option<Arc<WeaponTemplate>>,
    extra_bonus_flags: WeaponBonusConditionFlags,
    target_pos: Option<Coord3D>,
    flight_path_segments: usize,
    flight_path_speed: Real,
    flight_path_start: Coord3D,
    flight_path_end: Coord3D,
    flight_path: Vec<Coord3D>,
    current_step: UnsignedInt,
    lifespan_frame: UnsignedInt,
    has_detonated: Bool,
}

impl DumbProjectileBehavior {
    fn construct_with_object(
        object_id: ObjectID,
        module_data: Arc<DumbProjectileBehaviorModuleData>,
        object: Option<Arc<RwLock<GameObject>>>,
    ) -> Self {
        let handle = object
            .or_else(|| OBJECT_REGISTRY.get_object(object_id))
            .map(|obj| Arc::downgrade(&obj));

        Self {
            module_data,
            object_id,
            object_handle: Mutex::new(handle),
            launcher_id: OBJECT_INVALID_ID,
            victim_id: OBJECT_INVALID_ID,
            detonation_weapon: None,
            extra_bonus_flags: WeaponBonusConditionFlags::none(),
            target_pos: None,
            flight_path_segments: 0,
            flight_path_speed: 0.0,
            flight_path_start: Coord3D::new(0.0, 0.0, 0.0),
            flight_path_end: Coord3D::new(0.0, 0.0, 0.0),
            flight_path: Vec::new(),
            current_step: 0,
            lifespan_frame: 0,
            has_detonated: false,
        }
    }

    pub fn new_from_object(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<DumbProjectileBehaviorModuleData>,
    ) -> Self {
        let object_id = object
            .read()
            .map(|obj| obj.get_id())
            .unwrap_or(OBJECT_INVALID_ID);
        Self::construct_with_object(object_id, module_data, Some(object))
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<DumbProjectileBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "DumbProjectileBehavior requires an owning object".to_string())?;

        let object_id = module_object.get_object_id();
        let object = OBJECT_REGISTRY
            .get_object(object_id)
            .ok_or_else(|| format!("DumbProjectileBehavior missing object {}", object_id))?;

        Ok(Self::new_from_object(object, module_data))
    }

    fn get_object(
        &self,
    ) -> Result<Arc<RwLock<GameObject>>, Box<dyn std::error::Error + Send + Sync>> {
        if self.object_id == OBJECT_INVALID_ID {
            return Err("DumbProjectileBehavior missing owning object id".into());
        }

        if let Ok(mut handle) = self.object_handle.lock() {
            if let Some(weak) = handle.as_ref() {
                if let Some(object) = weak.upgrade() {
                    return Ok(object);
                }
            }

            if let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) {
                *handle = Some(Arc::downgrade(&object));
                return Ok(object);
            }
        }

        Err(format!(
            "DumbProjectileBehavior unable to upgrade handle for object {}",
            self.object_id
        )
        .into())
    }

    fn get_current_frame(&self) -> UnsignedInt {
        TheGameLogic::get_frame()
    }

    /// Initialize flight path using Bezier curve calculation
    /// Matches C++ DumbProjectileBehavior::calcFlightPath() from DumbProjectileBehavior.cpp:389-435
    fn init_flight_path(
        &mut self,
        object: &Arc<RwLock<GameObject>>,
        recalc_num_segments: bool,
        reset_step: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use crate::weapon::bezier::BezierSegment;

        let (start, target) = {
            let obj_guard = object.read().map_err(|_| {
                std::io::Error::other("DumbProjectileBehavior failed to read owning object")
            })?;
            let start = if self.flight_path_segments == 0 && self.current_step == 0 {
                *obj_guard.get_position()
            } else {
                self.flight_path_start
            };
            (start, self.flight_path_end)
        };
        self.flight_path_start = start;

        // Calculate highest terrain along path (C++ lines 382-387)
        // Matches C++ PartitionManager::estimateTerrainExtremesAlongLine usage.
        let mut highest_intervening = 0.0;
        if let Some(partition) = ThePartitionManager::get() {
            if !partition.estimate_terrain_extremes_along_line(
                start,
                target,
                &mut highest_intervening,
            ) {
                return Err("DumbProjectileBehavior calcFlightPath off-map".into());
            }
        }
        let highest_terrain = highest_intervening.max(start.z).max(target.z);

        // Create Bezier arc using module parameters (C++ lines 389-435)
        let bezier = BezierSegment::create_projectile_arc(
            start,
            target,
            self.module_data.first_height,
            self.module_data.second_height,
            self.module_data.first_percent_indent.clamp(0.0, 1.0),
            self.module_data.second_percent_indent.clamp(0.0, 1.0),
            highest_terrain,
        );

        // Generate flight path points (C++ lines 438-445)
        // Calculate number of steps based on arc length and flight speed
        if recalc_num_segments {
            self.flight_path_segments = 0;
        }
        if self.flight_path_segments == 0 {
            let arc_length = bezier.get_approximate_length();
            let speed = self.flight_path_speed.max(1.0);
            self.flight_path_segments = (arc_length / speed).ceil() as usize;
            if self.flight_path_segments == 0 {
                self.flight_path_segments = 1;
            }
        }

        // Get evenly spaced points along the curve
        self.flight_path = bezier.get_segment_points(self.flight_path_segments);
        if reset_step {
            self.current_step = 0;
        }

        Ok(())
    }

    /// Advance projectile one step along flight path
    /// Matches C++ DumbProjectileBehavior::Update() movement logic
    fn advance_one_step(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.flight_path.is_empty() {
            let object = self.get_object()?;
            self.init_flight_path(&object, true, true)?;
        }

        if (self.current_step as usize) >= self.flight_path.len() {
            // Reached end of path - detonate
            self.detonate()?;
            return Ok(());
        }

        let step = self.flight_path[self.current_step as usize];
        let object = self.get_object()?;
        let mut obj_guard = object.write().map_err(|_| {
            std::io::Error::other("DumbProjectileBehavior failed to write owning object")
        })?;

        // Update position (C++ line 453)
        let old_pos = *obj_guard.get_position();
        obj_guard.set_position(&step)?;

        // Orient to flight path if configured (C++ lines 455-460)
        if self.module_data.orient_to_flight_path && !self.module_data.tumble_randomly {
            let (prev_pos, cur_pos) = if (self.current_step as usize) > 0 {
                let prev = self.flight_path[(self.current_step - 1) as usize];
                (prev, step)
            } else if self.flight_path.len() > 1 {
                (self.flight_path[0], self.flight_path[1])
            } else {
                (old_pos, step)
            };

            let direction = Coord3D::new(
                cur_pos.x - prev_pos.x,
                cur_pos.y - prev_pos.y,
                cur_pos.z - prev_pos.z,
            );

            if direction.length() > 0.001 {
                // Build transform matrix aligned to flight direction (matches buildTransformMatrix usage).
                let forward = direction.normalize();
                let mut up = Coord3D::new(0.0, 0.0, 1.0);
                let mut right = forward.cross(up);
                if right.length() < 0.001 {
                    up = Coord3D::new(0.0, 1.0, 0.0);
                    right = forward.cross(up);
                }
                let right = right.normalize();
                let corrected_up = right.cross(forward);
                let transform = crate::common::Matrix3D::from_cols(
                    Vec4::new(right.x, right.y, right.z, 0.0),
                    Vec4::new(corrected_up.x, corrected_up.y, corrected_up.z, 0.0),
                    Vec4::new(forward.x, forward.y, forward.z, 0.0),
                    Vec4::new(step.x, step.y, step.z, 1.0),
                );
                obj_guard.set_transform_matrix(&transform);
            }
        }

        // Tumble randomly if configured (C++ lines 462-465)
        if self.module_data.tumble_randomly {
            // Random tumbling is set up during projectile launch in projectileFireAtObjectOrPosition
            // The PhysicsBehavior handles the actual rotation updates each frame
            // Matches C++ DumbProjectileBehavior.cpp:363-368
            // Note: Physics system integration pending - tumble rates would be applied there
        }

        // Update layer and detect bridge transition (C++ lines 654-669).
        let old_layer = obj_guard.get_layer();
        if let Ok(terrain) = crate::terrain::THE_TERRAIN_LOGIC.read() {
            let new_layer_path =
                terrain.get_highest_layer_for_destination(obj_guard.get_position());
            let new_layer = terrain_layer_to_logic_layer(new_layer_path);
            obj_guard.set_layer(new_layer);

            if old_layer != PathfindLayerEnum::Ground && new_layer == PathfindLayerEnum::Ground {
                let mut tmp = *obj_guard.get_position();
                tmp.z = 9999.0;
                let test_layer = terrain.get_highest_layer_for_destination(&tmp);
                if terrain_layer_to_logic_layer(test_layer) == old_layer {
                    const FUDGE: Real = 2.0;
                    tmp.z = terrain.get_layer_height(tmp.x, tmp.y, test_layer, None, true) + FUDGE;
                    let _ = obj_guard.set_position(&tmp);
                    drop(obj_guard);
                    self.detonate()?;
                    return Ok(());
                }
            }
        }

        drop(obj_guard);

        // Check for collision with terrain (C++ lines 467-475)
        self.check_collision(&step)?;

        self.current_step += 1;
        Ok(())
    }

    /// Check for collision at given position
    /// Matches C++ DumbProjectileBehavior collision detection logic
    fn check_collision(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check object collisions first (matches DumbProjectileBehavior::projectileHandleCollision).
        if let Some(partition) = ThePartitionManager::get() {
            let candidates = partition.get_objects_in_range_boundary_2d(pos, 0.0);
            for candidate_id in candidates {
                if candidate_id == self.object_id {
                    continue;
                }
                let Some(other_arc) = OBJECT_REGISTRY.get_object(candidate_id) else {
                    continue;
                };
                let Ok(other_guard) = other_arc.read() else {
                    continue;
                };

                if other_guard.is_effectively_dead() {
                    continue;
                }

                if let Some(template) = &self.detonation_weapon {
                    if !template.should_projectile_collide_with(
                        self.launcher_id,
                        self.object_id,
                        candidate_id,
                        self.victim_id,
                    ) {
                        continue;
                    }
                }

                if self.module_data.garrison_hit_kill_count > 0 {
                    if let Some(contain_handle) = other_guard.get_contain() {
                        if let Ok(contain_guard) = contain_handle.lock() {
                            let garrisoned = contain_guard.get_contained_count() > 0;
                            let garrisonable = contain_guard.is_garrisonable();
                            let immune = other_guard
                                .get_garrison_contain_module_data()
                                .ok()
                                .map(|data| data.immune_to_clear_building_attacks)
                                .unwrap_or(false);
                            if garrisoned && garrisonable && !immune {
                                let contained_ids = contain_guard.get_contained_objects().to_vec();
                                let mut num_killed = 0;
                                drop(contain_guard);

                                for contained_id in contained_ids {
                                    if num_killed >= self.module_data.garrison_hit_kill_count {
                                        break;
                                    }
                                    let Some(contained_arc) =
                                        OBJECT_REGISTRY.get_object(contained_id)
                                    else {
                                        continue;
                                    };
                                    let Ok(mut contained_guard) = contained_arc.write() else {
                                        continue;
                                    };
                                    if contained_guard.is_effectively_dead() {
                                        continue;
                                    }
                                    if !contained_guard.is_kind_of_multi(
                                        self.module_data.garrison_hit_kill_kindof,
                                        self.module_data.garrison_hit_kill_kindof_not,
                                    ) {
                                        continue;
                                    }

                                    if self.launcher_id != OBJECT_INVALID_ID {
                                        if let Some(launcher_arc) =
                                            OBJECT_REGISTRY.get_object(self.launcher_id)
                                        {
                                            if let Ok(mut launcher_guard) = launcher_arc.write() {
                                                launcher_guard.score_the_kill(&contained_guard);
                                            }
                                        }
                                    }
                                    contained_guard.kill(None, None);
                                    num_killed += 1;
                                }

                                if num_killed > 0 {
                                    if let Some(fx) = &self.module_data.garrison_hit_kill_fx {
                                        let _ = fx.do_fx_obj(&other_arc, None);
                                    }

                                    if let Ok(other_guard) = other_arc.read() {
                                        if let Some(player_arc) =
                                            other_guard.get_controlling_player()
                                        {
                                            if let Ok(mut player_guard) = player_arc.write() {
                                                player_guard
                                                    .get_academy_stats_mut()
                                                    .record_cleared_garrisoned_building();
                                            }
                                        }
                                    }

                                    if let Ok(projectile_guard) = self.get_object() {
                                        let guard = projectile_guard.read().map_err(|_| {
                                            "DumbProjectileBehavior failed to read projectile"
                                        })?;
                                        TheGameLogic::destroy_object(&*guard)?;
                                    }
                                    return Ok(());
                                }
                            }
                        }
                    }
                }

                self.detonate()?;

                if let Ok(projectile_guard) = self.get_object() {
                    if let Ok(mut projectile) = projectile_guard.write() {
                        projectile.set_status(ObjectStatusMaskType::NO_COLLISIONS, true);
                    }
                }

                return Ok(());
            }
        }

        Ok(())
    }

    /// Detonate projectile and trigger weapon effects
    /// Matches C++ DumbProjectileBehavior::Detonation() from DumbProjectileBehavior.cpp:505-532
    fn detonate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.has_detonated {
            return Ok(());
        }

        let object = match self.get_object() {
            Ok(obj) => obj,
            Err(_) => return Ok(()), // Object already destroyed
        };

        let obj_guard = object.read().map_err(|_| {
            std::io::Error::other("DumbProjectileBehavior failed to read object for detonation")
        })?;

        let position = *obj_guard.get_position();
        let object_id = obj_guard.get_id();
        drop(obj_guard);

        if let Some(weapon_template) = &self.detonation_weapon {
            let detonation_result = crate::weapon::with_weapon_store(|store| {
                store.handle_projectile_detonation(
                    weapon_template,
                    object_id,
                    &position,
                    self.extra_bonus_flags,
                    true,
                )
            })
            .map_err(|err| std::io::Error::other(err.to_string()))?;
            detonation_result.map_err(|err| std::io::Error::other(err.to_string()))?;

            if self.module_data.detonate_calls_kill {
                let max_health = object
                    .read()
                    .map_err(|_| {
                        std::io::Error::other(
                            "DumbProjectileBehavior failed to read object for max health",
                        )
                    })?
                    .get_max_health();
                let mut damage_info = crate::damage::DamageInfo::with_simple(
                    max_health,
                    OBJECT_INVALID_ID,
                    crate::damage::DamageType::Unresistable,
                    crate::damage::DeathType::Detonated,
                );
                damage_info.sync_from_input();
                let mut obj_guard = object.write().map_err(|_| {
                    std::io::Error::other("DumbProjectileBehavior failed to write object for kill")
                })?;
                obj_guard.attempt_damage(&mut damage_info).map_err(
                    |err: Box<dyn std::error::Error + Send + Sync>| {
                        std::io::Error::other(err.to_string())
                    },
                )?;
            } else {
                let obj_guard = object.read().map_err(|_| {
                    std::io::Error::other(
                        "DumbProjectileBehavior failed to read object for destroy",
                    )
                })?;
                crate::helpers::TheGameLogic::destroy_object(&*obj_guard)?;
            }
        } else {
            let max_health = object
                .read()
                .map_err(|_| {
                    std::io::Error::other(
                        "DumbProjectileBehavior failed to read object for max health",
                    )
                })?
                .get_max_health();
            let mut damage_info = crate::damage::DamageInfo::with_simple(
                max_health,
                OBJECT_INVALID_ID,
                crate::damage::DamageType::Unresistable,
                crate::damage::DeathType::Detonated,
            );
            damage_info.sync_from_input();
            let mut obj_guard = object.write().map_err(|_| {
                std::io::Error::other(
                    "DumbProjectileBehavior failed to write object for detonation",
                )
            })?;
            obj_guard.attempt_damage(&mut damage_info).map_err(
                |err: Box<dyn std::error::Error + Send + Sync>| {
                    std::io::Error::other(err.to_string())
                },
            )?;
        }

        if let Ok(obj_guard) = object.read() {
            if let Some(drawable) = obj_guard.get_drawable() {
                drawable.set_drawable_hidden(true);
            }
        }

        self.has_detonated = true;

        Ok(())
    }

    /// Set projectile target victim
    /// Matches C++ ProjectileUpdate::SetVictimObjectID()
    pub fn set_victim_id(&mut self, victim_id: ObjectID) {
        self.victim_id = victim_id;
    }

    /// Set projectile launcher
    /// Matches C++ ProjectileUpdate::SetLauncherObjectID()
    pub fn set_launcher_id(&mut self, launcher_id: ObjectID) {
        self.launcher_id = launcher_id;
    }

    /// Launch projectile toward an object or position (C++ parity helper).
    pub fn projectile_launch_at_object_or_position(
        &mut self,
        victim: Option<ObjectID>,
        victim_pos: &Coord3D,
        launcher: ObjectID,
        detonation_weapon: Option<Arc<WeaponTemplate>>,
    ) {
        self.set_launcher_id(launcher);
        self.victim_id = victim.unwrap_or(OBJECT_INVALID_ID);
        self.target_pos = Some(*victim_pos);
        self.detonation_weapon = detonation_weapon;
        self.extra_bonus_flags = if launcher != OBJECT_INVALID_ID {
            OBJECT_REGISTRY
                .get_object(launcher)
                .and_then(|arc| arc.read().ok().map(|obj| obj.get_weapon_bonus_condition()))
                .unwrap_or_else(WeaponBonusConditionFlags::none)
        } else {
            WeaponBonusConditionFlags::none()
        };
        self.lifespan_frame = self.get_current_frame() + self.module_data.max_lifespan;
        if let Ok(projectile) = self.get_object() {
            if let Ok(obj_guard) = projectile.read() {
                self.flight_path_start = *obj_guard.get_position();
            }
            if self.module_data.tumble_randomly {
                if let Ok(obj_guard) = projectile.write() {
                    if let Some(physics) = obj_guard.get_physics() {
                        if let Ok(mut phys_guard) = physics.lock() {
                            let min = -1.0 / std::f32::consts::PI;
                            let max = 1.0 / std::f32::consts::PI;
                            phys_guard.set_pitch_rate(get_game_logic_random_value_real(min, max));
                            phys_guard.set_yaw_rate(get_game_logic_random_value_real(min, max));
                            phys_guard.set_roll_rate(get_game_logic_random_value_real(min, max));
                        }
                    }
                }
            }
        }
        let end_pos = if let Some(victim_id) = victim {
            if let Some(victim_arc) = OBJECT_REGISTRY.get_object(victim_id) {
                if let Ok(victim_guard) = victim_arc.read() {
                    victim_guard
                        .get_geometry_info()
                        .get_center_position(victim_guard.get_position())
                } else {
                    *victim_pos
                }
            } else {
                *victim_pos
            }
        } else {
            *victim_pos
        };
        self.flight_path_end = end_pos;
        if let Some(weapon) = &self.detonation_weapon {
            if weapon.is_scale_weapon_speed {
                let min_range = weapon.get_minimum_attack_range();
                let max_range = weapon.get_unmodified_attack_range();
                let range = if let Ok(projectile) = self.get_object() {
                    projectile
                        .read()
                        .ok()
                        .map(|guard| {
                            ThePartitionManager::get_distance_squared_to_pos(
                                &*guard,
                                &end_pos,
                                crate::common::FROM_CENTER_2D,
                            )
                            .sqrt()
                        })
                        .unwrap_or(0.0)
                } else {
                    0.0
                };
                let mut range_ratio = 1.0;
                if max_range > min_range {
                    range_ratio = (range - min_range) / (max_range - min_range);
                }
                self.flight_path_speed = (range_ratio
                    * (weapon.weapon_speed - weapon.min_weapon_speed))
                    + weapon.min_weapon_speed;
            } else {
                self.flight_path_speed = weapon.weapon_speed;
            }
        } else {
            self.flight_path_speed = 0.0;
        }
        self.flight_path_segments = 0;
        self.flight_path.clear();
        self.current_step = 0;
        if let Ok(projectile) = self.get_object() {
            if self.init_flight_path(&projectile, true, true).is_err() {
                if let Ok(obj_guard) = projectile.read() {
                    let _ = TheGameLogic::destroy_object(&*obj_guard);
                }
            }
        }
    }

    /// Get current flight path progress (0.0 to 1.0)
    pub fn get_flight_progress(&self) -> Real {
        if self.flight_path.is_empty() {
            return 0.0;
        }
        (self.current_step as Real) / (self.flight_path.len() as Real)
    }

    /// Check if projectile has completed its flight
    pub fn is_flight_complete(&self) -> bool {
        if self.flight_path.is_empty() {
            return false;
        }
        self.current_step as usize >= self.flight_path.len()
    }
}

impl UpdateModuleInterface for DumbProjectileBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let now = self.get_current_frame();
        if self.lifespan_frame != 0 && now >= self.lifespan_frame {
            self.detonate()?;
            return Ok(crate::modules::UPDATE_SLEEP_NONE);
        }

        if !self.flight_path.is_empty() && (self.current_step as usize) >= self.flight_path.len() {
            self.detonate()?;
            return Ok(crate::modules::UPDATE_SLEEP_NONE);
        }

        if self.victim_id != OBJECT_INVALID_ID
            && self.module_data.flight_path_adjust_dist_per_frame > 0.0
        {
            if let Some(victim_arc) = OBJECT_REGISTRY.get_object(self.victim_id) {
                if let Ok(victim_guard) = victim_arc.read() {
                    let new_victim_pos = victim_guard
                        .get_geometry_info()
                        .get_center_position(victim_guard.get_position());
                    let delta = Coord3D::new(
                        new_victim_pos.x - self.flight_path_end.x,
                        new_victim_pos.y - self.flight_path_end.y,
                        new_victim_pos.z - self.flight_path_end.z,
                    );
                    let dist_sqr = delta.x * delta.x + delta.y * delta.y + delta.z * delta.z;
                    if dist_sqr > 0.1 {
                        let mut dist = dist_sqr.sqrt();
                        if dist > self.module_data.flight_path_adjust_dist_per_frame {
                            dist = self.module_data.flight_path_adjust_dist_per_frame;
                        }
                        if dist > 0.0 {
                            let inv = 1.0 / dist_sqr.sqrt();
                            self.flight_path_end.x += dist * delta.x * inv;
                            self.flight_path_end.y += dist * delta.y * inv;
                            self.flight_path_end.z += dist * delta.z * inv;
                            self.flight_path_segments = self.flight_path_segments.max(1);
                            let object = self.get_object()?;
                            if self.init_flight_path(&object, false, false).is_err() {
                                self.detonate()?;
                                return Ok(crate::modules::UPDATE_SLEEP_NONE);
                            }
                        }
                    }
                }
            }
        }

        self.advance_one_step()?;
        if self.has_detonated {
            return Ok(crate::modules::UPDATE_SLEEP_NONE);
        }
        Ok(UpdateSleepTime::None)
    }
}

impl ProjectileUpdateInterface for DumbProjectileBehavior {
    fn projectile_update(&mut self, _object_id: ObjectID, _delta_time: Real) {
        let _ = UpdateModuleInterface::update(self);
    }

    fn projectile_now_jammed(&mut self) {
        if let Some(object) = OBJECT_REGISTRY.get_object(self.object_id) {
            if let Ok(mut guard) = object.write() {
                guard.set_model_condition_state(MODELCONDITION_JAMMED);
            }
        }
    }
}

impl BehaviorModuleInterface for DumbProjectileBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_projectile_update_interface(&mut self) -> Option<&mut dyn ProjectileUpdateInterface> {
        Some(self)
    }
}

// -------------------------------------------------------------------------------------------------
// Module wrapper glue
// -------------------------------------------------------------------------------------------------

pub struct DumbProjectileBehaviorModule {
    behavior: DumbProjectileBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<DumbProjectileBehaviorModuleData>,
}

impl DumbProjectileBehaviorModule {
    pub fn new(
        behavior: DumbProjectileBehavior,
        module_name: &AsciiString,
        module_data: Arc<DumbProjectileBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut DumbProjectileBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for DumbProjectileBehaviorModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|err| err.to_string())?;

        let mut launcher_id = self.behavior.launcher_id;
        xfer.xfer_object_id(&mut launcher_id)
            .map_err(|err| err.to_string())?;
        self.behavior.launcher_id = launcher_id;

        let mut victim_id = self.behavior.victim_id;
        xfer.xfer_object_id(&mut victim_id)
            .map_err(|err| err.to_string())?;
        self.behavior.victim_id = victim_id;

        let mut segments = self.behavior.flight_path_segments as Int;
        xfer.xfer_int(&mut segments)
            .map_err(|err| err.to_string())?;
        if segments < 0 {
            segments = 0;
        }
        self.behavior.flight_path_segments = segments as usize;

        let mut speed = self.behavior.flight_path_speed;
        xfer.xfer_real(&mut speed).map_err(|err| err.to_string())?;
        self.behavior.flight_path_speed = speed;

        let mut start = self.behavior.flight_path_start;
        xfer.xfer_coord3d(&mut start);
        self.behavior.flight_path_start = start;

        let mut end = self.behavior.flight_path_end;
        xfer.xfer_coord3d(&mut end);
        self.behavior.flight_path_end = end;

        let mut weapon_name = self
            .behavior
            .detonation_weapon
            .as_ref()
            .map(|weapon| weapon.name.clone())
            .unwrap_or_default();
        xfer.xfer_ascii_string(&mut weapon_name)
            .map_err(|err| err.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            if weapon_name.is_empty() {
                self.behavior.detonation_weapon = None;
            } else {
                let template = crate::weapon::with_weapon_store(|store| {
                    store.find_weapon_template(&weapon_name).cloned()
                })
                .map_err(|err| err.to_string())?;
                let template = template.ok_or_else(|| {
                    format!(
                        "DumbProjectileBehavior::xfer missing template {}",
                        weapon_name
                    )
                })?;
                self.behavior.detonation_weapon = Some(template);
            }
        }

        let mut lifespan = self.behavior.lifespan_frame;
        xfer.xfer_unsigned_int(&mut lifespan)
            .map_err(|err| err.to_string())?;
        self.behavior.lifespan_frame = lifespan;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl EngineModule for DumbProjectileBehaviorModule {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

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

pub fn dumb_projectile_behavior_module_data_factory(
    ini: Option<&mut INI>,
) -> Box<dyn ThingModuleData> {
    let mut data = DumbProjectileBehaviorModuleData::default();

    if let Some(ini) = ini {
        if let Err(err) = data.parse_from_ini(ini) {
            warn!(
                "Failed to parse DumbProjectileBehavior module data at line {}: {}",
                ini.get_line_num(),
                err
            );
        }
    }

    Box::new(data)
}

pub fn dumb_projectile_behavior_module_factory(
    thing: Arc<dyn ModuleThing>,
    module_data: Arc<dyn ThingModuleData>,
) -> Box<dyn EngineModule> {
    let typed = module_data
        .as_any()
        .downcast_ref::<DumbProjectileBehaviorModuleData>()
        .expect("DumbProjectileBehaviorModuleData expected");

    let shared = Arc::new(typed.clone());
    let behavior =
        DumbProjectileBehavior::from_module_thing(Arc::clone(&thing), Arc::clone(&shared))
            .expect("DumbProjectileBehavior requires an owning object");

    let module_name = AsciiString::from("DumbProjectileBehavior");
    Box::new(DumbProjectileBehaviorModule::new(
        behavior,
        &module_name,
        shared,
    ))
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::thing::module::ModuleData as ThingModuleDataTrait;

    #[derive(Debug, Clone)]
    struct StubThing {
        object_id: ObjectID,
    }

    impl ModuleObject for StubThing {
        fn get_object_id(&self) -> ObjectID {
            self.object_id
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
    fn data_factory_sets_defaults() {
        let data_box = dumb_projectile_behavior_module_data_factory(None);
        let typed = data_box
            .as_ref()
            .as_any()
            .downcast_ref::<DumbProjectileBehaviorModuleData>()
            .expect("dumb projectile data");
        assert_eq!(
            typed.max_lifespan,
            (10 * LOGICFRAMES_PER_SECOND) as UnsignedInt
        );
        assert!(typed.garrison_hit_kill_fx.is_none());
    }

    #[test]
    fn module_factory_downcasts() {
        use crate::object::registry::OBJECT_REGISTRY;
        use crate::object::Object;

        OBJECT_REGISTRY.clear();
        let object_id = 1;
        let object = Arc::new(RwLock::new(Object::new_test(object_id, 100.0)));
        OBJECT_REGISTRY.register_object(object_id, &object);

        let data =
            Arc::new(DumbProjectileBehaviorModuleData::default()) as Arc<dyn ThingModuleData>;
        let module =
            dumb_projectile_behavior_module_factory(Arc::new(StubThing { object_id }), data);
        assert!(module
            .get_module_data()
            .as_any()
            .downcast_ref::<DumbProjectileBehaviorModuleData>()
            .is_some());
    }

    #[test]
    fn parse_fx_field_keeps_missing_reference_none() {
        let mut data = DumbProjectileBehaviorModuleData::default();
        let mut ini = INI::new();
        parse_fx_field(
            &mut ini,
            &mut data,
            &["MissingDumbProjectileFx_ParityTest_20260302"],
        )
        .expect("parse should succeed");
        assert!(data.garrison_hit_kill_fx.is_none());
    }

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames("1500ms").expect("duration"), 45);
        assert_eq!(parse_duration_frames("1.5s").expect("duration"), 45);
    }
}
