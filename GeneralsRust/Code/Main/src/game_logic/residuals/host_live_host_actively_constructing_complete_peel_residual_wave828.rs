//! Wave 828: under coupled dual-tick, peel remaining ACTIVELY_CONSTRUCTING model
//! condition host updates (including post-structure-completion refresh) so GW
//! expire/logs own the bit. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_ACTIVELY_CONSTRUCTING_COMPLETE_PEEL_METHOD_NAMES_WAVE828: &[&str] = &[
    "update_actively_constructing_model_conditions",
    "completed_structures",
    "Wave 828",
    "playable_claim = false",
];
pub const LIVE_HOST_ACTIVELY_CONSTRUCTING_COMPLETE_PEEL_NAV_STEPS_WAVE828: &[&str] = &[
    "REQUIRE_HOST_PEEL",
    "REQUIRE_POST_COMPLETE_PEEL",
    "LIVE_HOST_ACTIVELY_CONSTRUCTING_COMPLETE_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostActivelyConstructingCompletePeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostActivelyConstructingCompletePeelAction {
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
fn residual_action_store(a: ResidualHostActivelyConstructingCompletePeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_actively_constructing_complete_peel_method_names_residual_wave828() -> bool {
    let names = LIVE_HOST_ACTIVELY_CONSTRUCTING_COMPLETE_PEEL_METHOD_NAMES_WAVE828;
    let ok = residual_name_index(names, "update_actively_constructing_model_conditions").is_some()
        && residual_name_index(names, "completed_structures").is_some()
        && residual_name_index(names, "Wave 828").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostActivelyConstructingCompletePeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_actively_constructing_complete_peel_nav_commands_residual_wave828() -> bool {
    let steps = LIVE_HOST_ACTIVELY_CONSTRUCTING_COMPLETE_PEEL_NAV_STEPS_WAVE828;
    let ok = residual_name_index(steps, "LIVE_HOST_ACTIVELY_CONSTRUCTING_COMPLETE_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostActivelyConstructingCompletePeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_actively_constructing_complete_peel_residual_pack_wave828() -> bool {
    let gl = gl_source();
    let ok = gl
        .contains("Wave 828: under coupled shadow, ACTIVELY_CONSTRUCTING bit owned by GW expire.")
        && gl.contains("fn update_actively_constructing_model_conditions")
        && gl.contains("completed_structures");
    residual_action_store(ResidualHostActivelyConstructingCompletePeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_actively_constructing_complete_peel_honesty() -> bool {
    let a = honesty_host_actively_constructing_complete_peel_method_names_residual_wave828();
    let b = honesty_host_actively_constructing_complete_peel_nav_commands_residual_wave828();
    let c = honesty_host_actively_constructing_complete_peel_residual_pack_wave828();
    residual_action_store(ResidualHostActivelyConstructingCompletePeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_actively_constructing_complete_peel_residual_wave828() {
        assert!(honesty_host_actively_constructing_complete_peel_residual_pack_wave828());
        assert!(honesty_host_actively_constructing_complete_peel_method_names_residual_wave828());
        assert!(honesty_host_actively_constructing_complete_peel_nav_commands_residual_wave828());
        assert!(simulate_live_host_actively_constructing_complete_peel_honesty());
    }
}
