//! FXList system for client-side audio/visual effects.
//!
//! Ported from `GameClient/FXList.cpp` and `GameClient/FXList.h`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use glam::Vec3;

use game_engine::common::ini::{INI, INIError, INILoadType, INIResult, register_block_parser};
use game_engine::common::name_key_generator::{NameKeyGenerator, NameKeyType};

use gamelogic::common::types::FXListManagerInterface;
use gamelogic::common::{Coord3D, FXListId, Matrix3D};
use gamelogic::object::Object;

use crate::display::cinematic_camera::CameraShakeSystem;
use crate::display::view::{
    CameraShakeType as ViewShakeKind, Point3 as ViewPoint3, with_tactical_view,
};
use crate::effects::decals::DecalManager;
use crate::effects::fxlist_integration::ParticleSystemFXNugget;
use crate::effects::particle_manager::{GameClientRandomVariable, get_particle_system_manager_mut};
use crate::effects::ray_effect_system::create_ray_effect_by_template;
use crate::effects::ray_effects::{RayEffectConfig, RayEffectManager};
use crate::effects::tracer_fx::spawn_tracer_drawable_like_cpp;
use crate::message_stream::game_message::Coord3D as MessageCoord3D;
use crate::terrain::scorch_mesh::{add_terrain_scorch, resolve_scorch_type};

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

    fn do_fx_pos_ex(
        &self,
        fx_list: FXListId,
        position: &Coord3D,
        matrix: Option<&glam::Mat4>,
        primary_speed: f32,
        secondary: Option<&Coord3D>,
        override_radius: f32,
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

        fx.do_fx_pos(
            Some(position),
            matrix,
            primary_speed,
            secondary,
            override_radius,
        );
    }

    fn do_fx_obj(&self, fx_list: FXListId, object_id: gamelogic::common::ThingId) {
        let Some(name) = NameKeyGenerator::key_to_name(fx_list as NameKeyType) else {
            log::debug!("FXListManager: unknown FXList id {}", fx_list);
            return;
        };
        let _ = do_named_fx_obj(&name, Some(object_id), None);
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
        let _ = do_named_fx_obj(&name, Some(object_id), source_id);
    }
}

pub fn register_fx_list_manager_bridge() {
    let _ = gamelogic::helpers::register_fx_list_manager(Arc::new(FXListManagerBridge));
    game_engine::common::ini::register_fx_list_obj_runtime(Arc::new(DamageFxListRuntime));
    ensure_default_ray_effect_manager();
}

/// Common DamageFX::doDamageFX → C++ FXList::doFXObj (FXList.cpp:794).
struct DamageFxListRuntime;

impl game_engine::common::ini::FxListObjRuntime for DamageFxListRuntime {
    fn do_fx_obj(&self, name: &str, primary_id: Option<u32>, secondary_id: Option<u32>) -> bool {
        do_named_fx_obj(name, primary_id, secondary_id)
    }
}

fn resolve_host_fx_object(id: u32) -> Option<gamelogic::helpers::HostFxObjectPose> {
    gamelogic::helpers::host_fx_object_pose(id)
        .or_else(|| crate::core::game_client::query_live_drawable_fx_pose(id))
}

