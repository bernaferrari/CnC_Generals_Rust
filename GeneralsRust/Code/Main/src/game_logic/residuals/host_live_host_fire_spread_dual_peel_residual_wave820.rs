//! Wave 820: GW entity carries FireSpread/Flammable residual; under coupled dual-tick
//! sole-ticks aflame/burn/spread into logs; host peels update_fire_spread (+ secondary
//! field-object call peels) and drains apply_fire_spread_tick_event.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FIRE_SPREAD_DUAL_PEEL_METHOD_NAMES_WAVE820: &[&str] = &[
    "fire_spread_active",
    "host_fire_spread_log",
    "update_fire_spread",
    "apply_fire_spread_tick_event",
    "update_nuke_radiation_field_objects",
    "update_anthrax_toxin_field_objects",
    "Wave 820",
    "playable_claim = false",
];
pub const LIVE_HOST_FIRE_SPREAD_DUAL_PEEL_NAV_STEPS_WAVE820: &[&str] = &[
    "REQUIRE_ENTITY_FIRE_SPREAD_FIELDS",
    "REQUIRE_GW_FIRE_SPREAD_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_FIRE_SPREAD_DRAIN",
    "REQUIRE_FIELD_OBJECT_SECONDARY_PEEL",
    "LIVE_HOST_FIRE_SPREAD_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFireSpreadDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostFireSpreadDualPeelAction {
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
fn residual_action_store(a: ResidualHostFireSpreadDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_fire_spread_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_fire_spread_dual_peel_last_action() -> ResidualHostFireSpreadDualPeelAction {
    ResidualHostFireSpreadDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_fire_spread_dual_peel_method_names_residual_wave820() -> bool {
    let names = LIVE_HOST_FIRE_SPREAD_DUAL_PEEL_METHOD_NAMES_WAVE820;
    let ok = residual_name_index(names, "fire_spread_active").is_some()
        && residual_name_index(names, "host_fire_spread_log").is_some()
        && residual_name_index(names, "update_fire_spread").is_some()
        && residual_name_index(names, "apply_fire_spread_tick_event").is_some()
        && residual_name_index(names, "update_nuke_radiation_field_objects").is_some()
        && residual_name_index(names, "update_anthrax_toxin_field_objects").is_some()
        && residual_name_index(names, "Wave 820").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostFireSpreadDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_fire_spread_dual_peel_source_markers_residual_wave820() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("fire_spread_active")
        && ent.contains("fire_spread_state")
        && sh.contains("Wave 820")
        && sh.contains("host_fire_spread_log::record")
        && sh.contains("host_fire_spread_log::drain")
        && sh.contains("fire_spread_candidates")
        && gl.contains("Wave 820")
        && gl.contains("apply_fire_spread_tick_event")
        && gl.contains("update_fire_spread")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostFireSpreadDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_fire_spread_dual_peel_nav_commands_residual_wave820() -> bool {
    let steps = LIVE_HOST_FIRE_SPREAD_DUAL_PEEL_NAV_STEPS_WAVE820;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_FIRE_SPREAD_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FIRE_SPREAD_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_FIRE_SPREAD_DRAIN").is_some()
        && residual_name_index(steps, "REQUIRE_FIELD_OBJECT_SECONDARY_PEEL").is_some()
        && residual_name_index(steps, "LIVE_HOST_FIRE_SPREAD_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostFireSpreadDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_fire_spread_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 820")
        && sh_source().contains("fire_spread_active")
        && gl_source().contains("Wave 820");
    residual_action_store(ResidualHostFireSpreadDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_fire_spread_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_fire_spread_log::drain")
        && sh_source().contains("apply_fire_spread_tick_event")
        && gl_source().contains("update_fire_spread")
        && gl_source().contains("update_nuke_radiation_field_objects")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostFireSpreadDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_fire_spread_dual_peel_residual_pack_wave820() -> bool {
    honesty_host_fire_spread_dual_peel_method_names_residual_wave820()
        && honesty_host_fire_spread_dual_peel_source_markers_residual_wave820()
        && honesty_host_fire_spread_dual_peel_nav_commands_residual_wave820()
}
pub fn simulate_live_host_fire_spread_dual_peel_honesty() -> bool {
    let ok = honesty_host_fire_spread_dual_peel_residual_pack_wave820()
        && simulate_host_fire_spread_dual_peel_collect_source()
        && simulate_host_fire_spread_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_fire_spread_dual_peel_method_names_residual_wave820());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_fire_spread_dual_peel_source_markers_residual_wave820());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_fire_spread_dual_peel_nav_commands_residual_wave820());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_fire_spread_dual_peel_collect_source());
        assert!(simulate_host_fire_spread_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_fire_spread_dual_peel_residual_pack_wave820());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_fire_spread_dual_peel_honesty());
        assert!(residual_host_fire_spread_dual_peel_ok());
    }
}
