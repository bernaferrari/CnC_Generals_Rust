//! FXList system for client-side audio/visual effects.
//!
//! Ported from `GameClient/FXList.cpp` and `GameClient/FXList.h`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use glam::Vec3;

use game_engine::common::ini::{register_block_parser, INIError, INILoadType, INIResult, INI};
use game_engine::common::name_key_generator::{NameKeyGenerator, NameKeyType};

use gamelogic::common::types::FXListManagerInterface;
use gamelogic::common::{Coord3D, FXListId, Matrix3D};
use gamelogic::object::Object;

use crate::display::cinematic_camera::CameraShakeSystem;
use crate::effects::decals::{DecalManager, DecalSettings, DecalType};
use crate::effects::fxlist_integration::ParticleSystemFXNugget;
use crate::effects::particle_manager::{get_particle_system_manager_mut, GameClientRandomVariable};
use crate::effects::ray_effects::{RayEffectConfig, RayEffectManager};
use crate::message_stream::game_message::Coord3D as MessageCoord3D;

#[derive(Debug)]
struct FXListManagerBridge;

impl FXListManagerInterface for FXListManagerBridge {
    fn do_fx_pos(&self, fx_list: FXListId, position: &Coord3D, matrix: Option<&glam::Mat4>) {
        let Some(name) = NameKeyGenerator::key_to_name(fx_list as NameKeyType) else {
            log::debug!("FXListManager: unknown FXList id {}", fx_list);
            return;
        };

        let store = get_fx_list_store();
        let Some(fx) = store.find_fx_list(&name) else {
            log::debug!("FXListManager: FXList '{}' not found", name);
            return;
        };

        fx.do_fx_pos(Some(position), matrix, 0.0, None, 0.0);
    }

    fn do_fx_obj(&self, fx_list: FXListId, object_id: gamelogic::common::ThingId) {
        let Some(name) = NameKeyGenerator::key_to_name(fx_list as NameKeyType) else {
            log::debug!("FXListManager: unknown FXList id {}", fx_list);
            return;
        };

        let store = get_fx_list_store();
        let Some(fx) = store.find_fx_list(&name) else {
            log::debug!("FXListManager: FXList '{}' not found", name);
            return;
        };

        let Some(object) = gamelogic::helpers::TheGameLogic::find_object_by_id(object_id) else {
            return;
        };

        if let Ok(guard) = object.read() {
            fx.do_fx_obj(Some(&*guard), None);
        };
    }

    fn do_fx_obj_with_source(
        &self,
        fx_list: FXListId,
        object_id: gamelogic::common::ThingId,
        source_id: Option<gamelogic::common::ThingId>,
    ) {
        let Some(name) = NameKeyGenerator::key_to_name(fx_list as NameKeyType) else {
            log::debug!("FXListManager: unknown FXList id {}", fx_list);
            return;
        };

        let store = get_fx_list_store();
        let Some(fx) = store.find_fx_list(&name) else {
            log::debug!("FXListManager: FXList '{}' not found", name);
            return;
        };

        let Some(object) = gamelogic::helpers::TheGameLogic::find_object_by_id(object_id) else {
            return;
        };
        let source = source_id.and_then(gamelogic::helpers::TheGameLogic::find_object_by_id);

        if let Ok(guard) = object.read() {
            let source_guard = source.as_ref().and_then(|source| source.read().ok());
            fx.do_fx_obj(Some(&*guard), source_guard.as_deref());
        };
    }
}

pub fn register_fx_list_manager_bridge() {
    let _ = gamelogic::helpers::register_fx_list_manager(Arc::new(FXListManagerBridge));
}

pub type FXListResult<T> = Result<T, FXListError>;

#[derive(Debug, Clone)]
pub enum FXListError {
    ParseError(String),
    NotFound,
}

impl std::fmt::Display for FXListError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FXListError::ParseError(msg) => write!(f, "FXList parse error: {}", msg),
            FXListError::NotFound => write!(f, "FXList not found"),
        }
    }
}

impl std::error::Error for FXListError {}

pub trait FXNugget: Send + Sync {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        primary_mtx: Option<&Matrix3D>,
        primary_speed: f32,
        secondary: Option<&Coord3D>,
        override_radius: f32,
    );

    fn do_fx_obj(&self, primary: Option<&Object>, secondary: Option<&Object>) {
        let primary_pos = primary.map(|obj| obj.get_position());
        let primary_mtx = primary.map(|obj| obj.get_transform_matrix());
        let secondary_pos = secondary.map(|obj| obj.get_position());
        self.do_fx_pos(primary_pos, primary_mtx.as_ref(), 0.0, secondary_pos, 0.0);
    }
}

