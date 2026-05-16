//! Open Contain Module
//!
//! The base OpenContainer ContainModule allows objects to be contained inside of other
//! objects. This provides the fundamental containment functionality that is common to
//! all container modules.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::ai::THE_AI;
use crate::common::audio::AudioEventRts;
use crate::common::{
    CommandSourceType, Coord3D, GameResult, KindOf, KindOfMaskType, Matrix3D, ModelConditionFlags,
    ObjectID, PathfindLayerEnum, PlayerMaskType, TurretType, UnsignedInt, LOGICFRAMES_PER_SECOND,
    MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING,
};
use crate::damage::DamageInfo;
use crate::error::GameLogicError as GameError;
use crate::helpers::{TheAudio, TheGameLogic, TheTerrainLogic};
use crate::modules::{
    AIUpdateInterfaceExt, ContainModuleInterface, ContainWant, ExitDoorType, PhysicsBehaviorExt,
    UpdateSleepTime,
};
use crate::object::behavior::auto_heal_behavior::parse_kind_of_mask;
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::die::{
    parse_death_type_flags_tokens, parse_object_status_mask_tokens,
    parse_veterancy_level_flags_tokens, DieMuxData,
};
use crate::object::drawable::DrawableArcExt;
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

type ObjectId = ObjectID;
type FirePointMatrix = [[f32; 4]; 3];

struct ExitPrep {
    owner_id: ObjectID,
    end_pos: Coord3D,
    exit_path: Vec<Coord3D>,
}

/// Constant for unlimited contain capacity
pub const CONTAIN_MAX_UNKNOWN: i32 = -1;
const MAX_FIRE_POINTS: usize = 32;

/// Configuration data for OpenContain module
#[derive(Debug, Clone)]
pub struct OpenContainModuleData {
    /// Die mux data for filtering on death
    pub die_mux_data: DieMuxData,
    /// Maximum number of contained objects (-1 = unlimited)
    pub contain_max: i32,
    /// Sound to play when entering container
    pub enter_sound: Option<AudioEventRts>,
    /// Sound to play when exiting container
    pub exit_sound: Option<AudioEventRts>,
    /// Can passengers shoot out of container
    pub passengers_allowed_to_fire: bool,
    /// Firepoint bones are in turret, not chassis
    pub passengers_in_turret: bool,
    /// Number of exit paths to alternate through
    pub number_of_exit_paths: i32,
    /// Damage percentage passed to contained units
    pub damage_percentage_to_units: f32,
    /// Turn off hardcoded burn death for contained units
    pub is_burned_death_to_units: bool,
    /// Door open time in frames
    pub door_open_time: u32,
    /// Objects must have at least one of these kind bits to be contained
    pub allow_inside_kind_of: KindOfMaskType,
    /// Objects must have NONE of these kind bits to be contained
    pub forbid_inside_kind_of: KindOfMaskType,
    /// Do passengers get container's weapon bonuses
    pub weapon_bonus_passed_to_passengers: bool,
    /// Allow allies inside container
    pub allow_allies_inside: bool,
    /// Allow enemies inside container
    pub allow_enemies_inside: bool,
    /// Allow neutral units inside container
    pub allow_neutral_inside: bool,
}

impl Default for OpenContainModuleData {
    fn default() -> Self {
        Self {
            die_mux_data: DieMuxData::default(),
            contain_max: CONTAIN_MAX_UNKNOWN,
            enter_sound: None,
            exit_sound: None,
            passengers_allowed_to_fire: false,
            passengers_in_turret: false,
            number_of_exit_paths: 1,
            damage_percentage_to_units: 0.0,
            is_burned_death_to_units: true,
            door_open_time: 1,
            allow_inside_kind_of: 0,
            forbid_inside_kind_of: 0,
            weapon_bonus_passed_to_passengers: false,
            allow_allies_inside: true,
            allow_enemies_inside: false,
            allow_neutral_inside: false,
        }
    }
}

impl OpenContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, OPEN_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        super::parse_with_fields_allow_unknown(config, self, OPEN_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for OpenContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        OpenContainModuleData::parse_from_config(self, config)
    }
}

impl Snapshotable for OpenContainModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| e.to_string())?;
        let mut contain_max = self.contain_max;
        xfer.xfer_int(&mut contain_max).map_err(|e| e.to_string())?;
        self.contain_max = contain_max;
        let mut passengers_allowed_to_fire = self.passengers_allowed_to_fire;
        xfer.xfer_bool(&mut passengers_allowed_to_fire)
            .map_err(|e| e.to_string())?;
        self.passengers_allowed_to_fire = passengers_allowed_to_fire;
        let mut door_open_time = self.door_open_time as i32;
        xfer.xfer_int(&mut door_open_time).map_err(|e| e.to_string())?;
        self.door_open_time = door_open_time as u32;
        let mut allow_inside_kind_of = self.allow_inside_kind_of as u32;
        xfer.xfer_unsigned_int(&mut allow_inside_kind_of)
            .map_err(|e| e.to_string())?;
        self.allow_inside_kind_of = allow_inside_kind_of as u64;
        let mut forbid_inside_kind_of = self.forbid_inside_kind_of as u32;
        xfer.xfer_unsigned_int(&mut forbid_inside_kind_of)
            .map_err(|e| e.to_string())?;
        self.forbid_inside_kind_of = forbid_inside_kind_of as u64;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn parse_contain_max(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.contain_max = INI::parse_int(token)?;
    Ok(())
}

fn parse_enter_sound(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.enter_sound = None;
    } else {
        data.enter_sound = Some(AudioEventRts::new(*token));
    }
    Ok(())
}

fn parse_exit_sound(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.exit_sound = None;
    } else {
        data.exit_sound = Some(AudioEventRts::new(*token));
    }
    Ok(())
}

