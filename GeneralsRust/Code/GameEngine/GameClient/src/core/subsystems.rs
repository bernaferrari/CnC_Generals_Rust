//! Lightweight subsystem helpers used by the GameClient.  These implementations
//! provide enough behaviour for non-platform builds while keeping dependencies
//! minimal.

use std::collections::{HashMap, VecDeque};
use std::convert::TryFrom;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::{
    display::view::{with_tactical_view, with_tactical_view_ref, Point3},
    drawable::Drawable,
    effects::particle_manager::xfer_particle_system_manager_state,
    game_text::GameText,
    gui::campaign_manager::{get_campaign_manager, XferHelper as CampaignXferHelper},
    gui::window_video_manager::{with_window_video_manager, WindowVideoPlayType},
    helpers::{InGameUiHooks, PendingCommand, PendingSpecialPower},
    message_stream::game_message::{ICoord2D, ObjectID},
    message_stream::hot_key::with_hot_key_manager,
    system::{
        beacon_display::{BeaconMarker, BEACON_MATCH_THRESHOLD},
        BeaconNotification, Coord3D, SubsystemInterface,
    },
    terrain::{TerrainError, TerrainVisual},
    video_player::{
        get_video_player, init_video_player, VideoPlayerInterface as GlobalVideoPlayerInterface,
    },
};
use game_engine::common::ascii_string::AsciiString;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::System::XferVersion;
use game_engine::{Snapshot as XferSnapshotTrait, Xfer, XferMode, XferStatus};
use gamelogic::commands::command::CommandType;
use gamelogic::common::audio::AudioEventRts as LogicAudioEventRts;
use gamelogic::helpers::{TerrainTreeRegistration, TheAudio, TheGameLogic, TheScriptEngine};
use gamelogic::object::draw::W3DTreeDrawModuleData;
use glam::{Mat4, Vec3};
use kira::manager::{AudioManager, AudioManagerSettings};

use crate::core::game_client::{
    run_live_game_client_load_post_process, xfer_live_game_client_state, InGameUI,
    VideoPlayerInterface,
};

fn xfer_bool_flag(xfer: &mut dyn Xfer, value: &mut bool) -> Result<(), XferStatus> {
    xfer.xfer_bool(value)
}

fn xfer_string_value(xfer: &mut dyn Xfer, value: &mut String) -> Result<(), XferStatus> {
    xfer.xfer_string(value)
}

fn xfer_option_coord2d(
    xfer: &mut dyn Xfer,
    value: &mut Option<ICoord2D>,
) -> Result<(), XferStatus> {
    let mut present = value.is_some();
    xfer.xfer_bool(&mut present)?;
    if present {
        let mut coord = value.clone().unwrap_or_default();
        xfer.xfer_int(&mut coord.x)?;
        xfer.xfer_int(&mut coord.y)?;
        *value = Some(coord);
    } else {
        *value = None;
    }
    Ok(())
}

fn xfer_option_string(xfer: &mut dyn Xfer, value: &mut Option<String>) -> Result<(), XferStatus> {
    let mut present = value.is_some();
    xfer.xfer_bool(&mut present)?;
    if present {
        let mut text = value.clone().unwrap_or_default();
        xfer.xfer_string(&mut text)?;
        *value = Some(text);
    } else {
        *value = None;
    }
    Ok(())
}

fn xfer_ascii_value(xfer: &mut dyn Xfer, value: &mut AsciiString) -> Result<(), XferStatus> {
    let mut as_string = value.to_string();
    xfer.xfer_string(&mut as_string)?;
    *value = AsciiString::from(as_string.as_str());
    Ok(())
}

fn runtime_xfer_status_to_io(status: XferStatus) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("{status:?}"))
}

struct CampaignRuntimeXferAdapter<'a> {
    xfer: &'a mut dyn Xfer,
}

impl<'a> CampaignRuntimeXferAdapter<'a> {
    fn new(xfer: &'a mut dyn Xfer) -> Self {
        Self { xfer }
    }
}

impl CampaignXferHelper for CampaignRuntimeXferAdapter<'_> {
    fn xfer_version(&mut self, version: &mut u16, current: u16) -> io::Result<()> {
        let mut runtime_version = u8::try_from(*version)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "campaign version overflow"))?;
        let runtime_current = u8::try_from(current)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "campaign version overflow"))?;
        self.xfer
            .xfer_version(&mut runtime_version, runtime_current)
            .map_err(runtime_xfer_status_to_io)?;
        *version = runtime_version as u16;
        Ok(())
    }

    fn xfer_ascii_string(&mut self, s: &mut String) -> io::Result<()> {
        self.xfer
            .xfer_ascii_string(s)
            .map_err(runtime_xfer_status_to_io)
    }

    fn xfer_int(&mut self, value: &mut i32) -> io::Result<()> {
        self.xfer.xfer_int(value).map_err(runtime_xfer_status_to_io)
    }

    fn xfer_bool(&mut self, value: &mut bool) -> io::Result<()> {
        self.xfer
            .xfer_bool(value)
            .map_err(runtime_xfer_status_to_io)
    }

    fn xfer_user<T>(&mut self, value: &mut T) -> io::Result<()> {
        let len = std::mem::size_of::<T>();
        let ptr = value as *mut T as *mut u8;
        // SAFETY: `ptr` is derived from a valid mutable reference and `len` matches `T`.
        unsafe { self.xfer.xfer_user(ptr, len) }.map_err(runtime_xfer_status_to_io)
    }

    fn is_loading(&self) -> bool {
        self.xfer.get_xfer_mode() == XferMode::Load
    }
}

fn xfer_w3d_tree_draw_module_data(
    xfer: &mut dyn Xfer,
    data: &mut W3DTreeDrawModuleData,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    xfer.xfer_unsigned_int(&mut data.module_tag_name_key)?;
    xfer_ascii_value(xfer, &mut data.model_name)?;
    xfer_ascii_value(xfer, &mut data.texture_name)?;
    xfer.xfer_unsigned_int(&mut data.frames_to_move_outward)?;
    xfer.xfer_unsigned_int(&mut data.frames_to_move_inward)?;
    xfer.xfer_real(&mut data.max_outward_movement)?;
    xfer.xfer_real(&mut data.darkening)?;

    let mut topple_fx = data.topple_fx.as_ref().map(|value| value.to_string());
    xfer_option_string(xfer, &mut topple_fx)?;
    data.topple_fx = topple_fx.map(|value| AsciiString::from(value.as_str()));

    let mut bounce_fx = data.bounce_fx.as_ref().map(|value| value.to_string());
    xfer_option_string(xfer, &mut bounce_fx)?;
    data.bounce_fx = bounce_fx.map(|value| AsciiString::from(value.as_str()));

    xfer_ascii_value(xfer, &mut data.stump_name)?;
    xfer.xfer_real(&mut data.initial_velocity_percent)?;
    xfer.xfer_real(&mut data.initial_accel_percent)?;
    xfer.xfer_real(&mut data.bounce_velocity_percent)?;
    xfer.xfer_real(&mut data.minimum_topple_speed)?;
    xfer.xfer_bool(&mut data.kill_when_toppled)?;
    xfer.xfer_bool(&mut data.do_topple)?;
    xfer.xfer_unsigned_int(&mut data.sink_frames)?;
    xfer.xfer_real(&mut data.sink_distance)?;
    xfer.xfer_bool(&mut data.do_shadow)?;

    Ok(())
}

