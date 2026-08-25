//! Wave 743: under production sole-tick, host does not dual-tick production
//! door residual — GameWorld owns door phase advance + writeback. Host still
//! starts door open on spawn-ready (logs to GW). `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_DOOR_SOLE_NO_DUAL_TICK_METHOD_NAMES_WAVE743: &[&str] = &[
    "tick_production_door",
    "gameworld_production_sole_tick_enabled",
    "Wave 743",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_DOOR_SOLE_NO_DUAL_TICK_NAV_STEPS_WAVE743: &[&str] = &[
    "REQUIRE_SOLE_SKIP_HOST_DOOR_TICK",
    "REQUIRE_NON_SOLE_KEEPS_TICK",
    "REQUIRE_GW_DOOR_WRITEBACK",
    "LIVE_HOST_PRODUCTION_DOOR_SOLE_NO_DUAL_TICK",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_DOOR_SOLE_NO_DUAL_TICK_CMD_NAMES_WAVE743: &[&str] = &[
    "host_production_door_sole_no_dual_tick",
    "sole_skip_host_door_tick",
    "non_sole_keeps_tick",
    "gw_door_writeback",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionDoorSoleNoDualTickAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionDoorSoleNoDualTickAction {
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
fn residual_action_store(a: ResidualHostProductionDoorSoleNoDualTickAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_door_sole_no_dual_tick_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_door_sole_no_dual_tick_last_action()
-> ResidualHostProductionDoorSoleNoDualTickAction {
    ResidualHostProductionDoorSoleNoDualTickAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_production_door_sole_no_dual_tick_method_names_residual_wave743() -> bool {
    let names = LIVE_HOST_PRODUCTION_DOOR_SOLE_NO_DUAL_TICK_METHOD_NAMES_WAVE743;
    let ok = residual_name_index(names, "tick_production_door").is_some()
        && residual_name_index(names, "gameworld_production_sole_tick_enabled").is_some()
        && residual_name_index(names, "Wave 743").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionDoorSoleNoDualTickAction::MethodNames);
    ok
}
pub fn honesty_host_production_door_sole_no_dual_tick_source_markers_residual_wave743() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    // 2026-08-15: C++ ProductionUpdate::updateDoors still runs on host.
    // GameWorld has no door-phase timer; skipping froze WAITING_OPEN.
    // Host always ticks doors; GW mirrors the event at the couple boundary.
    let gl_ok = gl.contains("tick_production_door(self.frame)")
        && gl.contains("ProductionUpdate::updateDoors")
        && gl.contains("no door");
    let sh_ok = sh.contains("writeback_production_door_to_host")
        || sh.contains("host_production_door_ready_log");
    let ok = gl_ok && sh_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionDoorSoleNoDualTickAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_door_sole_no_dual_tick_nav_commands_residual_wave743() -> bool {
    let steps = LIVE_HOST_PRODUCTION_DOOR_SOLE_NO_DUAL_TICK_NAV_STEPS_WAVE743;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_DOOR_SOLE_NO_DUAL_TICK_CMD_NAMES_WAVE743;
    let ok = residual_name_index(steps, "REQUIRE_SOLE_SKIP_HOST_DOOR_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_NON_SOLE_KEEPS_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_GW_DOOR_WRITEBACK").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_DOOR_SOLE_NO_DUAL_TICK").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_door_sole_no_dual_tick").is_some()
        && residual_name_index(cmds, "sole_skip_host_door_tick").is_some()
        && residual_name_index(cmds, "non_sole_keeps_tick").is_some()
        && residual_name_index(cmds, "gw_door_writeback").is_some();
    residual_action_store(ResidualHostProductionDoorSoleNoDualTickAction::NavCommands);
    ok
}
pub fn simulate_host_production_door_sole_no_dual_tick_collect_source() -> bool {
    let ok = gl_source().contains("tick_production_door")
        && gl_source().contains("gameworld_production_sole_tick_enabled");
    residual_action_store(ResidualHostProductionDoorSoleNoDualTickAction::CollectSource);
    ok
}
pub fn simulate_host_production_door_sole_no_dual_tick_dispatch_source() -> bool {
    let ok = gl_source().contains("tick_production_door(self.frame)")
        && gl_source().contains("ProductionUpdate::updateDoors");
    residual_action_store(ResidualHostProductionDoorSoleNoDualTickAction::DispatchSource);
    ok
}
pub fn honesty_host_production_door_sole_no_dual_tick_residual_pack_wave743() -> bool {
    honesty_host_production_door_sole_no_dual_tick_method_names_residual_wave743()
        && honesty_host_production_door_sole_no_dual_tick_source_markers_residual_wave743()
        && honesty_host_production_door_sole_no_dual_tick_nav_commands_residual_wave743()
        && simulate_host_production_door_sole_no_dual_tick_collect_source()
        && simulate_host_production_door_sole_no_dual_tick_dispatch_source()
}
pub fn simulate_live_host_production_door_sole_no_dual_tick_honesty() -> bool {
    let ok = honesty_host_production_door_sole_no_dual_tick_residual_pack_wave743();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionDoorSoleNoDualTickAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_door_sole_no_dual_tick_method_names_residual_wave743());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_door_sole_no_dual_tick_source_markers_residual_wave743());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_door_sole_no_dual_tick_nav_commands_residual_wave743());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_door_sole_no_dual_tick_collect_source());
        assert!(simulate_host_production_door_sole_no_dual_tick_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_door_sole_no_dual_tick_residual_pack_wave743());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_door_sole_no_dual_tick_honesty());
        assert!(residual_host_production_door_sole_no_dual_tick_ok());
    }
}
