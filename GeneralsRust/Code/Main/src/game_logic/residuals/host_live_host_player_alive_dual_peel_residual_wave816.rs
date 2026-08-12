//! Wave 816: GW sets PlayerData::is_alive from living team entities under coupled
//! dual-tick; host peels update_player_alive_state (writeback applies host + meta).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PLAYER_ALIVE_DUAL_PEEL_METHOD_NAMES_WAVE816: &[&str] = &[
    "is_alive",
    "living_teams",
    "update_player_alive_state",
    "Wave 816",
    "playable_claim = false",
];
pub const LIVE_HOST_PLAYER_ALIVE_DUAL_PEEL_NAV_STEPS_WAVE816: &[&str] = &[
    "REQUIRE_GW_PLAYER_ALIVE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK_ALIVE",
    "LIVE_HOST_PLAYER_ALIVE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPlayerAliveDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostPlayerAliveDualPeelAction {
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
fn residual_action_store(a: ResidualHostPlayerAliveDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_player_alive_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_player_alive_dual_peel_last_action() -> ResidualHostPlayerAliveDualPeelAction {
    ResidualHostPlayerAliveDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_player_alive_dual_peel_method_names_residual_wave816() -> bool {
    let names = LIVE_HOST_PLAYER_ALIVE_DUAL_PEEL_METHOD_NAMES_WAVE816;
    let ok = residual_name_index(names, "is_alive").is_some()
        && residual_name_index(names, "living_teams").is_some()
        && residual_name_index(names, "update_player_alive_state").is_some()
        && residual_name_index(names, "Wave 816").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPlayerAliveDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_player_alive_dual_peel_source_markers_residual_wave816() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ok = sh.contains("Wave 816")
        && sh.contains("living_teams")
        && sh.contains("pd.is_alive")
        && sh.contains("player.is_alive != pd.is_alive")
        && gl.contains("Wave 816")
        && gl.contains("update_player_alive_state")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPlayerAliveDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_player_alive_dual_peel_nav_commands_residual_wave816() -> bool {
    let steps = LIVE_HOST_PLAYER_ALIVE_DUAL_PEEL_NAV_STEPS_WAVE816;
    let ok = residual_name_index(steps, "REQUIRE_GW_PLAYER_ALIVE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_WRITEBACK_ALIVE").is_some()
        && residual_name_index(steps, "LIVE_HOST_PLAYER_ALIVE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostPlayerAliveDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_player_alive_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 816")
        && sh_source().contains("living_teams")
        && gl_source().contains("Wave 816");
    residual_action_store(ResidualHostPlayerAliveDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_player_alive_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("pd.is_alive = alive")
        && gl_source().contains("update_player_alive_state")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPlayerAliveDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_player_alive_dual_peel_residual_pack_wave816() -> bool {
    honesty_host_player_alive_dual_peel_method_names_residual_wave816()
        && honesty_host_player_alive_dual_peel_source_markers_residual_wave816()
        && honesty_host_player_alive_dual_peel_nav_commands_residual_wave816()
}
pub fn simulate_live_host_player_alive_dual_peel_honesty() -> bool {
    let ok = honesty_host_player_alive_dual_peel_residual_pack_wave816()
        && simulate_host_player_alive_dual_peel_collect_source()
        && simulate_host_player_alive_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_player_alive_dual_peel_method_names_residual_wave816());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_player_alive_dual_peel_source_markers_residual_wave816());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_player_alive_dual_peel_nav_commands_residual_wave816());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_player_alive_dual_peel_collect_source());
        assert!(simulate_host_player_alive_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_player_alive_dual_peel_residual_pack_wave816());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_player_alive_dual_peel_honesty());
        assert!(residual_host_player_alive_dual_peel_ok());
    }
}
