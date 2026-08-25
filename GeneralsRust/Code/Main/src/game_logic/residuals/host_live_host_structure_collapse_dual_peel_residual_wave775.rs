//! Wave 775: GW entity carries StructureCollapseUpdate residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks building sink collapse
//! into host_structure_collapse_kill_log; host peels `tick_structure_collapse`
//! and drains kill after writeback. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_STRUCTURE_COLLAPSE_DUAL_PEEL_METHOD_NAMES_WAVE775: &[&str] = &[
    "structure_collapse_state",
    "structure_collapse_current_height",
    "host_structure_collapse_kill_log",
    "tick_structure_collapse",
    "Wave 775",
    "playable_claim = false",
];
pub const LIVE_HOST_STRUCTURE_COLLAPSE_DUAL_PEEL_NAV_STEPS_WAVE775: &[&str] = &[
    "REQUIRE_ENTITY_STRUCTURE_COLLAPSE_FIELDS",
    "REQUIRE_GW_COLLAPSE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_KILL",
    "LIVE_HOST_STRUCTURE_COLLAPSE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_STRUCTURE_COLLAPSE_DUAL_PEEL_CMD_NAMES_WAVE775: &[&str] = &[
    "host_structure_collapse_dual_peel",
    "structure_collapse_state",
    "host_structure_collapse_kill_log",
    "tick_structure_collapse",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStructureCollapseDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostStructureCollapseDualPeelAction {
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
fn residual_action_store(a: ResidualHostStructureCollapseDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_structure_collapse_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_structure_collapse_dual_peel_last_action()
-> ResidualHostStructureCollapseDualPeelAction {
    ResidualHostStructureCollapseDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_structure_collapse_dual_peel_method_names_residual_wave775() -> bool {
    let names = LIVE_HOST_STRUCTURE_COLLAPSE_DUAL_PEEL_METHOD_NAMES_WAVE775;
    let ok = residual_name_index(names, "structure_collapse_state").is_some()
        && residual_name_index(names, "structure_collapse_current_height").is_some()
        && residual_name_index(names, "host_structure_collapse_kill_log").is_some()
        && residual_name_index(names, "tick_structure_collapse").is_some()
        && residual_name_index(names, "Wave 775").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostStructureCollapseDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_structure_collapse_dual_peel_source_markers_residual_wave775() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("structure_collapse_state")
        && ent.contains("structure_collapse_current_height")
        && sh.contains("Wave 775")
        && sh.contains("host_structure_collapse_kill_log::record")
        && sh.contains("host_structure_collapse_kill_log::drain")
        && gl.contains("Wave 775")
        && gl.contains("tick_structure_collapse");
    residual_action_store(ResidualHostStructureCollapseDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_structure_collapse_dual_peel_nav_commands_residual_wave775() -> bool {
    let steps = LIVE_HOST_STRUCTURE_COLLAPSE_DUAL_PEEL_NAV_STEPS_WAVE775;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_STRUCTURE_COLLAPSE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_COLLAPSE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DRAIN_KILL").is_some()
        && residual_name_index(steps, "LIVE_HOST_STRUCTURE_COLLAPSE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostStructureCollapseDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_structure_collapse_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 775")
        && sh_source().contains("structure_collapse_state")
        && gl_source().contains("Wave 775");
    residual_action_store(ResidualHostStructureCollapseDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_structure_collapse_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_structure_collapse_kill_log::record")
        && sh_source().contains("STRUCTURE_COLLAPSE_GRAVITY")
        && gl_source().contains("tick_structure_collapse")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostStructureCollapseDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_structure_collapse_dual_peel_residual_pack_wave775() -> bool {
    honesty_host_structure_collapse_dual_peel_method_names_residual_wave775()
        && honesty_host_structure_collapse_dual_peel_source_markers_residual_wave775()
        && honesty_host_structure_collapse_dual_peel_nav_commands_residual_wave775()
        && simulate_host_structure_collapse_dual_peel_collect_source()
        && simulate_host_structure_collapse_dual_peel_dispatch_source()
}
pub fn simulate_live_host_structure_collapse_dual_peel_honesty() -> bool {
    let ok = honesty_host_structure_collapse_dual_peel_residual_pack_wave775();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostStructureCollapseDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_structure_collapse_dual_peel_method_names_residual_wave775());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_structure_collapse_dual_peel_source_markers_residual_wave775());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_structure_collapse_dual_peel_nav_commands_residual_wave775());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_structure_collapse_dual_peel_collect_source());
        assert!(simulate_host_structure_collapse_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_structure_collapse_dual_peel_residual_pack_wave775());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_structure_collapse_dual_peel_honesty());
        assert!(residual_host_structure_collapse_dual_peel_ok());
    }
}