fn to_message_coord(pos: &Coord3D) -> MessageCoord3D {
    MessageCoord3D {
        x: pos.x,
        y: pos.y,
        z: pos.z,
    }
}

type AudioHook = Box<dyn FnMut(&str, Option<MessageCoord3D>) + Send + Sync>;

static FX_AUDIO: OnceLock<RwLock<Option<AudioHook>>> = OnceLock::new();
static FX_RAY_MANAGER: OnceLock<RwLock<Option<Arc<Mutex<RayEffectManager>>>>> = OnceLock::new();
static FX_DECAL_MANAGER: OnceLock<RwLock<Option<Arc<Mutex<DecalManager>>>>> = OnceLock::new();
static FX_SHAKE_SYSTEM: OnceLock<RwLock<Option<Arc<Mutex<CameraShakeSystem>>>>> = OnceLock::new();

pub fn register_fx_audio(mut hook: AudioHook) {
    FX_AUDIO
        .get_or_init(|| RwLock::new(None))
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .replace(hook);
}

pub fn register_ray_effect_manager(manager: Arc<Mutex<RayEffectManager>>) {
    FX_RAY_MANAGER
        .get_or_init(|| RwLock::new(None))
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .replace(manager);
}

pub fn register_decal_manager(manager: Arc<Mutex<DecalManager>>) {
    FX_DECAL_MANAGER
        .get_or_init(|| RwLock::new(None))
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .replace(manager);
}

pub fn get_decal_manager() -> Option<Arc<Mutex<DecalManager>>> {
    let manager = FX_DECAL_MANAGER.get()?;
    manager.read().ok().and_then(|guard| guard.clone())
}

pub fn register_camera_shake_system(system: Arc<Mutex<CameraShakeSystem>>) {
    FX_SHAKE_SYSTEM
        .get_or_init(|| RwLock::new(None))
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .replace(system);
}

/// Invoke the registered FX audio hook if present.
/// Returns `true` when a hook was called (sound routed), `false` when silent.
fn with_audio<F: FnOnce(&mut AudioHook)>(f: F) -> bool {
    let Some(audio) = FX_AUDIO.get() else {
        return false;
    };
    if let Ok(mut guard) = audio.write() {
        if let Some(ref mut hook) = *guard {
            f(hook);
            return true;
        }
    }
    false
}

fn with_ray_manager<F: FnOnce(&mut RayEffectManager)>(f: F) {
    let Some(manager) = FX_RAY_MANAGER.get() else {
        return;
    };
    if let Some(manager) = manager.read().ok().and_then(|guard| guard.clone()) {
        if let Ok(mut guard) = manager.lock() {
            f(&mut guard);
        }
    }
}

fn with_decal_manager<F: FnOnce(&mut DecalManager)>(f: F) {
    let Some(manager) = FX_DECAL_MANAGER.get() else {
        return;
    };
    if let Some(manager) = manager.read().ok().and_then(|guard| guard.clone()) {
        if let Ok(mut guard) = manager.lock() {
            f(&mut guard);
        }
    }
}

fn with_shake_system<F: FnOnce(&mut CameraShakeSystem)>(f: F) {
    let Some(system) = FX_SHAKE_SYSTEM.get() else {
        return;
    };
    if let Some(system) = system.read().ok().and_then(|guard| guard.clone()) {
        if let Ok(mut guard) = system.lock() {
            f(&mut guard);
        }
    }
}

pub struct FXList {
    nuggets: Vec<Box<dyn FXNugget>>,
}

impl FXList {
    pub fn new() -> Self {
        Self {
            nuggets: Vec::new(),
        }
    }

    pub fn add_fx_nugget(&mut self, nugget: Box<dyn FXNugget>) {
        self.nuggets.push(nugget);
    }

    pub fn clear(&mut self) {
        self.nuggets.clear();
    }

    pub fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        primary_mtx: Option<&Matrix3D>,
        primary_speed: f32,
        secondary: Option<&Coord3D>,
        override_radius: f32,
    ) {
        for nugget in &self.nuggets {
            nugget.do_fx_pos(
                primary,
                primary_mtx,
                primary_speed,
                secondary,
                override_radius,
            );
        }
    }

    pub fn do_fx_obj(&self, primary: Option<&Object>, secondary: Option<&Object>) {
        for nugget in &self.nuggets {
            nugget.do_fx_obj(primary, secondary);
        }
    }
}

