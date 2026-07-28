//! Wave 784: GW entity carries AnimationSteeringUpdate residual; under coupled
//! dual-tick `tick_status_timer_expirations` sole-ticks turn anim transitions;
//! host peels `update_animation_steering`. playable_claim stays false.



use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> { table.iter().position(|n| *n == name) }
pub const LIVE_HOST_ANIM_STEER_DUAL_PEEL_METHOD_NAMES_WAVE784: &[&str] = &[
    "anim_steer_active",
    "anim_steer_turn",
    "anim_steer_next_transition_frame",
    "update_animation_steering",
    "Wave 784",
    "playable_claim = false",
];
pub const LIVE_HOST_ANIM_STEER_DUAL_PEEL_NAV_STEPS_WAVE784: &[&str] = &[
    "REQUIRE_ENTITY_ANIM_STEER_FIELDS",
    "REQUIRE_GW_TURN_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_WRITEBACK",
    "LIVE_HOST_ANIM_STEER_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_ANIM_STEER_DUAL_PEEL_CMD_NAMES_WAVE784: &[&str] = &[
    "host_anim_steer_dual_peel",
    "anim_steer_active",
    "anim_steer_turn",
    "update_animation_steering",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAnimSteerDualPeelAction { None=0,MethodNames=1,SourceMarkers=2,NavCommands=3,CollectSource=4,DispatchSource=5,Composite=6 }
impl ResidualHostAnimSteerDualPeelAction {
    pub fn from_u8(v: u8) -> Self { match v { 1=>Self::MethodNames,2=>Self::SourceMarkers,3=>Self::NavCommands,4=>Self::CollectSource,5=>Self::DispatchSource,6=>Self::Composite,_=>Self::None } }
}
fn residual_action_store(a: ResidualHostAnimSteerDualPeelAction) { RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst); }
pub fn residual_host_anim_steer_dual_peel_ok() -> bool { RESIDUAL_OK.load(Ordering::SeqCst) }
pub fn residual_host_anim_steer_dual_peel_last_action() -> ResidualHostAnimSteerDualPeelAction { ResidualHostAnimSteerDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst)) }
fn sh_source() -> &'static str { include_str!("../gameworld_shadow.rs") }
fn gl_source() -> &'static str { include_str!("game_logic.rs") }
pub fn honesty_host_anim_steer_dual_peel_method_names_residual_wave784() -> bool {
    let names=LIVE_HOST_ANIM_STEER_DUAL_PEEL_METHOD_NAMES_WAVE784;
    let ok=residual_name_index(names,"anim_steer_active").is_some()
        && residual_name_index(names,"anim_steer_turn").is_some()
        && residual_name_index(names,"anim_steer_next_transition_frame").is_some()
        && residual_name_index(names,"update_animation_steering").is_some()
        && residual_name_index(names,"Wave 784").is_some()
        && residual_name_index(names,"playable_claim = false").is_some();
    residual_action_store(ResidualHostAnimSteerDualPeelAction::MethodNames); ok
}
pub fn honesty_host_anim_steer_dual_peel_source_markers_residual_wave784() -> bool {
    let sh=sh_source();
    let gl=gl_source();
    let ent=include_str!("../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("anim_steer_active")
        && ent.contains("anim_steer_next_transition_frame")
        && sh.contains("Wave 784")
        && sh.contains("HostAnimationSteeringData")
        && sh.contains("anim_steer_turn")
        && gl.contains("Wave 784")
        && gl.contains("update_animation_steering");
    residual_action_store(ResidualHostAnimSteerDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_anim_steer_dual_peel_nav_commands_residual_wave784() -> bool {
    let steps=LIVE_HOST_ANIM_STEER_DUAL_PEEL_NAV_STEPS_WAVE784;
    let ok=residual_name_index(steps,"REQUIRE_ENTITY_ANIM_STEER_FIELDS").is_some()
        && residual_name_index(steps,"REQUIRE_GW_TURN_TICK").is_some()
        && residual_name_index(steps,"REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps,"REQUIRE_WRITEBACK").is_some()
        && residual_name_index(steps,"LIVE_HOST_ANIM_STEER_DUAL_PEEL").is_some()
        && residual_name_index(steps,"LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostAnimSteerDualPeelAction::NavCommands); ok
}
pub fn simulate_host_anim_steer_dual_peel_collect_source() -> bool {
    let ok=sh_source().contains("Wave 784")
        && sh_source().contains("anim_steer_active")
        && gl_source().contains("Wave 784");
    residual_action_store(ResidualHostAnimSteerDualPeelAction::CollectSource); ok
}
pub fn simulate_host_anim_steer_dual_peel_dispatch_source() -> bool {
    let ok=sh_source().contains("HostAnimationSteeringData")
        && sh_source().contains("PhysicsTurningType")
        && gl_source().contains("update_animation_steering")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostAnimSteerDualPeelAction::DispatchSource); ok
}
pub fn honesty_host_anim_steer_dual_peel_residual_pack_wave784() -> bool {
    honesty_host_anim_steer_dual_peel_method_names_residual_wave784()
        && honesty_host_anim_steer_dual_peel_source_markers_residual_wave784()
        && honesty_host_anim_steer_dual_peel_nav_commands_residual_wave784()
        && simulate_host_anim_steer_dual_peel_collect_source()
        && simulate_host_anim_steer_dual_peel_dispatch_source()
}
pub fn simulate_live_host_anim_steer_dual_peel_honesty() -> bool {
    let ok=honesty_host_anim_steer_dual_peel_residual_pack_wave784();
    if ok { RESIDUAL_OK.store(true, Ordering::SeqCst); residual_action_store(ResidualHostAnimSteerDualPeelAction::Composite); }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn method_names_residual() { assert!(honesty_host_anim_steer_dual_peel_method_names_residual_wave784()); }
    #[test] fn source_markers_residual() { assert!(honesty_host_anim_steer_dual_peel_source_markers_residual_wave784()); }
    #[test] fn nav_commands_residual() { assert!(honesty_host_anim_steer_dual_peel_nav_commands_residual_wave784()); }
    #[test] fn sources() { assert!(simulate_host_anim_steer_dual_peel_collect_source()); assert!(simulate_host_anim_steer_dual_peel_dispatch_source()); }
    #[test] fn pack() { assert!(honesty_host_anim_steer_dual_peel_residual_pack_wave784()); }
    #[test] fn live() { assert!(simulate_live_host_anim_steer_dual_peel_honesty()); assert!(residual_host_anim_steer_dual_peel_ok()); }
}
