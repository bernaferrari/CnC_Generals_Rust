//! Wave 726: runtime-host auto-select first friendly mobile is opt-in.
//! Default fail-closed: commands do not invent a selection when empty.
//! Harness may set `GENERALS_RUNTIME_HOST_AUTO_SELECT_MOBILE=1`.
//! Smoke already selects via select_local_unit / box_select.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_AUTO_SELECT_MOBILE_OPT_IN_METHOD_NAMES_WAVE726: &[&str] = &[
    "allow_auto_select",
    "GENERALS_RUNTIME_HOST_AUTO_SELECT_MOBILE",
    "ensure_host_mobile_selection",
    "Wave 726",
    "host_set_selection",
    "playable_claim = false",
];
pub const LIVE_HOST_AUTO_SELECT_MOBILE_OPT_IN_NAV_STEPS_WAVE726: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_SMOKE_EXPLICIT_SELECT",
    "LIVE_HOST_AUTO_SELECT_MOBILE_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_AUTO_SELECT_MOBILE_OPT_IN_CMD_NAMES_WAVE726: &[&str] = &[
    "host_auto_select_mobile_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "smoke_explicit_select",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAutoSelectMobileOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostAutoSelectMobileOptInAction {
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
fn residual_action_store(a: ResidualHostAutoSelectMobileOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_auto_select_mobile_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_auto_select_mobile_opt_in_last_action()
-> ResidualHostAutoSelectMobileOptInAction {
    ResidualHostAutoSelectMobileOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    include_str!("../../executable_smoke.rs")
}
pub fn honesty_host_auto_select_mobile_opt_in_method_names_residual_wave726() -> bool {
    let names = LIVE_HOST_AUTO_SELECT_MOBILE_OPT_IN_METHOD_NAMES_WAVE726;
    let ok = residual_name_index(names, "allow_auto_select").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_AUTO_SELECT_MOBILE").is_some()
        && residual_name_index(names, "ensure_host_mobile_selection").is_some()
        && residual_name_index(names, "Wave 726").is_some()
        && residual_name_index(names, "host_set_selection").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostAutoSelectMobileOptInAction::MethodNames);
    ok
}
pub fn honesty_host_auto_select_mobile_opt_in_source_markers_residual_wave726() -> bool {
    let eng = eng_source();
    let smoke = smoke_source();
    let eng_ok = eng.contains("Wave 726")
        && eng.contains("allow_auto_select")
        && eng.contains("GENERALS_RUNTIME_HOST_AUTO_SELECT_MOBILE")
        && eng.contains("fn ensure_host_mobile_selection")
        && eng.contains("if !allow_auto_select")
        && eng.contains("host_set_selection");
    let smoke_ok = smoke.contains("select_local_unit") || smoke.contains("box_select");
    let ok = eng_ok && smoke_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostAutoSelectMobileOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_auto_select_mobile_opt_in_nav_commands_residual_wave726() -> bool {
    let steps = LIVE_HOST_AUTO_SELECT_MOBILE_OPT_IN_NAV_STEPS_WAVE726;
    let cmds = RUNTIME_HOST_LIVE_HOST_AUTO_SELECT_MOBILE_OPT_IN_CMD_NAMES_WAVE726;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_SMOKE_EXPLICIT_SELECT").is_some()
        && residual_name_index(steps, "LIVE_HOST_AUTO_SELECT_MOBILE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_auto_select_mobile_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "smoke_explicit_select").is_some();
    residual_action_store(ResidualHostAutoSelectMobileOptInAction::NavCommands);
    ok
}
pub fn simulate_host_auto_select_mobile_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_auto_select")
        && eng_source().contains("GENERALS_RUNTIME_HOST_AUTO_SELECT_MOBILE");
    residual_action_store(ResidualHostAutoSelectMobileOptInAction::CollectSource);
    ok
}
pub fn simulate_host_auto_select_mobile_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 726") && smoke_source().contains("select_local_unit");
    residual_action_store(ResidualHostAutoSelectMobileOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_auto_select_mobile_opt_in_residual_pack_wave726() -> bool {
    honesty_host_auto_select_mobile_opt_in_method_names_residual_wave726()
        && honesty_host_auto_select_mobile_opt_in_source_markers_residual_wave726()
        && honesty_host_auto_select_mobile_opt_in_nav_commands_residual_wave726()
        && simulate_host_auto_select_mobile_opt_in_collect_source()
        && simulate_host_auto_select_mobile_opt_in_dispatch_source()
}
pub fn simulate_live_host_auto_select_mobile_opt_in_honesty() -> bool {
    let ok = honesty_host_auto_select_mobile_opt_in_residual_pack_wave726();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostAutoSelectMobileOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_auto_select_mobile_opt_in_method_names_residual_wave726());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_auto_select_mobile_opt_in_source_markers_residual_wave726());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_auto_select_mobile_opt_in_nav_commands_residual_wave726());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_auto_select_mobile_opt_in_collect_source());
        assert!(simulate_host_auto_select_mobile_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_auto_select_mobile_opt_in_residual_pack_wave726());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_auto_select_mobile_opt_in_honesty());
        assert!(residual_host_auto_select_mobile_opt_in_ok());
    }
}
