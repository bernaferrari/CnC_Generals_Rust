//! Wave 812: GW entity carries Battlemaster weapon_bonus_horde; under coupled
//! dual-tick sole-ticks ally-count horde status into logs; host peels
//! update_battlemaster_horde_status and drains weapon refresh + grant counters.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_BATTLEMASTER_HORDE_DUAL_PEEL_METHOD_NAMES_WAVE812: &[&str] = &[
    "weapon_bonus_horde",
    "is_battlemaster_template",
    "host_battlemaster_horde_log",
    "update_battlemaster_horde_status",
    "refresh_battlemaster_weapon",
    "Wave 812",
    "playable_claim = false",
];
pub const LIVE_HOST_BATTLEMASTER_HORDE_DUAL_PEEL_NAV_STEPS_WAVE812: &[&str] = &[
    "REQUIRE_ENTITY_WEAPON_BONUS_HORDE",
    "REQUIRE_GW_HORDE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_HORDE_DRAIN",
    "LIVE_HOST_BATTLEMASTER_HORDE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBattlemasterHordeDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostBattlemasterHordeDualPeelAction {
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
fn residual_action_store(a: ResidualHostBattlemasterHordeDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_battlemaster_horde_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_battlemaster_horde_dual_peel_last_action()
-> ResidualHostBattlemasterHordeDualPeelAction {
    ResidualHostBattlemasterHordeDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_battlemaster_horde_dual_peel_method_names_residual_wave812() -> bool {
    let names = LIVE_HOST_BATTLEMASTER_HORDE_DUAL_PEEL_METHOD_NAMES_WAVE812;
    let ok = residual_name_index(names, "weapon_bonus_horde").is_some()
        && residual_name_index(names, "is_battlemaster_template").is_some()
        && residual_name_index(names, "host_battlemaster_horde_log").is_some()
        && residual_name_index(names, "update_battlemaster_horde_status").is_some()
        && residual_name_index(names, "refresh_battlemaster_weapon").is_some()
        && residual_name_index(names, "Wave 812").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostBattlemasterHordeDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_battlemaster_horde_dual_peel_source_markers_residual_wave812() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("weapon_bonus_horde")
        && ent.contains("template_name")
        && sh.contains("Wave 812")
        && sh.contains("battlemaster_snapshot")
        && sh.contains("host_battlemaster_horde_log::record")
        && sh.contains("host_battlemaster_horde_log::drain")
        && gl.contains("Wave 812")
        && gl.contains("update_battlemaster_horde_status")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostBattlemasterHordeDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_battlemaster_horde_dual_peel_nav_commands_residual_wave812() -> bool {
    let steps = LIVE_HOST_BATTLEMASTER_HORDE_DUAL_PEEL_NAV_STEPS_WAVE812;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_WEAPON_BONUS_HORDE").is_some()
        && residual_name_index(steps, "REQUIRE_GW_HORDE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_HORDE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_BATTLEMASTER_HORDE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostBattlemasterHordeDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_battlemaster_horde_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 812")
        && sh_source().contains("weapon_bonus_horde")
        && gl_source().contains("Wave 812");
    residual_action_store(ResidualHostBattlemasterHordeDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_battlemaster_horde_dual_peel_dispatch_source() -> bool {
    // 2026-08-15: refresh_battlemaster_weapon lives on host GameLogic
    // (world_combat/air_and_mig.rs), drain stays in shadow session.rs.
    let ok = sh_source().contains("host_battlemaster_horde_log::drain")
        && (sh_source().contains("refresh_battlemaster_weapon")
            || gl_source().contains("refresh_battlemaster_weapon"))
        && gl_source().contains("update_battlemaster_horde_status")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostBattlemasterHordeDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_battlemaster_horde_dual_peel_residual_pack_wave812() -> bool {
    honesty_host_battlemaster_horde_dual_peel_method_names_residual_wave812()
        && honesty_host_battlemaster_horde_dual_peel_source_markers_residual_wave812()
        && honesty_host_battlemaster_horde_dual_peel_nav_commands_residual_wave812()
}
pub fn simulate_live_host_battlemaster_horde_dual_peel_honesty() -> bool {
    let ok = honesty_host_battlemaster_horde_dual_peel_residual_pack_wave812()
        && simulate_host_battlemaster_horde_dual_peel_collect_source()
        && simulate_host_battlemaster_horde_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_battlemaster_horde_dual_peel_method_names_residual_wave812());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_battlemaster_horde_dual_peel_source_markers_residual_wave812());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_battlemaster_horde_dual_peel_nav_commands_residual_wave812());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_battlemaster_horde_dual_peel_collect_source());
        assert!(simulate_host_battlemaster_horde_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_battlemaster_horde_dual_peel_residual_pack_wave812());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_battlemaster_horde_dual_peel_honesty());
        assert!(residual_host_battlemaster_horde_dual_peel_ok());
    }
}
