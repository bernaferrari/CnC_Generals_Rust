//! C++ save/load residuals that must not change nested WorldSnapshot layout.
//!
//! Version 18 appends one positional tail so v1-v17 streams stay aligned.
//! Live host is the player path; leftover globals are restored alongside.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::system::radar::{
    Coord3D, ICoord2D, MAX_RADAR_EVENTS, RGBAColorInt, RadarEvent, RadarEventType, get_radar_system,
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
pub struct SequentialScriptPersist {
    pub team_id: u32,
    pub object_id: u32,
    pub script_name: String,
    pub current_instruction: i32,
    pub times_to_loop: i32,
    pub frames_to_wait: i32,
    pub dont_advance_instruction: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct NamedRevealPersist {
    pub reveal_name: String,
    pub waypoint_name: String,
    pub radius_to_reveal: f32,
    pub player_name: String,
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

fn default_one_f32() -> f32 {
    1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DrawableIconPersist {
    pub name: String,
    pub keep_till_frame: u32,
    #[serde(default)]
    pub template_name: String,
    #[serde(default)]
    pub anim_frame: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrawableXferPersist {
    pub object_id: u32,
    pub selection_flash_remaining: u32,
    pub decal_opacity_fade_target: f32,
    pub decal_opacity_fade_rate: f32,
    pub decal_opacity: f32,
    #[serde(default = "default_one_f32")]
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
    #[serde(default = "default_one_f32")]
    pub stealth_opacity: f32,
    #[serde(default = "default_one_f32")]
    pub effective_stealth_opacity: f32,
    #[serde(default)]
    pub heat_vision_opacity: f32,
    #[serde(default = "default_one_f32")]
    pub instance_scale: f32,
    #[serde(default)]
    pub expiration_date: u32,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub hidden_by_stealth: bool,
    #[serde(default)]
    pub overlay_icons: Vec<DrawableIconPersist>,
    #[serde(default)]
    pub tint_envelope: crate::game_logic::DrawableTintEnvelopePersist,
}

impl Default for DrawableXferPersist {
    fn default() -> Self {
        Self {
            object_id: 0,
            selection_flash_remaining: 0,
            decal_opacity_fade_target: 0.0,
            decal_opacity_fade_rate: 0.0,
            decal_opacity: 0.0,
            explicit_opacity: 1.0,
            drawable_status: 0,
            tint_status: 0,
            prev_tint_status: 0,
            fade_mode: 0,
            time_elapsed_fade: 0,
            time_to_fade: 0,
            has_loco_info: false,
            loco_pitch: 0.0,
            loco_pitch_rate: 0.0,
            loco_roll: 0.0,
            loco_roll_rate: 0.0,
            loco_yaw: 0.0,
            loco_accel_pitch: 0.0,
            loco_accel_pitch_rate: 0.0,
            loco_accel_roll: 0.0,
            loco_accel_roll_rate: 0.0,
            stealth_opacity: 1.0,
            effective_stealth_opacity: 1.0,
            heat_vision_opacity: 0.0,
            instance_scale: 1.0,
            expiration_date: 0,
            hidden: false,
            hidden_by_stealth: false,
            overlay_icons: Vec::new(),
            tint_envelope: crate::game_logic::DrawableTintEnvelopePersist::default(),
        }
    }
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
    #[serde(default)]
    pub script_sequential: Vec<SequentialScriptPersist>,

    pub script_counters: Vec<ScriptCounterPersist>,
    pub script_flags: Vec<ScriptFlagPersist>,
    pub script_actives: Vec<ScriptActivePersist>,
    #[serde(default)]
    pub script_named_reveals: Vec<NamedRevealPersist>,

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
    /// Leftover ScriptEngine xfer tail. Skipped in WorldSnapshot bincode so
    /// CHUNK_ScriptEngine can grow without a v21 snapshot migration.
    #[serde(skip)]
    pub script_engine_tail: Option<gamelogic::scripting::engine::ScriptEngineXferTail>,
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
            script_sequential: Vec::new(),

            script_counters: Vec::new(),
            script_flags: Vec::new(),
            script_actives: Vec::new(),
            script_named_reveals: Vec::new(),

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
            script_engine_tail: None,
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
        color1: [
            event.color1.r,
            event.color1.g,
            event.color1.b,
            event.color1.a,
        ],
        color2: [
            event.color2.r,
            event.color2.g,
            event.color2.b,
            event.color2.a,
        ],
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
        color1: RGBAColorInt::new(
            entry.color1[0],
            entry.color1[1],
            entry.color1[2],
            entry.color1[3],
        ),
        color2: RGBAColorInt::new(
            entry.color2[0],
            entry.color2[1],
            entry.color2[2],
            entry.color2[3],
        ),
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

    if let Some((counters, flags, sequential, named_reveals, tail)) =
        gamelogic::scripting::engine::with_script_engine_ref(|engine| {
            let (counters, flags) = engine.snapshot_named_trackers();
            (
                counters,
                flags,
                engine.snapshot_sequential_scripts(),
                engine.snapshot_named_reveals(),
                engine.snapshot_xfer_tail(),
            )
        })
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
        persist.script_sequential = sequential
            .into_iter()
            .map(|script| SequentialScriptPersist {
                team_id: script.team_id,
                object_id: script.object_id,
                script_name: script.script_name,
                current_instruction: script.current_instruction,
                times_to_loop: script.times_to_loop,
                frames_to_wait: script.frames_to_wait,
                dont_advance_instruction: script.dont_advance_instruction,
            })
            .collect();
        persist.script_named_reveals = named_reveals
            .into_iter()
            .map(
                |(reveal_name, waypoint_name, radius_to_reveal, player_name)| NamedRevealPersist {
                    reveal_name,
                    waypoint_name,
                    radius_to_reveal,
                    player_name,
                },
            )
            .collect();
        persist.script_engine_tail = Some(tail);
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
        let tint_envelope =
            crate::game_logic::capture_drawable_tint_envelope(id.0).unwrap_or_default();
        let mut explicit_opacity = object.drawable_explicit_opacity;
        let mut stealth_opacity = object.camo_friendly_opacity;
        let mut effective_stealth_opacity = object.camo_friendly_opacity;
        let mut instance_scale = object.drawable_instance_scale;
        let mut heat_vision_opacity = object.camo_heat_vision_opacity;
        let mut tint_status = object.drawable_tint_status;
        let mut prev_tint_status = object.drawable_prev_tint_status;
        let mut hidden = object.drawable_hidden;
        let mut hidden_by_stealth = object.camo_stealth_look == 5;
        let mut expiration_date = object.drawable_expiration_date;
        let mut has_loco_info = object.drawable_loco_pitch != 0.0
            || object.drawable_loco_roll != 0.0
            || object.drawable_loco_yaw != 0.0
            || object.drawable_loco_pitch_rate != 0.0
            || object.drawable_loco_roll_rate != 0.0;
        let mut loco_pitch = object.drawable_loco_pitch;
        let mut loco_pitch_rate = object.drawable_loco_pitch_rate;
        let mut loco_roll = object.drawable_loco_roll;
        let mut loco_roll_rate = object.drawable_loco_roll_rate;
        let mut loco_yaw = object.drawable_loco_yaw;
        let mut loco_accel_pitch = object.drawable_loco_accel_pitch;
        let mut loco_accel_pitch_rate = object.drawable_loco_accel_pitch_rate;
        let mut loco_accel_roll = object.drawable_loco_accel_roll;
        let mut loco_accel_roll_rate = object.drawable_loco_accel_roll_rate;
        let mut overlay_icons: Vec<DrawableIconPersist> = object
            .drawable_overlay_icons
            .iter()
            .map(|icon| DrawableIconPersist {
                name: icon.name.clone(),
                keep_till_frame: icon.keep_till_frame,
                template_name: icon.template_name.clone(),
                anim_frame: icon.anim_frame,
            })
            .collect();
        #[cfg(feature = "game_client")]
        if let Some(left) = game_client::core::capture_live_drawable_xfer_visuals(id.0) {
            explicit_opacity = left.explicit_opacity;
            stealth_opacity = left.stealth_opacity;
            effective_stealth_opacity = left.effective_stealth_opacity;
            instance_scale = left.instance_scale;
            heat_vision_opacity = left.heat_vision_opacity;
            tint_status = left.tint_status;
            prev_tint_status = left.prev_tint_status;
            hidden = left.hidden;
            hidden_by_stealth = left.hidden_by_stealth;
            expiration_date = left.expiration_date;
            has_loco_info = left.has_loco;
            loco_pitch = left.loco_pitch;
            loco_pitch_rate = left.loco_pitch_rate;
            loco_roll = left.loco_roll;
            loco_roll_rate = left.loco_roll_rate;
            loco_yaw = left.loco_yaw;
            loco_accel_pitch = left.loco_accel_pitch;
            loco_accel_pitch_rate = left.loco_accel_pitch_rate;
            loco_accel_roll = left.loco_accel_roll;
            loco_accel_roll_rate = left.loco_accel_roll_rate;
            if !left.overlay_icons.is_empty() {
                overlay_icons = left
                    .overlay_icons
                    .into_iter()
                    .map(
                        |(name, keep_till_frame, template_name, anim_frame)| DrawableIconPersist {
                            name,
                            keep_till_frame,
                            template_name,
                            anim_frame,
                        },
                    )
                    .collect();
            }
        }
        let has_visuals = object.selection_flash_remaining != 0
            || object.terrain_decal_fade_rate != 0.0
            || object.terrain_decal_opacity != 0.0
            || object.terrain_decal_fade_target != 0.0
            || hidden
            || object.drawable_fade_mode != 0
            || (explicit_opacity - 1.0).abs() > f32::EPSILON
            || (instance_scale - 1.0).abs() > f32::EPSILON
            || tint_status != 0
            || prev_tint_status != 0
            || expiration_date != 0
            || heat_vision_opacity > 0.0
            || (stealth_opacity - 1.0).abs() > f32::EPSILON
            || loco_pitch != 0.0
            || loco_roll != 0.0
            || loco_yaw != 0.0
            || !overlay_icons.is_empty()
            || tint_envelope.seen;
        if !has_visuals {
            continue;
        }
        persist.drawable_xfer.push(DrawableXferPersist {
            object_id: id.0,
            selection_flash_remaining: object.selection_flash_remaining,
            decal_opacity_fade_target: object.terrain_decal_fade_target,
            decal_opacity_fade_rate: object.terrain_decal_fade_rate,
            decal_opacity: object.terrain_decal_opacity,
            explicit_opacity,
            drawable_status: u32::from(hidden),
            tint_status,
            prev_tint_status,
            fade_mode: object.drawable_fade_mode,
            time_elapsed_fade: object
                .drawable_fade_start_frame
                .min(object.drawable_fade_frames),
            time_to_fade: object.drawable_fade_frames,
            has_loco_info,
            loco_pitch,
            loco_pitch_rate,
            loco_roll,
            loco_roll_rate,
            loco_yaw,
            loco_accel_pitch,
            loco_accel_pitch_rate,
            loco_accel_roll,
            loco_accel_roll_rate,
            stealth_opacity,
            effective_stealth_opacity,
            heat_vision_opacity,
            instance_scale,
            expiration_date,
            hidden,
            hidden_by_stealth,
            overlay_icons,
            tint_envelope,
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

    game_logic.restore_script_named_timers(
        persist
            .named_timers
            .iter()
            .map(|timer| (timer.name.clone(), timer.text.clone(), timer.is_countdown)),
    );
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
    let sequential: Vec<gamelogic::scripting::engine::SequentialScriptSnapshot> = persist
        .script_sequential
        .iter()
        .map(
            |script| gamelogic::scripting::engine::SequentialScriptSnapshot {
                team_id: script.team_id,
                object_id: script.object_id,
                script_name: script.script_name.clone(),
                current_instruction: script.current_instruction,
                times_to_loop: script.times_to_loop,
                frames_to_wait: script.frames_to_wait,
                dont_advance_instruction: script.dont_advance_instruction,
            },
        )
        .collect();
    let named_reveals: Vec<(String, String, f32, String)> = persist
        .script_named_reveals
        .iter()
        .map(|reveal| {
            (
                reveal.reveal_name.clone(),
                reveal.waypoint_name.clone(),
                reveal.radius_to_reveal,
                reveal.player_name.clone(),
            )
        })
        .collect();
    let _ = gamelogic::scripting::engine::with_script_engine_mut(|engine| {
        let _ = engine.restore_named_trackers(&counters, &flags);
        engine.restore_sequential_scripts(&sequential);
        engine.restore_named_reveals(&named_reveals);
        if let Some(tail) = &persist.script_engine_tail {
            engine.restore_xfer_tail(tail);
        }
        engine.reapply_named_map_reveals();
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
            .map(
                |entry| gamelogic::terrain::TerrainDynamicWaterSnapshotEntry {
                    trigger_id: entry.trigger_id,
                    water_name: AsciiString::new(),
                    change_per_frame: entry.change_per_frame,
                    target_height: entry.target_height,
                    damage_amount: entry.damage_amount,
                    current_height: entry.current_height,
                },
            )
            .collect();
        let _ = terrain.restore_dynamic_water_entries(entries);
    }

    game_logic.restore_radar_script_state(!persist.radar_hidden, persist.radar_forced);
    if let Ok(mut radar) = get_radar_system().write() {
        let mut events = std::array::from_fn(|_| RadarEvent::default());
        for (idx, entry) in persist
            .radar_events
            .iter()
            .take(MAX_RADAR_EVENTS)
            .enumerate()
        {
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
        object.drawable_hidden = entry.hidden || entry.drawable_status != 0;
        object.drawable_fade_mode = entry.fade_mode;
        object.drawable_fade_frames = entry.time_to_fade;
        object.drawable_fade_start_frame = entry.time_elapsed_fade;
        object.drawable_explicit_opacity = entry.explicit_opacity;
        object.drawable_instance_scale = entry.instance_scale;
        object.drawable_tint_status = entry.tint_status;
        object.drawable_prev_tint_status = entry.prev_tint_status;
        object.drawable_expiration_date = entry.expiration_date;
        object.drawable_loco_pitch = entry.loco_pitch;
        object.drawable_loco_pitch_rate = entry.loco_pitch_rate;
        object.drawable_loco_roll = entry.loco_roll;
        object.drawable_loco_roll_rate = entry.loco_roll_rate;
        object.drawable_loco_yaw = entry.loco_yaw;
        object.drawable_loco_accel_pitch = entry.loco_accel_pitch;
        object.drawable_loco_accel_pitch_rate = entry.loco_accel_pitch_rate;
        object.drawable_loco_accel_roll = entry.loco_accel_roll;
        object.drawable_loco_accel_roll_rate = entry.loco_accel_roll_rate;
        object.camo_friendly_opacity = entry.stealth_opacity;
        object.camo_heat_vision_opacity = entry.heat_vision_opacity;
        if entry.hidden_by_stealth {
            object.camo_stealth_look = 5;
        }
        object.drawable_overlay_icons = entry
            .overlay_icons
            .iter()
            .map(|icon| crate::game_logic::DrawableOverlayIcon {
                name: icon.name.clone(),
                keep_till_frame: icon.keep_till_frame,
                template_name: icon.template_name.clone(),
                anim_frame: icon.anim_frame,
            })
            .collect();
        if entry.tint_envelope.seen {
            crate::game_logic::restore_drawable_tint_envelope(entry.object_id, entry.tint_envelope);
        }
        #[cfg(feature = "game_client")]
        {
            let visuals = game_client::drawable::DrawableXferVisualSnapshot {
                explicit_opacity: entry.explicit_opacity,
                stealth_opacity: entry.stealth_opacity,
                effective_stealth_opacity: entry.effective_stealth_opacity,
                instance_scale: entry.instance_scale,
                heat_vision_opacity: entry.heat_vision_opacity,
                tint_status: entry.tint_status,
                prev_tint_status: entry.prev_tint_status,
                hidden: entry.hidden || entry.drawable_status != 0,
                hidden_by_stealth: entry.hidden_by_stealth,
                expiration_date: entry.expiration_date,
                has_loco: entry.has_loco_info,
                loco_pitch: entry.loco_pitch,
                loco_pitch_rate: entry.loco_pitch_rate,
                loco_roll: entry.loco_roll,
                loco_roll_rate: entry.loco_roll_rate,
                loco_yaw: entry.loco_yaw,
                loco_accel_pitch: entry.loco_accel_pitch,
                loco_accel_pitch_rate: entry.loco_accel_pitch_rate,
                loco_accel_roll: entry.loco_accel_roll,
                loco_accel_roll_rate: entry.loco_accel_roll_rate,
                overlay_icons: entry
                    .overlay_icons
                    .iter()
                    .map(|icon| {
                        (
                            icon.name.clone(),
                            icon.keep_till_frame,
                            icon.template_name.clone(),
                            icon.anim_frame,
                        )
                    })
                    .collect(),
            };
            let _ =
                game_client::core::restore_live_drawable_xfer_visuals(entry.object_id, &visuals);
        }
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
    let mut version = 8u8;
    map_xfer(xfer.xfer_version(&mut version, 8))?;
    let mut sequential = persist.script_sequential.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut sequential))?;
    for script in &persist.script_sequential {
        let mut seq_version = 1u8;
        map_xfer(xfer.xfer_version(&mut seq_version, 1))?;
        let mut team_id = script.team_id;
        map_xfer(xfer.xfer_unsigned_int(&mut team_id))?;
        let mut object_id = script.object_id;
        map_xfer(xfer.xfer_object_id(&mut object_id))?;
        let mut script_name = script.script_name.clone();
        map_xfer(xfer.xfer_ascii_string(&mut script_name))?;
        let mut current_instruction = script.current_instruction;
        map_xfer(xfer.xfer_int(&mut current_instruction))?;
        let mut times_to_loop = script.times_to_loop;
        map_xfer(xfer.xfer_int(&mut times_to_loop))?;
        let mut frames_to_wait = script.frames_to_wait;
        map_xfer(xfer.xfer_int(&mut frames_to_wait))?;
        let mut dont_advance = script.dont_advance_instruction;
        map_xfer(xfer.xfer_bool(&mut dont_advance))?;
    }

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
    let mut named_reveal_count = persist.script_named_reveals.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut named_reveal_count))?;
    for reveal in &persist.script_named_reveals {
        let mut reveal_name = reveal.reveal_name.clone();
        let mut waypoint_name = reveal.waypoint_name.clone();
        let mut radius_to_reveal = reveal.radius_to_reveal;
        let mut player_name = reveal.player_name.clone();
        map_xfer(xfer.xfer_ascii_string(&mut reveal_name))?;
        map_xfer(xfer.xfer_ascii_string(&mut waypoint_name))?;
        map_xfer(xfer.xfer_real(&mut radius_to_reveal))?;
        map_xfer(xfer.xfer_ascii_string(&mut player_name))?;
    }
    if let Some(tail) = &persist.script_engine_tail {
        write_script_engine_xfer_tail(xfer, tail)?;
    } else {
        write_script_engine_xfer_tail(
            xfer,
            &gamelogic::scripting::engine::ScriptEngineXferTail::default(),
        )?;
    }
    Ok(())
}

const SCRIPT_ENGINE_PLAYER_COUNT: u16 = 16;

fn write_ascii_list<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    list: &[String],
) -> SaveLoadResult<()> {
    let mut version = 1u8;
    map_xfer(xfer.xfer_version(&mut version, 1))?;
    let mut count = list.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    for entry in list {
        let mut value = entry.clone();
        map_xfer(xfer.xfer_ascii_string(&mut value))?;
    }
    Ok(())
}

fn write_ascii_u32_list<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    list: &[(String, u32)],
) -> SaveLoadResult<()> {
    let mut version = 1u8;
    map_xfer(xfer.xfer_version(&mut version, 1))?;
    let mut count = list.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    for (name, value) in list {
        let mut entry_name = name.clone();
        let mut entry_value = *value;
        map_xfer(xfer.xfer_ascii_string(&mut entry_name))?;
        map_xfer(xfer.xfer_unsigned_int(&mut entry_value))?;
    }
    Ok(())
}

fn write_player_pair_lists<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    lists: &[Vec<(String, u32)>],
) -> SaveLoadResult<()> {
    let mut size = SCRIPT_ENGINE_PLAYER_COUNT;
    map_xfer(xfer.xfer_unsigned_short(&mut size))?;
    for index in 0..SCRIPT_ENGINE_PLAYER_COUNT as usize {
        let empty = Vec::new();
        write_ascii_u32_list(xfer, lists.get(index).unwrap_or(&empty))?;
    }
    Ok(())
}

fn write_script_engine_xfer_tail<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    tail: &gamelogic::scripting::engine::ScriptEngineXferTail,
) -> SaveLoadResult<()> {
    let mut attack_size = tail.attack_priorities.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut attack_size))?;
    for (name, default_priority, entries) in &tail.attack_priorities {
        let mut info_version = 1u8;
        map_xfer(xfer.xfer_version(&mut info_version, 1))?;
        let mut info_name = name.clone();
        map_xfer(xfer.xfer_ascii_string(&mut info_name))?;
        let mut priority = *default_priority;
        map_xfer(xfer.xfer_int(&mut priority))?;
        let mut entry_count = entries.len() as u16;
        map_xfer(xfer.xfer_unsigned_short(&mut entry_count))?;
        for (template_name, value) in entries {
            let mut entry_name = template_name.clone();
            let mut entry_value = *value;
            map_xfer(xfer.xfer_ascii_string(&mut entry_name))?;
            map_xfer(xfer.xfer_int(&mut entry_value))?;
        }
    }
    let mut num_attack = tail.attack_priorities.len() as i32;
    map_xfer(xfer.xfer_int(&mut num_attack))?;
    let mut object_priority = tail.object_attack_priority_sets.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut object_priority))?;
    for (object_id, set_name) in &tail.object_attack_priority_sets {
        let mut id = *object_id;
        let mut name = set_name.clone();
        map_xfer(xfer.xfer_object_id(&mut id))?;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
    }
    let mut end_game_timer = tail.end_game_timer;
    map_xfer(xfer.xfer_int(&mut end_game_timer))?;
    let mut close_window_timer = tail.close_window_timer;
    map_xfer(xfer.xfer_int(&mut close_window_timer))?;
    let mut named_count = tail.named_objects.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut named_count))?;
    for (name, object_id) in &tail.named_objects {
        let mut entry_name = name.clone();
        let mut entry_id = *object_id;
        map_xfer(xfer.xfer_ascii_string(&mut entry_name))?;
        map_xfer(xfer.xfer_object_id(&mut entry_id))?;
    }
    let mut first_update = tail.first_update;
    map_xfer(xfer.xfer_bool(&mut first_update))?;
    let mut fade = tail.fade;
    map_xfer(xfer.xfer_int(&mut fade))?;
    let mut min_fade = tail.min_fade;
    map_xfer(xfer.xfer_real(&mut min_fade))?;
    let mut max_fade = tail.max_fade;
    map_xfer(xfer.xfer_real(&mut max_fade))?;
    let mut cur_fade_value = tail.cur_fade_value;
    map_xfer(xfer.xfer_real(&mut cur_fade_value))?;
    let mut cur_fade_frame = tail.cur_fade_frame;
    map_xfer(xfer.xfer_int(&mut cur_fade_frame))?;
    let mut fade_frames_increase = tail.fade_frames_increase;
    map_xfer(xfer.xfer_int(&mut fade_frames_increase))?;
    let mut fade_frames_hold = tail.fade_frames_hold;
    map_xfer(xfer.xfer_int(&mut fade_frames_hold))?;
    let mut fade_frames_decrease = tail.fade_frames_decrease;
    map_xfer(xfer.xfer_int(&mut fade_frames_decrease))?;
    write_ascii_list(xfer, &tail.completed_video)?;
    write_ascii_u32_list(xfer, &tail.testing_speech)?;
    write_ascii_u32_list(xfer, &tail.testing_audio)?;
    write_ascii_list(xfer, &tail.ui_interactions)?;
    write_player_pair_lists(xfer, &tail.triggered_special_powers)?;
    write_player_pair_lists(xfer, &tail.midway_special_powers)?;
    write_player_pair_lists(xfer, &tail.finished_special_powers)?;
    write_player_pair_lists(xfer, &tail.completed_upgrades)?;
    let mut science_size = SCRIPT_ENGINE_PLAYER_COUNT;
    map_xfer(xfer.xfer_unsigned_short(&mut science_size))?;
    for index in 0..SCRIPT_ENGINE_PLAYER_COUNT as usize {
        let empty = Vec::new();
        let list = tail.acquired_sciences.get(index).unwrap_or(&empty);
        let mut list_version = 1u8;
        map_xfer(xfer.xfer_version(&mut list_version, 1))?;
        let mut count = list.len() as u16;
        map_xfer(xfer.xfer_unsigned_short(&mut count))?;
        for science in list {
            let mut value = *science;
            map_xfer(xfer.xfer_int(&mut value))?;
        }
    }
    let mut topple_version = 1u8;
    map_xfer(xfer.xfer_version(&mut topple_version, 1))?;
    let mut topple_count = tail.topple_directions.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut topple_count))?;
    for (name, xyz) in &tail.topple_directions {
        let mut entry_name = name.clone();
        let mut x = xyz[0];
        let mut y = xyz[1];
        let mut z = xyz[2];
        map_xfer(xfer.xfer_ascii_string(&mut entry_name))?;
        map_xfer(xfer.xfer_real(&mut x))?;
        map_xfer(xfer.xfer_real(&mut y))?;
        map_xfer(xfer.xfer_real(&mut z))?;
    }
    let mut breeze_direction = tail.breeze_direction;
    map_xfer(xfer.xfer_real(&mut breeze_direction))?;
    let mut breeze_x = tail.breeze_direction_vec[0];
    map_xfer(xfer.xfer_real(&mut breeze_x))?;
    let mut breeze_y = tail.breeze_direction_vec[1];
    map_xfer(xfer.xfer_real(&mut breeze_y))?;
    let mut breeze_intensity = tail.breeze_intensity;
    map_xfer(xfer.xfer_real(&mut breeze_intensity))?;
    let mut breeze_lean = tail.breeze_lean;
    map_xfer(xfer.xfer_real(&mut breeze_lean))?;
    let mut breeze_randomness = tail.breeze_randomness;
    map_xfer(xfer.xfer_real(&mut breeze_randomness))?;
    let mut breeze_period = tail.breeze_period;
    map_xfer(xfer.xfer_short(&mut breeze_period))?;
    let mut breeze_version = tail.breeze_version;
    map_xfer(xfer.xfer_short(&mut breeze_version))?;
    let mut game_difficulty = tail.game_difficulty;
    map_xfer(xfer.xfer_int(&mut game_difficulty))?;
    let mut freeze_by_script = tail.freeze_by_script;
    map_xfer(xfer.xfer_bool(&mut freeze_by_script))?;
    let mut object_type_count = tail.object_types.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut object_type_count))?;
    for (list_name, types) in &tail.object_types {
        let mut obj_version = 1u8;
        map_xfer(xfer.xfer_version(&mut obj_version, 1))?;
        let mut name = list_name.clone();
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        let mut type_count = types.len() as u16;
        map_xfer(xfer.xfer_unsigned_short(&mut type_count))?;
        for object_type in types {
            let mut type_name = object_type.clone();
            map_xfer(xfer.xfer_ascii_string(&mut type_name))?;
        }
    }
    let mut difficulty_bonus = tail.objects_should_receive_difficulty_bonus;
    map_xfer(xfer.xfer_bool(&mut difficulty_bonus))?;
    let mut current_track_name = tail.current_track_name.clone();
    map_xfer(xfer.xfer_ascii_string(&mut current_track_name))?;
    let mut choose_victim = tail.choose_victim_always_uses_normal;
    map_xfer(xfer.xfer_bool(&mut choose_victim))?;
    Ok(())
}

