//! Per-client meta-event demo/cheat toggles.
//!
//! C++ kept these as file-statics on MetaEvent.cpp. Rust owns them on
//! [`crate::core::subsystems::InGameUISubsystem`] so multiple GameClients never
//! share process TLS. Plain fields (not Cell/RefCell) keep InGameUI Sync and
//! match `&mut ui.state.meta_demo` dispatch threading.

use game_engine::common::ini::DynamicGameLODLevel;

const DROPPED_MAX_PARTICLE_COUNT: i32 = 1000;

/// Screen-space point for demo camera adjust (avoids message_stream import cycle).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DemoCoord2D {
    pub x: i32,
    pub y: i32,
}

impl DemoCoord2D {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone)]
pub struct LowerDetailToggleState {
    pub is_low_details: bool,
    pub old_use_shadow_volumes: bool,
    pub old_use_light_map: bool,
    pub old_use_cloud_map: bool,
    pub old_show_behind_building_markers: bool,
    pub old_max_particle_count: i32,
}

impl Default for LowerDetailToggleState {
    fn default() -> Self {
        Self {
            is_low_details: false,
            old_use_shadow_volumes: false,
            old_use_light_map: false,
            old_use_cloud_map: false,
            old_show_behind_building_markers: false,
            old_max_particle_count: DROPPED_MAX_PARTICLE_COUNT,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DemoCameraAdjustState {
    pub is_pitching: bool,
    pub is_changing_fov: bool,
    pub anchor: DemoCoord2D,
    pub current_pos: DemoCoord2D,
}

/// Per-client meta-event demo/cheat latch (was meta_event_impl/state TLS).
#[derive(Debug, Default)]
pub struct MetaDemoState {
    pub lower_detail_toggle: LowerDetailToggleState,
    pub motion_blur_zoom_saturate: bool,
    pub demo_camera_adjust: DemoCameraAdjustState,
    pub hand_of_god_mode: bool,
    pub hurt_me_mode: bool,
    pub debug_selection_mode: bool,
    pub bw_view_mode: u8,
    pub cycle_lod_level: DynamicGameLODLevel,
    pub last_plane_lock_object_id: Option<u32>,
    pub vtune_enabled: bool,
    pub skate_distance_override: f32,
}

impl MetaDemoState {
    pub fn toggle_bool(flag: &mut bool) -> bool {
        *flag = !*flag;
        *flag
    }
}
