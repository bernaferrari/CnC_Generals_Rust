//! Wave 556 residual peels: match-over / victory residual is centralized through
//! `presentation_or_boot_match_over_label` and `presentation_or_boot_victory_winner`
//! — presentation freeze owns match_over / victory_label / winner when installed
//! (no live `evaluate_victory_condition` dual-read); boot residual without freeze
//! uses host probe. Call sites: host status snapshot, InGame victory screen.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 555 science/team presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` presentation_or_boot_match_over_label /
//!   presentation_or_boot_victory_winner / call sites
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_VICTORY_PRESENTATION_HELPER_METHOD_NAMES_WAVE556: &[&str] = &[
    "presentation_or_boot_match_over_label",
    "presentation_or_boot_victory_winner",
    "evaluate_victory_condition",
    "match_over",
    "Wave 556",
    "playable_claim = false",
];

pub const LIVE_VICTORY_PRESENTATION_HELPER_NAV_STEPS_WAVE556: &[&str] = &[
    "REQUIRE_MATCH_OVER_PRESENTATION_HELPER",
    "REQUIRE_VICTORY_WINNER_PRESENTATION_HELPER",
    "LIVE_VICTORY_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_VICTORY_PRESENTATION_HELPER_CMD_NAMES_WAVE556: &[&str] = &[
    "match_over_presentation_helper",
    "victory_winner_presentation_helper",
    "boot_evaluate_victory_condition",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualVictoryPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualVictoryPresentationHelperAction {
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

fn residual_action_store(action: ResidualVictoryPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_victory_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_victory_presentation_helper_last_action() -> ResidualVictoryPresentationHelperAction
{
    ResidualVictoryPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
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

pub fn honesty_victory_presentation_helper_method_names_residual_wave556() -> bool {
    let names = LIVE_VICTORY_PRESENTATION_HELPER_METHOD_NAMES_WAVE556;
    let ok = residual_name_index(names, "presentation_or_boot_match_over_label").is_some()
        && residual_name_index(names, "presentation_or_boot_victory_winner").is_some()
        && residual_name_index(names, "evaluate_victory_condition").is_some()
        && residual_name_index(names, "match_over").is_some()
        && residual_name_index(names, "Wave 556").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualVictoryPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_victory_presentation_helper_source_markers_residual_wave556() -> bool {
    let eng = eng_source();
    let Some(label) = fn_body(eng, "fn presentation_or_boot_match_over_label(") else {
        residual_action_store(ResidualVictoryPresentationHelperAction::SourceMarkers);
        return false;
    };
    let Some(winner) = fn_body(eng, "fn presentation_or_boot_victory_winner(") else {
        residual_action_store(ResidualVictoryPresentationHelperAction::SourceMarkers);
        return false;
    };
    let label_ok = label.contains("Wave 556")
        && label.contains("pres.match_over")
        && label.contains("evaluate_victory_condition()");
    let winner_ok = winner.contains("Wave 556")
        && winner.contains("PresentationEvent::Victory")
        && winner.contains("evaluate_victory_condition()");
    let calls = eng.contains("presentation_or_boot_match_over_label()")
        && eng.contains("presentation_or_boot_victory_winner()");
    let raw = eng
        .matches("self.game_logic.evaluate_victory_condition()")
        .count();
    let ok = label_ok && winner_ok && calls && raw == 2 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualVictoryPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_victory_presentation_helper_nav_commands_residual_wave556() -> bool {
    let steps = LIVE_VICTORY_PRESENTATION_HELPER_NAV_STEPS_WAVE556;
    let cmds = RUNTIME_HOST_LIVE_VICTORY_PRESENTATION_HELPER_CMD_NAMES_WAVE556;
    let ok = residual_name_index(steps, "REQUIRE_MATCH_OVER_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_VICTORY_WINNER_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_VICTORY_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "match_over_presentation_helper").is_some()
        && residual_name_index(cmds, "victory_winner_presentation_helper").is_some()
        && residual_name_index(cmds, "boot_evaluate_victory_condition").is_some();
    residual_action_store(ResidualVictoryPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_victory_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 556")
        && eng.contains("fn presentation_or_boot_match_over_label")
        && eng.contains("fn presentation_or_boot_victory_winner");
    residual_action_store(ResidualVictoryPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_victory_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("presentation_or_boot_match_over_label()")
        && eng.contains("presentation_or_boot_victory_winner()")
        && eng.contains("show_victory_screen")
        && eng.contains("runtime_host_status_snapshot");
    residual_action_store(ResidualVictoryPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_victory_presentation_helper_residual_pack_wave556() -> bool {
    honesty_victory_presentation_helper_method_names_residual_wave556()
        && honesty_victory_presentation_helper_source_markers_residual_wave556()
        && honesty_victory_presentation_helper_nav_commands_residual_wave556()
        && simulate_victory_presentation_helper_collect_source()
        && simulate_victory_presentation_helper_dispatch_source()
}

pub fn simulate_live_victory_presentation_helper_honesty() -> bool {
    let ok = honesty_victory_presentation_helper_residual_pack_wave556();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualVictoryPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_victory_presentation_helper_method_names_residual_wave556());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_victory_presentation_helper_source_markers_residual_wave556());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_victory_presentation_helper_nav_commands_residual_wave556());
    }

    #[test]
    fn victory_presentation_helper_sources() {
        assert!(simulate_victory_presentation_helper_collect_source());
        assert!(simulate_victory_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave556_composite_pack() {
        assert!(honesty_victory_presentation_helper_residual_pack_wave556());
    }

    #[test]
    fn simulate_live_victory_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_victory_presentation_helper_honesty(),
            "victory presentation helper residual must latch"
        );
        assert!(residual_victory_presentation_helper_ok());
        assert_eq!(
            residual_victory_presentation_helper_last_action(),
            ResidualVictoryPresentationHelperAction::Composite
        );
    }
}