pub fn parse_script_engine_block(
    payload: &[u8],
) -> SaveLoadResult<(
    Vec<SequentialScriptPersist>,
    Vec<ScriptCounterPersist>,
    Vec<ScriptFlagPersist>,
    Vec<ScriptActivePersist>,
    Vec<NamedRevealPersist>,
    Option<gamelogic::scripting::engine::ScriptEngineXferTail>,
)> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), 1);
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, 8))?;
    let mut sequential_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut sequential_count))?;
    let mut sequential = Vec::new();
    for _ in 0..sequential_count {
        let mut seq_version = 0u8;
        map_xfer(xfer.xfer_version(&mut seq_version, 1))?;
        let mut team_id = 0u32;
        map_xfer(xfer.xfer_unsigned_int(&mut team_id))?;
        let mut object_id = 0u32;
        map_xfer(xfer.xfer_object_id(&mut object_id))?;
        let mut script_name = String::new();
        map_xfer(xfer.xfer_ascii_string(&mut script_name))?;
        let mut current_instruction = 0i32;
        map_xfer(xfer.xfer_int(&mut current_instruction))?;
        let mut times_to_loop = 0i32;
        map_xfer(xfer.xfer_int(&mut times_to_loop))?;
        let mut frames_to_wait = 0i32;
        map_xfer(xfer.xfer_int(&mut frames_to_wait))?;
        let mut dont_advance = false;
        map_xfer(xfer.xfer_bool(&mut dont_advance))?;
        sequential.push(SequentialScriptPersist {
            team_id,
            object_id,
            script_name,
            current_instruction,
            times_to_loop,
            frames_to_wait,
            dont_advance_instruction: dont_advance,
        });
    }
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
    let mut named_reveals = Vec::new();
    if version >= 7 {
        let mut named_reveal_count = 0u16;
        if map_xfer(xfer.xfer_unsigned_short(&mut named_reveal_count)).is_ok() {
            for _ in 0..named_reveal_count {
                let mut reveal_name = String::new();
                let mut waypoint_name = String::new();
                let mut radius_to_reveal = 0.0f32;
                let mut player_name = String::new();
                map_xfer(xfer.xfer_ascii_string(&mut reveal_name))?;
                map_xfer(xfer.xfer_ascii_string(&mut waypoint_name))?;
                map_xfer(xfer.xfer_real(&mut radius_to_reveal))?;
                map_xfer(xfer.xfer_ascii_string(&mut player_name))?;
                if !reveal_name.is_empty() {
                    named_reveals.push(NamedRevealPersist {
                        reveal_name,
                        waypoint_name,
                        radius_to_reveal,
                        player_name,
                    });
                }
            }
        }
    }
    let tail = if version >= 8 {
        Some(parse_script_engine_xfer_tail(&mut xfer)?)
    } else {
        None
    };
    Ok((sequential, counters, flags, actives, named_reveals, tail))
}

