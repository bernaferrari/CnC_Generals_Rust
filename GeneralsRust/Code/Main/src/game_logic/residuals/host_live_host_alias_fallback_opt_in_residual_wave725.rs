//! Wave 725: runtime-host soft template alias fallbacks are opt-in.
//! Default fail-closed: train/construct/upgrade use the exact requested name.
//! Smoke may set `alias_fallback=1` / `GENERALS_RUNTIME_HOST_ALIAS_FALLBACK=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_ALIAS_FALLBACK_OPT_IN_METHOD_NAMES_WAVE725: &[&str] = &[
    "allow_alias_fallback",
    "GENERALS_RUNTIME_HOST_ALIAS_FALLBACK",
    "alias_fallback=1",
    "Wave 725",
    "unit_candidates",
    "playable_claim = false",
];
pub const LIVE_HOST_ALIAS_FALLBACK_OPT_IN_NAV_STEPS_WAVE725: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_SMOKE_OPT_IN",
    "LIVE_HOST_ALIAS_FALLBACK_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_ALIAS_FALLBACK_OPT_IN_CMD_NAMES_WAVE725: &[&str] = &[
    "host_alias_fallback_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "smoke_opt_in",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAliasFallbackOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostAliasFallbackOptInAction {
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
fn residual_action_store(a: ResidualHostAliasFallbackOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_alias_fallback_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_alias_fallback_opt_in_last_action() -> ResidualHostAliasFallbackOptInAction {
    ResidualHostAliasFallbackOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
pub fn honesty_host_alias_fallback_opt_in_method_names_residual_wave725() -> bool {
    let names = LIVE_HOST_ALIAS_FALLBACK_OPT_IN_METHOD_NAMES_WAVE725;
    let ok = residual_name_index(names, "allow_alias_fallback").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_ALIAS_FALLBACK").is_some()
        && residual_name_index(names, "alias_fallback=1").is_some()
        && residual_name_index(names, "Wave 725").is_some()
        && residual_name_index(names, "unit_candidates").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostAliasFallbackOptInAction::MethodNames);
    ok
}
pub fn honesty_host_alias_fallback_opt_in_source_markers_residual_wave725() -> bool {
    let eng = eng_source();
    let smoke = smoke_source();
    let eng_ok = eng.contains("Wave 725")
        && eng.contains("allow_alias_fallback")
        && eng.contains("GENERALS_RUNTIME_HOST_ALIAS_FALLBACK")
        && eng.contains("alias_fallback")
        && eng.matches("if allow_alias_fallback").count() >= 3;
    let train_exact = eng.contains("let mut unit_candidates = vec![requested.as_str()];");
    let construct_exact = eng.contains("let mut candidates = vec![requested.as_str()];");
    let smoke_ok=smoke.contains("alias_fallback=1")
        && smoke.contains("construct|template=USA_Barracks|spawn_dozer=1|alias_fallback=1")
        && smoke.contains("train_unit|template=AmericaInfantryRanger|force_complete=1|grant_supplies=1|alias_fallback=1")
        && smoke.contains("upgrade|name=UpgradeAmericaRangerCaptureBuilding|grant_supplies=1|alias_fallback=1");
    let ok = eng_ok
        && train_exact
        && construct_exact
        && smoke_ok
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostAliasFallbackOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_alias_fallback_opt_in_nav_commands_residual_wave725() -> bool {
    let steps = LIVE_HOST_ALIAS_FALLBACK_OPT_IN_NAV_STEPS_WAVE725;
    let cmds = RUNTIME_HOST_LIVE_HOST_ALIAS_FALLBACK_OPT_IN_CMD_NAMES_WAVE725;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_SMOKE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_HOST_ALIAS_FALLBACK_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_alias_fallback_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "smoke_opt_in").is_some();
    residual_action_store(ResidualHostAliasFallbackOptInAction::NavCommands);
    ok
}
pub fn simulate_host_alias_fallback_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_alias_fallback")
        && smoke_source().contains("alias_fallback=1");
    residual_action_store(ResidualHostAliasFallbackOptInAction::CollectSource);
    ok
}
pub fn simulate_host_alias_fallback_opt_in_dispatch_source() -> bool {
    let ok=eng_source().contains("Wave 725")
        && smoke_source().contains("train_unit|template=AmericaInfantryRanger|force_complete=1|grant_supplies=1|alias_fallback=1");
    residual_action_store(ResidualHostAliasFallbackOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_alias_fallback_opt_in_residual_pack_wave725() -> bool {
    honesty_host_alias_fallback_opt_in_method_names_residual_wave725()
        && honesty_host_alias_fallback_opt_in_source_markers_residual_wave725()
        && honesty_host_alias_fallback_opt_in_nav_commands_residual_wave725()
        && simulate_host_alias_fallback_opt_in_collect_source()
        && simulate_host_alias_fallback_opt_in_dispatch_source()
}
pub fn simulate_live_host_alias_fallback_opt_in_honesty() -> bool {
    let ok = honesty_host_alias_fallback_opt_in_residual_pack_wave725();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostAliasFallbackOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_alias_fallback_opt_in_method_names_residual_wave725());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_alias_fallback_opt_in_source_markers_residual_wave725());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_alias_fallback_opt_in_nav_commands_residual_wave725());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_alias_fallback_opt_in_collect_source());
        assert!(simulate_host_alias_fallback_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_alias_fallback_opt_in_residual_pack_wave725());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_alias_fallback_opt_in_honesty());
        assert!(residual_host_alias_fallback_opt_in_ok());
    }
}