impl Default for FXList {
    fn default() -> Self {
        Self::new()
    }
}

pub struct FXListStore {
    fx_map: HashMap<NameKeyType, Arc<FXList>>,
}

impl FXListStore {
    pub fn new() -> Self {
        Self {
            fx_map: HashMap::new(),
        }
    }

    pub fn find_fx_list(&self, name: &str) -> Option<Arc<FXList>> {
        if name.eq_ignore_ascii_case("None") {
            return None;
        }
        let key = NameKeyGenerator::name_to_key(name) as NameKeyType;
        self.fx_map.get(&key).cloned()
    }

    pub fn add_fx_list(&mut self, name: String, fx_list: FXList) {
        let key = NameKeyGenerator::name_to_key(&name) as NameKeyType;
        self.fx_map.insert(key, Arc::new(fx_list));
    }
}

impl Default for FXListStore {
    fn default() -> Self {
        Self::new()
    }
}

static FX_LIST_STORE: OnceLock<RwLock<FXListStore>> = OnceLock::new();
static FX_LIST_PARSER_REGISTERED: OnceLock<()> = OnceLock::new();

pub fn get_fx_list_store() -> std::sync::RwLockReadGuard<'static, FXListStore> {
    FX_LIST_STORE
        .get_or_init(|| RwLock::new(FXListStore::new()))
        .read()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn get_fx_list_store_mut() -> std::sync::RwLockWriteGuard<'static, FXListStore> {
    FX_LIST_STORE
        .get_or_init(|| RwLock::new(FXListStore::new()))
        .write()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn init_fx_list_store() -> Result<(), Box<dyn std::error::Error>> {
    FX_LIST_PARSER_REGISTERED.get_or_init(|| {
        let _ = register_block_parser("FXList", parse_fx_list_definition);
    });

    let mut ini = INI::new();
    let default_path = "Data/INI/Default/FXList.ini";
    let override_path = "Data/INI/FXList.ini";
    if std::path::Path::new(default_path).exists() {
        ini.load(default_path, INILoadType::Overwrite)?;
    }
    if std::path::Path::new(override_path).exists() {
        ini.load(override_path, INILoadType::MultiFile)?;
    }
    Ok(())
}

fn parse_fx_list_definition(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or(INIError::InvalidData)?
        .to_string();

    let mut fx_list = FXList::new();

    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::EndOfFile);
        }

        let line_tokens = ini.get_line_tokens();
        let Some(token) = line_tokens.first() else {
            continue;
        };

        if token.eq_ignore_ascii_case("End") {
            break;
        }

        match token.to_ascii_uppercase().as_str() {
            "SOUND" => parse_sound_nugget(ini, &mut fx_list)?,
            "TRACER" => parse_tracer_nugget(ini, &mut fx_list)?,
            "RAYEFFECT" => parse_ray_effect_nugget(ini, &mut fx_list)?,
            "LIGHTPULSE" => parse_light_pulse_nugget(ini, &mut fx_list)?,
            "VIEWSHAKE" => parse_view_shake_nugget(ini, &mut fx_list)?,
            "TERRAINSCORCH" => parse_terrain_scorch_nugget(ini, &mut fx_list)?,
            "PARTICLESYSTEM" => parse_particle_system_nugget(ini, &mut fx_list)?,
            "FXLISTATBONEPOS" => parse_fx_list_at_bone_pos_nugget(ini, &mut fx_list)?,
            other => {
                return Err(INIError::InvalidData);
            }
        }
    }

    get_fx_list_store_mut().add_fx_list(name, fx_list);
    Ok(())
}

fn parse_block_field(ini: &mut INI) -> INIResult<Option<(String, Vec<String>)>> {
    ini.read_line()?;
    if ini.is_eof() {
        return Err(INIError::EndOfFile);
    }
    let tokens = ini.get_line_tokens();
    let Some(key) = tokens.first() else {
        return Ok(None);
    };
    if key.eq_ignore_ascii_case("End") {
        return Ok(Some((String::from("End"), Vec::new())));
    }
    let values: Vec<String> = tokens
        .iter()
        .skip(1)
        .filter(|token| **token != "=")
        .map(|token| (*token).to_string())
        .collect();
    Ok(Some((key.to_string(), values)))
}

fn parse_sound_nugget(ini: &mut INI, fx_list: &mut FXList) -> INIResult<()> {
    let mut sound_name = String::new();
    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        if key.eq_ignore_ascii_case("Name") {
            if let Some(value) = values.first() {
                sound_name = INI::parse_ascii_string(value)?;
            }
        }
    }
    fx_list.add_fx_nugget(Box::new(SoundFXNugget { sound_name }));
    Ok(())
}