fn xfer_terrain_tree_registration(
    xfer: &mut dyn Xfer,
    tree: &mut TerrainTreeRegistration,
) -> Result<(), XferStatus> {
    xfer.xfer_unsigned_int(&mut tree.drawable_id)?;
    xfer.xfer_real(&mut tree.location.x)?;
    xfer.xfer_real(&mut tree.location.y)?;
    xfer.xfer_real(&mut tree.location.z)?;
    xfer.xfer_real(&mut tree.scale)?;
    xfer.xfer_real(&mut tree.angle)?;
    xfer.xfer_real(&mut tree.random_scale_amount)?;
    xfer_w3d_tree_draw_module_data(xfer, &mut tree.module_data)?;
    Ok(())
}

fn xfer_terrain_visual_state(
    terrain: &mut TerrainVisualStub,
    xfer: &mut dyn Xfer,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut entries = if xfer.get_xfer_mode() == XferMode::Save {
        let mut trees = terrain.tree_registrations();
        trees.sort_by_key(|entry| entry.drawable_id);
        trees
    } else {
        Vec::new()
    };

    let mut tree_count = entries.len() as u32;
    xfer.xfer_unsigned_int(&mut tree_count)?;

    if xfer.get_xfer_mode() == XferMode::Load {
        entries.clear();
        entries.reserve(tree_count as usize);
        for _ in 0..tree_count {
            let mut tree = TerrainTreeRegistration {
                drawable_id: 0,
                location: Vec3::ZERO,
                scale: 1.0,
                angle: 0.0,
                random_scale_amount: 0.0,
                module_data: W3DTreeDrawModuleData::new(),
            };
            xfer_terrain_tree_registration(xfer, &mut tree)?;
            entries.push(tree);
        }
        terrain.registered_trees.clear();
        for tree in entries {
            terrain.registered_trees.insert(tree.drawable_id, tree);
        }
    } else {
        for tree in &mut entries {
            xfer_terrain_tree_registration(xfer, tree)?;
        }
    }

    Ok(())
}

fn xfer_pending_special_power(
    xfer: &mut dyn Xfer,
    value: &mut Option<PendingSpecialPower>,
) -> Result<(), XferStatus> {
    let mut present = value.is_some();
    xfer.xfer_bool(&mut present)?;
    if present {
        let mut power = value.clone().unwrap_or(PendingSpecialPower {
            power_id: 0,
            options: 0,
            source_object_id: 0,
        });
        xfer.xfer_unsigned_int(&mut power.power_id)?;
        xfer.xfer_unsigned_int(&mut power.options)?;
        xfer.xfer_unsigned_int(&mut power.source_object_id)?;
        *value = Some(power);
    } else {
        *value = None;
    }
    Ok(())
}

fn xfer_pending_command(
    xfer: &mut dyn Xfer,
    value: &mut Option<PendingCommand>,
) -> Result<(), XferStatus> {
    let mut present = value.is_some();
    xfer.xfer_bool(&mut present)?;
    if present {
        let mut command = value.clone().unwrap_or(PendingCommand {
            command_type: CommandType::Invalid,
            options: 0,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        });

        let mut command_type = command.command_type as u16;
        xfer.xfer_unsigned_short(&mut command_type)?;
        command.command_type = CommandType::try_from(command_type).unwrap_or(CommandType::Invalid);
        xfer.xfer_unsigned_int(&mut command.options)?;
        xfer.xfer_unsigned_int(&mut command.source_object_id)?;
        xfer.xfer_string(&mut command.cursor_name)?;
        xfer.xfer_string(&mut command.invalid_cursor_name)?;
        xfer.xfer_string(&mut command.radius_cursor_type)?;
        *value = Some(command);
    } else {
        *value = None;
    }
    Ok(())
}

fn xfer_in_game_ui_state(
    ui: &mut InGameUISubsystem,
    xfer: &mut dyn Xfer,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    xfer_option_string(xfer, &mut ui.pending_place_template)?;
    xfer.xfer_unsigned_int(&mut ui.pending_place_source_object_id)?;
    xfer_option_coord2d(xfer, &mut ui.placement_start)?;
    xfer_option_coord2d(xfer, &mut ui.placement_end)?;
    xfer.xfer_real(&mut ui.placement_angle)?;
    xfer_bool_flag(xfer, &mut ui.radius_cursor_active)?;
    xfer_string_value(xfer, &mut ui.radius_cursor_type)?;
    xfer_bool_flag(xfer, &mut ui.attack_move_to_mode)?;
    xfer_bool_flag(xfer, &mut ui.force_attack_mode)?;
    xfer_bool_flag(xfer, &mut ui.force_move_to_mode)?;
    xfer_bool_flag(xfer, &mut ui.prefer_selection_mode)?;
    xfer_bool_flag(xfer, &mut ui.waypoint_mode)?;
    xfer_bool_flag(xfer, &mut ui.camera_rotating_left)?;
    xfer_bool_flag(xfer, &mut ui.camera_rotating_right)?;
    xfer_bool_flag(xfer, &mut ui.camera_zooming_in)?;
    xfer_bool_flag(xfer, &mut ui.camera_zooming_out)?;
    xfer_bool_flag(xfer, &mut ui.camera_tracking_drawable)?;
    xfer_bool_flag(
        xfer,
        &mut ui.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click,
    )?;
    xfer_pending_special_power(xfer, &mut ui.pending_special_power)?;
    xfer_pending_command(xfer, &mut ui.pending_command)?;

    if xfer.get_xfer_mode() == XferMode::Load {
        if !ui.radius_cursor_active {
            ui.radius_cursor_type.clear();
        }
    }

    Ok(())
}

fn radar_ping_kind_to_u8(kind: RadarPingKind) -> u8 {
    match kind {
        RadarPingKind::Generic => 0,
        RadarPingKind::Attack => 1,
        RadarPingKind::Ally => 2,
    }
}

fn radar_ping_kind_from_u8(value: u8) -> RadarPingKind {
    match value {
        1 => RadarPingKind::Attack,
        2 => RadarPingKind::Ally,
        _ => RadarPingKind::Generic,
    }
}

fn xfer_radar_state(ui: &mut InGameUISubsystem, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut pings: Vec<RadarPingEvent> = if xfer.get_xfer_mode() == XferMode::Save {
        ui.radar_pings.iter().cloned().collect()
    } else {
        Vec::new()
    };

    let mut ping_count = pings.len() as u32;
    xfer.xfer_unsigned_int(&mut ping_count)?;

    if xfer.get_xfer_mode() == XferMode::Load {
        pings.clear();
        pings.reserve(ping_count as usize);
        for _ in 0..ping_count {
            let mut ping = RadarPingEvent {
                position: Coord3D::new(0.0, 0.0, 0.0),
                kind: RadarPingKind::Generic,
                age_seconds: 0.0,
            };
            xfer.xfer_real(&mut ping.position.x)?;
            xfer.xfer_real(&mut ping.position.y)?;
            xfer.xfer_real(&mut ping.position.z)?;
            let mut kind_value = 0u8;
            xfer.xfer_unsigned_byte(&mut kind_value)?;
            ping.kind = radar_ping_kind_from_u8(kind_value);
            xfer.xfer_real(&mut ping.age_seconds)?;
            pings.push(ping);
        }
        ui.radar_pings = pings.into_iter().collect();
    } else {
        for ping in &mut pings {
            xfer.xfer_real(&mut ping.position.x)?;
            xfer.xfer_real(&mut ping.position.y)?;
            xfer.xfer_real(&mut ping.position.z)?;
            let mut kind_value = radar_ping_kind_to_u8(ping.kind);
            xfer.xfer_unsigned_byte(&mut kind_value)?;
            xfer.xfer_real(&mut ping.age_seconds)?;
            ping.kind = radar_ping_kind_from_u8(kind_value);
        }
    }

    Ok(())
}

