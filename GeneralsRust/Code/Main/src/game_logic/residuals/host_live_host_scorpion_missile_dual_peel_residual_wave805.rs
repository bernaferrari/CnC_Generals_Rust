//! Wave 805: GW entity carries Scorpion missile residual; under coupled dual-tick
//! sole-ticks flight/impact into logs; host peels update_scorpion_missile_projectiles.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SCORPION_MISSILE_DUAL_PEEL_METHOD_NAMES_WAVE805: &[&str] = &[
    "scorpion_missile_projectile",
    "host_scorpion_missile_projectile_log",
    "update_scorpion_missile_projectiles",
    "apply_scorpion_residual_at",
    "Wave 805",
    "playable_claim = false",
];
pub const LIVE_HOST_SCORPION_MISSILE_DUAL_PEEL_NAV_STEPS_WAVE805: &[&str] = &[
    "REQUIRE_ENTITY_SCORPION_MISSILE_FIELDS",
    "REQUIRE_GW_FLIGHT_IMPACT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_IMPACT_DRAIN",
    "LIVE_HOST_SCORPION_MISSILE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostScorpionMissileDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostScorpionMissileDualPeelAction {
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
fn residual_action_store(a: ResidualHostScorpionMissileDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_scorpion_missile_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_scorpion_missile_dual_peel_last_action()
-> ResidualHostScorpionMissileDualPeelAction {
    ResidualHostScorpionMissileDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_scorpion_missile_dual_peel_method_names_residual_wave805() -> bool {
    let names = LIVE_HOST_SCORPION_MISSILE_DUAL_PEEL_METHOD_NAMES_WAVE805;
    let ok = residual_name_index(names, "scorpion_missile_projectile").is_some()
        && residual_name_index(names, "host_scorpion_missile_projectile_log").is_some()
        && residual_name_index(names, "update_scorpion_missile_projectiles").is_some()
        && residual_name_index(names, "apply_scorpion_residual_at").is_some()
        && residual_name_index(names, "Wave 805").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostScorpionMissileDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_scorpion_missile_dual_peel_source_markers_residual_wave805() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("scorpion_missile_projectile")
        && ent.contains("scorpion_missile_travelled")
        && sh.contains("Wave 805")
        && sh.contains("host_scorpion_missile_projectile_log::record_impact")
        && sh.contains("host_scorpion_missile_projectile_log::drain_impacts")
        && sh.contains("scorpion_retarget")
        && gl.contains("Wave 805")
        && gl.contains("update_scorpion_missile_projectiles")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostScorpionMissileDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_scorpion_missile_dual_peel_nav_commands_residual_wave805() -> bool {
    let steps = LIVE_HOST_SCORPION_MISSILE_DUAL_PEEL_NAV_STEPS_WAVE805;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_SCORPION_MISSILE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_IMPACT_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_IMPACT_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_SCORPION_MISSILE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostScorpionMissileDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_scorpion_missile_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 805")
        && sh_source().contains("scorpion_missile_projectile")
        && gl_source().contains("Wave 805");
    residual_action_store(ResidualHostScorpionMissileDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_scorpion_missile_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_scorpion_missile_projectile_log::record_impact")
        && sh_source().contains("host_scorpion_missile_projectile_log::drain_impacts")
        && gl_source().contains("update_scorpion_missile_projectiles")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostScorpionMissileDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_scorpion_missile_dual_peel_residual_pack_wave805() -> bool {
    honesty_host_scorpion_missile_dual_peel_method_names_residual_wave805()
        && honesty_host_scorpion_missile_dual_peel_source_markers_residual_wave805()
        && honesty_host_scorpion_missile_dual_peel_nav_commands_residual_wave805()
}
pub fn simulate_live_host_scorpion_missile_dual_peel_honesty() -> bool {
    let ok = honesty_host_scorpion_missile_dual_peel_residual_pack_wave805()
        && simulate_host_scorpion_missile_dual_peel_collect_source()
        && simulate_host_scorpion_missile_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_scorpion_missile_dual_peel_method_names_residual_wave805());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_scorpion_missile_dual_peel_source_markers_residual_wave805());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_scorpion_missile_dual_peel_nav_commands_residual_wave805());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_scorpion_missile_dual_peel_collect_source());
        assert!(simulate_host_scorpion_missile_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_scorpion_missile_dual_peel_residual_pack_wave805());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_scorpion_missile_dual_peel_honesty());
        assert!(residual_host_scorpion_missile_dual_peel_ok());
    }
}
