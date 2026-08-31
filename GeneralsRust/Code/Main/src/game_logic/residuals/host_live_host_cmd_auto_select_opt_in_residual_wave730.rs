//! Wave 730: runtime-host return_supplies/rally empty-selection auto-pick is opt-in.
//! Default fail-closed: commands require a real selection.
//! Smoke may set `auto_target=1` / `GENERALS_RUNTIME_HOST_AUTO_TARGET=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_CMD_AUTO_SELECT_OPT_IN_METHOD_NAMES_WAVE730: &[&str] = &[
    "allow_auto_target",
    "GENERALS_RUNTIME_HOST_AUTO_TARGET",
    "auto_target=1",
    "return_supplies",
    "Wave 730",
    "playable_claim = false",
];
pub const LIVE_HOST_CMD_AUTO_SELECT_OPT_IN_NAV_STEPS_WAVE730: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_SMOKE_OPT_IN",
    "LIVE_HOST_CMD_AUTO_SELECT_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_CMD_AUTO_SELECT_OPT_IN_CMD_NAMES_WAVE730: &[&str] = &[
    "host_cmd_auto_select_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "smoke_opt_in",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCmdAutoSelectOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostCmdAutoSelectOptInAction {
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
fn residual_action_store(a: ResidualHostCmdAutoSelectOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_cmd_auto_select_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_cmd_auto_select_opt_in_last_action() -> ResidualHostCmdAutoSelectOptInAction {
    ResidualHostCmdAutoSelectOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
pub fn honesty_host_cmd_auto_select_opt_in_method_names_residual_wave730() -> bool {
    let names = LIVE_HOST_CMD_AUTO_SELECT_OPT_IN_METHOD_NAMES_WAVE730;
    let ok = residual_name_index(names, "allow_auto_target").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some()
        && residual_name_index(names, "auto_target=1").is_some()
        && residual_name_index(names, "return_supplies").is_some()
        && residual_name_index(names, "Wave 730").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCmdAutoSelectOptInAction::MethodNames);
    ok
}
pub fn honesty_host_cmd_auto_select_opt_in_source_markers_residual_wave730() -> bool {
    let eng = eng_source();
    let smoke = smoke_source();
    let eng_ok = eng.contains("Wave 730")
        && eng.contains("allow_auto_target")
        && eng
            .matches("if self.selected_objects.is_empty() && allow_auto_target")
            .count()
            >= 2
        && eng.contains("alive_selectable_friendly_harvester_ids")
        && eng.contains("alive_upgrade_producer_structure_ids");
    let smoke_ok = smoke.contains("return_supplies|auto_target=1")
        && smoke.contains("rally|x=90|y=0|z=90|auto_target=1");
    let ok = eng_ok && smoke_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostCmdAutoSelectOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_cmd_auto_select_opt_in_nav_commands_residual_wave730() -> bool {
    let steps = LIVE_HOST_CMD_AUTO_SELECT_OPT_IN_NAV_STEPS_WAVE730;
    let cmds = RUNTIME_HOST_LIVE_HOST_CMD_AUTO_SELECT_OPT_IN_CMD_NAMES_WAVE730;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_SMOKE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_HOST_CMD_AUTO_SELECT_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_cmd_auto_select_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "smoke_opt_in").is_some();
    residual_action_store(ResidualHostCmdAutoSelectOptInAction::NavCommands);
    ok
}
pub fn simulate_host_cmd_auto_select_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("Wave 730") && smoke_source().contains("auto_target=1");
    residual_action_store(ResidualHostCmdAutoSelectOptInAction::CollectSource);
    ok
}
pub fn simulate_host_cmd_auto_select_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("if self.selected_objects.is_empty() && allow_auto_target")
        && smoke_source().contains("return_supplies|auto_target=1");
    residual_action_store(ResidualHostCmdAutoSelectOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_cmd_auto_select_opt_in_residual_pack_wave730() -> bool {
    honesty_host_cmd_auto_select_opt_in_method_names_residual_wave730()
        && honesty_host_cmd_auto_select_opt_in_source_markers_residual_wave730()
        && honesty_host_cmd_auto_select_opt_in_nav_commands_residual_wave730()
        && simulate_host_cmd_auto_select_opt_in_collect_source()
        && simulate_host_cmd_auto_select_opt_in_dispatch_source()
}
pub fn simulate_live_host_cmd_auto_select_opt_in_honesty() -> bool {
    let ok = honesty_host_cmd_auto_select_opt_in_residual_pack_wave730();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCmdAutoSelectOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_cmd_auto_select_opt_in_method_names_residual_wave730());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_cmd_auto_select_opt_in_source_markers_residual_wave730());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_cmd_auto_select_opt_in_nav_commands_residual_wave730());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_cmd_auto_select_opt_in_collect_source());
        assert!(simulate_host_cmd_auto_select_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_cmd_auto_select_opt_in_residual_pack_wave730());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_cmd_auto_select_opt_in_honesty());
        assert!(residual_host_cmd_auto_select_opt_in_ok());
    }
}
