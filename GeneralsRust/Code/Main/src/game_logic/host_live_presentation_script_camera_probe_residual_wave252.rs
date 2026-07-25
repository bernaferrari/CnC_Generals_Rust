//! Wave 252 residual peels: script default camera height/pitch prefer presentation
//! freeze via `ui_script_default_camera_*` helpers instead of dual-reading live
//! `GameLogic` when a frame is installed. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 251 presentation visual-speed probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` script_default_camera_max_height / pitch
//! - `cnc_game_engine.rs` ui_script_default_camera_* consumers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Boot residual still falls back to live GameLogic when no frame

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Presentation script-camera probe residual method names.
pub const LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_METHOD_NAMES_WAVE252: &[&str] = &[
    "script_default_camera_max_height",
    "script_default_camera_pitch",
    "ui_script_default_camera_max_height",
    "ui_script_default_camera_pitch",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_NAV_STEPS_WAVE252: &[&str] = &[
    "REQUIRE_PRESENTATION_SCRIPT_CAMERA_FIELDS",
    "REQUIRE_UI_CAMERA_HELPERS",
    "LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_CMD_NAMES_WAVE252: &[&str] = &[
    "click_live_presentation_script_camera_probe_ok_prepare",
    "click_live_presentation_script_camera_probe_ok_live",
    "click_live_presentation_script_camera_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_presentation_script_camera_probe_method_names_residual_wave252() -> bool {
    LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_METHOD_NAMES_WAVE252.len() == 5
        && residual_name_index(
            LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_METHOD_NAMES_WAVE252,
            "script_default_camera_max_height",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_METHOD_NAMES_WAVE252,
            "ui_script_default_camera_pitch",
        ) == Some(3)
        && residual_name_index(
            LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_METHOD_NAMES_WAVE252,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_presentation_script_camera_probe_nav_commands_residual_wave252() -> bool {
    LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_NAV_STEPS_WAVE252.len() == 4
        && residual_name_index(
            LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_NAV_STEPS_WAVE252,
            "REQUIRE_PRESENTATION_SCRIPT_CAMERA_FIELDS",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_NAV_STEPS_WAVE252,
            "LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRESENTATION_SCRIPT_CAMERA_PROBE_CMD_NAMES_WAVE252.len() == 3
}

/// Wave 252 composite residual honesty pack.
pub fn honesty_live_presentation_script_camera_probe_residual_pack_wave252() -> bool {
    honesty_live_presentation_script_camera_probe_method_names_residual_wave252()
        && honesty_live_presentation_script_camera_probe_nav_commands_residual_wave252()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: presentation freezes script camera; UI helpers prefer freeze.
pub fn honesty_presentation_script_camera_probe_source() -> bool {
    let pf = include_str!("../presentation_frame.rs");
    let eng = include_str!("../cnc_game_engine.rs");
    if !(pf.contains("pub script_default_camera_max_height: f32")
        && pf.contains("pub script_default_camera_pitch: f32")
        && pf.contains("Wave 252")
        && pf
            .contains("script_default_camera_max_height: logic.script_default_camera_max_height()"))
    {
        return false;
    }
    let Some(max_h) = fn_body(eng, "fn ui_script_default_camera_max_height(") else {
        return false;
    };
    let Some(pitch) = fn_body(eng, "fn ui_script_default_camera_pitch(") else {
        return false;
    };
    max_h.contains("Wave 252")
        && max_h.contains("last_presentation_frame")
        && pitch.contains("Wave 252")
        && pitch.contains("last_presentation_frame")
        // production consumers use helpers (not bare dual-read outside helpers)
        && eng.contains("self.ui_script_default_camera_max_height()")
        && eng.contains("self.ui_script_default_camera_pitch()")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_presentation_script_camera_probe_honesty() -> bool {
    honesty_live_presentation_script_camera_probe_residual_pack_wave252()
        && honesty_presentation_script_camera_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_presentation_script_camera_probe_method_names_residual_wave252());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_presentation_script_camera_probe_nav_commands_residual_wave252());
    }

    #[test]
    fn wave252_composite_pack() {
        assert!(honesty_live_presentation_script_camera_probe_residual_pack_wave252());
    }

    #[test]
    fn presentation_script_camera_probe_sources() {
        assert!(honesty_presentation_script_camera_probe_source());
    }

    #[test]
    fn simulate_live_presentation_script_camera_probe_honesty_residual_live() {
        assert!(
            simulate_live_presentation_script_camera_probe_honesty(),
            "presentation script camera probe residual must latch"
        );
    }
}
