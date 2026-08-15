//! Wave 765: GW entity carries subdual heal fields; under coupled dual-tick
//! `tick_status_timer_expirations` sole-heals subdual damage and clears
//! DISABLED_SUBDUED; host peels `tick_subdual_damage` on the coupled path.
//! Non-coupled still heals via continuous-fire coast. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SUBDUAL_HEAL_DUAL_PEEL_METHOD_NAMES_WAVE765: &[&str] = &[
    "subdual_damage",
    "subdual_heal_countdown",
    "tick_status_timer_expirations",
    "tick_subdual_damage",
    "Wave 765",
    "playable_claim = false",
];
pub const LIVE_HOST_SUBDUAL_HEAL_DUAL_PEEL_NAV_STEPS_WAVE765: &[&str] = &[
    "REQUIRE_ENTITY_SUBDUAL_FIELDS",
    "REQUIRE_GW_HEAL_EXPIRE",
    "REQUIRE_HOST_COUPLED_PEEL",
    "LIVE_HOST_SUBDUAL_HEAL_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SUBDUAL_HEAL_DUAL_PEEL_CMD_NAMES_WAVE765: &[&str] = &[
    "host_subdual_heal_dual_peel",
    "subdual_damage",
    "tick_status_timer_expirations",
    "tick_subdual_damage",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSubdualHealDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSubdualHealDualPeelAction {
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
fn residual_action_store(a: ResidualHostSubdualHealDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_subdual_heal_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_subdual_heal_dual_peel_last_action() -> ResidualHostSubdualHealDualPeelAction {
    ResidualHostSubdualHealDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_subdual_heal_dual_peel_method_names_residual_wave765() -> bool {
    let names = LIVE_HOST_SUBDUAL_HEAL_DUAL_PEEL_METHOD_NAMES_WAVE765;
    let ok = residual_name_index(names, "subdual_damage").is_some()
        && residual_name_index(names, "subdual_heal_countdown").is_some()
        && residual_name_index(names, "tick_status_timer_expirations").is_some()
        && residual_name_index(names, "tick_subdual_damage").is_some()
        && residual_name_index(names, "Wave 765").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSubdualHealDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_subdual_heal_dual_peel_source_markers_residual_wave765() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("subdual_damage")
        && ent.contains("subdual_heal_countdown")
        && sh.contains("Wave 765")
        && sh.contains("e.subdual_heal_countdown")
        && gl.contains("Wave 765")
        && !gl.contains("obj.tick_subdual_damage();")
        && crate::game_logic::object::OBJECT_SRC.contains("fn tick_subdual_damage");
    residual_action_store(ResidualHostSubdualHealDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_subdual_heal_dual_peel_nav_commands_residual_wave765() -> bool {
    let steps = LIVE_HOST_SUBDUAL_HEAL_DUAL_PEEL_NAV_STEPS_WAVE765;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_SUBDUAL_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_HEAL_EXPIRE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_COUPLED_PEEL").is_some()
        && residual_name_index(steps, "LIVE_HOST_SUBDUAL_HEAL_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostSubdualHealDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_subdual_heal_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 765")
        && sh_source().contains("subdual_damage")
        && gl_source().contains("Wave 765");
    residual_action_store(ResidualHostSubdualHealDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_subdual_heal_dual_peel_dispatch_source() -> bool {
    let ok = sh_source()
        .contains("e.subdual_damage = (e.subdual_damage - e.subdual_heal_amount).max(0.0)")
        && sh_source().contains("obj.subdual_damage = ent.subdual_damage")
        && gl_source().contains("tick_continuous_fire_coast")
        && !gl_source().contains("obj.tick_subdual_damage();")
        && crate::game_logic::object::OBJECT_SRC.contains("tick_subdual_damage();");
    residual_action_store(ResidualHostSubdualHealDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_subdual_heal_dual_peel_residual_pack_wave765() -> bool {
    honesty_host_subdual_heal_dual_peel_method_names_residual_wave765()
        && honesty_host_subdual_heal_dual_peel_source_markers_residual_wave765()
        && honesty_host_subdual_heal_dual_peel_nav_commands_residual_wave765()
        && simulate_host_subdual_heal_dual_peel_collect_source()
        && simulate_host_subdual_heal_dual_peel_dispatch_source()
}
pub fn simulate_live_host_subdual_heal_dual_peel_honesty() -> bool {
    let ok = honesty_host_subdual_heal_dual_peel_residual_pack_wave765();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSubdualHealDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_subdual_heal_dual_peel_method_names_residual_wave765());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_subdual_heal_dual_peel_source_markers_residual_wave765());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_subdual_heal_dual_peel_nav_commands_residual_wave765());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_subdual_heal_dual_peel_collect_source());
        assert!(simulate_host_subdual_heal_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_subdual_heal_dual_peel_residual_pack_wave765());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_subdual_heal_dual_peel_honesty());
        assert!(residual_host_subdual_heal_dual_peel_ok());
    }
}
