//! C++ save/load residuals that must not change nested WorldSnapshot layout.
//!
//! Version 18 appends one positional tail so v1-v17 streams stay aligned.
//! Live host is the player path; leftover globals are restored alongside.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::system::radar::{
    get_radar_system, Coord3D, ICoord2D, RadarEvent, RadarEventType, MAX_RADAR_EVENTS,
    RGBAColorInt,
};
use game_engine::common::system::xfer::Xfer as CommonXfer;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use gamelogic::helpers::TheGameLogic;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Seek, Write};
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NamedTimerPersist {
    pub name: String,
    pub text: String,
    pub is_countdown: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SuperweaponDisplayPersist {
    pub player_index: i32,
    pub template_name: String,
    pub power_name: String,
    pub object_id: u32,
    pub timestamp: u32,
    pub hidden_by_script: bool,
    pub hidden_by_science: bool,
    pub ready: bool,
    pub eva_ready_played: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScriptCounterPersist {
    pub name: String,
    pub value: i32,
    pub is_countdown_timer: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScriptFlagPersist {
    pub name: String,
    pub value: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ScriptActivePersist {
    pub name: String,
    pub is_group: bool,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WaterUpdatePersist {
    pub trigger_id: i32,
    pub change_per_frame: f32,
    pub target_height: f32,
    pub damage_amount: f32,
    pub current_height: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RadarEventPersist {
    pub event_type: u8,
    pub active: bool,
    pub create_frame: u32,
    pub die_frame: u32,
    pub fade_frame: u32,
    pub color1: [u8; 4],
    pub color2: [u8; 4],
    pub world_loc: [f32; 3],
    pub radar_loc: [i32; 2],
    pub sound_played: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DrawableXferPersist {
    pub object_id: u32,
    pub selection_flash_remaining: u32,
    pub decal_opacity_fade_target: f32,
    pub decal_opacity_fade_rate: f32,
    pub decal_opacity: f32,
    pub explicit_opacity: f32,
    pub drawable_status: u32,
    pub tint_status: u32,
    pub prev_tint_status: u32,
    pub fade_mode: u8,
    pub time_elapsed_fade: u32,
    pub time_to_fade: u32,
    pub has_loco_info: bool,
    pub loco_pitch: f32,
    pub loco_pitch_rate: f32,
    pub loco_roll: f32,
    pub loco_roll_rate: f32,
    pub loco_yaw: f32,
    pub loco_accel_pitch: f32,
    pub loco_accel_pitch_rate: f32,
    pub loco_accel_roll: f32,
    pub loco_accel_roll_rate: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldPersistV18 {
    pub rank_level_limit: i32,
    pub buildable_overrides: Vec<(String, i32)>,
    pub show_behind_building_markers: bool,
    pub draw_icon_ui: bool,
    pub show_dynamic_lod: bool,
    pub script_hulk_max_lifetime_override: i32,
    pub control_bar_overrides: Vec<(String, String)>,
    pub rank_points_to_add_at_game_start: i32,
    pub named_timer_last_flash_frame: i32,
    pub named_timer_used_flash_color: bool,
    pub named_timer_display_shown: bool,
    pub named_timers: Vec<NamedTimerPersist>,
    pub superweapon_hidden_by_script: bool,
    pub superweapon_entries: Vec<SuperweaponDisplayPersist>,
    pub superweapon_hidden_objects: Vec<u32>,
    pub script_counters: Vec<ScriptCounterPersist>,
    pub script_flags: Vec<ScriptFlagPersist>,
    pub script_actives: Vec<ScriptActivePersist>,
    pub terrain_active_boundary: i32,
    pub water_updates: Vec<WaterUpdatePersist>,
    pub radar_hidden: bool,
    pub radar_forced: bool,
    pub radar_events: Vec<RadarEventPersist>,
    pub radar_next_event: i32,
    pub radar_last_event: i32,
    pub camera_valid: bool,
    pub camera_angle: f32,
    pub camera_position: [f32; 3],
    pub camera_target: [f32; 3],
    pub camera_zoom: f32,
    pub drawable_xfer: Vec<DrawableXferPersist>,
}

impl Default for WorldPersistV18 {
    fn default() -> Self {
        Self {
            rank_level_limit: 1000,
            buildable_overrides: Vec::new(),
            show_behind_building_markers: true,
            draw_icon_ui: true,
            show_dynamic_lod: true,
            script_hulk_max_lifetime_override: -1,
            control_bar_overrides: Vec::new(),
            rank_points_to_add_at_game_start: 0,
            named_timer_last_flash_frame: 0,
            named_timer_used_flash_color: false,
            named_timer_display_shown: true,
            named_timers: Vec::new(),
            superweapon_hidden_by_script: false,
            superweapon_entries: Vec::new(),
            superweapon_hidden_objects: Vec::new(),
            script_counters: Vec::new(),
            script_flags: Vec::new(),
            script_actives: Vec::new(),
            terrain_active_boundary: 0,
            water_updates: Vec::new(),
            radar_hidden: false,
            radar_forced: false,
            radar_events: Vec::new(),
            radar_next_event: 0,
            radar_last_event: -1,
            camera_valid: false,
            camera_angle: 0.0,
            camera_position: [0.0; 3],
            camera_target: [0.0; 3],
            camera_zoom: 1.0,
            drawable_xfer: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CameraPersist {
    pub angle: f32,
    pub position: [f32; 3],
    pub target: [f32; 3],
    pub zoom: f32,
}

static PENDING_CAMERA: Mutex<Option<CameraPersist>> = Mutex::new(None);

pub fn set_pending_camera(camera: CameraPersist) {
    if let Ok(mut slot) = PENDING_CAMERA.lock() {
        *slot = Some(camera);
    }
}

pub fn take_pending_camera() -> Option<CameraPersist> {
    PENDING_CAMERA.lock().ok().and_then(|mut slot| slot.take())
}

pub fn peek_pending_camera() -> Option<CameraPersist> {
    PENDING_CAMERA.lock().ok().and_then(|slot| *slot)
}

fn radar_event_type_from_u8(value: u8) -> RadarEventType {
    match value {
        1 => RadarEventType::Construction,
        2 => RadarEventType::Upgrade,
        3 => RadarEventType::UnderAttack,
        4 => RadarEventType::Information,
        5 => RadarEventType::BeaconPulse,
        6 => RadarEventType::Infiltration,
        7 => RadarEventType::BattlePlan,
        8 => RadarEventType::StealthDiscovered,
        9 => RadarEventType::StealthNeutralized,
        10 => RadarEventType::Fake,
        _ => RadarEventType::Invalid,
    }
}

fn persist_radar_event(event: &RadarEvent) -> RadarEventPersist {
    RadarEventPersist {
        event_type: event.event_type as u8,
        active: event.active,
        create_frame: event.create_frame,
        die_frame: event.die_frame,
        fade_frame: event.fade_frame,
        color1: [event.color1.r, event.color1.g, event.color1.b, event.color1.a],
        color2: [event.color2.r, event.color2.g, event.color2.b, event.color2.a],
        world_loc: [event.world_loc.x, event.world_loc.y, event.world_loc.z],
        radar_loc: [event.radar_loc.x, event.radar_loc.y],
        sound_played: event.sound_played,
    }
}

fn restore_radar_event(entry: &RadarEventPersist) -> RadarEvent {
    RadarEvent {
        event_type: radar_event_type_from_u8(entry.event_type),
        active: entry.active,
        create_frame: entry.create_frame,
        die_frame: entry.die_frame,
        fade_frame: entry.fade_frame,
        color1: RGBAColorInt::new(entry.color1[0], entry.color1[1], entry.color1[2], entry.color1[3]),
        color2: RGBAColorInt::new(entry.color2[0], entry.color2[1], entry.color2[2], entry.color2[3]),
        world_loc: Coord3D::new(entry.world_loc[0], entry.world_loc[1], entry.world_loc[2]),
        radar_loc: ICoord2D::new(entry.radar_loc[0], entry.radar_loc[1]),
        sound_played: entry.sound_played,
    }
}

pub fn capture_persist_v18(game_logic: &GameLogic) -> WorldPersistV18 {
    let mut persist = WorldPersistV18::default();

    persist.rank_level_limit = TheGameLogic::get_rank_level_limit();
    persist.show_behind_building_markers = TheGameLogic::get_show_behind_building_markers();
    persist.draw_icon_ui = TheGameLogic::get_draw_icon_ui();
    persist.show_dynamic_lod = TheGameLogic::get_show_dynamic_lod();
    persist.script_hulk_max_lifetime_override = TheGameLogic::get_hulk_max_lifetime_override();
    persist.rank_points_to_add_at_game_start = TheGameLogic::get_rank_points_to_add_at_game_start();
    if let Ok(leftover) = gamelogic::system::game_logic::get_game_logic().lock() {
        persist.buildable_overrides = leftover.snapshot_buildable_status_overrides();
        persist.control_bar_overrides = leftover
            .snapshot_control_bar_overrides_raw()
            .into_iter()
            .map(|(key, value)| (key, value.unwrap_or_default()))
            .collect();
    }

    persist.named_timer_display_shown = game_logic.peek_script_named_timer_display_shown();
    persist.named_timers = {
        let mut timers: Vec<NamedTimerPersist> = game_logic
            .peek_script_named_timers()
            .iter()
            .map(|(name, (text, countdown))| NamedTimerPersist {
                name: name.clone(),
                text: text.clone(),
                is_countdown: *countdown,
            })
            .collect();
        timers.sort_by(|a, b| a.name.cmp(&b.name));
        timers
    };
    persist.superweapon_hidden_by_script = !game_logic.peek_script_superweapon_display_enabled();
    persist.superweapon_hidden_objects = {
        let mut ids: Vec<u32> = game_logic
            .peek_script_superweapon_hidden_objects()
            .iter()
            .map(|id| id.0)
            .collect();
        ids.sort_unstable();
        ids
    };

    if let Some((counters, flags)) =
        gamelogic::scripting::engine::with_script_engine_ref(|engine| engine.snapshot_named_trackers())
    {
        persist.script_counters = counters
            .into_iter()
            .map(|(name, value, is_countdown_timer)| ScriptCounterPersist {
                name,
                value,
                is_countdown_timer,
            })
            .collect();
        persist.script_flags = flags
            .into_iter()
            .map(|(name, value)| ScriptFlagPersist { name, value })
            .collect();
    }
    persist.script_actives = game_logic
        .snapshot_script_actives()
        .into_iter()
        .map(|(name, is_group, is_active)| ScriptActivePersist {
            name,
            is_group,
            is_active,
        })
        .collect();

    if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() {
        persist.terrain_active_boundary = terrain.get_active_boundary();
        persist.water_updates = terrain
            .snapshot_dynamic_water_entries()
            .into_iter()
            .map(|entry| WaterUpdatePersist {
                trigger_id: entry.trigger_id,
                change_per_frame: entry.change_per_frame,
                target_height: entry.target_height,
                damage_amount: entry.damage_amount,
                current_height: entry.current_height,
            })
            .collect();
    }

    persist.radar_hidden = !game_logic.radar_script_enabled();
    persist.radar_forced = game_logic.radar_forced();
    if let Ok(radar) = get_radar_system().read() {
        let (hidden, forced, events, next, last) = radar.snapshot_persist_state();
        persist.radar_hidden = hidden;
        persist.radar_forced = forced;
        persist.radar_events = events.iter().map(persist_radar_event).collect();
        persist.radar_next_event = next as i32;
        persist.radar_last_event = last.map(|idx| idx as i32).unwrap_or(-1);
    }

    if let Some(camera) = peek_pending_camera() {
        persist.camera_valid = true;
        persist.camera_angle = camera.angle;
        persist.camera_position = camera.position;
        persist.camera_target = camera.target;
        persist.camera_zoom = camera.zoom;
    }

    let mut ids: Vec<ObjectId> = game_logic.host_objects().keys().copied().collect();
    ids.sort();
    for id in ids {
        let Some(object) = game_logic.host_object(id) else {
            continue;
        };
        let has_loco = object.loco_extra_2d_friction != 0.0
            || object.loco_preferred_height != 0.0
            || object.drawable_fade_mode != 0
            || object.selection_flash_remaining != 0
            || object.terrain_decal_fade_rate != 0.0
            || object.terrain_decal_opacity != 0.0
            || object.terrain_decal_fade_target != 0.0
            || object.drawable_hidden;
        if !has_loco
            && object.selection_flash_remaining == 0
            && object.drawable_fade_mode == 0
            && object.terrain_decal_fade_rate == 0.0
            && object.terrain_decal_opacity == 0.0
        {
            continue;
        }
        persist.drawable_xfer.push(DrawableXferPersist {
            object_id: id.0,
            selection_flash_remaining: object.selection_flash_remaining,
            decal_opacity_fade_target: object.terrain_decal_fade_target,
            decal_opacity_fade_rate: object.terrain_decal_fade_rate,
            decal_opacity: object.terrain_decal_opacity,
            explicit_opacity: 1.0,
            drawable_status: u32::from(object.drawable_hidden),
            tint_status: 0,
            prev_tint_status: 0,
            fade_mode: object.drawable_fade_mode,
            time_elapsed_fade: object
                .drawable_fade_start_frame
                .min(object.drawable_fade_frames),
            time_to_fade: object.drawable_fade_frames,
            has_loco_info: true,
            loco_pitch: 0.0,
            loco_pitch_rate: 0.0,
            loco_roll: 0.0,
            loco_roll_rate: 0.0,
            loco_yaw: 0.0,
            loco_accel_pitch: 0.0,
            loco_accel_pitch_rate: 0.0,
            loco_accel_roll: 0.0,
            loco_accel_roll_rate: 0.0,
        });
    }

    persist
}

pub fn restore_persist_v18(persist: &WorldPersistV18, game_logic: &mut GameLogic) {
    TheGameLogic::set_rank_level_limit(persist.rank_level_limit);
    TheGameLogic::set_show_behind_building_markers(persist.show_behind_building_markers);
    TheGameLogic::set_draw_icon_ui(persist.draw_icon_ui);
    TheGameLogic::set_show_dynamic_lod(persist.show_dynamic_lod);
    TheGameLogic::set_hulk_max_lifetime_override(persist.script_hulk_max_lifetime_override);
    TheGameLogic::set_rank_points_to_add_at_game_start(persist.rank_points_to_add_at_game_start);
    if let Ok(mut leftover) = gamelogic::system::game_logic::get_game_logic().lock() {
        leftover.set_rank_level_limit(persist.rank_level_limit);
        leftover.restore_buildable_status_overrides(persist.buildable_overrides.clone());
        leftover.restore_control_bar_overrides_raw(
            persist
                .control_bar_overrides
                .iter()
                .map(|(key, value)| {
                    (
                        key.clone(),
                        if value.is_empty() {
                            None
                        } else {
                            Some(value.clone())
                        },
                    )
                })
                .collect(),
        );
    }

    game_logic.restore_script_named_timers(persist.named_timers.iter().map(|timer| {
        (
            timer.name.clone(),
            timer.text.clone(),
            timer.is_countdown,
        )
    }));
    game_logic.restore_script_named_timer_display_shown(persist.named_timer_display_shown);
    game_logic.restore_script_superweapon_display_enabled(!persist.superweapon_hidden_by_script);
    game_logic.restore_script_superweapon_hidden_objects(
        persist
            .superweapon_hidden_objects
            .iter()
            .copied()
            .map(ObjectId),
    );

    let _ = gamelogic::scripting::engine::initialize_script_engine();
    let counters: Vec<(String, i32, bool)> = persist
        .script_counters
        .iter()
        .map(|c| (c.name.clone(), c.value, c.is_countdown_timer))
        .collect();
    let flags: Vec<(String, bool)> = persist
        .script_flags
        .iter()
        .map(|f| (f.name.clone(), f.value))
        .collect();
    let _ = gamelogic::scripting::engine::with_script_engine_mut(|engine| {
        engine.restore_named_trackers(&counters, &flags)
    });
    game_logic.restore_script_actives(
        &persist
            .script_actives
            .iter()
            .map(|entry| (entry.name.clone(), entry.is_group, entry.is_active))
            .collect::<Vec<_>>(),
    );

    if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
        terrain.set_active_boundary(persist.terrain_active_boundary);
        let entries = persist
            .water_updates
            .iter()
            .map(|entry| gamelogic::terrain::TerrainDynamicWaterSnapshotEntry {
                trigger_id: entry.trigger_id,
                water_name: AsciiString::new(),
                change_per_frame: entry.change_per_frame,
                target_height: entry.target_height,
                damage_amount: entry.damage_amount,
                current_height: entry.current_height,
            })
            .collect();
        let _ = terrain.restore_dynamic_water_entries(entries);
    }

    game_logic.restore_radar_script_state(!persist.radar_hidden, persist.radar_forced);
    if let Ok(mut radar) = get_radar_system().write() {
        let mut events = std::array::from_fn(|_| RadarEvent::default());
        for (idx, entry) in persist.radar_events.iter().take(MAX_RADAR_EVENTS).enumerate() {
            events[idx] = restore_radar_event(entry);
        }
        radar.restore_persist_state(
            persist.radar_hidden,
            persist.radar_forced,
            events,
            persist.radar_next_event.max(0) as usize,
            if persist.radar_last_event >= 0 {
                Some(persist.radar_last_event as usize)
            } else {
                None
            },
        );
    }

    if persist.camera_valid {
        set_pending_camera(CameraPersist {
            angle: persist.camera_angle,
            position: persist.camera_position,
            target: persist.camera_target,
            zoom: persist.camera_zoom,
        });
    }

    for entry in &persist.drawable_xfer {
        let Some(object) = game_logic.host_object_mut(ObjectId(entry.object_id)) else {
            continue;
        };
        object.selection_flash_remaining = entry.selection_flash_remaining;
        object.terrain_decal_fade_target = entry.decal_opacity_fade_target;
        object.terrain_decal_fade_rate = entry.decal_opacity_fade_rate;
        object.terrain_decal_opacity = entry.decal_opacity;
        object.drawable_hidden = entry.drawable_status != 0;
        object.drawable_fade_mode = entry.fade_mode;
        object.drawable_fade_frames = entry.time_to_fade;
        object.drawable_fade_start_frame = entry.time_elapsed_fade;
    }
}

fn map_xfer<T>(result: std::io::Result<T>) -> SaveLoadResult<T> {
    result.map_err(|e| SaveLoadError::Serialization(e.to_string()))
}

pub fn write_ingame_ui_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    persist: &WorldPersistV18,
) -> SaveLoadResult<()> {
    let mut version = 3u8;
    map_xfer(xfer.xfer_version(&mut version, 3))?;
    let mut last_flash = persist.named_timer_last_flash_frame;
    let mut used_flash = persist.named_timer_used_flash_color;
    let mut show = persist.named_timer_display_shown;
    map_xfer(xfer.xfer_int(&mut last_flash))?;
    map_xfer(xfer.xfer_bool(&mut used_flash))?;
    map_xfer(xfer.xfer_bool(&mut show))?;
    let mut timer_count = persist.named_timers.len() as i32;
    map_xfer(xfer.xfer_int(&mut timer_count))?;
    for timer in &persist.named_timers {
        let mut name = timer.name.clone();
        let mut text = timer.text.clone();
        let mut countdown = timer.is_countdown;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_unicode_string(&mut text))?;
        map_xfer(xfer.xfer_bool(&mut countdown))?;
    }
    let mut hidden = persist.superweapon_hidden_by_script;
    map_xfer(xfer.xfer_bool(&mut hidden))?;
    for entry in &persist.superweapon_entries {
        let mut player_index = entry.player_index;
        let mut template_name = entry.template_name.clone();
        let mut power_name = entry.power_name.clone();
        let mut object_id = entry.object_id;
        let mut timestamp = entry.timestamp;
        let mut hidden_by_script = entry.hidden_by_script;
        let mut hidden_by_science = entry.hidden_by_science;
        let mut ready = entry.ready;
        let mut eva = entry.eva_ready_played;
        map_xfer(xfer.xfer_int(&mut player_index))?;
        map_xfer(xfer.xfer_ascii_string(&mut template_name))?;
        map_xfer(xfer.xfer_ascii_string(&mut power_name))?;
        map_xfer(xfer.xfer_unsigned_int(&mut object_id))?;
        map_xfer(xfer.xfer_unsigned_int(&mut timestamp))?;
        map_xfer(xfer.xfer_bool(&mut hidden_by_script))?;
        map_xfer(xfer.xfer_bool(&mut hidden_by_science))?;
        map_xfer(xfer.xfer_bool(&mut ready))?;
        map_xfer(xfer.xfer_bool(&mut eva))?;
    }
    let mut sentinel = -1i32;
    map_xfer(xfer.xfer_int(&mut sentinel))?;
    Ok(())
}

pub fn parse_ingame_ui_block(payload: &[u8]) -> SaveLoadResult<WorldPersistV18> {
    let mut persist = WorldPersistV18::default();
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), 1);
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, 3))?;
    if version >= 2 {
        map_xfer(xfer.xfer_int(&mut persist.named_timer_last_flash_frame))?;
        map_xfer(xfer.xfer_bool(&mut persist.named_timer_used_flash_color))?;
        map_xfer(xfer.xfer_bool(&mut persist.named_timer_display_shown))?;
        let mut timer_count = 0i32;
        map_xfer(xfer.xfer_int(&mut timer_count))?;
        for _ in 0..timer_count.max(0) {
            let mut name = String::new();
            let mut text = String::new();
            let mut countdown = false;
            map_xfer(xfer.xfer_ascii_string(&mut name))?;
            map_xfer(xfer.xfer_unicode_string(&mut text))?;
            map_xfer(xfer.xfer_bool(&mut countdown))?;
            persist.named_timers.push(NamedTimerPersist {
                name,
                text,
                is_countdown: countdown,
            });
        }
    }
    map_xfer(xfer.xfer_bool(&mut persist.superweapon_hidden_by_script))?;
    loop {
        let mut player_index = 0i32;
        map_xfer(xfer.xfer_int(&mut player_index))?;
        if player_index == -1 {
            break;
        }
        let mut template_name = String::new();
        let mut power_name = String::new();
        let mut object_id = 0u32;
        let mut timestamp = 0u32;
        let mut hidden_by_script = false;
        let mut hidden_by_science = false;
        let mut ready = false;
        let mut eva = false;
        map_xfer(xfer.xfer_ascii_string(&mut template_name))?;
        map_xfer(xfer.xfer_ascii_string(&mut power_name))?;
        map_xfer(xfer.xfer_unsigned_int(&mut object_id))?;
        map_xfer(xfer.xfer_unsigned_int(&mut timestamp))?;
        map_xfer(xfer.xfer_bool(&mut hidden_by_script))?;
        map_xfer(xfer.xfer_bool(&mut hidden_by_science))?;
        map_xfer(xfer.xfer_bool(&mut ready))?;
        if version >= 3 {
            map_xfer(xfer.xfer_bool(&mut eva))?;
        } else {
            eva = ready;
        }
        persist.superweapon_entries.push(SuperweaponDisplayPersist {
            player_index,
            template_name,
            power_name,
            object_id,
            timestamp,
            hidden_by_script,
            hidden_by_science,
            ready,
            eva_ready_played: eva,
        });
    }
    Ok(persist)
}

pub fn write_tactical_view_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    persist: &WorldPersistV18,
) -> SaveLoadResult<()> {
    let mut version = 1u8;
    map_xfer(xfer.xfer_version(&mut version, 1))?;
    let mut angle = persist.camera_angle;
    let mut x = persist.camera_position[0];
    let mut y = persist.camera_position[1];
    let mut z = persist.camera_position[2];
    map_xfer(xfer.xfer_real(&mut angle))?;
    map_xfer(xfer.xfer_real(&mut x))?;
    map_xfer(xfer.xfer_real(&mut y))?;
    map_xfer(xfer.xfer_real(&mut z))?;
    Ok(())
}

pub fn parse_tactical_view_block(payload: &[u8]) -> SaveLoadResult<CameraPersist> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), 1);
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, 1))?;
    let mut angle = 0.0f32;
    let mut x = 0.0f32;
    let mut y = 0.0f32;
    let mut z = 0.0f32;
    map_xfer(xfer.xfer_real(&mut angle))?;
    map_xfer(xfer.xfer_real(&mut x))?;
    map_xfer(xfer.xfer_real(&mut y))?;
    map_xfer(xfer.xfer_real(&mut z))?;
    Ok(CameraPersist {
        angle,
        position: [x, y, z],
        target: [x, 0.0, z],
        zoom: 1.0,
    })
}

