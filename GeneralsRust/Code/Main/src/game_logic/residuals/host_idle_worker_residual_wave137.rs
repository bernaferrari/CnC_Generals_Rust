//! Wave 137 residual peels: IdleWorker residual
//! (count/select-next; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 133 ControlBar ButtonIdleWorker, Wave 136 chat.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - IdleWorker.cpp / IdleWorker.wnd
//! - ButtonSelectNextIdleWorker
//!
//! Fail-closed:
//! - Not full object selection / camera snap residual
//! - Not full idle worker list rebuild residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Idle worker residual tables
// ---------------------------------------------------------------------------

/// Retail IdleWorker layout filename residual.
pub const IDLE_WORKER_LAYOUT_FILENAME_WAVE137: &str = "IdleWorker.wnd";

/// Retail IdleWorker control names residual.
pub const IDLE_WORKER_CONTROL_NAMES_WAVE137: &[&str] =
    &["IdleWorker.wnd:ButtonSelectNextIdleWorker"];

/// Ordered IdleWorker residual navigation steps.
pub const IDLE_WORKER_NAV_STEPS_WAVE137: &[&str] = &[
    "UPDATE_IDLE_WORKER_COUNT",
    "GBM_SELECTED_BUTTON_SELECT_NEXT",
    "CYCLE_NEXT_INDEX",
    "SELECT_OBJECT",
    "CAMERA_SNAP",
];

/// Runtime-host command residual names for IdleWorker peels.
pub const RUNTIME_HOST_IDLE_WORKER_CMD_NAMES_WAVE137: &[&str] = &[
    "click_idle_worker_ok_wnd_select",
    "click_idle_worker_ok_wnd_count",
    "click_idle_worker_ok_wnd_next",
    "click_idle_worker_ok_wnd_prepare",
    "click_idle_worker_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: IdleWorker control names residual pack.
pub fn honesty_idle_worker_control_names_residual_wave137() -> bool {
    IDLE_WORKER_LAYOUT_FILENAME_WAVE137 == "IdleWorker.wnd"
        && IDLE_WORKER_CONTROL_NAMES_WAVE137.len() == 1
        && residual_name_index(
            IDLE_WORKER_CONTROL_NAMES_WAVE137,
            "IdleWorker.wnd:ButtonSelectNextIdleWorker",
        ) == Some(0)
        && IDLE_WORKER_CONTROL_NAMES_WAVE137
            .iter()
            .all(|n| n.starts_with("IdleWorker.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_idle_worker_nav_commands_residual_wave137() -> bool {
    IDLE_WORKER_NAV_STEPS_WAVE137.len() == 5
        && residual_name_index(
            IDLE_WORKER_NAV_STEPS_WAVE137,
            "GBM_SELECTED_BUTTON_SELECT_NEXT",
        ) == Some(1)
        && residual_name_index(IDLE_WORKER_NAV_STEPS_WAVE137, "CAMERA_SNAP") == Some(4)
        && RUNTIME_HOST_IDLE_WORKER_CMD_NAMES_WAVE137.len() == 5
        && residual_name_index(
            RUNTIME_HOST_IDLE_WORKER_CMD_NAMES_WAVE137,
            "click_idle_worker_ok_wnd_prepare",
        ) == Some(3)
}

/// Wave 137 composite residual honesty pack.
pub fn honesty_idle_worker_residual_pack_wave137() -> bool {
    honesty_idle_worker_control_names_residual_wave137()
        && honesty_idle_worker_nav_commands_residual_wave137()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_idle_worker_control_names_residual_wave137());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_idle_worker_nav_commands_residual_wave137());
    }

    #[test]
    fn wave137_composite_pack() {
        assert!(honesty_idle_worker_residual_pack_wave137());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_idle_prepare_select_residual_live() {
        use game_client::gui::callbacks::{
            ResidualIdleWorkerAction, residual_idle_worker_count, residual_idle_worker_last_action,
            residual_idle_worker_next_index, simulate_idle_worker_prepare_select,
        };
        assert!(
            simulate_idle_worker_prepare_select(3),
            "count+button residual must latch"
        );
        assert_eq!(residual_idle_worker_count(), 3);
        assert_eq!(residual_idle_worker_next_index(), 1);
        assert_eq!(
            residual_idle_worker_last_action(),
            ResidualIdleWorkerAction::Button
        );
    }
}
