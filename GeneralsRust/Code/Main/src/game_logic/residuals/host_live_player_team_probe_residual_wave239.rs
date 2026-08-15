//! Wave 239 residual peels: camera boot/focus uses GameLogic `player_team` /
//! `player_command_center_position` probes instead of dual-reading `&Player`
//! via `get_player`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 238 player economy/science probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` player_team / player_command_center_position
//! - `cnc_game_engine.rs` bootstrap_camera_for_loaded_map / reset_camera_view_hotkey
//!
//! Fail-closed:
//! - ui_player_info live fallback may still use get_player for full roster fields
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Player team probe residual method names.
pub const LIVE_PLAYER_TEAM_PROBE_METHOD_NAMES_WAVE239: &[&str] = &[
    "player_team",
    "player_command_center_position",
    "bootstrap_camera_for_loaded_map",
    "reset_camera_view_hotkey",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PLAYER_TEAM_PROBE_NAV_STEPS_WAVE239: &[&str] = &[
    "REQUIRE_PLAYER_TEAM_PROBE",
    "REQUIRE_CAMERA_BOOT_USES_PROBES",
    "LIVE_PLAYER_TEAM_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PLAYER_TEAM_PROBE_CMD_NAMES_WAVE239: &[&str] = &[
    "click_live_player_team_probe_ok_prepare",
    "click_live_player_team_probe_ok_live",
    "click_live_player_team_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_player_team_probe_method_names_residual_wave239() -> bool {
    LIVE_PLAYER_TEAM_PROBE_METHOD_NAMES_WAVE239.len() == 5
        && residual_name_index(LIVE_PLAYER_TEAM_PROBE_METHOD_NAMES_WAVE239, "player_team")
            == Some(0)
        && residual_name_index(
            LIVE_PLAYER_TEAM_PROBE_METHOD_NAMES_WAVE239,
            "reset_camera_view_hotkey",
        ) == Some(3)
        && residual_name_index(
            LIVE_PLAYER_TEAM_PROBE_METHOD_NAMES_WAVE239,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_player_team_probe_nav_commands_residual_wave239() -> bool {
    LIVE_PLAYER_TEAM_PROBE_NAV_STEPS_WAVE239.len() == 4
        && residual_name_index(
            LIVE_PLAYER_TEAM_PROBE_NAV_STEPS_WAVE239,
            "REQUIRE_PLAYER_TEAM_PROBE",
        ) == Some(0)
        && residual_name_index(
            LIVE_PLAYER_TEAM_PROBE_NAV_STEPS_WAVE239,
            "LIVE_PLAYER_TEAM_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PLAYER_TEAM_PROBE_CMD_NAMES_WAVE239.len() == 3
}

/// Wave 239 composite residual honesty pack.
pub fn honesty_live_player_team_probe_residual_pack_wave239() -> bool {
    honesty_live_player_team_probe_method_names_residual_wave239()
        && honesty_live_player_team_probe_nav_commands_residual_wave239()
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

/// Source residual: team probes exist; camera boot uses them without get_player.
pub fn honesty_player_team_probe_source() -> bool {
    let gl = super::GAME_LOGIC_HOST_SRC;
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    if !(gl.contains("pub fn player_team(")
        && gl.contains("pub fn player_command_center_position("))
    {
        return false;
    }
    let Some(boot) = fn_body(eng, "fn bootstrap_camera_for_loaded_map(") else {
        return false;
    };
    // Wave 473: bootstrap prefers presentation local_team_base_position (no live player_team).
    // GameLogic::player_team remains available for other residual consumers.
    if !((boot.contains("Wave 239") && boot.contains("player_team("))
        || (boot.contains("local_team_base_position")
            && (boot.contains("Wave 223")
                || boot.contains("Wave 458")
                || boot.contains("Wave 473"))))
        || boot.contains("get_player(")
    {
        return false;
    }
    let Some(reset) = fn_body(eng, "fn reset_camera_view_hotkey(") else {
        return false;
    };
    reset.contains("player_command_center_position")
        && (reset.contains("Wave 239") || reset.contains("Wave 237"))
        && !reset.contains("get_player(")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_player_team_probe_honesty() -> bool {
    honesty_live_player_team_probe_residual_pack_wave239() && honesty_player_team_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_player_team_probe_method_names_residual_wave239());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_player_team_probe_nav_commands_residual_wave239());
    }

    #[test]
    fn wave239_composite_pack() {
        assert!(honesty_live_player_team_probe_residual_pack_wave239());
    }

    #[test]
    fn player_team_probe_sources() {
        assert!(honesty_player_team_probe_source());
    }

    #[test]
    fn simulate_live_player_team_probe_honesty_residual_live() {
        assert!(
            simulate_live_player_team_probe_honesty(),
            "player team probe residual must latch"
        );
    }
}
