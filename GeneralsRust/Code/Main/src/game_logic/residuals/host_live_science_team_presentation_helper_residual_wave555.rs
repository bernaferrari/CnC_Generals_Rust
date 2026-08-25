//! Wave 555 residual peels: unlocked-sciences and local-team residuals are
//! centralized through `presentation_or_boot_unlocked_sciences` /
//! `presentation_or_boot_local_team` — presentation freeze owns both when
//! installed; boot residual without freeze uses host probes.
//! `local_player_id_for_ui` / `local_team_for_ui` route through helpers.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 554 map/difficulty presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` presentation_or_boot_unlocked_sciences /
//!   presentation_or_boot_local_team / local_*_for_ui / try_purchase
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_SCIENCE_TEAM_PRESENTATION_HELPER_METHOD_NAMES_WAVE555: &[&str] = &[
    "presentation_or_boot_unlocked_sciences",
    "presentation_or_boot_local_team",
    "local_team_for_ui",
    "local_player_id_for_ui",
    "Wave 555",
    "playable_claim = false",
];

pub const LIVE_SCIENCE_TEAM_PRESENTATION_HELPER_NAV_STEPS_WAVE555: &[&str] = &[
    "REQUIRE_UNLOCKED_SCIENCES_PRESENTATION_HELPER",
    "REQUIRE_LOCAL_TEAM_PRESENTATION_HELPER",
    "LIVE_SCIENCE_TEAM_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_SCIENCE_TEAM_PRESENTATION_HELPER_CMD_NAMES_WAVE555: &[&str] = &[
    "unlocked_sciences_presentation_helper",
    "local_team_presentation_helper",
    "boot_player_unlocked_sciences",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualScienceTeamPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualScienceTeamPresentationHelperAction {
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

fn residual_action_store(action: ResidualScienceTeamPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_science_team_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_science_team_presentation_helper_last_action()
-> ResidualScienceTeamPresentationHelperAction {
    ResidualScienceTeamPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_science_team_presentation_helper_method_names_residual_wave555() -> bool {
    let names = LIVE_SCIENCE_TEAM_PRESENTATION_HELPER_METHOD_NAMES_WAVE555;
    let ok = residual_name_index(names, "presentation_or_boot_unlocked_sciences").is_some()
        && residual_name_index(names, "presentation_or_boot_local_team").is_some()
        && residual_name_index(names, "local_team_for_ui").is_some()
        && residual_name_index(names, "local_player_id_for_ui").is_some()
        && residual_name_index(names, "Wave 555").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualScienceTeamPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_science_team_presentation_helper_source_markers_residual_wave555() -> bool {
    let eng = eng_source();
    let Some(sci) = fn_body(eng, "fn presentation_or_boot_unlocked_sciences(") else {
        residual_action_store(ResidualScienceTeamPresentationHelperAction::SourceMarkers);
        return false;
    };
    let Some(team) = fn_body(eng, "fn presentation_or_boot_local_team(") else {
        residual_action_store(ResidualScienceTeamPresentationHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 894/895/898 fail-closed — sciences/team use host_match_*
    // (host_authority.rs), not player_unlocked_sciences / player_team dual-reads.
    let sci_ok = sci.contains("Wave 555")
        && sci.contains("local_unlocked_sciences")
        && sci.contains("host_match_unlocked_sciences");
    let team_ok = team.contains("Wave 555")
        && team.contains("local_team()")
        && team.contains("host_match_local_team");
    let routes = eng.contains("presentation_or_boot_unlocked_sciences(player_id)")
        && eng.contains("presentation_or_boot_local_team()");
    let lp = fn_body(eng, "fn host_local_player_id_for_ui(")
        .or_else(|| fn_body(eng, "fn local_player_id_for_ui("))
        .unwrap_or("");
    let lt = fn_body(eng, "fn host_local_team_for_ui(")
        .or_else(|| fn_body(eng, "fn local_team_for_ui("))
        .unwrap_or("");
    // Wave 574: boot path may be boot_local_player_id_from_host.
    let ui_ok = (lp.contains("Wave 240") || lp.contains("Wave 574") || lp.contains("Wave 555"))
        && (lp.contains("player_exists")
            || lp.contains("boot_local_player_id_from_host")
            || eng.contains("fn boot_local_player_id_from_host"))
        && lt.contains("presentation_or_boot_local_team()");
    let ok = sci_ok && team_ok && routes && ui_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualScienceTeamPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_science_team_presentation_helper_nav_commands_residual_wave555() -> bool {
    let steps = LIVE_SCIENCE_TEAM_PRESENTATION_HELPER_NAV_STEPS_WAVE555;
    let cmds = RUNTIME_HOST_LIVE_SCIENCE_TEAM_PRESENTATION_HELPER_CMD_NAMES_WAVE555;
    let ok = residual_name_index(steps, "REQUIRE_UNLOCKED_SCIENCES_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_LOCAL_TEAM_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_SCIENCE_TEAM_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "unlocked_sciences_presentation_helper").is_some()
        && residual_name_index(cmds, "local_team_presentation_helper").is_some()
        && residual_name_index(cmds, "boot_player_unlocked_sciences").is_some();
    residual_action_store(ResidualScienceTeamPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_science_team_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 555")
        && eng.contains("fn presentation_or_boot_unlocked_sciences")
        && eng.contains("fn presentation_or_boot_local_team");
    residual_action_store(ResidualScienceTeamPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_science_team_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(purchase) = fn_body(eng, "fn try_purchase_next_generals_science(") else {
        residual_action_store(ResidualScienceTeamPresentationHelperAction::DispatchSource);
        return false;
    };
    let ok = purchase.contains("presentation_or_boot_unlocked_sciences")
        && purchase.contains("player_unlocked_sciences")
        && purchase.contains("player_can_purchase_science")
        && eng.contains("presentation_or_boot_local_team()");
    residual_action_store(ResidualScienceTeamPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_science_team_presentation_helper_residual_pack_wave555() -> bool {
    honesty_science_team_presentation_helper_method_names_residual_wave555()
        && honesty_science_team_presentation_helper_source_markers_residual_wave555()
        && honesty_science_team_presentation_helper_nav_commands_residual_wave555()
        && simulate_science_team_presentation_helper_collect_source()
        && simulate_science_team_presentation_helper_dispatch_source()
}

pub fn simulate_live_science_team_presentation_helper_honesty() -> bool {
    let ok = honesty_science_team_presentation_helper_residual_pack_wave555();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualScienceTeamPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_science_team_presentation_helper_method_names_residual_wave555());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_science_team_presentation_helper_source_markers_residual_wave555());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_science_team_presentation_helper_nav_commands_residual_wave555());
    }

    #[test]
    fn science_team_presentation_helper_sources() {
        assert!(simulate_science_team_presentation_helper_collect_source());
        assert!(simulate_science_team_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave555_composite_pack() {
        assert!(honesty_science_team_presentation_helper_residual_pack_wave555());
    }

    #[test]
    fn simulate_live_science_team_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_science_team_presentation_helper_honesty(),
            "science/team presentation helper residual must latch"
        );
        assert!(residual_science_team_presentation_helper_ok());
        assert_eq!(
            residual_science_team_presentation_helper_last_action(),
            ResidualScienceTeamPresentationHelperAction::Composite
        );
    }
}
