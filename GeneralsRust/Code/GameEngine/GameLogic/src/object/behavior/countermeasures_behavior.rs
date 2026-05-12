//! CountermeasuresBehavior - Rust conversion of C++ CountermeasuresBehavior
//!
//! Handles countermeasure firing when under missile threat, and responsible
//! for diverting missiles to the flares.
//!
//! Author: Kris Morness, April 2003 (Original C++)
//! Rust conversion: 2025

use std::any::Any;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::behavior_module::{
    xfer_update_module_base_state, BehaviorModuleInterface, CountermeasuresBehaviorInterface,
};
use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, Coord3D, DisabledMaskType, ObjectID, Real, UnsignedInt, Vec3D, Xfer,
    XferVersion, FROM_CENTER_2D,
};
use crate::helpers::{
    get_game_logic_random_value_real, TheGameLogic, ThePartitionManager, TheThingFactory,
};
use crate::modules::{UpdateModuleInterface, UpdateSleepTime, UpgradeModuleInterface};
use crate::object::{
    registry::OBJECT_REGISTRY, Object as GameObject, INVALID_ID as OBJECT_INVALID_ID,
};
use crate::upgrade::{UpgradeMask, UpgradeMux, UpgradeMuxData};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as ThingModuleData, NameKeyType, Object as ModuleObject,
    Thing as ModuleThing,
};

pub type ObjectId = ObjectID;
pub const INVALID_OBJECT_ID: ObjectId = OBJECT_INVALID_ID;

pub type BehaviorResult<T> = Result<T, BehaviorError>;

/// Error types for behavior operations
#[derive(Debug, thiserror::Error)]
pub enum BehaviorError {
    #[error("Object not found: {id}")]
    ObjectNotFound { id: ObjectId },
    #[error("Invalid configuration: {message}")]
    InvalidConfig { message: String },
    #[error("Module is disabled")]
    ModuleDisabled,
    #[error("Insufficient resources")]
    InsufficientResources,
}

impl From<Box<dyn std::error::Error + Send + Sync>> for BehaviorError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::InvalidConfig {
            message: error.to_string(),
        }
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_ascii_string(value: &str) -> Result<AsciiString, INIError> {
    Ok(AsciiString::from(&INI::parse_ascii_string(value)?))
}

fn parse_unsigned(value: &str) -> Result<UnsignedInt, INIError> {
    INI::parse_unsigned_int(value)
}

fn parse_real(value: &str) -> Result<Real, INIError> {
    INI::parse_real(value)
}

fn parse_bool(value: &str) -> Result<Bool, INIError> {
    INI::parse_bool(value)
}

fn parse_angle(value: &str) -> Result<Real, INIError> {
    INI::parse_angle_real(value)
}

fn parse_duration_frames(value: &str) -> Result<UnsignedInt, INIError> {
    INI::parse_duration_unsigned_int(value)
}

fn parse_flare_template_name_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.flare_template_name = parse_ascii_string(value)?;
    Ok(())
}

fn parse_flare_bone_base_name_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.flare_bone_base_name = parse_ascii_string(value)?;
    Ok(())
}

fn parse_volley_size_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.volley_size = parse_unsigned(value)?;
    Ok(())
}

fn parse_volley_arc_angle_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.volley_arc_angle = parse_angle(value)?;
    Ok(())
}

fn parse_volley_velocity_factor_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.volley_velocity_factor = parse_real(value)?;
    Ok(())
}

fn parse_delay_between_volleys_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.frames_between_volleys = parse_duration_frames(value)?;
    Ok(())
}

fn parse_number_of_volleys_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.number_of_volleys = parse_unsigned(value)?;
    Ok(())
}

fn parse_reload_time_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.reload_frames = parse_duration_frames(value)?;
    Ok(())
}

fn parse_evasion_rate_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.evasion_rate = INI::parse_percent_to_real(value)?;
    Ok(())
}

fn parse_must_reload_at_airfield_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.must_reload_at_airfield = parse_bool(value)?;
    Ok(())
}

fn parse_missile_decoy_delay_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.missile_decoy_frames = parse_duration_frames(value)?;
    Ok(())
}