struct InGameUISnapshotBridge {
    ui: Arc<Mutex<InGameUISubsystem>>,
}

impl InGameUISnapshotBridge {
    fn new(ui: Arc<Mutex<InGameUISubsystem>>) -> Self {
        Self { ui }
    }
}

impl XferSnapshotTrait for InGameUISnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.ui.lock().map_err(|_| XferStatus::InvalidData)?;
        xfer_in_game_ui_state(&mut guard, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

pub fn register_in_game_ui_snapshot_block(ui: Arc<Mutex<InGameUISubsystem>>) {
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_InGameUI".to_string(),
        Box::new(InGameUISnapshotBridge::new(ui)),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

struct RadarSnapshotBridge {
    ui: Arc<Mutex<InGameUISubsystem>>,
}

impl RadarSnapshotBridge {
    fn new(ui: Arc<Mutex<InGameUISubsystem>>) -> Self {
        Self { ui }
    }
}

impl XferSnapshotTrait for RadarSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self.ui.lock().map_err(|_| XferStatus::InvalidData)?;
        xfer_radar_state(&mut guard, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

pub fn register_radar_snapshot_block(ui: Arc<Mutex<InGameUISubsystem>>) {
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_Radar".to_string(),
        Box::new(RadarSnapshotBridge::new(ui)),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

struct CampaignSnapshotBridge;

impl XferSnapshotTrait for CampaignSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut manager = get_campaign_manager();
        let mut adapter = CampaignRuntimeXferAdapter::new(xfer);
        manager
            .xfer(&mut adapter)
            .map_err(|_| XferStatus::InvalidData)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        let mut manager = get_campaign_manager();
        manager.load_post_process();
        Ok(())
    }
}

pub fn register_campaign_snapshot_block() {
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_Campaign".to_string(),
        Box::new(CampaignSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

fn xfer_tactical_view_state(xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut position = with_tactical_view_ref(|view| *view.position());
    let mut angle = with_tactical_view_ref(|view| view.angle());
    let mut pitch = with_tactical_view_ref(|view| view.pitch());
    let mut zoom = with_tactical_view_ref(|view| view.zoom());
    let mut height_above_ground = with_tactical_view_ref(|view| view.height_above_ground());
    let mut field_of_view = with_tactical_view_ref(|view| view.field_of_view());

    xfer.xfer_real(&mut position.x)?;
    xfer.xfer_real(&mut position.y)?;
    xfer.xfer_real(&mut position.z)?;
    xfer.xfer_real(&mut angle)?;
    xfer.xfer_real(&mut pitch)?;
    xfer.xfer_real(&mut zoom)?;
    xfer.xfer_real(&mut height_above_ground)?;
    xfer.xfer_real(&mut field_of_view)?;

    if xfer.get_xfer_mode() == XferMode::Load {
        with_tactical_view(|view| {
            view.set_position(&Point3::new(position.x, position.y, position.z));
            view.set_angle(angle);
            view.set_pitch(pitch);
            view.set_zoom(zoom);
            view.set_height_above_ground(height_above_ground);
            view.set_field_of_view(field_of_view);
        });
    }

    Ok(())
}

struct TacticalViewSnapshotBridge;

impl XferSnapshotTrait for TacticalViewSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_tactical_view_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

pub fn register_tactical_view_snapshot_block() {
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_TacticalView".to_string(),
        Box::new(TacticalViewSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

struct GameClientSnapshotBridge;

impl XferSnapshotTrait for GameClientSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_live_game_client_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        run_live_game_client_load_post_process()
    }
}

pub fn register_game_client_snapshot_block() {
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_GameClient".to_string(),
        Box::new(GameClientSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

struct TerrainVisualSnapshotBridge {
    terrain_visual: Arc<Mutex<TerrainVisualStub>>,
}

impl TerrainVisualSnapshotBridge {
    fn new(terrain_visual: Arc<Mutex<TerrainVisualStub>>) -> Self {
        Self { terrain_visual }
    }
}

impl XferSnapshotTrait for TerrainVisualSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let mut guard = self
            .terrain_visual
            .lock()
            .map_err(|_| XferStatus::InvalidData)?;
        xfer_terrain_visual_state(&mut guard, xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

pub fn register_terrain_visual_snapshot_block(terrain_visual: Arc<Mutex<TerrainVisualStub>>) {
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_TerrainVisual".to_string(),
        Box::new(TerrainVisualSnapshotBridge::new(terrain_visual)),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

struct ParticleSystemSnapshotBridge;

impl XferSnapshotTrait for ParticleSystemSnapshotBridge {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        xfer_particle_system_manager_state(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

pub fn register_particle_system_snapshot_block() {
    let mut state = game_engine::System::get_game_state();
    state.add_snapshot_block(
        "CHUNK_ParticleSystem".to_string(),
        Box::new(ParticleSystemSnapshotBridge),
        game_engine::System::SnapshotType::SaveLoad,
    );
}

/// Thin wrapper around the existing font library module.
#[derive(Default)]
pub struct FontLibrarySubsystem {
    inner: crate::gui::font::FontLibrary,
}

impl FontLibrarySubsystem {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SubsystemInterface for FontLibrarySubsystem {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.inner.init_mut()?;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.inner.reset_mut()?;
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.inner.update_mut()?;
        Ok(())
    }
}

/// Display string manager wrapper for legacy UI text.
#[derive(Default)]
pub struct DisplayStringManagerSubsystem;

impl DisplayStringManagerSubsystem {
    pub fn new() -> Self {
        Self
    }

    pub fn post_process_load(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // C++ parity: postProcessLoad runs after core client systems are up.
        // Prime shared display strings so first use matches legacy behavior.
        let mut manager = crate::gui::display_string::get_display_string_manager();
        for numeral in 0..=9 {
            let _ = manager.get_group_numeral_string(numeral);
        }
        let _ = manager.get_formation_letter_string();
        Ok(())
    }
}

impl SubsystemInterface for DisplayStringManagerSubsystem {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = crate::gui::display_string::get_display_string_manager();
        manager.init()?;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = crate::gui::display_string::get_display_string_manager();
        manager.reset()?;
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = crate::gui::display_string::get_display_string_manager();
        manager.update()?;
        Ok(())
    }
}

/// Hot key manager wrapper for GUI hotkey mappings.
#[derive(Default)]
pub struct HotKeyManagerSubsystem;

impl HotKeyManagerSubsystem {
    pub fn new() -> Self {
        Self
    }
}

impl SubsystemInterface for HotKeyManagerSubsystem {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        with_hot_key_manager(|manager| manager.init());
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        with_hot_key_manager(|manager| manager.reset());
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// Header template manager wrapper for unified UI font styles.
#[derive(Default)]
pub struct HeaderTemplateManagerSubsystem;

impl HeaderTemplateManagerSubsystem {
    pub fn new() -> Self {
        Self
    }
}

impl SubsystemInterface for HeaderTemplateManagerSubsystem {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::header_template::register_parser();
        let mut manager = crate::gui::header_template::get_header_template_manager();
        crate::gui::header_template::set_active_manager(&mut *manager);
        let result = manager.init();
        crate::gui::header_template::clear_active_manager();
        result
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = crate::gui::header_template::get_header_template_manager();
        manager.reset()?;
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(audio) = TheAudio::get() {
            audio.update();
        }
        Ok(())
    }
}

/// Lightweight window manager wrapper.
#[derive(Default)]
pub struct WindowManagerSubsystem;

impl WindowManagerSubsystem {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SubsystemInterface for WindowManagerSubsystem {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::window_manager::with_window_manager(|manager| manager.init());
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::window_manager::with_window_manager(|manager| manager.reset());
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::window_manager::with_window_manager(|manager| manager.update());
        Ok(())
    }
}

/// In-game UI subsystem bridge.
#[derive(Default)]
pub struct InGameUISubsystem {
    beacon_markers: Vec<BeaconMarker>,
    pending_beacon_events: VecDeque<BeaconNotification>,
    selection_events: VecDeque<SelectionEvent>,
    command_log: VecDeque<CommandLogEntry>,
    hud_messages: VecDeque<String>,
    military_subtitles: VecDeque<(String, i32)>,
    tooltips_disabled_until: u32,
    radar_pings: VecDeque<RadarPingEvent>,
    pending_place_template: Option<String>,
    pending_place_source_object_id: ObjectID,
    placement_start: Option<ICoord2D>,
    placement_end: Option<ICoord2D>,
    placement_angle: f32,
    radius_cursor_active: bool,
    radius_cursor_type: String,
    attack_move_to_mode: bool,
    force_attack_mode: bool,
    force_move_to_mode: bool,
    prefer_selection_mode: bool,
    waypoint_mode: bool,
    camera_rotating_left: bool,
    camera_rotating_right: bool,
    camera_zooming_in: bool,
    camera_zooming_out: bool,
    camera_tracking_drawable: bool,
    prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click: bool,
    pending_special_power: Option<PendingSpecialPower>,
    pending_command: Option<PendingCommand>,
}

impl InGameUISubsystem {
    fn map_cant_build_message(message: &str) -> String {
        let trimmed = message.trim();
        if trimmed.is_empty() {
            return "GUI:CantBuildThere".to_string();
        }
        if trimmed.starts_with("GUI:") {
            return trimmed.to_string();
        }

        let lower = trimmed.to_ascii_lowercase();
        if lower.contains("flat") {
            "GUI:CantBuildNotFlatEnough".to_string()
        } else if lower.contains("object") {
            "GUI:CantBuildObjectsInTheWay".to_string()
        } else if lower.contains("supply") {
            "GUI:CantBuildTooCloseToSupplies".to_string()
        } else if lower.contains("path") {
            "GUI:CantBuildNoClearPath".to_string()
        } else if lower.contains("shroud") || lower.contains("visible") {
            "GUI:CantBuildShroud".to_string()
        } else if lower.contains("terrain")
            || lower.contains("cliff")
            || lower.contains("underwater")
            || lower.contains("bridge")
        {
            "GUI:CantBuildRestrictedTerrain".to_string()
        } else {
            "GUI:CantBuildThere".to_string()
        }
    }

    fn beacon_distance_sq(a: &Coord3D, b: &Coord3D) -> f32 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        dx * dx + dy * dy + dz * dz
    }

    fn find_beacon_index(&self, player_id: i32, position: &Coord3D) -> Option<usize> {
        let threshold_sq = BEACON_MATCH_THRESHOLD * BEACON_MATCH_THRESHOLD;
        self.beacon_markers.iter().position(|marker| {
            marker.player_id == player_id
                && Self::beacon_distance_sq(&marker.position, position) <= threshold_sq
        })
    }

    fn upsert_beacon(&mut self, marker: BeaconMarker) {
        if let Some(index) = self.find_beacon_index(marker.player_id, &marker.position) {
            self.beacon_markers[index] = marker;
        } else {
            self.beacon_markers.push(marker);
        }
    }

    fn remove_beacon(&mut self, player_id: i32, position: &Coord3D) -> bool {
        if let Some(index) = self.find_beacon_index(player_id, position) {
            self.beacon_markers.remove(index);
            true
        } else {
            false
        }
    }

    /// Snapshot the current beacon markers for HUD/radar rendering.
    pub fn snapshot_beacons(&self) -> Vec<BeaconMarker> {
        self.beacon_markers.clone()
    }

    /// Drain notifications that higher-level UI components may transform into
    /// actual HUD messages.
    pub fn drain_beacon_events(&mut self) -> Vec<BeaconNotification> {
        self.pending_beacon_events.drain(..).collect()
    }

    pub fn drain_selection_events(&mut self) -> Vec<SelectionEvent> {
        self.selection_events.drain(..).collect()
    }

    pub fn drain_command_log(&mut self) -> Vec<CommandLogEntry> {
        self.command_log.drain(..).collect()
    }

    pub fn drain_hud_messages(&mut self) -> Vec<String> {
        self.hud_messages.drain(..).collect()
    }

    pub fn push_radar_ping(&mut self, ping: RadarPingEvent) {
        const MAX_PINGS: usize = 32;
        if self.radar_pings.len() >= MAX_PINGS {
            self.radar_pings.pop_front();
        }
        self.radar_pings.push_back(ping);
    }

    pub fn drain_radar_pings(&mut self) -> Vec<RadarPingEvent> {
        self.radar_pings.drain(..).collect()
    }

    fn record_selection(&mut self, upper_left: ICoord2D, lower_right: ICoord2D) {
        const MAX_SELECTION_EVENTS: usize = 32;
        if self.selection_events.len() == MAX_SELECTION_EVENTS {
            self.selection_events.pop_front();
        }
        self.selection_events.push_back(SelectionEvent {
            upper_left,
            lower_right,
        });
    }

    fn record_command(&mut self, entry: CommandLogEntry) {
        const MAX_COMMAND_EVENTS: usize = 64;
        if self.command_log.len() == MAX_COMMAND_EVENTS {
            self.command_log.pop_front();
        }
        self.command_log.push_back(entry);
    }

    fn push_hud_message(&mut self, message: String) {
        const MAX_HUD_MESSAGES: usize = 32;
        if self.hud_messages.len() == MAX_HUD_MESSAGES {
            self.hud_messages.pop_front();
        }
        self.hud_messages.push_back(message);
    }

    fn push_military_subtitle(&mut self, label: &str, duration_ms: i32) {
        const MAX_MILITARY_SUBTITLES: usize = 8;
        if self.military_subtitles.len() == MAX_MILITARY_SUBTITLES {
            self.military_subtitles.pop_front();
        }
        self.military_subtitles
            .push_back((label.to_string(), duration_ms));
        let duration_frames = ((duration_ms.max(0) as u32).saturating_mul(30)) / 1000;
        self.disable_tooltips_until(TheGameLogic::get_frame().saturating_add(duration_frames));
    }

    fn disable_tooltips_until(&mut self, frame_num: u32) {
        if frame_num > self.tooltips_disabled_until {
            self.tooltips_disabled_until = frame_num;
        }
    }

    fn clear_tooltips_disabled(&mut self) {
        self.tooltips_disabled_until = 0;
    }

    fn are_tooltips_disabled(&self) -> bool {
        TheGameLogic::get_frame() < self.tooltips_disabled_until
    }

    fn play_radar_movie(&mut self, movie_name: &str) -> bool {
        let target_window = [
            // C++ used this window name historically.
            "ControlBar.wnd:CameoMovieWindow",
            // Current layouts route portrait/radar media through RightHUD.
            "ControlBar.wnd:RightHUD",
        ]
        .into_iter()
        .find_map(|window_name| {
            let window_id = NameKeyGenerator::name_to_key(window_name) as i32;
            crate::gui::with_window_manager_ref(|manager| manager.get_window_by_id(window_id))
        });

        let Some(window) = target_window else {
            return false;
        };

        with_window_video_manager(|manager| {
            manager.play_movie(window, movie_name.to_string(), WindowVideoPlayType::Once)
        })
    }

    fn update_radar_movie_playback(&mut self) {
        with_window_video_manager(|manager| manager.update());
    }

    fn is_radar_movie_playing(&self, movie_name: &str) -> bool {
        with_window_video_manager(|manager| manager.is_movie_playing(movie_name))
    }

    fn get_pending_place_template(&self) -> Option<String> {
        self.pending_place_template.clone()
    }

    fn get_pending_place_source_object_id(&self) -> ObjectID {
        self.pending_place_source_object_id
    }

    fn set_pending_place(
        &mut self,
        template_name: Option<String>,
        source_object_id: Option<ObjectID>,
    ) {
        self.pending_place_template = template_name;
        self.pending_place_source_object_id = source_object_id.unwrap_or(0);
        self.placement_start = None;
        self.placement_end = None;
        self.placement_angle = 0.0;
    }

    fn get_pending_special_power(&self) -> Option<PendingSpecialPower> {
        self.pending_special_power.clone()
    }

    fn set_pending_special_power(&mut self, pending: Option<PendingSpecialPower>) {
        self.pending_special_power = pending;
    }

    fn clear_pending_special_power(&mut self) {
        self.pending_special_power = None;
    }

    fn get_pending_command(&self) -> Option<PendingCommand> {
        self.pending_command.clone()
    }

    fn set_pending_command(&mut self, pending: Option<PendingCommand>) {
        self.pending_command = pending;
    }

    fn clear_pending_command(&mut self) {
        self.pending_command = None;
    }

    fn is_placement_anchored(&self) -> bool {
        self.placement_start.is_some()
    }

    fn set_placement_start(&mut self, start: Option<ICoord2D>) {
        self.placement_start = start.clone();
        if start.is_none() {
            self.placement_end = None;
        } else if self.placement_end.is_none() {
            self.placement_end = start;
        }
    }

    fn set_placement_end(&mut self, end: Option<ICoord2D>) {
        self.placement_end = end;
    }

    fn get_placement_points(&self) -> Option<(ICoord2D, ICoord2D)> {
        let start = self.placement_start.clone()?;
        let end = self.placement_end.clone().unwrap_or_else(|| start.clone());
        Some((start, end))
    }

    fn get_placement_angle(&self) -> f32 {
        self.placement_angle
    }

    fn set_placement_angle(&mut self, angle: f32) {
        self.placement_angle = angle;
    }

    fn set_radius_cursor_active(&mut self, radius_cursor_type: Option<String>) {
        match radius_cursor_type {
            Some(radius_cursor_type) => {
                let radius_type = radius_cursor_type.trim().to_string();
                self.radius_cursor_active =
                    !radius_type.is_empty() && !radius_type.eq_ignore_ascii_case("NONE");
                self.radius_cursor_type = radius_type;
            }
            None => {
                self.radius_cursor_active = true;
                self.radius_cursor_type.clear();
            }
        }
    }

    fn set_radius_cursor_none(&mut self) {
        self.radius_cursor_active = false;
        self.radius_cursor_type.clear();
    }

    fn display_cant_build_message(&mut self, message: &str) {
        let key = Self::map_cant_build_message(message);
        self.message(&key);
    }

    fn message(&mut self, text: &str) {
        self.push_hud_message(GameText::fetch(text));
    }

    fn clear_attack_move_to_mode(&mut self) {
        self.attack_move_to_mode = false;
    }

    fn is_in_attack_move_to_mode(&self) -> bool {
        self.attack_move_to_mode
    }

    fn set_attack_move_to_mode(&mut self, enabled: bool) {
        self.attack_move_to_mode = enabled;
    }

    fn is_in_force_attack_mode(&self) -> bool {
        self.force_attack_mode
    }

    fn is_in_force_move_to_mode(&self) -> bool {
        self.force_move_to_mode
    }

    fn is_in_prefer_selection_mode(&self) -> bool {
        self.prefer_selection_mode
    }

    fn set_force_attack_mode(&mut self, enabled: bool) {
        self.force_attack_mode = enabled;
    }

    fn set_force_move_to_mode(&mut self, enabled: bool) {
        self.force_move_to_mode = enabled;
    }

    fn set_prefer_selection_mode(&mut self, enabled: bool) {
        self.prefer_selection_mode = enabled;
    }

    fn is_in_waypoint_mode(&self) -> bool {
        self.waypoint_mode
    }

    fn set_waypoint_mode(&mut self, enabled: bool) {
        self.waypoint_mode = enabled;
    }

    fn is_camera_rotating_left(&self) -> bool {
        self.camera_rotating_left
    }

    fn set_camera_rotate_left(&mut self, set: bool) {
        self.camera_rotating_left = set;
    }

    fn is_camera_rotating_right(&self) -> bool {
        self.camera_rotating_right
    }

    fn set_camera_rotate_right(&mut self, set: bool) {
        self.camera_rotating_right = set;
    }

    fn is_camera_zooming_in(&self) -> bool {
        self.camera_zooming_in
    }

    fn set_camera_zoom_in(&mut self, set: bool) {
        self.camera_zooming_in = set;
    }

    fn is_camera_zooming_out(&self) -> bool {
        self.camera_zooming_out
    }

    fn set_camera_zoom_out(&mut self, set: bool) {
        self.camera_zooming_out = set;
    }

    fn is_camera_tracking_drawable(&self) -> bool {
        self.camera_tracking_drawable
    }

    fn set_camera_tracking_drawable(&mut self, set: bool) {
        self.camera_tracking_drawable = set;
    }

    fn get_frame_selection_changed(&self) -> u32 {
        0
    }

    fn set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
        &mut self,
        enabled: bool,
    ) {
        self.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click = enabled;
    }

    fn get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(&self) -> bool {
        self.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click
    }

    fn clear_runtime_state(&mut self) {
        self.beacon_markers.clear();
        self.pending_beacon_events.clear();
        self.selection_events.clear();
        self.command_log.clear();
        self.hud_messages.clear();
        self.military_subtitles.clear();
        self.tooltips_disabled_until = 0;
        self.radar_pings.clear();
        self.pending_place_template = None;
        self.pending_place_source_object_id = 0;
        self.placement_start = None;
        self.placement_end = None;
        self.placement_angle = 0.0;
        self.radius_cursor_active = false;
        self.radius_cursor_type.clear();
        self.attack_move_to_mode = false;
        self.force_attack_mode = false;
        self.force_move_to_mode = false;
        self.prefer_selection_mode = false;
        self.waypoint_mode = false;
        self.camera_rotating_left = false;
        self.camera_rotating_right = false;
        self.camera_zooming_in = false;
        self.camera_zooming_out = false;
        self.camera_tracking_drawable = false;
        self.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click = false;
        self.pending_special_power = None;
        self.pending_command = None;
    }
}

impl SubsystemInterface for InGameUISubsystem {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        with_window_video_manager(|manager| manager.init());
        self.clear_runtime_state();
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        with_window_video_manager(|manager| manager.reset());
        self.clear_runtime_state();
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_radar_movie_playback();
        Ok(())
    }
}

impl InGameUI for InGameUISubsystem {
    fn disregard_drawable(
        &self,
        _drawable: &dyn Drawable,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn handle_beacon_notification(
        &mut self,
        notification: &BeaconNotification,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.pending_beacon_events.push_back(notification.clone());

        match notification {
            BeaconNotification::Placed(marker) => {
                self.upsert_beacon(marker.clone());
                let text = marker
                    .text
                    .as_deref()
                    .map(|t| format!(" '{t}'"))
                    .unwrap_or_default();
                let msg = format!(
                    "Beacon placed by player {} at ({:.1}, {:.1}, {:.1}){}",
                    marker.player_id, marker.position.x, marker.position.y, marker.position.z, text
                );
                log::info!("{msg}");
                self.push_hud_message(msg);
            }
            BeaconNotification::Removed {
                player_id,
                position,
            } => {
                let removed = self.remove_beacon(*player_id, position);
                let msg = if removed {
                    format!(
                        "Beacon removed for player {} near ({:.1}, {:.1}, {:.1})",
                        player_id, position.x, position.y, position.z
                    )
                } else {
                    format!(
                        "Beacon remove notification without matching marker (player {}, position {:.1},{:.1},{:.1})",
                        player_id, position.x, position.y, position.z
                    )
                };
                if removed {
                    log::info!("{msg}");
                } else {
                    log::warn!("{msg}");
                }
                self.push_hud_message(msg);
            }
            BeaconNotification::TextUpdated {
                player_id,
                position,
                text,
            } => {
                if let Some(index) = self.find_beacon_index(*player_id, position) {
                    self.beacon_markers[index].text = Some(text.clone());
                } else {
                    log::warn!(
                        "Beacon text update without marker (player {}, position {:.1},{:.1},{:.1})",
                        player_id,
                        position.x,
                        position.y,
                        position.z
                    );
                }
                let msg = format!(
                    "Beacon text updated for player {} near ({:.1}, {:.1}, {:.1}): {}",
                    player_id, position.x, position.y, position.z, text
                );
                log::info!("{msg}");
                self.push_hud_message(msg);
            }
        }
        Ok(())
    }
}

/// Represents a marquee selection performed by the player.
#[derive(Debug, Clone)]
pub struct SelectionEvent {
    pub upper_left: ICoord2D,
    pub lower_right: ICoord2D,
}

/// High-level command log derived from the player's UI interactions.
#[derive(Debug, Clone)]
pub enum CommandLogEntry {
    Move { position: Coord3D, queued: bool },
    ForceAttackGround { position: Coord3D },
    Attack { target_id: u32, queued: bool },
    Stop,
}

/// Simplified radar ping event forwarded to HUD/minimap layers.
#[derive(Debug, Clone)]
pub struct RadarPingEvent {
    pub position: Coord3D,
    pub kind: RadarPingKind,
    pub age_seconds: f32,
}

#[derive(Debug, Clone, Copy)]
pub enum RadarPingKind {
    Generic,
    Attack,
    Ally,
}

/// Thin handle that exposes the in‑game UI subsystem through the legacy
/// `TheInGameUI` facade.
#[derive(Clone)]
pub struct InGameUiHandle {
    inner: Arc<Mutex<InGameUISubsystem>>,
}

impl InGameUiHandle {
    pub fn new(inner: Arc<Mutex<InGameUISubsystem>>) -> Self {
        Self { inner }
    }
}

impl InGameUiHooks for InGameUiHandle {
    fn select_area(&self, upper_left: ICoord2D, lower_right: ICoord2D) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.record_selection(upper_left, lower_right);
        }
    }

    fn issue_move_command(&self, position: Coord3D, queue: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.record_command(CommandLogEntry::Move {
                position,
                queued: queue,
            });
        }
    }

    fn issue_force_attack_ground(&self, position: Coord3D) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.record_command(CommandLogEntry::ForceAttackGround { position });
        }
    }

    fn issue_attack_command(&self, target: u32, queue: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.record_command(CommandLogEntry::Attack {
                target_id: target,
                queued: queue,
            });
        }
    }

    fn issue_stop_command(&self) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.record_command(CommandLogEntry::Stop);
        }
    }

    fn set_hint_text(&self, hint: &str) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.push_hud_message(hint.to_string());
        }
    }

    fn get_pending_place_template(&self) -> Option<String> {
        self.inner
            .lock()
            .ok()
            .and_then(|ui| ui.get_pending_place_template())
    }

    fn get_pending_place_source_object_id(&self) -> u32 {
        self.inner
            .lock()
            .map(|ui| ui.get_pending_place_source_object_id())
            .unwrap_or(0)
    }

    fn set_pending_place(&self, template_name: Option<String>, source_object_id: Option<u32>) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_pending_place(template_name, source_object_id);
        }
    }

    fn get_pending_special_power(&self) -> Option<PendingSpecialPower> {
        self.inner
            .lock()
            .ok()
            .and_then(|ui| ui.get_pending_special_power())
    }

    fn set_pending_special_power(&self, pending: Option<PendingSpecialPower>) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_pending_special_power(pending);
        }
    }

    fn clear_pending_special_power(&self) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.clear_pending_special_power();
        }
    }

    fn get_pending_command(&self) -> Option<PendingCommand> {
        self.inner
            .lock()
            .ok()
            .and_then(|ui| ui.get_pending_command())
    }

    fn set_pending_command(&self, pending: Option<PendingCommand>) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_pending_command(pending);
        }
    }

    fn clear_pending_command(&self) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.clear_pending_command();
        }
    }

    fn is_placement_anchored(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_placement_anchored())
            .unwrap_or(false)
    }

    fn set_placement_start(&self, start: Option<ICoord2D>) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_placement_start(start);
        }
    }

    fn set_placement_end(&self, end: Option<ICoord2D>) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_placement_end(end);
        }
    }

    fn get_placement_points(&self) -> Option<(ICoord2D, ICoord2D)> {
        self.inner
            .lock()
            .ok()
            .and_then(|ui| ui.get_placement_points())
    }

    fn get_placement_angle(&self) -> f32 {
        self.inner
            .lock()
            .map(|ui| ui.get_placement_angle())
            .unwrap_or(0.0)
    }

    fn set_placement_angle(&self, angle: f32) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_placement_angle(angle);
        }
    }

    fn set_radius_cursor_none(&self) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_radius_cursor_none();
        }
    }

    fn set_radius_cursor_active(&self, radius_cursor_type: Option<String>) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_radius_cursor_active(radius_cursor_type);
        }
    }

    fn display_cant_build_message(&self, message: &str) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.display_cant_build_message(message);
        }
    }

    fn message(&self, text: &str) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.message(text);
        }
    }

    fn military_subtitle(&self, label: &str, duration_ms: i32) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.push_military_subtitle(label, duration_ms);
        }
    }

    fn disable_tooltips_until(&self, frame_num: u32) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.disable_tooltips_until(frame_num);
        }
    }

    fn clear_tooltips_disabled(&self) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.clear_tooltips_disabled();
        }
    }

    fn are_tooltips_disabled(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.are_tooltips_disabled())
            .unwrap_or(false)
    }

    fn clear_attack_move_to_mode(&self) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.clear_attack_move_to_mode();
        }
    }

    fn is_in_attack_move_to_mode(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_in_attack_move_to_mode())
            .unwrap_or(false)
    }

    fn set_attack_move_to_mode(&self, enabled: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_attack_move_to_mode(enabled);
        }
    }

    fn is_in_force_attack_mode(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_in_force_attack_mode())
            .unwrap_or(false)
    }

    fn is_in_force_move_to_mode(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_in_force_move_to_mode())
            .unwrap_or(false)
    }

    fn is_in_prefer_selection_mode(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_in_prefer_selection_mode())
            .unwrap_or(false)
    }

    fn set_force_attack_mode(&self, enabled: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_force_attack_mode(enabled);
        }
    }

    fn set_force_move_to_mode(&self, enabled: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_force_move_to_mode(enabled);
        }
    }

    fn set_prefer_selection_mode(&self, enabled: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_prefer_selection_mode(enabled);
        }
    }

    fn is_in_waypoint_mode(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_in_waypoint_mode())
            .unwrap_or(false)
    }

    fn set_waypoint_mode(&self, enabled: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_waypoint_mode(enabled);
        }
    }

    fn is_camera_rotating_left(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_camera_rotating_left())
            .unwrap_or(false)
    }

    fn set_camera_rotate_left(&self, set: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_camera_rotate_left(set);
        }
    }

    fn is_camera_rotating_right(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_camera_rotating_right())
            .unwrap_or(false)
    }

    fn set_camera_rotate_right(&self, set: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_camera_rotate_right(set);
        }
    }

    fn is_camera_zooming_in(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_camera_zooming_in())
            .unwrap_or(false)
    }

    fn set_camera_zoom_in(&self, set: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_camera_zoom_in(set);
        }
    }

    fn is_camera_zooming_out(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_camera_zooming_out())
            .unwrap_or(false)
    }

    fn set_camera_zoom_out(&self, set: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_camera_zoom_out(set);
        }
    }

    fn is_camera_tracking_drawable(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_camera_tracking_drawable())
            .unwrap_or(false)
    }

    fn set_camera_tracking_drawable(&self, set: bool) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_camera_tracking_drawable(set);
        }
    }

    fn get_frame_selection_changed(&self) -> u32 {
        self.inner
            .lock()
            .map(|ui| ui.get_frame_selection_changed())
            .unwrap_or(0)
    }

    fn set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
        &self,
        enabled: bool,
    ) {
        if let Ok(mut ui) = self.inner.lock() {
            ui.set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(enabled);
        }
    }

    fn get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(&self) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click())
            .unwrap_or(false)
    }

    fn play_movie(&self, movie_name: &str) -> bool {
        self.inner
            .lock()
            .map(|mut ui| ui.play_radar_movie(movie_name))
            .unwrap_or(false)
    }

    fn is_movie_playing(&self, movie_name: &str) -> bool {
        self.inner
            .lock()
            .map(|ui| ui.is_radar_movie_playing(movie_name))
            .unwrap_or(false)
    }
}

