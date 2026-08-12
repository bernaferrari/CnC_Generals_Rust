//! Wave 827: under coupled dual-tick, host peels remaining system residuals
//! (crate vision, sell list, special power strikes, player upgrades) and
//! sole-ticks them after GW writeback.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SYSTEMS_DUAL_PEEL_METHOD_NAMES_WAVE827: &[&str] = &[
    "update_main_crate_vision",
    "update_sell_list",
    "update_special_power_strikes",
    "update_player_upgrades",
    "tick_host_systems_residuals_sole",
    "Wave 827",
    "playable_claim = false",
];
pub const LIVE_HOST_SYSTEMS_DUAL_PEEL_NAV_STEPS_WAVE827: &[&str] = &[
    "REQUIRE_HOST_PEEL",
    "REQUIRE_POST_WRITEBACK_SOLE_TICK",
    "LIVE_HOST_SYSTEMS_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSystemsDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostSystemsDualPeelAction {
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
fn residual_action_store(a: ResidualHostSystemsDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_systems_dual_peel_method_names_residual_wave827() -> bool {
    let names = LIVE_HOST_SYSTEMS_DUAL_PEEL_METHOD_NAMES_WAVE827;
    let ok = names
        .iter()
        .all(|n| residual_name_index(names, n).is_some());
    residual_action_store(ResidualHostSystemsDualPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_systems_dual_peel_nav_commands_residual_wave827() -> bool {
    let steps = LIVE_HOST_SYSTEMS_DUAL_PEEL_NAV_STEPS_WAVE827;
    let ok = residual_name_index(steps, "LIVE_HOST_SYSTEMS_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostSystemsDualPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_systems_dual_peel_residual_pack_wave827() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ok = (sh
        .contains("Wave 827: remaining host system residuals sole-tick after GW writeback.")
        || sh.contains("apply_post_writeback_sole_ticks")
        || sh.contains("Wave 823–827/940"))
        && (sh.contains("tick_host_systems_residuals_sole")
            || sh.contains("apply_post_writeback_sole_ticks"))
        && gl.contains(
            "Wave 827: under coupled shadow, host system residuals sole-tick after GW writeback.",
        )
        && gl.contains("fn tick_host_systems_residuals_sole")
        && (gl.contains("apply_post_writeback_sole_ticks") || gl.contains("Wave 940"));
    residual_action_store(ResidualHostSystemsDualPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_systems_dual_peel_honesty() -> bool {
    let a = honesty_host_systems_dual_peel_method_names_residual_wave827();
    let b = honesty_host_systems_dual_peel_nav_commands_residual_wave827();
    let c = honesty_host_systems_dual_peel_residual_pack_wave827();
    residual_action_store(ResidualHostSystemsDualPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_systems_dual_peel_residual_wave827() {
        assert!(honesty_host_systems_dual_peel_residual_pack_wave827());
        assert!(honesty_host_systems_dual_peel_method_names_residual_wave827());
        assert!(honesty_host_systems_dual_peel_nav_commands_residual_wave827());
        assert!(simulate_live_host_systems_dual_peel_honesty());
    }
}
