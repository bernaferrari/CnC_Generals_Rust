//! Wave 216 residual peels: control-group assign/recall + presentation camera
//! follow use presentation identity (`ui_selected_ids` / `ui_object_alive` /
//! `camera_follow_position` only). No live dual-read residual when freeze
//! installed. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 215 UI helper presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` control_group_* / apply_presentation_camera_residual
//!
//! Fail-closed:
//! - Not full C++ control-group double-tap center parity
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Control-group + camera presentation-only residual method names.
pub const LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE216: &[&str] = &[
    "ui_selected_ids",
    "ui_object_alive",
    "camera_follow_position",
    "control_group_assign",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE216: &[&str] = &[
    "REQUIRE_CONTROL_GROUP_PRESENTATION_ONLY",
    "REQUIRE_CAMERA_FOLLOW_PRESENTATION_ONLY",
    "LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_CMD_NAMES_WAVE216: &[&str] = &[
    "click_live_control_group_camera_presentation_only_ok_prepare",
    "click_live_control_group_camera_presentation_only_ok_live",
    "click_live_control_group_camera_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_control_group_camera_presentation_only_method_names_residual_wave216() -> bool {
    LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE216.len() == 5
        && residual_name_index(
            LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE216,
            "ui_selected_ids",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE216,
            "camera_follow_position",
        ) == Some(2)
        && residual_name_index(
            LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE216,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_control_group_camera_presentation_only_nav_commands_residual_wave216() -> bool {
    LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE216.len() == 4
        && residual_name_index(
            LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE216,
            "REQUIRE_CONTROL_GROUP_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE216,
            "LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CONTROL_GROUP_CAMERA_PRESENTATION_ONLY_CMD_NAMES_WAVE216.len() == 3
}

/// Wave 216 composite residual honesty pack.
pub fn honesty_live_control_group_camera_presentation_only_residual_pack_wave216() -> bool {
    honesty_live_control_group_camera_presentation_only_method_names_residual_wave216()
        && honesty_live_control_group_camera_presentation_only_nav_commands_residual_wave216()
}

/// Source residual: control-group assign uses ui_selected_ids.
pub fn honesty_control_group_assign_presentation_only_source() -> bool {
    let eng = include_str!("../../cnc_game_engine.rs");
    let Some(i) = eng.find("control_group_assign_fail_no_selection") else {
        return false;
    };
    let window = &eng[i.saturating_sub(500)..i];
    window.contains("Wave 216")
        && window.contains("ui_selected_ids")
        && !window.contains("get_player(self.current_player_id)")
}

/// Source residual: control-group recall filters via ui_object_alive.
pub fn honesty_control_group_recall_presentation_only_source() -> bool {
    let eng = include_str!("../../cnc_game_engine.rs");
    let Some(i) = eng.find("control_group_recall_fail_empty") else {
        return false;
    };
    let window = &eng[i.saturating_sub(700)..i];
    window.contains("Wave 216")
        && window.contains("ui_object_alive")
        && !window.contains("get_object(*id)")
}

/// Source residual: presentation camera residual has no live camera_follow dual-read.
pub fn honesty_camera_follow_presentation_only_source() -> bool {
    let eng = include_str!("../../cnc_game_engine.rs");
    let Some(i) = eng.find("fn apply_presentation_camera_residual") else {
        return false;
    };
    let rest = &eng[i..];
    let end = rest.find("\n    fn ").unwrap_or(rest.len().min(1800));
    let body = &rest[..end];
    body.contains("Wave 216")
        && body.contains("camera_follow_position")
        && !body.contains("camera_follow_target_position")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_control_group_camera_presentation_only_honesty() -> bool {
    honesty_live_control_group_camera_presentation_only_residual_pack_wave216()
        && honesty_control_group_assign_presentation_only_source()
        && honesty_control_group_recall_presentation_only_source()
        && honesty_camera_follow_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_control_group_camera_presentation_only_method_names_residual_wave216()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_control_group_camera_presentation_only_nav_commands_residual_wave216()
        );
    }

    #[test]
    fn wave216_composite_pack() {
        assert!(honesty_live_control_group_camera_presentation_only_residual_pack_wave216());
    }

    #[test]
    fn control_group_camera_sources() {
        assert!(honesty_control_group_assign_presentation_only_source());
        assert!(honesty_control_group_recall_presentation_only_source());
        assert!(honesty_camera_follow_presentation_only_source());
    }

    #[test]
    fn simulate_live_control_group_camera_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_control_group_camera_presentation_only_honesty(),
            "control-group/camera presentation-only residual must latch"
        );
    }
}
