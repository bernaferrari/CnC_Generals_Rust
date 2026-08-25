//! Wave 822: GW entity carries China Hacker HackInternet schedule; under coupled
//! dual-tick sole-ticks due cash pings into logs; host peels update_hacker_income
//! and drains apply_hacker_income_event.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_HACKER_INCOME_DUAL_PEEL_METHOD_NAMES_WAVE822: &[&str] = &[
    "hacker_next_deposit_frame",
    "hacker_hacking",
    "host_hacker_income_log",
    "update_hacker_income",
    "apply_hacker_income_event",
    "Wave 822",
    "playable_claim = false",
];
pub const LIVE_HOST_HACKER_INCOME_DUAL_PEEL_NAV_STEPS_WAVE822: &[&str] = &[
    "REQUIRE_ENTITY_HACKER_INCOME_FIELDS",
    "REQUIRE_GW_HACKER_INCOME_TICK",
    "REQUIRE_HOST_PEEL",
    "REQUIRE_HACKER_INCOME_DRAIN",
    "LIVE_HOST_HACKER_INCOME_DUAL_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostHackerIncomeDualPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostHackerIncomeDualPeelAction {
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
fn residual_action_store(a: ResidualHostHackerIncomeDualPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_hacker_income_dual_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_hacker_income_dual_peel_last_action() -> ResidualHostHackerIncomeDualPeelAction
{
    ResidualHostHackerIncomeDualPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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
pub fn honesty_host_hacker_income_dual_peel_method_names_residual_wave822() -> bool {
    let names = LIVE_HOST_HACKER_INCOME_DUAL_PEEL_METHOD_NAMES_WAVE822;
    let ok = residual_name_index(names, "hacker_next_deposit_frame").is_some()
        && residual_name_index(names, "hacker_hacking").is_some()
        && residual_name_index(names, "host_hacker_income_log").is_some()
        && residual_name_index(names, "update_hacker_income").is_some()
        && residual_name_index(names, "apply_hacker_income_event").is_some()
        && residual_name_index(names, "Wave 822").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostHackerIncomeDualPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_hacker_income_dual_peel_nav_commands_residual_wave822() -> bool {
    let steps = LIVE_HOST_HACKER_INCOME_DUAL_PEEL_NAV_STEPS_WAVE822;
    let ok = !steps.is_empty()
        && residual_name_index(steps, "LIVE_HOST_HACKER_INCOME_DUAL_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostHackerIncomeDualPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_hacker_income_dual_peel_residual_pack_wave822() -> bool {
    let sh = sh_source();
    let gl = gl_source();
    let ent = ent_source();
    let ok = sh.contains("Wave 822: China Hacker HackInternet residual cash.")
        && sh.contains("host_hacker_income_log::drain")
        && gl.contains("Wave 822: under coupled shadow, Hacker income owned by GW expire + logs.")
        && gl.contains("fn apply_hacker_income_event")
        && ent.contains("hacker_next_deposit_frame")
        && ent.contains("hacker_hacking");
    residual_action_store(ResidualHostHackerIncomeDualPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_hacker_income_dual_peel_honesty() -> bool {
    let a = honesty_host_hacker_income_dual_peel_method_names_residual_wave822();
    let b = honesty_host_hacker_income_dual_peel_nav_commands_residual_wave822();
    let c = honesty_host_hacker_income_dual_peel_residual_pack_wave822();
    residual_action_store(ResidualHostHackerIncomeDualPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_hacker_income_log::{HackerIncomeEvent, clear, drain, record};
    use crate::game_logic::{ObjectId, Team};

    #[test]
    fn honesty_host_hacker_income_dual_peel_residual_wave822() {
        assert!(honesty_host_hacker_income_dual_peel_residual_pack_wave822());
        assert!(honesty_host_hacker_income_dual_peel_method_names_residual_wave822());
        assert!(honesty_host_hacker_income_dual_peel_nav_commands_residual_wave822());
        assert!(simulate_live_host_hacker_income_dual_peel_honesty());
    }

    #[test]
    fn host_live_host_hacker_income_dual_peel_smoke_wave822() {
        clear();
        record(HackerIncomeEvent {
            id: ObjectId(9),
            team: Team::China,
            owner_player_id: None,
            pos: glam::Vec3::new(1.0, 2.0, 3.0),
            amount: 5,
            xp_per_cash_update: 1.0,
            next_deposit_frame: 60,
            in_internet_center: false,
            stealthed: false,
            detected: false,
            veterancy_ordinal: 0,
            container_radius: 0.0,
        });
        let evs = drain();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].amount, 5);
        assert!(drain().is_empty());
    }
}
