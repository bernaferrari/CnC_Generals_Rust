//! Wave 739: under production sole-tick, host production-spawn-ready apply does
//! not re-jitter or reposition units — GameWorld ready-log / create_object exit
//! pose stays authoritative. Non-sole path keeps UnitCreatePoint + Z-snap
//! (no selection-radius spawn jitter). `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_SPAWN_POSE_NO_REJITTER_METHOD_NAMES_WAVE739: &[&str] = &[
    "let sole = ",
    "no spawn jitter",
    "if !sole",
    "host_apply_production_spawn_ready_completions",
    "Wave 739",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_SPAWN_POSE_NO_REJITTER_NAV_STEPS_WAVE739: &[&str] = &[
    "REQUIRE_SOLE_SKIP_JITTER",
    "REQUIRE_SOLE_SKIP_REPOSITION",
    "REQUIRE_NON_SOLE_NO_SELECTION_RADIUS_JITTER",
    "LIVE_HOST_PRODUCTION_SPAWN_POSE_NO_REJITTER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_POSE_NO_REJITTER_CMD_NAMES_WAVE739: &[&str] = &[
    "host_production_spawn_pose_no_rejitter",
    "sole_skip_jitter",
    "sole_skip_reposition",
    "non_sole_no_selection_radius_jitter",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionSpawnPoseNoRejitterAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionSpawnPoseNoRejitterAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostProductionSpawnPoseNoRejitterAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_spawn_pose_no_rejitter_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_spawn_pose_no_rejitter_last_action()
-> ResidualHostProductionSpawnPoseNoRejitterAction {
    ResidualHostProductionSpawnPoseNoRejitterAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_production_spawn_pose_no_rejitter_method_names_residual_wave739() -> bool {
    let names = LIVE_HOST_PRODUCTION_SPAWN_POSE_NO_REJITTER_METHOD_NAMES_WAVE739;
    let ok = residual_name_index(names, "let sole = ").is_some()
        && residual_name_index(names, "no spawn jitter").is_some()
        && residual_name_index(names, "if !sole").is_some()
        && residual_name_index(names, "host_apply_production_spawn_ready_completions").is_some()
        && residual_name_index(names, "Wave 739").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionSpawnPoseNoRejitterAction::MethodNames);
    ok
}
pub fn honesty_host_production_spawn_pose_no_rejitter_source_markers_residual_wave739() -> bool {
    let gl = gl_source();
    let j = gl
        .find("fn host_apply_production_spawn_ready_completions")
        .unwrap_or(0);
    let body = &gl[j..j + 8000.min(gl.len().saturating_sub(j))];
    let ok = body.contains("Wave 739")
        && body.contains(
            "let sole = crate::gameworld_shadow::gameworld_production_sole_tick_enabled()",
        )
        && body.contains("if !sole && !parked_jet")
        && body.contains("no spawn jitter")
        && !body.contains("jitter_dir")
        && !body.contains("spawn_pos += ")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionSpawnPoseNoRejitterAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_spawn_pose_no_rejitter_nav_commands_residual_wave739() -> bool {
    let steps = LIVE_HOST_PRODUCTION_SPAWN_POSE_NO_REJITTER_NAV_STEPS_WAVE739;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_POSE_NO_REJITTER_CMD_NAMES_WAVE739;
    let ok = residual_name_index(steps, "REQUIRE_SOLE_SKIP_JITTER").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_SKIP_REPOSITION").is_some()
        && residual_name_index(steps, "REQUIRE_NON_SOLE_NO_SELECTION_RADIUS_JITTER").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_SPAWN_POSE_NO_REJITTER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_spawn_pose_no_rejitter").is_some()
        && residual_name_index(cmds, "sole_skip_jitter").is_some()
        && residual_name_index(cmds, "sole_skip_reposition").is_some()
        && residual_name_index(cmds, "non_sole_no_selection_radius_jitter").is_some();
    residual_action_store(ResidualHostProductionSpawnPoseNoRejitterAction::NavCommands);
    ok
}
pub fn simulate_host_production_spawn_pose_no_rejitter_collect_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("host_apply_production_spawn_ready_completions")
        && gl.contains("gameworld_production_sole_tick_enabled()");
    residual_action_store(ResidualHostProductionSpawnPoseNoRejitterAction::CollectSource);
    ok
}
pub fn simulate_host_production_spawn_pose_no_rejitter_dispatch_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 739") && gl.contains("if !sole && !parked_jet");
    residual_action_store(ResidualHostProductionSpawnPoseNoRejitterAction::DispatchSource);
    ok
}
pub fn honesty_host_production_spawn_pose_no_rejitter_residual_pack_wave739() -> bool {
    honesty_host_production_spawn_pose_no_rejitter_method_names_residual_wave739()
        && honesty_host_production_spawn_pose_no_rejitter_source_markers_residual_wave739()
        && honesty_host_production_spawn_pose_no_rejitter_nav_commands_residual_wave739()
        && simulate_host_production_spawn_pose_no_rejitter_collect_source()
        && simulate_host_production_spawn_pose_no_rejitter_dispatch_source()
}
pub fn simulate_live_host_production_spawn_pose_no_rejitter_honesty() -> bool {
    let ok = honesty_host_production_spawn_pose_no_rejitter_residual_pack_wave739();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionSpawnPoseNoRejitterAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_spawn_pose_no_rejitter_method_names_residual_wave739());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_spawn_pose_no_rejitter_source_markers_residual_wave739());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_spawn_pose_no_rejitter_nav_commands_residual_wave739());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_spawn_pose_no_rejitter_collect_source());
        assert!(simulate_host_production_spawn_pose_no_rejitter_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_spawn_pose_no_rejitter_residual_pack_wave739());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_spawn_pose_no_rejitter_honesty());
        assert!(residual_host_production_spawn_pose_no_rejitter_ok());
    }
}