fn parse_reaction_launch_latency_field(
    _ini: &mut INI,
    data: &mut CountermeasuresBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    data.countermeasure_reaction_frames = parse_duration_frames(value)?;
    Ok(())
}

const COUNTERMEASURES_BEHAVIOR_FIELDS: &[FieldParse<CountermeasuresBehaviorModuleData>] = &[
    FieldParse {
        token: "FlareTemplateName",
        parse: parse_flare_template_name_field,
    },
    FieldParse {
        token: "FlareBoneBaseName",
        parse: parse_flare_bone_base_name_field,
    },
    FieldParse {
        token: "VolleySize",
        parse: parse_volley_size_field,
    },
    FieldParse {
        token: "VolleyArcAngle",
        parse: parse_volley_arc_angle_field,
    },
    FieldParse {
        token: "VolleyVelocityFactor",
        parse: parse_volley_velocity_factor_field,
    },
    FieldParse {
        token: "DelayBetweenVolleys",
        parse: parse_delay_between_volleys_field,
    },
    FieldParse {
        token: "NumberOfVolleys",
        parse: parse_number_of_volleys_field,
    },
    FieldParse {
        token: "ReloadTime",
        parse: parse_reload_time_field,
    },
    FieldParse {
        token: "EvasionRate",
        parse: parse_evasion_rate_field,
    },
    FieldParse {
        token: "MustReloadAtAirfield",
        parse: parse_must_reload_at_airfield_field,
    },
    FieldParse {
        token: "MissileDecoyDelay",
        parse: parse_missile_decoy_delay_field,
    },
    FieldParse {
        token: "ReactionLaunchLatency",
        parse: parse_reaction_launch_latency_field,
    },
];

/// Minimal Update module data carrier until the shared implementation lands.
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

/// Configuration data for countermeasures behavior
#[derive(Debug, Clone)]
pub struct CountermeasuresBehaviorModuleData {
    pub base: UpdateModuleData,
    pub upgrade_mux_data: UpgradeMuxData,
    pub flare_template_name: AsciiString,
    pub flare_bone_base_name: AsciiString,
    pub evasion_rate: Real,
    pub volley_size: UnsignedInt,
    pub volley_arc_angle: Real,
    pub volley_velocity_factor: Real,
    pub frames_between_volleys: UnsignedInt,
    pub number_of_volleys: UnsignedInt,
    pub reload_frames: UnsignedInt,
    pub missile_decoy_frames: UnsignedInt,
    pub countermeasure_reaction_frames: UnsignedInt,
    pub must_reload_at_airfield: Bool,
}

impl CountermeasuresBehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: UpdateModuleData::new(),
            upgrade_mux_data: UpgradeMuxData::default(),
            flare_template_name: AsciiString::default(),
            flare_bone_base_name: AsciiString::default(),
            evasion_rate: 0.0,
            volley_size: 0,
            volley_arc_angle: 0.0,
            volley_velocity_factor: 1.0,
            frames_between_volleys: 0,
            number_of_volleys: 0,
            reload_frames: 0,
            missile_decoy_frames: 0,
            countermeasure_reaction_frames: 0,
            must_reload_at_airfield: false,
        }
    }

    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, COUNTERMEASURES_BEHAVIOR_FIELDS)?;
        self.upgrade_mux_data.parse_from_ini(ini)?;
        Ok(())
    }
}