pub fn write_script_engine_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    persist: &WorldPersistV18,
) -> SaveLoadResult<()> {
    let mut version = 6u8;
    map_xfer(xfer.xfer_version(&mut version, 6))?;
    let mut sequential = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut sequential))?;
    let mut counters_size = persist.script_counters.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut counters_size))?;
    for counter in &persist.script_counters {
        let mut value = counter.value;
        let mut name = counter.name.clone();
        let mut countdown = counter.is_countdown_timer;
        map_xfer(xfer.xfer_int(&mut value))?;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_bool(&mut countdown))?;
    }
    let mut num_counters = persist.script_counters.len() as i32;
    map_xfer(xfer.xfer_int(&mut num_counters))?;
    let mut flags_size = persist.script_flags.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut flags_size))?;
    for flag in &persist.script_flags {
        let mut value = flag.value;
        let mut name = flag.name.clone();
        map_xfer(xfer.xfer_bool(&mut value))?;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
    }
    let mut num_flags = persist.script_flags.len() as i32;
    map_xfer(xfer.xfer_int(&mut num_flags))?;
    let mut attack_size = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut attack_size))?;
    let mut num_attack = 0i32;
    map_xfer(xfer.xfer_int(&mut num_attack))?;
    let mut object_priority = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut object_priority))?;
    let mut active_count = persist.script_actives.len() as i32;
    map_xfer(xfer.xfer_int(&mut active_count))?;
    for entry in &persist.script_actives {
        let mut name = entry.name.clone();
        let mut is_group = entry.is_group;
        let mut is_active = entry.is_active;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_bool(&mut is_group))?;
        map_xfer(xfer.xfer_bool(&mut is_active))?;
    }
    Ok(())
}