/// Audio subsystem backed by Kira.
pub struct AudioSubsystem {
    manager: Mutex<AudioManager<kira::manager::backend::DefaultBackend>>,
    debug_state: Mutex<AudioDebugState>,
}

impl AudioSubsystem {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let manager = AudioManager::new(AudioManagerSettings::default())?;
        Ok(Self {
            manager: Mutex::new(manager),
            debug_state: Mutex::new(AudioDebugState::new()),
        })
    }

    pub fn manager(
        &self,
    ) -> std::sync::MutexGuard<'_, AudioManager<kira::manager::backend::DefaultBackend>> {
        self.manager.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn debug_snapshot(&self) -> AudioDebugSnapshot {
        let state = self.debug_state.lock().unwrap_or_else(|e| e.into_inner());
        AudioDebugSnapshot {
            total_events: state.total_events,
            recent_events: state.recent_events.iter().cloned().collect(),
        }
    }

    fn record_event(&self, event: &str, position: Option<Coord3D>) {
        let mut state = self.debug_state.lock().unwrap_or_else(|e| e.into_inner());
        state.total_events = state.total_events.saturating_add(1);
        let timestamp_ms = state.start_time.elapsed().as_millis() as u64;
        state.recent_events.push_back(AudioDebugRecord {
            name: event.to_string(),
            position,
            timestamp_ms,
        });
        if state.recent_events.len() > MAX_AUDIO_DEBUG_EVENTS {
            state.recent_events.pop_front();
        }
    }
}