impl Default for CountermeasuresBehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl ThingModuleData for CountermeasuresBehaviorModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl Snapshotable for CountermeasuresBehaviorModuleData {
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

/// Internal state for countermeasures
#[derive(Debug)]
struct CountermeasuresState {
    countermeasures: VecDeque<ObjectId>,
    available_countermeasures: u32,
    active_countermeasures: u32,
    diverted_missiles: u32,
    incoming_missiles: u32,
    reaction_frame: u32,
    next_volley_frame: u32,
    reload_frame: u32,
}

impl Default for CountermeasuresState {
    fn default() -> Self {
        Self {
            countermeasures: VecDeque::new(),
            available_countermeasures: 0,
            active_countermeasures: 0,
            diverted_missiles: 0,
            incoming_missiles: 0,
            reaction_frame: 0,
            next_volley_frame: 0,
            reload_frame: 0,
        }
    }
}

/// Thread-safe countermeasures behavior implementation
#[derive(Debug)]
pub struct CountermeasuresBehavior {
    module_data: Arc<CountermeasuresBehaviorModuleData>,
    object_id: ObjectID,
    object_handle: Mutex<Option<Weak<RwLock<GameObject>>>>,
    state: Arc<RwLock<CountermeasuresState>>,
    next_call_frame_and_phase: UnsignedInt,
    upgrade_mux: UpgradeMux,
}

impl CountermeasuresBehavior {
    fn construct_with_object_id(
        object_id: ObjectID,
        module_data: Arc<CountermeasuresBehaviorModuleData>,
        initial_object: Option<Arc<RwLock<GameObject>>>,
    ) -> Self {
        let initial_handle = initial_object
            .or_else(|| OBJECT_REGISTRY.get_object(object_id))
            .map(|arc| Arc::downgrade(&arc));

        let mut state = CountermeasuresState::default();
        state.available_countermeasures = module_data
            .number_of_volleys
            .saturating_mul(module_data.volley_size)
            as u32;
        let upgrade_mux = UpgradeMux::new(module_data.upgrade_mux_data.clone());

        Self {
            module_data,
            object_id,
            object_handle: Mutex::new(initial_handle),
            state: Arc::new(RwLock::new(state)),
            next_call_frame_and_phase: 0,
            upgrade_mux,
        }
    }

    pub fn new(object_id: ObjectID, module_data: Arc<CountermeasuresBehaviorModuleData>) -> Self {
        Self::construct_with_object_id(object_id, module_data, None)
    }

    pub fn new_from_object_handle(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<CountermeasuresBehaviorModuleData>,
    ) -> Self {
        let object_id = object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(OBJECT_INVALID_ID);
        Self::construct_with_object_id(object_id, module_data, Some(object))
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<CountermeasuresBehaviorModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let module_object = thing
            .as_object()
            .ok_or_else(|| "CountermeasuresBehavior requires an owning object".to_string())?;

        let object_id = module_object.get_object_id();

        Ok(Self::new(object_id, module_data))
    }

    pub fn update(&mut self, current_frame: u32) -> BehaviorResult<UpdateSleepTime> {
        let object = self
            .get_object()
            .map_err(|_| BehaviorError::ModuleDisabled)?;
        let obj_guard = object.read().map_err(|_| BehaviorError::ModuleDisabled)?;
        if obj_guard.is_effectively_dead() {
            return Ok(UpdateSleepTime::Forever);
        }
        if !self.upgrade_mux.is_already_upgraded() {
            return Ok(UpdateSleepTime::Forever);
        }
        let is_airborne = obj_guard.is_airborne_target();
        drop(obj_guard);

        let mut state = self.state.write().unwrap();

        self.cleanup_expired_countermeasures(&mut state)?;

        if is_airborne {
            if state.available_countermeasures > 0 {
                if state.reaction_frame > 0 && state.reaction_frame == current_frame {
                    self.launch_volley(&mut state, current_frame)?;
                    state.next_volley_frame =
                        current_frame + self.module_data.frames_between_volleys;
                    state.reaction_frame = 0;
                }

                if state.next_volley_frame > 0 && state.next_volley_frame == current_frame {
                    self.launch_volley(&mut state, current_frame)?;
                    state.next_volley_frame =
                        current_frame + self.module_data.frames_between_volleys;
                }
            }
        }

        if state.available_countermeasures == 0 && self.module_data.reload_frames > 0 {
            if state.reload_frame == 0 {
                state.reload_frame = current_frame + self.module_data.reload_frames;
            } else if state.reload_frame <= current_frame {
                self.reload_countermeasures_internal(&mut state)?;
            }
        }

        Ok(UpdateSleepTime::None)
    }

    fn launch_volley(
        &self,
        state: &mut CountermeasuresState,
        _current_frame: u32,
    ) -> BehaviorResult<()> {
        let volley_size = self.module_data.volley_size;
        if volley_size == 0 {
            return Ok(());
        }

        for i in 0..volley_size {
            let ratio = if volley_size > 1 {
                (i as f32) / ((volley_size - 1) as f32) * 2.0 - 1.0
            } else {
                0.0
            };
            let angle = ratio * self.module_data.volley_arc_angle;
            if let Some(countermeasure_id) = self.create_countermeasure(angle)? {
                state.countermeasures.push_back(countermeasure_id);
                state.active_countermeasures += 1;
                state.available_countermeasures = state.available_countermeasures.saturating_sub(1);
            }
        }

        Ok(())
    }

