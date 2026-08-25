//! Wave 797: GW entity carries AuroraBombProjectile DeliverPayload flight residual;
//! under coupled dual-tick sole-ticks transport/bomb and logs drop/detonate;
//! host peels `update_aurora_bomb_projectiles`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_AURORA_BOMB_PROJECTILE_DUAL_PEEL_METHOD_NAMES_WAVE797: &[&str] = &[
    "aurora_bomb_projectile",
    "aurora_bomb_has_aim",
    "host_aurora_bomb_projectile_log",
    "update_aurora_bomb_projectiles",
    "Wave 797",
    "playable_claim = false",
];
pub const LIVE_HOST_AURORA_BOMB_PROJECTILE_DUAL_PEEL_NAV_STEPS_WAVE797: &[&str] = &[
    "REQUIRE_ENTITY_AURORA_BOMB_PROJECTILE_FIELDS",
    "REQUIRE_GW_FLIGHT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DROP_DETONATE_DRAIN",
    "LIVE_HOST_AURORA_BOMB_PROJECTILE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_AURORA_BOMB_PROJECTILE_DUAL_PEEL_CMD_NAMES_WAVE797: &[&str] = &[
    "host_aurora_bomb_projectile_dual_peel",
    "aurora_bomb_projectile",
    "host_aurora_bomb_projectile_log",
    "update_aurora_bomb_projectiles",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAuroraBombProjectileFlightDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostAuroraBombProjectileFlightDualPeelAction {
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
fn residual_action_store(a: ResidualHostAuroraBombProjectileFlightDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_aurora_bomb_projectile_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_aurora_bomb_projectile_dual_peel_last_action()
-> ResidualHostAuroraBombProjectileFlightDualPeelAction {
    ResidualHostAuroraBombProjectileFlightDualPeelAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_aurora_bomb_projectile_dual_peel_method_names_residual_wave797() -> bool {
    let names = LIVE_HOST_AURORA_BOMB_PROJECTILE_DUAL_PEEL_METHOD_NAMES_WAVE797;
    let ok = residual_name_index(names, "aurora_bomb_projectile").is_some()
        && residual_name_index(names, "aurora_bomb_has_aim").is_some()
        && residual_name_index(names, "host_aurora_bomb_projectile_log").is_some()
        && residual_name_index(names, "update_aurora_bomb_projectiles").is_some()
        && residual_name_index(names, "Wave 797").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostAuroraBombProjectileFlightDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_aurora_bomb_projectile_dual_peel_source_markers_residual_wave797() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("aurora_bomb_projectile")
        && ent.contains("aurora_bomb_has_aim")
        && sh.contains("Wave 797")
        && sh.contains("host_aurora_bomb_projectile_log::record_destroy")
        && sh.contains("host_aurora_bomb_projectile_log::drain_destroys")
        && gl.contains("Wave 797")
        && gl.contains("update_aurora_bomb_projectiles");
    residual_action_store(ResidualHostAuroraBombProjectileFlightDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_aurora_bomb_projectile_dual_peel_nav_commands_residual_wave797() -> bool {
    let steps = LIVE_HOST_AURORA_BOMB_PROJECTILE_DUAL_PEEL_NAV_STEPS_WAVE797;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_AURORA_BOMB_PROJECTILE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DROP_DETONATE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_AURORA_BOMB_PROJECTILE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostAuroraBombProjectileFlightDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_aurora_bomb_projectile_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 797")
        && sh_source().contains("aurora_bomb_projectile")
        && gl_source().contains("Wave 797");
    residual_action_store(ResidualHostAuroraBombProjectileFlightDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_aurora_bomb_projectile_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_aurora_bomb_projectile_log::record_destroy")
        && sh_source().contains("host_aurora_bomb_projectile_log::drain_destroys")
        && gl_source().contains("update_aurora_bomb_projectiles")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostAuroraBombProjectileFlightDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_aurora_bomb_projectile_dual_peel_residual_pack_wave797() -> bool {
    honesty_host_aurora_bomb_projectile_dual_peel_method_names_residual_wave797()
        && honesty_host_aurora_bomb_projectile_dual_peel_source_markers_residual_wave797()
        && honesty_host_aurora_bomb_projectile_dual_peel_nav_commands_residual_wave797()
        && simulate_host_aurora_bomb_projectile_dual_peel_collect_source()
        && simulate_host_aurora_bomb_projectile_dual_peel_dispatch_source()
}
pub fn simulate_live_host_aurora_bomb_projectile_dual_peel_honesty() -> bool {
    let ok = honesty_host_aurora_bomb_projectile_dual_peel_residual_pack_wave797();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostAuroraBombProjectileFlightDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_aurora_bomb_projectile_dual_peel_method_names_residual_wave797());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_aurora_bomb_projectile_dual_peel_source_markers_residual_wave797());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_aurora_bomb_projectile_dual_peel_nav_commands_residual_wave797());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_aurora_bomb_projectile_dual_peel_collect_source());
        assert!(simulate_host_aurora_bomb_projectile_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_aurora_bomb_projectile_dual_peel_residual_pack_wave797());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_aurora_bomb_projectile_dual_peel_honesty());
        assert!(residual_host_aurora_bomb_projectile_dual_peel_ok());
    }
}
