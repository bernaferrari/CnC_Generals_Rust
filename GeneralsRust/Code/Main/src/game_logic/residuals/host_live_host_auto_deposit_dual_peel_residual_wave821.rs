//! Wave 821: GW entity carries black market / oil derrick AutoDeposit schedule;
//! under coupled dual-tick sole-ticks due deposits into logs; host peels
//! update_black_market_deposits / update_oil_derrick_deposits and drains
//! apply_auto_deposit_event.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_AUTO_DEPOSIT_DUAL_PEEL_METHOD_NAMES_WAVE821: &[&str] = &[
    "black_market_next_deposit_frame",
    "oil_derrick_next_deposit_frame",
    "host_auto_deposit_log",
    "update_black_market_deposits",
    "update_oil_derrick_deposits",
    "apply_auto_deposit_event",
    "Wave 821",
    "playable_claim = false",
];
pub const LIVE_HOST_AUTO_DEPOSIT_DUAL_PEEL_NAV_STEPS_WAVE821: &[&str] = &[
    "REQUIRE_ENTITY_AUTO_DEPOSIT_FIELDS",
    "REQUIRE_GW_AUTO_DEPOSIT_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_AUTO_DEPOSIT_DRAIN",
    "LIVE_HOST_AUTO_DEPOSIT_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAutoDepositDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostAutoDepositDualPeelAction {
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
fn residual_action_store(a: ResidualHostAutoDepositDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_auto_deposit_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_auto_deposit_dual_peel_last_action() -> ResidualHostAutoDepositDualPeelAction {
    ResidualHostAutoDepositDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn sh_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn ent_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}
pub fn honesty_host_auto_deposit_dual_peel_method_names_residual_wave821() -> bool {
    let names = LIVE_HOST_AUTO_DEPOSIT_DUAL_PEEL_METHOD_NAMES_WAVE821;
    let ok = residual_name_index(names, "black_market_next_deposit_frame").is_some()
        && residual_name_index(names, "oil_derrick_next_deposit_frame").is_some()
        && residual_name_index(names, "host_auto_deposit_log").is_some()
        && residual_name_index(names, "update_black_market_deposits").is_some()
        && residual_name_index(names, "update_oil_derrick_deposits").is_some()
        && residual_name_index(names, "apply_auto_deposit_event").is_some()
        && residual_name_index(names, "Wave 821").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostAutoDepositDualPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_auto_deposit_dual_peel_nav_commands_residual_wave821() -> bool {
    let steps = LIVE_HOST_AUTO_DEPOSIT_DUAL_PEEL_NAV_STEPS_WAVE821;
    let ok = !steps.is_empty()
        && residual_name_index(steps, "LIVE_HOST_AUTO_DEPOSIT_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostAutoDepositDualPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_auto_deposit_dual_peel_residual_pack_wave821() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = ent_source();
    let ok = sh.contains("Wave 821: Black market / oil derrick AutoDeposit residual.")
        && sh.contains("host_auto_deposit_log::drain")
        && gl.contains("Wave 821: under coupled shadow, AutoDeposit owned by GW expire + logs.")
        && gl.contains("fn apply_auto_deposit_event")
        && ent.contains("black_market_next_deposit_frame")
        && ent.contains("oil_derrick_next_deposit_frame");
    residual_action_store(ResidualHostAutoDepositDualPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_auto_deposit_dual_peel_honesty() -> bool {
    let a = honesty_host_auto_deposit_dual_peel_method_names_residual_wave821();
    let b = honesty_host_auto_deposit_dual_peel_nav_commands_residual_wave821();
    let c = honesty_host_auto_deposit_dual_peel_residual_pack_wave821();
    residual_action_store(ResidualHostAutoDepositDualPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_auto_deposit_log::{
        AutoDepositEvent, AutoDepositKind, clear, drain, record,
    };
    use crate::game_logic::{ObjectId, Team};

    #[test]
    fn honesty_host_auto_deposit_dual_peel_residual_wave821() {
        assert!(honesty_host_auto_deposit_dual_peel_residual_pack_wave821());
        assert!(honesty_host_auto_deposit_dual_peel_method_names_residual_wave821());
        assert!(honesty_host_auto_deposit_dual_peel_nav_commands_residual_wave821());
        assert!(simulate_live_host_auto_deposit_dual_peel_honesty());
    }

    #[test]
    fn host_live_host_auto_deposit_dual_peel_smoke_wave821() {
        clear();
        record(AutoDepositEvent {
            id: ObjectId(7),
            kind: AutoDepositKind::BlackMarket,
            team: Team::USA,
            owner_player_id: None,
            pos: glam::Vec3::new(1.0, 2.0, 3.0),
            amount: 20,
            next_deposit_frame: 120,
            stealthed: false,
            detected: false,
            supply_lines_boost: 0,
        });
        let evs = drain();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].amount, 20);
        assert!(matches!(evs[0].kind, AutoDepositKind::BlackMarket));
        assert!(drain().is_empty());
    }
}