    fn create_countermeasure(&self, angle: f32) -> BehaviorResult<Option<ObjectId>> {
        let template_name = self.module_data.flare_template_name.as_str();
        if template_name.is_empty() {
            return Ok(None);
        }

        let Some(template) = TheThingFactory::find_template(template_name) else {
            return Ok(None);
        };

        let object = self.get_object()?;
        let (spawn_pos, owner_angle, team_arc, unit_dir, owner_velocity) = {
            let obj_guard = object.read().map_err(|_| BehaviorError::ModuleDisabled)?;
            let pos = *obj_guard.get_position();
            let angle_base = obj_guard.get_orientation();
            let team = obj_guard
                .get_controlling_player()
                .and_then(|player| player.read().ok()?.get_default_team())
                .or_else(|| obj_guard.get_team());
            let unit_dir = obj_guard.get_unit_direction_vector_2d();
            let velocity = obj_guard
                .get_physics()
                .and_then(|physics| physics.lock().ok().map(|phys| phys.get_velocity()))
                .unwrap_or_else(|| Vec3D::new(0.0, 0.0, 0.0));
            (pos, angle_base, team, unit_dir, velocity)
        };

        let Some(team_arc) = team_arc else {
            return Ok(None);
        };

        let team_guard = team_arc.read().map_err(|_| BehaviorError::ModuleDisabled)?;
        let factory = TheThingFactory::get().map_err(|_| BehaviorError::ModuleDisabled)?;
        let flare = factory
            .new_object(template, &*team_guard)
            .map_err(|_| BehaviorError::ModuleDisabled)?;

        let (dir_x, dir_y) = unit_dir;
        let (sin_angle, cos_angle) = angle.sin_cos();
        let rotated_x = dir_x * cos_angle - dir_y * sin_angle;
        let rotated_y = dir_x * sin_angle + dir_y * cos_angle;

        let mut velocity = owner_velocity.length();
        if velocity < 1.0 {
            velocity = -10.0;
        }
        let mut motive = Vec3D::new(rotated_x, rotated_y, 0.0);
        motive *= velocity * self.module_data.volley_velocity_factor;

        {
            let mut flare_guard = flare.write().map_err(|_| BehaviorError::ModuleDisabled)?;
            let _ = flare_guard.set_position(&spawn_pos);
            let _ = flare_guard.set_orientation(owner_angle);
            if let Some(flare_physics) = flare_guard.get_physics() {
                if let Ok(mut physics_guard) = flare_physics.lock() {
                    physics_guard.set_velocity(&owner_velocity);
                    physics_guard.apply_motive_force(&motive);
                }
            }
        }

        let id = flare
            .read()
            .map_err(|_| BehaviorError::ModuleDisabled)?
            .get_id();
        Ok(Some(id))
    }

    fn cleanup_expired_countermeasures(
        &self,
        state: &mut CountermeasuresState,
    ) -> BehaviorResult<()> {
        let mut to_remove = Vec::new();
        for (index, &countermeasure_id) in state.countermeasures.iter().enumerate() {
            if !self.is_object_valid(countermeasure_id) {
                to_remove.push(index);
            }
        }

        for &index in to_remove.iter().rev() {
            state.countermeasures.remove(index);
            state.active_countermeasures = state.active_countermeasures.saturating_sub(1);
        }

        Ok(())
    }

    fn is_object_valid(&self, object_id: ObjectId) -> bool {
        object_id != OBJECT_INVALID_ID && OBJECT_REGISTRY.get_object(object_id).is_some()
    }

    fn reload_countermeasures_internal(
        &self,
        state: &mut CountermeasuresState,
    ) -> BehaviorResult<()> {
        state.available_countermeasures =
            self.module_data
                .number_of_volleys
                .saturating_mul(self.module_data.volley_size) as u32;
        state.reload_frame = 0;
        Ok(())
    }

    fn random_value(&self) -> f32 {
        get_game_logic_random_value_real(0.0, 1.0)
    }

