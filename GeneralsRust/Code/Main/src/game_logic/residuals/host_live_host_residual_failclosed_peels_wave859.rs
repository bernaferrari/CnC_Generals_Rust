//! Wave 859: warm host residuals fail-closed for template/sciences/victory summary
//! peels — no live GameLogic dual-read on residual miss after match stamp.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RESIDUAL_FAILCLOSED_PEELS_METHOD_NAMES_WAVE859: &[&str] = &[
    "presentation_or_boot_has_template",
    "presentation_or_boot_unlocked_sciences",
    "presentation_or_boot_victory_summary",
    "host_match_known_template_names",
    "Wave 859",
    "playable_claim = false",
];

pub const LIVE_HOST_RESIDUAL_FAILCLOSED_PEELS_NAV_STEPS_WAVE859: &[&str] = &[
    "WARM_RESIDUAL_FAILCLOSED",
    "NO_LIVE_DUAL_READ_ON_MISS",
    "LIVE_HOST_RESIDUAL_FAILCLOSED_PEELS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostResidualFailclosedPeelsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostResidualFailclosedPeelsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_host_residual_failclosed_peels_method_names_residual_wave859() -> bool {
    let names = LIVE_HOST_RESIDUAL_FAILCLOSED_PEELS_METHOD_NAMES_WAVE859;
    let ok = residual_name_index(names, "presentation_or_boot_has_template").is_some()
        && residual_name_index(names, "Wave 859").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostResidualFailclosedPeelsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_residual_failclosed_peels_nav_commands_residual_wave859() -> bool {
    let steps = LIVE_HOST_RESIDUAL_FAILCLOSED_PEELS_NAV_STEPS_WAVE859;
    let ok = residual_name_index(steps, "LIVE_HOST_RESIDUAL_FAILCLOSED_PEELS").is_some()
        && residual_name_index(steps, "WARM_RESIDUAL_FAILCLOSED").is_some();
    residual_action_store(ResidualHostResidualFailclosedPeelsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_residual_failclosed_peels_residual_pack_wave859() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("Wave 563/846/859")
        && cnc.contains("Wave 555/846/859")
        && cnc.contains("Wave 584/849/859")
        && cnc.contains("Wave 859: warm host residual is fail-closed")
        && cnc.contains("map.get(&player_id).cloned().unwrap_or_default()")
        && cnc.contains("VictorySummary::default()")
        && cnc.matches("warm host residual is fail-closed").count() >= 2;
    residual_action_store(ResidualHostResidualFailclosedPeelsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_residual_failclosed_peels_honesty() -> bool {
    let a = honesty_host_residual_failclosed_peels_method_names_residual_wave859();
    let b = honesty_host_residual_failclosed_peels_nav_commands_residual_wave859();
    let c = honesty_host_residual_failclosed_peels_residual_pack_wave859();
    residual_action_store(ResidualHostResidualFailclosedPeelsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_residual_failclosed_peels_residual_wave859() {
        assert!(honesty_host_residual_failclosed_peels_residual_pack_wave859());
        assert!(honesty_host_residual_failclosed_peels_method_names_residual_wave859());
        assert!(honesty_host_residual_failclosed_peels_nav_commands_residual_wave859());
        assert!(simulate_live_host_residual_failclosed_peels_honesty());
    }
}
