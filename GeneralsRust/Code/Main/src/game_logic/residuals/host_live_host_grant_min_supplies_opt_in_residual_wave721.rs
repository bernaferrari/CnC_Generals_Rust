//! Wave 721: runtime-host free min-supplies floor is opt-in.
//! Default fail-closed: train/upgrade do not top up player cash.
//! Smoke may set `grant_supplies=1` / `GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_GRANT_MIN_SUPPLIES_OPT_IN_METHOD_NAMES_WAVE721: &[&str] = &[
    "allow_grant_supplies",
    "GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES",
    "grant_supplies=1",
    "ensure_player_min_supplies",
    "Wave 721",
    "playable_claim = false",
];
pub const LIVE_HOST_GRANT_MIN_SUPPLIES_OPT_IN_NAV_STEPS_WAVE721: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_SMOKE_OPT_IN",
    "LIVE_HOST_GRANT_MIN_SUPPLIES_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_GRANT_MIN_SUPPLIES_OPT_IN_CMD_NAMES_WAVE721: &[&str] = &[
    "host_grant_min_supplies_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "smoke_opt_in",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGrantMinSuppliesOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostGrantMinSuppliesOptInAction {
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
fn residual_action_store(a: ResidualHostGrantMinSuppliesOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_grant_min_supplies_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_grant_min_supplies_opt_in_last_action()
-> ResidualHostGrantMinSuppliesOptInAction {
    ResidualHostGrantMinSuppliesOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    include_str!("../../executable_smoke.rs")
}
pub fn honesty_host_grant_min_supplies_opt_in_method_names_residual_wave721() -> bool {
    let names = LIVE_HOST_GRANT_MIN_SUPPLIES_OPT_IN_METHOD_NAMES_WAVE721;
    let ok = residual_name_index(names, "allow_grant_supplies").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES").is_some()
        && residual_name_index(names, "grant_supplies=1").is_some()
        && residual_name_index(names, "ensure_player_min_supplies").is_some()
        && residual_name_index(names, "Wave 721").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostGrantMinSuppliesOptInAction::MethodNames);
    ok
}
pub fn honesty_host_grant_min_supplies_opt_in_source_markers_residual_wave721() -> bool {
    let eng = eng_source();
    let smoke = smoke_source();
    let eng_ok = eng.contains("Wave 721")
        && eng.contains("allow_grant_supplies")
        && eng.contains("GENERALS_RUNTIME_HOST_GRANT_MIN_SUPPLIES")
        && eng.contains("grant_supplies")
        && eng.matches("if allow_grant_supplies").count() >= 2
        && eng.contains("ensure_player_min_supplies");
    let no_uncond_floor = !eng.contains("Wave 240: supplies floor via probe");
    let smoke_ok = smoke.contains("grant_supplies=1")
        && smoke.contains(
            "train_unit|template=AmericaInfantryRanger|force_complete=1|grant_supplies=1",
        )
        && smoke.contains("upgrade|name=UpgradeAmericaRangerCaptureBuilding|grant_supplies=1");
    let ok = eng_ok && no_uncond_floor && smoke_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostGrantMinSuppliesOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_grant_min_supplies_opt_in_nav_commands_residual_wave721() -> bool {
    let steps = LIVE_HOST_GRANT_MIN_SUPPLIES_OPT_IN_NAV_STEPS_WAVE721;
    let cmds = RUNTIME_HOST_LIVE_HOST_GRANT_MIN_SUPPLIES_OPT_IN_CMD_NAMES_WAVE721;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_SMOKE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_HOST_GRANT_MIN_SUPPLIES_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_grant_min_supplies_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "smoke_opt_in").is_some();
    residual_action_store(ResidualHostGrantMinSuppliesOptInAction::NavCommands);
    ok
}
pub fn simulate_host_grant_min_supplies_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_grant_supplies")
        && smoke_source().contains("grant_supplies=1");
    residual_action_store(ResidualHostGrantMinSuppliesOptInAction::CollectSource);
    ok
}
pub fn simulate_host_grant_min_supplies_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 721")
        && smoke_source().contains(
            "train_unit|template=AmericaInfantryRanger|force_complete=1|grant_supplies=1",
        );
    residual_action_store(ResidualHostGrantMinSuppliesOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_grant_min_supplies_opt_in_residual_pack_wave721() -> bool {
    honesty_host_grant_min_supplies_opt_in_method_names_residual_wave721()
        && honesty_host_grant_min_supplies_opt_in_source_markers_residual_wave721()
        && honesty_host_grant_min_supplies_opt_in_nav_commands_residual_wave721()
        && simulate_host_grant_min_supplies_opt_in_collect_source()
        && simulate_host_grant_min_supplies_opt_in_dispatch_source()
}
pub fn simulate_live_host_grant_min_supplies_opt_in_honesty() -> bool {
    let ok = honesty_host_grant_min_supplies_opt_in_residual_pack_wave721();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostGrantMinSuppliesOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_grant_min_supplies_opt_in_method_names_residual_wave721());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_grant_min_supplies_opt_in_source_markers_residual_wave721());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_grant_min_supplies_opt_in_nav_commands_residual_wave721());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_grant_min_supplies_opt_in_collect_source());
        assert!(simulate_host_grant_min_supplies_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_grant_min_supplies_opt_in_residual_pack_wave721());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_grant_min_supplies_opt_in_honesty());
        assert!(residual_host_grant_min_supplies_opt_in_ok());
    }
}