pub fn parse_script_engine_block(payload: &[u8]) -> SaveLoadResult<(Vec<ScriptCounterPersist>, Vec<ScriptFlagPersist>, Vec<ScriptActivePersist>)> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), 1);
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, 6))?;
    let mut sequential = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut sequential))?;
    let mut counters_size = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut counters_size))?;
    let mut counters = Vec::new();
    for _ in 0..counters_size {
        let mut value = 0i32;
        let mut name = String::new();
        let mut countdown = false;
        map_xfer(xfer.xfer_int(&mut value))?;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_bool(&mut countdown))?;
        if !name.is_empty() {
            counters.push(ScriptCounterPersist {
                name,
                value,
                is_countdown_timer: countdown,
            });
        }
    }
    let mut num_counters = 0i32;
    map_xfer(xfer.xfer_int(&mut num_counters))?;
    let mut flags_size = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut flags_size))?;
    let mut flags = Vec::new();
    for _ in 0..flags_size {
        let mut value = false;
        let mut name = String::new();
        map_xfer(xfer.xfer_bool(&mut value))?;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        if !name.is_empty() {
            flags.push(ScriptFlagPersist { name, value });
        }
    }
    let mut num_flags = 0i32;
    map_xfer(xfer.xfer_int(&mut num_flags))?;
    let mut attack_size = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut attack_size))?;
    let mut num_attack = 0i32;
    map_xfer(xfer.xfer_int(&mut num_attack))?;
    if version >= 6 {
        let mut object_priority = 0u16;
        map_xfer(xfer.xfer_unsigned_short(&mut object_priority))?;
    }
    let mut actives = Vec::new();
    let mut active_count = 0i32;
    if map_xfer(xfer.xfer_int(&mut active_count)).is_ok() {
        for _ in 0..active_count.max(0) {
            let mut name = String::new();
            let mut is_group = false;
            let mut is_active = true;
            map_xfer(xfer.xfer_ascii_string(&mut name))?;
            map_xfer(xfer.xfer_bool(&mut is_group))?;
            map_xfer(xfer.xfer_bool(&mut is_active))?;
            actives.push(ScriptActivePersist {
                name,
                is_group,
                is_active,
            });
        }
    }
    Ok((counters, flags, actives))
}

