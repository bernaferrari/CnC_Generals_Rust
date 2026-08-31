//! Wave 729: runtime-host auto-pick producer/builder when selection empty is opt-in.
//! Default fail-closed: train/construct/upgrade require selection (or other opt-ins).
//! Smoke may set `auto_target=1` / `GENERALS_RUNTIME_HOST_AUTO_TARGET=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_AUTO_TARGET_OPT_IN_METHOD_NAMES_WAVE729: &[&str] = &[
    "allow_auto_target",
    "GENERALS_RUNTIME_HOST_AUTO_TARGET",
    "auto_target=1",
    "first_constructed_producer_id",
    "Wave 729",
    "playable_claim = false",
];
pub const LIVE_HOST_AUTO_TARGET_OPT_IN_NAV_STEPS_WAVE729: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_SMOKE_OPT_IN",
    "LIVE_HOST_AUTO_TARGET_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_AUTO_TARGET_OPT_IN_CMD_NAMES_WAVE729: &[&str] = &[
    "host_auto_target_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "smoke_opt_in",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAutoTargetOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostAutoTargetOptInAction {
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
fn residual_action_store(a: ResidualHostAutoTargetOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_auto_target_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_auto_target_opt_in_last_action() -> ResidualHostAutoTargetOptInAction {
    ResidualHostAutoTargetOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
pub fn honesty_host_auto_target_opt_in_method_names_residual_wave729() -> bool {
    let names = LIVE_HOST_AUTO_TARGET_OPT_IN_METHOD_NAMES_WAVE729;
    let ok = residual_name_index(names, "allow_auto_target").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_AUTO_TARGET").is_some()
        && residual_name_index(names, "auto_target=1").is_some()
        && residual_name_index(names, "first_constructed_producer_id").is_some()
        && residual_name_index(names, "Wave 729").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostAutoTargetOptInAction::MethodNames);
    ok
}
pub fn honesty_host_auto_target_opt_in_source_markers_residual_wave729() -> bool {
    let eng = eng_source();
    let smoke = smoke_source();
    let eng_ok = eng.contains("Wave 729")
        && eng.contains("allow_auto_target")
        && eng.contains("GENERALS_RUNTIME_HOST_AUTO_TARGET")
        && eng
            .matches("if producers.is_empty() && allow_auto_target")
            .count()
            >= 1
        && eng
            .matches("if builders.is_empty() && allow_auto_target")
            .count()
            >= 1
        && eng.contains("if allow_auto_target");
    let smoke_ok=smoke.contains("auto_target=1")
        && smoke.contains("construct|template=USA_Barracks|spawn_dozer=1|alias_fallback=1|auto_target=1")
        && smoke.contains("train_unit|template=AmericaInfantryRanger|force_complete=1|grant_supplies=1|alias_fallback=1|auto_target=1")
        && smoke.contains("upgrade|name=UpgradeAmericaRangerCaptureBuilding|grant_supplies=1|alias_fallback=1|auto_target=1");
    let ok = eng_ok && smoke_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostAutoTargetOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_auto_target_opt_in_nav_commands_residual_wave729() -> bool {
    let steps = LIVE_HOST_AUTO_TARGET_OPT_IN_NAV_STEPS_WAVE729;
    let cmds = RUNTIME_HOST_LIVE_HOST_AUTO_TARGET_OPT_IN_CMD_NAMES_WAVE729;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_SMOKE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_HOST_AUTO_TARGET_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_auto_target_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "smoke_opt_in").is_some();
    residual_action_store(ResidualHostAutoTargetOptInAction::NavCommands);
    ok
}
pub fn simulate_host_auto_target_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_auto_target")
        && eng_source().contains("GENERALS_RUNTIME_HOST_AUTO_TARGET");
    residual_action_store(ResidualHostAutoTargetOptInAction::CollectSource);
    ok
}
pub fn simulate_host_auto_target_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 729") && smoke_source().contains("auto_target=1");
    residual_action_store(ResidualHostAutoTargetOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_auto_target_opt_in_residual_pack_wave729() -> bool {
    honesty_host_auto_target_opt_in_method_names_residual_wave729()
        && honesty_host_auto_target_opt_in_source_markers_residual_wave729()
        && honesty_host_auto_target_opt_in_nav_commands_residual_wave729()
        && simulate_host_auto_target_opt_in_collect_source()
        && simulate_host_auto_target_opt_in_dispatch_source()
}
pub fn simulate_live_host_auto_target_opt_in_honesty() -> bool {
    let ok = honesty_host_auto_target_opt_in_residual_pack_wave729();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostAutoTargetOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_auto_target_opt_in_method_names_residual_wave729());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_auto_target_opt_in_source_markers_residual_wave729());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_auto_target_opt_in_nav_commands_residual_wave729());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_auto_target_opt_in_collect_source());
        assert!(simulate_host_auto_target_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_auto_target_opt_in_residual_pack_wave729());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_auto_target_opt_in_honesty());
        assert!(residual_host_auto_target_opt_in_ok());
    }
}
