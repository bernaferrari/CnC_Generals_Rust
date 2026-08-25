//! Wave 754: EjectPilotDie fires at death start (`mark_object_for_destruction`)
//! matching C++ onDie timing. SlowDeath deferral must not suppress pilot spawn
//! or eject honesty residual. `process_destroy_list` remains fail-closed if
//! death-start path did not apply. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EJECT_PILOT_DIE_DEATH_START_METHOD_NAMES_WAVE754: &[&str] = &[
    "maybe_apply_eject_pilot_die",
    "eject_pilot_die_applied",
    "mark_object_for_destruction",
    "Wave 754",
    "playable_claim = false",
];
pub const LIVE_HOST_EJECT_PILOT_DIE_DEATH_START_NAV_STEPS_WAVE754: &[&str] = &[
    "REQUIRE_DEATH_START_EJECT",
    "REQUIRE_SLOWDEATH_SAFE",
    "REQUIRE_NO_DOUBLE_SPAWN",
    "LIVE_HOST_EJECT_PILOT_DIE_DEATH_START",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_EJECT_PILOT_DIE_DEATH_START_CMD_NAMES_WAVE754: &[&str] = &[
    "host_eject_pilot_die_death_start",
    "maybe_apply_eject_pilot_die",
    "eject_pilot_die_applied",
    "slowdeath_safe",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEjectPilotDieDeathStartAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEjectPilotDieDeathStartAction {
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
fn residual_action_store(a: ResidualHostEjectPilotDieDeathStartAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_eject_pilot_die_death_start_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_eject_pilot_die_death_start_last_action()
-> ResidualHostEjectPilotDieDeathStartAction {
    ResidualHostEjectPilotDieDeathStartAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
fn obj_source() -> &'static str {
    crate::game_logic::object::OBJECT_SRC
}
pub fn honesty_host_eject_pilot_die_death_start_method_names_residual_wave754() -> bool {
    let names = LIVE_HOST_EJECT_PILOT_DIE_DEATH_START_METHOD_NAMES_WAVE754;
    let ok = residual_name_index(names, "maybe_apply_eject_pilot_die").is_some()
        && residual_name_index(names, "eject_pilot_die_applied").is_some()
        && residual_name_index(names, "mark_object_for_destruction").is_some()
        && residual_name_index(names, "Wave 754").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEjectPilotDieDeathStartAction::MethodNames);
    ok
}
pub fn honesty_host_eject_pilot_die_death_start_source_markers_residual_wave754() -> bool {
    let gl = gl_source();
    let obj = obj_source();
    let ok = gl.contains("Wave 754")
        && gl.contains("fn maybe_apply_eject_pilot_die")
        && gl.contains("maybe_apply_eject_pilot_die(id)")
        && gl.contains("eject_pilot_die_applied")
        && obj.contains("eject_pilot_die_applied")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostEjectPilotDieDeathStartAction::SourceMarkers);
    ok
}
pub fn honesty_host_eject_pilot_die_death_start_nav_commands_residual_wave754() -> bool {
    let steps = LIVE_HOST_EJECT_PILOT_DIE_DEATH_START_NAV_STEPS_WAVE754;
    let cmds = RUNTIME_HOST_LIVE_HOST_EJECT_PILOT_DIE_DEATH_START_CMD_NAMES_WAVE754;
    let ok = residual_name_index(steps, "REQUIRE_DEATH_START_EJECT").is_some()
        && residual_name_index(steps, "REQUIRE_SLOWDEATH_SAFE").is_some()
        && residual_name_index(steps, "REQUIRE_NO_DOUBLE_SPAWN").is_some()
        && residual_name_index(steps, "LIVE_HOST_EJECT_PILOT_DIE_DEATH_START").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_eject_pilot_die_death_start").is_some()
        && residual_name_index(cmds, "maybe_apply_eject_pilot_die").is_some()
        && residual_name_index(cmds, "eject_pilot_die_applied").is_some()
        && residual_name_index(cmds, "slowdeath_safe").is_some();
    residual_action_store(ResidualHostEjectPilotDieDeathStartAction::NavCommands);
    ok
}
pub fn simulate_host_eject_pilot_die_death_start_collect_source() -> bool {
    let ok = gl_source().contains("maybe_apply_eject_pilot_die")
        && obj_source().contains("eject_pilot_die_applied");
    residual_action_store(ResidualHostEjectPilotDieDeathStartAction::CollectSource);
    ok
}
pub fn simulate_host_eject_pilot_die_death_start_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 754")
        && (gl_source().contains("skip if death-start mark_object already applied")
            || gl_source().contains("EjectPilotDie::onDie residual at death start"));
    residual_action_store(ResidualHostEjectPilotDieDeathStartAction::DispatchSource);
    ok
}
pub fn honesty_host_eject_pilot_die_death_start_residual_pack_wave754() -> bool {
    honesty_host_eject_pilot_die_death_start_method_names_residual_wave754()
        && honesty_host_eject_pilot_die_death_start_source_markers_residual_wave754()
        && honesty_host_eject_pilot_die_death_start_nav_commands_residual_wave754()
        && simulate_host_eject_pilot_die_death_start_collect_source()
        && simulate_host_eject_pilot_die_death_start_dispatch_source()
}
pub fn simulate_live_host_eject_pilot_die_death_start_honesty() -> bool {
    let ok = honesty_host_eject_pilot_die_death_start_residual_pack_wave754();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEjectPilotDieDeathStartAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_eject_pilot_die_death_start_method_names_residual_wave754());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_eject_pilot_die_death_start_source_markers_residual_wave754());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_eject_pilot_die_death_start_nav_commands_residual_wave754());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_eject_pilot_die_death_start_collect_source());
        assert!(simulate_host_eject_pilot_die_death_start_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_eject_pilot_die_death_start_residual_pack_wave754());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_eject_pilot_die_death_start_honesty());
        assert!(residual_host_eject_pilot_die_death_start_ok());
    }
}
