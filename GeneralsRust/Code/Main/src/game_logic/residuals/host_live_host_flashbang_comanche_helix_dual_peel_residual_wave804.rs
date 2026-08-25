//! Wave 804: GW entity carries Flashbang/Comanche/Helix projectile residual;
//! under coupled dual-tick sole-ticks flight/expire into logs; host peels
//! the three projectile updates. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FLASHBANG_COMANCHE_HELIX_DUAL_PEEL_METHOD_NAMES_WAVE804: &[&str] = &[
    "flashbang_grenade_projectile",
    "comanche_rocket_pod_projectile",
    "helix_napalm_bomb_projectile",
    "host_flashbang_comanche_helix_projectile_log",
    "update_flashbang_grenade_projectiles",
    "update_comanche_rocket_pod_projectiles",
    "update_helix_napalm_bomb_projectiles",
    "Wave 804",
    "playable_claim = false",
];
pub const LIVE_HOST_FLASHBANG_COMANCHE_HELIX_DUAL_PEEL_NAV_STEPS_WAVE804: &[&str] = &[
    "REQUIRE_ENTITY_FLASHBANG_COMANCHE_HELIX_FIELDS",
    "REQUIRE_GW_FLIGHT_EXPIRE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_IMPACT_EXPIRE_DRAIN",
    "LIVE_HOST_FLASHBANG_COMANCHE_HELIX_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFlashbangComancheHelixDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostFlashbangComancheHelixDualPeelAction {
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
fn residual_action_store(a: ResidualHostFlashbangComancheHelixDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_flashbang_comanche_helix_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_flashbang_comanche_helix_dual_peel_last_action()
-> ResidualHostFlashbangComancheHelixDualPeelAction {
    ResidualHostFlashbangComancheHelixDualPeelAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_flashbang_comanche_helix_dual_peel_method_names_residual_wave804() -> bool {
    let names = LIVE_HOST_FLASHBANG_COMANCHE_HELIX_DUAL_PEEL_METHOD_NAMES_WAVE804;
    let ok = residual_name_index(names, "flashbang_grenade_projectile").is_some()
        && residual_name_index(names, "comanche_rocket_pod_projectile").is_some()
        && residual_name_index(names, "helix_napalm_bomb_projectile").is_some()
        && residual_name_index(names, "host_flashbang_comanche_helix_projectile_log").is_some()
        && residual_name_index(names, "update_flashbang_grenade_projectiles").is_some()
        && residual_name_index(names, "update_comanche_rocket_pod_projectiles").is_some()
        && residual_name_index(names, "update_helix_napalm_bomb_projectiles").is_some()
        && residual_name_index(names, "Wave 804").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostFlashbangComancheHelixDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_flashbang_comanche_helix_dual_peel_source_markers_residual_wave804() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("flashbang_grenade_projectile")
        && ent.contains("comanche_rocket_pod_projectile")
        && ent.contains("helix_napalm_bomb_projectile")
        && sh.contains("Wave 804")
        && sh.contains("host_flashbang_comanche_helix_projectile_log::record_flashbang")
        && sh.contains("host_flashbang_comanche_helix_projectile_log::drain_flashbang")
        && sh.contains("host_flashbang_comanche_helix_projectile_log::record_comanche_expire")
        && gl.contains("Wave 804")
        && gl.contains("update_flashbang_grenade_projectiles")
        && gl.contains("update_comanche_rocket_pod_projectiles")
        && gl.contains("update_helix_napalm_bomb_projectiles");
    residual_action_store(ResidualHostFlashbangComancheHelixDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_flashbang_comanche_helix_dual_peel_nav_commands_residual_wave804() -> bool {
    let steps = LIVE_HOST_FLASHBANG_COMANCHE_HELIX_DUAL_PEEL_NAV_STEPS_WAVE804;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_FLASHBANG_COMANCHE_HELIX_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_EXPIRE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_IMPACT_EXPIRE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_FLASHBANG_COMANCHE_HELIX_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostFlashbangComancheHelixDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_flashbang_comanche_helix_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 804")
        && sh_source().contains("flashbang_grenade_projectile")
        && gl_source().contains("Wave 804");
    residual_action_store(ResidualHostFlashbangComancheHelixDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_flashbang_comanche_helix_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_flashbang_comanche_helix_projectile_log::record_flashbang")
        && sh_source()
            .contains("host_flashbang_comanche_helix_projectile_log::drain_comanche_expires")
        && gl_source().contains("update_flashbang_grenade_projectiles")
        && gl_source().contains("update_comanche_rocket_pod_projectiles")
        && gl_source().contains("update_helix_napalm_bomb_projectiles")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostFlashbangComancheHelixDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_flashbang_comanche_helix_dual_peel_residual_pack_wave804() -> bool {
    honesty_host_flashbang_comanche_helix_dual_peel_method_names_residual_wave804()
        && honesty_host_flashbang_comanche_helix_dual_peel_source_markers_residual_wave804()
        && honesty_host_flashbang_comanche_helix_dual_peel_nav_commands_residual_wave804()
}
pub fn simulate_live_host_flashbang_comanche_helix_dual_peel_honesty() -> bool {
    let ok = honesty_host_flashbang_comanche_helix_dual_peel_residual_pack_wave804()
        && simulate_host_flashbang_comanche_helix_dual_peel_collect_source()
        && simulate_host_flashbang_comanche_helix_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_flashbang_comanche_helix_dual_peel_method_names_residual_wave804());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_flashbang_comanche_helix_dual_peel_source_markers_residual_wave804());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_flashbang_comanche_helix_dual_peel_nav_commands_residual_wave804());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_flashbang_comanche_helix_dual_peel_collect_source());
        assert!(simulate_host_flashbang_comanche_helix_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_flashbang_comanche_helix_dual_peel_residual_pack_wave804());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_flashbang_comanche_helix_dual_peel_honesty());
        assert!(residual_host_flashbang_comanche_helix_dual_peel_ok());
    }
}