pub fn write_terrain_logic_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    persist: &WorldPersistV18,
) -> SaveLoadResult<()> {
    let mut version = 2u8;
    map_xfer(xfer.xfer_version(&mut version, 2))?;
    let mut boundary = persist.terrain_active_boundary;
    map_xfer(xfer.xfer_int(&mut boundary))?;
    let mut count = persist.water_updates.len() as i32;
    map_xfer(xfer.xfer_int(&mut count))?;
    for entry in &persist.water_updates {
        let mut trigger_id = entry.trigger_id;
        let mut change = entry.change_per_frame;
        let mut target = entry.target_height;
        let mut damage = entry.damage_amount;
        let mut current = entry.current_height;
        map_xfer(xfer.xfer_int(&mut trigger_id))?;
        map_xfer(xfer.xfer_real(&mut change))?;
        map_xfer(xfer.xfer_real(&mut target))?;
        map_xfer(xfer.xfer_real(&mut damage))?;
        map_xfer(xfer.xfer_real(&mut current))?;
    }
    Ok(())
}

pub fn parse_terrain_logic_block(payload: &[u8]) -> SaveLoadResult<(i32, Vec<WaterUpdatePersist>)> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), 1);
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, 2))?;
    let mut boundary = 0i32;
    map_xfer(xfer.xfer_int(&mut boundary))?;
    let mut entries = Vec::new();
    if version >= 2 {
        let mut count = 0i32;
        map_xfer(xfer.xfer_int(&mut count))?;
        for _ in 0..count.max(0) {
            let mut trigger_id = -1i32;
            let mut change = 0.0f32;
            let mut target = 0.0f32;
            let mut damage = 0.0f32;
            let mut current = 0.0f32;
            map_xfer(xfer.xfer_int(&mut trigger_id))?;
            map_xfer(xfer.xfer_real(&mut change))?;
            map_xfer(xfer.xfer_real(&mut target))?;
            map_xfer(xfer.xfer_real(&mut damage))?;
            map_xfer(xfer.xfer_real(&mut current))?;
            entries.push(WaterUpdatePersist {
                trigger_id,
                change_per_frame: change,
                target_height: target,
                damage_amount: damage,
                current_height: current,
            });
        }
    }
    Ok((boundary, entries))
}

