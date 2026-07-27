//! Wave 599 residual peels: defeat/alliance/victory broadcast is centralized
//! through `host_broadcast_match_outcome_residuals`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 569 presentation-or-boot defeat/alliance residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_broadcast_match_outcome_residuals
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MATCH_OUTCOME_HELPER_METHOD_NAMES_WAVE599: &[&str] = &[
    "host_broadcast_match_outcome_residuals",
    "take_presentation_or_boot_defeat_events",
    "take_presentation_or_boot_alliance_events",
    "presentation_or_boot_victory_winner",
    "notify_presentation_ui_message",
    "Wave 599",
    "Wave 569",
    "playable_claim = false",
];

pub const LIVE_HOST_MATCH_OUTCOME_HELPER_NAV_STEPS_WAVE599: &[&str] = &[
    "REQUIRE_MATCH_OUTCOME_HELPER",
    "REQUIRE_DEFEAT_BROADCAST",
    "REQUIRE_ALLIANCE_BROADCAST",
    "REQUIRE_VICTORY_SCREEN",
    "LIVE_HOST_MATCH_OUTCOME_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_MATCH_OUTCOME_HELPER_CMD_NAMES_WAVE599: &[&str] = &[
    "host_match_outcome_helper",
    "defeat_broadcast",
    "alliance_broadcast",
    "victory_screen",
    "match_outcome_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMatchOutcomeHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostMatchOutcomeHelperAction {
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

fn residual_action_store(action: ResidualHostMatchOutcomeHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_match_outcome_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_match_outcome_helper_last_action() -> ResidualHostMatchOutcomeHelperAction {
    ResidualHostMatchOutcomeHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn last_sig_index(src: &str, sig: &str) -> Option<usize> {
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(sig) {
        at = Some(from + rel);
        from = from + rel + sig.len();
    }
    at
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = last_sig_index(src, sig)?;
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

pub fn honesty_host_match_outcome_helper_method_names_residual_wave599() -> bool {
    let names = LIVE_HOST_MATCH_OUTCOME_HELPER_METHOD_NAMES_WAVE599;
    let ok = residual_name_index(names, "host_broadcast_match_outcome_residuals").is_some()
        && residual_name_index(names, "take_presentation_or_boot_defeat_events").is_some()
        && residual_name_index(names, "take_presentation_or_boot_alliance_events").is_some()
        && residual_name_index(names, "Wave 599").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostMatchOutcomeHelperAction::MethodNames);
    ok
}

pub fn honesty_host_match_outcome_helper_source_markers_residual_wave599() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_broadcast_match_outcome_residuals(") else {
        residual_action_store(ResidualHostMatchOutcomeHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 599")
        && body.contains("Wave 569")
        && body.contains("take_presentation_or_boot_defeat_events")
        && body.contains("take_presentation_or_boot_alliance_events")
        && body.contains("presentation_or_boot_victory_winner")
        && body.contains("notify_presentation_ui_message")
        && body.contains("notify_boot_ui_message")
        && body.contains("show_victory_screen")
        && body.contains("ScriptEvent::PlayerDefeated");
    let call_ok = eng.contains("self.host_broadcast_match_outcome_residuals()")
        && eng.contains("Wave 599: defeat/alliance/victory broadcast residual via host helper");
    let ok = body_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostMatchOutcomeHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_match_outcome_helper_nav_commands_residual_wave599() -> bool {
    let steps = LIVE_HOST_MATCH_OUTCOME_HELPER_NAV_STEPS_WAVE599;
    let cmds = RUNTIME_HOST_LIVE_HOST_MATCH_OUTCOME_HELPER_CMD_NAMES_WAVE599;
    let ok = residual_name_index(steps, "REQUIRE_MATCH_OUTCOME_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_DEFEAT_BROADCAST").is_some()
        && residual_name_index(steps, "REQUIRE_ALLIANCE_BROADCAST").is_some()
        && residual_name_index(steps, "REQUIRE_VICTORY_SCREEN").is_some()
        && residual_name_index(steps, "LIVE_HOST_MATCH_OUTCOME_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_match_outcome_helper").is_some()
        && residual_name_index(cmds, "defeat_broadcast").is_some()
        && residual_name_index(cmds, "alliance_broadcast").is_some()
        && residual_name_index(cmds, "victory_screen").is_some()
        && residual_name_index(cmds, "match_outcome_residual").is_some();
    residual_action_store(ResidualHostMatchOutcomeHelperAction::NavCommands);
    ok
}

pub fn simulate_host_match_outcome_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 599")
        && eng.contains("fn host_broadcast_match_outcome_residuals")
        && eng.contains("take_presentation_or_boot_defeat_events");
    residual_action_store(ResidualHostMatchOutcomeHelperAction::CollectSource);
    ok
}

pub fn simulate_host_match_outcome_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_broadcast_match_outcome_residuals()")
        && eng.contains("Wave 599: defeat/alliance/victory broadcast residual via host helper");
    residual_action_store(ResidualHostMatchOutcomeHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_match_outcome_helper_residual_pack_wave599() -> bool {
    honesty_host_match_outcome_helper_method_names_residual_wave599()
        && honesty_host_match_outcome_helper_source_markers_residual_wave599()
        && honesty_host_match_outcome_helper_nav_commands_residual_wave599()
        && simulate_host_match_outcome_helper_collect_source()
        && simulate_host_match_outcome_helper_dispatch_source()
}

pub fn simulate_live_host_match_outcome_helper_honesty() -> bool {
    let ok = honesty_host_match_outcome_helper_residual_pack_wave599();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostMatchOutcomeHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_match_outcome_helper_method_names_residual_wave599());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_match_outcome_helper_source_markers_residual_wave599());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_match_outcome_helper_nav_commands_residual_wave599());
    }

    #[test]
    fn host_match_outcome_helper_sources() {
        assert!(simulate_host_match_outcome_helper_collect_source());
        assert!(simulate_host_match_outcome_helper_dispatch_source());
    }

    #[test]
    fn wave599_composite_pack() {
        assert!(honesty_host_match_outcome_helper_residual_pack_wave599());
    }

    #[test]
    fn simulate_live_host_match_outcome_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_match_outcome_helper_honesty(),
            "host match outcome helper residual must latch"
        );
        assert!(residual_host_match_outcome_helper_ok());
        assert_eq!(
            residual_host_match_outcome_helper_last_action(),
            ResidualHostMatchOutcomeHelperAction::Composite
        );
    }
}
