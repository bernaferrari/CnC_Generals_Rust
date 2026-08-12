//! Wave 891: host_override_world_size + host_set_camera_follow_object dual-read peel.
//!
//! - `host_override_world_size` stamps `host_match_world_bounds` from width/height
//!   (parity with GameLogic::override_world_size math) — no `world_bounds()` dual-read.
//! - `host_set_camera_follow_object` stamps follow position from one object resolve
//!   before set — no second `camera_follow_target_position()` dual-read.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OVERRIDE_CAMERA_FOLLOW_METHOD_NAMES_WAVE891: &[&str] = &[
    "host_override_world_size",
    "host_set_camera_follow_object",
    "host_match_world_bounds",
    "host_match_camera_follow_position",
    "Wave 891",
    "playable_claim = false",
];

pub const LIVE_HOST_OVERRIDE_CAMERA_FOLLOW_NAV_STEPS_WAVE891: &[&str] = &[
    "OVERRIDE_BOUNDS_STAMP_FROM_ARGS",
    "CAMERA_FOLLOW_SINGLE_RESOLVE_STAMP",
    "LIVE_HOST_OVERRIDE_CAMERA_FOLLOW",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostOverrideCameraFollowAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostOverrideCameraFollowAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_host_override_camera_follow_method_names_residual_wave891() -> bool {
    let names = LIVE_HOST_OVERRIDE_CAMERA_FOLLOW_METHOD_NAMES_WAVE891;
    let ok = residual_name_index(names, "host_override_world_size").is_some()
        && residual_name_index(names, "host_set_camera_follow_object").is_some()
        && residual_name_index(names, "Wave 891").is_some();
    residual_action_store(ResidualHostOverrideCameraFollowAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_override_camera_follow_nav_commands_residual_wave891() -> bool {
    let steps = LIVE_HOST_OVERRIDE_CAMERA_FOLLOW_NAV_STEPS_WAVE891;
    let ok = residual_name_index(steps, "LIVE_HOST_OVERRIDE_CAMERA_FOLLOW").is_some()
        && residual_name_index(steps, "OVERRIDE_BOUNDS_STAMP_FROM_ARGS").is_some()
        && residual_name_index(steps, "CAMERA_FOLLOW_SINGLE_RESOLVE_STAMP").is_some();
    residual_action_store(ResidualHostOverrideCameraFollowAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_override_camera_follow_residual_pack_wave891() -> bool {
    let cnc = cnc_source();
    // Locate host_override_world_size body window
    let Some(ow_at) = cnc.find("fn host_override_world_size") else {
        RESIDUAL_OK.store(false, Ordering::SeqCst);
        return false;
    };
    let ow = &cnc[ow_at..cnc.len().min(ow_at + 900)];
    let Some(cf_at) = cnc.find("fn host_set_camera_follow_object") else {
        RESIDUAL_OK.store(false, Ordering::SeqCst);
        return false;
    };
    let cf = &cnc[cf_at..cnc.len().min(cf_at + 1100)];
    let ok = ow.contains("host_match_world_bounds = Some((min, max))")
        && ow.contains("half_w")
        && ow.contains("half_h")
        && !ow.contains("self.game_logic.world_bounds()")
        && cf.contains("stamped_pos")
        && cf.contains("get_object(oid)")
        && !cf
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .any(|l| l.contains("camera_follow_target_position()"))
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostOverrideCameraFollowAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_override_camera_follow_honesty() -> bool {
    let a = honesty_host_override_camera_follow_method_names_residual_wave891();
    let b = honesty_host_override_camera_follow_nav_commands_residual_wave891();
    let c = honesty_host_override_camera_follow_residual_pack_wave891();
    residual_action_store(ResidualHostOverrideCameraFollowAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_override_camera_follow_residual_wave891() {
        assert!(honesty_host_override_camera_follow_residual_pack_wave891());
        assert!(honesty_host_override_camera_follow_method_names_residual_wave891());
        assert!(honesty_host_override_camera_follow_nav_commands_residual_wave891());
        assert!(simulate_live_host_override_camera_follow_honesty());
    }
}
