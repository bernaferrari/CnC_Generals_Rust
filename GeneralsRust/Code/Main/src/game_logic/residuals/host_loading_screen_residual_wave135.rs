//! Wave 135 residual peels: LoadingScreen residual
//! (show/hide/progress/map/stages; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 132 MapSelect start, skirmish start residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - GameClient loading screen progress / stage labels
//! - Map load progress notifications
//!
//! Fail-closed:
//! - Not full W3D/asset decode residual
//! - Not full tip rotation / faction logo residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Loading screen residual tables
// ---------------------------------------------------------------------------

/// Retail loading stage id residual order.
pub const LOADING_SCREEN_STAGE_IDS_WAVE135: &[&str] =
    &["init", "map", "textures", "models", "assets", "finalize"];

/// Ordered LoadingScreen residual navigation steps.
pub const LOADING_SCREEN_NAV_STEPS_WAVE135: &[&str] = &[
    "SHOW_LOADING_SCREEN",
    "SET_MAP_NAME",
    "STAGE_INIT",
    "STAGE_MAP",
    "STAGE_TEXTURES",
    "STAGE_MODELS",
    "STAGE_ASSETS",
    "STAGE_FINALIZING",
    "SET_PROGRESS_100",
    "HIDE_LOADING_SCREEN",
];

/// Runtime-host command residual names for LoadingScreen peels.
pub const RUNTIME_HOST_LOADING_SCREEN_CMD_NAMES_WAVE135: &[&str] = &[
    "show_loading_screen_ok_wnd",
    "show_loading_screen_miss",
    "click_loading_screen_ok_wnd_progress",
    "click_loading_screen_ok_wnd_map",
    "click_loading_screen_ok_wnd_next",
    "click_loading_screen_ok_wnd_finish",
    "click_loading_screen_ok_wnd_prepare",
    "click_loading_screen_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: LoadingScreen stage ids residual pack.
pub fn honesty_loading_screen_stages_residual_wave135() -> bool {
    LOADING_SCREEN_STAGE_IDS_WAVE135.len() == 6
        && residual_name_index(LOADING_SCREEN_STAGE_IDS_WAVE135, "init") == Some(0)
        && residual_name_index(LOADING_SCREEN_STAGE_IDS_WAVE135, "map") == Some(1)
        && residual_name_index(LOADING_SCREEN_STAGE_IDS_WAVE135, "finalize") == Some(5)
        && LOADING_SCREEN_STAGE_IDS_WAVE135
            .iter()
            .all(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase()))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_loading_screen_nav_commands_residual_wave135() -> bool {
    LOADING_SCREEN_NAV_STEPS_WAVE135.len() == 10
        && residual_name_index(LOADING_SCREEN_NAV_STEPS_WAVE135, "SHOW_LOADING_SCREEN") == Some(0)
        && residual_name_index(LOADING_SCREEN_NAV_STEPS_WAVE135, "SET_PROGRESS_100") == Some(8)
        && residual_name_index(LOADING_SCREEN_NAV_STEPS_WAVE135, "HIDE_LOADING_SCREEN") == Some(9)
        && RUNTIME_HOST_LOADING_SCREEN_CMD_NAMES_WAVE135.len() == 8
        && residual_name_index(
            RUNTIME_HOST_LOADING_SCREEN_CMD_NAMES_WAVE135,
            "show_loading_screen_ok_wnd",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_LOADING_SCREEN_CMD_NAMES_WAVE135,
            "click_loading_screen_ok_wnd_finish",
        ) == Some(5)
}

/// Wave 135 composite residual honesty pack.
pub fn honesty_loading_screen_residual_pack_wave135() -> bool {
    honesty_loading_screen_stages_residual_wave135()
        && honesty_loading_screen_nav_commands_residual_wave135()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stages_residual() {
        assert!(honesty_loading_screen_stages_residual_wave135());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_loading_screen_nav_commands_residual_wave135());
    }

    #[test]
    fn wave135_composite_pack() {
        assert!(honesty_loading_screen_residual_pack_wave135());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_loading_prepare_map_load_residual_live() {
        use game_client::gui::loading_screen::{
            ResidualLoadingScreenAction, residual_loading_screen_is_visible,
            residual_loading_screen_last_action, residual_loading_screen_map_name,
            residual_loading_screen_progress, simulate_loading_screen_finish,
            simulate_loading_screen_prepare_map_load,
        };
        assert!(
            simulate_loading_screen_prepare_map_load("Maps/Test/Test.map", 40),
            "show+map+progress residual must latch"
        );
        assert!(residual_loading_screen_is_visible());
        assert_eq!(
            residual_loading_screen_map_name().as_deref(),
            Some("Maps/Test/Test.map")
        );
        assert_eq!(residual_loading_screen_progress(), 40);
        assert_eq!(
            residual_loading_screen_last_action(),
            ResidualLoadingScreenAction::SetProgress
        );
        assert!(simulate_loading_screen_finish());
        assert!(!residual_loading_screen_is_visible());
        assert_eq!(residual_loading_screen_progress(), 100);
    }
}
