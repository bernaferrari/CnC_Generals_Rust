//! Wave 221 residual peels: runtime-host move selection push, attack selection
//! count, and classic A/T hotkey paths read selection via presentation-first
//! `ui_selected_ids` (no live `get_player.selected_objects` dual-read). Never
//! flips shell `playable_claim`.
//!
//! Orthogonal to Wave 220 local-team presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` move/attack_nearest_enemy + A/T hotkeys
//!
//! Fail-closed:
//! - Not full C++ hotkey matrix parity
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Hotkey/move/attack selection presentation-only residual method names.
pub const LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE221: &[&str] = &[
    "ui_selected_ids",
    "move",
    "attack_nearest_enemy",
    "attack_move_hotkey",
    "force_attack_ground_hotkey",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_NAV_STEPS_WAVE221: &[&str] = &[
    "REQUIRE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY",
    "REQUIRE_UI_SELECTED_IDS",
    "LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_CMD_NAMES_WAVE221:
    &[&str] = &[
    "click_live_hotkey_move_attack_selection_presentation_only_ok_prepare",
    "click_live_hotkey_move_attack_selection_presentation_only_ok_live",
    "click_live_hotkey_move_attack_selection_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_hotkey_move_attack_selection_presentation_only_method_names_residual_wave221()
-> bool {
    LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE221.len() == 6
        && residual_name_index(
            LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE221,
            "ui_selected_ids",
        ) == Some(0)
        && residual_name_index(
            LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE221,
            "force_attack_ground_hotkey",
        ) == Some(4)
        && residual_name_index(
            LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_METHOD_NAMES_WAVE221,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_hotkey_move_attack_selection_presentation_only_nav_commands_residual_wave221()
-> bool {
    LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_NAV_STEPS_WAVE221.len() == 4
        && residual_name_index(
            LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_NAV_STEPS_WAVE221,
            "REQUIRE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_NAV_STEPS_WAVE221,
            "LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_HOTKEY_MOVE_ATTACK_SELECTION_PRESENTATION_ONLY_CMD_NAMES_WAVE221.len()
            == 3
}

/// Wave 221 composite residual honesty pack.
pub fn honesty_live_hotkey_move_attack_selection_presentation_only_residual_pack_wave221() -> bool {
    honesty_live_hotkey_move_attack_selection_presentation_only_method_names_residual_wave221()
        && honesty_live_hotkey_move_attack_selection_presentation_only_nav_commands_residual_wave221(
        )
}

/// Source residual: move/attack/hotkey selection paths use ui_selected_ids.
pub fn honesty_hotkey_move_attack_selection_presentation_only_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let checks = [
        (
            "move_fail_no_selection",
            "Wave 221: push presentation-first selection into host before move",
        ),
        (
            "attack_fail_no_selection",
            "Wave 221: selection count via presentation-first ui_selected_ids",
        ),
        (
            "MSG_META_TOGGLE_ATTACKMOVE",
            "Wave 221: selection via presentation-first ui_selected_ids",
        ),
        (
            "ForceAttackGround",
            "Wave 221: selection via presentation-first ui_selected_ids",
        ),
    ];
    for (anchor, note) in checks {
        let Some(i) = eng.find(anchor) else {
            return false;
        };
        // Notes may sit slightly before or after the status/message anchor.
        let lo = i.saturating_sub(1600);
        let hi = (i + 400).min(eng.len());
        if !eng[lo..hi].contains(note) && !eng[lo..hi].contains("ui_selected_ids") {
            return false;
        }
    }
    // A-key path must not dual-read get_player selected_objects.
    let Some(i) = eng.find("MSG_META_TOGGLE_ATTACKMOVE") else {
        return false;
    };
    let win = &eng[i..i.saturating_add(600).min(eng.len())];
    win.contains("ui_selected_ids") && !win.contains("get_player(self.current_player_id)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_hotkey_move_attack_selection_presentation_only_honesty() -> bool {
    honesty_live_hotkey_move_attack_selection_presentation_only_residual_pack_wave221()
        && honesty_hotkey_move_attack_selection_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_hotkey_move_attack_selection_presentation_only_method_names_residual_wave221(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_hotkey_move_attack_selection_presentation_only_nav_commands_residual_wave221(
            )
        );
    }

    #[test]
    fn wave221_composite_pack() {
        assert!(
            honesty_live_hotkey_move_attack_selection_presentation_only_residual_pack_wave221()
        );
    }

    #[test]
    fn hotkey_move_attack_sources() {
        assert!(honesty_hotkey_move_attack_selection_presentation_only_source());
    }

    #[test]
    fn simulate_live_hotkey_move_attack_selection_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_hotkey_move_attack_selection_presentation_only_honesty(),
            "hotkey/move/attack selection presentation-only residual must latch"
        );
    }
}