pub fn write_radar_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    persist: &WorldPersistV18,
) -> SaveLoadResult<()> {
    let mut version = 1u8;
    map_xfer(xfer.xfer_version(&mut version, 1))?;
    let mut hidden = persist.radar_hidden;
    let mut forced = persist.radar_forced;
    map_xfer(xfer.xfer_bool(&mut hidden))?;
    map_xfer(xfer.xfer_bool(&mut forced))?;
    let mut local_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut local_count))?;
    let mut regular_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut regular_count))?;
    let mut event_count = MAX_RADAR_EVENTS as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut event_count))?;
    for i in 0..MAX_RADAR_EVENTS {
        let mut event = persist.radar_events.get(i).cloned().unwrap_or_default();
        let mut event_type = event.event_type as i32;
        map_xfer(xfer.xfer_int(&mut event_type))?;
        map_xfer(xfer.xfer_bool(&mut event.active))?;
        map_xfer(xfer.xfer_unsigned_int(&mut event.create_frame))?;
        map_xfer(xfer.xfer_unsigned_int(&mut event.die_frame))?;
        map_xfer(xfer.xfer_unsigned_int(&mut event.fade_frame))?;
        let mut c1 = u32::from_le_bytes(event.color1);
        let mut c2 = u32::from_le_bytes(event.color2);
        map_xfer(xfer.xfer_unsigned_int(&mut c1))?;
        map_xfer(xfer.xfer_unsigned_int(&mut c2))?;
        map_xfer(xfer.xfer_real(&mut event.world_loc[0]))?;
        map_xfer(xfer.xfer_real(&mut event.world_loc[1]))?;
        map_xfer(xfer.xfer_real(&mut event.world_loc[2]))?;
        map_xfer(xfer.xfer_int(&mut event.radar_loc[0]))?;
        map_xfer(xfer.xfer_int(&mut event.radar_loc[1]))?;
        map_xfer(xfer.xfer_bool(&mut event.sound_played))?;
    }
    let mut next = persist.radar_next_event;
    let mut last = persist.radar_last_event;
    map_xfer(xfer.xfer_int(&mut next))?;
    map_xfer(xfer.xfer_int(&mut last))?;
    Ok(())
}

