//! Wave 803: GW entity carries Inferno shell + SpySatellite ping residual;
//! under coupled dual-tick sole-ticks flight/expire into logs; host peels
//! `update_inferno_shell_projectiles` / `update_spy_satellite_pings`.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_INFERNO_SHELL_SPY_PING_DUAL_PEEL_METHOD_NAMES_WAVE803: &[&str] = &[
    "inferno_shell_projectile",
    "spy_satellite_ping",
    "host_inferno_shell_projectile_log",
    "host_spy_satellite_ping_log",
    "update_inferno_shell_projectiles",
    "update_spy_satellite_pings",
    "Wave 803",
    "playable_claim = false",
];
pub const LIVE_HOST_INFERNO_SHELL_SPY_PING_DUAL_PEEL_NAV_STEPS_WAVE803: &[&str] = &[
    "REQUIRE_ENTITY_INFERNO_SPY_FIELDS",
    "REQUIRE_GW_FLIGHT_EXPIRE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_IMPACT_EXPIRE_DRAIN",
    "LIVE_HOST_INFERNO_SHELL_SPY_PING_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostInfernoShellSpyPingDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostInfernoShellSpyPingDualPeelAction {
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
fn residual_action_store(a: ResidualHostInfernoShellSpyPingDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_inferno_shell_spy_ping_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_inferno_shell_spy_ping_dual_peel_last_action()
-> ResidualHostInfernoShellSpyPingDualPeelAction {
    ResidualHostInfernoShellSpyPingDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_inferno_shell_spy_ping_dual_peel_method_names_residual_wave803() -> bool {
    let names = LIVE_HOST_INFERNO_SHELL_SPY_PING_DUAL_PEEL_METHOD_NAMES_WAVE803;
    let ok = residual_name_index(names, "inferno_shell_projectile").is_some()
        && residual_name_index(names, "spy_satellite_ping").is_some()
        && residual_name_index(names, "host_inferno_shell_projectile_log").is_some()
        && residual_name_index(names, "host_spy_satellite_ping_log").is_some()
        && residual_name_index(names, "update_inferno_shell_projectiles").is_some()
        && residual_name_index(names, "update_spy_satellite_pings").is_some()
        && residual_name_index(names, "Wave 803").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostInfernoShellSpyPingDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_inferno_shell_spy_ping_dual_peel_source_markers_residual_wave803() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("inferno_shell_projectile")
        && ent.contains("spy_satellite_ping")
        && sh.contains("Wave 803")
        && sh.contains("host_inferno_shell_projectile_log::record_impact")
        && sh.contains("host_inferno_shell_projectile_log::drain_impacts")
        && sh.contains("host_spy_satellite_ping_log::record_expire")
        && sh.contains("host_spy_satellite_ping_log::drain_expires")
        && gl.contains("Wave 803")
        && gl.contains("update_inferno_shell_projectiles")
        && gl.contains("update_spy_satellite_pings");
    residual_action_store(ResidualHostInfernoShellSpyPingDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_inferno_shell_spy_ping_dual_peel_nav_commands_residual_wave803() -> bool {
    let steps = LIVE_HOST_INFERNO_SHELL_SPY_PING_DUAL_PEEL_NAV_STEPS_WAVE803;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_INFERNO_SPY_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_EXPIRE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_IMPACT_EXPIRE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_INFERNO_SHELL_SPY_PING_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostInfernoShellSpyPingDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_inferno_shell_spy_ping_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 803")
        && sh_source().contains("inferno_shell_projectile")
        && gl_source().contains("Wave 803");
    residual_action_store(ResidualHostInfernoShellSpyPingDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_inferno_shell_spy_ping_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_inferno_shell_projectile_log::record_impact")
        && sh_source().contains("host_spy_satellite_ping_log::drain_expires")
        && gl_source().contains("update_inferno_shell_projectiles")
        && gl_source().contains("update_spy_satellite_pings")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostInfernoShellSpyPingDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_inferno_shell_spy_ping_dual_peel_residual_pack_wave803() -> bool {
    honesty_host_inferno_shell_spy_ping_dual_peel_method_names_residual_wave803()
        && honesty_host_inferno_shell_spy_ping_dual_peel_source_markers_residual_wave803()
        && honesty_host_inferno_shell_spy_ping_dual_peel_nav_commands_residual_wave803()
}
pub fn simulate_live_host_inferno_shell_spy_ping_dual_peel_honesty() -> bool {
    let ok = honesty_host_inferno_shell_spy_ping_dual_peel_residual_pack_wave803()
        && simulate_host_inferno_shell_spy_ping_dual_peel_collect_source()
        && simulate_host_inferno_shell_spy_ping_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_inferno_shell_spy_ping_dual_peel_method_names_residual_wave803());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_inferno_shell_spy_ping_dual_peel_source_markers_residual_wave803());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_inferno_shell_spy_ping_dual_peel_nav_commands_residual_wave803());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_inferno_shell_spy_ping_dual_peel_collect_source());
        assert!(simulate_host_inferno_shell_spy_ping_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_inferno_shell_spy_ping_dual_peel_residual_pack_wave803());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_inferno_shell_spy_ping_dual_peel_honesty());
        assert!(residual_host_inferno_shell_spy_ping_dual_peel_ok());
    }
}