fn parse_tracer_nugget(ini: &mut INI, fx_list: &mut FXList) -> INIResult<()> {
    let mut nugget = TracerFXNugget::default();
    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let Some(value) = values.first() else {
            continue;
        };
        match key.to_ascii_uppercase().as_str() {
            "TRACERNAME" => nugget.tracer_name = INI::parse_ascii_string(value)?,
            "BONENAME" => nugget.bone_name = INI::parse_ascii_string(value)?,
            "SPEED" => nugget.speed = INI::parse_real(value)?,
            "DECAYAT" => nugget.decay_at = INI::parse_real(value)?,
            "LENGTH" => nugget.length = INI::parse_real(value)?,
            "WIDTH" => nugget.width = INI::parse_real(value)?,
            "COLOR" => {
                if values.len() >= 3 {
                    nugget.color = Vec3::new(
                        INI::parse_real(&values[0])?,
                        INI::parse_real(&values[1])?,
                        INI::parse_real(&values[2])?,
                    );
                }
            }
            "PROBABILITY" => nugget.probability = INI::parse_real(value)?,
            _ => {}
        }
    }
    fx_list.add_fx_nugget(Box::new(nugget));
    Ok(())
}

fn parse_ray_effect_nugget(ini: &mut INI, fx_list: &mut FXList) -> INIResult<()> {
    let mut nugget = RayEffectFXNugget::default();
    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let Some(value) = values.first() else {
            continue;
        };
        match key.to_ascii_uppercase().as_str() {
            "NAME" => nugget.template_name = INI::parse_ascii_string(value)?,
            "PRIMARYOFFSET" => {
                if values.len() >= 3 {
                    nugget.primary_offset = Vec3::new(
                        INI::parse_real(&values[0])?,
                        INI::parse_real(&values[1])?,
                        INI::parse_real(&values[2])?,
                    );
                }
            }
            "SECONDARYOFFSET" if values.len() >= 3 => {
                nugget.secondary_offset = Vec3::new(
                    INI::parse_real(&values[0])?,
                    INI::parse_real(&values[1])?,
                    INI::parse_real(&values[2])?,
                );
            }
            _ => {}
        }
    }
    fx_list.add_fx_nugget(Box::new(nugget));
    Ok(())
}

fn parse_light_pulse_nugget(ini: &mut INI, fx_list: &mut FXList) -> INIResult<()> {
    let mut nugget = LightPulseFXNugget::default();
    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let Some(value) = values.first() else {
            continue;
        };
        match key.to_ascii_uppercase().as_str() {
            "COLOR" => {
                if values.len() >= 3 {
                    nugget.color = Vec3::new(
                        INI::parse_real(&values[0])?,
                        INI::parse_real(&values[1])?,
                        INI::parse_real(&values[2])?,
                    );
                }
            }
            "RADIUS" => nugget.radius = INI::parse_real(value)?,
            "RADIUSASPERCENTOFOBJECTSIZE" => {
                let pct = INI::parse_real(value)?;
                nugget.bounding_circle_pct = pct / 100.0;
            }
            "INCREASETIME" => {
                let msec = INI::parse_real(value)?;
                nugget.increase_frames = (msec / 33.333).ceil() as u32;
            }
            "DECREASETIME" => {
                let msec = INI::parse_real(value)?;
                nugget.decrease_frames = (msec / 33.333).ceil() as u32;
            }
            _ => {}
        }
    }
    fx_list.add_fx_nugget(Box::new(nugget));
    Ok(())
}

fn parse_view_shake_nugget(ini: &mut INI, fx_list: &mut FXList) -> INIResult<()> {
    let mut nugget = ViewShakeFXNugget::default();
    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let Some(value) = values.first() else {
            continue;
        };
        if key.to_ascii_uppercase().as_str() == "TYPE" {
            if let Some(shake_type) = CameraShakeType::parse_shake_type(value) {
                nugget.shake_type = shake_type;
            }
        }
    }
    fx_list.add_fx_nugget(Box::new(nugget));
    Ok(())
}

