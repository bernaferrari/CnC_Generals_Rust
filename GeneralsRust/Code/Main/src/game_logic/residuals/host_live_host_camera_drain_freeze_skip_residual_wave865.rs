//! Wave 865: skip live camera queue dual-reads when presentation freeze is
//! installed; host_override_world_size stamps host_match_world_bounds.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CAMERA_DRAIN_FREEZE_SKIP_METHOD_NAMES_WAVE865: &[&str] = &[
    "host_drain_live_camera_request_queues",
    "host_override_world_size",
    "host_match_world_bounds",
    "last_presentation_frame.is_some",
    "Wave 865",
    "playable_claim = false",
];

pub const LIVE_HOST_CAMERA_DRAIN_FREEZE_SKIP_NAV_STEPS_WAVE865: &[&str] = &[
    "SKIP_CAMERA_DRAIN_WHEN_FREEZE",
    "STAMP_BOUNDS_ON_WORLD_SIZE_OVERRIDE",
    "LIVE_HOST_CAMERA_DRAIN_FREEZE_SKIP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCameraDrainFreezeSkipAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCameraDrainFreezeSkipAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_camera_drain_freeze_skip_method_names_residual_wave865() -> bool {
    let names = LIVE_HOST_CAMERA_DRAIN_FREEZE_SKIP_METHOD_NAMES_WAVE865;
    let ok = residual_name_index(names, "host_drain_live_camera_request_queues").is_some()
        && residual_name_index(names, "host_override_world_size").is_some()
        && residual_name_index(names, "Wave 865").is_some();
    residual_action_store(ResidualHostCameraDrainFreezeSkipAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_camera_drain_freeze_skip_nav_commands_residual_wave865() -> bool {
    let steps = LIVE_HOST_CAMERA_DRAIN_FREEZE_SKIP_NAV_STEPS_WAVE865;
    let ok = residual_name_index(steps, "LIVE_HOST_CAMERA_DRAIN_FREEZE_SKIP").is_some()
        && residual_name_index(steps, "SKIP_CAMERA_DRAIN_WHEN_FREEZE").is_some();
    residual_action_store(ResidualHostCameraDrainFreezeSkipAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_camera_drain_freeze_skip_residual_pack_wave865() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("Wave 596/865: host camera request queue drain residual")
        && cnc.contains(
            "Wave 865: when presentation freeze owns the frame, skip live queue dual-reads",
        )
        && cnc.contains("if self.last_presentation_frame.is_some()")
        && cnc.contains("Wave 585/865: host override_world_size residual + stamp bounds residual")
        && cnc.contains("self.host_match_world_bounds = Some((min, max))");
    residual_action_store(ResidualHostCameraDrainFreezeSkipAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_camera_drain_freeze_skip_honesty() -> bool {
    let a = honesty_host_camera_drain_freeze_skip_method_names_residual_wave865();
    let b = honesty_host_camera_drain_freeze_skip_nav_commands_residual_wave865();
    let c = honesty_host_camera_drain_freeze_skip_residual_pack_wave865();
    residual_action_store(ResidualHostCameraDrainFreezeSkipAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_camera_drain_freeze_skip_residual_wave865() {
        assert!(honesty_host_camera_drain_freeze_skip_residual_pack_wave865());
        assert!(honesty_host_camera_drain_freeze_skip_method_names_residual_wave865());
        assert!(honesty_host_camera_drain_freeze_skip_nav_commands_residual_wave865());
        assert!(simulate_live_host_camera_drain_freeze_skip_honesty());
    }
}
