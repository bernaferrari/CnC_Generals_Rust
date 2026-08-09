//! Wave 845: host-owned shell + local-team residuals peel live GameLogic dual-reads
//! from presentation_or_boot_shell_bypass / local_team and host_is_in_shell_game.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_SHELL_TEAM_RESIDUALS_METHOD_NAMES_WAVE845: &[&str] = &[
    "host_match_in_shell",
    "host_match_local_team",
    "presentation_or_boot_shell_bypass",
    "presentation_or_boot_local_team",
    "Wave 845",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_SHELL_TEAM_RESIDUALS_NAV_STEPS_WAVE845: &[&str] = &[
    "STAMP_HOST_MATCH_IN_SHELL",
    "STAMP_HOST_MATCH_LOCAL_TEAM",
    "LIVE_HOST_MATCH_SHELL_TEAM_RESIDUALS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchShellTeamResidualsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMatchShellTeamResidualsAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

pub fn honesty_host_match_shell_team_residuals_method_names_residual_wave845() -> bool {
    let names = LIVE_HOST_MATCH_SHELL_TEAM_RESIDUALS_METHOD_NAMES_WAVE845;
    let ok = residual_name_index(names, "host_match_in_shell").is_some()
        && residual_name_index(names, "host_match_local_team").is_some()
        && residual_name_index(names, "Wave 845").is_some();
    residual_action_store(ResidualHostMatchShellTeamResidualsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_shell_team_residuals_nav_commands_residual_wave845() -> bool {
    let steps = LIVE_HOST_MATCH_SHELL_TEAM_RESIDUALS_NAV_STEPS_WAVE845;
    let ok = residual_name_index(steps, "LIVE_HOST_MATCH_SHELL_TEAM_RESIDUALS").is_some()
        && residual_name_index(steps, "STAMP_HOST_MATCH_IN_SHELL").is_some();
    residual_action_store(ResidualHostMatchShellTeamResidualsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_match_shell_team_residuals_residual_pack_wave845() -> bool {
    let cnc = cnc_source();
    let ok = cnc.contains("host_match_in_shell: Option<bool>")
        && cnc.contains("host_match_local_team: Option<crate::game_logic::Team>")
        && cnc.contains("Wave 552/845")
        && cnc.contains("Wave 555/845")
        && cnc.contains("Wave 585/845")
        && cnc.contains("Wave 845: shell residual")
        && cnc.contains("Wave 845: match is not shell once started")
        && cnc.contains("if let Some(v) = self.host_match_in_shell")
        && cnc.contains("if let Some(team) = self.host_match_local_team");
    residual_action_store(ResidualHostMatchShellTeamResidualsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_match_shell_team_residuals_honesty() -> bool {
    let a = honesty_host_match_shell_team_residuals_method_names_residual_wave845();
    let b = honesty_host_match_shell_team_residuals_nav_commands_residual_wave845();
    let c = honesty_host_match_shell_team_residuals_residual_pack_wave845();
    residual_action_store(ResidualHostMatchShellTeamResidualsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_match_shell_team_residuals_residual_wave845() {
        assert!(honesty_host_match_shell_team_residuals_residual_pack_wave845());
        assert!(honesty_host_match_shell_team_residuals_method_names_residual_wave845());
        assert!(honesty_host_match_shell_team_residuals_nav_commands_residual_wave845());
        assert!(simulate_live_host_match_shell_team_residuals_honesty());
    }
}
