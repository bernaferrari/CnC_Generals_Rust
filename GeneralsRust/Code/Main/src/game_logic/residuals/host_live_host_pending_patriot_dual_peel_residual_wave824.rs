//! Wave 824: under coupled dual-tick, host peels update_pending_patriot_assists and
//! sole-ticks assist clips after GW writeback (GW-authoritative poses/HP).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PENDING_PATRIOT_DUAL_PEEL_METHOD_NAMES_WAVE824: &[&str] = &[
    "update_pending_patriot_assists",
    "tick_pending_patriot_assists_sole",
    "Wave 824",
    "playable_claim = false",
];
pub const LIVE_HOST_PENDING_PATRIOT_DUAL_PEEL_NAV_STEPS_WAVE824: &[&str] = &[
    "REQUIRE_HOST_PEEL",
    "REQUIRE_POST_WRITEBACK_SOLE_TICK",
    "LIVE_HOST_PENDING_PATRIOT_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPendingPatriotDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostPendingPatriotDualPeelAction {
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
fn residual_action_store(a: ResidualHostPendingPatriotDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn sh_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_pending_patriot_dual_peel_method_names_residual_wave824() -> bool {
    let names = LIVE_HOST_PENDING_PATRIOT_DUAL_PEEL_METHOD_NAMES_WAVE824;
    let ok = residual_name_index(names, "update_pending_patriot_assists").is_some()
        && residual_name_index(names, "tick_pending_patriot_assists_sole").is_some()
        && residual_name_index(names, "Wave 824").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPendingPatriotDualPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_pending_patriot_dual_peel_nav_commands_residual_wave824() -> bool {
    let steps = LIVE_HOST_PENDING_PATRIOT_DUAL_PEEL_NAV_STEPS_WAVE824;
    let ok = residual_name_index(steps, "LIVE_HOST_PENDING_PATRIOT_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostPendingPatriotDualPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_pending_patriot_dual_peel_residual_pack_wave824() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ok = (sh.contains("Wave 824: pending patriot assist clips sole-tick after GW writeback.")
        || sh.contains("apply_post_writeback_sole_ticks")
        || sh.contains("Wave 823–827/940"))
        && (sh.contains("tick_pending_patriot_assists_sole")
            || sh.contains("apply_post_writeback_sole_ticks"))
        && gl.contains(
            "Wave 824: under coupled shadow, pending patriot assists sole-tick after GW writeback.",
        )
        && gl.contains("fn tick_pending_patriot_assists_sole")
        && (gl.contains("apply_post_writeback_sole_ticks") || gl.contains("Wave 940"));
    residual_action_store(ResidualHostPendingPatriotDualPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_pending_patriot_dual_peel_honesty() -> bool {
    let a = honesty_host_pending_patriot_dual_peel_method_names_residual_wave824();
    let b = honesty_host_pending_patriot_dual_peel_nav_commands_residual_wave824();
    let c = honesty_host_pending_patriot_dual_peel_residual_pack_wave824();
    residual_action_store(ResidualHostPendingPatriotDualPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_pending_patriot_dual_peel_residual_wave824() {
        assert!(honesty_host_pending_patriot_dual_peel_residual_pack_wave824());
        assert!(honesty_host_pending_patriot_dual_peel_method_names_residual_wave824());
        assert!(honesty_host_pending_patriot_dual_peel_nav_commands_residual_wave824());
        assert!(simulate_live_host_pending_patriot_dual_peel_honesty());
    }
}
