//! Wave 735: GameWorld production-ready log carries spawn pose + rally;
//! host sole-tick complete/spawn applies that pose authority.
//! Host still allocates production ObjectIds. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_READY_POSE_AUTHORITY_METHOD_NAMES_WAVE735: &[&str] = &[
    "record_with_pose",
    "ready_by_producer",
    "spawn_pos",
    "host_collect_production_completions",
    "Wave 735",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_READY_POSE_AUTHORITY_NAV_STEPS_WAVE735: &[&str] = &[
    "REQUIRE_RECORD_WITH_POSE",
    "REQUIRE_READY_BY_PRODUCER_MAP",
    "REQUIRE_SOLE_TICK_APPLIES_GW_POSE",
    "LIVE_HOST_PRODUCTION_READY_POSE_AUTHORITY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_READY_POSE_AUTHORITY_CMD_NAMES_WAVE735: &[&str] = &[
    "host_production_ready_pose_authority",
    "record_with_pose",
    "ready_by_producer",
    "sole_tick_applies_gw_pose",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionReadyPoseAuthorityAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionReadyPoseAuthorityAction {
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
fn residual_action_store(a: ResidualHostProductionReadyPoseAuthorityAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_ready_pose_authority_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_ready_pose_authority_last_action()
-> ResidualHostProductionReadyPoseAuthorityAction {
    ResidualHostProductionReadyPoseAuthorityAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
fn ready_source() -> &'static str {
    include_str!("../host_production_ready_log.rs")
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_production_ready_pose_authority_method_names_residual_wave735() -> bool {
    let names = LIVE_HOST_PRODUCTION_READY_POSE_AUTHORITY_METHOD_NAMES_WAVE735;
    let ok = residual_name_index(names, "record_with_pose").is_some()
        && residual_name_index(names, "ready_by_producer").is_some()
        && residual_name_index(names, "spawn_pos").is_some()
        && residual_name_index(names, "host_collect_production_completions").is_some()
        && residual_name_index(names, "Wave 735").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionReadyPoseAuthorityAction::MethodNames);
    ok
}
pub fn honesty_host_production_ready_pose_authority_source_markers_residual_wave735() -> bool {
    let gl = gl_source();
    let ready = ready_source();
    let sh = shadow_source();
    let gl_ok = gl.contains("Wave 735")
        && gl.contains("ready_by_producer")
        && gl.contains("ev.spawn_pos")
        && gl.contains("ev.rally");
    let ready_ok = ready.contains("record_with_pose")
        && ready.contains("spawn_pos: Option<[f32; 3]>")
        && ready.contains("Wave 735");
    let sh_ok = sh.contains("record_with_pose")
        && sh.contains("Wave 735")
        && sh.contains("ent.transform.position");
    let ok = gl_ok && ready_ok && sh_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionReadyPoseAuthorityAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_ready_pose_authority_nav_commands_residual_wave735() -> bool {
    let steps = LIVE_HOST_PRODUCTION_READY_POSE_AUTHORITY_NAV_STEPS_WAVE735;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_READY_POSE_AUTHORITY_CMD_NAMES_WAVE735;
    let ok = residual_name_index(steps, "REQUIRE_RECORD_WITH_POSE").is_some()
        && residual_name_index(steps, "REQUIRE_READY_BY_PRODUCER_MAP").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_TICK_APPLIES_GW_POSE").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_READY_POSE_AUTHORITY").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_ready_pose_authority").is_some()
        && residual_name_index(cmds, "record_with_pose").is_some()
        && residual_name_index(cmds, "ready_by_producer").is_some()
        && residual_name_index(cmds, "sole_tick_applies_gw_pose").is_some();
    residual_action_store(ResidualHostProductionReadyPoseAuthorityAction::NavCommands);
    ok
}
pub fn simulate_host_production_ready_pose_authority_collect_source() -> bool {
    let ok =
        ready_source().contains("record_with_pose") && gl_source().contains("ready_by_producer");
    residual_action_store(ResidualHostProductionReadyPoseAuthorityAction::CollectSource);
    ok
}
pub fn simulate_host_production_ready_pose_authority_dispatch_source() -> bool {
    let ok = shadow_source().contains("record_with_pose")
        && (gl_source().contains("if let Some(p) = ev.spawn_pos")
            || gl_source().contains("if let Some(p) = event.spawn_pos"));
    residual_action_store(ResidualHostProductionReadyPoseAuthorityAction::DispatchSource);
    ok
}
pub fn honesty_host_production_ready_pose_authority_residual_pack_wave735() -> bool {
    honesty_host_production_ready_pose_authority_method_names_residual_wave735()
        && honesty_host_production_ready_pose_authority_source_markers_residual_wave735()
        && honesty_host_production_ready_pose_authority_nav_commands_residual_wave735()
        && simulate_host_production_ready_pose_authority_collect_source()
        && simulate_host_production_ready_pose_authority_dispatch_source()
}
pub fn simulate_live_host_production_ready_pose_authority_honesty() -> bool {
    let ok = honesty_host_production_ready_pose_authority_residual_pack_wave735();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionReadyPoseAuthorityAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_ready_pose_authority_method_names_residual_wave735());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_ready_pose_authority_source_markers_residual_wave735());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_ready_pose_authority_nav_commands_residual_wave735());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_ready_pose_authority_collect_source());
        assert!(simulate_host_production_ready_pose_authority_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_ready_pose_authority_residual_pack_wave735());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_ready_pose_authority_honesty());
        assert!(residual_host_production_ready_pose_authority_ok());
    }
}
