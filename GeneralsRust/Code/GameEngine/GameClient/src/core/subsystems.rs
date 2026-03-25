//! Lightweight subsystem helpers used by the GameClient.  These implementations
//! provide enough behaviour for non-platform builds while keeping dependencies
//! minimal.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::{
    drawable::Drawable,
    game_text::GameText,
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
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::common::audio::AudioEventRts as LogicAudioEventRts;
use gamelogic::helpers::{TerrainTreeRegistration, TheAudio, TheScriptEngine};
use glam::{Mat4, Vec3};
use kira::manager::{AudioManager, AudioManagerSettings};

use crate::core::game_client::{InGameUI, VideoPlayerInterface};

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
        let mut manager = crate::gui::header_template::get_header_template_manager();
        manager.init()?;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut manager = crate::gui::header_template::get_header_template_manager();
        manager.reset()?;
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
    radar_pings: VecDeque<RadarPingEvent>,
    pending_place_template: Option<String>,
    pending_place_source_object_id: ObjectID,
    placement_start: Option<ICoord2D>,
    placement_end: Option<ICoord2D>,
    placement_angle: f32,
    radius_cursor_active: bool,
    attack_move_to_mode: bool,
    force_attack_mode: bool,
    force_move_to_mode: bool,
    prefer_selection_mode: bool,
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

    fn set_radius_cursor_none(&mut self) {
        self.radius_cursor_active = false;
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

    fn clear_runtime_state(&mut self) {
        self.beacon_markers.clear();
        self.pending_beacon_events.clear();
        self.selection_events.clear();
        self.command_log.clear();
        self.hud_messages.clear();
        self.radar_pings.clear();
        self.pending_place_template = None;
        self.pending_place_source_object_id = 0;
        self.placement_start = None;
        self.placement_end = None;
        self.placement_angle = 0.0;
        self.radius_cursor_active = false;
        self.attack_move_to_mode = false;
        self.force_attack_mode = false;
        self.force_move_to_mode = false;
        self.prefer_selection_mode = false;
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
        self.manager.lock().unwrap()
    }

    pub fn debug_snapshot(&self) -> AudioDebugSnapshot {
        let state = self.debug_state.lock().unwrap();
        AudioDebugSnapshot {
            total_events: state.total_events,
            recent_events: state.recent_events.iter().cloned().collect(),
        }
    }

    fn record_event(&self, event: &str, position: Option<Coord3D>) {
        let mut state = self.debug_state.lock().unwrap();
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
        Ok(Vec3::Y)
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
        ui.pending_special_power = Some(PendingSpecialPower {
            power_id: 11,
            options: 12,
            source_object_id: 13,
        });
        ui.pending_command = Some(PendingCommand {
            command_type: CommandType::Invalid,
            options: 22,
            source_object_id: 23,
        });

        ui.reset().unwrap();

        assert!(ui.beacon_markers.is_empty());
        assert!(ui.pending_beacon_events.is_empty());
        assert!(ui.selection_events.is_empty());
        assert!(ui.command_log.is_empty());
        assert!(ui.hud_messages.is_empty());
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
        assert!(ui.pending_special_power.is_none());
        assert!(ui.pending_command.is_none());
    }
}
