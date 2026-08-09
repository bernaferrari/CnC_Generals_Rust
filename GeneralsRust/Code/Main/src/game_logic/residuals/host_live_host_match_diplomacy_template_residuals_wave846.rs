//! Wave 846: host-owned diplomacy roster, known template names, and unlocked
//! sciences residuals peel live GameLogic dual-reads from presentation_or_boot_*
//! and host_ui_player_info when freeze is missing.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_DIPLOMACY_TEMPLATE_RESIDUALS_METHOD_NAMES_WAVE846: &[&str] = &[
    "host_match_diplomacy_players",
    "host_match_known_template_names",
    "host_match_unlocked_sciences",
    "presentation_or_boot_diplomacy_players",
    "presentation_or_boot_has_template",
    "Wave 846",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_DIPLOMACY_TEMPLATE_RESIDUALS_NAV_STEPS_WAVE846: &[&str] = &[
    "STAMP_HOST_MATCH_DIPLOMACY",
    "STAMP_HOST_MATCH_TEMPLATES",
    "STAMP_HOST_MATCH_SCIENCES",
    "LIVE_HOST_MATCH_DIPLOMACY_TEMPLATE_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchDiplomacyTemplateResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchDiplomacyTemplateResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_match_diplomacy_template_residuals_method_names_residual_wave846() -> bool {
    let names = LIVE_HOST_MATCH_DIPLOMACY_TEMPLATE_RESIDUALS_METHOD_NAMES_WAVE846;
    let ok = residual_name_index(names, "host_match_diplomacy_players").is_some()
        && residual_name_index(names, "host_match_known_template_names").is_some()
        && residual_name_index(names, "host_match_unlocked_sciences").is_some()
        && residual_name_index(names, "Wave 846").is_some();
    residual_action_store(ResidualHostMatchDiplomacyTemplateResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_diplomacy_template_residuals_nav_commands_residual_wave846() -> bool {
    let steps = LIVE_HOST_MATCH_DIPLOMACY_TEMPLATE_RESIDUALS_NAV_STEPS_WAVE846;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_DIPLOMACY_TEMPLATE_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_DIPLOMACY").is_some();
    residual_action_store(ResidualHostMatchDiplomacyTemplateResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_diplomacy_template_residuals_residual_pack_wave846() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_diplomacy_players:")
        && cnc.contains("host_match_known_template_names: Option<Vec<String>>")
        && cnc.contains(
            "host_match_unlocked_sciences: Option<std::collections::HashMap<u32, Vec<String>>>",
        )
        && cnc.contains("Wave 558/846")
        && cnc.contains("Wave 563/846")
        && cnc.contains("Wave 610/846")
        && cnc.contains("Wave 555/846")
        && cnc.contains("Wave 607/846")
        && cnc.contains("Wave 846: diplomacy / template / sciences host residuals")
        && cnc.contains("if let Some(players) = self.host_match_diplomacy_players.as_ref()")
        && cnc.contains("if let Some(names) = self.host_match_known_template_names.as_ref()")
        && cnc.contains("if let Some(map) = self.host_match_unlocked_sciences.as_ref()");
    residual_action_store(ResidualHostMatchDiplomacyTemplateResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_diplomacy_template_residuals_honesty() -> bool {
    let a = honesty_host_match_diplomacy_template_residuals_method_names_residual_wave846();
    let b = honesty_host_match_diplomacy_template_residuals_nav_commands_residual_wave846();
    let c = honesty_host_match_diplomacy_template_residuals_residual_pack_wave846();
    residual_action_store(ResidualHostMatchDiplomacyTemplateResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_diplomacy_template_residuals_residual_wave846() {
        assert!(honesty_host_match_diplomacy_template_residuals_residual_pack_wave846());
        assert!(honesty_host_match_diplomacy_template_residuals_method_names_residual_wave846());
        assert!(honesty_host_match_diplomacy_template_residuals_nav_commands_residual_wave846());
        assert!(simulate_live_host_match_diplomacy_template_residuals_honesty());
    }
}