impl SubsystemInterface for AudioSubsystem {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // Nothing to do – the manager is ready after construction.
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(audio) = TheAudio::get() {
            audio.update();
        }
        Ok(())
    }
}

impl crate::audio::GameAudio for AudioSubsystem {
    fn play_event(
        &mut self,
        event: &str,
        position: Option<Coord3D>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.record_event(event, position.clone());
        let translated = translate_audio_event(event);
        let mut audio_event = LogicAudioEventRts::new(translated);
        if let Some(pos) = position.as_ref() {
            audio_event.set_position(&(pos.x, pos.y, pos.z));
        }

        if let Some(audio) = TheAudio::get() {
            let handle = audio.add_audio_event(&audio_event);
            audio_event.set_playing_handle(handle);
        } else {
            match position.as_ref() {
                Some(pos) => log::debug!(
                    "AudioSubsystem::play_event: {} @ ({:.1}, {:.1}, {:.1}) [no audio manager]",
                    translated,
                    pos.x,
                    pos.y,
                    pos.z
                ),
                None => log::debug!(
                    "AudioSubsystem::play_event: {} [no audio manager]",
                    translated
                ),
            }
        }

        // Hold the manager guard briefly to mirror the C++ audio accessor pattern.
        let _guard = self.manager();
        Ok(())
    }
}

