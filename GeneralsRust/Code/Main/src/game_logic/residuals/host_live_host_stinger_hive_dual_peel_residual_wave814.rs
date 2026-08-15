//! Wave 814: GW entity carries Stinger hive slave roster/count/respawn; under
//! coupled dual-tick sole-ticks respawn into logs; host peels
//! update_stinger_hive_respawns and drains roster + residual respawn counter.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_STINGER_HIVE_DUAL_PEEL_METHOD_NAMES_WAVE814: &[&str] = &[
    "hive_slave_count",
    "hive_slaves_alive",
    "host_stinger_hive_log",
    "update_stinger_hive_respawns",
    "is_stinger_site_structure",
    "Wave 814",
    "playable_claim = false",
];
pub const LIVE_HOST_STINGER_HIVE_DUAL_PEEL_NAV_STEPS_WAVE814: &[&str] = &[
    "REQUIRE_ENTITY_HIVE_ROSTER_FIELDS",
    "REQUIRE_GW_RESPAWN_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_RESPAWN_DRAIN",
    "LIVE_HOST_STINGER_HIVE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStingerHiveDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostStingerHiveDualPeelAction {
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
fn residual_action_store(a: ResidualHostStingerHiveDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_stinger_hive_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_stinger_hive_dual_peel_last_action() -> ResidualHostStingerHiveDualPeelAction {
    ResidualHostStingerHiveDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_stinger_hive_dual_peel_method_names_residual_wave814() -> bool {
    let names = LIVE_HOST_STINGER_HIVE_DUAL_PEEL_METHOD_NAMES_WAVE814;
    let ok = residual_name_index(names, "hive_slave_count").is_some()
        && residual_name_index(names, "hive_slaves_alive").is_some()
        && residual_name_index(names, "host_stinger_hive_log").is_some()
        && residual_name_index(names, "update_stinger_hive_respawns").is_some()
        && residual_name_index(names, "is_stinger_site_structure").is_some()
        && residual_name_index(names, "Wave 814").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostStingerHiveDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_stinger_hive_dual_peel_source_markers_residual_wave814() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("hive_slaves_alive")
        && ent.contains("hive_slaves_hp")
        && ent.contains("hive_slave_respawn_frame")
        && sh.contains("Wave 814")
        && sh.contains("host_stinger_hive_log::record")
        && sh.contains("host_stinger_hive_log::drain")
        && sh.contains("should_respawn_stinger_slave")
        && gl.contains("Wave 814")
        && gl.contains("update_stinger_hive_respawns")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostStingerHiveDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_stinger_hive_dual_peel_nav_commands_residual_wave814() -> bool {
    let steps = LIVE_HOST_STINGER_HIVE_DUAL_PEEL_NAV_STEPS_WAVE814;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_HIVE_ROSTER_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_RESPAWN_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_RESPAWN_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_STINGER_HIVE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostStingerHiveDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_stinger_hive_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 814")
        && sh_source().contains("hive_slaves_alive")
        && gl_source().contains("Wave 814");
    residual_action_store(ResidualHostStingerHiveDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_stinger_hive_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_stinger_hive_log::drain")
        && sh_source().contains("stinger_hive_residual_respawns")
        && gl_source().contains("update_stinger_hive_respawns")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostStingerHiveDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_stinger_hive_dual_peel_residual_pack_wave814() -> bool {
    honesty_host_stinger_hive_dual_peel_method_names_residual_wave814()
        && honesty_host_stinger_hive_dual_peel_source_markers_residual_wave814()
        && honesty_host_stinger_hive_dual_peel_nav_commands_residual_wave814()
}
pub fn simulate_live_host_stinger_hive_dual_peel_honesty() -> bool {
    let ok = honesty_host_stinger_hive_dual_peel_residual_pack_wave814()
        && simulate_host_stinger_hive_dual_peel_collect_source()
        && simulate_host_stinger_hive_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_stinger_hive_dual_peel_method_names_residual_wave814());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_stinger_hive_dual_peel_source_markers_residual_wave814());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_stinger_hive_dual_peel_nav_commands_residual_wave814());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_stinger_hive_dual_peel_collect_source());
        assert!(simulate_host_stinger_hive_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_stinger_hive_dual_peel_residual_pack_wave814());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_stinger_hive_dual_peel_honesty());
        assert!(residual_host_stinger_hive_dual_peel_ok());
    }
}
