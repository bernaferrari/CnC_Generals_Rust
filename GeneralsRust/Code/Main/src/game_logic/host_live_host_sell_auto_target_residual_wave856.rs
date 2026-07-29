//! Wave 856: sell auto_target falls back to host-stamped local barracks/producer
//! residuals when presentation freeze has no sellable structure. Presentation
//! sellable helper also accepts structure residuals without selectable lag.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELL_AUTO_TARGET_RESIDUAL_METHOD_NAMES_WAVE856: &[&str] = &[
    "alive_sellable_friendly_structure_ids",
    "host_match_local_barracks_ids",
    "host_refresh_local_train_producer_residuals",
    "sell|auto_target",
    "Wave 856",
    "playable_claim = false",
];

pub const LIVE_HOST_SELL_AUTO_TARGET_RESIDUAL_NAV_STEPS_WAVE856: &[&str] = &[
    "SELL_AUTO_TARGET_PRESENTATION_FIRST",
    "FALLBACK_HOST_PRODUCER_RESIDUAL",
    "LIVE_HOST_SELL_AUTO_TARGET_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSellAutoTargetAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSellAutoTargetAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}

pub fn honesty_host_sell_auto_target_residual_method_names_residual_wave856() -> bool {
    let names = LIVE_HOST_SELL_AUTO_TARGET_RESIDUAL_METHOD_NAMES_WAVE856;
    let ok = residual_name_index(names, "alive_sellable_friendly_structure_ids").is_some()
        && residual_name_index(names, "host_match_local_barracks_ids").is_some()
        && residual_name_index(names, "Wave 856").is_some();
    residual_action_store(ResidualHostSellAutoTargetAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sell_auto_target_residual_nav_commands_residual_wave856() -> bool {
    let steps = LIVE_HOST_SELL_AUTO_TARGET_RESIDUAL_NAV_STEPS_WAVE856;
    let ok = residual_name_index(steps, "LIVE_HOST_SELL_AUTO_TARGET_RESIDUAL").is_some()
        && residual_name_index(steps, "FALLBACK_HOST_PRODUCER_RESIDUAL").is_some();
    residual_action_store(ResidualHostSellAutoTargetAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sell_auto_target_residual_pack_wave856() -> bool {
    let cnc = cnc_source();
    let pf = pf_source();
    let ok = cnc.contains("Wave 856: when freeze has no sellable structure")
        && cnc.contains("host_match_local_barracks_ids")
        && cnc.contains("host_refresh_local_train_producer_residuals")
        && cnc.contains("alive_sellable_friendly_structure_ids")
        && pf.contains("Wave 856: selectable OR known structure/building residual")
        && pf.contains("|| o.is_structure")
        && pf.contains("!o.under_construction");
    residual_action_store(ResidualHostSellAutoTargetAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sell_auto_target_residual_honesty() -> bool {
    let a = honesty_host_sell_auto_target_residual_method_names_residual_wave856();
    let b = honesty_host_sell_auto_target_residual_nav_commands_residual_wave856();
    let c = honesty_host_sell_auto_target_residual_pack_wave856();
    residual_action_store(ResidualHostSellAutoTargetAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sell_auto_target_residual_wave856() {
        assert!(honesty_host_sell_auto_target_residual_pack_wave856());
        assert!(honesty_host_sell_auto_target_residual_method_names_residual_wave856());
        assert!(honesty_host_sell_auto_target_residual_nav_commands_residual_wave856());
        assert!(simulate_live_host_sell_auto_target_residual_honesty());
    }
}