    fn calculate_distance_squared(&self, obj1_id: ObjectId, obj2_id: ObjectId) -> f32 {
        let Some(obj1) = OBJECT_REGISTRY.get_object(obj1_id) else {
            return 100.0;
        };
        let Some(obj2) = OBJECT_REGISTRY.get_object(obj2_id) else {
            return 100.0;
        };
        let Ok(obj1_guard) = obj1.read() else {
            return 100.0;
        };
        let Ok(obj2_guard) = obj2.read() else {
            return 100.0;
        };
        ThePartitionManager::get_distance_squared(&*obj1_guard, &*obj2_guard, FROM_CENTER_2D)
    }

    #[allow(dead_code)]
    fn get_statistics(&self) -> CountermeasuresStatistics {
        let state = self.state.read().unwrap();
        CountermeasuresStatistics {
            available_countermeasures: state.available_countermeasures,
            active_countermeasures: state.active_countermeasures,
            diverted_missiles: state.diverted_missiles,
            incoming_missiles: state.incoming_missiles,
        }
    }

    fn get_object(&self) -> BehaviorResult<Arc<RwLock<GameObject>>> {
        if self.object_id == OBJECT_INVALID_ID {
            return Err(BehaviorError::ObjectNotFound {
                id: OBJECT_INVALID_ID,
            });
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

        Err(BehaviorError::ObjectNotFound { id: self.object_id })
    }

    fn get_current_frame(&self) -> UnsignedInt {
        TheGameLogic::get_frame()
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.upgrade_mux.crc(xfer).map_err(|err| {
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, err))
                as Box<dyn std::error::Error + Send + Sync>
        })?;
        Ok(())
    }

    pub fn xfer(
        &mut self,
        xfer: &mut dyn Xfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_version: XferVersion = 2;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase).map_err(
            |err| {
                Box::new(std::io::Error::new(std::io::ErrorKind::Other, err))
                    as Box<dyn std::error::Error + Send + Sync>
            },
        )?;
        self.upgrade_mux.xfer(xfer).map_err(|err| {
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, err))
                as Box<dyn std::error::Error + Send + Sync>
        })?;

        if current_version >= 2 {
            let mut state = self.state.write().unwrap();
            let mut count: UnsignedInt = state.countermeasures.len() as UnsignedInt;
            xfer.xfer_unsigned_int(&mut count)?;
            if xfer.is_loading() {
                state.countermeasures.clear();
            }
            if xfer.is_loading() {
                for _ in 0..count {
                    let mut id: UnsignedInt = 0;
                    xfer.xfer_unsigned_int(&mut id)?;
                    state.countermeasures.push_back(id);
                }
            } else {
                for id in state.countermeasures.iter() {
                    let mut tmp = *id;
                    xfer.xfer_unsigned_int(&mut tmp)?;
                }
            }

            xfer.xfer_unsigned_int(&mut state.available_countermeasures)?;
            xfer.xfer_unsigned_int(&mut state.active_countermeasures)?;
            xfer.xfer_unsigned_int(&mut state.diverted_missiles)?;
            xfer.xfer_unsigned_int(&mut state.incoming_missiles)?;
            xfer.xfer_unsigned_int(&mut state.reaction_frame)?;
            xfer.xfer_unsigned_int(&mut state.next_volley_frame)?;
        }
        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.upgrade_mux.load_post_process().map_err(|err| {
            Box::new(std::io::Error::new(std::io::ErrorKind::Other, err))
                as Box<dyn std::error::Error + Send + Sync>
        })?;
        Ok(())
    }
}