fn parse_damage_percent_to_units(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.damage_percentage_to_units = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_burned_death_to_units(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.is_burned_death_to_units = INI::parse_bool(token)?;
    Ok(())
}

fn parse_allow_inside_kind_of(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.allow_inside_kind_of = parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_forbid_inside_kind_of(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.forbid_inside_kind_of = parse_kind_of_mask(tokens);
    Ok(())
}

fn parse_passengers_allowed_to_fire(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.passengers_allowed_to_fire = INI::parse_bool(token)?;
    Ok(())
}

fn parse_passengers_in_turret(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.passengers_in_turret = INI::parse_bool(token)?;
    Ok(())
}

fn parse_number_of_exit_paths(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.number_of_exit_paths = INI::parse_int(token)?;
    Ok(())
}

fn parse_door_open_time(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.door_open_time = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_weapon_bonus_passed_to_passengers(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.weapon_bonus_passed_to_passengers = INI::parse_bool(token)?;
    Ok(())
}

fn parse_allow_allies_inside(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.allow_allies_inside = INI::parse_bool(token)?;
    Ok(())
}

fn parse_allow_enemies_inside(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.allow_enemies_inside = INI::parse_bool(token)?;
    Ok(())
}

fn parse_allow_neutral_inside(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.allow_neutral_inside = INI::parse_bool(token)?;
    Ok(())
}

fn parse_death_types(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.death_types = parse_death_type_flags_tokens(tokens)?;
    Ok(())
}

fn parse_veterancy_levels(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.veterancy_levels = parse_veterancy_level_flags_tokens(tokens)?;
    Ok(())
}

fn parse_exempt_status(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.exempt_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

fn parse_required_status(
    _ini: &mut INI,
    data: &mut OpenContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.die_mux_data.required_status = parse_object_status_mask_tokens(tokens)?;
    Ok(())
}

const OPEN_CONTAIN_FIELDS: &[FieldParse<OpenContainModuleData>] = &[
    FieldParse {
        token: "ContainMax",
        parse: parse_contain_max,
    },
    FieldParse {
        token: "EnterSound",
        parse: parse_enter_sound,
    },
    FieldParse {
        token: "ExitSound",
        parse: parse_exit_sound,
    },
    FieldParse {
        token: "DamagePercentToUnits",
        parse: parse_damage_percent_to_units,
    },
    FieldParse {
        token: "BurnedDeathToUnits",
        parse: parse_burned_death_to_units,
    },
    FieldParse {
        token: "AllowInsideKindOf",
        parse: parse_allow_inside_kind_of,
    },
    FieldParse {
        token: "ForbidInsideKindOf",
        parse: parse_forbid_inside_kind_of,
    },
    FieldParse {
        token: "PassengersAllowedToFire",
        parse: parse_passengers_allowed_to_fire,
    },
    FieldParse {
        token: "PassengersInTurret",
        parse: parse_passengers_in_turret,
    },
    FieldParse {
        token: "NumberOfExitPaths",
        parse: parse_number_of_exit_paths,
    },
    FieldParse {
        token: "DoorOpenTime",
        parse: parse_door_open_time,
    },
    FieldParse {
        token: "WeaponBonusPassedToPassengers",
        parse: parse_weapon_bonus_passed_to_passengers,
    },
    FieldParse {
        token: "AllowAlliesInside",
        parse: parse_allow_allies_inside,
    },
    FieldParse {
        token: "AllowEnemiesInside",
        parse: parse_allow_enemies_inside,
    },
    FieldParse {
        token: "AllowNeutralInside",
        parse: parse_allow_neutral_inside,
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

/// Open contain module - base functionality for all containers
#[derive(Debug)]
pub struct OpenContain {
    /// Reference to the owning object
    object: Weak<RwLock<Object>>,
    /// UpdateModule base scheduler state.
    next_call_frame_and_phase: UnsignedInt,
    /// List of contained objects
    contained_objects: Vec<Arc<RwLock<Object>>>,
    /// Cached IDs for ContainModuleInterface access.
    contained_object_ids: Vec<ObjectID>,
    /// Track objects requesting enter/exit to support container-specific gating.
    object_enter_exit_info: HashMap<ObjectID, ContainWant>,
    /// Contained IDs read from a save stream and resolved after all objects load.
    xfer_contain_id_list: Vec<ObjectID>,
    /// Player mask for the last player that entered this container.
    player_who_entered: PlayerMaskType,
    /// Last frame a load sound played.
    last_load_sound_frame: UnsignedInt,
    /// Last frame an unload sound played.
    last_unload_sound_frame: UnsignedInt,
    /// Whether load sounds are enabled.
    load_sounds_enabled: bool,
    /// Frames remaining before door closes (0 when idle).
    door_close_countdown: AtomicU32,
    /// Number of stealth garrison units in this container.
    stealth_units_contained: UnsignedInt,
    /// Exit path suffix to use next when multiple paths are available.
    which_exit_path: i32,
    /// Cached drawable condition state used to redeploy firepoint occupants.
    condition_state: ModelConditionFlags,
    /// Cached FIREPOINT transforms; C++ Matrix3D serializes 3 rows of 4 floats.
    fire_points: [FirePointMatrix; MAX_FIRE_POINTS],
    fire_point_start: i32,
    fire_point_next: i32,
    fire_point_size: i32,
    no_fire_points_in_art: bool,
    rally_point: Coord3D,
    rally_point_exists: bool,
    /// Module configuration data
    module_data: OpenContainModuleData,
}

impl OpenContain {
    /// Create a new OpenContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &OpenContainModuleData,
    ) -> GameResult<Self> {
        Ok(Self {
            object,
            next_call_frame_and_phase: 0,
            contained_objects: Vec::new(),
            contained_object_ids: Vec::new(),
            object_enter_exit_info: HashMap::new(),
            xfer_contain_id_list: Vec::new(),
            player_who_entered: PlayerMaskType::none(),
            last_load_sound_frame: 0,
            last_unload_sound_frame: 0,
            load_sounds_enabled: true,
            door_close_countdown: AtomicU32::new(0),
            stealth_units_contained: 0,
            which_exit_path: 1,
            condition_state: ModelConditionFlags::empty(),
            fire_points: [Self::identity_fire_point_matrix(); MAX_FIRE_POINTS],
            fire_point_start: -1,
            fire_point_next: 0,
            fire_point_size: 0,
            no_fire_points_in_art: false,
            rally_point: Coord3D::new(0.0, 0.0, 0.0),
            rally_point_exists: false,
            module_data: module_data.clone(),
        })
    }

    const fn identity_fire_point_matrix() -> FirePointMatrix {
        [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ]
    }

    fn xfer_coord_3d(xfer: &mut dyn Xfer, coord: &mut Coord3D) -> Result<(), String> {
        xfer.xfer_real(&mut coord.x).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut coord.y).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut coord.z).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer_fire_point_matrix(
        xfer: &mut dyn Xfer,
        matrix: &mut FirePointMatrix,
    ) -> Result<(), String> {
        for row in matrix {
            for value in row {
                xfer.xfer_real(value).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    fn xfer_model_condition_flags(
        xfer: &mut dyn Xfer,
        flags: &mut ModelConditionFlags,
    ) -> Result<(), String> {
        let mut bits = flags.bits();
        xfer.xfer_u128(&mut bits).map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            *flags = ModelConditionFlags::from_bits_retain(bits);
        }
        Ok(())
    }

    fn fire_point_matrix_from_transform(transform: Matrix3D) -> FirePointMatrix {
        let columns = transform.to_cols_array_2d();
        [
            [columns[0][0], columns[1][0], columns[2][0], columns[3][0]],
            [columns[0][1], columns[1][1], columns[2][1], columns[3][1]],
            [columns[0][2], columns[1][2], columns[2][2], columns[3][2]],
        ]
    }

    fn fire_point_matrix_from_position(pos: Coord3D) -> FirePointMatrix {
        let transform = Matrix3D::from_translation(pos);
        Self::fire_point_matrix_from_transform(transform)
    }

    fn fire_point_position(matrix: &FirePointMatrix) -> Coord3D {
        Coord3D::new(matrix[0][3], matrix[1][3], matrix[2][3])
    }

    fn contain_want_to_cpp_value(want: ContainWant) -> i32 {
        match want {
            ContainWant::WantsToEnter => 0,
            ContainWant::WantsToExit => 1,
            ContainWant::WantsNeither => 2,
        }
    }

    fn contain_want_from_cpp_value(value: i32) -> Result<ContainWant, String> {
        match value {
            0 => Ok(ContainWant::WantsToEnter),
            1 => Ok(ContainWant::WantsToExit),
            2 => Ok(ContainWant::WantsNeither),
            _ => Err(format!("invalid ObjectEnterExitType value {value}")),
        }
    }

    /// Get the object this module belongs to
    pub fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.object.upgrade()
    }

    /// Update method called once per frame
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        self.player_who_entered = PlayerMaskType::none();
        self.monitor_condition_changes()?;
        let countdown = self.door_close_countdown.load(Ordering::Relaxed);
        if countdown > 0 {
            let next = countdown.saturating_sub(1);
            self.door_close_countdown.store(next, Ordering::Relaxed);
            if next == 0 {
                if let Some(owner) = self.get_object() {
                    if let Ok(mut owner_guard) = owner.write() {
                        let _ = owner_guard.clear_and_set_model_condition_flags(
                            MODELCONDITION_DOOR_1_OPENING,
                            MODELCONDITION_DOOR_1_CLOSING,
                        );
                    }
                }
            }
        }
        if !self.object_enter_exit_info.is_empty() {
            self.prune_dead_wanters();
        }
        Ok(UpdateSleepTime::None)
    }

    /// Check art condition changes and redeploy occupants when FIREPOINT bones may have changed.
    pub fn monitor_condition_changes(&mut self) -> GameResult<()> {
        let Some(owner) = self.get_object() else {
            return Ok(());
        };
        let curr_condition = owner
            .read()
            .ok()
            .and_then(|owner_guard| owner_guard.get_drawable())
            .map(|drawable| drawable.get_model_condition_flags());

        let Some(curr_condition) = curr_condition else {
            return Ok(());
        };
        if curr_condition != self.condition_state {
            self.redeploy_occupants()?;
            self.condition_state = curr_condition;
        }
        Ok(())
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        // Check kind restrictions
        let obj_kind = obj.get_kind_of();

        // Must have at least one allowed kind bit
        if self.module_data.allow_inside_kind_of != 0
            && (obj_kind & self.module_data.allow_inside_kind_of) == 0
        {
            return false;
        }

        // Must have none of the forbidden kind bits
        if (obj_kind & self.module_data.forbid_inside_kind_of) != 0 {
            return false;
        }

        // Check capacity if requested
        if check_capacity && self.module_data.contain_max != CONTAIN_MAX_UNKNOWN {
            if self.contained_objects.len() >= self.module_data.contain_max as usize {
                return false;
            }
        }

        // Check ally/enemy restrictions
        if let Some(owner_obj) = self.get_object() {
            if let (Ok(owner), Ok(candidate)) = (owner_obj.read(), obj.try_read()) {
                let relationship = owner.get_relationship_to(&candidate);
                match relationship {
                    ObjectRelationship::Ally => return self.module_data.allow_allies_inside,
                    ObjectRelationship::Enemy => return self.module_data.allow_enemies_inside,
                    ObjectRelationship::Neutral => return self.module_data.allow_neutral_inside,
                    _ => return false,
                }
            }
        }

        true
    }

    /// Add object to containment
    pub fn add_to_contain(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        if let Some(owner) = self.get_object() {
            if let (Ok(owner_guard), Ok(obj_guard)) = (owner.read(), obj.read()) {
                if owner_guard.check_and_detonate_booby_trap(Some(&*obj_guard)) {
                    if owner_guard.is_effectively_dead() || obj_guard.is_effectively_dead() {
                        return Ok(());
                    }
                }
            }
        }

        let was_selected = obj
            .read()
            .ok()
            .and_then(|guard| guard.get_drawable())
            .and_then(|drawable| drawable.read().ok().map(|draw| draw.is_selected()))
            .unwrap_or(false);

        let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
        if !self.is_valid_container_for(&*obj_guard, true) {
            return Err("Object not valid for this container".into());
        }
        if obj_guard.get_contained_by().is_some() {
            return Ok(());
        }
        drop(obj_guard);

        self.add_to_contain_list(obj.clone())?;

        if let Ok(obj_guard) = obj.read() {
            if self.is_enclosing_container_for(&*obj_guard) {
                let _ = self.add_or_remove_obj_from_world(obj.clone(), false);
            }
        }

        self.redeploy_occupants()?;
        self.on_containing(obj, was_selected)?;

        Ok(())
    }

    /// Add object to contain list (can be overridden by inheritors)
    pub fn add_to_contain_list(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        let obj_id = obj.read().map_err(|_| GameError::LockError)?.get_id();
        let is_stealth_garrison = obj
            .read()
            .map(|guard| guard.is_kind_of(KindOf::StealthGarrison))
            .unwrap_or(false);
        self.contained_objects.push(obj);
        self.contained_object_ids.push(obj_id);
        if is_stealth_garrison {
            self.stealth_units_contained = self.stealth_units_contained.saturating_add(1);
        }
        Ok(())
    }

    /// Remove object from contain list without triggering containment callbacks.
    pub fn remove_from_contain_list(&mut self, object_id: ObjectID) {
        if let Some(id_pos) = self
            .contained_object_ids
            .iter()
            .position(|id| *id == object_id)
        {
            self.contained_object_ids.remove(id_pos);
        }

        if let Some(pos) = self
            .contained_objects
            .iter()
            .position(|obj| obj.read().ok().map(|guard| guard.get_id()) == Some(object_id))
        {
            let is_stealth_garrison = self
                .contained_objects
                .get(pos)
                .and_then(|obj| {
                    obj.read()
                        .ok()
                        .map(|guard| guard.is_kind_of(KindOf::StealthGarrison))
                })
                .unwrap_or(false);
            self.contained_objects.remove(pos);
            if is_stealth_garrison {
                self.stealth_units_contained = self.stealth_units_contained.saturating_sub(1);
            }
        }
    }

    /// Remove object from containment
    pub fn remove_from_contain(
        &mut self,
        obj: Arc<RwLock<Object>>,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        // Only allow removal if this object is actually contained by us (C++ safety check).
        if let Some(owner) = self.get_object() {
            let owner_id = owner.read().ok().map(|guard| guard.get_id());
            if let (Some(owner_id), Ok(obj_guard)) = (owner_id, obj.read()) {
                if obj_guard.get_contained_by() != Some(owner_id) {
                    return Ok(());
                }
            }
        }

        // Find and remove object from list
        if let Some(pos) = self
            .contained_objects
            .iter()
            .position(|x| Arc::ptr_eq(x, &obj))
        {
            let obj_id = self
                .contained_objects
                .get(pos)
                .and_then(|arc| arc.read().ok().map(|guard| guard.get_id()));
            let is_stealth_garrison = self
                .contained_objects
                .get(pos)
                .and_then(|arc| {
                    arc.read()
                        .ok()
                        .map(|guard| guard.is_kind_of(KindOf::StealthGarrison))
                })
                .unwrap_or(false);
            self.contained_objects.remove(pos);
            if is_stealth_garrison {
                self.stealth_units_contained = self.stealth_units_contained.saturating_sub(1);
            }
            if expose_stealth_units {
                if let Ok(obj_guard) = obj.read() {
                    if let Some(stealth) = obj_guard.get_stealth() {
                        if let Ok(mut stealth_guard) = stealth.lock() {
                            stealth_guard.mark_as_detected();
                        }
                    }
                }
            }
            self.do_unload_sound();
            self.on_removing(obj.clone())?;
            if let Some(obj_id) = obj_id {
                if let Some(id_pos) = self
                    .contained_object_ids
                    .iter()
                    .position(|id| *id == obj_id)
                {
                    self.contained_object_ids.remove(id_pos);
                }
            }

            if let Ok(obj_guard) = obj.read() {
                if self.is_enclosing_container_for(&*obj_guard) {
                    let _ = self.add_or_remove_obj_from_world(obj.clone(), true);
                    if let Some(owner) = self.get_object() {
                        if let (Ok(owner_guard), Ok(mut obj_guard)) = (owner.read(), obj.write()) {
                            if let Err(err) = obj_guard.set_position(owner_guard.get_position()) {
                                log::warn!(
                                    "OpenContain::remove_from_contain failed to place object {}: {}",
                                    obj_guard.get_id(),
                                    err
                                );
                            }
                        }
                    }
                }
            }
            if let Some(owner) = self.get_object() {
                if let Ok(owner_guard) = owner.read() {
                    if let Ok(mut obj_guard) = obj.write() {
                        obj_guard.set_layer(owner_guard.get_layer());
                    }
                }
            }
        }

        Ok(())
    }

    /// Remove all contained objects
    pub fn remove_all_contained(&mut self, expose_stealth_units: bool) -> GameResult<()> {
        let objects = self.contained_objects.clone();
        for obj in objects {
            self.remove_from_contain(obj, expose_stealth_units)?;
        }
        Ok(())
    }

    /// Kill all contained objects.
    /// Matches C++ OpenContain::killAllContained.
    pub fn kill_all_contained(&mut self) -> GameResult<()> {
        while let Some(obj) = self.contained_objects.first().cloned() {
            self.remove_from_contain(obj.clone(), true)?;
            if let Ok(mut guard) = obj.write() {
                guard.kill(None, None);
            }
        }

        Ok(())
    }

    /// Force all contained objects to exit and apply damage.
    /// Matches C++ OpenContain::harmAndForceExitAllContained.
    pub fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> GameResult<()> {
        while let Some(obj) = self.contained_objects.first().cloned() {
            self.remove_from_contain(obj.clone(), true)?;
            if let Ok(mut guard) = obj.write() {
                let _ = guard.attempt_damage(damage_info);
            }
        }

        Ok(())
    }

    /// Called when this object starts containing another object
    pub fn on_containing(
        &mut self,
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> GameResult<()> {
        let _ = was_selected;

        // Object-level containment processing (matches C++ Object::onContainedBy).
        let container = self
            .get_object()
            .ok_or_else(|| GameError::ModuleError("OpenContain has no owning object".into()))?;
        if let Ok(mut contained) = obj.write() {
            contained
                .on_contained_by(container)
                .map_err(|e| GameError::ModuleError(e.to_string()))?;

            if let Some(player) = contained.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    self.player_who_entered = player_guard.get_player_mask();
                }
            }
        }

        // Play enter sound
        self.do_load_sound();

        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        // Object-level containment removal processing (matches C++ Object::onRemovedFrom).
        let container = self
            .get_object()
            .ok_or_else(|| GameError::ModuleError("OpenContain has no owning object".into()))?;
        if let Ok(mut contained) = obj.write() {
            contained
                .on_removed_from(container)
                .map_err(|e| GameError::ModuleError(e.to_string()))?;
        }

        Ok(())
    }

    /// Enable or disable load sounds.
    pub fn enable_load_sounds(&mut self, enabled: bool) {
        self.load_sounds_enabled = enabled;
    }

    /// Play a load sound (once per frame when enabled).
    pub fn do_load_sound(&mut self) {
        if !self.load_sounds_enabled {
            return;
        }
        let Some(enter_sound) = &self.module_data.enter_sound else {
            return;
        };
        let now = TheGameLogic::get_frame();
        if now == self.last_load_sound_frame {
            return;
        }

        let object_id = self
            .get_object()
            .and_then(|obj| obj.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(0);

        let mut event = AudioEventRts::new(enter_sound.get_event_name());
        if object_id != 0 {
            event.set_object_id(object_id);
        }
        if let Some(audio) = TheAudio::get() {
            audio.add_audio_event(&event);
        }

        self.last_load_sound_frame = now;
    }

    /// Handle death event — C++ OpenContain::onDie (lines 833-851)
    pub fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> GameResult<()> {
        // C++ line 839-843: Apply damage to contained units based on damage percentage
        if self.module_data.damage_percentage_to_units > 0.0 {
            if let Some(info) = damage_info {
                let damage_to_units =
                    info.input.amount * self.module_data.damage_percentage_to_units;

                for obj in &self.contained_objects {
                    if let Ok(contained) = obj.read() {
                        if let Some(ai) = contained.get_ai_update_interface() {
                            if let Ok(ai_guard) = ai.lock() {
                                let mut unit_damage = info.clone();
                                unit_damage.input.amount = damage_to_units;
                                unit_damage.sync_from_input();
                                drop(ai_guard);
                                drop(contained);
                                if let Ok(mut contained_mut) = obj.write() {
                                    let _ = contained_mut.attempt_damage(&mut unit_damage);
                                }
                            }
                        }
                    }
                }
            }
        }

        // C++ line 845: Kill riders who are not free to exit
        // Default implementation is no-op (C++ OpenContain has empty virtual method)
        // TransportContain overrides with actual logic
        self.kill_riders_who_are_not_free_to_exit()?;

        // C++ line 850: Remove all contained units
        self.remove_all_contained(true)?;
        Ok(())
    }

    /// Kill riders who are not free to exit — default no-op (C++ OpenContain virtual)
    /// TransportContain overrides with actual logic.
    fn kill_riders_who_are_not_free_to_exit(&mut self) -> GameResult<()> {
        Ok(())
    }

    /// Track objects that want to enter/exit (C++ OpenContain::onObjectWantsToEnterOrExit).
    pub fn on_object_wants_to_enter_or_exit(&mut self, obj: &Object, want: ContainWant) {
        let id = obj.get_id();
        if matches!(want, ContainWant::WantsNeither) {
            self.object_enter_exit_info.remove(&id);
        } else {
            self.object_enter_exit_info.insert(id, want);
        }
    }

    /// Prune dead wanters (C++ OpenContain::pruneDeadWanters).
    pub fn prune_dead_wanters(&mut self) {
        self.object_enter_exit_info.retain(|id, _| {
            if let Some(obj) = TheGameLogic::find_object_by_id(*id) {
                if let Ok(obj_guard) = obj.read() {
                    return !obj_guard.is_effectively_dead();
                }
            }
            false
        });
    }

    /// Handle damage event
    pub fn on_damage(&mut self, info: &mut DamageInfo) -> GameResult<()> {
        // Distribute damage to contained units if configured
        if self.module_data.damage_percentage_to_units > 0.0 {
            let damage_to_units = info.input.amount * self.module_data.damage_percentage_to_units;

            for obj in &self.contained_objects {
                if let Ok(_contained) = obj.read() {
                    // Apply damage to contained unit
                    let mut unit_damage = info.clone();
                    unit_damage.input.amount = damage_to_units;
                    unit_damage.sync_from_input();
                    // contained.apply_damage(&unit_damage)?;
                }
            }
        }

        Ok(())
    }

    /// Iterate contained objects with callback
    pub fn iterate_contained<F>(&self, mut func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(Arc<RwLock<Object>>) -> GameResult<()>,
    {
        if reverse {
            for obj in self.contained_objects.iter().rev() {
                func(obj.clone())?;
            }
        } else {
            for obj in &self.contained_objects {
                func(obj.clone())?;
            }
        }
        Ok(())
    }

    /// Get count of contained objects
    pub fn get_contain_count(&self) -> u32 {
        self.contained_object_ids.len() as u32
    }

    /// Get maximum containment capacity
    pub fn get_contain_max(&self) -> i32 {
        self.module_data.contain_max
    }

    /// Get list of contained items
    pub fn get_contained_items_list(&self) -> GameResult<Vec<Arc<RwLock<Object>>>> {
        Ok(self.contained_objects.clone())
    }

    /// Check if passenger is allowed to fire
    pub fn is_passenger_allowed_to_fire(&self, _id: Option<ObjectId>) -> bool {
        self.module_data.passengers_allowed_to_fire
    }

    /// Whether passengers inherit the container's weapon bonus flags.
    pub fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.module_data.weapon_bonus_passed_to_passengers
    }

    /// Toggle whether passengers may fire from this container.
    pub fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.module_data.passengers_allowed_to_fire = allowed;
    }

    pub fn get_stealth_units_contained(&self) -> UnsignedInt {
        self.stealth_units_contained
    }

    pub fn set_rally_point(&mut self, pos: Coord3D) {
        self.rally_point = pos;
        self.rally_point_exists = true;
    }

    pub fn get_rally_point(&self) -> Option<Coord3D> {
        self.rally_point_exists.then_some(self.rally_point)
    }

    pub fn get_natural_rally_point(&self) -> Option<Coord3D> {
        let owner = self.get_object()?;
        let owner_guard = owner.read().ok()?;
        let number_exits = self.module_data.number_of_exit_paths;
        if number_exits > 0 {
            let end_bone = if number_exits > 1 {
                "ExitEnd01"
            } else {
                "ExitEnd"
            };
            let (_, rally_point, _) = owner_guard.get_single_logical_bone_position(end_bone);
            Some(rally_point)
        } else {
            Some(*owner_guard.get_position())
        }
    }

    /// Check if this is an enclosing container
    pub fn is_enclosing_container_for(&self, _obj: &Object) -> bool {
        true // Most containers enclose their contents
    }

    /// Whether any objects are requesting enter/exit.
    /// Matches C++ OpenContain::hasObjectsWantingToEnterOrExit
    pub fn has_objects_wanting_to_enter_or_exit(&self) -> bool {
        !self.object_enter_exit_info.is_empty()
    }

    /// Reserve door for exit
    pub fn reserve_door_for_exit(
        &self,
        obj_type: &ObjectTemplate,
        specific_object: &Object,
    ) -> GameResult<ExitDoorType> {
        let _ = (obj_type, specific_object);
        if self.module_data.door_open_time > 0 {
            if let Some(owner) = self.get_object() {
                if let Ok(mut owner_guard) = owner.write() {
                    let _ = owner_guard.clear_and_set_model_condition_flags(
                        MODELCONDITION_DOOR_1_CLOSING,
                        MODELCONDITION_DOOR_1_OPENING,
                    );
                }
            }
        }
        Ok(ExitDoorType::Primary)
    }

    fn exit_bone_names_for_next_path(&mut self) -> (&'static str, String, String) {
        let number_exits = self.module_data.number_of_exit_paths;
        if number_exits > 1 {
            let suffix = format!("{:02}", self.which_exit_path);
            self.which_exit_path = (self.which_exit_path % number_exits) + 1;
            (
                "numbered",
                format!("ExitStart{suffix}"),
                format!("ExitEnd{suffix}"),
            )
        } else {
            ("single", "ExitStart".to_string(), "ExitEnd".to_string())
        }
    }

    fn next_exit_positions(&mut self, owner: &Object) -> (Coord3D, Coord3D) {
        if self.module_data.number_of_exit_paths <= 0 {
            let pos = *owner.get_position();
            return (pos, pos);
        }

        let (_, start_bone, end_bone) = self.exit_bone_names_for_next_path();
        let (_, start_pos, _) = owner.get_single_logical_bone_position(&start_bone);
        let (_, end_pos, _) = owner.get_single_logical_bone_position(&end_bone);
        (start_pos, end_pos)
    }

    fn destination_layer(pos: &Coord3D) -> PathfindLayerEnum {
        TheTerrainLogic::get()
            .map(|terrain| terrain.get_layer_for_destination(pos))
            .unwrap_or(PathfindLayerEnum::Ground)
    }

    fn add_to_pathfind_map(object_id: ObjectID, pos: Coord3D) {
        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    pf.add_object_to_map(object_id, &[pos], false);
                }
            }
        }
    }

    fn refresh_owner_pathfind_goal(owner: &Object) {
        let Some(owner_ai) = owner.get_ai_update_interface() else {
            return;
        };
        let Ok(mut owner_ai_guard) = owner_ai.try_lock() else {
            return;
        };
        if !owner_ai_guard.is_idle() || !owner.is_kind_of(KindOf::Vehicle) {
            return;
        }

        let owner_id = owner.get_id();
        let owner_pos = *owner.get_position();
        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    pf.remove_object_from_map(owner_id, &[owner_pos]);
                    pf.add_object_to_map(owner_id, &[owner_pos], false);
                }
            }
        }
        let owner_layer = Self::destination_layer(&owner_pos);
        let _ = owner_ai_guard.update_goal_position(&owner_pos, owner_layer);
    }

    fn prepare_exit_object(
        &mut self,
        exit_obj: &Arc<RwLock<Object>>,
        hurry: bool,
    ) -> GameResult<Option<ExitPrep>> {
        self.remove_from_contain(Arc::clone(exit_obj), false)?;

        let Some(owner) = self.get_object() else {
            return Ok(None);
        };
        let Ok(owner_guard) = owner.read() else {
            return Ok(None);
        };

        self.door_close_countdown
            .store(self.module_data.door_open_time, Ordering::Relaxed);
        if self.module_data.door_open_time > 0 {
            drop(owner_guard);
            if let Ok(mut owner_guard) = owner.write() {
                let _ = owner_guard.clear_and_set_model_condition_flags(
                    MODELCONDITION_DOOR_1_CLOSING,
                    MODELCONDITION_DOOR_1_OPENING,
                );
            }
        }

        let Ok(owner_guard) = owner.read() else {
            return Ok(None);
        };
        let (mut start_pos, mut end_pos) = self.next_exit_positions(&owner_guard);
        let exit_angle = owner_guard.get_orientation();
        let owner_layer = owner_guard.get_layer();
        let owner_id = owner_guard.get_id();

        if let Some(terrain) = TheTerrainLogic::get() {
            start_pos.z = terrain.get_ground_height(start_pos.x, start_pos.y, None);
            end_pos.z = terrain.get_ground_height(end_pos.x, end_pos.y, None);
        }

        let exit_id = if let Ok(mut exit_guard) = exit_obj.write() {
            let _ = exit_guard.set_position(&start_pos);
            let _ = exit_guard.set_orientation(exit_angle);
            exit_guard.set_layer(owner_layer);
            exit_guard.get_id()
        } else {
            return Ok(None);
        };

        Self::add_to_pathfind_map(exit_id, start_pos);
        Self::refresh_owner_pathfind_goal(&owner_guard);

        if let Ok(exit_guard) = exit_obj.read() {
            if let Some(ai) = exit_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.try_lock() {
                    ai_guard.set_ignore_collision_time(LOGICFRAMES_PER_SECOND as UnsignedInt);
                    let _ = ai_guard.ignore_obstacle(None);
                    let _ = ai_guard.adjust_destination(&mut end_pos);
                    let _ =
                        ai_guard.update_goal_position(&end_pos, Self::destination_layer(&end_pos));
                }
            }
        }

        let mut exit_path = if hurry {
            vec![end_pos]
        } else {
            vec![end_pos, end_pos]
        };
        if hurry {
            exit_path.push(end_pos);
        }
        if self.rally_point_exists {
            exit_path.push(self.rally_point);
        }

        Ok(Some(ExitPrep {
            owner_id,
            end_pos,
            exit_path,
        }))
    }

    pub fn exit_object_via_door(
        &mut self,
        exit_obj: &Arc<RwLock<Object>>,
        exit_door: ExitDoorType,
    ) -> GameResult<()> {
        if matches!(exit_door, ExitDoorType::None | ExitDoorType::NoneAvailable) {
            return Ok(());
        }

        let Some(prep) = self.prepare_exit_object(exit_obj, false)? else {
            return Ok(());
        };

        if let Ok(exit_guard) = exit_obj.read() {
            let previous_allow_to_fall = exit_guard.get_physics().map(|physics| {
                let previous = physics.get_allow_to_fall();
                physics.set_allow_to_fall(false);
                (physics, previous)
            });

            if let Some(ai) = exit_guard.get_ai_update_interface() {
                ai.ai_follow_path(
                    &prep.exit_path,
                    Some(prep.owner_id),
                    CommandSourceType::FromAi,
                );
                if let Ok(mut ai_guard) = ai.try_lock() {
                    let _ = ai_guard.update_goal_position(
                        &prep.end_pos,
                        Self::destination_layer(&prep.end_pos),
                    );
                }
            }

            if let Some((physics, previous)) = previous_allow_to_fall {
                physics.set_allow_to_fall(previous);
            }
        }

        Ok(())
    }

    pub fn exit_object_in_a_hurry(&mut self, exit_obj: &Arc<RwLock<Object>>) -> GameResult<()> {
        let Some(prep) = self.prepare_exit_object(exit_obj, true)? else {
            return Ok(());
        };

        if let Ok(exit_guard) = exit_obj.read() {
            if let Some(ai) = exit_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.try_lock() {
                    let _ = ai_guard.set_path_from_coords(&prep.exit_path);
                    let _ = ai_guard.update_goal_position(
                        &prep.end_pos,
                        Self::destination_layer(&prep.end_pos),
                    );
                }
            }
        }

        Ok(())
    }

    /// Unreserve door for exit
    pub fn unreserve_door_for_exit(&self, exit_door: ExitDoorType) -> GameResult<()> {
        let _ = exit_door;
        if self.module_data.door_open_time > 0 {
            self.door_close_countdown
                .store(self.module_data.door_open_time, Ordering::Relaxed);
        }
        Ok(())
    }

    /// Check if exit is currently busy
    pub fn is_exit_busy(&self) -> bool {
        // Implementation would check if exit paths are busy
        false
    }

    /// Get container pips info for UI
    pub fn get_container_pips_info(&self) -> (i32, i32) {
        let total = if self.module_data.contain_max == CONTAIN_MAX_UNKNOWN {
            10
        } else {
            self.module_data.contain_max
        };
        let full = self.contained_objects.len() as i32;
        (total, full)
    }

    /// Redeploy occupants (can be overridden)
    pub fn redeploy_occupants(&mut self) -> GameResult<()> {
        self.no_fire_points_in_art = false;
        self.fire_point_start = -1;
        self.fire_point_next = 0;
        self.fire_point_size = 0;

        let contained = self.contained_objects.clone();
        for obj in contained.iter().rev() {
            self.put_obj_at_next_fire_point(obj)?;
        }
        Ok(())
    }

    fn put_obj_at_next_fire_point(&mut self, obj: &Arc<RwLock<Object>>) -> GameResult<()> {
        let Some(owner) = self.get_object() else {
            return Ok(());
        };

        if self.fire_point_size == 0 && !self.no_fire_points_in_art {
            let fire_points = owner
                .read()
                .map(|owner_guard| {
                    owner_guard.get_multi_logical_bone_position("FIREPOINT", MAX_FIRE_POINTS)
                })
                .unwrap_or_default();

            self.fire_point_size = fire_points.len() as i32;
            if self.fire_point_size == 0 {
                self.no_fire_points_in_art = true;
            } else {
                for (index, pos) in fire_points.into_iter().enumerate().take(MAX_FIRE_POINTS) {
                    self.fire_points[index] = Self::fire_point_matrix_from_position(pos);
                }
            }
        }

        let pos = if self.no_fire_points_in_art {
            owner
                .read()
                .map(|owner_guard| *owner_guard.get_position())
                .unwrap_or_else(|_| Coord3D::new(0.0, 0.0, 0.0))
        } else if self.module_data.passengers_in_turret {
            let firepoint = format!("FIREPOINT{:02}", self.fire_point_next + 1);
            owner
                .read()
                .map(|owner_guard| {
                    let (_, pos, matrix) = owner_guard.get_single_logical_bone_position_on_turret(
                        TurretType::Primary,
                        &firepoint,
                    );
                    self.fire_points[self.fire_point_next as usize] =
                        Self::fire_point_matrix_from_transform(matrix);
                    pos
                })
                .unwrap_or_else(|_| Coord3D::new(0.0, 0.0, 0.0))
        } else {
            Self::fire_point_position(&self.fire_points[self.fire_point_next as usize])
        };

        if let Ok(mut guard) = obj.write() {
            if self.is_enclosing_container_for(&guard) {
                if let Err(err) = guard.set_position(&pos) {
                    log::warn!(
                        "OpenContain::put_obj_at_next_fire_point failed to place object {}: {}",
                        guard.get_id(),
                        err
                    );
                }
            } else {
                let matrix = Matrix3D::from_translation(pos);
                guard.set_transform_matrix(&matrix);
            }
        }

        if self.fire_point_size > 0 {
            self.fire_point_next += 1;
            if self.fire_point_next >= self.fire_point_size {
                self.fire_point_next = 0;
            }
        }

        Ok(())
    }

    fn add_or_remove_obj_from_world(
        &mut self,
        obj: Arc<RwLock<Object>>,
        add: bool,
    ) -> GameResult<()> {
        if add {
            if let Ok(mut guard) = obj.write() {
                let _ = guard.register_in_partition_manager();
                if let Some(drawable) = guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        let _ = draw_guard.set_drawable_hidden(false);
                    }
                }
            }
        } else {
            if let Ok(mut guard) = obj.write() {
                guard.leave_group();
                if let Some(drawable) = guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        let _ = draw_guard.set_drawable_hidden(true);
                    }
                }
            }
        }

        let contained_ids = {
            let guard = obj.read().map_err(|_| GameError::LockError)?;
            if let Some(contain) = guard.get_contain() {
                if let Ok(contain_guard) = contain.lock() {
                    contain_guard.get_contained_objects().to_vec()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        };

        for object_id in contained_ids {
            if let Some(child) = TheGameLogic::find_object_by_id(object_id) {
                let should_recurse = {
                    let child_guard = child.read().map_err(|_| GameError::LockError)?;
                    let obj_guard = obj.read().map_err(|_| GameError::LockError)?;
                    if let Some(contain) = obj_guard.get_contain() {
                        if let Ok(contain_guard) = contain.lock() {
                            !contain_guard.is_enclosing_container_for(&*child_guard)
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                };
                if should_recurse {
                    let _ = self.add_or_remove_obj_from_world(child, add);
                }
            }
        }

        Ok(())
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        let mut state = HashMap::new();

        // Save contained object IDs
        let ids_bytes: Vec<u8> = self
            .contained_object_ids
            .iter()
            .flat_map(|id| id.to_le_bytes())
            .collect();

        state.insert("contained_objects".to_string(), ids_bytes);

        Ok(state)
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        if let Some(_data) = state.get("contained_objects") {
            // Implementation would reconstruct contained objects from IDs
            // This requires access to object lookup system
        }

        Ok(())
    }

    /// Calculate CRC for network synchronization
    pub fn calculate_crc(&self) -> u32 {
        // Implementation would calculate CRC of relevant state
        0
    }

    /// Post-process after loading
    pub fn load_post_process(&mut self) -> GameResult<()> {
        // Base implementation - nothing special needed
        Ok(())
    }

    /// Flash selected for visible contained units.
    /// Matches C++ OpenContain::clientVisibleContainedFlashAsSelected
    pub fn client_visible_contained_flash_as_selected(&self) -> GameResult<()> {
        for obj in &self.contained_objects {
            if let Ok(obj_guard) = obj.read() {
                if self.is_enclosing_container_for(&*obj_guard) {
                    continue;
                }
                if let Some(drawable) = obj_guard.get_drawable() {
                    if let Ok(mut drawable_guard) = drawable.write() {
                        drawable_guard.flash_as_selected();
                    }
                }
            }
        }
        Ok(())
    }

    /// Play unload sound.
    /// Stub implementation - matches C++ OpenContain::doUnloadSound
    pub fn do_unload_sound(&mut self) {
        let Some(exit_sound) = &self.module_data.exit_sound else {
            return;
        };

        let now = TheGameLogic::get_frame();
        if now == self.last_unload_sound_frame {
            return;
        }

        let object_id = self
            .get_object()
            .and_then(|obj| obj.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(0);

        let mut event = AudioEventRts::new(exit_sound.get_event_name());
        if object_id != 0 {
            event.set_object_id(object_id);
        }

        if let Some(audio) = TheAudio::get() {
            audio.add_audio_event(&event);
        }

        self.last_unload_sound_frame = now;
    }
}

impl ContainModuleInterface for OpenContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return OpenContain::is_valid_container_for(self, &*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        self.add_to_contain(obj).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.remove_from_contain(obj, false)
            .map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        &self.contained_object_ids
    }

    fn get_contained_count(&self) -> usize {
        self.contained_object_ids.len()
    }

    fn get_max_capacity(&self) -> usize {
        if self.module_data.contain_max < 0 {
            usize::MAX
        } else {
            self.module_data.contain_max as usize
        }
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::update(self).map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_damage(self, info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_die(self, damage_info).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        OpenContain::is_valid_container_for(self, obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::enable_load_sounds(self, enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_object_wants_to_enter_or_exit(self, obj, want);
        Ok(())
    }

    fn is_immune_to_clear_building_attacks(&self) -> bool {
        true
    }

    fn is_passenger_allowed_to_fire(&self, id: Option<ObjectID>) -> bool {
        OpenContain::is_passenger_allowed_to_fire(self, id)
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.passes_weapon_bonus_to_passengers()
    }

    fn set_rally_point(&mut self, pos: Coord3D) {
        OpenContain::set_rally_point(self, pos);
    }

    fn get_rally_point(&self) -> Option<Coord3D> {
        OpenContain::get_rally_point(self)
    }

    fn reserve_door_for_exit(
        &mut self,
        _spawner: Option<&Object>,
        _spawn: Option<&Object>,
    ) -> ExitDoorType {
        if self.module_data.door_open_time > 0 {
            if let Some(owner) = self.get_object() {
                if let Ok(mut owner_guard) = owner.write() {
                    let _ = owner_guard.clear_and_set_model_condition_flags(
                        MODELCONDITION_DOOR_1_CLOSING,
                        MODELCONDITION_DOOR_1_OPENING,
                    );
                }
            }
        }
        ExitDoorType::Primary
    }

    fn unreserve_door_for_exit(&mut self, door: ExitDoorType) {
        let _ = OpenContain::unreserve_door_for_exit(self, door);
    }

    fn exit_object_via_door(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        door: ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::exit_object_via_door(self, obj, door).map_err(|err| err.into())
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::exit_object_in_a_hurry(self, obj).map_err(|err| err.into())
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        OpenContain::set_passenger_allowed_to_fire(self, allowed);
    }

    fn has_objects_wanting_to_enter_or_exit(&self) -> bool {
        self.has_objects_wanting_to_enter_or_exit()
    }

    fn on_containing(
        &mut self,
        obj: Arc<RwLock<Object>>,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_containing(self, obj, was_selected).map_err(|e| e.into())
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.player_who_entered
    }

    fn on_removing(
        &mut self,
        obj: Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_removing(self, obj).map_err(|e| e.into())
    }

    fn remove_all_contained(
        &mut self,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::remove_all_contained(self, expose_stealth).map_err(|e| e.into())
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::harm_and_force_exit_all_contained(self, damage_info).map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::kill_all_contained(self).map_err(|e| e.into())
    }

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::client_visible_contained_flash_as_selected(self).map_err(|e| e.into())
    }
}

impl Snapshotable for OpenContain {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| e.to_string())?;

        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;

        let mut contain_list_size = self.contained_object_ids.len() as u32;
        xfer.xfer_unsigned_int(&mut contain_list_size)
            .map_err(|e| e.to_string())?;
        for id in &self.contained_object_ids {
            let mut object_id = *id;
            xfer.xfer_object_id(&mut object_id)
                .map_err(|e| e.to_string())?;
        }

        let mut player_mask_bits = self.player_who_entered.bits();
        xfer.xfer_unsigned_int(&mut player_mask_bits)
            .map_err(|e| e.to_string())?;

        let mut last_unload_sound_frame = self.last_unload_sound_frame;
        xfer.xfer_unsigned_int(&mut last_unload_sound_frame)
            .map_err(|e| e.to_string())?;
        let mut last_load_sound_frame = self.last_load_sound_frame;
        xfer.xfer_unsigned_int(&mut last_load_sound_frame)
            .map_err(|e| e.to_string())?;

        let mut stealth_units_contained = self.stealth_units_contained;
        xfer.xfer_unsigned_int(&mut stealth_units_contained)
            .map_err(|e| e.to_string())?;

        let mut door_close_countdown = self.door_close_countdown.load(Ordering::Relaxed);
        xfer.xfer_unsigned_int(&mut door_close_countdown)
            .map_err(|e| e.to_string())?;

        Self::xfer_model_condition_flags(xfer, &mut self.condition_state.clone())?;

        let mut fire_points = self.fire_points.clone();
        for matrix in &mut fire_points {
            Self::xfer_fire_point_matrix(xfer, matrix)?;
        }

        let mut fire_point_start = self.fire_point_start;
        xfer.xfer_int(&mut fire_point_start)
            .map_err(|e| e.to_string())?;
        let mut fire_point_next = self.fire_point_next;
        xfer.xfer_int(&mut fire_point_next)
            .map_err(|e| e.to_string())?;
        let mut fire_point_size = self.fire_point_size;
        xfer.xfer_int(&mut fire_point_size)
            .map_err(|e| e.to_string())?;
        let mut no_fire_points_in_art = self.no_fire_points_in_art;
        xfer.xfer_bool(&mut no_fire_points_in_art)
            .map_err(|e| e.to_string())?;

        let mut rally_point = self.rally_point.clone();
        Self::xfer_coord_3d(xfer, &mut rally_point)?;
        let mut rally_point_exists = self.rally_point_exists;
        xfer.xfer_bool(&mut rally_point_exists)
            .map_err(|e| e.to_string())?;

        let mut enter_exit_count = self.object_enter_exit_info.len() as u16;
        xfer.xfer_unsigned_short(&mut enter_exit_count)
            .map_err(|e| e.to_string())?;
        for (id, want) in &self.object_enter_exit_info {
            let mut object_id = *id;
            let mut enter_exit_type = Self::contain_want_to_cpp_value(*want);
            xfer.xfer_object_id(&mut object_id)
                .map_err(|e| e.to_string())?;
            xfer.xfer_int(&mut enter_exit_type)
                .map_err(|e| e.to_string())?;
        }

        let mut which_exit_path = self.which_exit_path;
        xfer.xfer_int(&mut which_exit_path)
            .map_err(|e| e.to_string())?;

        if version >= 2 {
            let mut passengers_allowed_to_fire = self.module_data.passengers_allowed_to_fire;
            xfer.xfer_bool(&mut passengers_allowed_to_fire)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| e.to_string())?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                let mut contain_list_size = self.contained_object_ids.len() as u32;
                xfer.xfer_unsigned_int(&mut contain_list_size)
                    .map_err(|e| e.to_string())?;
                for id in &self.contained_object_ids {
                    let mut object_id = *id;
                    xfer.xfer_object_id(&mut object_id)
                        .map_err(|e| e.to_string())?;
                }
            }
            XferMode::Load => {
                self.contained_objects.clear();
                self.contained_object_ids.clear();
                self.xfer_contain_id_list.clear();

                let mut contain_list_size = 0_u32;
                xfer.xfer_unsigned_int(&mut contain_list_size)
                    .map_err(|e| e.to_string())?;
                for _ in 0..contain_list_size {
                    let mut object_id = 0;
                    xfer.xfer_object_id(&mut object_id)
                        .map_err(|e| e.to_string())?;
                    self.xfer_contain_id_list.push(object_id);
                }
            }
            XferMode::Invalid => return Err("invalid xfer mode for OpenContain".to_string()),
        }

        let mut player_mask_bits = self.player_who_entered.bits();
        xfer.xfer_unsigned_int(&mut player_mask_bits)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.player_who_entered = PlayerMaskType::from_bits_truncate(player_mask_bits);
        }

        xfer.xfer_unsigned_int(&mut self.last_unload_sound_frame)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.last_load_sound_frame)
            .map_err(|e| e.to_string())?;

        xfer.xfer_unsigned_int(&mut self.stealth_units_contained)
            .map_err(|e| e.to_string())?;

        let mut door_close_countdown = self.door_close_countdown.load(Ordering::Relaxed);
        xfer.xfer_unsigned_int(&mut door_close_countdown)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.door_close_countdown
                .store(door_close_countdown, Ordering::Relaxed);
        }

        Self::xfer_model_condition_flags(xfer, &mut self.condition_state)?;

        for matrix in &mut self.fire_points {
            Self::xfer_fire_point_matrix(xfer, matrix)?;
        }

        xfer.xfer_int(&mut self.fire_point_start)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.fire_point_next)
            .map_err(|e| e.to_string())?;
        xfer.xfer_int(&mut self.fire_point_size)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.no_fire_points_in_art)
            .map_err(|e| e.to_string())?;

        Self::xfer_coord_3d(xfer, &mut self.rally_point)?;
        xfer.xfer_bool(&mut self.rally_point_exists)
            .map_err(|e| e.to_string())?;

        let mut enter_exit_count = self.object_enter_exit_info.len() as u16;
        xfer.xfer_unsigned_short(&mut enter_exit_count)
            .map_err(|e| e.to_string())?;
        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for (id, want) in &self.object_enter_exit_info {
                    let mut object_id = *id;
                    let mut enter_exit_type = Self::contain_want_to_cpp_value(*want);
                    xfer.xfer_object_id(&mut object_id)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut enter_exit_type)
                        .map_err(|e| e.to_string())?;
                }
            }
            XferMode::Load => {
                if !self.object_enter_exit_info.is_empty() {
                    return Err("OpenContain enter/exit map must be empty before load".to_string());
                }
                for _ in 0..enter_exit_count {
                    let mut object_id = 0;
                    let mut enter_exit_type = 0;
                    xfer.xfer_object_id(&mut object_id)
                        .map_err(|e| e.to_string())?;
                    xfer.xfer_int(&mut enter_exit_type)
                        .map_err(|e| e.to_string())?;
                    self.object_enter_exit_info.insert(
                        object_id,
                        Self::contain_want_from_cpp_value(enter_exit_type)?,
                    );
                }
            }
            XferMode::Invalid => return Err("invalid xfer mode for OpenContain".to_string()),
        }

        xfer.xfer_int(&mut self.which_exit_path)
            .map_err(|e| e.to_string())?;

        if version >= 2 {
            xfer.xfer_bool(&mut self.module_data.passengers_allowed_to_fire)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if !self.contained_objects.is_empty() || !self.contained_object_ids.is_empty() {
            return Err("OpenContain list must be empty before load_post_process".to_string());
        }

        let owner = self.get_object().ok_or_else(|| {
            "OpenContain has no owning object during load_post_process".to_string()
        })?;
        let ids = std::mem::take(&mut self.xfer_contain_id_list);
        for object_id in ids {
            let obj = TheGameLogic::find_object_by_id(object_id).ok_or_else(|| {
                format!("OpenContain could not resolve contained object {object_id}")
            })?;
            self.contained_object_ids.push(object_id);
            self.contained_objects.push(obj.clone());
            if let Ok(obj_guard) = obj.read() {
                if self.is_enclosing_container_for(&*obj_guard) {
                    let _ = self.add_or_remove_obj_from_world(obj.clone(), false);
                }
            }
            {
                let mut obj_guard = obj.write().map_err(|e| e.to_string())?;
                obj_guard
                    .on_contained_by(owner.clone())
                    .map_err(|e| e.to_string())?;
            }
        }

        Ok(())
    }
}

