//! Wave 142 residual peels: Beacon display residual
//! (place/remove/text/drain; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 133 ControlBar beacon buttons.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - GameClient beacon place/remove/text notifications
//! - ControlBar beacon helpers
//!
//! Fail-closed:
//! - Not full multiplayer beacon command stream residual
//! - Not full EVA/radar marker render residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Beacon residual tables
// ---------------------------------------------------------------------------

/// Retail ControlBar beacon button names residual.
pub const BEACON_CONTROL_NAMES_WAVE142: &[&str] = &[
    "ControlBar.wnd:ButtonPlaceBeacon",
    "ControlBar.wnd:ButtonDeleteBeacon",
    "ControlBar.wnd:ButtonClearBeaconText",
    "ControlBar.wnd:EditBeaconText",
];

/// Beacon match threshold residual (world units).
pub const BEACON_MATCH_THRESHOLD_WAVE142: f32 = 3.0;

/// Ordered Beacon residual navigation steps.
pub const BEACON_NAV_STEPS_WAVE142: &[&str] = &[
    "GBM_SELECTED_BUTTON_PLACE_BEACON",
    "SET_PENDING_PLACE_BEACON",
    "CLICK_WORLD_POSITION",
    "RECORD_BEACON_PLACED",
    "EDIT_BEACON_TEXT",
    "RECORD_BEACON_TEXT",
    "GBM_SELECTED_BUTTON_DELETE_BEACON",
    "RECORD_BEACON_REMOVED",
    "DRAIN_UI_NOTIFICATIONS",
];

/// Runtime-host command residual names for Beacon peels.
pub const RUNTIME_HOST_BEACON_CMD_NAMES_WAVE142: &[&str] = &[
    "click_beacon_ok_wnd_place",
    "click_beacon_ok_wnd_remove",
    "click_beacon_ok_wnd_text",
    "click_beacon_ok_wnd_drain",
    "click_beacon_ok_wnd_prepare",
    "click_beacon_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: Beacon control names + threshold residual pack.
pub fn honesty_beacon_control_names_residual_wave142() -> bool {
    BEACON_MATCH_THRESHOLD_WAVE142 == 3.0
        && BEACON_CONTROL_NAMES_WAVE142.len() == 4
        && residual_name_index(
            BEACON_CONTROL_NAMES_WAVE142,
            "ControlBar.wnd:ButtonPlaceBeacon",
        ) == Some(0)
        && residual_name_index(
            BEACON_CONTROL_NAMES_WAVE142,
            "ControlBar.wnd:ButtonDeleteBeacon",
        ) == Some(1)
        && BEACON_CONTROL_NAMES_WAVE142
            .iter()
            .all(|n| n.starts_with("ControlBar.wnd:"))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_beacon_nav_commands_residual_wave142() -> bool {
    BEACON_NAV_STEPS_WAVE142.len() == 9
        && residual_name_index(BEACON_NAV_STEPS_WAVE142, "RECORD_BEACON_PLACED") == Some(3)
        && residual_name_index(BEACON_NAV_STEPS_WAVE142, "RECORD_BEACON_REMOVED") == Some(7)
        && residual_name_index(BEACON_NAV_STEPS_WAVE142, "DRAIN_UI_NOTIFICATIONS") == Some(8)
        && RUNTIME_HOST_BEACON_CMD_NAMES_WAVE142.len() == 6
        && residual_name_index(
            RUNTIME_HOST_BEACON_CMD_NAMES_WAVE142,
            "click_beacon_ok_wnd_prepare",
        ) == Some(4)
}

/// Wave 142 composite residual honesty pack.
pub fn honesty_beacon_residual_pack_wave142() -> bool {
    honesty_beacon_control_names_residual_wave142()
        && honesty_beacon_nav_commands_residual_wave142()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_names_residual() {
        assert!(honesty_beacon_control_names_residual_wave142());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_beacon_nav_commands_residual_wave142());
    }

    #[test]
    fn wave142_composite_pack() {
        assert!(honesty_beacon_residual_pack_wave142());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_beacon_prepare_place_residual_live() {
        use game_client::system::{
            ResidualBeaconAction, residual_beacon_last_action, residual_beacon_marker_count,
            simulate_beacon_prepare_place_with_text, simulate_beacon_remove,
        };
        assert!(
            simulate_beacon_prepare_place_with_text(1, 10.0, 20.0, 0.0, "here"),
            "place+text residual must latch"
        );
        assert!(residual_beacon_marker_count() >= 1);
        assert_eq!(residual_beacon_last_action(), ResidualBeaconAction::Text);
        assert!(simulate_beacon_remove(1, 10.0, 20.0, 0.0));
        assert_eq!(residual_beacon_last_action(), ResidualBeaconAction::Remove);
    }
}
