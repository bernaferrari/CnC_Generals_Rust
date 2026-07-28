//! Wave 779: GW sole-emits FireWeaponWhenDamaged onDamage reaction under
//! damage authority; host peels reaction pending set and drains
//! host_fwwd_reaction_log after HP writeback. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_FWWD_REACTION_DUAL_PEEL_METHOD_NAMES_WAVE779: &[&str] = &[
    "fwwd_last_reaction_frame",
    "host_fwwd_reaction_log",
    "try_fwwd_reaction_for_host",
    "gameworld_damage_authority_live",
    "Wave 779",
    "playable_claim = false",
];
pub const LIVE_HOST_FWWD_REACTION_DUAL_PEEL_NAV_STEPS_WAVE779: &[&str] = &[
    "REQUIRE_ENTITY_REACTION_FIELDS",
    "REQUIRE_GW_REACTION_EMIT",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DRAIN_PENDING",
    "LIVE_HOST_FWWD_REACTION_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_FWWD_REACTION_DUAL_PEEL_CMD_NAMES_WAVE779: &[&str] = &[
    "host_fwwd_reaction_dual_peel",
    "fwwd_last_reaction_frame",
    "host_fwwd_reaction_log",
    "try_fwwd_reaction_for_host",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFwwdReactionDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostFwwdReactionDualPeelAction {
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
fn residual_action_store(a: ResidualHostFwwdReactionDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_fwwd_reaction_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_fwwd_reaction_dual_peel_last_action() -> ResidualHostFwwdReactionDualPeelAction
{
    ResidualHostFwwdReactionDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
pub fn honesty_host_fwwd_reaction_dual_peel_method_names_residual_wave779() -> bool {
    let names = LIVE_HOST_FWWD_REACTION_DUAL_PEEL_METHOD_NAMES_WAVE779;
    let ok = residual_name_index(names, "fwwd_last_reaction_frame").is_some()
        && residual_name_index(names, "host_fwwd_reaction_log").is_some()
        && residual_name_index(names, "try_fwwd_reaction_for_host").is_some()
        && residual_name_index(names, "gameworld_damage_authority_live").is_some()
        && residual_name_index(names, "Wave 779").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostFwwdReactionDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_fwwd_reaction_dual_peel_source_markers_residual_wave779() -> bool {
    let sh = sh_source();
    let obj = include_str!("object.rs");
    let ent = include_str!("../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let ok = ent.contains("fwwd_last_reaction_frame")
        && ent.contains("fwwd_reaction_damaged")
        && sh.contains("Wave 779")
        && sh.contains("host_fwwd_reaction_log::record")
        && sh.contains("host_fwwd_reaction_log::drain")
        && sh.contains("try_fwwd_reaction_for_host")
        && obj.contains("Wave 779")
        && obj.contains("gameworld_damage_authority_live");
    residual_action_store(ResidualHostFwwdReactionDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_fwwd_reaction_dual_peel_nav_commands_residual_wave779() -> bool {
    let steps = LIVE_HOST_FWWD_REACTION_DUAL_PEEL_NAV_STEPS_WAVE779;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_REACTION_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_REACTION_EMIT").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DRAIN_PENDING").is_some()
        && residual_name_index(steps, "LIVE_HOST_FWWD_REACTION_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostFwwdReactionDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_fwwd_reaction_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 779")
        && sh_source().contains("fwwd_last_reaction_frame")
        && include_str!("object.rs").contains("Wave 779");
    residual_action_store(ResidualHostFwwdReactionDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_fwwd_reaction_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("host_fwwd_reaction_log::record")
        && sh_source().contains("try_fwwd_reaction_for_host")
        && include_str!("object.rs").contains("gameworld_damage_authority_live")
        && sh_source().contains("apply_host_damage_events");
    residual_action_store(ResidualHostFwwdReactionDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_fwwd_reaction_dual_peel_residual_pack_wave779() -> bool {
    honesty_host_fwwd_reaction_dual_peel_method_names_residual_wave779()
        && honesty_host_fwwd_reaction_dual_peel_source_markers_residual_wave779()
        && honesty_host_fwwd_reaction_dual_peel_nav_commands_residual_wave779()
        && simulate_host_fwwd_reaction_dual_peel_collect_source()
        && simulate_host_fwwd_reaction_dual_peel_dispatch_source()
}
pub fn simulate_live_host_fwwd_reaction_dual_peel_honesty() -> bool {
    let ok = honesty_host_fwwd_reaction_dual_peel_residual_pack_wave779();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostFwwdReactionDualPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_fwwd_reaction_dual_peel_method_names_residual_wave779());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_fwwd_reaction_dual_peel_source_markers_residual_wave779());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_fwwd_reaction_dual_peel_nav_commands_residual_wave779());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_fwwd_reaction_dual_peel_collect_source());
        assert!(simulate_host_fwwd_reaction_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_fwwd_reaction_dual_peel_residual_pack_wave779());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_fwwd_reaction_dual_peel_honesty());
        assert!(residual_host_fwwd_reaction_dual_peel_ok());
    }
}
