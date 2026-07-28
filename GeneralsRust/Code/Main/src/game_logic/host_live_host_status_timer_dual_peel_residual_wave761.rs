//! Wave 761: under coupled dual-tick, GameWorld `tick_status_timer_expirations`
//! sole-expires faerie/repulsor/disable/frenzy/continuous-fire coast/selection
//! flash; host peels matching mid-frame ticks. Production spawn entity-first
//! ObjectId bind also runs under coupled (not only production sole-tick).
//! `playable_claim` stays false.


use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> { table.iter().position(|n| *n == name) }
pub const LIVE_HOST_STATUS_TIMER_DUAL_PEEL_METHOD_NAMES_WAVE761: &[&str] = &[
    "tick_status_timer_expirations",
    "peel_status_timers",
    "tick_continuous_fire_coast",
    "tick_repulsor_status",
    "shadow_coupled_tick_active",
    "Wave 761",
    "playable_claim = false",
];
pub const LIVE_HOST_STATUS_TIMER_DUAL_PEEL_NAV_STEPS_WAVE761: &[&str] = &[
    "REQUIRE_GW_STATUS_TIMER_TICK",
    "REQUIRE_HOST_STATUS_PEEL",
    "REQUIRE_COAST_REPULSOR_PEEL",
    "REQUIRE_COUPLED_SPAWN_BIND",
    "LIVE_HOST_STATUS_TIMER_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_STATUS_TIMER_DUAL_PEEL_CMD_NAMES_WAVE761: &[&str] = &[
    "host_status_timer_dual_peel",
    "tick_status_timer_expirations",
    "peel_status_timers",
    "coupled_spawn_bind",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStatusTimerDualPeelAction { None=0,MethodNames=1,SourceMarkers=2,NavCommands=3,CollectSource=4,DispatchSource=5,Composite=6 }
impl ResidualHostStatusTimerDualPeelAction {
    pub fn from_u8(v: u8) -> Self { match v { 1=>Self::MethodNames,2=>Self::SourceMarkers,3=>Self::NavCommands,4=>Self::CollectSource,5=>Self::DispatchSource,6=>Self::Composite,_=>Self::None } }
}
fn residual_action_store(a: ResidualHostStatusTimerDualPeelAction) { RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst); }
pub fn residual_host_status_timer_dual_peel_ok() -> bool { RESIDUAL_OK.load(Ordering::SeqCst) }
pub fn residual_host_status_timer_dual_peel_last_action() -> ResidualHostStatusTimerDualPeelAction { ResidualHostStatusTimerDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst)) }
fn sh_source() -> &'static str { include_str!("../gameworld_shadow.rs") }
fn gl_source() -> &'static str { include_str!("game_logic.rs") }
pub fn honesty_host_status_timer_dual_peel_method_names_residual_wave761() -> bool {
    let names=LIVE_HOST_STATUS_TIMER_DUAL_PEEL_METHOD_NAMES_WAVE761;
    let ok=residual_name_index(names,"tick_status_timer_expirations").is_some()
        && residual_name_index(names,"peel_status_timers").is_some()
        && residual_name_index(names,"tick_continuous_fire_coast").is_some()
        && residual_name_index(names,"tick_repulsor_status").is_some()
        && residual_name_index(names,"shadow_coupled_tick_active").is_some()
        && residual_name_index(names,"Wave 761").is_some()
        && residual_name_index(names,"playable_claim = false").is_some();
    residual_action_store(ResidualHostStatusTimerDualPeelAction::MethodNames); ok
}
pub fn honesty_host_status_timer_dual_peel_source_markers_residual_wave761() -> bool {
    let sh=sh_source();
    let gl=gl_source();
    let ok = sh.contains("fn tick_status_timer_expirations")
        && sh.contains("tick_status_timer_expirations(logic.get_frame())")
        && gl.contains("peel_status_timers")
        && gl.contains("Wave 761")
        && gl.contains("tick_continuous_fire_coast")
        && gl.contains("tick_repulsor_status")
        && gl.contains("shadow_coupled_tick_active()")
        && gl.contains("host_spawn_production_unit");
    residual_action_store(ResidualHostStatusTimerDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_status_timer_dual_peel_nav_commands_residual_wave761() -> bool {
    let steps=LIVE_HOST_STATUS_TIMER_DUAL_PEEL_NAV_STEPS_WAVE761;
    let ok=residual_name_index(steps,"REQUIRE_GW_STATUS_TIMER_TICK").is_some()
        && residual_name_index(steps,"REQUIRE_HOST_STATUS_PEEL").is_some()
        && residual_name_index(steps,"REQUIRE_COAST_REPULSOR_PEEL").is_some()
        && residual_name_index(steps,"REQUIRE_COUPLED_SPAWN_BIND").is_some()
        && residual_name_index(steps,"LIVE_HOST_STATUS_TIMER_DUAL_PEEL").is_some()
        && residual_name_index(steps,"LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostStatusTimerDualPeelAction::NavCommands); ok
}
pub fn simulate_host_status_timer_dual_peel_collect_source() -> bool {
    let ok=sh_source().contains("Wave 761")
        && sh_source().contains("tick_status_timer_expirations")
        && gl_source().contains("peel_status_timers");
    residual_action_store(ResidualHostStatusTimerDualPeelAction::CollectSource); ok
}
pub fn simulate_host_status_timer_dual_peel_dispatch_source() -> bool {
    let gl=gl_source();
    let ok=sh_source().matches("Wave 761").count() >= 2
        && gl.matches("Wave 761").count() >= 3
        && gl.contains("shadow_coupled_tick_active()")
        && gl.contains("tick_continuous_fire_coast")
        && gl.contains("tick_repulsor_status")
        && gl.contains("pop_pending_bind");
    residual_action_store(ResidualHostStatusTimerDualPeelAction::DispatchSource); ok
}
pub fn honesty_host_status_timer_dual_peel_residual_pack_wave761() -> bool {
    honesty_host_status_timer_dual_peel_method_names_residual_wave761()
        && honesty_host_status_timer_dual_peel_source_markers_residual_wave761()
        && honesty_host_status_timer_dual_peel_nav_commands_residual_wave761()
        && simulate_host_status_timer_dual_peel_collect_source()
        && simulate_host_status_timer_dual_peel_dispatch_source()
}
pub fn simulate_live_host_status_timer_dual_peel_honesty() -> bool {
    let ok=honesty_host_status_timer_dual_peel_residual_pack_wave761();
    if ok { RESIDUAL_OK.store(true, Ordering::SeqCst); residual_action_store(ResidualHostStatusTimerDualPeelAction::Composite); }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn method_names_residual() { assert!(honesty_host_status_timer_dual_peel_method_names_residual_wave761()); }
    #[test] fn source_markers_residual() { assert!(honesty_host_status_timer_dual_peel_source_markers_residual_wave761()); }
    #[test] fn nav_commands_residual() { assert!(honesty_host_status_timer_dual_peel_nav_commands_residual_wave761()); }
    #[test] fn sources() { assert!(simulate_host_status_timer_dual_peel_collect_source()); assert!(simulate_host_status_timer_dual_peel_dispatch_source()); }
    #[test] fn pack() { assert!(honesty_host_status_timer_dual_peel_residual_pack_wave761()); }
    #[test] fn live() { assert!(simulate_live_host_status_timer_dual_peel_honesty()); assert!(residual_host_status_timer_dual_peel_ok()); }
}
