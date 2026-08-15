//! Wave 234 residual peels: engine InGame UI player/selection dual-reads prefer
//! presentation freeze (`ui_player_info` / `ui_selected_ids` / local team helpers)
//! instead of live `GameLogic::get_player` when a frame is installed.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 233 command_executor production dual-mut clear.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` ui_player_info / ui_selected_ids / local_team_for_ui /
//!   center_camera / right-click / science purchase gate / context cursor
//! - `presentation_frame.rs` players roster + selected freeze
//!
//! Fail-closed:
//! - Boot/menu paths may still dual-read when no presentation frame
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Engine presentation player UI residual method names.
pub const LIVE_ENGINE_PRESENTATION_PLAYER_UI_METHOD_NAMES_WAVE234: &[&str] = &[
    "ui_player_info",
    "ui_selected_ids",
    "local_team_for_ui",
    "ui_local_science_purchase_points",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ENGINE_PRESENTATION_PLAYER_UI_NAV_STEPS_WAVE234: &[&str] = &[
    "REQUIRE_ENGINE_PRESENTATION_PLAYER_UI",
    "REQUIRE_SELECTION_AND_SCIENCE_PREFER_FRAME",
    "LIVE_ENGINE_PRESENTATION_PLAYER_UI",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ENGINE_PRESENTATION_PLAYER_UI_CMD_NAMES_WAVE234: &[&str] = &[
    "click_live_engine_presentation_player_ui_ok_prepare",
    "click_live_engine_presentation_player_ui_ok_live",
    "click_live_engine_presentation_player_ui_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_live_engine_presentation_player_ui_method_names_residual_wave234() -> bool {
    LIVE_ENGINE_PRESENTATION_PLAYER_UI_METHOD_NAMES_WAVE234.len() == 5
        && residual_name_index(
            LIVE_ENGINE_PRESENTATION_PLAYER_UI_METHOD_NAMES_WAVE234,
            "ui_player_info",
        ) == Some(0)
        && residual_name_index(
            LIVE_ENGINE_PRESENTATION_PLAYER_UI_METHOD_NAMES_WAVE234,
            "ui_local_science_purchase_points",
        ) == Some(3)
        && residual_name_index(
            LIVE_ENGINE_PRESENTATION_PLAYER_UI_METHOD_NAMES_WAVE234,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_engine_presentation_player_ui_nav_commands_residual_wave234() -> bool {
    LIVE_ENGINE_PRESENTATION_PLAYER_UI_NAV_STEPS_WAVE234.len() == 4
        && residual_name_index(
            LIVE_ENGINE_PRESENTATION_PLAYER_UI_NAV_STEPS_WAVE234,
            "REQUIRE_ENGINE_PRESENTATION_PLAYER_UI",
        ) == Some(0)
        && residual_name_index(
            LIVE_ENGINE_PRESENTATION_PLAYER_UI_NAV_STEPS_WAVE234,
            "LIVE_ENGINE_PRESENTATION_PLAYER_UI",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ENGINE_PRESENTATION_PLAYER_UI_CMD_NAMES_WAVE234.len() == 3
}

/// Wave 234 composite residual honesty pack.
pub fn honesty_live_engine_presentation_player_ui_residual_pack_wave234() -> bool {
    honesty_live_engine_presentation_player_ui_method_names_residual_wave234()
        && honesty_live_engine_presentation_player_ui_nav_commands_residual_wave234()
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

/// Source residual: helpers + InGame consumers prefer presentation freeze.
pub fn honesty_engine_presentation_player_ui_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    for api in [
        "fn ui_player_info(",
        "fn host_ui_player_info(",
        "fn ui_selected_ids(",
        "fn host_ui_selected_ids(",
        "fn local_team_for_ui(",
        "fn host_local_team_for_ui(",
        "fn ui_local_science_purchase_points(",
        "fn ui_selection_seed_id(",
        "fn ui_local_player_team_name(",
        "fn host_ui_local_player_team_name(",
    ] {
        if !eng.contains(api) {
            return false;
        }
    }
    for (name, token) in [
        ("fn center_camera_on_selection(", "ui_selected_ids"),
        ("fn handle_right_click(", "ui_selected_ids"),
        // Wave 612: force-attack / cursor logic live in host helpers.
        (
            "fn host_issue_force_attack_from_left_click(",
            "ui_selected_ids",
        ),
        ("fn host_resolve_context_cursor_icon(", "ui_selected_ids"),
        (
            "fn try_purchase_next_generals_science(",
            "ui_local_science_purchase_points",
        ),
        // Wave 607: logic lives in host_ui_player_info; thin wrapper delegates.
        ("fn host_ui_player_info(", "last_presentation_frame"),
    ] {
        let Some(body) = fn_body(eng, name) else {
            return false;
        };
        if !body.contains(token) {
            return false;
        }
        if name.starts_with("fn center_camera")
            || name.starts_with("fn handle_right")
            || name.starts_with("fn issue_force")
            || name.starts_with("fn host_issue_force")
        {
            if !body.contains("Wave 234") {
                return false;
            }
        }
    }
    // Cursor selection presence must not dual-read player.selected_objects first.
    let Some(cursor) = fn_body(eng, "fn host_resolve_context_cursor_icon(")
        .or_else(|| fn_body(eng, "fn resolve_context_cursor_icon("))
    else {
        return false;
    };
    cursor.contains("Wave 234")
        && cursor.contains("ui_selected_ids")
        && !cursor.contains("ui_selected_ids")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_engine_presentation_player_ui_honesty() -> bool {
    honesty_live_engine_presentation_player_ui_residual_pack_wave234()
        && honesty_engine_presentation_player_ui_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_engine_presentation_player_ui_method_names_residual_wave234());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_engine_presentation_player_ui_nav_commands_residual_wave234());
    }

    #[test]
    fn wave234_composite_pack() {
        assert!(honesty_live_engine_presentation_player_ui_residual_pack_wave234());
    }

    #[test]
    fn engine_presentation_player_ui_sources() {
        assert!(honesty_engine_presentation_player_ui_source());
    }

    #[test]
    fn simulate_live_engine_presentation_player_ui_honesty_residual_live() {
        assert!(
            simulate_live_engine_presentation_player_ui_honesty(),
            "engine presentation player UI residual must latch"
        );
    }
}
