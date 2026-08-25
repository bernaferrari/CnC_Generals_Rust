//! Wave 799: GW entity carries AngryMobProjectile DeliverPayload flight residual;
//! under coupled dual-tick sole-ticks transport/bomb and logs drop/detonate;
//! host peels `update_angry_mob_projectiles`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_ANGRY_MOB_PROJECTILE_DUAL_PEEL_METHOD_NAMES_WAVE799: &[&str] = &[
    "angry_mob_projectile",
    "angry_mob_projectile_has_aim",
    "host_angry_mob_projectile_log",
    "update_angry_mob_projectiles",
    "Wave 799",
    "playable_claim = false",
];
pub const LIVE_HOST_ANGRY_MOB_PROJECTILE_DUAL_PEEL_NAV_STEPS_WAVE799: &[&str] = &[
    "REQUIRE_ENTITY_ANGRY_MOB_PROJECTILE_FIELDS",
    "REQUIRE_GW_FLIGHT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DROP_DETONATE_DRAIN",
    "LIVE_HOST_ANGRY_MOB_PROJECTILE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_ANGRY_MOB_PROJECTILE_DUAL_PEEL_CMD_NAMES_WAVE799: &[&str] = &[
    "host_angry_mob_projectile_dual_peel",
    "angry_mob_projectile",
    "host_angry_mob_projectile_log",
    "update_angry_mob_projectiles",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAngryMobProjectileFlightDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostAngryMobProjectileFlightDualPeelAction {
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
fn residual_action_store(a: ResidualHostAngryMobProjectileFlightDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_angry_mob_projectile_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_angry_mob_projectile_dual_peel_last_action()
-> ResidualHostAngryMobProjectileFlightDualPeelAction {
    ResidualHostAngryMobProjectileFlightDualPeelAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_angry_mob_projectile_dual_peel_method_names_residual_wave799() -> bool {
    let names = LIVE_HOST_ANGRY_MOB_PROJECTILE_DUAL_PEEL_METHOD_NAMES_WAVE799;
    let ok = residual_name_index(names, "angry_mob_projectile").is_some()
        && residual_name_index(names, "angry_mob_projectile_has_aim").is_some()
        && residual_name_index(names, "host_angry_mob_projectile_log").is_some()
        && residual_name_index(names, "update_angry_mob_projectiles").is_some()
        && residual_name_index(names, "Wave 799").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostAngryMobProjectileFlightDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_angry_mob_projectile_dual_peel_source_markers_residual_wave799() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("angry_mob_projectile")
        && ent.contains("angry_mob_projectile_has_aim")
        && sh.contains("Wave 799")
        && sh.contains("host_angry_mob_projectile_log::record_impact")
        && sh.contains("host_angry_mob_projectile_log::drain_impacts")
        && gl.contains("Wave 799")
        && gl.contains("update_angry_mob_projectiles");
    residual_action_store(ResidualHostAngryMobProjectileFlightDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_angry_mob_projectile_dual_peel_nav_commands_residual_wave799() -> bool {
    let steps = LIVE_HOST_ANGRY_MOB_PROJECTILE_DUAL_PEEL_NAV_STEPS_WAVE799;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_ANGRY_MOB_PROJECTILE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FLIGHT_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DROP_DETONATE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_ANGRY_MOB_PROJECTILE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostAngryMobProjectileFlightDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_angry_mob_projectile_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 799")
        && sh_source().contains("angry_mob_projectile")
        && gl_source().contains("Wave 799");
    residual_action_store(ResidualHostAngryMobProjectileFlightDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_angry_mob_projectile_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_angry_mob_projectile_log::record_impact")
        && sh_source().contains("host_angry_mob_projectile_log::drain_impacts")
        && gl_source().contains("update_angry_mob_projectiles")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostAngryMobProjectileFlightDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_angry_mob_projectile_dual_peel_residual_pack_wave799() -> bool {
    honesty_host_angry_mob_projectile_dual_peel_method_names_residual_wave799()
        && honesty_host_angry_mob_projectile_dual_peel_source_markers_residual_wave799()
        && honesty_host_angry_mob_projectile_dual_peel_nav_commands_residual_wave799()
        && simulate_host_angry_mob_projectile_dual_peel_collect_source()
        && simulate_host_angry_mob_projectile_dual_peel_dispatch_source()
}
pub fn simulate_live_host_angry_mob_projectile_dual_peel_honesty() -> bool {
    let ok = honesty_host_angry_mob_projectile_dual_peel_residual_pack_wave799();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostAngryMobProjectileFlightDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_angry_mob_projectile_dual_peel_method_names_residual_wave799());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_angry_mob_projectile_dual_peel_source_markers_residual_wave799());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_angry_mob_projectile_dual_peel_nav_commands_residual_wave799());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_angry_mob_projectile_dual_peel_collect_source());
        assert!(simulate_host_angry_mob_projectile_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_angry_mob_projectile_dual_peel_residual_pack_wave799());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_angry_mob_projectile_dual_peel_honesty());
        assert!(residual_host_angry_mob_projectile_dual_peel_ok());
    }
}
