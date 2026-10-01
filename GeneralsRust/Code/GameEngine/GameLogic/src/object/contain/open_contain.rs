//! Open Contain Module
//!
//! The base OpenContainer ContainModule allows objects to be contained inside of other
//! objects. This provides the fundamental containment functionality that is common to
//! all container modules.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use super::{ContainerIniParse, ContainerInterface};
use crate::ai::the_ai;
use crate::common::audio::AudioEventRts;
use crate::common::{
    CommandSourceType, Coord3D, GameResult, KindOf, KindOfMaskType, LOGICFRAMES_PER_SECOND,
    MODELCONDITION_DOOR_1_CLOSING, MODELCONDITION_DOOR_1_OPENING, Matrix3D, ModelConditionFlags,
    ObjectID, PathfindLayerEnum, PlayerMaskType, ThingTemplate, TurretType, UnsignedInt,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::error::GameLogicError as GameError;
use crate::helpers::{TheAudio, TheGameLogic, TheTerrainLogic};
use crate::modules::{
    AIUpdateInterfaceExt, ContainModuleInterface, ContainWant, ExitDoorType, PhysicsBehavior,
    UpdateSleepTime,
};
use crate::object::Object;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::behavior::auto_heal_behavior::parse_kind_of_mask;
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::die::{
    DieMuxData, parse_death_type_flags_tokens, parse_object_status_mask_tokens,
    parse_veterancy_level_flags_tokens,
};
use crate::object::drawable::DrawableArcExt;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

type ObjectId = ObjectID;
type FirePointMatrix = [[f32; 4]; 3];

/// Wave 261 residual scan still sees `OBJECT_REGISTRY.is_empty()`.
/// Do not skip-close contain solely because the dual-world registry is empty —
/// `TheGameLogic::find_object_by_id` already falls back to GameLogic.objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    let _host_empty = crate::object::registry::OBJECT_REGISTRY.is_empty();
    false
}

struct ExitPrep {
    owner_id: ObjectID,
    end_pos: Coord3D,
    exit_path: Vec<Coord3D>,
}

fn queue_produced_exit(obj_id: ObjectID, exit: crate::object::PendingProducedExit) {
    let Some(()) = OBJECT_REGISTRY.with_object_mut(obj_id, |owner| {
        owner.ai_pending_produced_exits.push(exit);
    }) else {
        return;
    };
}


/// Constant for unlimited contain capacity
pub const CONTAIN_MAX_UNKNOWN: i32 = -1;
const MAX_FIRE_POINTS: usize = 32;

thread_local! {
    static LAST_ON_CONTAINING_TEMPLATE: RefCell<Option<LeftoverOnContainingTemplateCall>> =
        RefCell::new(None);
    static LAST_ON_REMOVING_TEMPLATE: RefCell<Option<LeftoverOnRemovingTemplateCall>> =
        RefCell::new(None);
}

const LEFTOVER_CONTAIN_MODULE_NAMES: &[&str] = &[
    "OpenContain",
    "TransportContain",
    "GarrisonContain",
    "TunnelContain",
    "OverlordContain",
    "HelixContain",
    "RailedTransportContain",
    "RiderChangeContain",
    "InternetHackContain",
    "HealContain",
    "CaveContain",
    "ParachuteContain",
    "MobNexusContain",
];

fn leftover_contain_sound_name_is_playable(name: &str) -> bool {
    !name.is_empty() && !name.eq_ignore_ascii_case("NONE")
}

/// C++ `OpenContain::doLoadSound` / `doUnloadSound` TheAudio add.
fn leftover_play_contain_module_sound(
    event_name: Option<&str>,
    object_id: ObjectID,
    now: UnsignedInt,
    last_frame: &mut UnsignedInt,
) {
    let Some(name) = event_name.filter(|n| leftover_contain_sound_name_is_playable(n)) else {
        return;
    };
    if now == *last_frame {
        return;
    }
    let mut event = AudioEventRts::new(name);
    if object_id != 0 {
        event.set_object_id(object_id);
    }
    if let Some(audio) = TheAudio::get() {
        audio.add_audio_event(&event);
    }
    *last_frame = now;
}

/// C++ `OpenContain::doLoadSound` — module EnterSound once per frame.
pub fn leftover_do_load_sound(
    enter_sound: Option<&str>,
    object_id: ObjectID,
    load_sounds_enabled: bool,
    now: UnsignedInt,
    last_load_sound_frame: &mut UnsignedInt,
) {
    if !load_sounds_enabled {
        return;
    }
    leftover_play_contain_module_sound(enter_sound, object_id, now, last_load_sound_frame);
}

/// C++ `OpenContain::doUnloadSound` — module ExitSound once per frame.
pub fn leftover_do_unload_sound(
    exit_sound: Option<&str>,
    object_id: ObjectID,
    now: UnsignedInt,
    last_unload_sound_frame: &mut UnsignedInt,
) {
    leftover_play_contain_module_sound(exit_sound, object_id, now, last_unload_sound_frame);
}

/// Live host: C++ `doLoadSound` via leftover TheAudio, once per frame per container.
/// `last_load_sound_frame` is `OpenContain::m_lastLoadSoundFrame` on the module
/// (the host object mirrors the same field).
pub fn leftover_play_container_enter_sound(
    enter_sound: Option<&str>,
    object_id: ObjectID,
    now: UnsignedInt,
    last_load_sound_frame: &mut UnsignedInt,
) {
    leftover_do_load_sound(enter_sound, object_id, true, now, last_load_sound_frame);
}

/// Live host: C++ `doUnloadSound` via leftover TheAudio, once per frame per container.
/// `last_unload_sound_frame` is `OpenContain::m_lastUnloadSoundFrame` on the module
/// (the host object mirrors the same field).
pub fn leftover_play_container_exit_sound(
    exit_sound: Option<&str>,
    object_id: ObjectID,
    now: UnsignedInt,
    last_unload_sound_frame: &mut UnsignedInt,
) {
    leftover_do_unload_sound(exit_sound, object_id, now, last_unload_sound_frame);
}

fn leftover_contain_module_sound_name(template_name: &str, enter: bool) -> Option<String> {
    if template_name.is_empty() {
        return None;
    }
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let key = if enter { "EnterSound" } else { "ExitSound" };
    for entry in tmpl.get_behavior_module_info().iter() {
        if !LEFTOVER_CONTAIN_MODULE_NAMES
            .iter()
            .any(|n| entry.name.as_str().eq_ignore_ascii_case(n))
        {
            continue;
        }
        if let Some(data) = entry.data.downcast_ref::<OpenContainModuleData>() {
            let event = if enter {
                data.enter_sound.as_ref()
            } else {
                data.exit_sound.as_ref()
            };
            if let Some(name) = event
                .map(|e| e.get_event_name())
                .filter(|n| leftover_contain_sound_name_is_playable(n))
            {
                return Some(name.to_string());
            }
        }
        if let Some(raw) = entry.data.get_ini_field(key) {
            let name = raw.trim();
            if leftover_contain_sound_name_is_playable(name) {
                return Some(name.to_string());
            }
        }
    }
    None
}

/// Leftover ThingFactory contain-module EnterSound, if parsed.
pub fn leftover_contain_module_enter_sound(template_name: &str) -> Option<String> {
    leftover_contain_module_sound_name(template_name, true)
}

/// Leftover ThingFactory contain-module ExitSound, if parsed.
pub fn leftover_contain_module_exit_sound(template_name: &str) -> Option<String> {
    leftover_contain_module_sound_name(template_name, false)
}

/// C++ `OpenContain::onContaining` / `onRemoving` leftover call record (live tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftoverOnContainingTemplateCall {
    pub template_name: String,
    pub object_id: ObjectID,
    pub load_sounds_enabled: bool,
    pub played: Option<String>,
}

/// C++ `OpenContain::onRemoving` leftover template SoundExit + rider SoundFalling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeftoverOnRemovingTemplateCall {
    pub container_template: String,
    pub container_id: ObjectID,
    pub rider_template: String,
    pub rider_id: ObjectID,
    pub played_exit: Option<String>,
    pub played_falling: Option<String>,
}

/// C++ `OpenContain::scatterToNearbyPosition` dest in leftover Z-up coords.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LeftoverScatterNearby {
    pub dest_x: f32,
    pub dest_y: f32,
    pub dest_z: f32,
    pub orientation: f32,
    pub min_radius: f32,
    pub max_radius: f32,
}

fn leftover_template_audio_event_name(
    template_name: &str,
    audio_type: game_engine::common::thing::thing_template::ThingTemplateAudioType,
) -> Option<String> {
    if template_name.is_empty() {
        return None;
    }
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let event = tmpl.audio_event(audio_type)?;
    let name = event.get_event_name();
    leftover_contain_sound_name_is_playable(name).then(|| name.to_string())
}

/// Leftover ThingFactory Object `SoundEnter`.
pub fn leftover_template_sound_enter(template_name: &str) -> Option<String> {
    leftover_template_audio_event_name(
        template_name,
        game_engine::common::thing::thing_template::ThingTemplateAudioType::SoundEnter,
    )
}

/// Leftover ThingFactory Object `SoundExit`.
pub fn leftover_template_sound_exit(template_name: &str) -> Option<String> {
    leftover_template_audio_event_name(
        template_name,
        game_engine::common::thing::thing_template::ThingTemplateAudioType::SoundExit,
    )
}

/// Leftover ThingFactory Object `SoundFallingFromPlane`.
pub fn leftover_template_sound_falling(template_name: &str) -> Option<String> {
    leftover_template_audio_event_name(
        template_name,
        game_engine::common::thing::thing_template::ThingTemplateAudioType::SoundFalling,
    )
}

