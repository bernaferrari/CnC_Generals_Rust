//! Wave 145 residual peels: Smudge terrain-decal residual
//! (add set/smudge/remove/reset; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 144 IME residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - System/Smudge.cpp SmudgeManager / SmudgeSet
//!
//! Fail-closed:
//! - Not full terrain smudge GPU render residual
//! - Not full hardware support probe residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Smudge residual tables
// ---------------------------------------------------------------------------

/// SmudgeManager residual method names.
pub const SMUDGE_METHOD_NAMES_WAVE145: &[&str] = &[
    "addSmudgeSet",
    "removeSmudgeSet",
    "addSmudgeToSet",
    "removeSmudgeFromSet",
    "setSmudgeCountLastFrame",
    "reset",
];

/// Ordered Smudge residual navigation steps.
pub const SMUDGE_NAV_STEPS_WAVE145: &[&str] = &[
    "ADD_SMUDGE_SET",
    "ADD_SMUDGE_TO_SET",
    "SET_SIZE_OPACITY",
    "SET_COUNT_LAST_FRAME",
    "REMOVE_SMUDGE_FROM_SET",
    "REMOVE_SMUDGE_SET",
    "RESET_SMUDGE_MANAGER",
];

/// Runtime-host command residual names for Smudge peels.
pub const RUNTIME_HOST_SMUDGE_CMD_NAMES_WAVE145: &[&str] = &[
    "click_smudge_ok_wnd_add_set",
    "click_smudge_ok_wnd_add",
    "click_smudge_ok_wnd_remove",
    "click_smudge_ok_wnd_remove_set",
    "click_smudge_ok_wnd_reset",
    "click_smudge_ok_wnd_count",
    "click_smudge_ok_wnd_prepare",
    "click_smudge_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: Smudge method names residual pack.
pub fn honesty_smudge_method_names_residual_wave145() -> bool {
    SMUDGE_METHOD_NAMES_WAVE145.len() == 6
        && residual_name_index(SMUDGE_METHOD_NAMES_WAVE145, "addSmudgeSet") == Some(0)
        && residual_name_index(SMUDGE_METHOD_NAMES_WAVE145, "addSmudgeToSet") == Some(2)
        && residual_name_index(SMUDGE_METHOD_NAMES_WAVE145, "reset") == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_smudge_nav_commands_residual_wave145() -> bool {
    SMUDGE_NAV_STEPS_WAVE145.len() == 7
        && residual_name_index(SMUDGE_NAV_STEPS_WAVE145, "ADD_SMUDGE_TO_SET") == Some(1)
        && residual_name_index(SMUDGE_NAV_STEPS_WAVE145, "RESET_SMUDGE_MANAGER") == Some(6)
        && RUNTIME_HOST_SMUDGE_CMD_NAMES_WAVE145.len() == 8
        && residual_name_index(
            RUNTIME_HOST_SMUDGE_CMD_NAMES_WAVE145,
            "click_smudge_ok_wnd_prepare",
        ) == Some(6)
}

/// Wave 145 composite residual honesty pack.
pub fn honesty_smudge_residual_pack_wave145() -> bool {
    honesty_smudge_method_names_residual_wave145() && honesty_smudge_nav_commands_residual_wave145()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_smudge_method_names_residual_wave145());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_smudge_nav_commands_residual_wave145());
    }

    #[test]
    fn wave145_composite_pack() {
        assert!(honesty_smudge_residual_pack_wave145());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_smudge_prepare_set_residual_live() {
        use game_client::system::{
            ResidualSmudgeAction, residual_smudge_count, residual_smudge_last_action,
            residual_smudge_set_count, simulate_smudge_prepare_set_with_smudge,
            simulate_smudge_remove_first, simulate_smudge_reset,
        };
        assert!(
            simulate_smudge_prepare_set_with_smudge(12.0),
            "add set+smudge residual must latch"
        );
        assert_eq!(residual_smudge_set_count(), 1);
        assert!(residual_smudge_count() >= 1);
        assert_eq!(
            residual_smudge_last_action(),
            ResidualSmudgeAction::AddSmudge
        );
        assert!(simulate_smudge_remove_first());
        assert_eq!(
            residual_smudge_last_action(),
            ResidualSmudgeAction::RemoveSmudge
        );
        assert!(simulate_smudge_reset());
        assert_eq!(residual_smudge_set_count(), 0);
        assert_eq!(residual_smudge_count(), 0);
        assert_eq!(residual_smudge_last_action(), ResidualSmudgeAction::Reset);
    }
}