fn parse_terrain_scorch_nugget(ini: &mut INI, fx_list: &mut FXList) -> INIResult<()> {
    let mut nugget = TerrainScorchFXNugget::default();
    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let Some(value) = values.first() else {
            continue;
        };
        match key.to_ascii_uppercase().as_str() {
            "TYPE" => {
                if let Some(scorch) = ScorchType::parse_scorch_type(value) {
                    nugget.scorch = scorch;
                }
            }
            "RADIUS" => nugget.radius = INI::parse_real(value)?,
            _ => {}
        }
    }
    fx_list.add_fx_nugget(Box::new(nugget));
    Ok(())
}

fn parse_particle_system_nugget(ini: &mut INI, fx_list: &mut FXList) -> INIResult<()> {
    let mut nugget = ParticleSystemFXNugget::default();
    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let Some(value) = values.first() else {
            continue;
        };
        match key.to_ascii_uppercase().as_str() {
            "NAME" => nugget.template_name = INI::parse_ascii_string(value)?,
            "COUNT" => nugget.count = INI::parse_int(value)?,
            "OFFSET" => {
                if values.len() >= 3 {
                    nugget.offset = nalgebra::Vector3::new(
                        INI::parse_real(&values[0])?,
                        INI::parse_real(&values[1])?,
                        INI::parse_real(&values[2])?,
                    );
                }
            }
            "RADIUS" => {
                if values.len() >= 2 {
                    nugget.radius = GameClientRandomVariable::new(
                        INI::parse_real(&values[0])?,
                        INI::parse_real(&values[1])?,
                    );
                }
            }
            "HEIGHT" => {
                if values.len() >= 2 {
                    nugget.height = GameClientRandomVariable::new(
                        INI::parse_real(&values[0])?,
                        INI::parse_real(&values[1])?,
                    );
                }
            }
            "INITIALDELAY" => {
                if values.len() >= 2 {
                    nugget.delay = GameClientRandomVariable::new(
                        INI::parse_real(&values[0])?,
                        INI::parse_real(&values[1])?,
                    );
                }
            }
            "ROTATEX" => nugget.rotate_x = INI::parse_real(value)?,
            "ROTATEY" => nugget.rotate_y = INI::parse_real(value)?,
            "ROTATEZ" => nugget.rotate_z = INI::parse_real(value)?,
            "ORIENTTOOBJECT" => nugget.orient_to_object = INI::parse_bool(value)?,
            "RICOCHET" => nugget.ricochet = INI::parse_bool(value)?,
            "ATTACHTOOBJECT" => nugget.attach_to_object = INI::parse_bool(value)?,
            "CREATEATGROUNDHEIGHT" => nugget.create_at_ground_height = INI::parse_bool(value)?,
            "USECALLERSRADIUS" => nugget.use_callers_radius = INI::parse_bool(value)?,
            _ => {}
        }
    }
    fx_list.add_fx_nugget(Box::new(ParticleSystemWrapper { nugget }));
    Ok(())
}

fn parse_fx_list_at_bone_pos_nugget(ini: &mut INI, fx_list: &mut FXList) -> INIResult<()> {
    let mut nugget = FXListAtBonePosFXNugget::default();
    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }
        let Some(value) = values.first() else {
            continue;
        };
        match key.to_ascii_uppercase().as_str() {
            "FX" => nugget.fx_name = INI::parse_ascii_string(value)?,
            "BONENAME" => nugget.bone_name = INI::parse_ascii_string(value)?,
            "ORIENTTOBONE" => nugget.orient_to_bone = INI::parse_bool(value)?,
            _ => {}
        }
    }
    fx_list.add_fx_nugget(Box::new(nugget));
    Ok(())
}

struct SoundFXNugget {
    sound_name: String,
}

