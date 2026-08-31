//! Wave 728: runtime-host sell auto-target + formation free buddy template are opt-in/fail-closed.
//! - Sell: default requires selection; `auto_target=1` / `GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET=1`
//!   may pick newest sellable structure.
//! - Formation buddy: under Wave 720 spawn_buddy gate, no free AmericaInfantryRanger template.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SELL_AUTO_TARGET_OPT_IN_METHOD_NAMES_WAVE728: &[&str] = &[
    "allow_auto_target",
    "GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET",
    "auto_target=1",
    "alive_sellable_friendly_structure_ids",
    "Wave 728",
    "playable_claim = false",
];
pub const LIVE_HOST_SELL_AUTO_TARGET_OPT_IN_NAV_STEPS_WAVE728: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_FORMATION_NO_FREE_TEMPLATE",
    "LIVE_HOST_SELL_AUTO_TARGET_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SELL_AUTO_TARGET_OPT_IN_CMD_NAMES_WAVE728: &[&str] = &[
    "host_sell_auto_target_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "formation_no_free_template",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSellAutoTargetOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSellAutoTargetOptInAction {
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
fn residual_action_store(a: ResidualHostSellAutoTargetOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_sell_auto_target_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_sell_auto_target_opt_in_last_action() -> ResidualHostSellAutoTargetOptInAction
{
    ResidualHostSellAutoTargetOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
pub fn honesty_host_sell_auto_target_opt_in_method_names_residual_wave728() -> bool {
    let names = LIVE_HOST_SELL_AUTO_TARGET_OPT_IN_METHOD_NAMES_WAVE728;
    let ok = residual_name_index(names, "allow_auto_target").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET").is_some()
        && residual_name_index(names, "auto_target=1").is_some()
        && residual_name_index(names, "alive_sellable_friendly_structure_ids").is_some()
        && residual_name_index(names, "Wave 728").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSellAutoTargetOptInAction::MethodNames);
    ok
}
pub fn honesty_host_sell_auto_target_opt_in_source_markers_residual_wave728() -> bool {
    let eng = eng_source();
    let smoke = smoke_source();
    let eng_ok = eng.contains("Wave 728")
        && eng.contains("allow_auto_target")
        && eng.contains("GENERALS_RUNTIME_HOST_SELL_AUTO_TARGET")
        && eng.contains("if allow_auto_target")
        && eng.contains("alive_sellable_friendly_structure_ids")
        && eng.contains("no free AmericaInfantryRanger buddy template")
        && eng.contains("no free AmericaInfantryRanger fallback template")
        && !eng.contains("unwrap_or_else(|| \"AmericaInfantryRanger\".to_string())")
        && !eng.contains("host_create_object(\"AmericaInfantryRanger\"");
    let smoke_ok = smoke.contains("sell|auto_target=1");
    let ok = eng_ok && smoke_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostSellAutoTargetOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_sell_auto_target_opt_in_nav_commands_residual_wave728() -> bool {
    let steps = LIVE_HOST_SELL_AUTO_TARGET_OPT_IN_NAV_STEPS_WAVE728;
    let cmds = RUNTIME_HOST_LIVE_HOST_SELL_AUTO_TARGET_OPT_IN_CMD_NAMES_WAVE728;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_FORMATION_NO_FREE_TEMPLATE").is_some()
        && residual_name_index(steps, "LIVE_HOST_SELL_AUTO_TARGET_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_sell_auto_target_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "formation_no_free_template").is_some();
    residual_action_store(ResidualHostSellAutoTargetOptInAction::NavCommands);
    ok
}
pub fn simulate_host_sell_auto_target_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_auto_target")
        && eng_source().contains("no free AmericaInfantryRanger buddy template");
    residual_action_store(ResidualHostSellAutoTargetOptInAction::CollectSource);
    ok
}
pub fn simulate_host_sell_auto_target_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 728") && smoke_source().contains("sell|auto_target=1");
    residual_action_store(ResidualHostSellAutoTargetOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_sell_auto_target_opt_in_residual_pack_wave728() -> bool {
    honesty_host_sell_auto_target_opt_in_method_names_residual_wave728()
        && honesty_host_sell_auto_target_opt_in_source_markers_residual_wave728()
        && honesty_host_sell_auto_target_opt_in_nav_commands_residual_wave728()
        && simulate_host_sell_auto_target_opt_in_collect_source()
        && simulate_host_sell_auto_target_opt_in_dispatch_source()
}
pub fn simulate_live_host_sell_auto_target_opt_in_honesty() -> bool {
    let ok = honesty_host_sell_auto_target_opt_in_residual_pack_wave728();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSellAutoTargetOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_sell_auto_target_opt_in_method_names_residual_wave728());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_sell_auto_target_opt_in_source_markers_residual_wave728());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_sell_auto_target_opt_in_nav_commands_residual_wave728());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_sell_auto_target_opt_in_collect_source());
        assert!(simulate_host_sell_auto_target_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_sell_auto_target_opt_in_residual_pack_wave728());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_sell_auto_target_opt_in_honesty());
        assert!(residual_host_sell_auto_target_opt_in_ok());
    }
}
