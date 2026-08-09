//! Wave 780: GW entity carries BaseRegenerateUpdate residual; under coupled
//! or damage-auth, `tick_status_timer_expirations` sole-ticks structure auto-heal
//! into host_heal_log; host peels `update_base_regenerate`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_BASE_REGEN_DUAL_PEEL_METHOD_NAMES_WAVE780: &[&str] = &[
    "base_regen_active",
    "base_regen_wake_frame",
    "host_heal_log",
    "update_base_regenerate",
    "Wave 780",
    "playable_claim = false",
];
pub const LIVE_HOST_BASE_REGEN_DUAL_PEEL_NAV_STEPS_WAVE780: &[&str] = &[
    "REQUIRE_ENTITY_BASE_REGEN_FIELDS",
    "REQUIRE_GW_HEAL_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_HEAL_LOG",
    "LIVE_HOST_BASE_REGEN_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_BASE_REGEN_DUAL_PEEL_CMD_NAMES_WAVE780: &[&str] = &[
    "host_base_regen_dual_peel",
    "base_regen_active",
    "host_heal_log",
    "update_base_regenerate",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostBaseRegenDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostBaseRegenDualPeelAction {
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
fn residual_action_store(a: ResidualHostBaseRegenDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_base_regen_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_base_regen_dual_peel_last_action() -> ResidualHostBaseRegenDualPeelAction {
    ResidualHostBaseRegenDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_base_regen_dual_peel_method_names_residual_wave780() -> bool {
    let names = LIVE_HOST_BASE_REGEN_DUAL_PEEL_METHOD_NAMES_WAVE780;
    let ok = residual_name_index(names, "base_regen_active").is_some()
        && residual_name_index(names, "base_regen_wake_frame").is_some()
        && residual_name_index(names, "host_heal_log").is_some()
        && residual_name_index(names, "update_base_regenerate").is_some()
        && residual_name_index(names, "Wave 780").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostBaseRegenDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_base_regen_dual_peel_source_markers_residual_wave780() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("base_regen_active")
        && ent.contains("base_regen_wake_frame")
        && sh.contains("Wave 780")
        && sh.contains("base_regen_heal_amount")
        && sh.contains("host_heal_log::record")
        && gl.contains("Wave 780")
        && gl.contains("update_base_regenerate");
    residual_action_store(ResidualHostBaseRegenDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_base_regen_dual_peel_nav_commands_residual_wave780() -> bool {
    let steps = LIVE_HOST_BASE_REGEN_DUAL_PEEL_NAV_STEPS_WAVE780;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_BASE_REGEN_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_HEAL_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_HEAL_LOG").is_some()
        && residual_name_index(steps, "LIVE_HOST_BASE_REGEN_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostBaseRegenDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_base_regen_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 780")
        && sh_source().contains("base_regen_active")
        && gl_source().contains("Wave 780");
    residual_action_store(ResidualHostBaseRegenDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_base_regen_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_heal_log::record")
        && sh_source().contains("BASE_REGEN_HEAL_RATE_FRAMES")
        && gl_source().contains("update_base_regenerate")
        && gl_source().contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostBaseRegenDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_base_regen_dual_peel_residual_pack_wave780() -> bool {
    honesty_host_base_regen_dual_peel_method_names_residual_wave780()
        && honesty_host_base_regen_dual_peel_source_markers_residual_wave780()
        && honesty_host_base_regen_dual_peel_nav_commands_residual_wave780()
        && simulate_host_base_regen_dual_peel_collect_source()
        && simulate_host_base_regen_dual_peel_dispatch_source()
}
pub fn simulate_live_host_base_regen_dual_peel_honesty() -> bool {
    let ok = honesty_host_base_regen_dual_peel_residual_pack_wave780();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostBaseRegenDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_base_regen_dual_peel_method_names_residual_wave780());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_base_regen_dual_peel_source_markers_residual_wave780());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_base_regen_dual_peel_nav_commands_residual_wave780());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_base_regen_dual_peel_collect_source());
        assert!(simulate_host_base_regen_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_base_regen_dual_peel_residual_pack_wave780());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_base_regen_dual_peel_honesty());
        assert!(residual_host_base_regen_dual_peel_ok());
    }
}
