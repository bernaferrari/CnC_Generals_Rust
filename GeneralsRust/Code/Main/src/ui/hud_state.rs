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
    pub selected_units: Vec<ObjectId>,
    pub selected_unit_infos: Vec<UnitDisplayInfo>,
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
            selected_units: Vec::new(),
            selected_unit_infos: Vec::new(),
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

#[derive(Debug, Clone)]
pub struct UnitDisplayInfo {
    pub object_id: ObjectId,
    pub name: String,
    pub health_current: f32,
    pub health_maximum: f32,
    pub unit_type: String,
    pub current_order: String,
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