pub fn parse_radar_block(
    payload: &[u8],
) -> SaveLoadResult<(bool, bool, Vec<RadarEventPersist>, i32, i32)> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), 1);
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, 1))?;
    let mut hidden = false;
    let mut forced = false;
    map_xfer(xfer.xfer_bool(&mut hidden))?;
    map_xfer(xfer.xfer_bool(&mut forced))?;
    let mut local_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut local_count))?;
    let mut regular_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut regular_count))?;
    let mut event_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut event_count))?;
    let mut events = Vec::new();
    for _ in 0..event_count {
        let mut event = RadarEventPersist::default();
        let mut event_type = 0i32;
        map_xfer(xfer.xfer_int(&mut event_type))?;
        event.event_type = event_type.max(0) as u8;
        map_xfer(xfer.xfer_bool(&mut event.active))?;
        map_xfer(xfer.xfer_unsigned_int(&mut event.create_frame))?;
        map_xfer(xfer.xfer_unsigned_int(&mut event.die_frame))?;
        map_xfer(xfer.xfer_unsigned_int(&mut event.fade_frame))?;
        let mut c1 = 0u32;
        let mut c2 = 0u32;
        map_xfer(xfer.xfer_unsigned_int(&mut c1))?;
        map_xfer(xfer.xfer_unsigned_int(&mut c2))?;
        event.color1 = c1.to_le_bytes();
        event.color2 = c2.to_le_bytes();
        map_xfer(xfer.xfer_real(&mut event.world_loc[0]))?;
        map_xfer(xfer.xfer_real(&mut event.world_loc[1]))?;
        map_xfer(xfer.xfer_real(&mut event.world_loc[2]))?;
        map_xfer(xfer.xfer_int(&mut event.radar_loc[0]))?;
        map_xfer(xfer.xfer_int(&mut event.radar_loc[1]))?;
        map_xfer(xfer.xfer_bool(&mut event.sound_played))?;
        events.push(event);
    }
    let mut next = 0i32;
    let mut last = -1i32;
    map_xfer(xfer.xfer_int(&mut next))?;
    map_xfer(xfer.xfer_int(&mut last))?;
    Ok((hidden, forced, events, next, last))
}

