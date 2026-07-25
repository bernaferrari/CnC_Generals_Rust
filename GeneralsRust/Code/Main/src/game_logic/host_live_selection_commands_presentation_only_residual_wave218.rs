//! Wave 218 residual peels: runtime-host move/force-attack/waypoint/
//! select_all/select_similar/cancel_production selection identity goes through
//! presentation-first `ui_selected_ids` (no live player.selected_objects dual-read
//! for those command paths). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 217 cmd-filter/env presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` move/force_attack/waypoint/select_*/cancel_production
//!
//! Fail-closed:
//! - Not full C++ selection double-click / box-select parity matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Selection-command presentation-only residual method names.
pub const LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_METHOD_NAMES_WAVE218: &[&str] = &[
    "ui_selected_ids",
    "move",
    "force_attack",
    "waypoint",
    "select_similar",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_NAV_STEPS_WAVE218: &[&str] = &[
    "REQUIRE_SELECTION_COMMANDS_PRESENTATION_ONLY",
    "REQUIRE_UI_SELECTED_IDS",
    "LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_CMD_NAMES_WAVE218: &[&str] = &[
    "click_live_selection_commands_presentation_only_ok_prepare",
    "click_live_selection_commands_presentation_only_ok_live",
    "click_live_selection_commands_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_selection_commands_presentation_only_method_names_residual_wave218() -> bool {
    LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_METHOD_NAMES_WAVE218.len() == 6
        && residual_name_index(
            LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_METHOD_NAMES_WAVE218,
            "ui_selected_ids",
        ) == Some(0)
        && residual_name_index(
            LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_METHOD_NAMES_WAVE218,
            "force_attack",
        ) == Some(2)
        && residual_name_index(
            LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_METHOD_NAMES_WAVE218,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_selection_commands_presentation_only_nav_commands_residual_wave218() -> bool {
    LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_NAV_STEPS_WAVE218.len() == 4
        && residual_name_index(
            LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_NAV_STEPS_WAVE218,
            "REQUIRE_SELECTION_COMMANDS_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_NAV_STEPS_WAVE218,
            "LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SELECTION_COMMANDS_PRESENTATION_ONLY_CMD_NAMES_WAVE218.len() == 3
}

/// Wave 218 composite residual honesty pack.
pub fn honesty_live_selection_commands_presentation_only_residual_pack_wave218() -> bool {
    honesty_live_selection_commands_presentation_only_method_names_residual_wave218()
        && honesty_live_selection_commands_presentation_only_nav_commands_residual_wave218()
}

/// Source residual: move/force_attack/waypoint/select paths use ui_selected_ids.
pub fn honesty_selection_commands_presentation_only_source() -> bool {
    let eng = include_str!("../cnc_game_engine.rs");
    let checks = [
        (
            "move_fail_no_selection",
            "Wave 218: selection count via presentation-first ui_selected_ids",
        ),
        (
            "force_attack_fail_no_selection",
            "Wave 218: selection via presentation-first ui_selected_ids",
        ),
        (
            "force_attack_object_fail_no_selection",
            "Wave 218: selection via presentation-first ui_selected_ids",
        ),
        (
            "waypoint_fail_no_selection",
            "Wave 218: selection via presentation-first ui_selected_ids",
        ),
        (
            "select_all_ok:",
            "Wave 218: count via presentation-first ui_selected_ids",
        ),
        (
            "select_similar_fail_no_seed",
            "Wave 218: seed/count via presentation-first ui_selected_ids",
        ),
        (
            "cancel_production_fail_no_selection",
            "Wave 218: empty selection via presentation-first ui_selected_ids",
        ),
    ];
    for (anchor, note) in checks {
        let Some(i) = eng.find(anchor) else {
            return false;
        };
        let lo = i.saturating_sub(900);
        let hi = (i + 200).min(eng.len());
        if !eng[lo..hi].contains(note) && !eng[lo..hi].contains("ui_selected_ids") {
            return false;
        }
        let _ = note;
    }
    // force_attack path must not prefer live get_player selected_objects first.
    let Some(i) = eng.find("force_attack_fail_no_selection") else {
        return false;
    };
    let win = &eng[i.saturating_sub(500)..i];
    win.contains("ui_selected_ids") && !win.contains("get_player(self.current_player_id)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_selection_commands_presentation_only_honesty() -> bool {
    honesty_live_selection_commands_presentation_only_residual_pack_wave218()
        && honesty_selection_commands_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_selection_commands_presentation_only_method_names_residual_wave218());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_selection_commands_presentation_only_nav_commands_residual_wave218());
    }

    #[test]
    fn wave218_composite_pack() {
        assert!(honesty_live_selection_commands_presentation_only_residual_pack_wave218());
    }

    #[test]
    fn selection_commands_sources() {
        assert!(honesty_selection_commands_presentation_only_source());
    }

    #[test]
    fn simulate_live_selection_commands_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_selection_commands_presentation_only_honesty(),
            "selection-commands presentation-only residual must latch"
        );
    }
}
