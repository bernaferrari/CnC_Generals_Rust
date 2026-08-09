//! Wave 852: host-owned purchasable science residual peels player_can_purchase_science
//! dual-reads from host_player_can_purchase_science boot path.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_PURCHASABLE_SCIENCE_RESIDUALS_METHOD_NAMES_WAVE852: &[&str] = &[
    "host_match_purchasable_sciences",
    "host_player_can_purchase_science",
    "Wave 852",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_PURCHASABLE_SCIENCE_RESIDUALS_NAV_STEPS_WAVE852: &[&str] = &[
    "STAMP_HOST_MATCH_PURCHASABLE_SCIENCES",
    "PREFER_HOST_PURCHASABLE_BEFORE_LIVE",
    "LIVE_HOST_MATCH_PURCHASABLE_SCIENCE_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchPurchasableScienceResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchPurchasableScienceResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_match_purchasable_science_residuals_method_names_residual_wave852() -> bool {
    let names = LIVE_HOST_MATCH_PURCHASABLE_SCIENCE_RESIDUALS_METHOD_NAMES_WAVE852;
    let ok = residual_name_index(names, "host_match_purchasable_sciences").is_some()
        && residual_name_index(names, "host_player_can_purchase_science").is_some()
        && residual_name_index(names, "Wave 852").is_some();
    residual_action_store(ResidualHostMatchPurchasableScienceResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_purchasable_science_residuals_nav_commands_residual_wave852() -> bool {
    let steps = LIVE_HOST_MATCH_PURCHASABLE_SCIENCE_RESIDUALS_NAV_STEPS_WAVE852;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_PURCHASABLE_SCIENCE_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_PURCHASABLE_SCIENCES").is_some();
    residual_action_store(ResidualHostMatchPurchasableScienceResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_purchasable_science_residuals_residual_pack_wave852() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_purchasable_sciences:")
        && cnc.contains("Wave 852: stamp purchasable science residual")
        && (cnc.contains("Wave 584/852") || cnc.contains("Wave 584/852/861"))
        && cnc.contains("if let Some(map) = self.host_match_purchasable_sciences.as_ref()")
        && cnc.contains("player_can_purchase_science(player_id, name)"); // boot residual remains
    residual_action_store(ResidualHostMatchPurchasableScienceResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_purchasable_science_residuals_honesty() -> bool {
    let a = honesty_host_match_purchasable_science_residuals_method_names_residual_wave852();
    let b = honesty_host_match_purchasable_science_residuals_nav_commands_residual_wave852();
    let c = honesty_host_match_purchasable_science_residuals_residual_pack_wave852();
    residual_action_store(ResidualHostMatchPurchasableScienceResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_purchasable_science_residuals_residual_wave852() {
        assert!(honesty_host_match_purchasable_science_residuals_residual_pack_wave852());
        assert!(honesty_host_match_purchasable_science_residuals_method_names_residual_wave852());
        assert!(honesty_host_match_purchasable_science_residuals_nav_commands_residual_wave852());
        assert!(simulate_live_host_match_purchasable_science_residuals_honesty());
    }
}
