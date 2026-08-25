//! Wave 144 residual peels: IME manager residual
//! (enable/composition/candidates/result; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 143 EVA residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - IMEManager enable/disable/service messages
//! - Candidate list + composition cycle
//!
//! Fail-closed:
//! - Not full OS IME backend residual
//! - Not full IME candidate WND draw residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// IME residual tables
// ---------------------------------------------------------------------------

/// IME message residual variants (ImeMessage).
pub const IME_MESSAGE_NAMES_WAVE144: &[&str] = &[
    "StartComposition",
    "EndComposition",
    "UpdateComposition",
    "ResultString",
    "CandidateList",
    "ClearCandidateList",
];

/// Ordered IME residual navigation steps.
pub const IME_NAV_STEPS_WAVE144: &[&str] = &[
    "IME_ENABLE",
    "START_COMPOSITION",
    "UPDATE_COMPOSITION_TEXT",
    "OPEN_CANDIDATE_LIST",
    "SELECT_CANDIDATE",
    "RESULT_STRING",
    "CLEAR_CANDIDATE_LIST",
    "END_COMPOSITION",
    "IME_DISABLE",
    "IME_RESET",
];

/// Runtime-host command residual names for IME peels.
pub const RUNTIME_HOST_IME_CMD_NAMES_WAVE144: &[&str] = &[
    "click_ime_ok_wnd_enable",
    "click_ime_ok_wnd_disable",
    "click_ime_ok_wnd_start",
    "click_ime_ok_wnd_update",
    "click_ime_ok_wnd_result",
    "click_ime_ok_wnd_clear",
    "click_ime_ok_wnd_end",
    "click_ime_ok_wnd_reset",
    "click_ime_ok_wnd_prepare",
    "click_ime_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: IME message names residual pack.
pub fn honesty_ime_message_names_residual_wave144() -> bool {
    IME_MESSAGE_NAMES_WAVE144.len() == 6
        && residual_name_index(IME_MESSAGE_NAMES_WAVE144, "StartComposition") == Some(0)
        && residual_name_index(IME_MESSAGE_NAMES_WAVE144, "CandidateList") == Some(4)
        && residual_name_index(IME_MESSAGE_NAMES_WAVE144, "ClearCandidateList") == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_ime_nav_commands_residual_wave144() -> bool {
    IME_NAV_STEPS_WAVE144.len() == 10
        && residual_name_index(IME_NAV_STEPS_WAVE144, "UPDATE_COMPOSITION_TEXT") == Some(2)
        && residual_name_index(IME_NAV_STEPS_WAVE144, "IME_RESET") == Some(9)
        && RUNTIME_HOST_IME_CMD_NAMES_WAVE144.len() == 10
        && residual_name_index(
            RUNTIME_HOST_IME_CMD_NAMES_WAVE144,
            "click_ime_ok_wnd_prepare",
        ) == Some(8)
}

/// Wave 144 composite residual honesty pack.
pub fn honesty_ime_residual_pack_wave144() -> bool {
    honesty_ime_message_names_residual_wave144() && honesty_ime_nav_commands_residual_wave144()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_names_residual() {
        assert!(honesty_ime_message_names_residual_wave144());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_ime_nav_commands_residual_wave144());
    }

    #[test]
    fn wave144_composite_pack() {
        assert!(honesty_ime_residual_pack_wave144());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_ime_prepare_composition_cycle_residual_live() {
        use game_client::gui::{
            ResidualImeAction, residual_ime_candidate_count, residual_ime_is_composing,
            residual_ime_is_enabled, residual_ime_last_action, simulate_ime_clear_candidates,
            simulate_ime_prepare_composition_cycle,
        };
        assert!(
            simulate_ime_prepare_composition_cycle("nihao"),
            "enable+composition+candidates residual must latch"
        );
        assert!(residual_ime_is_enabled());
        assert!(residual_ime_is_composing());
        assert_eq!(residual_ime_candidate_count(), 3);
        assert_eq!(residual_ime_last_action(), ResidualImeAction::CandidateList);
        assert!(simulate_ime_clear_candidates());
        assert_eq!(residual_ime_candidate_count(), 0);
        assert_eq!(
            residual_ime_last_action(),
            ResidualImeAction::ClearCandidates
        );
    }
}