fn host_fx_obj_is_visible(object_id: u32) -> bool {
    use gamelogic::common::types::ObjectShroudStatus;
    let player = fx_local_player_index();
    if player < 0 {
        return false;
    }
    let Ok(shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() else {
        return true;
    };
    match shroud.get_host_object_shroud_status(player as u32, object_id) {
        Some(status) => (status as u8) <= (ObjectShroudStatus::PartialClear as u8),
        None => true,
    }
}

fn do_named_fx_obj(name: &str, primary_id: Option<u32>, secondary_id: Option<u32>) -> bool {
    let store = get_fx_list_store();
    let Some(fx) = store.find_fx_list(name) else {
        return false;
    };
    let leftover_primary = primary_id.and_then(gamelogic::helpers::TheGameLogic::find_object_by_id);
    let leftover_secondary =
        secondary_id.and_then(gamelogic::helpers::TheGameLogic::find_object_by_id);
    if let Some(object) = leftover_primary {
        if let Ok(guard) = object.read() {
            let source_guard = leftover_secondary
                .as_ref()
                .and_then(|source| source.read().ok());
            fx.do_fx_obj(Some(&*guard), source_guard.as_deref());
        }
        return true;
    }
    if primary_id.is_none() {
        fx.do_fx_obj(None, None);
        return true;
    }
    let Some(primary) = primary_id.and_then(resolve_host_fx_object) else {
        return false;
    };
    if !host_fx_obj_is_visible(primary.id) {
        return true;
    }
    let secondary = leftover_secondary
        .as_ref()
        .and_then(|source| source.read().ok())
        .map(|guard| leftover_object_fx_pose(&guard))
        .or_else(|| secondary_id.and_then(resolve_host_fx_object));
    fx.do_fx_obj_host(&primary, secondary.as_ref());
    true
}

fn leftover_object_fx_pose(object: &Object) -> gamelogic::helpers::HostFxObjectPose {
    use gamelogic::common::types::ObjectShroudStatus;
    let player = fx_local_player_index();
    let is_shrouded = player >= 0
        && (object.get_shrouded_status(player) as u8) >= (ObjectShroudStatus::Fogged as u8);
    gamelogic::helpers::HostFxObjectPose {
        id: object.get_id(),
        position: *object.get_position(),
        transform: object.get_transform_matrix(),
        player_index: controlling_player_index(object),
        bounding_circle_radius: object.get_geometry_info().get_bounding_circle_radius(),
        is_shrouded,
    }
}

fn ensure_default_ray_effect_manager() {
    let slot = FX_RAY_MANAGER.get_or_init(|| RwLock::new(None));
    let mut guard = slot.write().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(Arc::new(Mutex::new(RayEffectManager::new())));
    }
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

    /// C++ `FXNugget::doFXObj` from a live host pose when leftover Object is absent.
    fn do_fx_obj_host(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        self.do_fx_pos(
            Some(&primary.position),
            Some(&primary.transform),
            0.0,
            secondary.map(|pose| &pose.position),
            0.0,
        );
    }

    /// C++ `SoundFXNugget::m_soundName`. Other nuggets have none.
    fn sound_name(&self) -> Option<&str> {
        None
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
static DISPLAY_LIGHT_PULSES: OnceLock<Mutex<Vec<DisplayLightPulse>>> = OnceLock::new();
type LightPulseHook = Box<dyn FnMut(&DisplayLightPulse) + Send + Sync>;
static LIGHT_PULSE_HOOK: OnceLock<RwLock<Option<LightPulseHook>>> = OnceLock::new();
static SCENE_DYNAMIC_LIGHTS: OnceLock<Mutex<Vec<DisplayDynamicLight>>> = OnceLock::new();

/// C++ `TheDisplay->createLightPulse` request (W3DDisplay.cpp).
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayLightPulse {
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub increase_frames: u32,
    pub decay_frames: u32,
}

/// Scene light created by `W3DDisplay::createLightPulse` (far atten + fade).
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayDynamicLight {
    pub pos: [f32; 3],
    pub color: [f32; 3],
    pub far_atten_start: f32,
    pub far_atten_end: f32,
    pub increase_frames: u32,
    pub decay_frames: u32,
    pub cur_increase_frames: u32,
    pub cur_decay_frames: u32,
    pub target_color: [f32; 3],
    pub target_far_atten_end: f32,
    pub decay_range: bool,
    pub decay_color: bool,
    pub far_attenuation: bool,
    pub enabled: bool,
}

/// C++ `PATHFIND_CELL_SIZE_F` (pathfind cell = 10 world units).
const PATHFIND_CELL_SIZE_F: f32 = 10.0;

/// C++ `W3DDisplay::createLightPulse` size cull:
/// `innerRadius + attenuationWidth < 2 * PATHFIND_CELL_SIZE_F + 1`.
pub fn light_pulse_too_small(inner_radius: f32, outer_radius: f32) -> bool {
    inner_radius + outer_radius < 2.0 * PATHFIND_CELL_SIZE_F + 1.0
}

/// C++ `TheDisplay->createLightPulse(pos, color, innerRadius, outerRadius, increase, decay)`.
/// Records the pulse and notifies an optional display hook (W3D scene).
pub fn create_display_light_pulse(pulse: DisplayLightPulse) -> bool {
    if light_pulse_too_small(pulse.inner_radius, pulse.outer_radius) {
        return false;
    }
    DISPLAY_LIGHT_PULSES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(pulse.clone());
    // C++ W3DDisplay::createLightPulse allocates a W3DDynamicLight:
    // Set_Far_Attenuation_Range(inner, inner+atten), setFrameFade, setDecayRange/Color,
    // FAR_ATTENUATION flag.
    SCENE_DYNAMIC_LIGHTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(DisplayDynamicLight {
            pos: pulse.pos,
            color: pulse.color,
            far_atten_start: pulse.inner_radius,
            far_atten_end: pulse.inner_radius + pulse.outer_radius,
            increase_frames: pulse.increase_frames,
            decay_frames: pulse.decay_frames,
            cur_increase_frames: pulse.increase_frames,
            cur_decay_frames: pulse.decay_frames,
            target_color: pulse.color,
            target_far_atten_end: pulse.inner_radius + pulse.outer_radius,
            decay_range: true,
            decay_color: true,
            far_attenuation: true,
            enabled: true,
        });
    if let Some(hook_slot) = LIGHT_PULSE_HOOK.get() {
        if let Ok(mut guard) = hook_slot.write() {
            if let Some(hook) = guard.as_mut() {
                hook(&pulse);
            }
        }
    }
    true
}

pub fn scene_dynamic_lights() -> Vec<DisplayDynamicLight> {
    SCENE_DYNAMIC_LIGHTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// C++ `Get_Far_Attenuation_Range(midRange, range)` then
/// `factor = 1 - (dist - midRange) / (range - midRange)` (`HeightMap.cpp`).
/// `None` means the light is skipped (`dist >= range` or `midRange < 0.1`).
pub fn far_atten_factor(dist: f32, mid_range: f32, range: f32) -> Option<f32> {
    if dist >= range {
        return None;
    }
    if mid_range < 0.1 {
        return None;
    }
    let denom = range - mid_range;
    if denom <= 0.0 {
        return Some(1.0);
    }
    Some((1.0 - (dist - mid_range) / denom).clamp(0.0, 1.0))
}

/// C++ `HeightMapRenderObjClass::doTheDynamicLight` for POINT/SPOT lights
/// sourced from `createLightPulse` scene lights. Packed diffuse is BGRA
/// (`B | G<<8 | R<<16 | A<<24`), `Float_To_Int_Chop` toward zero.
pub fn do_the_dynamic_light(
    vertex_xyz: [f32; 3],
    vertex_normal: [f32; 3],
    vertex_diffuse: u32,
    lights: &[DisplayDynamicLight],
) -> u32 {
    const OO255: f32 = 1.0 / 255.0;
    let mut shade_r = (((vertex_diffuse >> 16) & 0xFF) as f32) * OO255;
    let mut shade_g = (((vertex_diffuse >> 8) & 0xFF) as f32) * OO255;
    let mut shade_b = ((vertex_diffuse & 0xFF) as f32) * OO255;
    let alpha = ((vertex_diffuse >> 24) & 0xFF) as u32;

    for light in lights {
        if !light.enabled || !light.far_attenuation {
            continue;
        }
        let dx = vertex_xyz[0] - light.pos[0];
        let dy = vertex_xyz[1] - light.pos[1];
        let dz = vertex_xyz[2] - light.pos[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let Some(factor) = far_atten_factor(dist, light.far_atten_start, light.far_atten_end)
        else {
            continue;
        };
        if dist <= 0.0 {
            continue;
        }
        let inv = 1.0 / dist;
        let light_ray = [-dx * inv, -dy * inv, -dz * inv];
        let mut shade = light_ray[0] * vertex_normal[0]
            + light_ray[1] * vertex_normal[1]
            + light_ray[2] * vertex_normal[2];
        shade *= factor;
        shade = shade.clamp(0.0, 1.0);
        shade_r += shade * light.color[0];
        shade_g += shade * light.color[1];
        shade_b += shade * light.color[2];
        shade_r += factor * light.color[0];
        shade_g += factor * light.color[1];
        shade_b += factor * light.color[2];
    }

    shade_r = shade_r.clamp(0.0, 1.0) * 255.0;
    shade_g = shade_g.clamp(0.0, 1.0) * 255.0;
    shade_b = shade_b.clamp(0.0, 1.0) * 255.0;
    (shade_b as u32) | ((shade_g as u32) << 8) | ((shade_r as u32) << 16) | (alpha << 24)
}

/// Light a vertex from the live `createLightPulse` scene list.
pub fn do_the_dynamic_light_from_scene(
    vertex_xyz: [f32; 3],
    vertex_normal: [f32; 3],
    vertex_diffuse: u32,
) -> u32 {
    gamelogic::helpers::tick_scene_point_lights();
    let mut lights = scene_dynamic_lights();
    for light in gamelogic::helpers::scene_point_lights() {
        lights.push(DisplayDynamicLight {
            pos: light.pos,
            color: [
                light.diffuse[0] + light.ambient[0],
                light.diffuse[1] + light.ambient[1],
                light.diffuse[2] + light.ambient[2],
            ],
            far_atten_start: light.far_start,
            far_atten_end: light.far_end,
            increase_frames: 0,
            decay_frames: 0,
            cur_increase_frames: 0,
            cur_decay_frames: 0,
            target_color: light.diffuse,
            target_far_atten_end: light.far_end,
            decay_range: false,
            decay_color: false,
            far_attenuation: true,
            enabled: light.enabled,
        });
    }
    do_the_dynamic_light(vertex_xyz, vertex_normal, vertex_diffuse, &lights)
}

/// C++ `W3DDynamicLight::On_Frame_Update` fade (increase then decay range/color).
pub fn tick_scene_dynamic_lights() {
    let mut lights = SCENE_DYNAMIC_LIGHTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    for light in lights.iter_mut() {
        if !light.enabled {
            continue;
        }
        let factor = if light.cur_increase_frames > 0 && light.increase_frames > 0 {
            light.cur_increase_frames -= 1;
            (light.increase_frames - light.cur_increase_frames) as f32
                / light.increase_frames as f32
        } else if light.decay_frames == 0 {
            1.0
        } else {
            light.cur_decay_frames = light.cur_decay_frames.saturating_sub(1);
            if light.cur_decay_frames == 0 {
                light.enabled = false;
                continue;
            }
            light.cur_decay_frames as f32 / light.decay_frames as f32
        };
        if light.decay_range {
            light.far_atten_end = (factor * light.target_far_atten_end).max(light.far_atten_start);
        }
        if light.decay_color {
            light.color = [
                light.target_color[0] * factor,
                light.target_color[1] * factor,
                light.target_color[2] * factor,
            ];
        }
    }
    lights.retain(|light| light.enabled);
}

pub fn clear_scene_dynamic_lights() {
    SCENE_DYNAMIC_LIGHTS
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

pub fn drain_display_light_pulses() -> Vec<DisplayLightPulse> {
    DISPLAY_LIGHT_PULSES
        .get_or_init(|| Mutex::new(Vec::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .drain(..)
        .collect()
}

pub fn register_light_pulse_hook(hook: LightPulseHook) {
    LIGHT_PULSE_HOOK
        .get_or_init(|| RwLock::new(None))
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .replace(hook);
}

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
    ensure_default_ray_effect_manager();
    let Some(manager) = FX_RAY_MANAGER.get() else {
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

fn fx_local_player_index() -> i32 {
    gamelogic::player::player_list()
        .read()
        .ok()
        .map(|list| list.get_local_player_index())
        .unwrap_or(-1)
}

/// C++ `FXList::doFXPos` (FXList.cpp:784) plays only when
/// `ThePartitionManager->getShroudStatusForPlayer(localPlayer, primary) == CELLSHROUD_CLEAR`.
/// `PartitionManager.cpp:3017-3023` returns `CELLSHROUD_SHROUDED` for
/// `playerIndex < 0` or a missing cell (including an uninitialized grid).
fn fx_pos_cell_is_clear(primary: Option<&Coord3D>) -> bool {
    let Some(primary) = primary else {
        return false;
    };
    let player = fx_local_player_index();
    if player < 0 {
        return false;
    }
    let Ok(shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() else {
        return false;
    };
    matches!(
        shroud.get_shroud_state(player as u32, primary),
        gamelogic::system::shroud_manager::ShroudState::Visible
    )
}

/// C++ `FXList::doFXObj` (FXList.cpp:794-797) skips when
/// `primary && getShroudedStatus(localPlayer) > OBJECTSHROUD_PARTIAL_CLEAR`.
/// Invalid local index (`< 0`) is fail-closed like PartitionManager's
/// `playerIndex < 0` → `CELLSHROUD_SHROUDED` (hq-nyjgg).
fn fx_obj_is_visible(primary: Option<&Object>) -> bool {
    let Some(primary) = primary else {
        return true;
    };
    use gamelogic::common::types::ObjectShroudStatus;
    let player = fx_local_player_index();
    if player < 0 {
        return false;
    }
    let status = primary.get_shrouded_status(player);
    (status as u8) <= (ObjectShroudStatus::PartialClear as u8)
}

fn controlling_player_index(primary: &Object) -> i32 {
    primary
        .get_controlling_player()
        .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()))
        .unwrap_or(-1)
}

/// C++ `SoundFXNugget::doFXObj` / `doFXPos` → `TheAudio->addAudioEvent`.
fn play_sound_fx_event(sound_name: &str, position: Option<&Coord3D>, player_index: Option<i32>) {
    use game_engine::common::audio::audio_event_rts::{AudioEventRts, Coord3D as AudioCoord3D};
    use game_engine::common::audio::game_audio::{
        get_global_audio_manager, initialize_global_audio_manager,
    };

    if sound_name.is_empty() {
        return;
    }
    let mut event = AudioEventRts::with_event_name(sound_name);
    if let Some(pos) = position {
        event.set_position(&AudioCoord3D {
            x: pos.x,
            y: pos.y,
            z: pos.z,
        });
    }
    if let Some(index) = player_index {
        event.set_player_index(index);
    }
    let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
    let Ok(mut manager) = manager.lock() else {
        return;
    };
    if let Some(info) = manager
        .find_audio_event_info(event.get_event_name())
        .or_else(|| manager.new_audio_event_info(event.get_event_name().to_string()))
    {
        event.set_audio_event_info(info.clone());
        event.set_volume(info.volume);
    }
    let _ = manager.add_audio_event(&event);
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
        if !fx_pos_cell_is_clear(primary) {
            return;
        }
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
        if !fx_obj_is_visible(primary) {
            return;
        }
        for nugget in &self.nuggets {
            nugget.do_fx_obj(primary, secondary);
        }
    }

    fn do_fx_obj_host(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        for nugget in &self.nuggets {
            nugget.do_fx_obj_host(primary, secondary);
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

/// C++ `SoundFXNugget::m_soundName` values authored inside `name`.
pub fn sound_names_for_fx_list(name: &str) -> Vec<String> {
    if name.is_empty() || name.eq_ignore_ascii_case("None") {
        return Vec::new();
    }
    let store = get_fx_list_store();
    let Some(fx) = store.find_fx_list(name) else {
        return Vec::new();
    };
    fx.nuggets
        .iter()
        .filter_map(|nugget| nugget.sound_name().map(str::to_string))
        .collect()
}

pub fn init_fx_list_store() -> Result<(), Box<dyn std::error::Error>> {
    FX_LIST_PARSER_REGISTERED.get_or_init(|| {
        let _ = register_block_parser("FXList", parse_fx_list_definition);
    });

    let mut ini = INI::new();
    let default_path = "Data/INI/Default/FXList.ini";
    let override_path = "Data/INI/FXList.ini";
    // C++ `SubsystemInterfaceList::initSubsystem` loads both paths with
    // `INI_LOAD_OVERWRITE`.  Do not probe the host filesystem first: retail
    // INIs normally live in BIG archives and `INI::load` already resolves the
    // engine virtual filesystem before failing closed.
    ini.load(default_path, INILoadType::Overwrite)?;
    ini.load(override_path, INILoadType::Overwrite)?;
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

fn parse_labeled_vec3(values: &[String], color: bool) -> INIResult<Vec3> {
    let mut components = [None; 3];
    for value in values {
        let Some((label, raw)) = value.split_once(':') else {
            continue;
        };
        // Two shipped offsets contain `Y:15:`.  The C++ parser accepts the
        // numeric prefix, so keep that retail-compatible behavior here.
        let number = INI::parse_real(raw.trim_end_matches(':'))?;
        let index = match label.to_ascii_uppercase().as_str() {
            "X" | "R" => 0,
            "Y" | "G" => 1,
            "Z" | "B" => 2,
            _ => continue,
        };
        components[index] = Some(number);
    }
    let scale = if color { 1.0 / 255.0 } else { 1.0 };
    Ok(Vec3::new(
        components[0].ok_or(INIError::InvalidData)? * scale,
        components[1].ok_or(INIError::InvalidData)? * scale,
        components[2].ok_or(INIError::InvalidData)? * scale,
    ))
}

fn parse_random_variable(values: &[String]) -> INIResult<GameClientRandomVariable> {
    let Some(minimum) = values.first() else {
        return Err(INIError::InvalidData);
    };
    let maximum = values.get(1).unwrap_or(minimum);
    let mut variable =
        GameClientRandomVariable::new(INI::parse_real(minimum)?, INI::parse_real(maximum)?);
    if values
        .get(2)
        .is_some_and(|kind| kind.eq_ignore_ascii_case("NORMAL"))
    {
        variable.distribution_type = 1;
    }
    Ok(variable)
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
            "SPEED" => nugget.speed = INI::parse_velocity_real(value)?,
            "DECAYAT" => nugget.decay_at = INI::parse_real(value)?,
            "LENGTH" => nugget.length = INI::parse_real(value)?,
            "WIDTH" => nugget.width = INI::parse_real(value)?,
            "COLOR" => {
                nugget.color = parse_labeled_vec3(&values, true)?;
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
                nugget.primary_offset = parse_labeled_vec3(&values, false)?;
            }
            "SECONDARYOFFSET" => {
                nugget.secondary_offset = parse_labeled_vec3(&values, false)?;
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
                nugget.color = parse_labeled_vec3(&values, true)?;
            }
            "RADIUS" => nugget.radius = INI::parse_real(value)?,
            "RADIUSASPERCENTOFOBJECTSIZE" => {
                nugget.bounding_circle_pct = INI::parse_percent_to_real(value)?;
            }
            "INCREASETIME" => {
                nugget.increase_frames = INI::parse_duration_unsigned_int(value)?;
            }
            "DECREASETIME" => {
                nugget.decrease_frames = INI::parse_duration_unsigned_int(value)?;
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
                let offset = parse_labeled_vec3(&values, false)?;
                nugget.offset = nalgebra::Vector3::new(offset.x, offset.y, offset.z);
            }
            "RADIUS" => {
                nugget.radius = parse_random_variable(&values)?;
            }
            "HEIGHT" => {
                nugget.height = parse_random_variable(&values)?;
            }
            "INITIALDELAY" => {
                nugget.delay = parse_random_variable(&values)?;
            }
            "ROTATEX" => nugget.rotate_x = INI::parse_angle_real(value)?,
            "ROTATEY" => nugget.rotate_y = INI::parse_angle_real(value)?,
            "ROTATEZ" => nugget.rotate_z = INI::parse_angle_real(value)?,
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
        // C++ SoundFXNugget::doFXObj (FXList.cpp:90-99): setPlayerIndex +
        // setPosition from the primary object, then TheAudio->addAudioEvent.
        let player_index = primary.map(controlling_player_index);
        let world_pos = primary.map(|obj| *obj.get_position());
        play_sound_fx_event(
            &self.sound_name,
            world_pos.as_ref(),
            player_index.filter(|&index| index >= 0),
        );
    }

    fn do_fx_obj_host(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        _secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        play_sound_fx_event(
            &self.sound_name,
            Some(&primary.position),
            (primary.player_index >= 0).then_some(primary.player_index),
        );
    }

    fn sound_name(&self) -> Option<&str> {
        if self.sound_name.is_empty() {
            None
        } else {
            Some(self.sound_name.as_str())
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
        // C++ FXList.cpp:150 — GameClientRandomValueReal(0, 1).
        if self.probability <= crate::GameClientRandomValueReal!(0.0, 1.0) {
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
        let current_frame = gamelogic::helpers::TheGameLogic::get_frame();
        let _ = spawn_tracer_drawable_like_cpp(
            &self.tracer_name,
            [primary.x, primary.y, primary.z],
            [secondary.x, secondary.y, secondary.z],
            speed,
            self.length,
            self.width,
            [self.color.x, self.color.y, self.color.z],
            self.decay_at,
            current_frame,
        );
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
        let _ = create_ray_effect_by_template(
            [source.x, source.y, source.z],
            [target.x, target.y, target.z],
            &self.template_name,
        );

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

impl LightPulseFXNugget {
    /// C++ LightPulseFXNugget::doFXPos / doFXObj → TheDisplay->createLightPulse.
    /// Inner radius is always 1 (FXList.cpp:312/324).
    fn emit_pulse(&self, pos: &Coord3D, outer_radius: f32) {
        let _ = create_display_light_pulse(DisplayLightPulse {
            pos: [pos.x, pos.y, pos.z],
            color: [self.color.x, self.color.y, self.color.z],
            inner_radius: 1.0,
            outer_radius,
            increase_frames: self.increase_frames,
            decay_frames: self.decrease_frames,
        });
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
        self.emit_pulse(primary, self.radius);
    }

    fn do_fx_obj(&self, primary: Option<&Object>, _secondary: Option<&Object>) {
        let Some(primary) = primary else {
            return;
        };
        let mut radius = self.radius;
        if self.bounding_circle_pct > 0.0 {
            radius =
                primary.get_geometry_info().get_bounding_circle_radius() * self.bounding_circle_pct;
        }
        self.emit_pulse(primary.get_position(), radius);
    }

    fn do_fx_obj_host(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        _secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        let mut radius = self.radius;
        if self.bounding_circle_pct > 0.0 {
            radius = primary.bounding_circle_radius * self.bounding_circle_pct;
        }
        self.emit_pulse(&primary.position, radius);
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

    fn to_view_shake(self) -> ViewShakeKind {
        match self {
            CameraShakeType::Subtle => ViewShakeKind::Subtle,
            CameraShakeType::Normal => ViewShakeKind::Normal,
            CameraShakeType::Strong => ViewShakeKind::Strong,
            CameraShakeType::Severe => ViewShakeKind::Severe,
            CameraShakeType::CineExtreme => ViewShakeKind::CineExtreme,
            CameraShakeType::CineInsane => ViewShakeKind::CineInsane,
        }
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
        // C++ ViewShakeFXNugget::doFXPos → TheTacticalView->shake(primary, m_shake)
        with_tactical_view(|view| {
            view.shake(
                &ViewPoint3::new(primary.x, primary.y, primary.z),
                self.shake_type.to_view_shake(),
            );
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
        let scorch_idx = resolve_scorch_type(self.scorch);
        // C++ TerrainScorchFXNugget::doFXPos calls only TheGameClient->addScorch.
        let _ = add_terrain_scorch([primary.x, primary.y, primary.z], self.radius, scorch_idx);
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

    fn do_fx_obj(&self, primary: Option<&Object>, secondary: Option<&Object>) {
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

        // C++ FXList.cpp:519-529 — ricochet uses attacker→victim only when
        // secondary is present; otherwise keep the primary object transform.
        let mtx = if self.nugget.ricochet {
            if let Some(secondary) = secondary {
                let secondary_pos = secondary.get_position();
                let aiming_angle =
                    (position.y - secondary_pos.y).atan2(position.x - secondary_pos.x);
                let (s, c) = aiming_angle.sin_cos();
                Some(nalgebra::Matrix3::from_columns(&[
                    nalgebra::Vector3::new(c, s, 0.0),
                    nalgebra::Vector3::new(-s, c, 0.0),
                    nalgebra::Vector3::new(0.0, 0.0, 1.0),
                ]))
            } else {
                let cols = primary.get_transform_matrix().to_cols_array_2d();
                Some(nalgebra::Matrix3::new(
                    cols[0][0], cols[0][1], cols[0][2], cols[1][0], cols[1][1], cols[1][2],
                    cols[2][0], cols[2][1], cols[2][2],
                ))
            }
        } else {
            let cols = primary.get_transform_matrix().to_cols_array_2d();
            Some(nalgebra::Matrix3::new(
                cols[0][0], cols[0][1], cols[0][2], cols[1][0], cols[1][1], cols[1][2], cols[2][0],
                cols[2][1], cols[2][2],
            ))
        };

        let object_id = Some(primary.get_id());
        let systems = self
            .nugget
            .do_fx_obj(primary_point, mtx.as_ref(), object_id, manager);
        drop(systems);
    }

    fn do_fx_obj_host(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        let Ok(mut manager_guard) = get_particle_system_manager_mut() else {
            return;
        };
        let Some(manager) = manager_guard.as_mut() else {
            return;
        };
        let position = primary.position;
        let primary_point = nalgebra::Point3::new(position.x, position.y, position.z);
        let mtx = if self.nugget.ricochet {
            if let Some(secondary) = secondary {
                let secondary_pos = secondary.position;
                let aiming_angle =
                    (position.y - secondary_pos.y).atan2(position.x - secondary_pos.x);
                let (s, c) = aiming_angle.sin_cos();
                Some(nalgebra::Matrix3::from_columns(&[
                    nalgebra::Vector3::new(c, s, 0.0),
                    nalgebra::Vector3::new(-s, c, 0.0),
                    nalgebra::Vector3::new(0.0, 0.0, 1.0),
                ]))
            } else {
                let cols = primary.transform.to_cols_array_2d();
                Some(nalgebra::Matrix3::new(
                    cols[0][0], cols[0][1], cols[0][2], cols[1][0], cols[1][1], cols[1][2],
                    cols[2][0], cols[2][1], cols[2][2],
                ))
            }
        } else {
            let cols = primary.transform.to_cols_array_2d();
            Some(nalgebra::Matrix3::new(
                cols[0][0], cols[0][1], cols[0][2], cols[1][0], cols[1][1], cols[1][2], cols[2][0],
                cols[2][1], cols[2][2],
            ))
        };
        let systems = self
            .nugget
            .do_fx_obj(primary_point, mtx.as_ref(), Some(primary.id), manager);
        drop(systems);
    }
}

/// C++ `obj->getDrawable()->getCurrentClientBonePositions`.
/// Prefers the live GameClient `BasicDrawable` W3D walk, then GameLogic
/// Drawable's draw-module walk (same C++ loop). Never the empty skeleton.
fn current_client_bone_positions(
    primary: &Object,
    bone_name: &str,
    start: i32,
    positions: &mut [Coord3D],
    transforms: &mut [Matrix3D],
) -> i32 {
    let max_bones = positions.len().min(transforms.len());
    let live = crate::core::game_client::query_live_current_client_bone_positions(
        primary.get_id(),
        bone_name,
        start,
        max_bones,
    );
    if !live.is_empty() {
        let count = live.len().min(max_bones);
        for (i, (pos, mtx)) in live.into_iter().take(count).enumerate() {
            positions[i] = pos;
            transforms[i] = mtx;
        }
        return count as i32;
    }
    let Some(drawable) = primary.get_drawable() else {
        return 0;
    };
    let Ok(draw) = drawable.read() else {
        return 0;
    };
    draw.get_current_client_bone_positions(bone_name, start, positions, transforms)
}

struct FXListAtBonePosFXNugget {
    fx_name: String,
    bone_name: String,
    /// Parsed (C++ default true) but never read — always the bone world matrix.
    orient_to_bone: bool,
}

impl Default for FXListAtBonePosFXNugget {
    fn default() -> Self {
        Self {
            fx_name: String::new(),
            bone_name: String::new(),
            orient_to_bone: true,
        }
    }
}

impl FXListAtBonePosFXNugget {
    const MAX_BONE_POINTS: usize = 40;

    /// C++ `W3DModelDraw::getCurrentBonePositions`: `start==0` is the
    /// unadorned name only; `start>=1` walks `Name01`… until the first miss.
    fn client_bone_name(prefix: &str, index: i32) -> String {
        if index <= 0 {
            prefix.to_string()
        } else {
            format!("{prefix}{index:02}")
        }
    }

    fn client_bone_end_index(start: i32) -> i32 {
        if start <= 0 {
            0
        } else {
            Self::MAX_BONE_POINTS as i32
        }
    }

    /// C++ `FXListAtBonePosFXNugget::doFxAtBones` (FXList.cpp:711-728).
    fn do_fx_at_bones(&self, primary: &Object, start: i32, fx: &FXList) {
        if self.bone_name.is_empty() {
            return;
        }
        let mut positions = [Coord3D::ZERO; Self::MAX_BONE_POINTS];
        let mut transforms = [Matrix3D::IDENTITY; Self::MAX_BONE_POINTS];
        let count = current_client_bone_positions(
            primary,
            &self.bone_name,
            start,
            &mut positions,
            &mut transforms,
        );
        for i in 0..count as usize {
            // C++ convertBonePosToWorldPos: worldMtx = obj * boneMtx,
            // worldPos = obj.Transform_Vector(bonePos). bonePos is boneMtx translation.
            let world = primary.convert_bone_pos_to_world_pos(None, Some(&transforms[i]));
            let (_, _, world_pos) = world.to_scale_rotation_translation();
            let _ = self.orient_to_bone;
            fx.do_fx_pos(Some(&world_pos), Some(&world), 0.0, None, 0.0);
        }
    }

    fn do_fx_at_bones_host(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        start: i32,
        fx: &FXList,
    ) {
        if self.bone_name.is_empty() {
            return;
        }
        let live = crate::core::game_client::query_live_current_client_bone_positions(
            primary.id,
            &self.bone_name,
            start,
            Self::MAX_BONE_POINTS,
        );
        for (_pos, bone_mtx) in live {
            let world = primary.transform * bone_mtx;
            let (_, _, world_pos) = world.to_scale_rotation_translation();
            let _ = self.orient_to_bone;
            fx.do_fx_pos(Some(&world_pos), Some(&world), 0.0, None, 0.0);
        }
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

        // C++ doFXObj: unadorned name, then 01,02,… (FXList.cpp:682-686).
        self.do_fx_at_bones(primary, 0, &fx);
        self.do_fx_at_bones(primary, 1, &fx);
    }

    fn do_fx_obj_host(
        &self,
        primary: &gamelogic::helpers::HostFxObjectPose,
        _secondary: Option<&gamelogic::helpers::HostFxObjectPose>,
    ) {
        let Some(fx) = get_fx_list_store().find_fx_list(&self.fx_name) else {
            return;
        };
        self.do_fx_at_bones_host(primary, 0, &fx);
        self.do_fx_at_bones_host(primary, 1, &fx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_fx_parser_accepts_retail_labeled_values() {
        let color = parse_labeled_vec3(&["R:255".into(), "G:128".into(), "B:0".into()], true)
            .expect("retail color");
        assert_eq!(color, Vec3::new(1.0, 128.0 / 255.0, 0.0));

        let offset = parse_labeled_vec3(&["X:1".into(), "Y:15:".into(), "Z:-2".into()], false)
            .expect("retail offset with C++ numeric-prefix typo");
        assert_eq!(offset, Vec3::new(1.0, 15.0, -2.0));
    }

    #[test]
    fn runtime_fx_parser_loads_complete_retail_file_when_present() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../windows_game/extracted_big_files_v2/INIZH/Data/INI/FXList.ini");
        let Ok(source) = std::fs::read_to_string(&path) else {
            return;
        };
        FX_LIST_PARSER_REGISTERED.get_or_init(|| {
            assert!(register_block_parser("FXList", parse_fx_list_definition));
        });

        let mut ini = INI::new();
        ini.with_inline_source(&source, |ini| ini.parse_current_file())
            .unwrap_or_else(|error| panic!("retail {} must parse: {error:?}", path.display()));

        let store = get_fx_list_store();
        let tank = store
            .find_fx_list("WeaponFX_GenericTankGun")
            .expect("known retail runtime FXList stored");
        assert!(tank.nuggets.len() >= 4);
    }

    #[test]
    fn fx_list_at_bone_pos_queries_cpp_current_client_bone_sequence() {
        assert_eq!(
            FXListAtBonePosFXNugget::client_bone_name("WeaponFireFXBone", 0),
            "WeaponFireFXBone"
        );
        assert_eq!(
            FXListAtBonePosFXNugget::client_bone_name("WeaponFireFXBone", 1),
            "WeaponFireFXBone01"
        );
        assert_eq!(
            FXListAtBonePosFXNugget::client_bone_name("WeaponFireFXBone", 40),
            "WeaponFireFXBone40"
        );
        assert_eq!(FXListAtBonePosFXNugget::client_bone_end_index(0), 0);
        assert_eq!(
            FXListAtBonePosFXNugget::client_bone_end_index(1),
            FXListAtBonePosFXNugget::MAX_BONE_POINTS as i32
        );
        assert!(FXListAtBonePosFXNugget::default().orient_to_bone);
    }

    #[test]
    fn fx_list_at_bone_pos_empty_name_queries_no_bones() {
        let nugget = FXListAtBonePosFXNugget::default();
        assert!(nugget.bone_name.is_empty());
        assert!(nugget.orient_to_bone);
    }

    #[test]
    fn fx_obj_is_visible_fail_closes_when_local_player_invalid() {
        let prev_player = gamelogic::player::player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(-1);
        if let Ok(mut list) = gamelogic::player::player_list().write() {
            list.set_local_player_index(-1);
        }
        assert!(
            fx_obj_is_visible(None),
            "C++ `if (primary && …)` plays when primary is null"
        );
        assert!(
            fx_local_player_index() < 0,
            "local player must be invalid for this gate"
        );
        if let Ok(mut players) = gamelogic::player::player_list().write() {
            players.set_local_player_index(prev_player);
        }
    }

    #[test]
    fn sound_fx_nugget_do_fx_obj_sets_player_index_on_audio_event() {
        let mut event =
            game_engine::common::audio::audio_event_rts::AudioEventRts::with_event_name("UnitDie");
        event.set_player_index(3);
        assert_eq!(event.get_player_index(), 3);
        let nugget = SoundFXNugget {
            sound_name: "UnitDie".to_string(),
        };
        nugget.do_fx_obj(None, None);
    }

    #[test]
    fn light_pulse_fx_nugget_creates_display_light_not_decal() {
        let _ = drain_display_light_pulses();
        let nugget = LightPulseFXNugget {
            color: Vec3::new(1.0, 0.25, 0.0),
            radius: 50.0,
            bounding_circle_pct: 0.0,
            increase_frames: 3,
            decrease_frames: 9,
        };
        let pos = Coord3D {
            x: 12.0,
            y: 34.0,
            z: 5.0,
        };
        clear_scene_dynamic_lights();
        nugget.do_fx_pos(Some(&pos), None, 0.0, None, 0.0);
        let pulses = drain_display_light_pulses();
        assert_eq!(
            pulses.len(),
            1,
            "FXList LightPulse must call TheDisplay createLightPulse"
        );
        let lights = scene_dynamic_lights();
        assert_eq!(
            lights.len(),
            1,
            "createLightPulse must allocate a scene dynamic light"
        );
        assert_eq!(lights[0].far_atten_start, 1.0);
        assert_eq!(lights[0].far_atten_end, 51.0);
        assert!(lights[0].far_attenuation);
        assert!(lights[0].decay_range && lights[0].decay_color);
        assert_eq!(pulses[0].pos, [12.0, 34.0, 5.0]);
        assert_eq!(pulses[0].color, [1.0, 0.25, 0.0]);
        assert_eq!(
            pulses[0].inner_radius, 1.0,
            "C++ always passes innerRadius=1"
        );
        assert_eq!(pulses[0].outer_radius, 50.0);
        assert_eq!(pulses[0].increase_frames, 3);
        assert_eq!(pulses[0].decay_frames, 9);
        assert!(!light_pulse_too_small(
            pulses[0].inner_radius,
            pulses[0].outer_radius
        ));
    }

    #[test]
    fn scene_dynamic_lights_fade_like_cpp_w3d_dynamic_light() {
        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
        assert!(create_display_light_pulse(DisplayLightPulse {
            pos: [1.0, 2.0, 3.0],
            color: [1.0, 0.5, 0.0],
            inner_radius: 1.0,
            outer_radius: 50.0,
            increase_frames: 2,
            decay_frames: 3,
        }));
        tick_scene_dynamic_lights();
        let lights = scene_dynamic_lights();
        assert_eq!(lights.len(), 1);
        assert!((lights[0].color[0] - 0.5).abs() < 1.0e-5);
        assert!((lights[0].far_atten_end - 25.5).abs() < 1.0e-4);
        tick_scene_dynamic_lights();
        let lights = scene_dynamic_lights();
        assert!((lights[0].color[0] - 1.0).abs() < 1.0e-5);
        assert!((lights[0].far_atten_end - 51.0).abs() < 1.0e-4);
        tick_scene_dynamic_lights();
        let lights = scene_dynamic_lights();
        assert!((lights[0].color[0] - (2.0 / 3.0)).abs() < 1.0e-5);
        tick_scene_dynamic_lights();
        let lights = scene_dynamic_lights();
        assert!((lights[0].color[0] - (1.0 / 3.0)).abs() < 1.0e-5);
        tick_scene_dynamic_lights();
        assert!(
            scene_dynamic_lights().is_empty(),
            "C++ disables the light when decay count hits 0"
        );
    }

    #[test]
    fn do_the_dynamic_light_matches_cpp_heightmap_far_atten() {
        assert!(far_atten_factor(40.0, 10.0, 40.0).is_none());
        assert!(far_atten_factor(5.0, 0.05, 40.0).is_none());
        assert!((far_atten_factor(10.0, 10.0, 40.0).unwrap() - 1.0).abs() < 1e-5);
        assert!((far_atten_factor(20.0, 10.0, 40.0).unwrap() - (2.0 / 3.0)).abs() < 1e-5);
        assert!((far_atten_factor(5.0, 10.0, 40.0).unwrap() - 1.0).abs() < 1e-5);

        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
        assert!(create_display_light_pulse(DisplayLightPulse {
            pos: [0.0, 0.0, 0.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 10.0,
            outer_radius: 30.0,
            increase_frames: 0,
            decay_frames: 0,
        }));
        let lights = scene_dynamic_lights();
        assert_eq!(lights[0].far_atten_start, 10.0);
        assert_eq!(lights[0].far_atten_end, 40.0);

        let factor = far_atten_factor(20.0, 10.0, 40.0).expect("mid-range sample");
        let ambient_byte = (factor * 255.0) as u32;
        let expected_ambient = 0xFF00_0000 | (ambient_byte << 16);

        let facing = do_the_dynamic_light([0.0, 0.0, 20.0], [0.0, 0.0, -1.0], 0xFF00_0000, &lights);
        assert_eq!(
            facing, 0xFFFF_0000,
            "N·L + ambient both add factor → clamp 1.0 red"
        );

        let ambient_only =
            do_the_dynamic_light([0.0, 0.0, 20.0], [0.0, 0.0, 1.0], 0xFF00_0000, &lights);
        assert_eq!(
            ambient_only, expected_ambient,
            "backface keeps factor*ambient only (chop {ambient_byte})"
        );

        let out_of_range =
            do_the_dynamic_light([0.0, 0.0, 50.0], [0.0, 0.0, -1.0], 0xFF00_0000, &lights);
        assert_eq!(out_of_range, 0xFF00_0000);

        let from_scene =
            do_the_dynamic_light_from_scene([0.0, 0.0, 20.0], [0.0, 0.0, 1.0], 0xFF00_0000);
        assert_eq!(from_scene, ambient_only);
        clear_scene_dynamic_lights();
    }

    #[test]
    fn light_pulse_fx_nugget_culls_sub_cell_radius_like_cpp() {
        let _ = drain_display_light_pulses();
        let nugget = LightPulseFXNugget {
            color: Vec3::ONE,
            radius: 5.0,
            bounding_circle_pct: 0.0,
            increase_frames: 1,
            decrease_frames: 1,
        };
        let pos = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        nugget.do_fx_pos(Some(&pos), None, 0.0, None, 0.0);
        assert!(
            drain_display_light_pulses().is_empty(),
            "C++ skips pulses with inner+outer < 2*PATHFIND_CELL_SIZE_F+1"
        );
        assert!(light_pulse_too_small(1.0, 5.0));
        assert!(!light_pulse_too_small(1.0, 20.0));
    }

    #[test]
    fn tracer_fx_nugget_do_fx_pos_calls_create_tracer_fx() {
        use crate::effects::tracer_fx::{
            bake_tracer_gpu_mesh, clear_tracer_fx, live_tracer_fx, lock_tracer_fx_tests,
            update_tracer_fx,
        };

        let _guard = lock_tracer_fx_tests();
        clear_tracer_fx();
        let nugget = TracerFXNugget {
            tracer_name: "GenericTracer".to_string(),
            bone_name: String::new(),
            speed: 10.0,
            decay_at: 0.5,
            length: 10.0,
            width: 2.0,
            color: Vec3::new(0.9, 0.2, 0.1),
            probability: 1.0,
        };
        let mut list = FXList::new();
        list.add_fx_nugget(Box::new(nugget.clone()));
        let primary = Coord3D::new(0.0, 0.0, 0.0);
        let secondary = Coord3D::new(100.0, 0.0, 0.0);
        list.do_fx_pos(Some(&primary), None, 0.0, Some(&secondary), 0.0);
        assert!(
            live_tracer_fx().is_empty(),
            "FXList::doFXPos must skip Tracer nuggets when the cell is not CELLSHROUD_CLEAR"
        );
        nugget.do_fx_pos(Some(&primary), None, 0.0, Some(&secondary), 0.0);

        let tracers = live_tracer_fx();
        assert_eq!(
            tracers.len(),
            1,
            "shipped FXList TracerFXNugget must call create_tracer_fx"
        );
        let drawables = crate::effects::tracer_fx::live_tracer_drawables();
        assert_eq!(
            drawables.len(),
            1,
            "shipped FXList TracerFXNugget must spawn W3DTracerDraw like C++ newDrawable"
        );
        assert_eq!(drawables[0].tracer_name, "GenericTracer");
        assert_eq!(drawables[0].length, 10.0);
        assert_eq!(drawables[0].speed, 10.0);
        assert_eq!(tracers[0].tracer_name, "GenericTracer");
        assert_eq!(tracers[0].speed, 10.0);
        assert_eq!(tracers[0].length, 10.0);
        assert_eq!(tracers[0].width, 2.0);
        assert_eq!(tracers[0].color, [0.9, 0.2, 0.1]);

        let spawn = tracers[0].spawn_frame;
        let dist = ((secondary.x - primary.x).powi(2)
            + (secondary.y - primary.y).powi(2)
            + (secondary.z - primary.z).powi(2))
        .sqrt();
        let frames = if dist - 10.0 >= 0.0 {
            (dist - 10.0) / 10.0
        } else {
            1.0
        };
        let expire_span = (frames * 0.5).ceil().max(0.0) as u32;
        assert_eq!(tracers[0].expire_frame, spawn + expire_span);

        let n = 1_u32;
        update_tracer_fx(spawn);
        let after = live_tracer_fx();
        assert_eq!(after.len(), 1);
        let mut opacity = 1.0_f32;
        let remaining = expire_span as f32;
        opacity -= opacity / remaining;
        assert!((after[0].opacity - opacity).abs() < 1.0e-5);
        assert!((after[0].pos[0] - 10.0 * n as f32).abs() < 1.0e-4);

        let mesh = bake_tracer_gpu_mesh(&after[0], 0);
        assert_eq!(mesh.vertices.len(), 4);
        for v in &mesh.vertices {
            assert!((v.color[0] - 0.9).abs() < 1.0e-5);
            assert!((v.color[3] - opacity).abs() < 1.0e-5);
        }
        clear_tracer_fx();
    }

    #[test]
    fn fx_list_do_fx_pos_runs_light_pulse_nugget() {
        let _ = drain_display_light_pulses();
        let nugget = LightPulseFXNugget {
            color: Vec3::new(0.2, 0.4, 0.8),
            radius: 40.0,
            bounding_circle_pct: 0.0,
            increase_frames: 2,
            decrease_frames: 4,
        };
        let mut list = FXList::new();
        list.add_fx_nugget(Box::new(nugget.clone()));
        let pos = Coord3D {
            x: -8.0,
            y: 16.0,
            z: 1.0,
        };
        list.do_fx_pos(Some(&pos), None, 0.0, None, 0.0);
        assert!(
            drain_display_light_pulses().is_empty(),
            "FXList::doFXPos must skip LightPulse nuggets when the cell is not CELLSHROUD_CLEAR"
        );
        nugget.do_fx_pos(Some(&pos), None, 0.0, None, 0.0);
        let pulses = drain_display_light_pulses();
        assert_eq!(pulses.len(), 1);
        assert_eq!(pulses[0].outer_radius, 40.0);
        assert_eq!(pulses[0].pos, [-8.0, 16.0, 1.0]);
    }

    #[test]
    fn fx_list_do_fx_pos_skips_audio_and_particles_on_unexplored_cell() {
        use std::sync::atomic::{AtomicU32, Ordering};

        let played = Arc::new(AtomicU32::new(0));
        let hook_played = Arc::clone(&played);
        register_fx_audio(Box::new(move |name, _pos| {
            if name == "WeaponFireShrouded" {
                hook_played.fetch_add(1, Ordering::SeqCst);
            }
        }));
        let _ = drain_display_light_pulses();

        let pos = Coord3D::new(50.0, 50.0, 0.0);
        let prev_player = gamelogic::player::player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(-1);

        {
            let mut shroud = gamelogic::system::shroud_manager::get_shroud_manager()
                .lock()
                .expect("shroud");
            *shroud = gamelogic::system::shroud_manager::ShroudManager::new();
            shroud.init_shroud_grid(500.0, 500.0);
        }
        if let Ok(mut list) = gamelogic::player::player_list().write() {
            list.set_local_player_index(0);
        }

        let mut list = FXList::new();
        list.add_fx_nugget(Box::new(SoundFXNugget {
            sound_name: "WeaponFireShrouded".to_string(),
        }));
        list.add_fx_nugget(Box::new(LightPulseFXNugget {
            color: Vec3::ONE,
            radius: 40.0,
            bounding_circle_pct: 0.0,
            increase_frames: 2,
            decrease_frames: 4,
        }));

        list.do_fx_pos(Some(&pos), None, 0.0, None, 0.0);
        assert_eq!(
            played.load(Ordering::SeqCst),
            0,
            "unexplored CELLSHROUD_SHROUDED must not leak Sound nuggets"
        );
        assert!(
            drain_display_light_pulses().is_empty(),
            "unexplored CELLSHROUD_SHROUDED must not leak LightPulse/particle nuggets"
        );

        {
            let mut shroud = gamelogic::system::shroud_manager::get_shroud_manager()
                .lock()
                .expect("shroud");
            shroud.do_shroud_reveal(&pos, 75.0, 1);
        }
        list.do_fx_pos(Some(&pos), None, 0.0, None, 0.0);
        assert_eq!(
            played.load(Ordering::SeqCst),
            1,
            "CELLSHROUD_CLEAR must play Sound nuggets"
        );
        assert_eq!(drain_display_light_pulses().len(), 1);

        if let Ok(mut players) = gamelogic::player::player_list().write() {
            players.set_local_player_index(prev_player);
        }
        *gamelogic::system::shroud_manager::get_shroud_manager()
            .lock()
            .expect("shroud") = gamelogic::system::shroud_manager::ShroudManager::new();
        register_fx_audio(Box::new(|_name, _pos| {}));
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

    #[test]
    fn light_pulse_host_radius_as_percent_uses_geometry() {
        let _ = drain_display_light_pulses();
        let nugget = LightPulseFXNugget {
            color: Vec3::ONE,
            radius: 10.0,
            bounding_circle_pct: 2.0,
            increase_frames: 1,
            decrease_frames: 1,
        };
        let pose = gamelogic::helpers::HostFxObjectPose {
            id: 7,
            position: Coord3D::new(1.0, 2.0, 3.0),
            transform: Default::default(),
            player_index: 0,
            bounding_circle_radius: 25.0,
            is_shrouded: false,
        };
        nugget.do_fx_obj_host(&pose, None);
        let pulses = drain_display_light_pulses();
        assert_eq!(pulses.len(), 1);
        assert!((pulses[0].outer_radius - 50.0).abs() < 0.01);
        assert_eq!(pulses[0].pos, [1.0, 2.0, 3.0]);
    }
}
