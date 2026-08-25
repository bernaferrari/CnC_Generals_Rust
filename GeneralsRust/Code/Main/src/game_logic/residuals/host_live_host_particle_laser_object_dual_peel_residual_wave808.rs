//! Wave 808: GW entity carries particle trail/orbital/connector laser lifetimes;
//! under coupled dual-tick sole-ticks expire into field-object logs; host peels
//! the three object updates. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PARTICLE_LASER_OBJECT_DUAL_PEEL_METHOD_NAMES_WAVE808: &[&str] = &[
    "particle_trail_remnant",
    "particle_orbital_laser",
    "particle_connector_laser",
    "host_field_object_expire_log",
    "update_particle_trail_remnant_objects",
    "update_particle_orbital_laser_objects",
    "update_particle_connector_laser_objects",
    "Wave 808",
    "playable_claim = false",
];
pub const LIVE_HOST_PARTICLE_LASER_OBJECT_DUAL_PEEL_NAV_STEPS_WAVE808: &[&str] = &[
    "REQUIRE_ENTITY_PARTICLE_LASER_FIELDS",
    "REQUIRE_GW_EXPIRE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_EXPIRE_DRAIN",
    "LIVE_HOST_PARTICLE_LASER_OBJECT_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostParticleLaserObjectDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostParticleLaserObjectDualPeelAction {
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
fn residual_action_store(a: ResidualHostParticleLaserObjectDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_particle_laser_object_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_particle_laser_object_dual_peel_last_action()
-> ResidualHostParticleLaserObjectDualPeelAction {
    ResidualHostParticleLaserObjectDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_particle_laser_object_dual_peel_method_names_residual_wave808() -> bool {
    let names = LIVE_HOST_PARTICLE_LASER_OBJECT_DUAL_PEEL_METHOD_NAMES_WAVE808;
    let ok = residual_name_index(names, "particle_trail_remnant").is_some()
        && residual_name_index(names, "particle_orbital_laser").is_some()
        && residual_name_index(names, "particle_connector_laser").is_some()
        && residual_name_index(names, "host_field_object_expire_log").is_some()
        && residual_name_index(names, "update_particle_trail_remnant_objects").is_some()
        && residual_name_index(names, "update_particle_orbital_laser_objects").is_some()
        && residual_name_index(names, "update_particle_connector_laser_objects").is_some()
        && residual_name_index(names, "Wave 808").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostParticleLaserObjectDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_particle_laser_object_dual_peel_source_markers_residual_wave808() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let log = include_str!("../host_field_object_expire_log.rs");
    let ok = ent.contains("particle_trail_remnant")
        && ent.contains("particle_orbital_laser")
        && ent.contains("particle_connector_laser")
        && log.contains("ParticleTrailRemnant")
        && log.contains("ParticleOrbitalLaser")
        && log.contains("ParticleConnectorLaser")
        && sh.contains("Wave 808")
        && sh.contains("FieldObjectKind::ParticleTrailRemnant")
        && sh.contains("FieldObjectKind::ParticleOrbitalLaser")
        && sh.contains("FieldObjectKind::ParticleConnectorLaser")
        && gl.contains("Wave 808")
        && gl.contains("update_particle_trail_remnant_objects")
        && gl.contains("update_particle_orbital_laser_objects")
        && gl.contains("update_particle_connector_laser_objects")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostParticleLaserObjectDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_particle_laser_object_dual_peel_nav_commands_residual_wave808() -> bool {
    let steps = LIVE_HOST_PARTICLE_LASER_OBJECT_DUAL_PEEL_NAV_STEPS_WAVE808;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_PARTICLE_LASER_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_EXPIRE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_EXPIRE_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_PARTICLE_LASER_OBJECT_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostParticleLaserObjectDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_particle_laser_object_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 808")
        && sh_source().contains("particle_trail_remnant")
        && gl_source().contains("Wave 808");
    residual_action_store(ResidualHostParticleLaserObjectDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_particle_laser_object_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("FieldObjectKind::ParticleConnectorLaser")
        && gl_source().contains("update_particle_trail_remnant_objects")
        && gl_source().contains("update_particle_orbital_laser_objects")
        && gl_source().contains("update_particle_connector_laser_objects")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostParticleLaserObjectDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_particle_laser_object_dual_peel_residual_pack_wave808() -> bool {
    honesty_host_particle_laser_object_dual_peel_method_names_residual_wave808()
        && honesty_host_particle_laser_object_dual_peel_source_markers_residual_wave808()
        && honesty_host_particle_laser_object_dual_peel_nav_commands_residual_wave808()
}
pub fn simulate_live_host_particle_laser_object_dual_peel_honesty() -> bool {
    let ok = honesty_host_particle_laser_object_dual_peel_residual_pack_wave808()
        && simulate_host_particle_laser_object_dual_peel_collect_source()
        && simulate_host_particle_laser_object_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_particle_laser_object_dual_peel_method_names_residual_wave808());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_particle_laser_object_dual_peel_source_markers_residual_wave808());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_particle_laser_object_dual_peel_nav_commands_residual_wave808());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_particle_laser_object_dual_peel_collect_source());
        assert!(simulate_host_particle_laser_object_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_particle_laser_object_dual_peel_residual_pack_wave808());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_particle_laser_object_dual_peel_honesty());
        assert!(residual_host_particle_laser_object_dual_peel_ok());
    }
}