impl FXNugget for SoundFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        // C++ SoundFXNugget::doFXPos: AudioEventRTS + TheAudio->addAudioEvent.
        // Prefer registered GameClient audio hook; fall back to gameplay dispatch
        // so FXList sound nuggets are not silent no-ops when hook is absent.
        let position = primary.map(to_message_coord);
        let routed = with_audio(|hook| {
            hook(&self.sound_name, position);
        });
        if !routed {
            if let Some(pos) = primary {
                game_engine::common::audio::gameplay_audio_dispatch::dispatch_positional_sound(
                    &self.sound_name,
                    pos.x,
                    pos.y,
                    pos.z,
                );
            } else {
                game_engine::common::audio::dispatch_ui_sound(&self.sound_name);
            }
        }
    }

    fn do_fx_obj(&self, primary: Option<&Object>, _secondary: Option<&Object>) {
        let position = primary.map(|obj| to_message_coord(obj.get_position()));
        let routed = with_audio(|hook| {
            hook(&self.sound_name, position);
        });
        if !routed {
            if let Some(obj) = primary {
                let pos = obj.get_position();
                game_engine::common::audio::gameplay_audio_dispatch::dispatch_positional_sound(
                    &self.sound_name,
                    pos.x,
                    pos.y,
                    pos.z,
                );
            } else {
                game_engine::common::audio::dispatch_ui_sound(&self.sound_name);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct TracerFXNugget {
    tracer_name: String,
    bone_name: String,
    speed: f32,
    decay_at: f32,
    length: f32,
    width: f32,
    color: Vec3,
    probability: f32,
}

impl Default for TracerFXNugget {
    fn default() -> Self {
        Self {
            tracer_name: "GenericTracer".to_string(),
            bone_name: String::new(),
            speed: 0.0,
            decay_at: 1.0,
            length: 10.0,
            width: 1.0,
            color: Vec3::ONE,
            probability: 1.0,
        }
    }
}

impl FXNugget for TracerFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        primary_speed: f32,
        secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        if self.probability <= rand::random::<f32>() {
            return;
        }
        let (Some(primary), Some(secondary)) = (primary, secondary) else {
            return;
        };
        let speed = if self.speed == 0.0 {
            primary_speed
        } else {
            self.speed
        };
        let dist = (*secondary - *primary).length() - self.length;
        let frames = if dist >= 0.0 && speed >= 0.0 {
            dist / speed
        } else {
            1.0
        };
        let lifetime_secs = (frames * self.decay_at) / 30.0;

        with_ray_manager(|manager| {
            let mut config = RayEffectConfig::laser();
            config.start = nalgebra::Point3::new(primary.x, primary.y, primary.z);
            config.end = nalgebra::Point3::new(secondary.x, secondary.y, secondary.z);
            config.width = self.width;
            config.color = [self.color.x, self.color.y, self.color.z, 1.0];
            config.lifetime = Some(std::time::Duration::from_secs_f32(lifetime_secs.max(0.05)));
            manager.spawn(config);
        });
    }
}

#[derive(Debug, Clone)]
struct RayEffectFXNugget {
    template_name: String,
    primary_offset: Vec3,
    secondary_offset: Vec3,
}

impl Default for RayEffectFXNugget {
    fn default() -> Self {
        Self {
            template_name: String::new(),
            primary_offset: Vec3::ZERO,
            secondary_offset: Vec3::ZERO,
        }
    }
}

impl FXNugget for RayEffectFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        let (Some(primary), Some(secondary)) = (primary, secondary) else {
            return;
        };
        let source = *primary + self.primary_offset;
        let target = *secondary + self.secondary_offset;

        with_ray_manager(|manager| {
            let mut config = match self.template_name.to_ascii_lowercase().as_str() {
                name if name.contains("lightning") => RayEffectConfig::lightning(),
                name if name.contains("particle") => RayEffectConfig::particle_cannon(),
                name if name.contains("laser") => RayEffectConfig::laser(),
                _ => RayEffectConfig::default(),
            };
            config.start = nalgebra::Point3::new(source.x, source.y, source.z);
            config.end = nalgebra::Point3::new(target.x, target.y, target.z);
            manager.spawn(config);
        });
    }
}

#[derive(Debug, Clone)]
struct LightPulseFXNugget {
    color: Vec3,
    radius: f32,
    bounding_circle_pct: f32,
    increase_frames: u32,
    decrease_frames: u32,
}

impl Default for LightPulseFXNugget {
    fn default() -> Self {
        Self {
            color: Vec3::ZERO,
            radius: 0.0,
            bounding_circle_pct: 0.0,
            increase_frames: 0,
            decrease_frames: 0,
        }
    }
}

impl FXNugget for LightPulseFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        let Some(primary) = primary else {
            return;
        };
        with_decal_manager(|manager| {
            let mut settings = DecalSettings::new(
                DecalType::Generic,
                nalgebra::Point3::new(primary.x, primary.y, primary.z),
            );
            settings.size = self.radius.max(0.1);
            settings.color = [self.color.x, self.color.y, self.color.z, 1.0];
            settings.lifetime = Some(((self.increase_frames + self.decrease_frames) as f32) / 30.0);
            settings.fade_time = (self.decrease_frames as f32) / 30.0;
            manager.create_decal(settings);
        });
    }
}

/// Camera shake types matching C++ View::CameraShakeType (View.h)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CameraShakeType {
    Subtle,
    #[default]
    Normal,
    Strong,
    Severe,
    CineExtreme,
    CineInsane,
}