const MAX_AUDIO_DEBUG_EVENTS: usize = 32;

#[derive(Clone)]
pub struct AudioDebugRecord {
    pub name: String,
    pub position: Option<Coord3D>,
    pub timestamp_ms: u64,
}

pub struct AudioDebugSnapshot {
    pub total_events: u64,
    pub recent_events: Vec<AudioDebugRecord>,
}

struct AudioDebugState {
    start_time: Instant,
    total_events: u64,
    recent_events: VecDeque<AudioDebugRecord>,
}

impl AudioDebugState {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_events: 0,
            recent_events: VecDeque::new(),
        }
    }
}

/// Map high-level cues into concrete EVA/UI audio event ids used by the client.
fn translate_audio_event(event: &str) -> &str {
    match event {
        "EVA_BeaconPlaced" => "UI_BeaconPlaced",
        "EVA_BeaconRemoved" => "UI_BeaconRemoved",
        "Radar_Event" => "UI_RadarEvent",
        "Radar_Attack" => "UI_RadarAttack",
        "Radar_Ally" => "UI_RadarAllyRequest",
        "Radar_BaseAttacked" => "UI_RadarAttack",
        "Radar_EnemyDetected" => "UI_RadarEvent",
        "Radar_UnitCreated" => "UI_RadarEvent",
        "Radar_UnitDestroyed" => "UI_RadarEvent",
        other => other,
    }
}

