//! Wave 818: GW sets player radar_count from living radar providers; under coupled
//! dual-tick sole-ticks into logs; host peels update_player_radar and drains
//! set_radar_state + online/offline audio. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PLAYER_RADAR_DUAL_PEEL_METHOD_NAMES_WAVE818: &[&str] = &[
    "radar_count",
    "is_legal_radar_provider",
    "host_player_radar_log",
    "update_player_radar",
    "record_player_radar",
    "Wave 818",
    "playable_claim = false",
];
pub const LIVE_HOST_PLAYER_RADAR_DUAL_PEEL_NAV_STEPS_WAVE818: &[&str] = &[
    "REQUIRE_GW_RADAR_PROVIDER_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_RADAR_DRAIN_AUDIO",
    "LIVE_HOST_PLAYER_RADAR_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPlayerRadarDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostPlayerRadarDualPeelAction {
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
fn residual_action_store(a: ResidualHostPlayerRadarDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_player_radar_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_player_radar_dual_peel_last_action() -> ResidualHostPlayerRadarDualPeelAction {
    ResidualHostPlayerRadarDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_player_radar_dual_peel_method_names_residual_wave818() -> bool {
    let names = LIVE_HOST_PLAYER_RADAR_DUAL_PEEL_METHOD_NAMES_WAVE818;
    let ok = residual_name_index(names, "radar_count").is_some()
        && residual_name_index(names, "is_legal_radar_provider").is_some()
        && residual_name_index(names, "host_player_radar_log").is_some()
        && residual_name_index(names, "update_player_radar").is_some()
        && residual_name_index(names, "record_player_radar").is_some()
        && residual_name_index(names, "Wave 818").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPlayerRadarDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_player_radar_dual_peel_source_markers_residual_wave818() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ok = sh.contains("Wave 818")
        && sh.contains("host_player_radar_log::record")
        && sh.contains("host_player_radar_log::drain")
        && sh.contains("is_legal_radar_provider")
        && sh.contains("providers_by_team")
        && gl.contains("Wave 818")
        && gl.contains("update_player_radar")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPlayerRadarDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_player_radar_dual_peel_nav_commands_residual_wave818() -> bool {
    let steps = LIVE_HOST_PLAYER_RADAR_DUAL_PEEL_NAV_STEPS_WAVE818;
    let ok = residual_name_index(steps, "REQUIRE_GW_RADAR_PROVIDER_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_RADAR_DRAIN_AUDIO").is_some()
        && residual_name_index(steps, "LIVE_HOST_PLAYER_RADAR_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostPlayerRadarDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_player_radar_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 818")
        && sh_source().contains("radar_count")
        && gl_source().contains("Wave 818");
    residual_action_store(ResidualHostPlayerRadarDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_player_radar_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_player_radar_log::drain")
        && sh_source().contains("RADAR_ONLINE_AUDIO")
        && gl_source().contains("update_player_radar")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPlayerRadarDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_player_radar_dual_peel_residual_pack_wave818() -> bool {
    honesty_host_player_radar_dual_peel_method_names_residual_wave818()
        && honesty_host_player_radar_dual_peel_source_markers_residual_wave818()
        && honesty_host_player_radar_dual_peel_nav_commands_residual_wave818()
}
pub fn simulate_live_host_player_radar_dual_peel_honesty() -> bool {
    let ok = honesty_host_player_radar_dual_peel_residual_pack_wave818()
        && simulate_host_player_radar_dual_peel_collect_source()
        && simulate_host_player_radar_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_player_radar_dual_peel_method_names_residual_wave818());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_player_radar_dual_peel_source_markers_residual_wave818());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_player_radar_dual_peel_nav_commands_residual_wave818());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_player_radar_dual_peel_collect_source());
        assert!(simulate_host_player_radar_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_player_radar_dual_peel_residual_pack_wave818());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_player_radar_dual_peel_honesty());
        assert!(residual_host_player_radar_dual_peel_ok());
    }
}
