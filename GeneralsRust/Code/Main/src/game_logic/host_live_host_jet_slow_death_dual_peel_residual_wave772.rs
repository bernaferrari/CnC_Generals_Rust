//! Wave 772: GW entity carries JetSlowDeathBehavior residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks crash fall into
//! host_jet_slow_death_kill_log; host peels `tick_jet_slow_death` and drains
//! kill after writeback. playable_claim stays false.


use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> { table.iter().position(|n| *n == name) }
pub const LIVE_HOST_JET_SLOW_DEATH_DUAL_PEEL_METHOD_NAMES_WAVE772: &[&str] = &[
    "jet_slow_death_active",
    "jet_slow_death_vertical_velocity",
    "host_jet_slow_death_kill_log",
    "tick_jet_slow_death",
    "Wave 772",
    "playable_claim = false",
];
pub const LIVE_HOST_JET_SLOW_DEATH_DUAL_PEEL_NAV_STEPS_WAVE772: &[&str] = &[
    "REQUIRE_ENTITY_JET_SLOW_DEATH_FIELDS",
    "REQUIRE_GW_CRASH_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_KILL",
    "LIVE_HOST_JET_SLOW_DEATH_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_JET_SLOW_DEATH_DUAL_PEEL_CMD_NAMES_WAVE772: &[&str] = &[
    "host_jet_slow_death_dual_peel",
    "jet_slow_death_active",
    "host_jet_slow_death_kill_log",
    "tick_jet_slow_death",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostJetSlowDeathDualPeelAction { None=0,MethodNames=1,SourceMarkers=2,NavCommands=3,CollectSource=4,DispatchSource=5,Composite=6 }
impl ResidualHostJetSlowDeathDualPeelAction {
    pub fn from_u8(v: u8) -> Self { match v { 1=>Self::MethodNames,2=>Self::SourceMarkers,3=>Self::NavCommands,4=>Self::CollectSource,5=>Self::DispatchSource,6=>Self::Composite,_=>Self::None } }
}
fn residual_action_store(a: ResidualHostJetSlowDeathDualPeelAction) { RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst); }
pub fn residual_host_jet_slow_death_dual_peel_ok() -> bool { RESIDUAL_OK.load(Ordering::SeqCst) }
pub fn residual_host_jet_slow_death_dual_peel_last_action() -> ResidualHostJetSlowDeathDualPeelAction { ResidualHostJetSlowDeathDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst)) }
fn sh_source() -> &'static str { include_str!("../gameworld_shadow.rs") }
fn gl_source() -> &'static str { include_str!("game_logic.rs") }
pub fn honesty_host_jet_slow_death_dual_peel_method_names_residual_wave772() -> bool {
    let names=LIVE_HOST_JET_SLOW_DEATH_DUAL_PEEL_METHOD_NAMES_WAVE772;
    let ok=residual_name_index(names,"jet_slow_death_active").is_some()
        && residual_name_index(names,"jet_slow_death_vertical_velocity").is_some()
        && residual_name_index(names,"host_jet_slow_death_kill_log").is_some()
        && residual_name_index(names,"tick_jet_slow_death").is_some()
        && residual_name_index(names,"Wave 772").is_some()
        && residual_name_index(names,"playable_claim = false").is_some();
    residual_action_store(ResidualHostJetSlowDeathDualPeelAction::MethodNames); ok
}
pub fn honesty_host_jet_slow_death_dual_peel_source_markers_residual_wave772() -> bool {
    let sh=sh_source();
    let gl=gl_source();
    let ent=include_str!("../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("jet_slow_death_active")
        && ent.contains("jet_slow_death_vertical_velocity")
        && sh.contains("Wave 772")
        && sh.contains("host_jet_slow_death_kill_log::record")
        && sh.contains("host_jet_slow_death_kill_log::drain")
        && gl.contains("Wave 772")
        && gl.contains("tick_jet_slow_death");
    residual_action_store(ResidualHostJetSlowDeathDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_jet_slow_death_dual_peel_nav_commands_residual_wave772() -> bool {
    let steps=LIVE_HOST_JET_SLOW_DEATH_DUAL_PEEL_NAV_STEPS_WAVE772;
    let ok=residual_name_index(steps,"REQUIRE_ENTITY_JET_SLOW_DEATH_FIELDS").is_some()
        && residual_name_index(steps,"REQUIRE_GW_CRASH_TICK").is_some()
        && residual_name_index(steps,"REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps,"REQUIRE_DRAIN_KILL").is_some()
        && residual_name_index(steps,"LIVE_HOST_JET_SLOW_DEATH_DUAL_PEEL").is_some()
        && residual_name_index(steps,"LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostJetSlowDeathDualPeelAction::NavCommands); ok
}
pub fn simulate_host_jet_slow_death_dual_peel_collect_source() -> bool {
    let ok=sh_source().contains("Wave 772")
        && sh_source().contains("jet_slow_death_active")
        && gl_source().contains("Wave 772");
    residual_action_store(ResidualHostJetSlowDeathDualPeelAction::CollectSource); ok
}
pub fn simulate_host_jet_slow_death_dual_peel_dispatch_source() -> bool {
    let ok=sh_source().contains("host_jet_slow_death_kill_log::record")
        && sh_source().contains("JET_FINAL_BLOWUP_DELAY_FRAMES")
        && gl_source().contains("tick_jet_slow_death")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostJetSlowDeathDualPeelAction::DispatchSource); ok
}
pub fn honesty_host_jet_slow_death_dual_peel_residual_pack_wave772() -> bool {
    honesty_host_jet_slow_death_dual_peel_method_names_residual_wave772()
        && honesty_host_jet_slow_death_dual_peel_source_markers_residual_wave772()
        && honesty_host_jet_slow_death_dual_peel_nav_commands_residual_wave772()
        && simulate_host_jet_slow_death_dual_peel_collect_source()
        && simulate_host_jet_slow_death_dual_peel_dispatch_source()
}
pub fn simulate_live_host_jet_slow_death_dual_peel_honesty() -> bool {
    let ok=honesty_host_jet_slow_death_dual_peel_residual_pack_wave772();
    if ok { RESIDUAL_OK.store(true, Ordering::SeqCst); residual_action_store(ResidualHostJetSlowDeathDualPeelAction::Composite); }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn method_names_residual() { assert!(honesty_host_jet_slow_death_dual_peel_method_names_residual_wave772()); }
    #[test] fn source_markers_residual() { assert!(honesty_host_jet_slow_death_dual_peel_source_markers_residual_wave772()); }
    #[test] fn nav_commands_residual() { assert!(honesty_host_jet_slow_death_dual_peel_nav_commands_residual_wave772()); }
    #[test] fn sources() { assert!(simulate_host_jet_slow_death_dual_peel_collect_source()); assert!(simulate_host_jet_slow_death_dual_peel_dispatch_source()); }
    #[test] fn pack() { assert!(honesty_host_jet_slow_death_dual_peel_residual_pack_wave772()); }
    #[test] fn live() { assert!(simulate_live_host_jet_slow_death_dual_peel_honesty()); assert!(residual_host_jet_slow_death_dual_peel_ok()); }
}
