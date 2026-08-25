//! Wave 152 residual peels: ChallengeGenerals residual
//! (init/starts/bio/difficulty/template; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 151 Credits roll residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - ChallengeGenerals.h NUM_GENERALS = 12
//! - GameDifficulty Easy/Normal/Hard
//!
//! Fail-closed:
//! - Not full ChallengeMode.ini load residual
//! - Not full challenge menu WND residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// ChallengeGenerals residual tables
// ---------------------------------------------------------------------------

/// Retail challenge general count residual.
pub const CHALLENGE_NUM_GENERALS_WAVE152: usize = 12;

/// GameDifficulty residual names.
pub const CHALLENGE_DIFFICULTY_NAMES_WAVE152: &[&str] = &["Easy", "Normal", "Hard"];

/// ChallengeGenerals method residual names.
pub const CHALLENGE_GENERALS_METHOD_NAMES_WAVE152: &[&str] = &[
    "init",
    "setStartsEnabled",
    "setBioName",
    "setCurrentDifficulty",
    "setCurrentPlayerTemplateNum",
    "getGeneralByGeneralName",
];

/// Ordered ChallengeGenerals residual navigation steps.
pub const CHALLENGE_GENERALS_NAV_STEPS_WAVE152: &[&str] = &[
    "INIT_CHALLENGE_GENERALS",
    "ENABLE_PERSONA_STARTS",
    "SET_BIO_NAME",
    "SET_DIFFICULTY_HARD",
    "SELECT_PLAYER_TEMPLATE",
    "OPEN_CHALLENGE_MENU",
];

/// Runtime-host command residual names for ChallengeGenerals peels.
pub const RUNTIME_HOST_CHALLENGE_GENERALS_CMD_NAMES_WAVE152: &[&str] = &[
    "click_challenge_generals_ok_wnd_init",
    "click_challenge_generals_ok_wnd_starts",
    "click_challenge_generals_ok_wnd_bio",
    "click_challenge_generals_ok_wnd_difficulty",
    "click_challenge_generals_ok_wnd_template",
    "click_challenge_generals_ok_wnd_prepare",
    "click_challenge_generals_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: ChallengeGenerals constants + method names residual pack.
pub fn honesty_challenge_generals_method_names_residual_wave152() -> bool {
    CHALLENGE_NUM_GENERALS_WAVE152 == 12
        && CHALLENGE_DIFFICULTY_NAMES_WAVE152.len() == 3
        && residual_name_index(CHALLENGE_DIFFICULTY_NAMES_WAVE152, "Easy") == Some(0)
        && residual_name_index(CHALLENGE_DIFFICULTY_NAMES_WAVE152, "Hard") == Some(2)
        && CHALLENGE_GENERALS_METHOD_NAMES_WAVE152.len() == 6
        && residual_name_index(CHALLENGE_GENERALS_METHOD_NAMES_WAVE152, "setBioName") == Some(2)
        && residual_name_index(
            CHALLENGE_GENERALS_METHOD_NAMES_WAVE152,
            "setCurrentDifficulty",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_challenge_generals_nav_commands_residual_wave152() -> bool {
    CHALLENGE_GENERALS_NAV_STEPS_WAVE152.len() == 6
        && residual_name_index(
            CHALLENGE_GENERALS_NAV_STEPS_WAVE152,
            "ENABLE_PERSONA_STARTS",
        ) == Some(1)
        && residual_name_index(CHALLENGE_GENERALS_NAV_STEPS_WAVE152, "SET_DIFFICULTY_HARD")
            == Some(3)
        && residual_name_index(CHALLENGE_GENERALS_NAV_STEPS_WAVE152, "OPEN_CHALLENGE_MENU")
            == Some(5)
        && RUNTIME_HOST_CHALLENGE_GENERALS_CMD_NAMES_WAVE152.len() == 7
        && residual_name_index(
            RUNTIME_HOST_CHALLENGE_GENERALS_CMD_NAMES_WAVE152,
            "click_challenge_generals_ok_wnd_prepare",
        ) == Some(5)
}

/// Wave 152 composite residual honesty pack.
pub fn honesty_challenge_generals_residual_pack_wave152() -> bool {
    honesty_challenge_generals_method_names_residual_wave152()
        && honesty_challenge_generals_nav_commands_residual_wave152()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_challenge_generals_method_names_residual_wave152());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_challenge_generals_nav_commands_residual_wave152());
    }

    #[test]
    fn wave152_composite_pack() {
        assert!(honesty_challenge_generals_residual_pack_wave152());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_challenge_generals_prepare_default_residual_live() {
        use game_client::gui::{
            NUM_GENERALS, ResidualChallengeGeneralsAction,
            residual_challenge_generals_bio_name_len, residual_challenge_generals_difficulty,
            residual_challenge_generals_last_action, residual_challenge_generals_starts_enabled,
            residual_challenge_generals_template_num, simulate_challenge_generals_prepare_default,
        };
        assert_eq!(NUM_GENERALS, CHALLENGE_NUM_GENERALS_WAVE152);
        assert!(
            simulate_challenge_generals_prepare_default(),
            "init+starts+bio+hard+template residual must latch"
        );
        assert!(residual_challenge_generals_starts_enabled());
        assert!(residual_challenge_generals_bio_name_len() > 0);
        assert_eq!(residual_challenge_generals_difficulty(), 2);
        assert_eq!(residual_challenge_generals_template_num(), 0);
        assert_eq!(
            residual_challenge_generals_last_action(),
            ResidualChallengeGeneralsAction::SelectTemplate
        );
    }
}
