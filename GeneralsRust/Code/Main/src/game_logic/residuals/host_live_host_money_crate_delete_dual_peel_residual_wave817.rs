//! Wave 817: GW entity carries money/salvage crate expires_frame; under coupled
//! dual-tick sole-ticks DeletionUpdate into field-object logs; host peels
//! update_crate_deletion_updates and drains destroy + registry forget.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_MONEY_CRATE_DELETE_DUAL_PEEL_METHOD_NAMES_WAVE817: &[&str] = &[
    "money_crate",
    "money_crate_expires_frame",
    "MoneyCrate",
    "host_money_crates",
    "update_crate_deletion_updates",
    "Wave 817",
    "playable_claim = false",
];
pub const LIVE_HOST_MONEY_CRATE_DELETE_DUAL_PEEL_NAV_STEPS_WAVE817: &[&str] = &[
    "REQUIRE_ENTITY_MONEY_CRATE_FIELDS",
    "REQUIRE_GW_EXPIRE_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_DESTROY_FORGET_DRAIN",
    "LIVE_HOST_MONEY_CRATE_DELETE_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMoneyCrateDeleteDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostMoneyCrateDeleteDualPeelAction {
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
fn residual_action_store(a: ResidualHostMoneyCrateDeleteDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_money_crate_delete_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_money_crate_delete_dual_peel_last_action()
-> ResidualHostMoneyCrateDeleteDualPeelAction {
    ResidualHostMoneyCrateDeleteDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_money_crate_delete_dual_peel_method_names_residual_wave817() -> bool {
    let names = LIVE_HOST_MONEY_CRATE_DELETE_DUAL_PEEL_METHOD_NAMES_WAVE817;
    let ok = residual_name_index(names, "money_crate").is_some()
        && residual_name_index(names, "money_crate_expires_frame").is_some()
        && residual_name_index(names, "MoneyCrate").is_some()
        && residual_name_index(names, "host_money_crates").is_some()
        && residual_name_index(names, "update_crate_deletion_updates").is_some()
        && residual_name_index(names, "Wave 817").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostMoneyCrateDeleteDualPeelAction::MethodNames);
    ok
}
pub fn honesty_host_money_crate_delete_dual_peel_source_markers_residual_wave817() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs");
    let log = include_str!("../host_field_object_expire_log.rs");
    let ok = ent.contains("money_crate_expires_frame")
        && ent.contains("money_crate")
        && log.contains("MoneyCrate")
        && sh.contains("Wave 817")
        && sh.contains("FieldObjectKind::MoneyCrate")
        && sh.contains("host_money_crates.forget")
        && sh.contains("host_money_crates.get")
        && gl.contains("Wave 817")
        && gl.contains("update_crate_deletion_updates")
        && gl.contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostMoneyCrateDeleteDualPeelAction::SourceMarkers);
    ok
}
pub fn honesty_host_money_crate_delete_dual_peel_nav_commands_residual_wave817() -> bool {
    let steps = LIVE_HOST_MONEY_CRATE_DELETE_DUAL_PEEL_NAV_STEPS_WAVE817;
    let ok = residual_name_index(steps, "REQUIRE_ENTITY_MONEY_CRATE_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_GW_EXPIRE_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PEEL").is_some()
        && residual_name_index(steps, "REQUIRE_DESTROY_FORGET_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_MONEY_CRATE_DELETE_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostMoneyCrateDeleteDualPeelAction::NavCommands);
    ok
}
pub fn simulate_host_money_crate_delete_dual_peel_collect_source() -> bool {
    let ok = sh_source().contains("Wave 817")
        && sh_source().contains("money_crate_expires_frame")
        && gl_source().contains("Wave 817");
    residual_action_store(ResidualHostMoneyCrateDeleteDualPeelAction::CollectSource);
    ok
}
pub fn simulate_host_money_crate_delete_dual_peel_dispatch_source() -> bool {
    let ok = sh_source().contains("FieldObjectKind::MoneyCrate")
        && sh_source().contains("host_money_crates.forget")
        && gl_source().contains("update_crate_deletion_updates")
        && gl_source().contains("shadow_coupled_tick_active()");
    residual_action_store(ResidualHostMoneyCrateDeleteDualPeelAction::DispatchSource);
    ok
}
pub fn honesty_host_money_crate_delete_dual_peel_residual_pack_wave817() -> bool {
    honesty_host_money_crate_delete_dual_peel_method_names_residual_wave817()
        && honesty_host_money_crate_delete_dual_peel_source_markers_residual_wave817()
        && honesty_host_money_crate_delete_dual_peel_nav_commands_residual_wave817()
}
pub fn simulate_live_host_money_crate_delete_dual_peel_honesty() -> bool {
    let ok = honesty_host_money_crate_delete_dual_peel_residual_pack_wave817()
        && simulate_host_money_crate_delete_dual_peel_collect_source()
        && simulate_host_money_crate_delete_dual_peel_dispatch_source();
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_money_crate_delete_dual_peel_method_names_residual_wave817());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_money_crate_delete_dual_peel_source_markers_residual_wave817());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_money_crate_delete_dual_peel_nav_commands_residual_wave817());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_money_crate_delete_dual_peel_collect_source());
        assert!(simulate_host_money_crate_delete_dual_peel_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_money_crate_delete_dual_peel_residual_pack_wave817());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_money_crate_delete_dual_peel_honesty());
        assert!(residual_host_money_crate_delete_dual_peel_ok());
    }
}
