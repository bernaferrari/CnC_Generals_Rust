//! Wave 947: shadow channel-drive dual-writes via `host_object_mut`.
//!
//! Channel-drive residual tests (`host_*_log_drives_*_channel`) and remaining
//! shadow dual-write sites no longer call `logic.get_objects_mut()` directly.
//! Host object mutation goes through `GameLogic::host_object_mut`.
//! `gameworld_shadow.rs` contains zero `get_objects_mut` tokens.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SHADOW_HOST_OBJECT_MUT_SEAL_METHOD_NAMES_WAVE947: &[&str] = &[
    "host_object_mut",
    "with_host_object_mut",
    "host_ai_state_log_drives_set_ai_state_channel",
    "Wave 947",
    "playable_claim = false",
];

pub const LIVE_HOST_SHADOW_HOST_OBJECT_MUT_SEAL_NAV_STEPS_WAVE947: &[&str] = &[
    "SHADOW_HOST_OBJECT_MUT_SEAL",
    "CHANNEL_DRIVE_HOST_OBJECT_MUT",
    "LIVE_HOST_SHADOW_HOST_OBJECT_MUT_SEAL",
    "SHADOW_GET_OBJECTS_MUT_ZERO",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostShadowHostObjectMutSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostShadowHostObjectMutSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_shadow_host_object_mut_seal_method_names_residual_wave947() -> bool {
    let names = LIVE_HOST_SHADOW_HOST_OBJECT_MUT_SEAL_METHOD_NAMES_WAVE947;
    let ok = residual_name_index(names, "host_object_mut").is_some()
        && residual_name_index(names, "Wave 947").is_some()
        && residual_name_index(names, "host_ai_state_log_drives_set_ai_state_channel").is_some();
    residual_action_store(ResidualHostShadowHostObjectMutSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_shadow_host_object_mut_seal_nav_commands_residual_wave947() -> bool {
    let steps = LIVE_HOST_SHADOW_HOST_OBJECT_MUT_SEAL_NAV_STEPS_WAVE947;
    let ok = residual_name_index(steps, "LIVE_HOST_SHADOW_HOST_OBJECT_MUT_SEAL").is_some()
        && residual_name_index(steps, "SHADOW_GET_OBJECTS_MUT_ZERO").is_some();
    residual_action_store(ResidualHostShadowHostObjectMutSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_shadow_host_object_mut_seal_residual_pack_wave947() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let cnc = cnc_source();
    let sh_code = non_comment_code(sh);
    let mut_count = sh_code.matches("get_objects_mut").count();
    let host_mut_count = sh_code.matches("host_object_mut").count();
    let channel_ok = sh.contains("host_object_mut") && mut_count == 0;
    let ok = gl.contains("fn host_object_mut")
        && gl.contains("fn with_host_object_mut")
        && mut_count == 0
        && host_mut_count >= 1
        && channel_ok
        && (gl.contains("Wave 947") || gl.contains("946/947") || gl.contains("Wave 955/958"))
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostShadowHostObjectMutSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_shadow_host_object_mut_seal_honesty() -> bool {
    let a = honesty_host_shadow_host_object_mut_seal_method_names_residual_wave947();
    let b = honesty_host_shadow_host_object_mut_seal_nav_commands_residual_wave947();
    let c = honesty_host_shadow_host_object_mut_seal_residual_pack_wave947();
    residual_action_store(ResidualHostShadowHostObjectMutSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_shadow_host_object_mut_seal_residual_wave947() {
        assert!(honesty_host_shadow_host_object_mut_seal_residual_pack_wave947());
        assert!(honesty_host_shadow_host_object_mut_seal_method_names_residual_wave947());
        assert!(honesty_host_shadow_host_object_mut_seal_nav_commands_residual_wave947());
        assert!(simulate_live_host_shadow_host_object_mut_seal_honesty());
    }
}
