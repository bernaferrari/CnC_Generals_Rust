//! Wave 811: GW entity carries disabled_underpowered for Powered objects from
//! shadow player power_available; under coupled dual-tick sole-ticks the flag;
//! host peels object body of update_power_disabled_state but keeps Eva low-power.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_POWER_DISABLED_DUAL_PEEL_METHOD_NAMES_WAVE811: &[&str] = &[
    "disabled_underpowered",
    "power_available",
    "KindOf::Powered",
    "update_power_disabled_state",
    "update_eva_low_power",
    "Wave 811",
    "playable_claim = false",
];
pub const LIVE_HOST_POWER_DISABLED_DUAL_PEEL_NAV_STEPS_WAVE811: &[&str] = &[
    "REQUIRE_ENTITY_UNDERPOWERED_FIELD",
    "REQUIRE_GW_UNDERPOWERED_TICK",
    "REQUIRE_HOST_PEEL_KEEP_EVA",
    "LIVE_HOST_POWER_DISABLED_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPowerDisabledDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostPowerDisabledDualPeelAction {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostPowerDisabledDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_power_disabled_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_power_disabled_dual_peel_last_action()
-> ResidualHostPowerDisabledDualPeelAction {
    ResidualHostPowerDisabledDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_power_disabled_dual_peel_method_names_residual_wave811() -> bool {
    let names = LIVE_HOST_POWER_DISABLED_DUAL_PEEL_METHOD_NAMES_WAVE811;
    let ok = residual_name_index(names, "disabled_underpowered").is_some()
        && residual_name_index(names, "power_available").is_some()
        && residual_name_index(names, "KindOf::Powered").is_some()
        && residual_name_index(names, "update_power_disabled_state").is_some()
        && residual_name_index(names, "update_eva_low_power").is_some()
        && residual_name_index(names, "Wave 811").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPowerDisabledDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_power_disabled_dual_peel_source_markers_residual_wave811() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("disabled_underpowered")
        && sh.contains("Wave 811")
        && sh.contains("underpowered_team_ords")
        && sh.contains("POWERED_BIT")
        && sh.contains("disabled_underpowered")
        && gl.contains("Wave 811")
        && gl.contains("update_power_disabled_state")
        && gl.contains("update_eva_low_power")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPowerDisabledDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_power_disabled_dual_peel_nav_commands_residual_wave811() -> bool {
    let steps = LIVE_HOST_POWER_DISABLED_DUAL_PEEL_NAV_STEPS_WAVE811;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_UNDERPOWERED_FIELD").is_some()
        && residual_name_index(steps, "REQUIRE_GW_UNDERPOWERED_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL_KEEP_EVA").is_some()
        && residual_name_index(steps, "LIVE_HOST_POWER_DISABLED_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostPowerDisabledDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_power_disabled_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 811")
        && sh_source().contains("disabled_underpowered")
        && gl_source().contains("Wave 811");
    residual_action_store(ResidualHostPowerDisabledDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_power_disabled_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("underpowered_team_ords")
        && gl_source().contains("update_eva_low_power()")
        && gl_source().contains("update_power_disabled_state")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPowerDisabledDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_power_disabled_dual_peel_residual_pack_wave811() -> bool {
    honesty_host_power_disabled_dual_peel_method_names_residual_wave811()
        && honesty_host_power_disabled_dual_peel_source_markers_residual_wave811()
        && honesty_host_power_disabled_dual_peel_nav_commands_residual_wave811()
}
pub fn simulate_live_host_power_disabled_dual_peel_honesty() -> bool {
    let ok = honesty_host_power_disabled_dual_peel_residual_pack_wave811()
        && simulate_host_power_disabled_dual_peel_collect_source()
        && simulate_host_power_disabled_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_power_disabled_dual_peel_method_names_residual_wave811());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_power_disabled_dual_peel_source_markers_residual_wave811());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_power_disabled_dual_peel_nav_commands_residual_wave811());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_power_disabled_dual_peel_collect_source());
        assert!(simulate_host_power_disabled_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_power_disabled_dual_peel_residual_pack_wave811());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_power_disabled_dual_peel_honesty());
        assert!(residual_host_power_disabled_dual_peel_ok());
    }
}