impl CountermeasuresBehaviorInterface for CountermeasuresBehavior {
    fn report_missile_for_countermeasures(
        &mut self,
        missile_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if missile_id == OBJECT_INVALID_ID {
            return Ok(());
        }
        let mut state = self.state.write().unwrap();
        state.incoming_missiles += 1;

        if state.available_countermeasures + state.active_countermeasures > 0 {
            if self.random_value() < self.module_data.evasion_rate {
                debug_assert!(
                    self.module_data.countermeasure_reaction_frames
                        < self.module_data.missile_decoy_frames,
                    "MissileDecoyDelay must be larger than ReactionLaunchLatency"
                );

                let Some(missile_arc) = TheGameLogic::find_object_by_id(missile_id) else {
                    return Ok(());
                };
                let Ok(missile_guard) = missile_arc.write() else {
                    return Ok(());
                };

                let current_frame = self.get_current_frame();
                let modules = missile_guard.behavior_modules();
                drop(missile_guard);

                let mut diverted = false;
                for module in modules {
                    let matched = module.with_module_downcast::<
                        crate::object::update::missile_ai_update::MissileAIUpdateBehavior,
                        _,
                        _,
                    >(|missile| {
                        missile.set_frames_till_countermeasure_diversion_occurs(
                            self.module_data.missile_decoy_frames,
                            current_frame,
                        );
                    });
                    if matched.is_some() {
                        diverted = true;
                        break;
                    }
                }

                if diverted {
                    state.diverted_missiles += 1;

                    if state.active_countermeasures == 0 && state.reaction_frame == 0 {
                        state.reaction_frame =
                            current_frame + self.module_data.countermeasure_reaction_frames;
                    }
                }
            }
        }

        Ok(())
    }

    fn calculate_countermeasure_to_divert_to(
        &self,
        _victim_id: ObjectID,
    ) -> Result<ObjectID, Box<dyn std::error::Error + Send + Sync>> {
        let state = self.state.read().unwrap();
        let max_check = std::cmp::max(self.module_data.volley_size as usize, 1);
        let mut closest_distance_sq = f32::INFINITY;
        let mut closest_countermeasure = INVALID_OBJECT_ID;

        for &countermeasure_id in state.countermeasures.iter().rev().take(max_check) {
            if self.is_object_valid(countermeasure_id) {
                let distance_sq =
                    self.calculate_distance_squared(self.object_id, countermeasure_id);
                if distance_sq < closest_distance_sq {
                    closest_distance_sq = distance_sq;
                    closest_countermeasure = countermeasure_id;
                }
            }
        }

        Ok(closest_countermeasure)
    }

    fn reload_countermeasures(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.state.write().unwrap();
        self.reload_countermeasures_internal(&mut state)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    fn is_active(&self) -> bool {
        self.upgrade_mux.is_already_upgraded()
    }
}

impl UpdateModuleInterface for CountermeasuresBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let current_frame = self.get_current_frame();
        self.update(current_frame)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
    }

    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::HELD
    }
}

impl UpgradeModuleInterface for CountermeasuresBehavior {
    fn can_upgrade(&self, upgrade_mask: crate::common::UpgradeMaskType) -> bool {
        let mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        self.upgrade_mux.test_upgrade_conditions(mask)
    }

    fn apply_upgrade(&mut self, upgrade_mask: crate::common::UpgradeMaskType) -> bool {
        let Ok(object_arc) = self.get_object() else {
            return false;
        };
        let Ok(mut object_guard) = object_arc.write() else {
            return false;
        };
        let mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        if self.upgrade_mux.attempt_upgrade(mask, &mut object_guard) {
            TheGameLogic::set_wake_frame(object_guard.get_id(), UpdateSleepTime::None);
            true
        } else {
            false
        }
    }

    fn remove_upgrade(&mut self, upgrade_mask: crate::common::UpgradeMaskType) {
        let mask = UpgradeMask::from_bits_retain(upgrade_mask.bits());
        let _ = self.upgrade_mux.reset_upgrade(mask);
    }
}

impl BehaviorModuleInterface for CountermeasuresBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_upgrade(&mut self) -> Option<&mut dyn UpgradeModuleInterface> {
        Some(self)
    }

    fn get_countermeasures_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn CountermeasuresBehaviorInterface> {
        Some(self)
    }

    fn get_countermeasures_behavior_interface_const(
        &self,
    ) -> Option<&dyn CountermeasuresBehaviorInterface> {
        Some(self)
    }
}

/// Glue object that exposes CountermeasuresBehavior through the shared Module trait.
pub struct CountermeasuresBehaviorModule {
    behavior: CountermeasuresBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<CountermeasuresBehaviorModuleData>,
}

impl CountermeasuresBehaviorModule {
    pub fn new(
        behavior: CountermeasuresBehavior,
        module_name: &AsciiString,
        module_data: Arc<CountermeasuresBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &CountermeasuresBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut CountermeasuresBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for CountermeasuresBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer).map_err(|err| err.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer).map_err(|err| err.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior
            .load_post_process()
            .map_err(|err| err.to_string())
    }
}