impl ContainerInterface for OpenContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.add_to_contain(obj)
    }

    fn remove_object(&mut self, obj: Arc<RwLock<Object>>) -> GameResult<()> {
        self.remove_from_contain(obj, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.get_contain_count();
        let max = match self.get_contain_max() {
            CONTAIN_MAX_UNKNOWN => u32::MAX,
            value if value < 0 => u32::MAX,
            value => value as u32,
        };
        (current, max)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectRelationship {
    Ally,
    Enemy,
    Neutral,
    Self_,
}

#[derive(Debug, Clone)]
pub struct ObjectTemplate {
    // Implementation would be defined elsewhere
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn test_open_contain_creation() {
        let module_data = OpenContainModuleData {
            contain_max: 5,
            passengers_allowed_to_fire: true,
            ..Default::default()
        };

        assert_eq!(module_data.contain_max, 5);
        assert_eq!(module_data.passengers_allowed_to_fire, true);
    }

    #[test]
    fn test_contain_max_unknown() {
        assert_eq!(CONTAIN_MAX_UNKNOWN, -1);
    }

    #[test]
    fn parse_door_open_time_accepts_duration_suffixes() {
        let mut data = OpenContainModuleData::default();
        let mut ini = INI::new();

        parse_door_open_time(&mut ini, &mut data, &["1500ms"]).expect("duration");
        assert_eq!(data.door_open_time, 45);

        parse_door_open_time(&mut ini, &mut data, &["1.5s"]).expect("duration");
        assert_eq!(data.door_open_time, 45);
    }

    #[test]
    fn xfer_preserves_cpp_open_contain_runtime_fields() {
        let mut saved =
            OpenContain::new(Weak::new(), &OpenContainModuleData::default()).expect("contain");
        saved.next_call_frame_and_phase = 0x5511;
        saved.contained_object_ids = vec![101, 202];
        saved.player_who_entered = PlayerMaskType::PLAYER_1 | PlayerMaskType::PLAYER_3;
        saved.last_unload_sound_frame = 12;
        saved.last_load_sound_frame = 34;
        saved.door_close_countdown.store(56, Ordering::Relaxed);
        saved.stealth_units_contained = 2;
        saved.which_exit_path = 3;
        saved.condition_state = ModelConditionFlags::LOADED | ModelConditionFlags::DOOR_1_OPENING;
        saved.fire_points[1][2][3] = 7.5;
        saved.fire_point_start = 4;
        saved.fire_point_next = 5;
        saved.fire_point_size = 6;
        saved.no_fire_points_in_art = true;
        saved.rally_point = Coord3D::new(1.0, 2.0, 3.0);
        saved.rally_point_exists = true;
        saved
            .object_enter_exit_info
            .insert(303, ContainWant::WantsToEnter);
        saved
            .object_enter_exit_info
            .insert(404, ContainWant::WantsToExit);
        saved.module_data.passengers_allowed_to_fire = true;

        let mut bytes = Cursor::new(Vec::new());
        {
            let mut xfer = XferSave::new(&mut bytes, 1);
            saved.xfer(&mut xfer).unwrap();
        }

        bytes.set_position(0);
        let mut loaded =
            OpenContain::new(Weak::new(), &OpenContainModuleData::default()).expect("contain");
        loaded.last_unload_sound_frame = 1;
        loaded.last_load_sound_frame = 2;
        {
            let mut xfer = XferLoad::new(&mut bytes, 1);
            loaded.xfer(&mut xfer).unwrap();
        }

        assert_eq!(loaded.next_call_frame_and_phase, 0x5511);
        assert_eq!(loaded.xfer_contain_id_list, vec![101, 202]);
        assert!(loaded.contained_object_ids.is_empty());
        assert_eq!(
            loaded.player_who_entered,
            PlayerMaskType::PLAYER_1 | PlayerMaskType::PLAYER_3
        );
        assert_eq!(loaded.last_unload_sound_frame, 12);
        assert_eq!(loaded.last_load_sound_frame, 34);
        assert_eq!(loaded.door_close_countdown.load(Ordering::Relaxed), 56);
        assert_eq!(loaded.stealth_units_contained, 2);
        assert_eq!(loaded.which_exit_path, 3);
        assert_eq!(
            loaded.condition_state,
            ModelConditionFlags::LOADED | ModelConditionFlags::DOOR_1_OPENING
        );
        assert_eq!(loaded.fire_points[1][2][3], 7.5);
        assert_eq!(loaded.fire_point_start, 4);
        assert_eq!(loaded.fire_point_next, 5);
        assert_eq!(loaded.fire_point_size, 6);
        assert!(loaded.no_fire_points_in_art);
        assert_eq!(loaded.rally_point, Coord3D::new(1.0, 2.0, 3.0));
        assert!(loaded.rally_point_exists);
        assert_eq!(
            loaded.object_enter_exit_info.get(&303),
            Some(&ContainWant::WantsToEnter)
        );
        assert_eq!(
            loaded.object_enter_exit_info.get(&404),
            Some(&ContainWant::WantsToExit)
        );
        assert!(loaded.module_data.passengers_allowed_to_fire);
    }

    #[test]
    fn contain_interface_routes_rally_point_to_open_contain() {
        let mut contain =
            OpenContain::new(Weak::new(), &OpenContainModuleData::default()).expect("contain");
        let rally = Coord3D::new(11.0, 22.0, 33.0);

        ContainModuleInterface::set_rally_point(&mut contain, rally);

        assert_eq!(
            ContainModuleInterface::get_rally_point(&contain),
            Some(rally)
        );
    }

    #[test]
    fn exit_bone_names_cycle_numbered_paths_like_cpp() {
        let data = OpenContainModuleData {
            number_of_exit_paths: 3,
            ..OpenContainModuleData::default()
        };
        let mut contain = OpenContain::new(Weak::new(), &data).expect("contain");

        let (_, start_1, end_1) = contain.exit_bone_names_for_next_path();
        let (_, start_2, end_2) = contain.exit_bone_names_for_next_path();
        let (_, start_3, end_3) = contain.exit_bone_names_for_next_path();
        let (_, start_4, end_4) = contain.exit_bone_names_for_next_path();

        assert_eq!(
            (start_1.as_str(), end_1.as_str()),
            ("ExitStart01", "ExitEnd01")
        );
        assert_eq!(
            (start_2.as_str(), end_2.as_str()),
            ("ExitStart02", "ExitEnd02")
        );
        assert_eq!(
            (start_3.as_str(), end_3.as_str()),
            ("ExitStart03", "ExitEnd03")
        );
        assert_eq!(
            (start_4.as_str(), end_4.as_str()),
            ("ExitStart01", "ExitEnd01")
        );
        assert_eq!(contain.which_exit_path, 2);
    }
}
