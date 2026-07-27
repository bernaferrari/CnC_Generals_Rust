//! Wave 223 residual peels: `bootstrap_camera_for_loaded_map` prefers
//! presentation freeze for world bounds, local team base, camera focus, and
//! height samples (no live get_player/team_base dual-read when a frame is
//! installed). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 222 pick-object presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` bootstrap_camera_for_loaded_map
//!
//! Fail-closed:
//! - Not full C++ W3DView pitch/yaw/zoom matrix parity
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Bootstrap-camera presentation-only residual method names.
pub const LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE223: &[&str] = &[
    "bootstrap_camera_for_loaded_map",
    "local_team_base_position",
    "world_bounds_vec3",
    "sample_startup_camera_heights",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE223: &[&str] = &[
    "REQUIRE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY",
    "REQUIRE_PRESENTATION_ARG",
    "LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_CMD_NAMES_WAVE223: &[&str] = &[
    "click_live_bootstrap_camera_presentation_only_ok_prepare",
    "click_live_bootstrap_camera_presentation_only_ok_live",
    "click_live_bootstrap_camera_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_bootstrap_camera_presentation_only_method_names_residual_wave223() -> bool {
    LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE223.len() == 5
        && residual_name_index(
            LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE223,
            "bootstrap_camera_for_loaded_map",
        ) == Some(0)
        && residual_name_index(
            LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE223,
            "sample_startup_camera_heights",
        ) == Some(3)
        && residual_name_index(
            LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_METHOD_NAMES_WAVE223,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_bootstrap_camera_presentation_only_nav_commands_residual_wave223() -> bool {
    LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE223.len() == 4
        && residual_name_index(
            LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE223,
            "REQUIRE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_NAV_STEPS_WAVE223,
            "LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_BOOTSTRAP_CAMERA_PRESENTATION_ONLY_CMD_NAMES_WAVE223.len() == 3
}

/// Wave 223 composite residual honesty pack.
pub fn honesty_live_bootstrap_camera_presentation_only_residual_pack_wave223() -> bool {
    honesty_live_bootstrap_camera_presentation_only_method_names_residual_wave223()
        && honesty_live_bootstrap_camera_presentation_only_nav_commands_residual_wave223()
}

/// Source residual: bootstrap camera takes presentation and prefers freeze fields.
pub fn honesty_bootstrap_camera_presentation_only_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    let Some(i) = eng.find("fn bootstrap_camera_for_loaded_map(") else {
        return false;
    };
    let rest = &eng[i..];
    let end = rest.find("\n    fn ").unwrap_or(rest.len().min(5500));
    let body = &rest[..end];
    // Wave 458 supersedes Wave 223 call-site dual-read: live GameLogic is Option and
    // call sites prefer pipeline presentation freeze.
    body.contains("presentation: Option<&crate::presentation_frame::PresentationFrame>")
        && body.contains("local_team_base_position")
        && body.contains("world_bounds_vec3")
        && body.contains("sample_startup_camera_heights")
        && body.contains("game_logic: Option<&GameLogic>")
        && body.contains("is_shell_game: bool")
        && eng.contains("startup_camera_presentation")
        && eng.contains("startup_camera_live_logic")
        && eng
            .matches("Wave 458: prefer pipeline presentation freeze")
            .count()
            >= 2
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_bootstrap_camera_presentation_only_honesty() -> bool {
    honesty_live_bootstrap_camera_presentation_only_residual_pack_wave223()
        && honesty_bootstrap_camera_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_bootstrap_camera_presentation_only_method_names_residual_wave223());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_bootstrap_camera_presentation_only_nav_commands_residual_wave223());
    }

    #[test]
    fn wave223_composite_pack() {
        assert!(honesty_live_bootstrap_camera_presentation_only_residual_pack_wave223());
    }

    #[test]
    fn bootstrap_camera_sources() {
        assert!(honesty_bootstrap_camera_presentation_only_source());
    }

    #[test]
    fn simulate_live_bootstrap_camera_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_bootstrap_camera_presentation_only_honesty(),
            "bootstrap-camera presentation-only residual must latch"
        );
    }
}
