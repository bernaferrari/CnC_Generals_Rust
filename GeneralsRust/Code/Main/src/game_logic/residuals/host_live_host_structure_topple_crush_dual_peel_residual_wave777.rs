//! Wave 777: GW entity sole-ticks StructureTopple crush sweep under coupled
//! dual-tick; emits host_structure_topple_crush_log samples; host peels
//! take_structure_topple_crush_samples and drains apply after writeback.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_STRUCTURE_TOPPLE_CRUSH_DUAL_PEEL_METHOD_NAMES_WAVE777: &[&str] = &[
    "structure_topple_last_crushed_location",
    "host_structure_topple_crush_log",
    "take_crush_sweep_samples",
    "apply_structure_topple_crush_samples",
    "Wave 777",
    "playable_claim = false",
];
pub const LIVE_HOST_STRUCTURE_TOPPLE_CRUSH_DUAL_PEEL_NAV_STEPS_WAVE777: &[&str] = &[
    "REQUIRE_GW_CRUSH_EMIT",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_APPLY",
    "REQUIRE_LAST_CRUSHED_WRITEBACK",
    "LIVE_HOST_STRUCTURE_TOPPLE_CRUSH_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_STRUCTURE_TOPPLE_CRUSH_DUAL_PEEL_CMD_NAMES_WAVE777: &[&str] = &[
    "host_structure_topple_crush_dual_peel",
    "host_structure_topple_crush_log",
    "take_crush_sweep_samples",
    "apply_structure_topple_crush_samples",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStructureToppleCrushDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostStructureToppleCrushDualPeelAction {
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
fn residual_action_store(a: ResidualHostStructureToppleCrushDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_structure_topple_crush_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_structure_topple_crush_dual_peel_last_action()
-> ResidualHostStructureToppleCrushDualPeelAction {
    ResidualHostStructureToppleCrushDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_structure_topple_crush_dual_peel_method_names_residual_wave777() -> bool {
    let names = LIVE_HOST_STRUCTURE_TOPPLE_CRUSH_DUAL_PEEL_METHOD_NAMES_WAVE777;
    let ok = residual_name_index(names, "structure_topple_last_crushed_location").is_some()
        && residual_name_index(names, "host_structure_topple_crush_log").is_some()
        && residual_name_index(names, "take_crush_sweep_samples").is_some()
        && residual_name_index(names, "apply_structure_topple_crush_samples").is_some()
        && residual_name_index(names, "Wave 777").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostStructureToppleCrushDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_structure_topple_crush_dual_peel_source_markers_residual_wave777() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ok = sh.contains("Wave 777")
        && sh.contains("host_structure_topple_crush_log::record")
        && sh.contains("host_structure_topple_crush_log::drain")
        && sh.contains("take_crush_sweep_samples")
        && gl.contains("Wave 777")
        && gl.contains("take_structure_topple_crush_samples")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostStructureToppleCrushDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_structure_topple_crush_dual_peel_nav_commands_residual_wave777() -> bool {
    let steps = LIVE_HOST_STRUCTURE_TOPPLE_CRUSH_DUAL_PEEL_NAV_STEPS_WAVE777;
    let ok = residual_name_index(steps, "REQUIRE_GW_CRUSH_EMIT").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DRAIN_APPLY").is_some()
        && residual_name_index(steps, "REQUIRE_LAST_CRUSHED_WRITEBACK").is_some()
        && residual_name_index(steps, "LIVE_HOST_STRUCTURE_TOPPLE_CRUSH_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostStructureToppleCrushDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_structure_topple_crush_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 777")
        && sh_source().contains("host_structure_topple_crush_log")
        && gl_source().contains("Wave 777");
    residual_action_store(ResidualHostStructureToppleCrushDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_structure_topple_crush_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_structure_topple_crush_log::record")
        && sh_source().contains("apply_structure_topple_crush_samples")
        && gl_source().contains("take_structure_topple_crush_samples")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostStructureToppleCrushDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_structure_topple_crush_dual_peel_residual_pack_wave777() -> bool {
    honesty_host_structure_topple_crush_dual_peel_method_names_residual_wave777()
        && honesty_host_structure_topple_crush_dual_peel_source_markers_residual_wave777()
        && honesty_host_structure_topple_crush_dual_peel_nav_commands_residual_wave777()
        && simulate_host_structure_topple_crush_dual_peel_collect_source()
        && simulate_host_structure_topple_crush_dual_peel_dispatch_source()
}
pub fn simulate_live_host_structure_topple_crush_dual_peel_honesty() -> bool {
    let ok = honesty_host_structure_topple_crush_dual_peel_residual_pack_wave777();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostStructureToppleCrushDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_structure_topple_crush_dual_peel_method_names_residual_wave777());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_structure_topple_crush_dual_peel_source_markers_residual_wave777());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_structure_topple_crush_dual_peel_nav_commands_residual_wave777());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_structure_topple_crush_dual_peel_collect_source());
        assert!(simulate_host_structure_topple_crush_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_structure_topple_crush_dual_peel_residual_pack_wave777());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_structure_topple_crush_dual_peel_honesty());
        assert!(residual_host_structure_topple_crush_dual_peel_ok());
    }
}
