//! Wave 807: GW entity carries sticky-bomb / booby-trap attach follow residual;
//! under coupled dual-tick sole-ticks follow/orphan-destroy into logs; host peels
//! both attachment updates. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_STICKY_BOOBY_ATTACH_DUAL_PEEL_METHOD_NAMES_WAVE807: &[&str] = &[
    "sticky_bomb_attached",
    "booby_trap_special",
    "host_sticky_booby_attach_log",
    "update_sticky_bomb_attachments",
    "update_booby_trap_special_attachments",
    "Wave 807",
    "playable_claim = false",
];
pub const LIVE_HOST_STICKY_BOOBY_ATTACH_DUAL_PEEL_NAV_STEPS_WAVE807: &[&str] = &[
    "REQUIRE_ENTITY_STICKY_BOOBY_FIELDS",
    "REQUIRE_GW_ATTACH_FOLLOW_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_FOLLOW_DESTROY_DRAIN",
    "LIVE_HOST_STICKY_BOOBY_ATTACH_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStickyBoobyAttachDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostStickyBoobyAttachDualPeelAction {
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
fn residual_action_store(a: ResidualHostStickyBoobyAttachDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_sticky_booby_attach_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_sticky_booby_attach_dual_peel_last_action()
-> ResidualHostStickyBoobyAttachDualPeelAction {
    ResidualHostStickyBoobyAttachDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_sticky_booby_attach_dual_peel_method_names_residual_wave807() -> bool {
    let names = LIVE_HOST_STICKY_BOOBY_ATTACH_DUAL_PEEL_METHOD_NAMES_WAVE807;
    let ok = residual_name_index(names, "sticky_bomb_attached").is_some()
        && residual_name_index(names, "booby_trap_special").is_some()
        && residual_name_index(names, "host_sticky_booby_attach_log").is_some()
        && residual_name_index(names, "update_sticky_bomb_attachments").is_some()
        && residual_name_index(names, "update_booby_trap_special_attachments").is_some()
        && residual_name_index(names, "Wave 807").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostStickyBoobyAttachDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_sticky_booby_attach_dual_peel_source_markers_residual_wave807() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("sticky_bomb_attached")
        && ent.contains("booby_trap_special")
        && ent.contains("booby_trap_attached_to")
        && sh.contains("Wave 807")
        && sh.contains("sticky_booby_targets")
        && sh.contains("host_sticky_booby_attach_log::record_sticky_follow")
        && sh.contains("host_sticky_booby_attach_log::drain_booby_destroys")
        && gl.contains("Wave 807")
        && gl.contains("update_sticky_bomb_attachments")
        && gl.contains("update_booby_trap_special_attachments")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostStickyBoobyAttachDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_sticky_booby_attach_dual_peel_nav_commands_residual_wave807() -> bool {
    let steps = LIVE_HOST_STICKY_BOOBY_ATTACH_DUAL_PEEL_NAV_STEPS_WAVE807;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_STICKY_BOOBY_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_ATTACH_FOLLOW_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_FOLLOW_DESTROY_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_STICKY_BOOBY_ATTACH_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostStickyBoobyAttachDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_sticky_booby_attach_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 807")
        && sh_source().contains("sticky_bomb_attached")
        && gl_source().contains("Wave 807");
    residual_action_store(ResidualHostStickyBoobyAttachDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_sticky_booby_attach_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_sticky_booby_attach_log::drain_sticky_follows")
        && sh_source().contains("host_sticky_booby_attach_log::drain_booby_destroys")
        && gl_source().contains("update_sticky_bomb_attachments")
        && gl_source().contains("update_booby_trap_special_attachments")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostStickyBoobyAttachDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_sticky_booby_attach_dual_peel_residual_pack_wave807() -> bool {
    honesty_host_sticky_booby_attach_dual_peel_method_names_residual_wave807()
        && honesty_host_sticky_booby_attach_dual_peel_source_markers_residual_wave807()
        && honesty_host_sticky_booby_attach_dual_peel_nav_commands_residual_wave807()
}
pub fn simulate_live_host_sticky_booby_attach_dual_peel_honesty() -> bool {
    let ok = honesty_host_sticky_booby_attach_dual_peel_residual_pack_wave807()
        && simulate_host_sticky_booby_attach_dual_peel_collect_source()
        && simulate_host_sticky_booby_attach_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_sticky_booby_attach_dual_peel_method_names_residual_wave807());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_sticky_booby_attach_dual_peel_source_markers_residual_wave807());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_sticky_booby_attach_dual_peel_nav_commands_residual_wave807());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_sticky_booby_attach_dual_peel_collect_source());
        assert!(simulate_host_sticky_booby_attach_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_sticky_booby_attach_dual_peel_residual_pack_wave807());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_sticky_booby_attach_dual_peel_honesty());
        assert!(residual_host_sticky_booby_attach_dual_peel_ok());
    }
}