impl EngineModule for CountermeasuresBehaviorModule {
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

/// Statistics for countermeasures behavior
#[derive(Debug, Clone)]
pub struct CountermeasuresStatistics {
    pub available_countermeasures: u32,
    pub active_countermeasures: u32,
    pub diverted_missiles: u32,
    pub incoming_missiles: u32,
}

/// Factory for creating CountermeasuresBehavior instances
pub struct CountermeasuresBehaviorFactory;

impl CountermeasuresBehaviorFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn crate::common::ModuleData>,
    ) -> Result<
        Box<dyn crate::modules::BehaviorModuleInterface>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let data = module_data
            .as_any()
            .downcast_ref::<CountermeasuresBehaviorModuleData>()
            .ok_or("Invalid module data type for CountermeasuresBehavior")?
            .clone();

        let behavior = CountermeasuresBehavior::new_from_object_handle(thing, Arc::new(data));
        Ok(Box::new(behavior))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn create_test_behavior() -> CountermeasuresBehavior {
        let mut data = CountermeasuresBehaviorModuleData::default();
        data.number_of_volleys = 3;
        data.volley_size = 4;
        data.frames_between_volleys = 5;
        data.reload_frames = 30;
        data.countermeasure_reaction_frames = 3;
        data.evasion_rate = 0.5;
        let data_arc = Arc::new(data);
        CountermeasuresBehavior::new(1, data_arc)
    }

    #[test]
    fn behavior_initializes_available_countermeasures() {
        let behavior = create_test_behavior();
        let stats = behavior.get_statistics();
        assert_eq!(stats.available_countermeasures, 12);
    }

    #[test]
    fn reload_sets_available_countermeasures() {
        let mut behavior = create_test_behavior();
        {
            let mut state = behavior.state.write().unwrap();
            state.available_countermeasures = 0;
        }
        behavior.reload_countermeasures().unwrap();
        assert_eq!(behavior.get_statistics().available_countermeasures, 12);
    }

    #[test]
    fn random_value_is_between_zero_and_one() {
        let behavior = create_test_behavior();
        let value = behavior.random_value();
        assert!(value >= 0.0 && value <= 1.0);
    }

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames("1500ms").expect("duration"), 45);
        assert_eq!(parse_duration_frames("1.5s").expect("duration"), 45);
    }

    #[test]
    fn xfer_preserves_cpp_countermeasure_runtime_fields_only() {
        let mut saved = create_test_behavior();
        saved.next_call_frame_and_phase = 0x6721;
        {
            let mut state = saved.state.write().unwrap();
            state.countermeasures.push_back(10);
            state.countermeasures.push_back(20);
            state.available_countermeasures = 7;
            state.active_countermeasures = 2;
            state.diverted_missiles = 3;
            state.incoming_missiles = 4;
            state.reaction_frame = 100;
            state.next_volley_frame = 125;
            state.reload_frame = 900;
        }

        let mut bytes = Cursor::new(Vec::new());
        {
            let mut xfer = XferSave::new(&mut bytes, 1);
            saved.xfer(&mut xfer).unwrap();
        }

        bytes.set_position(0);
        let mut loaded = create_test_behavior();
        loaded.next_call_frame_and_phase = 0;
        {
            let mut state = loaded.state.write().unwrap();
            state.reload_frame = 55;
        }
        {
            let mut xfer = XferLoad::new(&mut bytes, 1);
            loaded.xfer(&mut xfer).unwrap();
        }

        assert_eq!(loaded.next_call_frame_and_phase, 0x6721);
        let state = loaded.state.read().unwrap();
        assert_eq!(
            state.countermeasures.iter().copied().collect::<Vec<_>>(),
            vec![10, 20]
        );
        assert_eq!(state.available_countermeasures, 7);
        assert_eq!(state.active_countermeasures, 2);
        assert_eq!(state.diverted_missiles, 3);
        assert_eq!(state.incoming_missiles, 4);
        assert_eq!(state.reaction_frame, 100);
        assert_eq!(state.next_volley_frame, 125);
        assert_eq!(state.reload_frame, 55);
    }
}
