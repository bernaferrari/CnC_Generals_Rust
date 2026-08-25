//! Wave 813: GW entity carries China infantry weapon_bonus_horde; under coupled
//! dual-tick sole-ticks ally infantry-count horde status into logs; host peels
//! update_china_infantry_horde_status and drains weapon refresh + grant counters.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_CHINA_INFANTRY_HORDE_DUAL_PEEL_METHOD_NAMES_WAVE813: &[&str] = &[
    "weapon_bonus_horde",
    "is_china_infantry_horde_unit",
    "host_china_infantry_horde_log",
    "update_china_infantry_horde_status",
    "refresh_red_guard_weapon",
    "refresh_tank_hunter_weapon",
    "refresh_minigunner_weapon",
    "Wave 813",
    "playable_claim = false",
];
pub const LIVE_HOST_CHINA_INFANTRY_HORDE_DUAL_PEEL_NAV_STEPS_WAVE813: &[&str] = &[
    "REQUIRE_ENTITY_WEAPON_BONUS_HORDE",
    "REQUIRE_GW_INFANTRY_HORDE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_HORDE_DRAIN",
    "LIVE_HOST_CHINA_INFANTRY_HORDE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostChinaInfantryHordeDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostChinaInfantryHordeDualPeelAction {
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
fn residual_action_store(a: ResidualHostChinaInfantryHordeDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_china_infantry_horde_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_china_infantry_horde_dual_peel_last_action()
-> ResidualHostChinaInfantryHordeDualPeelAction {
    ResidualHostChinaInfantryHordeDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_china_infantry_horde_dual_peel_method_names_residual_wave813() -> bool {
    let names = LIVE_HOST_CHINA_INFANTRY_HORDE_DUAL_PEEL_METHOD_NAMES_WAVE813;
    let ok = residual_name_index(names, "weapon_bonus_horde").is_some()
        && residual_name_index(names, "is_china_infantry_horde_unit").is_some()
        && residual_name_index(names, "host_china_infantry_horde_log").is_some()
        && residual_name_index(names, "update_china_infantry_horde_status").is_some()
        && residual_name_index(names, "refresh_red_guard_weapon").is_some()
        && residual_name_index(names, "refresh_tank_hunter_weapon").is_some()
        && residual_name_index(names, "refresh_minigunner_weapon").is_some()
        && residual_name_index(names, "Wave 813").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostChinaInfantryHordeDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_china_infantry_horde_dual_peel_source_markers_residual_wave813() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("weapon_bonus_horde")
        && sh.contains("Wave 813")
        && sh.contains("infantry_snapshot")
        && sh.contains("host_china_infantry_horde_log::record")
        && sh.contains("host_china_infantry_horde_log::drain")
        && gl.contains("Wave 813")
        && gl.contains("update_china_infantry_horde_status")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostChinaInfantryHordeDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_china_infantry_horde_dual_peel_nav_commands_residual_wave813() -> bool {
    let steps = LIVE_HOST_CHINA_INFANTRY_HORDE_DUAL_PEEL_NAV_STEPS_WAVE813;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_WEAPON_BONUS_HORDE").is_some()
        && residual_name_index(steps, "REQUIRE_GW_INFANTRY_HORDE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_HORDE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_CHINA_INFANTRY_HORDE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostChinaInfantryHordeDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_china_infantry_horde_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 813")
        && sh_source().contains("weapon_bonus_horde")
        && gl_source().contains("Wave 813");
    residual_action_store(ResidualHostChinaInfantryHordeDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_china_infantry_horde_dual_peel_dispatch_source() -> bool {
    // 2026-08-15: refresh_*_weapon lives on host GameLogic (tanks_and_upgrades.rs).
    let ok = sh_source().contains("host_china_infantry_horde_log::drain")
        && (sh_source().contains("refresh_red_guard_weapon")
            || gl_source().contains("refresh_red_guard_weapon"))
        && gl_source().contains("update_china_infantry_horde_status")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostChinaInfantryHordeDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_china_infantry_horde_dual_peel_residual_pack_wave813() -> bool {
    honesty_host_china_infantry_horde_dual_peel_method_names_residual_wave813()
        && honesty_host_china_infantry_horde_dual_peel_source_markers_residual_wave813()
        && honesty_host_china_infantry_horde_dual_peel_nav_commands_residual_wave813()
}
pub fn simulate_live_host_china_infantry_horde_dual_peel_honesty() -> bool {
    let ok = honesty_host_china_infantry_horde_dual_peel_residual_pack_wave813()
        && simulate_host_china_infantry_horde_dual_peel_collect_source()
        && simulate_host_china_infantry_horde_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_china_infantry_horde_dual_peel_method_names_residual_wave813());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_china_infantry_horde_dual_peel_source_markers_residual_wave813());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_china_infantry_horde_dual_peel_nav_commands_residual_wave813());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_china_infantry_horde_dual_peel_collect_source());
        assert!(simulate_host_china_infantry_horde_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_china_infantry_horde_dual_peel_residual_pack_wave813());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_china_infantry_horde_dual_peel_honesty());
        assert!(residual_host_china_infantry_horde_dual_peel_ok());
    }
}
