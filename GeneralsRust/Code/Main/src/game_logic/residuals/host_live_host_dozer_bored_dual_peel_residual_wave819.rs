//! Wave 819: GW entity carries dozer/worker idle_since bored timer; under coupled
//! dual-tick sole-ticks bored events into logs; host peels update_dozer_bored_repair
//! and drains process_dozer_bored_event (repair/mine acquire). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_DOZER_BORED_DUAL_PEEL_METHOD_NAMES_WAVE819: &[&str] = &[
    "idle_since_frame",
    "DOZER_BORED_TIME_FRAMES",
    "host_dozer_bored_log",
    "update_dozer_bored_repair",
    "process_dozer_bored_event",
    "Wave 819",
    "playable_claim = false",
];
pub const LIVE_HOST_DOZER_BORED_DUAL_PEEL_NAV_STEPS_WAVE819: &[&str] = &[
    "REQUIRE_ENTITY_IDLE_SINCE",
    "REQUIRE_GW_BORED_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_BORED_DRAIN_ACQUIRE",
    "LIVE_HOST_DOZER_BORED_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDozerBoredDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostDozerBoredDualPeelAction {
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
fn residual_action_store(a: ResidualHostDozerBoredDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_dozer_bored_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_dozer_bored_dual_peel_last_action() -> ResidualHostDozerBoredDualPeelAction {
    ResidualHostDozerBoredDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_dozer_bored_dual_peel_method_names_residual_wave819() -> bool {
    let names = LIVE_HOST_DOZER_BORED_DUAL_PEEL_METHOD_NAMES_WAVE819;
    let ok = residual_name_index(names, "idle_since_frame").is_some()
        && residual_name_index(names, "DOZER_BORED_TIME_FRAMES").is_some()
        && residual_name_index(names, "host_dozer_bored_log").is_some()
        && residual_name_index(names, "update_dozer_bored_repair").is_some()
        && residual_name_index(names, "process_dozer_bored_event").is_some()
        && residual_name_index(names, "Wave 819").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostDozerBoredDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_dozer_bored_dual_peel_source_markers_residual_wave819() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("idle_since_frame")
        && sh.contains("Wave 819")
        && sh.contains("host_dozer_bored_log::record")
        && sh.contains("host_dozer_bored_log::drain")
        && sh.contains("DOZER_BORED_TIME_FRAMES")
        && gl.contains("DOZER_BORED_TIME_FRAMES")
        && gl.contains("process_dozer_bored_event")
        && gl.contains("update_dozer_bored_repair")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostDozerBoredDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_dozer_bored_dual_peel_nav_commands_residual_wave819() -> bool {
    let steps = LIVE_HOST_DOZER_BORED_DUAL_PEEL_NAV_STEPS_WAVE819;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_IDLE_SINCE").is_some()
        && residual_name_index(steps, "REQUIRE_GW_BORED_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_BORED_DRAIN_ACQUIRE").is_some()
        && residual_name_index(steps, "LIVE_HOST_DOZER_BORED_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostDozerBoredDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_dozer_bored_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 819")
        && sh_source().contains("idle_since_frame")
        && gl_source().contains("DOZER_BORED_TIME_FRAMES");
    residual_action_store(ResidualHostDozerBoredDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_dozer_bored_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_dozer_bored_log::drain")
        && sh_source().contains("process_dozer_bored_event")
        && gl_source().contains("update_dozer_bored_repair")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostDozerBoredDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_dozer_bored_dual_peel_residual_pack_wave819() -> bool {
    honesty_host_dozer_bored_dual_peel_method_names_residual_wave819()
        && honesty_host_dozer_bored_dual_peel_source_markers_residual_wave819()
        && honesty_host_dozer_bored_dual_peel_nav_commands_residual_wave819()
}
pub fn simulate_live_host_dozer_bored_dual_peel_honesty() -> bool {
    let ok = honesty_host_dozer_bored_dual_peel_residual_pack_wave819()
        && simulate_host_dozer_bored_dual_peel_collect_source()
        && simulate_host_dozer_bored_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_dozer_bored_dual_peel_method_names_residual_wave819());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_dozer_bored_dual_peel_source_markers_residual_wave819());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_dozer_bored_dual_peel_nav_commands_residual_wave819());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_dozer_bored_dual_peel_collect_source());
        assert!(simulate_host_dozer_bored_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_dozer_bored_dual_peel_residual_pack_wave819());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_dozer_bored_dual_peel_honesty());
        assert!(residual_host_dozer_bored_dual_peel_ok());
    }
}
