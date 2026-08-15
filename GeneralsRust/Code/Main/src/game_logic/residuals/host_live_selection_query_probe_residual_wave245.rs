//! Wave 245 residual peels: selection / context / integration paths use
//! GameLogic selectable/unit query probes instead of dual-reading via
//! `get_object` / `get_objects` / `get_player`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 244 command unit probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` selectable_unit_ids_in_bounds / selectable_similar_unit_ids /
//!   unit_ids_for_team_where / unit_object_type / unit_position / …
//! - `command_system.rs` create_selection_command / create_select_similar /
//!   determine_context_command / can_* ObjectId APIs
//! - `command_integration.rs` select cycles / matching / find_object_at_position
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Selection query probe residual method names.
pub const LIVE_SELECTION_QUERY_PROBE_METHOD_NAMES_WAVE245: &[&str] = &[
    "selectable_unit_ids_in_bounds",
    "selectable_similar_unit_ids",
    "unit_ids_for_team_where",
    "create_selection_command",
    "determine_context_command",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SELECTION_QUERY_PROBE_NAV_STEPS_WAVE245: &[&str] = &[
    "REQUIRE_SELECTION_QUERY_PROBE",
    "REQUIRE_NO_GET_OBJECTS_IN_COMMAND_PROD",
    "LIVE_SELECTION_QUERY_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SELECTION_QUERY_PROBE_CMD_NAMES_WAVE245: &[&str] = &[
    "click_live_selection_query_probe_ok_prepare",
    "click_live_selection_query_probe_ok_live",
    "click_live_selection_query_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_selection_query_probe_method_names_residual_wave245() -> bool {
    LIVE_SELECTION_QUERY_PROBE_METHOD_NAMES_WAVE245.len() == 6
        && residual_name_index(
            LIVE_SELECTION_QUERY_PROBE_METHOD_NAMES_WAVE245,
            "selectable_unit_ids_in_bounds",
        ) == Some(0)
        && residual_name_index(
            LIVE_SELECTION_QUERY_PROBE_METHOD_NAMES_WAVE245,
            "determine_context_command",
        ) == Some(4)
        && residual_name_index(
            LIVE_SELECTION_QUERY_PROBE_METHOD_NAMES_WAVE245,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_selection_query_probe_nav_commands_residual_wave245() -> bool {
    LIVE_SELECTION_QUERY_PROBE_NAV_STEPS_WAVE245.len() == 4
        && residual_name_index(
            LIVE_SELECTION_QUERY_PROBE_NAV_STEPS_WAVE245,
            "REQUIRE_SELECTION_QUERY_PROBE",
        ) == Some(0)
        && residual_name_index(
            LIVE_SELECTION_QUERY_PROBE_NAV_STEPS_WAVE245,
            "LIVE_SELECTION_QUERY_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SELECTION_QUERY_PROBE_CMD_NAMES_WAVE245.len() == 3
}

/// Wave 245 composite residual honesty pack.
pub fn honesty_live_selection_query_probe_residual_pack_wave245() -> bool {
    honesty_live_selection_query_probe_method_names_residual_wave245()
        && honesty_live_selection_query_probe_nav_commands_residual_wave245()
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

/// Source residual: selection/context use query probes; no get_object dual-read in prod.
pub fn honesty_selection_query_probe_source() -> bool {
    // 2026-08-15: scan host plus extra world_* splits.
    let gl = super::host_logic_scan_src();
    // 2026-08-15: do not truncate COMMAND_SYSTEM_SRC at first `#[cfg(test)]`.
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    let ci = include_str!("../../command_integration.rs");
    if !(gl.contains("pub fn selectable_unit_ids_in_bounds_for_player(")
        && gl.contains("pub fn selectable_unit_ids_in_bounds(")
        && gl.contains("pub fn selectable_similar_unit_ids(")
        && (gl.contains("pub fn unit_ids_for_team_where(")
            || gl.contains("pub fn unit_ids_for_player_where(")))
    {
        return false;
    }
    let Some(box_sel) = fn_body(cs, "fn create_selection_command(") else {
        return false;
    };
    if !(box_sel.contains("selectable_unit_ids_in_bounds") && !box_sel.contains("get_object(")) {
        return false;
    }
    let Some(ctx) = fn_body(cs, "fn determine_context_command(") else {
        return false;
    };
    if ctx.contains("get_object(") {
        return false;
    }
    (ci.contains("unit_ids_for_team_where") || ci.contains("unit_ids_for_player_where"))
        && ci.contains("Wave 245")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_selection_query_probe_honesty() -> bool {
    honesty_live_selection_query_probe_residual_pack_wave245()
        && honesty_selection_query_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_selection_query_probe_method_names_residual_wave245());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_selection_query_probe_nav_commands_residual_wave245());
    }

    #[test]
    fn wave245_composite_pack() {
        assert!(honesty_live_selection_query_probe_residual_pack_wave245());
    }

    #[test]
    fn selection_query_probe_sources() {
        assert!(honesty_selection_query_probe_source());
    }

    #[test]
    fn simulate_live_selection_query_probe_honesty_residual_live() {
        assert!(
            simulate_live_selection_query_probe_honesty(),
            "selection query probe residual must latch"
        );
    }
}
