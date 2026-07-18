use crate::game_logic::{
    victory::{PlayerOutcome, VictorySummary},
    ObjectId,
};
use crate::graphics::MinimapCoordinates;
use crate::localization;
use crate::ui::objectives::ObjectiveDisplay;
use glam::Vec3;
#[cfg(feature = "integration-diagnostics")]
use integration::diagnostics::SystemDiagnostics;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl UiColor {
    pub const fn from_rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }
}

pub fn color_for_player(index: u8) -> UiColor {
    const COLORS: [UiColor; 8] = [
        UiColor::from_rgb(255, 0, 0),
        UiColor::from_rgb(50, 160, 255),
        UiColor::from_rgb(80, 200, 120),
        UiColor::from_rgb(255, 255, 0),
        UiColor::from_rgb(255, 120, 0),
        UiColor::from_rgb(200, 80, 255),
        UiColor::from_rgb(255, 255, 255),
        UiColor::from_rgb(120, 120, 120),
    ];
    COLORS[(index as usize) % COLORS.len()]
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiPos2 {
    pub x: f32,
    pub y: f32,
}

impl UiPos2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiVec2 {
    pub x: f32,
    pub y: f32,
}

impl UiVec2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const fn splat(value: f32) -> Self {
        Self { x: value, y: value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct UiRect {
    pub min: UiPos2,
    pub max: UiPos2,
}

impl UiRect {
    pub const fn from_min_max(min: UiPos2, max: UiPos2) -> Self {
        Self { min, max }
    }

    pub fn from_min_size(min: UiPos2, size: UiVec2) -> Self {
        Self {
            min,
            max: UiPos2::new(min.x + size.x, min.y + size.y),
        }
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiTextureId {
    Managed(u64),
    User(u64),
}

/// Presentation-fed PublicTimer superweapon residual (InGameUI countdown).
#[derive(Debug, Clone, PartialEq)]
pub struct UiSuperweaponTimer {
    pub name: String,
    pub template_name: String,
    pub icon: String,
    pub recharge_time: f32,
    pub remaining: f32,
    pub unlocked: bool,
    pub ready: bool,
}

/// ControlBar CanMake residual UI freeze.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CanMakeCameoUi {
    pub template_name: String,
    pub can_make: u32,
    pub available: bool,
    pub help_status: Option<String>,
}

/// UI State extracted from game logic.
#[derive(Debug, Clone)]
pub struct GameUIState {
    pub credits: i32,
    pub power_generated: i32,
    pub power_used: i32,
    pub max_power: i32,
    pub credits_per_second: f32,
    pub player_id: u32,
    pub player_name: String,
    /// Generals rank residual (1-based).
    pub rank_level: u32,
    /// GeneralsExperience skill points residual.
    pub skill_points: i32,
    /// Remaining science purchase points residual.
    pub science_purchase_points: i32,
    /// ControlBar rank bar 0..100 residual.
    pub rank_progress_percent: i32,
    /// PublicTimer superweapon countdown residual (presentation-fed).
    pub can_make_cameos: Vec<CanMakeCameoUi>,
    pub can_make_producer_id: Option<u32>,
    pub superweapon_timers: Vec<UiSuperweaponTimer>,
    pub selected_units: Vec<ObjectId>,
    pub selected_unit_infos: Vec<UnitDisplayInfo>,
    /// ControlBar/WND selection panel (health/name) from PresentationFrame when available.
    pub selection_panel: ControlBarSelectionPanelState,
    pub build_queue: Vec<BuildQueueEntry>,
    pub is_game_paused: bool,
    pub current_game_time: f32,
    pub fps: f32,
    pub frame_time_ms: f32,
    pub performance_score: f32,
    pub asset_memory_mb: f32,
    pub asset_cache_usage: f32,
    pub assets_loaded: u64,
    pub show_debug_overlay: bool,
    /// FPS counter residual.
    pub show_fps: bool,
    pub diagnostics: Option<DiagnosticsOverlayStats>,
    pub match_over: bool,
    pub player_outcome: Option<PlayerOutcome>,
    pub victory_summary: Option<VictorySummary>,
    pub minimap_unit_dots: Vec<MinimapDot>,
    pub minimap_beacons: Vec<MinimapDot>,
    pub minimap_viewport: UiRect,
    pub minimap_texture_id: Option<UiTextureId>,
    pub minimap_coordinates: Option<MinimapCoordinates>,
    pub radar_messages: Vec<String>,
    pub radar_events: Vec<RadarMessageEntry>,
    pub radar_pings: Vec<RadarPing>,
    pub last_radar_ping: Option<Vec3>,
    pub new_beacons: Vec<Vec3>,
    pub script_messages: Vec<String>,
    pub cinematic_letterbox: bool,
    pub cinematic_text: Option<String>,
    pub military_caption: Option<String>,
    /// Pending script movie residual from PresentationFrame.
    pub pending_movie: Option<String>,
    /// Pending radar movie residual from PresentationFrame.
    pub pending_radar_movie: Option<String>,
    /// Pending music-stop residual from PresentationFrame.
    pub pending_music_stop: bool,
    /// Pending popup message texts residual from PresentationFrame.
    pub pending_popup_messages: Vec<String>,
    /// Script time-freeze residual from PresentationFrame.
    pub script_time_frozen: bool,
    /// Script camera time-freeze residual from PresentationFrame.
    pub script_camera_time_frozen: bool,
    /// Combined sim freeze residual (script + camera).
    pub time_frozen_for_simulation: bool,
    /// Pending script FPS limit residual.
    pub script_fps_limit: Option<i32>,
    /// Pending view guardband residual (x,y bias).
    pub view_guardband: Option<(f32, f32)>,
    /// Pending camera focus residual.
    pub camera_focus: Option<[f32; 3]>,
    /// Pending BW mode residual (enabled, frames).
    pub camera_bw_mode: Option<(bool, i32)>,
    /// Pending camera shaker residual (amplitude, duration, radius).
    pub camera_shakers: Vec<(f32, f32, f32)>,
    /// Pending camera motion-blur request count residual.
    pub camera_motion_blur_count: usize,
    /// Pending camera zoom residual (zoom, duration).
    pub camera_zoom: Option<(f32, f32)>,
    pub camera_zoom_reset: bool,
    /// Pending camera pitch residual (pitch, duration).
    pub camera_pitch: Option<(f32, f32)>,
    /// Pending camera rotate residual (rotations, duration).
    pub camera_rotate: Option<(f32, f32)>,
    /// Pending look-toward residual.
    pub camera_look_toward: Option<[f32; 3]>,
    /// Pending slave-mode enable residual (template, bone).
    pub camera_slave_enable: Option<(String, String)>,
    pub camera_slave_disable: bool,
    /// Active script named timers residual (name, text, countdown).
    pub named_timers: Vec<(String, String, bool)>,
    /// Cameo flash residual (button, count).
    pub cameo_flash: Vec<(String, i32)>,
    /// Pending screen-shake intensities residual.
    pub screen_shakes: Vec<i32>,
    /// Script skybox enable residual.
    pub script_skybox_enabled: bool,
    /// Superweapon display enable residual.
    pub superweapon_display_enabled: bool,
    /// Named-timer display shown residual.
    pub named_timer_display_shown: bool,
    /// Hidden superweapon object ids residual.
    pub superweapon_hidden_objects: Vec<u32>,
    pub radar_enabled: bool,
    pub radar_forced: bool,
    pub objectives: Vec<ObjectiveDisplay>,
}

impl Default for GameUIState {
    fn default() -> Self {
        Self {
            credits: 10000,
            power_generated: 100,
            power_used: 60,
            max_power: 100,
            credits_per_second: 5.0,
            player_id: 0,
            player_name: localization::localize("hud.commander", "Commander"),
            rank_level: 1,
            skill_points: 0,
            science_purchase_points: 0,
            rank_progress_percent: 0,
            can_make_cameos: Vec::new(),
            can_make_producer_id: None,
            superweapon_timers: Vec::new(),
            selected_units: Vec::new(),
            selected_unit_infos: Vec::new(),
            selection_panel: ControlBarSelectionPanelState::default(),
            build_queue: Vec::new(),
            is_game_paused: false,
            current_game_time: 0.0,
            fps: 60.0,
            frame_time_ms: 16.6,
            performance_score: 1.0,
            asset_memory_mb: 0.0,
            asset_cache_usage: 0.0,
            assets_loaded: 0,
            show_debug_overlay: false,
            show_fps: false,
            diagnostics: None,
            match_over: false,
            player_outcome: None,
            victory_summary: None,
            minimap_unit_dots: Vec::new(),
            minimap_beacons: Vec::new(),
            minimap_viewport: default_minimap_viewport(),
            minimap_texture_id: None,
            minimap_coordinates: None,
            radar_messages: Vec::new(),
            radar_events: Vec::new(),
            radar_pings: Vec::new(),
            last_radar_ping: None,
            new_beacons: Vec::new(),
            script_messages: Vec::new(),
            cinematic_letterbox: false,
            cinematic_text: None,
            military_caption: None,
            pending_movie: None,
            pending_radar_movie: None,
            pending_music_stop: false,
            pending_popup_messages: Vec::new(),
            script_time_frozen: false,
            script_camera_time_frozen: false,
            time_frozen_for_simulation: false,
            script_fps_limit: None,
            view_guardband: None,
            camera_focus: None,
            camera_bw_mode: None,
            camera_shakers: Vec::new(),
            camera_motion_blur_count: 0,
            camera_zoom: None,
            camera_zoom_reset: false,
            camera_pitch: None,
            camera_rotate: None,
            camera_look_toward: None,
            camera_slave_enable: None,
            camera_slave_disable: false,
            named_timers: Vec::new(),
            cameo_flash: Vec::new(),
            screen_shakes: Vec::new(),
            script_skybox_enabled: false,
            superweapon_display_enabled: true,
            named_timer_display_shown: false,
            superweapon_hidden_objects: Vec::new(),
            radar_enabled: true,
            radar_forced: false,
            objectives: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RadarPing {
    pub position: Vec3,
    pub intensity: f32,
    pub age_seconds: f32,
    pub kind: RadarPingKind,
}

#[derive(Debug, Clone, Default)]
pub struct RadarMessageEntry {
    pub text: String,
    pub position: Option<Vec3>,
    pub kind: RadarPingKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RadarPingKind {
    #[default]
    Generic,
    Attack,
    Ally,
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticsOverlayStats {
    pub health_score: f32,
    pub engine: f32,
    pub graphics: f32,
    pub audio: f32,
    pub network: f32,
    pub logic: f32,
    pub warnings: u32,
    pub errors: u32,
    pub critical_errors: u32,
}

impl DiagnosticsOverlayStats {
    pub fn from_overall(health_percent: f32) -> Self {
        let clamped = health_percent.clamp(0.0, 150.0);
        Self {
            health_score: clamped,
            engine: clamped,
            graphics: clamped,
            audio: clamped,
            network: clamped,
            logic: clamped,
            warnings: 0,
            errors: 0,
            critical_errors: 0,
        }
    }

    #[cfg(feature = "integration-diagnostics")]
    pub fn from_system(diag: &SystemDiagnostics) -> Self {
        Self {
            health_score: diag.health_score as f32,
            engine: diag.subsystem_health.engine as f32,
            graphics: diag.subsystem_health.graphics as f32,
            audio: diag.subsystem_health.audio as f32,
            network: diag.subsystem_health.network as f32,
            logic: diag.subsystem_health.logic as f32,
            warnings: diag.error_counts.warnings,
            errors: diag.error_counts.errors,
            critical_errors: diag.error_counts.critical_errors,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnitDisplayInfo {
    pub object_id: ObjectId,
    pub name: String,
    pub health_current: f32,
    pub health_maximum: f32,
    pub unit_type: String,
    pub current_order: String,
    /// C++ SSChevron* residual from presentation veterancy.
    pub veterancy_overlay: Option<String>,
    /// First production queue item progress 0..1 (structures).
    pub production_progress: Option<f32>,
    /// First production queue template name (structures).
    pub production_template: Option<String>,
    /// Head queue entry is PRODUCTION_UPGRADE residual.
    pub production_is_upgrade: bool,
    /// Host/presentation command_set_override residual (empty = template default).
    pub command_set_override: String,
    /// Structure can enqueue production residual.
    pub can_produce: bool,
}

/// ControlBar / WND selection panel display (portrait + health strip).
///
/// Filled from `PresentationFrame` only — never re-reads live GameLogic.
/// Maps to the retail right-HUD selection region (`WinUnitSelected` / cameo + health).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ControlBarSelectionPanelState {
    pub visible: bool,
    /// Primary (first) selected unit template / display name.
    pub primary_name: String,
    pub health_current: f32,
    pub health_maximum: f32,
    pub selected_count: usize,
    pub primary_object_id: Option<ObjectId>,
    pub unit_infos: Vec<UnitDisplayInfo>,
    /// Primary selection C++ SSChevron* residual.
    pub veterancy_overlay: Option<String>,
    /// Primary production progress residual (first queue item).
    pub production_progress: Option<f32>,
    /// Primary production template residual (first queue item).
    pub production_template: Option<String>,
    /// Full production queue residual for selected structure (capped).
    pub production_queue: Vec<(String, f32, bool)>,
    /// Max garrison slots residual (0 = not a container).
    pub max_garrison: usize,
    /// Contained unit count residual.
    pub garrisoned_count: usize,
    /// Under-construction residual for CancelConstruction command.
    pub under_construction: bool,
    /// Construction progress 0..1 residual.
    pub construction_percent: f32,
    /// Applied upgrade tags residual from primary selection.
    pub applied_upgrades: Vec<String>,
    /// Structure rally point residual (presentation-only).
    pub rally_point: Option<[f32; 3]>,
    /// Special power ready residual on primary selection.
    pub special_power_ready: bool,
    /// Special power cooldown remaining residual (seconds).
    pub special_power_cooldown_remaining: f32,
    /// Head queue entry is PRODUCTION_UPGRADE residual.
    pub production_is_upgrade: bool,
}

impl ControlBarSelectionPanelState {
    /// Build selection panel state from presentation-owned unit infos.
    pub fn from_unit_infos(infos: &[UnitDisplayInfo]) -> Self {
        if infos.is_empty() {
            return Self::default();
        }
        let primary = &infos[0];
        Self {
            visible: true,
            primary_name: primary.name.clone(),
            health_current: primary.health_current,
            health_maximum: primary.health_maximum.max(1.0),
            selected_count: infos.len(),
            primary_object_id: Some(primary.object_id),
            unit_infos: infos.to_vec(),
            veterancy_overlay: primary.veterancy_overlay.clone(),
            production_progress: primary.production_progress,
            production_template: primary.production_template.clone(),
            production_queue: primary
                .production_template
                .as_ref()
                .zip(primary.production_progress)
                .map(|(t, p)| vec![(t.clone(), p, primary.production_is_upgrade)])
                .unwrap_or_default(),
            max_garrison: 0,
            garrisoned_count: 0,
            under_construction: false,
            construction_percent: 0.0,
            applied_upgrades: Vec::new(),
            rally_point: None,
            special_power_ready: false,
            special_power_cooldown_remaining: 0.0,
            production_is_upgrade: primary.production_is_upgrade,
        }
    }

    pub fn health_ratio(&self) -> f32 {
        if self.health_maximum <= 0.0 {
            0.0
        } else {
            (self.health_current / self.health_maximum).clamp(0.0, 1.0)
        }
    }

    /// Non-zero positive health suitable for "panel is showing real selection HP".
    pub fn has_positive_health(&self) -> bool {
        self.visible && self.health_current > 0.0 && self.health_maximum >= self.health_current
    }
}

#[derive(Debug, Clone)]
pub struct BuildQueueEntry {
    pub template_name: String,
    pub percent_complete: f32,
    pub time_remaining: f32,
}

#[derive(Debug, Clone)]
pub struct MinimapDot {
    pub position: UiPos2,
    pub color: UiColor,
    pub size: f32,
}

impl MinimapDot {
    pub fn normalized(normalized_x: f32, normalized_y: f32, color: UiColor, size: f32) -> Self {
        Self {
            position: UiPos2::new(normalized_x, normalized_y),
            color,
            size,
        }
    }
}

pub fn normalized_minimap_rect(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> UiRect {
    UiRect::from_min_max(
        UiPos2::new(min_x.clamp(0.0, 1.0), min_y.clamp(0.0, 1.0)),
        UiPos2::new(max_x.clamp(0.0, 1.0), max_y.clamp(0.0, 1.0)),
    )
}

pub fn default_minimap_viewport() -> UiRect {
    UiRect::from_min_size(UiPos2::ZERO, UiVec2::new(1.0, 1.0))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinimapActionKind {
    LeftClick,
    LeftDrag,
    RightClick,
}

#[derive(Debug, Clone, Copy)]
pub struct MinimapInteraction {
    pub screen_position: UiPos2,
    pub kind: MinimapActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VictoryOverlayAction {
    ExitToMenu,
}