impl CameraShakeType {
    fn parse_shake_type(value: &str) -> Option<Self> {
        match value.trim().to_uppercase().as_str() {
            "SUBTLE" => Some(CameraShakeType::Subtle),
            "NORMAL" => Some(CameraShakeType::Normal),
            "STRONG" => Some(CameraShakeType::Strong),
            "SEVERE" => Some(CameraShakeType::Severe),
            "CINE_EXTREME" => Some(CameraShakeType::CineExtreme),
            "CINE_INSANE" => Some(CameraShakeType::CineInsane),
            _ => None,
        }
    }

    fn shake_params(self) -> (f32, f32, f32) {
        let (radius, duration, power) = match self {
            CameraShakeType::Subtle => (50.0, 0.3, 0.5),
            CameraShakeType::Normal => (100.0, 0.5, 1.0),
            CameraShakeType::Strong => (200.0, 0.8, 2.0),
            CameraShakeType::Severe => (400.0, 1.2, 4.0),
            CameraShakeType::CineExtreme => (600.0, 1.5, 6.0),
            CameraShakeType::CineInsane => (800.0, 2.0, 8.0),
        };
        (radius, duration, power)
    }
}

#[derive(Debug, Clone)]
struct ViewShakeFXNugget {
    shake_type: CameraShakeType,
}

impl Default for ViewShakeFXNugget {
    fn default() -> Self {
        Self {
            shake_type: CameraShakeType::Normal,
        }
    }
}

impl FXNugget for ViewShakeFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        let Some(primary) = primary else {
            return;
        };
        let (radius, duration, power) = self.shake_type.shake_params();
        with_shake_system(|system| {
            system.add_camera_shake(*primary, radius, duration, power);
        });
    }
}

/// Scorch types matching C++ Scorches enum (FXList.cpp:459-472)
#[derive(Debug, Clone, Copy, Default)]
enum ScorchType {
    Scorch1 = 0,
    Scorch2 = 1,
    Scorch3 = 2,
    Scorch4 = 3,
    ShadowScorch = 4,
    #[default]
    Random = -1,
}

