//! Wave 226 residual peels: camera-tracking selection, resume-construction
//! selection/team, and CAMERA_RESET focus prefer presentation freeze /
//! `ui_selected_ids` / `local_team_for_ui` (no live get_player dual-read when
//! installed). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 225 path/guard authority API residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` update_camera_tracking_drawable /
//!   resume_selected_construction / reset_camera_view_hotkey
//!
//! Fail-closed:
//! - Not full C++ hotkey matrix parity
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Hotkey selection/camera presentation-only residual method names.
pub const LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE226: &[&str] = &[
    "ui_selected_ids",
    "local_team_for_ui",
    "update_camera_tracking_drawable",
    "resume_selected_construction",
    "reset_camera_view_hotkey",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE226: &[&str] = &[
    "REQUIRE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY",
    "REQUIRE_UI_SELECTED_IDS",
    "LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_CMD_NAMES_WAVE226: &[&str] =
    &[
        "click_live_hotkey_selection_camera_presentation_only_ok_prepare",
        "click_live_hotkey_selection_camera_presentation_only_ok_live",
        "click_live_hotkey_selection_camera_presentation_only_miss",
    ];

/// Honesty: method names residual pack.
pub fn honesty_live_hotkey_selection_camera_presentation_only_method_names_residual_wave226() -> bool
{
    LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE226.len() == 6
        && residual_name_index(
            LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE226,
            "ui_selected_ids",
        ) == Some(0)
        && residual_name_index(
            LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE226,
            "reset_camera_view_hotkey",
        ) == Some(4)
        && residual_name_index(
            LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE226,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_hotkey_selection_camera_presentation_only_nav_commands_residual_wave226() -> bool
{
    LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE226.len() == 4
        && residual_name_index(
            LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE226,
            "REQUIRE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE226,
            "LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_HOTKEY_SELECTION_CAMERA_PRESENTATION_ONLY_CMD_NAMES_WAVE226.len() == 3
}

/// Wave 226 composite residual honesty pack.
pub fn honesty_live_hotkey_selection_camera_presentation_only_residual_pack_wave226() -> bool {
    honesty_live_hotkey_selection_camera_presentation_only_method_names_residual_wave226()
        && honesty_live_hotkey_selection_camera_presentation_only_nav_commands_residual_wave226()
}

/// Source residual: three hotkey helpers presentation-first.
pub fn honesty_hotkey_selection_camera_presentation_only_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    let checks = [
        (
            "fn update_camera_tracking_drawable",
            "Wave 226: selection via presentation-first ui_selected_ids",
            "ui_selected_ids",
        ),
        (
            "fn resume_selected_construction",
            "Wave 226: selection/team via presentation-first helpers",
            "local_team_for_ui",
        ),
        (
            "fn reset_camera_view_hotkey",
            "Wave 226: prefer presentation freeze for reset focus",
            "local_team_base_position",
        ),
    ];
    for (sig, note, token) in checks {
        let Some(i) = eng.find(sig) else {
            return false;
        };
        let body = &eng[i..i + 900.min(eng.len() - i)];
        if !body.contains(note) || !body.contains(token) {
            return false;
        }
    }
    // tracking must not dual-read get_player selected_objects first.
    let Some(i) = eng.find("fn update_camera_tracking_drawable") else {
        return false;
    };
    let body = &eng[i..i + 500.min(eng.len() - i)];
    !body.contains("get_player(self.current_player_id)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_hotkey_selection_camera_presentation_only_honesty() -> bool {
    honesty_live_hotkey_selection_camera_presentation_only_residual_pack_wave226()
        && honesty_hotkey_selection_camera_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_hotkey_selection_camera_presentation_only_method_names_residual_wave226()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_hotkey_selection_camera_presentation_only_nav_commands_residual_wave226()
        );
    }

    #[test]
    fn wave226_composite_pack() {
        assert!(honesty_live_hotkey_selection_camera_presentation_only_residual_pack_wave226());
    }

    #[test]
    fn hotkey_selection_camera_sources() {
        assert!(honesty_hotkey_selection_camera_presentation_only_source());
    }

    #[test]
    fn simulate_live_hotkey_selection_camera_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_hotkey_selection_camera_presentation_only_honesty(),
            "hotkey selection/camera presentation-only residual must latch"
        );
    }
}