fn parse_ascii_list(xfer: &mut CommonXferLoad<Cursor<&[u8]>>) -> SaveLoadResult<Vec<String>> {
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, 1))?;
    let mut count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    let mut list = Vec::new();
    for _ in 0..count {
        let mut value = String::new();
        map_xfer(xfer.xfer_ascii_string(&mut value))?;
        list.push(value);
    }
    Ok(list)
}

fn parse_ascii_u32_list(
    xfer: &mut CommonXferLoad<Cursor<&[u8]>>,
) -> SaveLoadResult<Vec<(String, u32)>> {
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, 1))?;
    let mut count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    let mut list = Vec::new();
    for _ in 0..count {
        let mut name = String::new();
        let mut value = 0u32;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_unsigned_int(&mut value))?;
        list.push((name, value));
    }
    Ok(list)
}

fn parse_player_pair_lists(
    xfer: &mut CommonXferLoad<Cursor<&[u8]>>,
) -> SaveLoadResult<Vec<Vec<(String, u32)>>> {
    let mut size = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut size))?;
    let mut lists = Vec::new();
    for _ in 0..size {
        lists.push(parse_ascii_u32_list(xfer)?);
    }
    Ok(lists)
}

fn parse_script_engine_xfer_tail(
    xfer: &mut CommonXferLoad<Cursor<&[u8]>>,
) -> SaveLoadResult<gamelogic::scripting::engine::ScriptEngineXferTail> {
    let mut tail = gamelogic::scripting::engine::ScriptEngineXferTail::default();
    let mut attack_size = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut attack_size))?;
    for _ in 0..attack_size {
        let mut info_version = 0u8;
        map_xfer(xfer.xfer_version(&mut info_version, 1))?;
        let mut name = String::new();
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        let mut default_priority = 1i32;
        map_xfer(xfer.xfer_int(&mut default_priority))?;
        let mut entry_count = 0u16;
        map_xfer(xfer.xfer_unsigned_short(&mut entry_count))?;
        let mut entries = Vec::new();
        for _ in 0..entry_count {
            let mut template_name = String::new();
            let mut value = 0i32;
            map_xfer(xfer.xfer_ascii_string(&mut template_name))?;
            map_xfer(xfer.xfer_int(&mut value))?;
            entries.push((template_name, value));
        }
        tail.attack_priorities
            .push((name, default_priority, entries));
    }
    let mut num_attack = 0i32;
    map_xfer(xfer.xfer_int(&mut num_attack))?;
    let mut object_priority = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut object_priority))?;
    for _ in 0..object_priority {
        let mut object_id = 0u32;
        let mut set_name = String::new();
        map_xfer(xfer.xfer_object_id(&mut object_id))?;
        map_xfer(xfer.xfer_ascii_string(&mut set_name))?;
        if object_id != 0 && !set_name.is_empty() {
            tail.object_attack_priority_sets.push((object_id, set_name));
        }
    }
    map_xfer(xfer.xfer_int(&mut tail.end_game_timer))?;
    map_xfer(xfer.xfer_int(&mut tail.close_window_timer))?;
    let mut named_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut named_count))?;
    for _ in 0..named_count {
        let mut name = String::new();
        let mut object_id = 0u32;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_object_id(&mut object_id))?;
        if !name.is_empty() {
            tail.named_objects.push((name, object_id));
        }
    }
    map_xfer(xfer.xfer_bool(&mut tail.first_update))?;
    map_xfer(xfer.xfer_int(&mut tail.fade))?;
    map_xfer(xfer.xfer_real(&mut tail.min_fade))?;
    map_xfer(xfer.xfer_real(&mut tail.max_fade))?;
    map_xfer(xfer.xfer_real(&mut tail.cur_fade_value))?;
    map_xfer(xfer.xfer_int(&mut tail.cur_fade_frame))?;
    map_xfer(xfer.xfer_int(&mut tail.fade_frames_increase))?;
    map_xfer(xfer.xfer_int(&mut tail.fade_frames_hold))?;
    map_xfer(xfer.xfer_int(&mut tail.fade_frames_decrease))?;
    tail.completed_video = parse_ascii_list(xfer)?;
    tail.testing_speech = parse_ascii_u32_list(xfer)?;
    tail.testing_audio = parse_ascii_u32_list(xfer)?;
    tail.ui_interactions = parse_ascii_list(xfer)?;
    tail.triggered_special_powers = parse_player_pair_lists(xfer)?;
    tail.midway_special_powers = parse_player_pair_lists(xfer)?;
    tail.finished_special_powers = parse_player_pair_lists(xfer)?;
    tail.completed_upgrades = parse_player_pair_lists(xfer)?;
    let mut science_size = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut science_size))?;
    for _ in 0..science_size {
        let mut list_version = 0u8;
        map_xfer(xfer.xfer_version(&mut list_version, 1))?;
        let mut count = 0u16;
        map_xfer(xfer.xfer_unsigned_short(&mut count))?;
        let mut sciences = Vec::new();
        for _ in 0..count {
            let mut value = 0i32;
            map_xfer(xfer.xfer_int(&mut value))?;
            sciences.push(value);
        }
        tail.acquired_sciences.push(sciences);
    }
    let mut topple_version = 0u8;
    map_xfer(xfer.xfer_version(&mut topple_version, 1))?;
    let mut topple_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut topple_count))?;
    for _ in 0..topple_count {
        let mut name = String::new();
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        let mut z = 0.0f32;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_real(&mut x))?;
        map_xfer(xfer.xfer_real(&mut y))?;
        map_xfer(xfer.xfer_real(&mut z))?;
        tail.topple_directions.push((name, [x, y, z]));
    }
    map_xfer(xfer.xfer_real(&mut tail.breeze_direction))?;
    map_xfer(xfer.xfer_real(&mut tail.breeze_direction_vec[0]))?;
    map_xfer(xfer.xfer_real(&mut tail.breeze_direction_vec[1]))?;
    map_xfer(xfer.xfer_real(&mut tail.breeze_intensity))?;
    map_xfer(xfer.xfer_real(&mut tail.breeze_lean))?;
    map_xfer(xfer.xfer_real(&mut tail.breeze_randomness))?;
    map_xfer(xfer.xfer_short(&mut tail.breeze_period))?;
    map_xfer(xfer.xfer_short(&mut tail.breeze_version))?;
    map_xfer(xfer.xfer_int(&mut tail.game_difficulty))?;
    map_xfer(xfer.xfer_bool(&mut tail.freeze_by_script))?;
    let mut object_type_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut object_type_count))?;
    for _ in 0..object_type_count {
        let mut obj_version = 0u8;
        map_xfer(xfer.xfer_version(&mut obj_version, 1))?;
        let mut list_name = String::new();
        map_xfer(xfer.xfer_ascii_string(&mut list_name))?;
        let mut type_count = 0u16;
        map_xfer(xfer.xfer_unsigned_short(&mut type_count))?;
        let mut types = Vec::new();
        for _ in 0..type_count {
            let mut type_name = String::new();
            map_xfer(xfer.xfer_ascii_string(&mut type_name))?;
            types.push(type_name);
        }
        tail.object_types.push((list_name, types));
    }
    map_xfer(xfer.xfer_bool(&mut tail.objects_should_receive_difficulty_bonus))?;
    map_xfer(xfer.xfer_ascii_string(&mut tail.current_track_name))?;
    map_xfer(xfer.xfer_bool(&mut tail.choose_victim_always_uses_normal))?;
    Ok(tail)
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
        let (boundary, water) =
            parse_terrain_logic_block(&cursor.into_inner()).expect("parse terrain");
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

    #[test]
    fn script_engine_chunk_round_trips_sequential_instances() {
        let mut persist = WorldPersistV18::default();
        persist.script_sequential.push(SequentialScriptPersist {
            team_id: 0,
            object_id: 42,
            script_name: "UnitHuntSeq".into(),
            current_instruction: 3,
            times_to_loop: 2,
            frames_to_wait: 15,
            dont_advance_instruction: true,
        });
        persist.script_counters.push(ScriptCounterPersist {
            name: "AfterSeq".into(),
            value: 7,
            is_countdown_timer: false,
        });
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, 1);
            write_script_engine_block(&mut xfer, &persist).expect("write");
        }
        let (sequential, counters, _flags, _actives, named_reveals, tail) =
            parse_script_engine_block(&cursor.into_inner()).expect("parse");
        assert_eq!(
            sequential.len(),
            1,
            "sequential count must not be written as 0"
        );
        assert_eq!(sequential[0].object_id, 42);
        assert_eq!(sequential[0].script_name, "UnitHuntSeq");
        assert_eq!(sequential[0].current_instruction, 3);
        assert_eq!(sequential[0].times_to_loop, 2);
        assert_eq!(sequential[0].frames_to_wait, 15);
        assert!(sequential[0].dont_advance_instruction);
        assert_eq!(counters[0].name, "AfterSeq");
        assert_eq!(counters[0].value, 7);
        assert!(named_reveals.is_empty());
        assert!(tail.is_some());
    }

    #[test]
    fn script_engine_chunk_round_trips_named_reveals() {
        let mut persist = WorldPersistV18::default();
        persist.script_named_reveals.push(NamedRevealPersist {
            reveal_name: "BaseLook".into(),
            waypoint_name: "WP_Base".into(),
            radius_to_reveal: 250.0,
            player_name: "PlyrAmerica".into(),
        });
        persist.script_counters.push(ScriptCounterPersist {
            name: "AfterReveal".into(),
            value: 4,
            is_countdown_timer: true,
        });
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, 1);
            write_script_engine_block(&mut xfer, &persist).expect("write");
        }
        let (_sequential, counters, _flags, _actives, named_reveals, tail) =
            parse_script_engine_block(&cursor.into_inner()).expect("parse");
        assert_eq!(named_reveals.len(), 1);
        assert_eq!(named_reveals[0].reveal_name, "BaseLook");
        assert_eq!(named_reveals[0].waypoint_name, "WP_Base");
        assert!((named_reveals[0].radius_to_reveal - 250.0).abs() < f32::EPSILON);
        assert_eq!(named_reveals[0].player_name, "PlyrAmerica");
        assert_eq!(counters[0].name, "AfterReveal");
        assert_eq!(counters[0].value, 4);
        assert!(counters[0].is_countdown_timer);
        assert!(tail.is_some());
    }

    #[test]
    fn script_engine_chunk_round_trips_leftover_xfer_tail() {
        let mut persist = WorldPersistV18::default();
        persist.script_engine_tail = Some(gamelogic::scripting::engine::ScriptEngineXferTail {
            attack_priorities: vec![("Heroes".into(), 10, vec![("AmericaRanger".into(), 20)])],
            object_attack_priority_sets: vec![(42, "Heroes".into())],
            end_game_timer: 30,
            close_window_timer: 12,
            named_objects: vec![("NamedHero".into(), 42)],
            first_update: false,
            fade: 2,
            min_fade: 0.2,
            max_fade: 0.8,
            cur_fade_value: 0.4,
            cur_fade_frame: 5,
            fade_frames_increase: 3,
            fade_frames_hold: 4,
            fade_frames_decrease: 6,
            completed_video: vec!["Intro".into()],
            freeze_by_script: true,
            objects_should_receive_difficulty_bonus: false,
            current_track_name: "Combat".into(),
            choose_victim_always_uses_normal: true,
            object_types: vec![("Tanks".into(), vec!["AmericaTank".into()])],
            ..Default::default()
        });
        let mut cursor = Cursor::new(Vec::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, 1);
            write_script_engine_block(&mut xfer, &persist).expect("write");
        }
        let (_sequential, _counters, _flags, _actives, _reveals, tail) =
            parse_script_engine_block(&cursor.into_inner()).expect("parse");
        let tail = tail.expect("v8 tail");
        assert_eq!(tail.attack_priorities[0].0, "Heroes");
        assert_eq!(tail.object_attack_priority_sets[0], (42, "Heroes".into()));
        assert_eq!(tail.end_game_timer, 30);
        assert_eq!(tail.close_window_timer, 12);
        assert_eq!(tail.named_objects[0], ("NamedHero".into(), 42));
        assert_eq!(tail.fade, 2);
        assert!(tail.freeze_by_script);
        assert!(!tail.objects_should_receive_difficulty_bonus);
        assert_eq!(tail.current_track_name, "Combat");
        assert!(tail.choose_victim_always_uses_normal);
        assert_eq!(tail.object_types[0].0, "Tanks");
        assert_eq!(tail.completed_video[0], "Intro");
    }
}
