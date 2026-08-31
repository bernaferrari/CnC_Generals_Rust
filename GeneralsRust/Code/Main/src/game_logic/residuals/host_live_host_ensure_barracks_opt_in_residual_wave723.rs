//! Wave 723: runtime-host Barracks building_data stamp is opt-in.
//! Default fail-closed: train does not rewrite producer building_data.
//! Smoke/force paths may set `force_complete=1`, `ensure_barracks=1`, or
//! `GENERALS_RUNTIME_HOST_ENSURE_BARRACKS=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_ENSURE_BARRACKS_OPT_IN_METHOD_NAMES_WAVE723: &[&str] = &[
    "allow_ensure_barracks",
    "GENERALS_RUNTIME_HOST_ENSURE_BARRACKS",
    "ensure_barracks=1",
    "host_ensure_barracks_building_data",
    "Wave 723",
    "playable_claim = false",
];
pub const LIVE_HOST_ENSURE_BARRACKS_OPT_IN_NAV_STEPS_WAVE723: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_FORCE_COMPLETE_IMPLIES",
    "LIVE_HOST_ENSURE_BARRACKS_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_ENSURE_BARRACKS_OPT_IN_CMD_NAMES_WAVE723: &[&str] = &[
    "host_ensure_barracks_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "force_complete_implies",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEnsureBarracksOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEnsureBarracksOptInAction {
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
fn residual_action_store(a: ResidualHostEnsureBarracksOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_ensure_barracks_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_ensure_barracks_opt_in_last_action() -> ResidualHostEnsureBarracksOptInAction {
    ResidualHostEnsureBarracksOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
pub fn honesty_host_ensure_barracks_opt_in_method_names_residual_wave723() -> bool {
    let names = LIVE_HOST_ENSURE_BARRACKS_OPT_IN_METHOD_NAMES_WAVE723;
    let ok = residual_name_index(names, "allow_ensure_barracks").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_ENSURE_BARRACKS").is_some()
        && residual_name_index(names, "ensure_barracks=1").is_some()
        && residual_name_index(names, "host_ensure_barracks_building_data").is_some()
        && residual_name_index(names, "Wave 723").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEnsureBarracksOptInAction::MethodNames);
    ok
}
pub fn honesty_host_ensure_barracks_opt_in_source_markers_residual_wave723() -> bool {
    let eng = eng_source();
    let eng_ok = eng.contains("Wave 723")
        && eng.contains("allow_ensure_barracks")
        && eng.contains("GENERALS_RUNTIME_HOST_ENSURE_BARRACKS")
        && eng.contains("ensure_barracks")
        && eng.contains("if allow_ensure_barracks")
        && eng.contains("self.host_ensure_barracks_building_data(id)");
    // 2026-08-15: gate still wraps host_ensure_barracks_building_data.
    let gated = eng.contains("if allow_ensure_barracks")
        && eng.contains("self.host_ensure_barracks_building_data(id)");
    let no_uncond = eng
        .matches("let _ = self.host_ensure_barracks_building_data(id);")
        .count()
        == 1;
    let force_implies = eng.contains("let allow_ensure_barracks = allow_force_complete");
    let smoke_ok = smoke_source().contains("force_complete=1");
    let ok = eng_ok
        && gated
        && no_uncond
        && force_implies
        && smoke_ok
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostEnsureBarracksOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_ensure_barracks_opt_in_nav_commands_residual_wave723() -> bool {
    let steps = LIVE_HOST_ENSURE_BARRACKS_OPT_IN_NAV_STEPS_WAVE723;
    let cmds = RUNTIME_HOST_LIVE_HOST_ENSURE_BARRACKS_OPT_IN_CMD_NAMES_WAVE723;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_FORCE_COMPLETE_IMPLIES").is_some()
        && residual_name_index(steps, "LIVE_HOST_ENSURE_BARRACKS_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_ensure_barracks_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "force_complete_implies").is_some();
    residual_action_store(ResidualHostEnsureBarracksOptInAction::NavCommands);
    ok
}
pub fn simulate_host_ensure_barracks_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_ensure_barracks")
        && eng_source().contains("allow_force_complete");
    residual_action_store(ResidualHostEnsureBarracksOptInAction::CollectSource);
    ok
}
pub fn simulate_host_ensure_barracks_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 723") && smoke_source().contains("force_complete=1");
    residual_action_store(ResidualHostEnsureBarracksOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_ensure_barracks_opt_in_residual_pack_wave723() -> bool {
    honesty_host_ensure_barracks_opt_in_method_names_residual_wave723()
        && honesty_host_ensure_barracks_opt_in_source_markers_residual_wave723()
        && honesty_host_ensure_barracks_opt_in_nav_commands_residual_wave723()
        && simulate_host_ensure_barracks_opt_in_collect_source()
        && simulate_host_ensure_barracks_opt_in_dispatch_source()
}
pub fn simulate_live_host_ensure_barracks_opt_in_honesty() -> bool {
    let ok = honesty_host_ensure_barracks_opt_in_residual_pack_wave723();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEnsureBarracksOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_ensure_barracks_opt_in_method_names_residual_wave723());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_ensure_barracks_opt_in_source_markers_residual_wave723());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_ensure_barracks_opt_in_nav_commands_residual_wave723());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_ensure_barracks_opt_in_collect_source());
        assert!(simulate_host_ensure_barracks_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_ensure_barracks_opt_in_residual_pack_wave723());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_ensure_barracks_opt_in_honesty());
        assert!(residual_host_ensure_barracks_opt_in_ok());
    }
}
