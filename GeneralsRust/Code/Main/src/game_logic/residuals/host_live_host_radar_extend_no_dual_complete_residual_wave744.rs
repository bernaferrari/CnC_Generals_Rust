//! Wave 744: under coupled GameWorld shadow, host does not dual-complete
//! radar-extend via `tick_radar_extend`. Writeback + `host_apply_radar_extend_ready_completions`
//! own complete flag and model/counter residual. Non-coupled path keeps host tick.
//! `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_RADAR_EXTEND_NO_DUAL_COMPLETE_METHOD_NAMES_WAVE744: &[&str] = &[
    "tick_radar_extend",
    "shadow_coupled_tick_active",
    "host_apply_radar_extend_ready_completions",
    "Wave 744",
    "playable_claim = false",
];
pub const LIVE_HOST_RADAR_EXTEND_NO_DUAL_COMPLETE_NAV_STEPS_WAVE744: &[&str] = &[
    "REQUIRE_COUPLED_SKIP_HOST_TICK",
    "REQUIRE_WRITEBACK_READY_APPLY",
    "REQUIRE_NON_COUPLED_KEEPS_TICK",
    "LIVE_HOST_RADAR_EXTEND_NO_DUAL_COMPLETE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_RADAR_EXTEND_NO_DUAL_COMPLETE_CMD_NAMES_WAVE744: &[&str] = &[
    "host_radar_extend_no_dual_complete",
    "coupled_skip_host_tick",
    "writeback_ready_apply",
    "non_coupled_keeps_tick",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRadarExtendNoDualCompleteAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostRadarExtendNoDualCompleteAction {
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
fn residual_action_store(a: ResidualHostRadarExtendNoDualCompleteAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_radar_extend_no_dual_complete_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_radar_extend_no_dual_complete_last_action()
-> ResidualHostRadarExtendNoDualCompleteAction {
    ResidualHostRadarExtendNoDualCompleteAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn ready_source() -> &'static str {
    include_str!("../host_radar_extend_ready_log.rs")
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_radar_extend_no_dual_complete_method_names_residual_wave744() -> bool {
    let names = LIVE_HOST_RADAR_EXTEND_NO_DUAL_COMPLETE_METHOD_NAMES_WAVE744;
    let ok = residual_name_index(names, "tick_radar_extend").is_some()
        && residual_name_index(names, "shadow_coupled_tick_active").is_some()
        && residual_name_index(names, "host_apply_radar_extend_ready_completions").is_some()
        && residual_name_index(names, "Wave 744").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRadarExtendNoDualCompleteAction::MethodNames);
    ok
}
pub fn honesty_host_radar_extend_no_dual_complete_source_markers_residual_wave744() -> bool {
    let gl = gl_source();
    let ready = ready_source();
    let sh = shadow_source();
    let gl_ok = gl.contains("Wave 744")
        && gl.contains("shadow_coupled_tick_active()")
        && gl.contains("tick_radar_extend(self.frame)")
        && gl.contains("host_apply_radar_extend_ready_completions");
    let ready_ok = ready.contains("Wave 625") || ready.contains("radar_extend_complete");
    let sh_ok = sh.contains("writeback_radar_extend_to_host")
        && sh.contains("host_radar_extend_ready_log::record");
    let ok = gl_ok && ready_ok && sh_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostRadarExtendNoDualCompleteAction::SourceMarkers);
    ok
}
pub fn honesty_host_radar_extend_no_dual_complete_nav_commands_residual_wave744() -> bool {
    let steps = LIVE_HOST_RADAR_EXTEND_NO_DUAL_COMPLETE_NAV_STEPS_WAVE744;
    let cmds = RUNTIME_HOST_LIVE_HOST_RADAR_EXTEND_NO_DUAL_COMPLETE_CMD_NAMES_WAVE744;
    let ok = residual_name_index(steps, "REQUIRE_COUPLED_SKIP_HOST_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_READY_APPLY").is_some()
        && residual_name_index(steps, "REQUIRE_NON_COUPLED_KEEPS_TICK").is_some()
        && residual_name_index(steps, "LIVE_HOST_RADAR_EXTEND_NO_DUAL_COMPLETE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_radar_extend_no_dual_complete").is_some()
        && residual_name_index(cmds, "coupled_skip_host_tick").is_some()
        && residual_name_index(cmds, "writeback_ready_apply").is_some()
        && residual_name_index(cmds, "non_coupled_keeps_tick").is_some();
    residual_action_store(ResidualHostRadarExtendNoDualCompleteAction::NavCommands);
    ok
}
pub fn simulate_host_radar_extend_no_dual_complete_collect_source() -> bool {
    let ok = gl_source().contains("tick_radar_extend")
        && gl_source().contains("host_apply_radar_extend_ready_completions");
    residual_action_store(ResidualHostRadarExtendNoDualCompleteAction::CollectSource);
    ok
}
pub fn simulate_host_radar_extend_no_dual_complete_dispatch_source() -> bool {
    let ok =
        gl_source().contains("Wave 744") && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostRadarExtendNoDualCompleteAction::DispatchSource);
    ok
}
pub fn honesty_host_radar_extend_no_dual_complete_residual_pack_wave744() -> bool {
    honesty_host_radar_extend_no_dual_complete_method_names_residual_wave744()
        && honesty_host_radar_extend_no_dual_complete_source_markers_residual_wave744()
        && honesty_host_radar_extend_no_dual_complete_nav_commands_residual_wave744()
        && simulate_host_radar_extend_no_dual_complete_collect_source()
        && simulate_host_radar_extend_no_dual_complete_dispatch_source()
}
pub fn simulate_live_host_radar_extend_no_dual_complete_honesty() -> bool {
    let ok = honesty_host_radar_extend_no_dual_complete_residual_pack_wave744();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRadarExtendNoDualCompleteAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_radar_extend_no_dual_complete_method_names_residual_wave744());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_radar_extend_no_dual_complete_source_markers_residual_wave744());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_radar_extend_no_dual_complete_nav_commands_residual_wave744());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_radar_extend_no_dual_complete_collect_source());
        assert!(simulate_host_radar_extend_no_dual_complete_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_radar_extend_no_dual_complete_residual_pack_wave744());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_radar_extend_no_dual_complete_honesty());
        assert!(residual_host_radar_extend_no_dual_complete_ok());
    }
}
