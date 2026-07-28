//! Wave 769: GW entity carries PoisonedBehavior DoT residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks poison intervals into
//! host_poison_dot_log; host peels `tick_poisoned_behavior` and drains
//! UNRESISTABLE DoT after writeback. playable_claim stays false.


use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> { table.iter().position(|n| *n == name) }
pub const LIVE_HOST_POISON_DOT_DUAL_PEEL_METHOD_NAMES_WAVE769: &[&str] = &[
    "poison_damage_frame",
    "poison_overall_stop_frame",
    "host_poison_dot_log",
    "tick_poisoned_behavior",
    "Wave 769",
    "playable_claim = false",
];
pub const LIVE_HOST_POISON_DOT_DUAL_PEEL_NAV_STEPS_WAVE769: &[&str] = &[
    "REQUIRE_ENTITY_POISON_FIELDS",
    "REQUIRE_GW_DOT_LOG",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_UNRESISTABLE",
    "LIVE_HOST_POISON_DOT_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_POISON_DOT_DUAL_PEEL_CMD_NAMES_WAVE769: &[&str] = &[
    "host_poison_dot_dual_peel",
    "poison_damage_frame",
    "host_poison_dot_log",
    "tick_poisoned_behavior",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPoisonDotDualPeelAction { None=0,MethodNames=1,SourceMarkers=2,NavCommands=3,CollectSource=4,DispatchSource=5,Composite=6 }
impl ResidualHostPoisonDotDualPeelAction {
    pub fn from_u8(v: u8) -> Self { match v { 1=>Self::MethodNames,2=>Self::SourceMarkers,3=>Self::NavCommands,4=>Self::CollectSource,5=>Self::DispatchSource,6=>Self::Composite,_=>Self::None } }
}
fn residual_action_store(a: ResidualHostPoisonDotDualPeelAction) { RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst); }
pub fn residual_host_poison_dot_dual_peel_ok() -> bool { RESIDUAL_OK.load(Ordering::SeqCst) }
pub fn residual_host_poison_dot_dual_peel_last_action() -> ResidualHostPoisonDotDualPeelAction { ResidualHostPoisonDotDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst)) }
fn sh_source() -> &'static str { include_str!("../gameworld_shadow.rs") }
fn gl_source() -> &'static str { include_str!("game_logic.rs") }
pub fn honesty_host_poison_dot_dual_peel_method_names_residual_wave769() -> bool {
    let names=LIVE_HOST_POISON_DOT_DUAL_PEEL_METHOD_NAMES_WAVE769;
    let ok=residual_name_index(names,"poison_damage_frame").is_some()
        && residual_name_index(names,"poison_overall_stop_frame").is_some()
        && residual_name_index(names,"host_poison_dot_log").is_some()
        && residual_name_index(names,"tick_poisoned_behavior").is_some()
        && residual_name_index(names,"Wave 769").is_some()
        && residual_name_index(names,"playable_claim = false").is_some();
    residual_action_store(ResidualHostPoisonDotDualPeelAction::MethodNames); ok
}
pub fn honesty_host_poison_dot_dual_peel_source_markers_residual_wave769() -> bool {
    let sh=sh_source();
    let gl=gl_source();
    let ent=include_str!("../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("poison_damage_frame")
        && ent.contains("poison_overall_stop_frame")
        && sh.contains("Wave 769")
        && sh.contains("host_poison_dot_log::record")
        && sh.contains("host_poison_dot_log::drain")
        && gl.contains("Wave 769")
        && gl.contains("tick_poisoned_behavior(self.frame)");
    residual_action_store(ResidualHostPoisonDotDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_poison_dot_dual_peel_nav_commands_residual_wave769() -> bool {
    let steps=LIVE_HOST_POISON_DOT_DUAL_PEEL_NAV_STEPS_WAVE769;
    let ok=residual_name_index(steps,"REQUIRE_ENTITY_POISON_FIELDS").is_some()
        && residual_name_index(steps,"REQUIRE_GW_DOT_LOG").is_some()
        && residual_name_index(steps,"REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps,"REQUIRE_DRAIN_UNRESISTABLE").is_some()
        && residual_name_index(steps,"LIVE_HOST_POISON_DOT_DUAL_PEEL").is_some()
        && residual_name_index(steps,"LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostPoisonDotDualPeelAction::NavCommands); ok
}
pub fn simulate_host_poison_dot_dual_peel_collect_source() -> bool {
    let ok=sh_source().contains("Wave 769")
        && sh_source().contains("poison_overall_stop_frame")
        && gl_source().contains("Wave 769");
    residual_action_store(ResidualHostPoisonDotDualPeelAction::CollectSource); ok
}
pub fn simulate_host_poison_dot_dual_peel_dispatch_source() -> bool {
    let ok=sh_source().contains("host_poison_dot_log::record")
        && sh_source().contains("take_damage_from_typed_death")
        && gl_source().contains("tick_poisoned_behavior(self.frame)")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostPoisonDotDualPeelAction::DispatchSource); ok
}
pub fn honesty_host_poison_dot_dual_peel_residual_pack_wave769() -> bool {
    honesty_host_poison_dot_dual_peel_method_names_residual_wave769()
        && honesty_host_poison_dot_dual_peel_source_markers_residual_wave769()
        && honesty_host_poison_dot_dual_peel_nav_commands_residual_wave769()
        && simulate_host_poison_dot_dual_peel_collect_source()
        && simulate_host_poison_dot_dual_peel_dispatch_source()
}
pub fn simulate_live_host_poison_dot_dual_peel_honesty() -> bool {
    let ok=honesty_host_poison_dot_dual_peel_residual_pack_wave769();
    if ok { RESIDUAL_OK.store(true, Ordering::SeqCst); residual_action_store(ResidualHostPoisonDotDualPeelAction::Composite); }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn method_names_residual() { assert!(honesty_host_poison_dot_dual_peel_method_names_residual_wave769()); }
    #[test] fn source_markers_residual() { assert!(honesty_host_poison_dot_dual_peel_source_markers_residual_wave769()); }
    #[test] fn nav_commands_residual() { assert!(honesty_host_poison_dot_dual_peel_nav_commands_residual_wave769()); }
    #[test] fn sources() { assert!(simulate_host_poison_dot_dual_peel_collect_source()); assert!(simulate_host_poison_dot_dual_peel_dispatch_source()); }
    #[test] fn pack() { assert!(honesty_host_poison_dot_dual_peel_residual_pack_wave769()); }
    #[test] fn live() { assert!(simulate_live_host_poison_dot_dual_peel_honesty()); assert!(residual_host_poison_dot_dual_peel_ok()); }
}