pub fn merge_chunk_persist(base: &mut WorldPersistV18, chunk: WorldPersistV18) {
    if !chunk.named_timers.is_empty() {
        base.named_timers = chunk.named_timers;
        base.named_timer_display_shown = chunk.named_timer_display_shown;
        base.named_timer_last_flash_frame = chunk.named_timer_last_flash_frame;
        base.named_timer_used_flash_color = chunk.named_timer_used_flash_color;
    }
    base.superweapon_hidden_by_script = chunk.superweapon_hidden_by_script;
    if !chunk.superweapon_entries.is_empty() {
        base.superweapon_entries = chunk.superweapon_entries;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persist_v18_defaults_match_cpp_globals() {
        let persist = WorldPersistV18::default();
        assert_eq!(persist.rank_level_limit, 1000);
        assert!(persist.draw_icon_ui);
        assert!(persist.show_behind_building_markers);
        assert_eq!(persist.script_hulk_max_lifetime_override, -1);
        assert!(persist.named_timer_display_shown);
        assert!(!persist.radar_hidden);
        assert!(!persist.radar_forced);
    }

    #[test]
    fn ingame_ui_chunk_round_trips_named_timers() {
        let mut persist = WorldPersistV18::default();
        persist.named_timer_display_shown = false;
        persist.named_timers.push(NamedTimerPersist {
            name: "LaunchClock".into(),
            text: "Launch in".into(),
            is_countdown: true,
        });
        persist.superweapon_hidden_by_script = true;
        persist.superweapon_entries.push(SuperweaponDisplayPersist {
            player_index: 0,
            template_name: "SuperweaponDaisyCutter".into(),
            power_name: "DaisyCutter".into(),
            object_id: 12,
            timestamp: 30,
            hidden_by_script: true,
            hidden_by_science: false,
            ready: false,
            eva_ready_played: false,
        });
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, 1);
            write_ingame_ui_block(&mut xfer, &persist).expect("write");
        }
        let parsed = parse_ingame_ui_block(&cursor.into_inner()).expect("parse");
        assert!(!parsed.named_timer_display_shown);
        assert_eq!(parsed.named_timers[0].name, "LaunchClock");
        assert!(parsed.superweapon_hidden_by_script);
        assert_eq!(parsed.superweapon_entries[0].object_id, 12);
    }

    #[test]
    fn tactical_view_chunk_round_trips_angle_and_position() {
        let mut persist = WorldPersistV18::default();
        persist.camera_angle = 0.5;
        persist.camera_position = [10.0, 20.0, 30.0];
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, 1);
            write_tactical_view_block(&mut xfer, &persist).expect("write");
        }
        let camera = parse_tactical_view_block(&cursor.into_inner()).expect("parse");
        assert!((camera.angle - 0.5).abs() < f32::EPSILON);
        assert_eq!(camera.position, [10.0, 20.0, 30.0]);
    }

    #[test]
    fn terrain_and_radar_chunks_round_trip() {
        let mut persist = WorldPersistV18::default();
        persist.terrain_active_boundary = 1;
        persist.water_updates.push(WaterUpdatePersist {
            trigger_id: 7,
            change_per_frame: 0.25,
            target_height: 12.0,
            damage_amount: 5.0,
            current_height: 4.0,
        });
        persist.radar_hidden = true;
        persist.radar_forced = true;
        persist.radar_events.push(RadarEventPersist {
            event_type: 3,
            active: true,
            create_frame: 10,
            die_frame: 40,
            fade_frame: 30,
            color1: [255, 0, 0, 255],
            color2: [255, 128, 128, 255],
            world_loc: [1.0, 2.0, 3.0],
            radar_loc: [8, 9],
            sound_played: true,
        });
        persist.radar_next_event = 1;
        persist.radar_last_event = 0;
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, 1);
            write_terrain_logic_block(&mut xfer, &persist).expect("write terrain");
        }
        let (boundary, water) = parse_terrain_logic_block(&cursor.into_inner()).expect("parse terrain");
        assert_eq!(boundary, 1);
        assert_eq!(water[0].trigger_id, 7);
        assert!((water[0].damage_amount - 5.0).abs() < f32::EPSILON);

        let mut cursor = Cursor::new(Vec::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, 1);
            write_radar_block(&mut xfer, &persist).expect("write radar");
        }
        let (hidden, forced, events, next, last) =
            parse_radar_block(&cursor.into_inner()).expect("parse radar");
        assert!(hidden && forced);
        assert!(events[0].active);
        assert_eq!(events[0].event_type, 3);
        assert_eq!(next, 1);
        assert_eq!(last, 0);
    }
}
