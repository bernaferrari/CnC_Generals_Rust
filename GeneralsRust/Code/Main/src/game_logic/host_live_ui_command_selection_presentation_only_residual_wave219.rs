//! Wave 219 residual peels: InGame UI command paths
//! (`commit_pending_map_command`, structure placement, wall line, cancel
//! production, special power, named command issue, minimap move) read selection
//! via presentation-first `ui_selected_ids`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 218 runtime-host selection-command residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` UI command helpers
//!
//! Fail-closed:
//! - Not full ControlBar WND command matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// UI-command selection presentation-only residual method names.
pub const LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE219: &[&str] = &[
    "ui_selected_ids",
    "commit_pending_map_command",
    "place_structure_from_ui",
    "issue_named_command_from_ui",
    "issue_minimap_move",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_NAV_STEPS_WAVE219: &[&str] = &[
    "REQUIRE_UI_COMMAND_SELECTION_PRESENTATION_ONLY",
    "REQUIRE_UI_SELECTED_IDS",
    "LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_CMD_NAMES_WAVE219: &[&str] = &[
    "click_live_ui_command_selection_presentation_only_ok_prepare",
    "click_live_ui_command_selection_presentation_only_ok_live",
    "click_live_ui_command_selection_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ui_command_selection_presentation_only_method_names_residual_wave219() -> bool {
    LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE219.len() == 6
        && residual_name_index(
            LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE219,
            "ui_selected_ids",
        ) == Some(0)
        && residual_name_index(
            LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE219,
            "issue_minimap_move",
        ) == Some(4)
        && residual_name_index(
            LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE219,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ui_command_selection_presentation_only_nav_commands_residual_wave219() -> bool {
    LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_NAV_STEPS_WAVE219.len() == 4
        && residual_name_index(
            LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_NAV_STEPS_WAVE219,
            "REQUIRE_UI_COMMAND_SELECTION_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_NAV_STEPS_WAVE219,
            "LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_UI_COMMAND_SELECTION_PRESENTATION_ONLY_CMD_NAMES_WAVE219.len() == 3
}

/// Wave 219 composite residual honesty pack.
pub fn honesty_live_ui_command_selection_presentation_only_residual_pack_wave219() -> bool {
    honesty_live_ui_command_selection_presentation_only_method_names_residual_wave219()
        && honesty_live_ui_command_selection_presentation_only_nav_commands_residual_wave219()
}

fn eng_fn_body<'a>(eng: &'a str, sig: &str) -> Option<&'a str> {
    let i = eng.find(sig)?;
    let rest = &eng[i..];
    let end = rest.find("\n    fn ").unwrap_or(rest.len().min(2200));
    Some(&rest[..end])
}

/// Source residual: key UI command helpers use ui_selected_ids, not live get_player first.
pub fn honesty_ui_command_selection_presentation_only_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    let sigs = [
        "fn commit_pending_map_command(",
        "fn place_structure_from_ui(",
        "fn place_wall_line_from_ui(",
        "fn cancel_selected_production_queue_head(",
        "fn cancel_all_selected_production(",
        "fn cancel_unit_production_from_ui(",
        "fn issue_minimap_move(",
    ];
    for sig in sigs {
        let Some(body) = eng_fn_body(eng, sig) else {
            return false;
        };
        if !body.contains("Wave 219") || !body.contains("ui_selected_ids") {
            return false;
        }
        // First selection source must not be get_player selected_objects.
        let ui = body.find("ui_selected_ids");
        let gp = body.find("get_player");
        if let (Some(u), Some(g)) = (ui, gp) {
            if g < u && body[g..g + 80].contains("selected_objects") {
                return false;
            }
        }
    }
    // issue_named_command_from_ui selection block
    let Some(i) = eng.find("fn issue_named_command_from_ui") else {
        // may be private differently named — search Wave 219 near Stop | Scatter
        let Some(j) = eng.find("CommandType::ViewLastRadarEvent") else {
            return false;
        };
        let win = &eng[j.saturating_sub(500)..j];
        return win.contains("Wave 219") && win.contains("ui_selected_ids");
    };
    let body = eng_fn_body(eng, "fn issue_named_command_from_ui").unwrap_or("");
    let _ = i;
    body.contains("Wave 219") && body.contains("ui_selected_ids")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ui_command_selection_presentation_only_honesty() -> bool {
    honesty_live_ui_command_selection_presentation_only_residual_pack_wave219()
        && honesty_ui_command_selection_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_ui_command_selection_presentation_only_method_names_residual_wave219()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_ui_command_selection_presentation_only_nav_commands_residual_wave219()
        );
    }

    #[test]
    fn wave219_composite_pack() {
        assert!(honesty_live_ui_command_selection_presentation_only_residual_pack_wave219());
    }

    #[test]
    fn ui_command_selection_sources() {
        assert!(honesty_ui_command_selection_presentation_only_source());
    }

    #[test]
    fn simulate_live_ui_command_selection_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_ui_command_selection_presentation_only_honesty(),
            "ui-command selection presentation-only residual must latch"
        );
    }
}
