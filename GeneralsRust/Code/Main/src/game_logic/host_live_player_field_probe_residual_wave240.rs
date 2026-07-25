//! Wave 240 residual peels: UI helpers + diplomacy/supplies boot paths use
//! GameLogic player field probes instead of dual-reading `&Player` /
//! `&mut Player` via `get_player` / `get_players` / `get_player_mut`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 239 player team/camera probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` player_exists / player_name / player_team / player_ids /
//!   player_selected_objects / ensure_player_min_supplies
//! - `cnc_game_engine.rs` local_player_id_for_ui / local_team_for_ui /
//!   ui_player_info / ui_selected_ids / diplomacy roster / supplies floor
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Player field probe residual method names.
pub const LIVE_PLAYER_FIELD_PROBE_METHOD_NAMES_WAVE240: &[&str] = &[
    "player_exists",
    "player_name",
    "player_ids",
    "player_selected_objects",
    "ensure_player_min_supplies",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PLAYER_FIELD_PROBE_NAV_STEPS_WAVE240: &[&str] = &[
    "REQUIRE_PLAYER_FIELD_PROBE",
    "REQUIRE_UI_HELPERS_USE_PROBES",
    "LIVE_PLAYER_FIELD_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PLAYER_FIELD_PROBE_CMD_NAMES_WAVE240: &[&str] = &[
    "click_live_player_field_probe_ok_prepare",
    "click_live_player_field_probe_ok_live",
    "click_live_player_field_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_player_field_probe_method_names_residual_wave240() -> bool {
    LIVE_PLAYER_FIELD_PROBE_METHOD_NAMES_WAVE240.len() == 6
        && residual_name_index(
            LIVE_PLAYER_FIELD_PROBE_METHOD_NAMES_WAVE240,
            "player_exists",
        ) == Some(0)
        && residual_name_index(
            LIVE_PLAYER_FIELD_PROBE_METHOD_NAMES_WAVE240,
            "ensure_player_min_supplies",
        ) == Some(4)
        && residual_name_index(
            LIVE_PLAYER_FIELD_PROBE_METHOD_NAMES_WAVE240,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_player_field_probe_nav_commands_residual_wave240() -> bool {
    LIVE_PLAYER_FIELD_PROBE_NAV_STEPS_WAVE240.len() == 4
        && residual_name_index(
            LIVE_PLAYER_FIELD_PROBE_NAV_STEPS_WAVE240,
            "REQUIRE_PLAYER_FIELD_PROBE",
        ) == Some(0)
        && residual_name_index(
            LIVE_PLAYER_FIELD_PROBE_NAV_STEPS_WAVE240,
            "LIVE_PLAYER_FIELD_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PLAYER_FIELD_PROBE_CMD_NAMES_WAVE240.len() == 3
}

/// Wave 240 composite residual honesty pack.
pub fn honesty_live_player_field_probe_residual_pack_wave240() -> bool {
    honesty_live_player_field_probe_method_names_residual_wave240()
        && honesty_live_player_field_probe_nav_commands_residual_wave240()
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

/// Source residual: field probes exist; UI helpers avoid get_player dual-read.
pub fn honesty_player_field_probe_source() -> bool {
    let gl = include_str!("game_logic.rs");
    let eng = include_str!("../cnc_game_engine.rs");
    if !(gl.contains("pub fn player_exists(")
        && gl.contains("pub fn player_name(")
        && gl.contains("pub fn player_ids(")
        && gl.contains("pub fn player_selected_objects(")
        && gl.contains("pub fn ensure_player_min_supplies("))
    {
        return false;
    }
    let Some(local) = fn_body(eng, "fn local_player_id_for_ui(") else {
        return false;
    };
    if !(local.contains("Wave 240")
        && local.contains("player_exists")
        && !local.contains("get_player("))
    {
        return false;
    }
    let Some(info) = fn_body(eng, "fn ui_player_info(") else {
        return false;
    };
    if !(info.contains("Wave 240") && !info.contains("get_player(")) {
        return false;
    }
    let Some(sel) = fn_body(eng, "fn ui_selected_ids(") else {
        return false;
    };
    sel.contains("player_selected_objects")
        && !sel.contains("get_player(")
        && eng.contains("ensure_player_min_supplies")
        && !eng.contains("get_players()")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_player_field_probe_honesty() -> bool {
    honesty_live_player_field_probe_residual_pack_wave240() && honesty_player_field_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_player_field_probe_method_names_residual_wave240());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_player_field_probe_nav_commands_residual_wave240());
    }

    #[test]
    fn wave240_composite_pack() {
        assert!(honesty_live_player_field_probe_residual_pack_wave240());
    }

    #[test]
    fn player_field_probe_sources() {
        assert!(honesty_player_field_probe_source());
    }

    #[test]
    fn simulate_live_player_field_probe_honesty_residual_live() {
        assert!(
            simulate_live_player_field_probe_honesty(),
            "player field probe residual must latch"
        );
    }
}
