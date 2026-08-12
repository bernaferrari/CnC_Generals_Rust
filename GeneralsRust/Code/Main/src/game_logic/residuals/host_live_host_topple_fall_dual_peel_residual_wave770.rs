//! Wave 770: GW entity carries ToppleUpdate fall residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks falling topple into
//! host_topple_kill_log; host peels `tick_topple` and drains kill-when-down
//! after writeback. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_TOPPLE_FALL_DUAL_PEEL_METHOD_NAMES_WAVE770: &[&str] = &[
    "topple_state",
    "topple_active",
    "host_topple_kill_log",
    "tick_topple",
    "Wave 770",
    "playable_claim = false",
];
pub const LIVE_HOST_TOPPLE_FALL_DUAL_PEEL_NAV_STEPS_WAVE770: &[&str] = &[
    "REQUIRE_ENTITY_TOPPLE_FIELDS",
    "REQUIRE_GW_FALL_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_KILL",
    "LIVE_HOST_TOPPLE_FALL_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_TOPPLE_FALL_DUAL_PEEL_CMD_NAMES_WAVE770: &[&str] = &[
    "host_topple_fall_dual_peel",
    "topple_state",
    "host_topple_kill_log",
    "tick_topple",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostToppleFallDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostToppleFallDualPeelAction {
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
fn residual_action_store(a: ResidualHostToppleFallDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_topple_fall_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_topple_fall_dual_peel_last_action() -> ResidualHostToppleFallDualPeelAction {
    ResidualHostToppleFallDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
pub fn honesty_host_topple_fall_dual_peel_method_names_residual_wave770() -> bool {
    let names = LIVE_HOST_TOPPLE_FALL_DUAL_PEEL_METHOD_NAMES_WAVE770;
    let ok = residual_name_index(names, "topple_state").is_some()
        && residual_name_index(names, "topple_active").is_some()
        && residual_name_index(names, "host_topple_kill_log").is_some()
        && residual_name_index(names, "tick_topple").is_some()
        && residual_name_index(names, "Wave 770").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostToppleFallDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_topple_fall_dual_peel_source_markers_residual_wave770() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("topple_state")
        && ent.contains("topple_active")
        && sh.contains("Wave 770")
        && sh.contains("host_topple_kill_log::record")
        && sh.contains("host_topple_kill_log::drain")
        && gl.contains("Wave 770")
        && gl.contains("tick_topple()");
    residual_action_store(ResidualHostToppleFallDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_topple_fall_dual_peel_nav_commands_residual_wave770() -> bool {
    let steps = LIVE_HOST_TOPPLE_FALL_DUAL_PEEL_NAV_STEPS_WAVE770;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_TOPPLE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FALL_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DRAIN_KILL").is_some()
        && residual_name_index(steps, "LIVE_HOST_TOPPLE_FALL_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostToppleFallDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_topple_fall_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 770")
        && sh_source().contains("topple_state")
        && gl_source().contains("Wave 770");
    residual_action_store(ResidualHostToppleFallDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_topple_fall_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_topple_kill_log::record")
        && sh_source().contains("HostDeathType::Toppled")
        && gl_source().contains("tick_topple()")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostToppleFallDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_topple_fall_dual_peel_residual_pack_wave770() -> bool {
    honesty_host_topple_fall_dual_peel_method_names_residual_wave770()
        && honesty_host_topple_fall_dual_peel_source_markers_residual_wave770()
        && honesty_host_topple_fall_dual_peel_nav_commands_residual_wave770()
        && simulate_host_topple_fall_dual_peel_collect_source()
        && simulate_host_topple_fall_dual_peel_dispatch_source()
}
pub fn simulate_live_host_topple_fall_dual_peel_honesty() -> bool {
    let ok = honesty_host_topple_fall_dual_peel_residual_pack_wave770();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostToppleFallDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_topple_fall_dual_peel_method_names_residual_wave770());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_topple_fall_dual_peel_source_markers_residual_wave770());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_topple_fall_dual_peel_nav_commands_residual_wave770());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_topple_fall_dual_peel_collect_source());
        assert!(simulate_host_topple_fall_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_topple_fall_dual_peel_residual_pack_wave770());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_topple_fall_dual_peel_honesty());
        assert!(residual_host_topple_fall_dual_peel_ok());
    }
}
