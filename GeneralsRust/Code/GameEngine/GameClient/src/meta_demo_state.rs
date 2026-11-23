//! Per-client meta-event demo/cheat toggles.
//!
//! C++ kept these as file-statics on MetaEvent.cpp. Rust owns them on
//! [`crate::core::subsystems::InGameUISubsystem`] so multiple GameClients never
//! share process TLS. Plain fields (not Cell/RefCell) keep InGameUI Sync and
//! match `&mut ui.state.meta_demo` dispatch threading.

use game_engine::common::ini::DynamicGameLODLevel;

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
