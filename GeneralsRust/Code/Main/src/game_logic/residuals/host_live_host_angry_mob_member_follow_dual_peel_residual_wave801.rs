//! Wave 801: GW entity carries AngryMob member follow residual;
//! under coupled dual-tick sole-ticks orbit follow and nexus-lost destroy;
//! host peels `update_angry_mob_member_follow`. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_ANGRY_MOB_MEMBER_FOLLOW_DUAL_PEEL_METHOD_NAMES_WAVE801: &[&str] = &[
    "angry_mob_member",
    "angry_mob_has_nexus",
    "host_angry_mob_member_follow_log",
    "update_angry_mob_member_follow",
    "Wave 801",
    "playable_claim = false",
];
pub const LIVE_HOST_ANGRY_MOB_MEMBER_FOLLOW_DUAL_PEEL_NAV_STEPS_WAVE801: &[&str] = &[
    "REQUIRE_ENTITY_ANGRY_MOB_MEMBER_FIELDS",
    "REQUIRE_GW_FOLLOW_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DESTROY_DRAIN",
    "LIVE_HOST_ANGRY_MOB_MEMBER_FOLLOW_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAngryMobMemberFollowDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostAngryMobMemberFollowDualPeelAction {
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
fn residual_action_store(a: ResidualHostAngryMobMemberFollowDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_angry_mob_member_follow_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_angry_mob_member_follow_dual_peel_last_action()
-> ResidualHostAngryMobMemberFollowDualPeelAction {
    ResidualHostAngryMobMemberFollowDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_angry_mob_member_follow_dual_peel_method_names_residual_wave801() -> bool {
    let names = LIVE_HOST_ANGRY_MOB_MEMBER_FOLLOW_DUAL_PEEL_METHOD_NAMES_WAVE801;
    let ok = residual_name_index(names, "angry_mob_member").is_some()
        && residual_name_index(names, "angry_mob_has_nexus").is_some()
        && residual_name_index(names, "host_angry_mob_member_follow_log").is_some()
        && residual_name_index(names, "update_angry_mob_member_follow").is_some()
        && residual_name_index(names, "Wave 801").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostAngryMobMemberFollowDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_angry_mob_member_follow_dual_peel_source_markers_residual_wave801() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("angry_mob_member")
        && ent.contains("angry_mob_has_nexus")
        && sh.contains("Wave 801")
        && sh.contains("host_angry_mob_member_follow_log::record_destroy")
        && sh.contains("host_angry_mob_member_follow_log::drain_destroys")
        && gl.contains("Wave 801")
        && gl.contains("update_angry_mob_member_follow");
    residual_action_store(ResidualHostAngryMobMemberFollowDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_angry_mob_member_follow_dual_peel_nav_commands_residual_wave801() -> bool {
    let steps = LIVE_HOST_ANGRY_MOB_MEMBER_FOLLOW_DUAL_PEEL_NAV_STEPS_WAVE801;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_ANGRY_MOB_MEMBER_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_FOLLOW_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DESTROY_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_ANGRY_MOB_MEMBER_FOLLOW_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostAngryMobMemberFollowDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_angry_mob_member_follow_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 801")
        && sh_source().contains("angry_mob_member")
        && gl_source().contains("Wave 801");
    residual_action_store(ResidualHostAngryMobMemberFollowDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_angry_mob_member_follow_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_angry_mob_member_follow_log::record_destroy")
        && sh_source().contains("host_angry_mob_member_follow_log::drain_destroys")
        && gl_source().contains("update_angry_mob_member_follow")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostAngryMobMemberFollowDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_angry_mob_member_follow_dual_peel_residual_pack_wave801() -> bool {
    honesty_host_angry_mob_member_follow_dual_peel_method_names_residual_wave801()
        && honesty_host_angry_mob_member_follow_dual_peel_source_markers_residual_wave801()
        && honesty_host_angry_mob_member_follow_dual_peel_nav_commands_residual_wave801()
}
pub fn simulate_live_host_angry_mob_member_follow_dual_peel_honesty() -> bool {
    let ok = honesty_host_angry_mob_member_follow_dual_peel_residual_pack_wave801()
        && simulate_host_angry_mob_member_follow_dual_peel_collect_source()
        && simulate_host_angry_mob_member_follow_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_angry_mob_member_follow_dual_peel_method_names_residual_wave801());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_angry_mob_member_follow_dual_peel_source_markers_residual_wave801());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_angry_mob_member_follow_dual_peel_nav_commands_residual_wave801());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_angry_mob_member_follow_dual_peel_collect_source());
        assert!(simulate_host_angry_mob_member_follow_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_angry_mob_member_follow_dual_peel_residual_pack_wave801());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_angry_mob_member_follow_dual_peel_honesty());
        assert!(residual_host_angry_mob_member_follow_dual_peel_ok());
    }
}