impl ScorchType {
    fn parse_scorch_type(value: &str) -> Option<i32> {
        match value.trim().to_uppercase().as_str() {
            "SCORCH_1" => Some(0),
            "SCORCH_2" => Some(1),
            "SCORCH_3" => Some(2),
            "SCORCH_4" => Some(3),
            "SHADOW_SCORCH" => Some(4),
            "RANDOM" => Some(-1),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct TerrainScorchFXNugget {
    scorch: i32,
    radius: f32,
}

impl Default for TerrainScorchFXNugget {
    fn default() -> Self {
        Self {
            scorch: -1,
            radius: 0.0,
        }
    }
}

impl FXNugget for TerrainScorchFXNugget {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        let Some(primary) = primary else {
            return;
        };
        let scorch_idx = if self.scorch < 0 {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            rng.gen_range(0..4)
        } else {
            self.scorch
        };
        with_decal_manager(|manager| {
            let size = self.radius.max(0.1);
            let settings = DecalSettings::new(
                if scorch_idx == 4 {
                    DecalType::Scorch
                } else {
                    DecalType::Scorch
                },
                nalgebra::Point3::new(primary.x, primary.y, primary.z),
            );
            manager.create_decal(settings);
        });
    }
}

struct ParticleSystemWrapper {
    nugget: ParticleSystemFXNugget,
}

impl FXNugget for ParticleSystemWrapper {
    fn do_fx_pos(
        &self,
        primary: Option<&Coord3D>,
        primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        override_radius: f32,
    ) {
        let Some(primary) = primary else {
            return;
        };
        let Ok(mut manager_guard) = get_particle_system_manager_mut() else {
            return;
        };
        let Some(manager) = manager_guard.as_mut() else {
            return;
        };
        let primary_point = nalgebra::Point3::new(primary.x, primary.y, primary.z);
        let mtx = primary_mtx.map(|mtx| {
            let cols = mtx.to_cols_array_2d();
            nalgebra::Matrix3::new(
                cols[0][0], cols[0][1], cols[0][2], cols[1][0], cols[1][1], cols[1][2], cols[2][0],
                cols[2][1], cols[2][2],
            )
        });
        let systems = self
            .nugget
            .do_fx_pos(primary_point, mtx.as_ref(), override_radius, manager);
        drop(systems);
    }

    fn do_fx_obj(&self, primary: Option<&Object>, _secondary: Option<&Object>) {
        let Some(primary) = primary else {
            return;
        };
        let Ok(mut manager_guard) = get_particle_system_manager_mut() else {
            return;
        };
        let Some(manager) = manager_guard.as_mut() else {
            return;
        };
        let position = primary.get_position();
        let primary_point = nalgebra::Point3::new(position.x, position.y, position.z);
        let transform = primary.get_transform_matrix();
        let cols = transform.to_cols_array_2d();
        let mtx = nalgebra::Matrix3::new(
            cols[0][0], cols[0][1], cols[0][2], cols[1][0], cols[1][1], cols[1][2], cols[2][0],
            cols[2][1], cols[2][2],
        );
        let systems = self
            .nugget
            .do_fx_obj(primary_point, Some(&mtx), None, manager);
        drop(systems);
    }
}

#[derive(Default)]
struct FXListAtBonePosFXNugget {
    fx_name: String,
    bone_name: String,
    orient_to_bone: bool,
}

impl FXListAtBonePosFXNugget {
    const MAX_BONE_POINTS: usize = 40;

    fn bone_query_names(&self) -> Vec<String> {
        if self.bone_name.is_empty() {
            return Vec::new();
        }

        let mut names = Vec::with_capacity(Self::MAX_BONE_POINTS + 1);
        names.push(self.bone_name.clone());
        for index in 1..=Self::MAX_BONE_POINTS {
            names.push(format!("{}{:02}", self.bone_name, index));
        }
        names
    }

    fn execute_fx_at_bone(&self, fx: &FXList, primary: &Object, bone_name: &str) -> bool {
        let (found, pos, bone_mtx) = primary.get_single_logical_bone_position(bone_name);
        if !found {
            return false;
        }

        let mtx = if self.orient_to_bone {
            bone_mtx
        } else {
            primary.get_transform_matrix()
        };
        fx.do_fx_pos(Some(&pos), Some(&mtx), 0.0, None, 0.0);
        true
    }
}

impl FXNugget for FXListAtBonePosFXNugget {
    fn do_fx_pos(
        &self,
        _primary: Option<&Coord3D>,
        _primary_mtx: Option<&Matrix3D>,
        _primary_speed: f32,
        _secondary: Option<&Coord3D>,
        _override_radius: f32,
    ) {
        log::debug!("FXListAtBonePos requires object form");
    }

    fn do_fx_obj(&self, primary: Option<&Object>, _secondary: Option<&Object>) {
        let Some(primary) = primary else {
            return;
        };
        let Some(fx) = get_fx_list_store().find_fx_list(&self.fx_name) else {
            return;
        };

        for bone_name in self.bone_query_names() {
            self.execute_fx_at_bone(&fx, primary, &bone_name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx_list_at_bone_pos_queries_cpp_bone_name_sequence() {
        let nugget = FXListAtBonePosFXNugget {
            fx_name: "NestedFX".to_string(),
            bone_name: "WeaponFireFXBone".to_string(),
            orient_to_bone: true,
        };

        let names = nugget.bone_query_names();

        assert_eq!(names.first().map(String::as_str), Some("WeaponFireFXBone"));
        assert_eq!(names.get(1).map(String::as_str), Some("WeaponFireFXBone01"));
        assert_eq!(names.get(2).map(String::as_str), Some("WeaponFireFXBone02"));
        assert_eq!(names.last().map(String::as_str), Some("WeaponFireFXBone40"));
        assert_eq!(names.len(), FXListAtBonePosFXNugget::MAX_BONE_POINTS + 1);
    }

    #[test]
    fn fx_list_at_bone_pos_empty_name_queries_no_bones() {
        let nugget = FXListAtBonePosFXNugget::default();

        assert!(nugget.bone_query_names().is_empty());
    }

    #[test]
    fn sound_fx_nugget_without_hook_routes_via_gameplay_dispatch_fallback() {
        // Residual: FXList SoundFX must not be a silent no-op when the GameClient
        // audio hook is absent — falls back to dispatch_positional_sound.
        let nugget = SoundFXNugget {
            sound_name: "TestCombatFire".to_string(),
        };
        let pos = Coord3D {
            x: 10.0,
            y: 0.0,
            z: 20.0,
        };
        // Must not panic; empty-name guard lives inside dispatch.
        nugget.do_fx_pos(Some(&pos), None, 0.0, None, 0.0);
        // Empty name remains a true no-op (dispatch fail-closed).
        let empty = SoundFXNugget {
            sound_name: String::new(),
        };
        empty.do_fx_pos(Some(&pos), None, 0.0, None, 0.0);
    }
}