/// Terrain visual bridge that implements the legacy trait.
#[derive(Default)]
pub struct TerrainVisualStub {
    registered_trees: HashMap<u32, TerrainTreeRegistration>,
}

impl TerrainVisualStub {
    pub fn add_tree_registration(&mut self, tree: TerrainTreeRegistration) {
        self.registered_trees.insert(tree.drawable_id, tree);
    }

    pub fn remove_tree_registration(&mut self, drawable_id: u32) {
        self.registered_trees.remove(&drawable_id);
    }

    pub fn tree_registrations(&self) -> Vec<TerrainTreeRegistration> {
        self.registered_trees.values().cloned().collect()
    }
}

impl SubsystemInterface for TerrainVisualStub {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(mut terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if terrain_guard.is_none() {
                *terrain_guard = Some(crate::terrain::terrain_visual::TerrainVisualSystem::new());
            }
            if let Some(terrain) = terrain_guard.as_mut() {
                terrain.init()?;
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.registered_trees.clear();
        if let Ok(mut terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_mut() {
                terrain.reset()?;
            }
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(mut terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_mut() {
                terrain.update()?;
            }
        }
        Ok(())
    }
}

impl TerrainVisual for TerrainVisualStub {
    fn render(&mut self, view_matrix: &Mat4, projection_matrix: &Mat4) -> Result<(), TerrainError> {
        if let Ok(mut terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_mut() {
                return terrain.render(view_matrix, projection_matrix);
            }
        }
        Ok(())
    }

    fn get_height_at(&self, x: f32, y: f32) -> Result<f32, TerrainError> {
        if let Ok(terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_ref() {
                return terrain.get_height_at(x, y);
            }
        }
        Ok(0.0)
    }

    fn get_normal_at(&self, x: f32, y: f32) -> Result<Vec3, TerrainError> {
        if let Ok(terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_ref() {
                return terrain.get_normal_at(x, y);
            }
        }
        Ok(Vec3::Z)
    }

    fn is_valid_position(&self, x: f32, y: f32) -> bool {
        if let Ok(terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_ref() {
                return terrain.is_valid_position(x, y);
            }
        }
        x.is_finite() && y.is_finite()
    }

    fn chunk_manager(&self) -> &crate::terrain::chunk::ChunkManager {
        static EMPTY: once_cell::sync::Lazy<crate::terrain::chunk::ChunkManager> =
            once_cell::sync::Lazy::new(crate::terrain::chunk::ChunkManager::new);
        &EMPTY
    }

    fn chunk_draw_count(&self) -> usize {
        if let Ok(terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_ref() {
                return terrain.chunk_draw_count();
            }
        }
        0
    }

    fn oversize_terrain(&mut self, amount: i32) {
        if let Ok(mut terrain_guard) = crate::terrain::terrain_visual::get_terrain_visual() {
            if let Some(terrain) = terrain_guard.as_mut() {
                terrain.oversize_terrain(amount);
            }
        }
    }
}

/// Video player subsystem state.
#[derive(Default)]
pub struct VideoPlayerSubsystem;

impl SubsystemInterface for VideoPlayerSubsystem {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        init_video_player();
        if let Some(player) = get_video_player() {
            if let Ok(mut guard) = player.lock() {
                if let Some(player) = guard.as_mut() {
                    player.init();
                }
            }
        }
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        with_window_video_manager(|manager| manager.reset());
        if let Some(player) = get_video_player() {
            if let Ok(mut guard) = player.lock() {
                if let Some(player) = guard.as_mut() {
                    player.reset();
                }
            }
        }
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(player) = get_video_player() {
            if let Ok(mut guard) = player.lock() {
                if let Some(player) = guard.as_mut() {
                    player.update();
                }
            }
        }
        Ok(())
    }
}

impl VideoPlayerInterface for VideoPlayerSubsystem {}

pub type KeyboardHandle = Arc<Mutex<crate::input::Keyboard>>;
pub type MouseHandle = Arc<Mutex<crate::input::Mouse>>;

pub fn create_keyboard() -> KeyboardHandle {
    Arc::new(Mutex::new(crate::input::Keyboard::new()))
}

pub fn create_mouse() -> MouseHandle {
    Arc::new(Mutex::new(crate::input::Mouse::new()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::commands::command::CommandType;

    #[test]
    fn terrain_visual_bridge_fallback_normal_is_z_up_like_cpp_world() {
        let terrain = TerrainVisualStub::default();

        assert_eq!(terrain.get_normal_at(10.0, 20.0).unwrap(), Vec3::Z);
    }

    #[test]
    fn in_game_ui_reset_clears_transient_state() {
        let mut ui = InGameUISubsystem::default();
        ui.beacon_markers.push(BeaconMarker {
            player_id: 1,
            position: Coord3D::new(10.0, 20.0, 0.0),
            text: Some("Beacon".to_string()),
        });
        ui.pending_beacon_events
            .push_back(BeaconNotification::Removed {
                player_id: 1,
                position: Coord3D::new(10.0, 20.0, 0.0),
            });
        ui.selection_events.push_back(SelectionEvent {
            upper_left: ICoord2D::new(1, 2),
            lower_right: ICoord2D::new(3, 4),
        });
        ui.command_log.push_back(CommandLogEntry::Stop);
        ui.hud_messages.push_back("hello".to_string());
        ui.military_subtitles
            .push_back(("SCRIPT:Caption".to_string(), 2000));
        ui.tooltips_disabled_until = 99;
        ui.radar_pings.push_back(RadarPingEvent {
            position: Coord3D::new(5.0, 6.0, 0.0),
            kind: RadarPingKind::Generic,
            age_seconds: 1.0,
        });
        ui.pending_place_template = Some("SomeBuilding".to_string());
        ui.pending_place_source_object_id = 77;
        ui.placement_start = Some(ICoord2D::new(9, 9));
        ui.placement_end = Some(ICoord2D::new(12, 12));
        ui.placement_angle = 45.0;
        ui.radius_cursor_active = true;
        ui.attack_move_to_mode = true;
        ui.force_attack_mode = true;
        ui.force_move_to_mode = true;
        ui.prefer_selection_mode = true;
        ui.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click = true;
        ui.pending_special_power = Some(PendingSpecialPower {
            power_id: 11,
            options: 12,
            source_object_id: 13,
        });
        ui.pending_command = Some(PendingCommand {
            command_type: CommandType::Invalid,
            options: 22,
            source_object_id: 23,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        });

        ui.reset().unwrap();

        assert!(ui.beacon_markers.is_empty());
        assert!(ui.pending_beacon_events.is_empty());
        assert!(ui.selection_events.is_empty());
        assert!(ui.command_log.is_empty());
        assert!(ui.hud_messages.is_empty());
        assert!(ui.military_subtitles.is_empty());
        assert_eq!(ui.tooltips_disabled_until, 0);
        assert!(ui.radar_pings.is_empty());
        assert!(ui.pending_place_template.is_none());
        assert_eq!(ui.pending_place_source_object_id, 0);
        assert!(ui.placement_start.is_none());
        assert!(ui.placement_end.is_none());
        assert_eq!(ui.placement_angle, 0.0);
        assert!(!ui.radius_cursor_active);
        assert!(!ui.attack_move_to_mode);
        assert!(!ui.force_attack_mode);
        assert!(!ui.force_move_to_mode);
        assert!(!ui.prefer_selection_mode);
        assert!(!ui.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click);
        assert!(ui.pending_special_power.is_none());
        assert!(ui.pending_command.is_none());
    }

    #[test]
    fn in_game_ui_radius_cursor_type_state_transitions() {
        let mut ui = InGameUISubsystem::default();

        ui.set_radius_cursor_active(Some("   ".to_string()));
        assert!(!ui.radius_cursor_active);
        assert_eq!(ui.radius_cursor_type, "");

        ui.set_radius_cursor_active(Some("NONE".to_string()));
        assert!(!ui.radius_cursor_active);
        assert_eq!(ui.radius_cursor_type, "NONE");

        ui.set_radius_cursor_active(Some("GUARD_AREA".to_string()));
        assert!(ui.radius_cursor_active);
        assert_eq!(ui.radius_cursor_type, "GUARD_AREA");

        ui.set_radius_cursor_none();
        assert!(!ui.radius_cursor_active);
        assert_eq!(ui.radius_cursor_type, "");
    }

    #[test]
    fn in_game_ui_military_subtitle_disables_tooltips_until_lifetime() {
        let mut ui = InGameUISubsystem::default();
        let current_frame = TheGameLogic::get_frame();

        ui.push_military_subtitle("SCRIPT:Briefing", 2500);

        assert_eq!(ui.military_subtitles.len(), 1);
        assert_eq!(ui.tooltips_disabled_until, current_frame.saturating_add(75));
        assert!(ui.are_tooltips_disabled());

        ui.clear_tooltips_disabled();
        assert_eq!(ui.tooltips_disabled_until, 0);
        assert!(!ui.are_tooltips_disabled());
    }

    #[test]
    fn in_game_ui_handle_records_military_subtitles_separately_from_hud_messages() {
        let ui = Arc::new(Mutex::new(InGameUISubsystem::default()));
        let handle = InGameUiHandle::new(ui.clone());

        handle.military_subtitle("SCRIPT:Caption", 2500);

        let guard = ui.lock().unwrap();
        assert_eq!(
            guard.military_subtitles.front(),
            Some(&("SCRIPT:Caption".to_string(), 2500))
        );
        assert!(guard.hud_messages.is_empty());
    }
}