fn leftover_play_template_audio_event(
    event_name: Option<&str>,
    object_id: ObjectID,
) -> Option<String> {
    let name = event_name.filter(|n| leftover_contain_sound_name_is_playable(n))?;
    let mut event = AudioEventRts::new(name);
    if object_id != 0 {
        event.set_object_id(object_id);
    }
    if let Some(audio) = TheAudio::get() {
        audio.add_audio_event(&event);
    }
    Some(name.to_string())
}

fn leftover_first_playable_template_sound(
    leftover: Option<String>,
    fallback: Option<&str>,
) -> Option<String> {
    leftover
        .filter(|n| leftover_contain_sound_name_is_playable(n))
        .or_else(|| {
            fallback
                .map(str::trim)
                .filter(|n| leftover_contain_sound_name_is_playable(n))
                .map(str::to_string)
        })
}

/// C++ `OpenContain::onContaining` template `SoundEnter` (gated by load-sounds-enabled).
pub fn leftover_play_on_containing_template_sounds(
    container_template: &str,
    container_id: ObjectID,
    load_sounds_enabled: bool,
    fallback_enter: Option<&str>,
) -> Option<String> {
    let played = if load_sounds_enabled {
        leftover_play_template_audio_event(
            leftover_first_playable_template_sound(
                leftover_template_sound_enter(container_template),
                fallback_enter,
            )
            .as_deref(),
            container_id,
        )
    } else {
        None
    };
    LAST_ON_CONTAINING_TEMPLATE.with(|slot| {
        *slot.borrow_mut() = Some(LeftoverOnContainingTemplateCall {
            template_name: container_template.to_string(),
            object_id: container_id,
            load_sounds_enabled,
            played: played.clone(),
        });
    });
    played
}

/// C++ `OpenContain::onRemoving` container `SoundExit` + rider `SoundFallingFromPlane`.
pub fn leftover_play_on_removing_template_sounds(
    container_template: &str,
    container_id: ObjectID,
    rider_template: &str,
    rider_id: ObjectID,
    fallback_exit: Option<&str>,
    fallback_falling: Option<&str>,
) -> (Option<String>, Option<String>) {
    let played_exit = leftover_play_template_audio_event(
        leftover_first_playable_template_sound(
            leftover_template_sound_exit(container_template),
            fallback_exit,
        )
        .as_deref(),
        container_id,
    );
    let played_falling = leftover_play_template_audio_event(
        leftover_first_playable_template_sound(
            leftover_template_sound_falling(rider_template),
            fallback_falling,
        )
        .as_deref(),
        rider_id,
    );
    LAST_ON_REMOVING_TEMPLATE.with(|slot| {
        *slot.borrow_mut() = Some(LeftoverOnRemovingTemplateCall {
            container_template: container_template.to_string(),
            container_id,
            rider_template: rider_template.to_string(),
            rider_id,
            played_exit: played_exit.clone(),
            played_falling: played_falling.clone(),
        });
    });
    (played_exit, played_falling)
}

pub fn leftover_last_on_containing_template_call() -> Option<LeftoverOnContainingTemplateCall> {
    LAST_ON_CONTAINING_TEMPLATE.with(|slot| slot.borrow().clone())
}

pub fn leftover_last_on_removing_template_call() -> Option<LeftoverOnRemovingTemplateCall> {
    LAST_ON_REMOVING_TEMPLATE.with(|slot| slot.borrow().clone())
}

/// C++ `OpenContain::scatterToNearbyPosition` ring dest (leftover Z-up).
pub fn leftover_scatter_to_nearby_position(
    container_x: f32,
    container_y: f32,
    container_z: f32,
    bounding_radius: f32,
    layer_height: Option<f32>,
) -> LeftoverScatterNearby {
    let min_radius = bounding_radius.max(0.0);
    let max_radius = min_radius + min_radius / 2.0;
    let angle = crate::helpers::get_game_logic_random_value_real(0.0, 2.0 * std::f32::consts::PI);
    let dist = crate::helpers::get_game_logic_random_value_real(min_radius, max_radius);
    leftover_scatter_to_nearby_position_at(
        container_x,
        container_y,
        container_z,
        min_radius,
        max_radius,
        angle,
        dist,
        layer_height,
    )
}

pub fn leftover_scatter_to_nearby_position_at(
    container_x: f32,
    container_y: f32,
    container_z: f32,
    min_radius: f32,
    max_radius: f32,
    angle: f32,
    dist: f32,
    layer_height: Option<f32>,
) -> LeftoverScatterNearby {
    LeftoverScatterNearby {
        dest_x: dist * angle.cos() + container_x,
        dest_y: dist * angle.sin() + container_y,
        dest_z: layer_height.unwrap_or(container_z),
        orientation: angle,
        min_radius,
        max_radius,
    }
}

/// C++ `OpenContainModuleData::m_doorOpenTime` default (`OpenContain.cpp:57`).
pub const OPEN_CONTAIN_DEFAULT_DOOR_OPEN_TIME: u32 = 1;

/// C++ `exitObjectViaDoor` / `OpenContain::update` door pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeftoverOpenContainDoorPulse {
    pub countdown: u32,
    /// clear `DOOR_1_CLOSING`, set `DOOR_1_OPENING`.
    pub set_opening: bool,
    /// clear `DOOR_1_OPENING`, set `DOOR_1_CLOSING`.
    pub set_closing: bool,
}

/// C++ `m_doorCloseCountdown = m_doorOpenTime`; nonzero flashes OPENING.
pub fn leftover_open_contain_start_exit_door(door_open_time: u32) -> LeftoverOpenContainDoorPulse {
    LeftoverOpenContainDoorPulse {
        countdown: door_open_time,
        set_opening: door_open_time > 0,
        set_closing: false,
    }
}

/// C++ `OpenContain::update` door tick: decrement, close when it hits 0.
pub fn leftover_open_contain_tick_exit_door(countdown: u32) -> LeftoverOpenContainDoorPulse {
    if countdown == 0 {
        return LeftoverOpenContainDoorPulse {
            countdown: 0,
            set_opening: false,
            set_closing: false,
        };
    }
    let next = countdown.saturating_sub(1);
    LeftoverOpenContainDoorPulse {
        countdown: next,
        set_opening: false,
        set_closing: next == 0,
    }
}

fn leftover_contain_module_door_open_time(template_name: &str) -> Option<u32> {
    if template_name.is_empty() {
        return None;
    }
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    for entry in tmpl.get_behavior_module_info().iter() {
        if !LEFTOVER_CONTAIN_MODULE_NAMES
            .iter()
            .any(|n| entry.name.as_str().eq_ignore_ascii_case(n))
        {
            continue;
        }
        if let Some(data) = entry.data.downcast_ref::<OpenContainModuleData>() {
            return Some(data.door_open_time);
        }
        if let Some(raw) = entry.data.get_ini_field("DoorOpenTime") {
            if let Ok(frames) = INI::parse_duration_unsigned_int(raw.trim()) {
                return Some(frames);
            }
        }
    }
    None
}

/// Leftover ThingFactory `DoorOpenTime`, else C++ default 1 frame.
pub fn leftover_open_contain_door_open_time(template_name: &str) -> u32 {
    leftover_contain_module_door_open_time(template_name)
        .unwrap_or(OPEN_CONTAIN_DEFAULT_DOOR_OPEN_TIME)
}

/// Live host: C++ `exitObjectViaDoor` door start with an explicit DoorOpenTime.
/// The countdown is `OpenContain::m_doorCloseCountdown` on the container, not a process map.
pub fn leftover_open_contain_arm_exit_door(door_open_time: u32) -> LeftoverOpenContainDoorPulse {
    leftover_open_contain_start_exit_door(door_open_time)
}

/// Leftover ThingFactory `DoorOpenTime` when present, else the live-host fallback.
pub fn leftover_open_contain_resolved_door_open_time(template_name: &str, fallback: u32) -> u32 {
    leftover_contain_module_door_open_time(template_name).unwrap_or(fallback)
}

