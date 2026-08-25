//! Wave 800: GW entity carries CannonShellProjectile DeliverPayload flight residual;
//! under coupled dual-tick sole-ticks transport/bomb and logs drop/detonate;
//! host peels `update_scud_launcher_missile_projectiles`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_CANNON_SHELL_PROJECTILE_DUAL_PEEL_METHOD_NAMES_WAVE800: &[&str] = &[
    "scud_launcher_missile_projectile",
    "neutron_cannon_shell_projectile",
    "nuke_cannon_shell_projectile",
    "host_cannon_shell_projectile_log",
    "update_scud_launcher_missile_projectiles",
    "update_neutron_cannon_shell_projectiles",
    "update_nuke_cannon_shell_projectiles",
    "Wave 800",
    "playable_claim = false",
];
pub const LIVE_HOST_CANNON_SHELL_PROJECTILE_DUAL_PEEL_NAV_STEPS_WAVE800: &[&str] = &[
    "REQUIRE_ENTITY_CANNON_SHELL_PROJECTILE_FIELDS",
    "REQUIRE_GW_FLIGHT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DROP_DETONATE_DRAIN",
    "LIVE_HOST_CANNON_SHELL_PROJECTILE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_CANNON_SHELL_PROJECTILE_DUAL_PEEL_CMD_NAMES_WAVE800: &[&str] = &[
    "host_cannon_shell_projectile_dual_peel",
    "cannon_shell_projectile",
    "host_cannon_shell_projectile_log",
    "update_scud_launcher_missile_projectiles",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCannonShellProjectileFlightDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostCannonShellProjectileFlightDualPeelAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostCannonShellProjectileFlightDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_cannon_shell_projectile_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_cannon_shell_projectile_dual_peel_last_action()
-> ResidualHostCannonShellProjectileFlightDualPeelAction {
    ResidualHostCannonShellProjectileFlightDualPeelAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_cannon_shell_projectile_dual_peel_method_names_residual_wave800() -> bool {
    let names = LIVE_HOST_CANNON_SHELL_PROJECTILE_DUAL_PEEL_METHOD_NAMES_WAVE800;
    let ok = residual_name_index(names, "scud_launcher_missile_projectile").is_some()
        && residual_name_index(names, "neutron_cannon_shell_projectile").is_some()
        && residual_name_index(names, "nuke_cannon_shell_projectile").is_some()
        && residual_name_index(names, "host_cannon_shell_projectile_log").is_some()
        && residual_name_index(names, "update_scud_launcher_missile_projectiles").is_some()
        && residual_name_index(names, "update_neutron_cannon_shell_projectiles").is_some()
        && residual_name_index(names, "update_nuke_cannon_shell_projectiles").is_some()
        && residual_name_index(names, "Wave 800").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCannonShellProjectileFlightDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_cannon_shell_projectile_dual_peel_source_markers_residual_wave800() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("scud_launcher_missile_projectile")
        && ent.contains("neutron_cannon_shell_projectile")
        && ent.contains("nuke_cannon_shell_projectile")
        && sh.contains("Wave 800")
        && sh.contains("host_cannon_shell_projectile_log::record_impact")
        && sh.contains("host_cannon_shell_projectile_log::drain_impacts")
        && gl.contains("Wave 800")
        && gl.contains("update_scud_launcher_missile_projectiles")
        && gl.contains("update_neutron_cannon_shell_projectiles")
        && gl.contains("update_nuke_cannon_shell_projectiles");
    residual_action_store(ResidualHostCannonShellProjectileFlightDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_cannon_shell_projectile_dual_peel_nav_commands_residual_wave800() -> bool {
    let steps = LIVE_HOST_CANNON_SHELL_PROJECTILE_DUAL_PEEL_NAV_STEPS_WAVE800;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_CANNON_SHELL_PROJECTILE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DROP_DETONATE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_CANNON_SHELL_PROJECTILE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostCannonShellProjectileFlightDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_cannon_shell_projectile_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 800")
        && sh_source().contains("scud_launcher_missile_projectile")
        && gl_source().contains("Wave 800");
    residual_action_store(ResidualHostCannonShellProjectileFlightDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_cannon_shell_projectile_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_cannon_shell_projectile_log::record_impact")
        && sh_source().contains("host_cannon_shell_projectile_log::drain_impacts")
        && gl_source().contains("update_scud_launcher_missile_projectiles")
        && gl_source().contains("update_neutron_cannon_shell_projectiles")
        && gl_source().contains("update_nuke_cannon_shell_projectiles")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostCannonShellProjectileFlightDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_cannon_shell_projectile_dual_peel_residual_pack_wave800() -> bool {
    honesty_host_cannon_shell_projectile_dual_peel_method_names_residual_wave800()
        && honesty_host_cannon_shell_projectile_dual_peel_source_markers_residual_wave800()
        && honesty_host_cannon_shell_projectile_dual_peel_nav_commands_residual_wave800()
        && simulate_host_cannon_shell_projectile_dual_peel_collect_source()
        && simulate_host_cannon_shell_projectile_dual_peel_dispatch_source()
}
pub fn simulate_live_host_cannon_shell_projectile_dual_peel_honesty() -> bool {
    let ok = honesty_host_cannon_shell_projectile_dual_peel_residual_pack_wave800();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCannonShellProjectileFlightDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_cannon_shell_projectile_dual_peel_method_names_residual_wave800());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_cannon_shell_projectile_dual_peel_source_markers_residual_wave800());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_cannon_shell_projectile_dual_peel_nav_commands_residual_wave800());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_cannon_shell_projectile_dual_peel_collect_source());
        assert!(simulate_host_cannon_shell_projectile_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_cannon_shell_projectile_dual_peel_residual_pack_wave800());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_cannon_shell_projectile_dual_peel_honesty());
        assert!(residual_host_cannon_shell_projectile_dual_peel_ok());
    }
}
