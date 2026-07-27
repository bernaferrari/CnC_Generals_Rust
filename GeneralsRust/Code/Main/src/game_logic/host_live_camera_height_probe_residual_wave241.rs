//! Wave 241 residual peels: startup camera height sampling uses presentation
//! freeze when installed and only dual-reads live `GameLogic` as a no-frame
//! boot residual (`Option<&GameLogic>`). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 240 player field probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` sample_startup_camera_heights /
//!   bootstrap_camera_for_loaded_map / compute_default_camera_zoom_for_target
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Camera height probe residual method names.
pub const LIVE_CAMERA_HEIGHT_PROBE_METHOD_NAMES_WAVE241: &[&str] = &[
    "sample_startup_camera_heights",
    "presentation-only heights (Wave 473)",
    "bootstrap_camera_for_loaded_map",
    "compute_default_camera_zoom_for_target",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CAMERA_HEIGHT_PROBE_NAV_STEPS_WAVE241: &[&str] = &[
    "REQUIRE_CAMERA_HEIGHT_OPTION_LOGIC",
    "REQUIRE_PRESENTATION_SKIPS_LIVE",
    "LIVE_CAMERA_HEIGHT_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CAMERA_HEIGHT_PROBE_CMD_NAMES_WAVE241: &[&str] = &[
    "click_live_camera_height_probe_ok_prepare",
    "click_live_camera_height_probe_ok_live",
    "click_live_camera_height_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_camera_height_probe_method_names_residual_wave241() -> bool {
    LIVE_CAMERA_HEIGHT_PROBE_METHOD_NAMES_WAVE241.len() == 5
        && residual_name_index(
            LIVE_CAMERA_HEIGHT_PROBE_METHOD_NAMES_WAVE241,
            "sample_startup_camera_heights",
        ) == Some(0)
        && residual_name_index(
            LIVE_CAMERA_HEIGHT_PROBE_METHOD_NAMES_WAVE241,
            "presentation-only heights (Wave 473)",
        ) == Some(1)
        && residual_name_index(
            LIVE_CAMERA_HEIGHT_PROBE_METHOD_NAMES_WAVE241,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_camera_height_probe_nav_commands_residual_wave241() -> bool {
    LIVE_CAMERA_HEIGHT_PROBE_NAV_STEPS_WAVE241.len() == 4
        && residual_name_index(
            LIVE_CAMERA_HEIGHT_PROBE_NAV_STEPS_WAVE241,
            "REQUIRE_CAMERA_HEIGHT_OPTION_LOGIC",
        ) == Some(0)
        && residual_name_index(
            LIVE_CAMERA_HEIGHT_PROBE_NAV_STEPS_WAVE241,
            "LIVE_CAMERA_HEIGHT_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CAMERA_HEIGHT_PROBE_CMD_NAMES_WAVE241.len() == 3
}

/// Wave 241 composite residual honesty pack.
pub fn honesty_live_camera_height_probe_residual_pack_wave241() -> bool {
    honesty_live_camera_height_probe_method_names_residual_wave241()
        && honesty_live_camera_height_probe_nav_commands_residual_wave241()
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

/// Source residual: sample takes Option; presentation path skips live dual-read.
pub fn honesty_camera_height_probe_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    let Some(sample) = fn_body(eng, "fn sample_startup_camera_heights(") else {
        return false;
    };
    if !(sample.contains("presentation")
        && sample.contains("sample_height")
        && (sample.contains("Wave 241") || sample.contains("Wave 473"))
        && !sample.contains("game_logic: Option<&GameLogic>")
        && !sample.contains("if let Some(gl) = game_logic")
        && !sample.contains("game_logic:"))
    {
        return false;
    }
    // Wave 473: bootstrap is presentation-only (no live_logic gate).
    let Some(boot) = fn_body(eng, "fn bootstrap_camera_for_loaded_map(") else {
        return false;
    };
    boot.contains("presentation")
        && boot.contains("world_bounds_vec3()")
        && !boot.contains("game_logic:")
        && eng.contains("sample_startup_camera_heights")
        && !eng.contains("startup_camera_live_logic")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_camera_height_probe_honesty() -> bool {
    honesty_live_camera_height_probe_residual_pack_wave241() && honesty_camera_height_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_camera_height_probe_method_names_residual_wave241());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_camera_height_probe_nav_commands_residual_wave241());
    }

    #[test]
    fn wave241_composite_pack() {
        assert!(honesty_live_camera_height_probe_residual_pack_wave241());
    }

    #[test]
    fn camera_height_probe_sources() {
        assert!(honesty_camera_height_probe_source());
    }

    #[test]
    fn simulate_live_camera_height_probe_honesty_residual_live() {
        assert!(
            simulate_live_camera_height_probe_honesty(),
            "camera height probe residual must latch"
        );
    }
}