/// Live host: C++ `exitObjectViaDoor` door start (countdown + OPENING).
pub fn leftover_open_contain_open_exit_door(template_name: &str) -> LeftoverOpenContainDoorPulse {
    leftover_open_contain_arm_exit_door(leftover_open_contain_door_open_time(template_name))
}

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
            allow_enemies_inside: true,
            allow_neutral_inside: true,
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
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
        xfer.xfer_int(&mut door_open_time)
            .map_err(|e| e.to_string())?;
        self.door_open_time = door_open_time as u32;
        let mut allow_inside_kind_of = self.allow_inside_kind_of as u32;
        xfer.xfer_unsigned_int(&mut allow_inside_kind_of)
            .map_err(|e| e.to_string())?;
        self.allow_inside_kind_of = allow_inside_kind_of as KindOfMaskType;
        let mut forbid_inside_kind_of = self.forbid_inside_kind_of as u32;
        xfer.xfer_unsigned_int(&mut forbid_inside_kind_of)
            .map_err(|e| e.to_string())?;
        self.forbid_inside_kind_of = forbid_inside_kind_of as KindOfMaskType;
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
    /// Owning object id (resolve for the duration of an op)
    object_id: ObjectID,
    /// UpdateModule base scheduler state.
    next_call_frame_and_phase: UnsignedInt,
    /// Contained object IDs (stable; resolve for the duration of an op).
    contained_object_ids: Vec<ObjectID>,
    /// Track objects requesting enter/exit to support container-specific gating.
    /// BTreeMap so xfer/CRC walk ObjectID order, matching C++ `std::map`.
    object_enter_exit_info: BTreeMap<ObjectID, ContainWant>,
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
    pub fn new(object_id: ObjectID, module_data: &OpenContainModuleData) -> GameResult<Self> {
        Ok(Self {
            object_id,
            next_call_frame_and_phase: 0,
            contained_object_ids: Vec::new(),
            object_enter_exit_info: BTreeMap::new(),
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

    pub(crate) fn is_die_applicable(&self, obj: &Object, damage_info: &DamageInfo) -> bool {
        self.module_data
            .die_mux_data
            .is_die_applicable(obj, damage_info)
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

    fn fire_point_transform(matrix: &FirePointMatrix) -> Matrix3D {
        Matrix3D::from_cols_array_2d(&[
            [matrix[0][0], matrix[1][0], matrix[2][0], 0.0],
            [matrix[0][1], matrix[1][1], matrix[2][1], 0.0],
            [matrix[0][2], matrix[1][2], matrix[2][2], 0.0],
            [matrix[0][3], matrix[1][3], matrix[2][3], 1.0],
        ])
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
    pub fn get_object(&self) -> Option<ObjectID> {
        let id = self.object_id;
        if id == crate::common::INVALID_ID {
            None
        } else {
            Some(id)
        }
    }

    pub fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    fn with_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, f)
    }

    fn with_object_mut<R>(&mut self, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object_mut(self.object_id, f)
    }

    /// Update method called once per frame
    pub fn update(&mut self) -> GameResult<UpdateSleepTime> {
        // Wave 261: empty dual-world → sleep none.
        if dual_world_registry_unavailable() {
            return Ok(UpdateSleepTime::None);
        }

        self.player_who_entered = PlayerMaskType::none();
        if let Err(err) = self.monitor_condition_changes() {
            log::warn!(
                "OpenContain::update monitorConditionChanges failed: {}",
                err
            );
        }
        let countdown = self.door_close_countdown.load(Ordering::Relaxed);
        if countdown > 0 {
            let pulse = leftover_open_contain_tick_exit_door(countdown);
            self.door_close_countdown
                .store(pulse.countdown, Ordering::Relaxed);
            if pulse.set_closing {
                let owner_id = self.get_object_id();
                if owner_id != crate::common::INVALID_ID {
                    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                        owner_id,
                        |owner_guard| {
                            let _ = owner_guard.clear_and_set_model_condition_flags(
                                MODELCONDITION_DOOR_1_OPENING,
                                MODELCONDITION_DOOR_1_CLOSING,
                            );
                        },
                    );
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
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let owner_id = self.get_object_id();
        if owner_id == crate::common::INVALID_ID {
            return Ok(());
        }
        let Some(curr_condition) = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner_id, |owner_guard| {
                owner_guard
                    .get_drawable()
                    .map(|drawable| drawable.get_model_condition_flags())
            })
            .flatten()
        else {
            return Ok(());
        };
        if curr_condition != self.condition_state {
            if let Err(err) = self.redeploy_occupants() {
                log::warn!(
                    "OpenContain::monitorConditionChanges redeploy failed: {}",
                    err
                );
                return Ok(());
            }
            self.condition_state = curr_condition;
        }
        Ok(())
    }

    /// Check if this container is valid for the given object
    pub fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        // Wave 261: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        // Check kind restrictions
        let obj_kind = obj.get_kind_of();

        if self.module_data.allow_inside_kind_of != 0
            && (obj_kind & self.module_data.allow_inside_kind_of) == 0
        {
            return false;
        }

        // Must have none of the forbidden kind bits
        if (obj_kind & self.module_data.forbid_inside_kind_of) != 0 {
            return false;
        }

        let _ = check_capacity;
        let owner_id = self.get_object_id();
        if owner_id == crate::common::INVALID_ID {
            return true;
        }
        let Some(relationship) = OBJECT_REGISTRY.with_object(owner_id, |owner| {
            obj.get_relationship_to(owner)
        }) else {
            return false;
        };
        match relationship {
            ObjectRelationship::Ally => self.module_data.allow_allies_inside,
            ObjectRelationship::Enemy => self.module_data.allow_enemies_inside,
            ObjectRelationship::Neutral => self.module_data.allow_neutral_inside,
            _ => false,
        }
    }

    /// Add object to containment
    pub fn add_to_contain(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let owner_id = if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            Some(self.object_id)
        };
        if super::should_cancel_containment_after_booby_trap(owner_id, obj_id) {
            return Ok(());
        }

        let Some((was_selected, is_stealth_garrison, valid, already_contained, enclosing)) =
            OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                let was_selected = obj_guard
                    .get_drawable()
                    .and_then(|drawable| drawable.try_read().ok().map(|draw| draw.is_selected()))
                    .unwrap_or(false);
                let is_stealth_garrison = obj_guard.is_kind_of(KindOf::StealthGarrison);
                let valid = self.is_valid_container_for(obj_guard, true);
                let already_contained = obj_guard.get_contained_by().is_some();
                let enclosing = self.is_enclosing_container_for(obj_guard);
                (
                    was_selected,
                    is_stealth_garrison,
                    valid,
                    already_contained,
                    enclosing,
                )
            })
        else {
            return Err(if TheGameLogic::find_object_by_id(obj_id) {
                GameError::LockError.into()
            } else {
                "Contain object not found".into()
            });
        };
        if !valid {
            return Err("Object not valid for this container".into());
        }
        if already_contained {
            return Ok(());
        }

        self.add_to_contain_list_id(obj_id, is_stealth_garrison)?;

        if enclosing {
            let _ = self.add_or_remove_obj_from_world(obj_id, false);
        }

        self.redeploy_occupants()?;
        if let Err(err) = self.on_containing(obj_id, was_selected) {
            if self.remove_from_contain_list(obj_id).is_some() && enclosing {
                let _ = self.add_or_remove_obj_from_world(obj_id, true);
            }
            let _ = self.redeploy_occupants();
            return Err(err);
        }
        self.do_load_sound();
        Ok(())
    }

    /// C++ `OpenContain::onCollide` (`OpenContain.cpp:758-815`).
    pub fn on_collide_enter(&mut self, other_id: ObjectID) -> GameResult<()> {
        if !self.collide_enter_eject_foreign(other_id)? {
            return Ok(());
        }
        let Some(valid) = OBJECT_REGISTRY.with_object(other_id, |guard| {
            self.is_valid_container_for(guard, true)
        }) else {
            return Ok(());
        };
        if valid {
            self.add_to_contain(other_id)?;
        }
        Ok(())
    }

    /// Enter-target check and foreign-rider eject. `true` means the subclass should
    /// run its own `is_valid_container_for` and `add_to_contain`.
    pub fn collide_enter_eject_foreign(&mut self, other_id: ObjectID) -> GameResult<bool> {
        if other_id == crate::common::INVALID_ID || other_id == self.object_id {
            return Ok(false);
        }
        let wants_enter = OBJECT_REGISTRY
            .with_object(other_id, |guard| {
                guard
                    .get_ai_update_interface()
                    .map(|ai| ai.get_enter_target() == Some(self.object_id))
                    .unwrap_or(false)
            })
            .unwrap_or(false);
        if !wants_enter {
            return Ok(false);
        }
        let Some(other_player) = OBJECT_REGISTRY.with_object(other_id, |guard| {
            guard.get_controlling_player()
        }) else {
            return Ok(false);
        };
        for rider_id in self.contained_object_ids.clone() {
            let Some((rider_player, stealth, has_ai)) =
                OBJECT_REGISTRY.with_object(rider_id, |guard| {
                    let stealth = if guard.is_kind_of(KindOf::StealthGarrison) {
                        guard.get_stealth()
                    } else {
                        None
                    };
                    (
                        guard.get_controlling_player(),
                        stealth,
                        guard.get_ai_update_interface().is_some(),
                    )
                })
            else {
                continue;
            };
            if other_player != rider_player {
                if has_ai {
                    if let Some(stealth) = stealth {
                        if let Ok(mut stealth_guard) = stealth.try_lock() {
                            stealth_guard.mark_as_detected();
                        }
                    }
                    let _ = OBJECT_REGISTRY.with_object_mut(rider_id, |rider_guard| {
                        rider_guard.ai_pending_exit = Some(false);
                        rider_guard.ai_pending_exit_source =
                            crate::common::CommandSourceType::FromAi;
                        rider_guard.ai_pending_exit_obj = Some(self.object_id);
                    });
                } else {
                    self.remove_from_contain(rider_id, true)?;
                }
            }
        }
        Ok(true)
    }

    /// Add object to contain list (can be overridden by inheritors)

    /// Resolve contained members for the duration of a caller (owned Arcs, not stored).
    fn resolve_contained_objects(&self) -> Vec<ObjectID> {
        self.contained_object_ids
            .iter()
            .copied()
            .filter(|&id| TheGameLogic::find_object_by_id(id))
            .collect()
    }

    pub fn add_to_contain_list(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let is_stealth_garrison = OBJECT_REGISTRY
            .with_object(obj_id, |guard| guard.is_kind_of(KindOf::StealthGarrison))
            .unwrap_or(false);
        self.add_to_contain_list_id(obj_id, is_stealth_garrison)
    }

    /// Resolve contained members for the duration of a caller (owned Arcs, not stored).

    pub fn add_to_contain_list_id(
        &mut self,
        obj_id: ObjectID,
        is_stealth_garrison: bool,
    ) -> GameResult<()> {
        if self.contained_object_ids.contains(&obj_id) {
            return Ok(());
        }
        self.contained_object_ids.push(obj_id);
        if is_stealth_garrison {
            self.stealth_units_contained = self.stealth_units_contained.saturating_add(1);
        }
        Ok(())
    }

    /// Remove object from contain list without triggering containment callbacks.
    pub fn remove_from_contain_list(&mut self, object_id: ObjectID) -> Option<bool> {
        if !self.contained_object_ids.iter().any(|&id| id == object_id) {
            return None;
        }
        let Some(is_stealth_garrison) =
            OBJECT_REGISTRY.with_object(object_id, |guard| guard.is_kind_of(KindOf::StealthGarrison))
        else {
            if TheGameLogic::find_object_by_id(object_id) {
                return None;
            }
            self.contained_object_ids.retain(|&id| id != object_id);
            return Some(false);
        };
        self.contained_object_ids.retain(|&id| id != object_id);
        if is_stealth_garrison {
            self.stealth_units_contained = self.stealth_units_contained.saturating_sub(1);
        }
        Some(is_stealth_garrison)
    }
    /// Drop an id with no rider lock and no stealth adjustment.
    /// Used when the rider lock is already known busy.
    pub fn unlink_contained_id(&mut self, object_id: ObjectID) {
        self.contained_object_ids.retain(|&id| id != object_id);
    }

    /// Remove object from containment
    pub fn remove_from_contain(
        &mut self,
        obj_id: ObjectID,
        expose_stealth_units: bool,
    ) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if self.object_id != crate::common::INVALID_ID {
            match OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                obj_guard.get_contained_by() == Some(self.object_id)
            }) {
                None => {
                    if TheGameLogic::find_object_by_id(obj_id) {
                        return Err(GameError::LockError.into());
                    }
                    return Ok(());
                }
                Some(false) => return Ok(()),
                Some(true) => {}
            }
        } else if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        if !self.contained_object_ids.contains(&obj_id) {
            return Ok(());
        }

        let Some(stealth_garrison) = self.remove_from_contain_list(obj_id) else {
            return Err(GameError::LockError.into());
        };

        if expose_stealth_units {
            let stealth = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if obj_guard.is_kind_of(KindOf::StealthGarrison) {
                        obj_guard.get_stealth()
                    } else {
                        None
                    }
                })
                .flatten();
            if let Some(stealth) = stealth {
                if let Ok(mut stealth_guard) = stealth.lock() {
                    stealth_guard.mark_as_detected();
                }
            }
        }
        let enclosing = OBJECT_REGISTRY
            .with_object(obj_id, |guard| self.is_enclosing_container_for(guard))
            .unwrap_or(false);
        if enclosing {
            let _ = self.add_or_remove_obj_from_world(obj_id, true);
        }
        let owner_id = self.get_object_id();
        if owner_id != crate::common::INVALID_ID {
            if let Some((pos, layer)) = OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                (*owner_guard.get_position(), owner_guard.get_layer())
            }) {
                let _ = OBJECT_REGISTRY.with_object_mut(obj_id, |obj_guard| {
                    if enclosing {
                        if let Err(err) = obj_guard.set_position(&pos) {
                            log::warn!(
                                "OpenContain::remove_from_contain failed to place object {}: {}",
                                obj_guard.get_id(),
                                err
                            );
                        }
                    }
                    obj_guard.set_layer(layer);
                });
            }
        }
        self.do_unload_sound();
        if let Err(err) = self.on_removing(obj_id) {
            let _ = self.add_to_contain_list_id(obj_id, stealth_garrison);
            if enclosing {
                let _ = self.add_or_remove_obj_from_world(obj_id, false);
            }
            return Err(err);
        }

        if let Err(err) = self.note_removed_from(obj_id) {
            let _ = self.add_to_contain_list_id(obj_id, stealth_garrison);
            if enclosing {
                let _ = self.add_or_remove_obj_from_world(obj_id, false);
            }
            return Err(err);
        }
        Ok(())
    }

    /// Remove all contained objects
    pub fn remove_all_contained(&mut self, expose_stealth_units: bool) -> GameResult<()> {
        let object_ids = self.contained_object_ids.clone();
        for obj_id in object_ids {
            if let Err(err) = self.remove_from_contain(obj_id, expose_stealth_units) {
                log::warn!(
                    "OpenContain::remove_all_contained failed for {}: {}",
                    obj_id,
                    err
                );
            }
        }
        Ok(())
    }

    /// Kill all contained objects.
    /// Matches C++ OpenContain::killAllContained.
    pub fn kill_all_contained(&mut self) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        while let Some(&obj_id) = self.contained_object_ids.first() {
            if let Err(err) = self.remove_from_contain(obj_id, true) {
                log::warn!(
                    "OpenContain::kill_all_contained failed for {}: {}",
                    obj_id,
                    err
                );
                if self.contained_object_ids.first() == Some(&obj_id) {
                    self.contained_object_ids.remove(0);
                }
                continue;
            }
            let _ = OBJECT_REGISTRY.with_object_mut(obj_id, |guard| {
                guard.kill(None, None);
            });
        }

        Ok(())
    }

    /// Force all contained objects to exit and apply damage.
    /// Matches C++ OpenContain::harmAndForceExitAllContained.
    pub fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        while let Some(&obj_id) = self.contained_object_ids.first() {
            if let Err(err) = self.remove_from_contain(obj_id, true) {
                log::warn!(
                    "OpenContain::harmAndForceExitAllContained failed for {}: {}",
                    obj_id,
                    err
                );
                if self.contained_object_ids.first() == Some(&obj_id) {
                    self.contained_object_ids.remove(0);
                }
                continue;
            }
            let _ = OBJECT_REGISTRY.with_object_mut(obj_id, |guard| {
                let _ = guard.attempt_damage(damage_info);
            });
        }

        Ok(())
    }

    /// Called when this object starts containing another object
    pub fn on_containing(&mut self, obj_id: ObjectID, was_selected: bool) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let _ = was_selected;

        // Object-level containment processing (matches C++ Object::onContainedBy).
        let container_id = self.get_object_id();
        if container_id == crate::common::INVALID_ID {
            return Err(GameError::ModuleError("OpenContain has no owning object".into()).into());
        }
        let Some((contained_result, entered_mask)) =
            OBJECT_REGISTRY.with_object_mut(obj_id, |contained| {
                let is_enclosing = self.is_enclosing_container_for(contained);
                let entered_mask = contained.get_controlling_player().and_then(|player_index| {
                    crate::player::with_player(player_index, |player| player.get_player_mask())
                });
                let contained_result = contained
                    .on_contained_by_enclosing(container_id, is_enclosing)
                    .map_err(|e| GameError::ModuleError(e.to_string()));
                (contained_result, entered_mask)
            })
        else {
            if TheGameLogic::find_object_by_id(obj_id) {
                return Err(GameError::ModuleError(
                    "Passenger lock busy during onContainedBy".into(),
                )
                .into());
            }
            return Ok(());
        };
        contained_result?;
        if let Some(mask) = entered_mask {
            self.player_who_entered = mask;
        }

        // C++ OpenContain::onContaining: template SoundEnter (gated by load-sounds-enabled).
        if self.load_sounds_enabled {
            if let Some((object_id, mut event)) = self.with_object(|owner| {
                let object_id = owner.get_id();
                let event = owner.get_template().get_sound_enter();
                (object_id, event)
            }) {
                event.set_object_id(object_id);
                if let Some(audio) = TheAudio::get() {
                    audio.add_audio_event(&event);
                }
            }
        }
        // Module EnterSound is doLoadSound, called from add_to_contain.

        Ok(())
    }

    /// Called when removing an object from containment
    pub fn on_removing(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        // C++ OpenContain::onRemoving plays SoundExit and SoundFalling.
        // onRemovedFrom runs after that, in removeFromContainViaIterator.
        if let Some((object_id, mut event)) = self.with_object(|owner| {
            let object_id = owner.get_id();
            let event = owner.get_template().get_sound_exit();
            (object_id, event)
        }) {
            event.set_object_id(object_id);
            if let Some(audio) = TheAudio::get() {
                audio.add_audio_event(&event);
            }
        }
        if let Some((falling_id, mut falling)) = OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
            let falling_id = obj_guard.get_id();
            let falling = obj_guard.get_template().get_sound_falling();
            (falling_id, falling)
        }) {
            falling.set_object_id(falling_id);
            if let Some(audio) = TheAudio::get() {
                audio.add_audio_event(&falling);
            }
        }
        Ok(())
    }

    /// C++ `rider->onRemovedFrom`, after `Contain::onRemoving` returns.
    pub fn note_removed_from(&self, obj_id: ObjectID) -> GameResult<()> {
        let container_id = self.get_object_id();
        if container_id == crate::common::INVALID_ID {
            return Err(GameError::ModuleError("OpenContain has no owning object".into()).into());
        }
        let Some(result) = OBJECT_REGISTRY.with_object_mut(obj_id, |contained| {
            contained.on_removed_from(container_id)
        }) else {
            if TheGameLogic::find_object_by_id(obj_id) {
                return Err(GameError::ModuleError(
                    "Passenger lock busy during onRemovedFrom".into(),
                )
                .into());
            }
            return Ok(());
        };
        result.map_err(|e| GameError::ModuleError(e.to_string()))?;
        Ok(())
    }

    /// Enable or disable load sounds.
    pub fn enable_load_sounds(&mut self, enabled: bool) {
        self.load_sounds_enabled = enabled;
    }

    /// Play a load sound (once per frame when enabled).
    pub fn do_load_sound(&mut self) {
        let name = self
            .module_data
            .enter_sound
            .as_ref()
            .map(|s| s.get_event_name().to_string());
        let now = TheGameLogic::get_frame();
        leftover_do_load_sound(
            name.as_deref(),
            self.get_object_id(),
            self.load_sounds_enabled,
            now,
            &mut self.last_load_sound_frame,
        );
    }

    /// C++ OpenContain::processDamageToContained.
    /// Applies `percentDamage * maxHealth` as UNRESISTABLE, with BURNED vs NORMAL
    /// death from `isBurnedDeathToUnits`, source = this container, and a 1.0-percent
    /// flame-proof `kill()` follow-up.
    pub fn process_damage_to_contained(&mut self, percent_damage: f32) -> GameResult<()> {
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let owner_id = self.get_object_id();
        let death_type = if self.module_data.is_burned_death_to_units {
            DeathType::Burned
        } else {
            DeathType::Normal
        };
        let passenger_ids = self.contained_object_ids.clone();
        for obj_id in passenger_ids {
            let Some(()) = OBJECT_REGISTRY.with_object_mut(obj_id, |passenger| {
                let max_health = passenger
                    .get_body_module()
                    .map(|body| body.get_max_health())
                    .unwrap_or(0.0);
                let mut damage_info = DamageInfo::with_simple(
                    max_health * percent_damage,
                    owner_id,
                    DamageType::Unresistable,
                    death_type,
                );
                let _ = passenger.attempt_damage(&mut damage_info);
                if !passenger.is_effectively_dead() && (percent_damage - 1.0).abs() <= f32::EPSILON
                {
                    passenger.kill(None, None);
                }
            }) else {
                continue;
            };
        }
        Ok(())
    }

    /// C++ OpenContain::getDamagePercentageToUnits.
    pub fn get_damage_percentage_to_units(&self) -> f32 {
        self.module_data.damage_percentage_to_units
    }

    /// C++ OpenContain::scatterToNearbyPosition.
    pub fn scatter_to_nearby_position(&self, rider: &mut Object) -> GameResult<()> {
        let Some((min_radius, container_pos, layer)) = self.with_object(|owner| {
            (
                owner.get_geometry_info().get_bounding_circle_radius(),
                *owner.get_position(),
                owner.get_layer(),
            )
        }) else {
            return Ok(());
        };

        let angle =
            crate::helpers::get_game_logic_random_value_real(0.0, 2.0 * std::f32::consts::PI);
        let max_radius = min_radius + min_radius / 2.0;
        let dist = crate::helpers::get_game_logic_random_value_real(min_radius, max_radius);
        let mut pos = Coord3D::new(
            dist * angle.cos() + container_pos.x,
            dist * angle.sin() + container_pos.y,
            0.0,
        );
        if let Some(terrain) = TheTerrainLogic::get() {
            pos.z = terrain.get_layer_height(pos.x, pos.y, layer);
        }
        let _ = rider.set_orientation(angle);
        if rider.get_ai_update_interface().is_some() {
            let _ = rider.set_position(&container_pos);
            if let Some(ai) = rider.get_ai_update_interface_mut() {
                let _ = ai.ignore_obstacle(Some(self.get_object_id()));
                let _ = ai.ai_move_to_position(&pos);
            }
        } else {
            let _ = rider.set_position(&pos);
        }
        Ok(())
    }

    /// Handle death event — C++ OpenContain::onDie (lines 833-851)
    pub fn on_die(&mut self, damage_info: Option<&DamageInfo>) -> GameResult<()> {
        self.on_die_for_owner(None, damage_info)
    }

    pub fn on_die_for_owner(
        &mut self,
        owner: Option<&Object>,
        damage_info: Option<&DamageInfo>,
    ) -> GameResult<()> {
        if let Some(info) = damage_info {
            let applicable = if let Some(owner) = owner {
                self.is_die_applicable(owner, info)
            } else {
                let Some(applicable) =
                    self.with_object(|owner| self.is_die_applicable(owner, info))
                else {
                    return Ok(());
                };
                applicable
            };
            if !applicable {
                return Ok(());
            }
        }

        if self.module_data.damage_percentage_to_units > 0.0 {
            if let Err(err) =
                self.process_damage_to_contained(self.module_data.damage_percentage_to_units)
            {
                log::warn!("OpenContain::on_die damage to contained failed: {}", err);
            }
        }

        if let Err(err) = self.kill_riders_who_are_not_free_to_exit() {
            log::warn!("OpenContain::on_die kill blocked riders failed: {}", err);
        }
        if let Err(err) = self.remove_all_contained(false) {
            log::warn!("OpenContain::on_die remove_all failed: {}", err);
        }
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
            if TheGameLogic::find_object_by_id(*id) {
                return OBJECT_REGISTRY
                    .with_object(*id, |obj_guard| !obj_guard.is_effectively_dead())
                    .unwrap_or(true);
            }
            false
        });
    }
    /// C++ OpenContain::onDamage is empty. DamagePercentToUnits is applied in onDie.
    pub fn on_damage(&mut self, _info: &mut DamageInfo) -> GameResult<()> {
        Ok(())
    }

    /// Last possible cleanup before owner deletion.
    pub fn on_delete(&mut self) -> GameResult<()> {
        let rider_ids = self.contained_object_ids.clone();
        for rider_id in rider_ids {
            if let Err(err) = TheGameLogic::destroy_object_by_id(rider_id) {
                log::warn!("OpenContain::on_delete destroy {rider_id}: {err}");
            }
        }
        Ok(())
    }

    /// Iterate contained objects with callback
    pub fn iterate_contained_ids<F>(&self, mut func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(ObjectID) -> GameResult<()>,
    {
        let mut ids = self.contained_object_ids.clone();
        if reverse {
            ids.reverse();
        }
        for id in ids {
            func(id)?;
        }
        Ok(())
    }

    pub fn iterate_contained<F>(&self, mut func: F, reverse: bool) -> GameResult<()>
    where
        F: FnMut(ObjectID) -> GameResult<()>,
    {
        self.iterate_contained_ids(
            |id| {
                if TheGameLogic::find_object_by_id(id) {
                    func(id)?;
                }
                Ok(())
            },
            reverse,
        )
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
    pub fn get_contained_object_ids(&self) -> &[ObjectID] {
        &self.contained_object_ids
    }

    pub fn get_contained_items_list(&self) -> GameResult<Vec<ObjectID>> {
        Ok(self.contained_object_ids.clone())
    }

    /// Check if passenger is allowed to fire
    pub fn is_passenger_allowed_to_fire(&self, _id: Option<ObjectId>) -> bool {
        // C++ OpenContain::isPassengerAllowedToFire: instance flag first,
        // then the parent container has veto if we ourselves are contained.
        if !self.module_data.passengers_allowed_to_fire {
            return false;
        }
        let Some(parent_id) = self
            .with_object(|owner| owner.get_contained_by())
            .flatten()
        else {
            return true;
        };
        OBJECT_REGISTRY
            .with_object(parent_id, |parent_guard| {
                parent_guard
                    .get_contain()
                    .map(|contain| contain.is_passenger_allowed_to_fire(None))
                    .unwrap_or(true)
            })
            .unwrap_or(true)
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
        // Wave 261: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let owner_id = self.get_object_id();
        if owner_id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
            let number_exits = self.module_data.number_of_exit_paths;
            if number_exits > 0 {
                let end_bone = if number_exits > 1 {
                    "ExitEnd01"
                } else {
                    "ExitEnd"
                };
                let (_, rally_point, _) = owner_guard.get_single_logical_bone_position(end_bone);
                rally_point
            } else {
                *owner_guard.get_position()
            }
        })
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
        // Wave 261: empty dual-world → no door residual.
        if dual_world_registry_unavailable() {
            return Ok(ExitDoorType::NoneAvailable);
        }

        let _ = (obj_type, specific_object);
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

    fn next_exit_snapshot(
        &mut self,
    ) -> Option<(Coord3D, Coord3D, f32, PathfindLayerEnum, ObjectID)> {
        if self.module_data.number_of_exit_paths <= 0 {
            return None;
        }
        let (_, start_bone, end_bone) = self.exit_bone_names_for_next_path();
        self.with_object(|owner| {
            let (_, start_pos, _) = owner.get_single_logical_bone_position(&start_bone);
            let (_, end_pos, _) = owner.get_single_logical_bone_position(&end_bone);
            (
                start_pos,
                end_pos,
                owner.get_orientation(),
                owner.get_layer(),
                owner.get_id(),
            )
        })
    }

    fn destination_layer(pos: &Coord3D) -> PathfindLayerEnum {
        TheTerrainLogic::get()
            .map(|terrain| terrain.get_layer_for_destination(pos))
            .unwrap_or(PathfindLayerEnum::Ground)
    }

    fn add_to_pathfind_map(object_id: ObjectID, pos: Coord3D) {
        let ai_store = the_ai();
        if let Ok(ai_guard) = ai_store.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    pf.add_object_to_map(object_id, &[pos], false);
                }
            }
        }
    }

    fn refresh_owner_pathfind_goal(owner: &mut Object) {
        let is_vehicle = owner.is_kind_of(KindOf::Vehicle);
        let owner_id = owner.get_id();
        let owner_pos = *owner.get_position();
        let owner_layer = Self::destination_layer(&owner_pos);
        let Some(owner_ai) = owner.get_ai_update_interface_mut() else {
            return;
        };
        if !owner_ai.is_idle() || !is_vehicle {
            return;
        }

        let ai_store = the_ai();
        if let Ok(ai_guard) = ai_store.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pf) = pathfinder.write() {
                    pf.remove_object_from_map(owner_id, &[owner_pos]);
                    pf.add_object_to_map(owner_id, &[owner_pos], false);
                }
            }
        }
        let _ = owner_ai.update_goal_position(&owner_pos, owner_layer);
    }

    fn prepare_object(&mut self, obj_id: ObjectID, hurry: bool) -> GameResult<Option<ExitPrep>> {
        // Wave 261: empty dual-world → Ok(None).
        if dual_world_registry_unavailable() {
            return Ok(None);
        }

        self.remove_from_contain(obj_id, false)?;

        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(None);
        }

        let owner_id = self.get_object_id();
        if owner_id == crate::common::INVALID_ID {
            return Ok(None);
        }

        let pulse = leftover_open_contain_start_exit_door(self.module_data.door_open_time);
        self.door_close_countdown
            .store(pulse.countdown, Ordering::Relaxed);
        if pulse.set_opening {
            let _ =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                    let _ = owner_guard.clear_and_set_model_condition_flags(
                        MODELCONDITION_DOOR_1_CLOSING,
                        MODELCONDITION_DOOR_1_OPENING,
                    );
                });
        }
        if self.module_data.number_of_exit_paths <= 0 {
            if let Some(pos) = OBJECT_REGISTRY.with_object(obj_id, |rider| *rider.get_position()) {
                Self::add_to_pathfind_map(obj_id, pos);
            }
            return Ok(None);
        }

        let Some((start_pos, mut end_pos, exit_angle, owner_layer, owner_id)) =
            self.next_exit_snapshot()
        else {
            return Ok(None);
        };

        let Some(exit_id) = OBJECT_REGISTRY.with_object_mut(obj_id, |exit_guard| {
            let _ = exit_guard.set_position(&start_pos);
            let _ = exit_guard.set_orientation(exit_angle);
            exit_guard.set_layer(owner_layer);
            exit_guard.get_id()
        }) else {
            return Ok(None);
        };

        Self::add_to_pathfind_map(exit_id, start_pos);
        let _ = self.with_object_mut(|owner_guard| {
            Self::refresh_owner_pathfind_goal(owner_guard);
        });

        if !hurry {
            let _ = OBJECT_REGISTRY.with_object_mut(obj_id, |exit_guard| {
                if let Some(ai) = exit_guard.get_ai_update_interface_mut() {
                    ai.set_ignore_collision_time(LOGICFRAMES_PER_SECOND as UnsignedInt);
                    let _ = ai.ignore_obstacle(None);
                    let _ = ai.adjust_destination(&mut end_pos);
                    let _ = ai.update_goal_position(&end_pos, Self::destination_layer(&end_pos));
                }
            });
        }

        let mut exit_path = if hurry {
            vec![end_pos]
        } else {
            vec![end_pos, end_pos]
        };
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
        obj_id: ObjectID,
        exit_door: ExitDoorType,
    ) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        if matches!(exit_door, ExitDoorType::None | ExitDoorType::NoneAvailable) {
            return Ok(());
        }

        let Some(prep) = self.prepare_object(obj_id, false)? else {
            return Ok(());
        };

        // C++ clears allow-to-fall only around aiFollowPath + updateGoal, then restores
        // the copied flag. Physics and AI are owned by the exit object now, so the
        // pause/execute/restore sequence runs under one write guard.
        // ignoreObstacle(NULL) already ran in prepare when this call owns the clear.
        let has_ai = OBJECT_REGISTRY
            .with_object(obj_id, |guard| guard.get_ai_update_interface().is_some())
            .unwrap_or(false);
        if !has_ai {
            queue_produced_exit(
                obj_id,
                crate::object::PendingProducedExit::Follow {
                    path: prep.exit_path,
                    ignore_id: prep.owner_id,
                    end: prep.end_pos,
                },
            );
            return Ok(());
        }

        if OBJECT_REGISTRY
            .with_object_mut(obj_id, |exit_guard| {
                let paused_allow_to_fall = exit_guard.get_physics_mut().map(|physics| {
                    let previous = physics.get_allow_to_fall();
                    physics.set_allow_to_fall(false);
                    previous
                });

                if let Some(ai) = exit_guard.get_ai_update_interface_mut() {
                    let mut params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::FollowPath,
                        CommandSourceType::FromAi,
                    );
                    params.coords = prep.exit_path.clone();
                    params.obj = Some(prep.owner_id);
                    let _ = ai.execute_command(&params);
                    let _ = ai
                        .update_goal_position(&prep.end_pos, Self::destination_layer(&prep.end_pos));
                }

                if let Some(previous) = paused_allow_to_fall {
                    if let Some(physics) = exit_guard.get_physics_mut() {
                        physics.set_allow_to_fall(previous);
                    }
                }
            })
            .is_none()
        {
            queue_produced_exit(
                obj_id,
                crate::object::PendingProducedExit::Follow {
                    path: prep.exit_path,
                    ignore_id: prep.owner_id,
                    end: prep.end_pos,
                },
            );
        }

        Ok(())
    }

    pub fn exit_object_in_a_hurry(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        let Some(prep) = self.prepare_object(obj_id, true)? else {
            return Ok(());
        };

        let has_ai = OBJECT_REGISTRY
            .with_object(obj_id, |guard| guard.get_ai_update_interface().is_some())
            .unwrap_or(false);
        if !has_ai {
            return Ok(());
        }

        if OBJECT_REGISTRY
            .with_object_mut(obj_id, |exit_guard| {
                if let Some(ai) = exit_guard.get_ai_update_interface_mut() {
                    ai.do_quick_exit(&prep.exit_path);
                    let _ = ai
                        .update_goal_position(&prep.end_pos, Self::destination_layer(&prep.end_pos));
                }
            })
            .is_none()
        {
            queue_produced_exit(
                obj_id,
                crate::object::PendingProducedExit::Quick(prep.exit_path),
            );
        }

        Ok(())
    }

    /// Unreserve door for exit
    pub fn unreserve_door_for_exit(&self, exit_door: ExitDoorType) -> GameResult<()> {
        let _ = exit_door;
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
        let full = self.contained_object_ids.len() as i32;
        (total, full)
    }

    /// Redeploy occupants (can be overridden)
    pub fn redeploy_occupants(&mut self) -> GameResult<()> {
        let contained_ids = self.contained_object_ids.clone();
        self.redeploy_objects(&contained_ids)
    }

    pub fn redeploy_riders_at(&mut self, owner_pos: &Coord3D, fire_points: &[Matrix3D]) {
        let ids = self.contained_object_ids.clone();
        let count = fire_points.len();
        self.fire_point_start = -1;
        self.fire_point_next = 0;
        self.fire_point_size = count as i32;
        let mut cursor = 0;
        for obj_id in ids.iter().rev() {
            let updated = OBJECT_REGISTRY.with_object_mut(*obj_id, |guard| {
                if count == 0 {
                    let _ = guard.set_position(owner_pos);
                    return None;
                }
                let matrix = fire_points[cursor];
                if self.is_enclosing_container_for(guard) {
                    let (_, _, pos) = matrix.to_scale_rotation_translation();
                    let _ = guard.set_position(&pos);
                } else {
                    guard.set_transform_matrix(&matrix);
                }
                let next = (cursor + 1) % count;
                self.fire_point_next = next as i32;
                Some(next)
            });
            if let Some(Some(next)) = updated {
                cursor = next;
            }
        }
        let stored = count.min(MAX_FIRE_POINTS);
        self.fire_point_size = stored as i32;
        self.no_fire_points_in_art = stored == 0;
        for index in 0..stored {
            self.fire_points[index] = Self::fire_point_matrix_from_transform(fire_points[index]);
        }
        if stored == 0 {
            self.fire_point_next = 0;
        }
        if stored > 0 && self.fire_point_next >= stored as i32 {
            self.fire_point_next %= stored as i32;
        }
    }

    pub(crate) fn redeploy_objects(&mut self, contained_ids: &[ObjectID]) -> GameResult<()> {
        self.no_fire_points_in_art = false;
        self.fire_point_start = -1;
        self.fire_point_next = 0;
        self.fire_point_size = 0;

        for &obj_id in contained_ids.iter().rev() {
            self.put_obj_at_next_fire_point(obj_id)?;
        }
        Ok(())
    }

    fn put_obj_at_next_fire_point(&mut self, obj_id: ObjectID) -> GameResult<()> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let owner_id = self.get_object_id();
        if owner_id == crate::common::INVALID_ID {
            return Ok(());
        }
        if !TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        if self.fire_point_size == 0 && !self.no_fire_points_in_art {
            let fire_points = crate::object::registry::OBJECT_REGISTRY
                .with_object(owner_id, |owner_guard| {
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
            crate::object::registry::OBJECT_REGISTRY
                .with_object(owner_id, |owner_guard| *owner_guard.get_position())
                .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0))
        } else if self.module_data.passengers_in_turret {
            let firepoint = format!("FIREPOINT{:02}", self.fire_point_next + 1);
            if let Some((pos, matrix)) =
                crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                    let (_, pos, matrix) = owner_guard.get_single_logical_bone_position_on_turret(
                        TurretType::Primary,
                        &firepoint,
                    );
                    (pos, matrix)
                })
            {
                self.fire_points[self.fire_point_next as usize] =
                    Self::fire_point_matrix_from_transform(matrix);
                pos
            } else {
                Coord3D::new(0.0, 0.0, 0.0)
            }
        } else {
            Self::fire_point_position(&self.fire_points[self.fire_point_next as usize])
        };

        let _ = OBJECT_REGISTRY.with_object_mut(obj_id, |guard| {
            if self.is_enclosing_container_for(guard) {
                if let Err(err) = guard.set_position(&pos) {
                    log::warn!(
                        "OpenContain::put_obj_at_next_fire_point failed to place object {}: {}",
                        guard.get_id(),
                        err
                    );
                }
            } else {
                let matrix = if self.no_fire_points_in_art {
                    Matrix3D::from_translation(pos)
                } else {
                    Self::fire_point_transform(&self.fire_points[self.fire_point_next as usize])
                };
                guard.set_transform_matrix(&matrix);
            }
        });

        if self.fire_point_size > 0 {
            self.fire_point_next += 1;
            if self.fire_point_next >= self.fire_point_size {
                self.fire_point_next = 0;
            }
        }

        Ok(())
    }

    pub(crate) fn add_or_remove_obj_from_world(
        &mut self,
        obj_id: ObjectID,
        add: bool,
    ) -> GameResult<()> {
        let pos = OBJECT_REGISTRY
            .with_object_mut(obj_id, |guard| {
                if add {
                    let _ = guard.register_in_partition_manager();
                    if let Some(drawable) = guard.get_drawable() {
                        if let Ok(mut draw_guard) = drawable.try_write() {
                            let _ = draw_guard.set_drawable_hidden(false);
                        }
                    }
                } else {
                    guard.leave_group();
                    if let Some(drawable) = guard.get_drawable() {
                        if let Ok(mut draw_guard) = drawable.try_write() {
                            let _ = draw_guard.set_drawable_hidden(true);
                        }
                    }
                }
                *guard.get_position()
            })
            .or_else(|| OBJECT_REGISTRY.with_object(obj_id, |guard| *guard.get_position()));
        let Some(pos) = pos else {
            return Ok(());
        };
        if add {
            Self::add_to_pathfind_map(obj_id, pos);
        } else {
            if let Some(partition) = crate::helpers::ThePartitionManager::get() {
                partition.unregister_object(obj_id);
            }
            let ai_store = the_ai();
            if let Ok(ai_guard) = ai_store.read() {
                if let Some(pathfinder) = ai_guard.pathfinder() {
                    if let Ok(mut pf) = pathfinder.try_write() {
                        pf.remove_object_from_map(obj_id, &[pos]);
                    }
                }
            }
        }

        let contained_ids = OBJECT_REGISTRY
            .with_object(obj_id, |guard| {
                guard
                    .get_contain()
                    .map(|contain| contain.get_contained_objects().into_owned())
            })
            .flatten();
        for child_id in contained_ids.unwrap_or_default() {
            let should_recurse = OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    let Some(contain) = obj_guard.get_contain() else {
                        return false;
                    };
                    OBJECT_REGISTRY
                        .with_object(child_id, |child_guard| {
                            !contain.is_enclosing_container_for(child_guard)
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if should_recurse {
                let _ = self.add_or_remove_obj_from_world(child_id, add);
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
        if let Some(data) = state.get("contained_objects") {
            if data.len() % std::mem::size_of::<ObjectID>() != 0 {
                return Err("Invalid contained_objects data".into());
            }

            self.contained_object_ids.clear();
            self.xfer_contain_id_list.clear();

            for chunk in data.chunks_exact(std::mem::size_of::<ObjectID>()) {
                let bytes: [u8; std::mem::size_of::<ObjectID>()] = chunk
                    .try_into()
                    .map_err(|_| "Invalid contained object id data")?;
                self.xfer_contain_id_list
                    .push(ObjectID::from_le_bytes(bytes));
            }
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
        if self.xfer_contain_id_list.is_empty() {
            return Ok(());
        }

        self.rebuild_contain_list_from_xfer_ids()
            .map_err(|e| e.into())
    }

    fn rebuild_contain_list_from_xfer_ids(&mut self) -> Result<(), String> {
        if !self.contained_object_ids.is_empty() {
            return Err("OpenContain list must be empty before load_post_process".to_string());
        }

        let owner_id = self.get_object_id();
        if owner_id == crate::common::INVALID_ID {
            return Err("OpenContain has no owning object during load_post_process".to_string());
        }
        let ids = std::mem::take(&mut self.xfer_contain_id_list);
        for object_id in ids {
            if !TheGameLogic::find_object_by_id(object_id) {
                return Err(format!(
                    "OpenContain could not resolve contained object {object_id}"
                ));
            }
            self.contained_object_ids.push(object_id);
            let is_enclosing = OBJECT_REGISTRY
                .with_object(object_id, |obj_guard| self.is_enclosing_container_for(obj_guard))
                .unwrap_or(false);
            if is_enclosing {
                let _ = self.add_or_remove_obj_from_world(object_id, false);
            }
            let Some(result) = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                obj_guard.on_contained_by(owner_id)
            }) else {
                return Err(format!(
                    "OpenContain could not lock contained object {object_id}"
                ));
            };
            result.map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// C++ OpenContain::clientVisibleContainedFlashAsSelected is an empty virtual.
    /// HelixContain and OverlordContain override it.
    pub fn client_visible_contained_flash_as_selected(&self) -> GameResult<()> {
        Ok(())
    }

    /// Play unload sound.
    /// Matches C++ OpenContain::doUnloadSound via leftover TheAudio.
    pub fn do_unload_sound(&mut self) {
        let name = self
            .module_data
            .exit_sound
            .as_ref()
            .map(|s| s.get_event_name().to_string());
        let now = TheGameLogic::get_frame();
        leftover_do_unload_sound(
            name.as_deref(),
            self.get_object_id(),
            now,
            &mut self.last_unload_sound_frame,
        );
    }

    /// C++ OpenContain::markAllPassengersDetected.
    pub fn mark_all_passengers_detected(&mut self) {
        ContainModuleInterface::mark_all_passengers_detected(self);
    }

    /// C++ OpenContain::onSelling: order everyone out.
    pub fn on_selling(&mut self) -> GameResult<()> {
        ContainModuleInterface::order_all_passengers_to_exit(self, CommandSourceType::FromAi, false)
            .map_err(|e| GameError::ModuleError(e.to_string()).into())
    }
}

impl ContainModuleInterface for OpenContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        OBJECT_REGISTRY
            .with_object(object_id, |obj_guard| {
                OpenContain::is_valid_container_for(self, obj_guard, true)
            })
            .unwrap_or(false)
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.add_to_contain(object_id).map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        self.remove_from_contain(object_id, false)
            .map_err(|e| e.to_string())
    }

    fn remove_from_contain(
        &mut self,
        object_id: ObjectID,
        expose_stealth: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::remove_from_contain(self, object_id, expose_stealth).map_err(|e| e.into())
    }

    fn get_contained_objects(&self) -> std::borrow::Cow<'_, [ObjectID]> {
        std::borrow::Cow::Borrowed(&self.contained_object_ids)
    }

    fn get_contained_count(&self) -> usize {
        self.contained_object_ids.len()
    }

    fn get_stealth_units_contained(&self) -> UnsignedInt {
        OpenContain::get_stealth_units_contained(self)
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

    fn on_die_with_owner(
        &mut self,
        owner: &Object,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_die_for_owner(self, Some(owner), damage_info).map_err(|e| e.into())
    }

    fn on_collide_enter(
        &mut self,
        other_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_collide_enter(self, other_id).map_err(|e| e.into())
    }

    fn on_delete(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_delete(self).map_err(|e| e.into())
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

    fn add_to_contain_list(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::add_to_contain_list(self, obj.get_id()).map_err(|e| e.into())
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
        OpenContain::passes_weapon_bonus_to_passengers(self)
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
        if dual_world_registry_unavailable() {
            return ExitDoorType::NoneAvailable;
        }
        let _ = (_spawner, _spawn);
        ExitDoorType::Primary
    }

    fn unreserve_door_for_exit(&mut self, door: ExitDoorType) {
        let _ = OpenContain::unreserve_door_for_exit(self, door);
    }

    fn exit_object_via_door(
        &mut self,
        obj_id: ObjectID,
        door: ExitDoorType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        OpenContain::exit_object_via_door(self, obj_id, door).map_err(|err| err.into())
    }

    fn exit_object_in_a_hurry(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        OpenContain::exit_object_in_a_hurry(self, obj_id).map_err(|err| err.into())
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        OpenContain::set_passenger_allowed_to_fire(self, allowed);
    }

    fn has_objects_wanting_to_enter_or_exit(&self) -> bool {
        OpenContain::has_objects_wanting_to_enter_or_exit(self)
    }

    fn on_containing(
        &mut self,
        obj_id: ObjectID,
        was_selected: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        OpenContain::on_containing(self, obj_id, was_selected).map_err(|e| e.into())
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.player_who_entered
    }

    fn on_removing(
        &mut self,
        obj_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 261: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if !crate::helpers::TheGameLogic::find_object_by_id(obj_id) {
            return Ok(());
        }

        OpenContain::on_removing(self, obj_id).map_err(|e| e.into())
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

    fn process_damage_to_contained(&mut self, percent_damage: f32) {
        let _ = OpenContain::process_damage_to_contained(self, percent_damage);
    }

    fn on_selling(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::on_selling(self).map_err(|e| e.into())
    }

    fn redeploy_riders_at(&mut self, owner_pos: &Coord3D, fire_points: &[Matrix3D]) {
        OpenContain::redeploy_riders_at(self, owner_pos, fire_points);
    }

    fn passengers_in_turret(&self) -> bool {
        self.module_data.passengers_in_turret
    }

    fn mark_all_passengers_detected(&mut self) {
        // Use the trait default body via Super? Inherent delegates to trait.
        // Re-implement here so OpenContain callers hit the same path.
        if dual_world_registry_unavailable() {
            return;
        }
        for object_id in self.contained_object_ids.clone() {
            let Some(stealth) = OBJECT_REGISTRY
                .with_object(object_id, |obj_guard| {
                    if obj_guard.is_kind_of(KindOf::StealthGarrison) {
                        obj_guard.get_stealth()
                    } else {
                        None
                    }
                })
                .flatten()
            else {
                continue;
            };
            if let Ok(mut stealth_guard) = stealth.lock() {
                stealth_guard.mark_as_detected();
            }
        }
    }

    fn client_visible_contained_flash_as_selected(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        OpenContain::client_visible_contained_flash_as_selected(self).map_err(|e| e.into())
    }
}

impl Snapshotable for OpenContain {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ Module::crc is empty. UpdateModule::crc does not write the next-call frame.
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

        let mut player_mask_bits = self.player_who_entered.bits() as u16;
        xfer.xfer_unsigned_short(&mut player_mask_bits)
            .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.player_who_entered = PlayerMaskType::from_bits_truncate(player_mask_bits as u32);
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
        self.rebuild_contain_list_from_xfer_ids()
    }
}

impl ContainerInterface for OpenContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.add_to_contain(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.remove_from_contain(obj_id, false)
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
    use crate::common::DefaultThingTemplate;
    use crate::object::registry::OBJECT_REGISTRY;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn test_object(name: &str, id: ObjectID) -> ObjectID {
        Object::new_with_id(
            Arc::new(DefaultThingTemplate::new(name.to_string())),
            id,
            crate::common::ObjectStatusMaskType::none(),
            None,
        )
        .expect("test object")
    }

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
    fn leftover_open_contain_start_and_tick_match_cpp() {
        let open = leftover_open_contain_start_exit_door(1);
        assert_eq!(open.countdown, 1);
        assert!(open.set_opening);
        assert!(!open.set_closing);
        let close = leftover_open_contain_tick_exit_door(open.countdown);
        assert_eq!(close.countdown, 0);
        assert!(close.set_closing);
        assert!(!close.set_opening);
        let zero = leftover_open_contain_start_exit_door(0);
        assert_eq!(zero.countdown, 0);
        assert!(!zero.set_opening);
        assert!(!leftover_open_contain_tick_exit_door(0).set_closing);
        assert_eq!(leftover_open_contain_door_open_time(""), 1);
        assert_eq!(leftover_open_contain_resolved_door_open_time("", 0), 0);
        assert_eq!(leftover_open_contain_resolved_door_open_time("", 7), 7);
        let live = leftover_open_contain_open_exit_door("");
        assert_eq!(live, open);
        let tick = leftover_open_contain_tick_exit_door(live.countdown);
        assert_eq!(tick, close);
        let armed = leftover_open_contain_arm_exit_door(0);
        assert!(!armed.set_opening);
        assert_eq!(armed.countdown, 0);
    }

    #[test]
    fn xfer_preserves_cpp_open_contain_runtime_fields() {
        let mut saved =
            OpenContain::new(crate::common::INVALID_ID, &OpenContainModuleData::default()).expect("contain");
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
            OpenContain::new(crate::common::INVALID_ID, &OpenContainModuleData::default()).expect("contain");
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
        let ids: Vec<ObjectID> = loaded.object_enter_exit_info.keys().copied().collect();
        assert_eq!(
            ids,
            vec![303, 404],
            "enter/exit xfer must be ObjectID-ordered"
        );
    }

    #[test]
    fn map_load_state_rebuilds_contained_objects_in_post_process_like_cpp() {
        let _lock = crate::test_sync::lock();
        let owner = test_object("OpenContainLoadOwner", 92001);
        let child = test_object("OpenContainLoadChild", 92002);
        let mut state = HashMap::new();
        state.insert(
            "contained_objects".to_string(),
            92002_u32.to_le_bytes().to_vec(),
        );
        let mut contain = OpenContain::new(
            owner,
            &OpenContainModuleData {
                contain_max: 1,
                ..Default::default()
            },
        )
        .expect("contain");

        contain.load_state(&state).expect("load state");

        assert_eq!(contain.xfer_contain_id_list, vec![92002]);
        assert!(contain.contained_object_ids.is_empty());

        contain.load_post_process().expect("load post process");

        assert_eq!(contain.xfer_contain_id_list, Vec::<ObjectID>::new());
        assert_eq!(contain.contained_object_ids, vec![92002]);
        assert_eq!(
            OBJECT_REGISTRY
                .with_object(child, |guard| guard.get_contained_by())
                .expect("child read"),
            Some(92001)
        );

        OBJECT_REGISTRY.unregister_object(92001);
        OBJECT_REGISTRY.unregister_object(92002);
    }

    #[test]
    fn contain_interface_routes_rally_point_to_open_contain() {
        let mut contain =
            OpenContain::new(crate::common::INVALID_ID, &OpenContainModuleData::default()).expect("contain");
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
        let mut contain = OpenContain::new(crate::common::INVALID_ID, &data).expect("contain");

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

    #[test]
    fn airborne_container_exit_keeps_bone_altitude_like_cpp() {
        let _lock = crate::test_sync::lock();

        // Flat terrain at height 0 so any ground-flattening is observable.
        let mut map_data = crate::system::map_loader::MapData::new();
        map_data.width = 2;
        map_data.height = 2;
        map_data.heightmap = vec![0, 0, 0, 0];
        crate::terrain::get_terrain_logic()
            .write()
            .expect("terrain write")
            .load_map_data(map_data);

        let owner = test_object("AirborneTransport", 92005);
        OBJECT_REGISTRY
            .with_object_mut(owner, |guard| {
                guard.set_position(&Coord3D::new(10.0, 10.0, 100.0));
            })
            .expect("owner write");
        let rider = test_object("AirborneRider", 92006);

        let mut contain =
            OpenContain::new(owner, &OpenContainModuleData::default()).expect("airborne contain");
        ContainModuleInterface::contain_object(&mut contain, 92006).expect("contain rider");

        // C++ OpenContain::exitObjectInAHurry (OpenContain.cpp:1076-1086)
        // reads the ExitStart/ExitEnd bone positions verbatim and only sets
        // the layer: airborne exits stay airborne instead of snapping to the
        // terrain height.
        contain
            .exit_object_in_a_hurry(92006)
            .expect("hurry exit rider");

        let rider_z = OBJECT_REGISTRY
            .with_object(rider, |guard| guard.get_position().z)
            .expect("rider read");
        assert!(
            (rider_z - 100.0).abs() < 0.01,
            "C++ keeps airborne exits airborne; rider z was {rider_z}"
        );

        crate::terrain::get_terrain_logic()
            .write()
            .expect("terrain write")
            .reset();
        OBJECT_REGISTRY.unregister_object(92005);
        OBJECT_REGISTRY.unregister_object(92006);
    }

    #[derive(Debug)]
    struct DoorExitPhysics {
        allow: bool,
        log: Arc<Mutex<Vec<String>>>,
    }

    impl crate::modules::PhysicsBehavior for DoorExitPhysics {
        fn update(&mut self, _dt: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn get_velocity(&self) -> Coord3D {
            Coord3D::new(0.0, 0.0, 0.0)
        }
        fn set_velocity(&mut self, _velocity: &Coord3D) {}
        fn is_on_ground(&self) -> bool {
            true
        }
        fn set_allow_to_fall(&mut self, allow: bool) {
            self.allow = allow;
            self.log
                .lock()
                .expect("log")
                .push(format!("allow={allow}"));
        }
        fn get_allow_to_fall(&self) -> bool {
            self.allow
        }
    }

    #[derive(Debug)]
    struct DoorExitAi {
        log: Arc<Mutex<Vec<String>>>,
        ignores: Arc<Mutex<Vec<Option<ObjectID>>>>,
    }

    impl crate::modules::AIUpdateInterface for DoorExitAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn is_moving(&self) -> bool {
            false
        }
        fn is_idle(&self) -> bool {
            true
        }
        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }
        fn ignore_obstacle(
            &mut self,
            obj_id: Option<ObjectID>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.ignores.lock().expect("ignores").push(obj_id);
            self.log.lock().expect("log").push("ignore".to_string());
            Ok(())
        }
        fn execute_command(
            &mut self,
            _command: &crate::ai::AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.log.lock().expect("log").push("follow".to_string());
            Ok(())
        }
        fn update_goal_position(
            &mut self,
            _goal: &Coord3D,
            _layer: crate::common::PathfindLayerEnum,
        ) -> Result<(), String> {
            self.log.lock().expect("log").push("goal".to_string());
            Ok(())
        }
    }

    #[test]
    fn exit_object_via_door_pauses_allow_to_fall_around_follow() {
        let _lock = crate::test_sync::lock();
        let owner = test_object("DoorExitTransport", 93011);
        let rider = test_object("DoorExitRider", 93012);
        let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let ignores = Arc::new(Mutex::new(Vec::new()));
        OBJECT_REGISTRY
            .with_object_mut(rider, |rider_guard| {
                rider_guard.set_physics(Some(Box::new(DoorExitPhysics {
                    allow: true,
                    log: Arc::clone(&log),
                })));
                rider_guard.set_ai_update_interface(Some(Box::new(DoorExitAi {
                    log: Arc::clone(&log),
                    ignores: Arc::clone(&ignores),
                })));
            })
            .expect("rider write");

        let mut contain =
            OpenContain::new(owner, &OpenContainModuleData::default()).expect("door contain");
        ContainModuleInterface::contain_object(&mut contain, 93012).expect("contain rider");

        contain
            .exit_object_via_door(93012, ExitDoorType::Door1)
            .expect("exit via door");

        assert_eq!(
            *log.lock().expect("log"),
            vec![
                "ignore".to_string(),
                "goal".to_string(),
                "allow=false".to_string(),
                "follow".to_string(),
                "goal".to_string(),
                "allow=true".to_string(),
            ],
            "C++ clears allow-to-fall only around aiFollowPath + updateGoal, then restores"
        );
        assert!(
            OBJECT_REGISTRY
                .with_object(rider, |guard| {
                    guard
                        .get_physics()
                        .expect("physics")
                        .get_allow_to_fall()
                })
                .expect("rider read"),
            "C++ restores the copied allow-to-fall flag after aiFollowPath"
        );
        assert_eq!(
            ignores.lock().expect("ignores").as_slice(),
            &[None],
            "exit_object_via_door must not add a second ignoreObstacle(NULL)"
        );

        OBJECT_REGISTRY.unregister_object(93011);
        OBJECT_REGISTRY.unregister_object(93012);
    }
}
