//! Wave 222 residual peels: `find_object_at_position` is presentation-only and
//! no longer accepts a live `&GameLogic` dual-read argument. Call sites (minimap,
//! mouse select/command) pass world pose + command_context only. Never flips
//! shell `playable_claim`.
//!
//! Orthogonal to Wave 221 hotkey/move/attack selection residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` find_object_at_position
//!
//! Fail-closed:
//! - Not full C++ selection radius / shroud pick matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Pick-object presentation-only residual method names.
pub const LIVE_PICK_OBJECT_PRESENTATION_ONLY_METHOD_NAMES_WAVE222: &[&str] = &[
    "find_object_at_position",
    "pick_object_id_at_world_from_presentation",
    "minimap",
    "mouse_select",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PICK_OBJECT_PRESENTATION_ONLY_NAV_STEPS_WAVE222: &[&str] = &[
    "REQUIRE_PICK_OBJECT_PRESENTATION_ONLY",
    "REQUIRE_NO_GAMELOGIC_ARG",
    "LIVE_PICK_OBJECT_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PICK_OBJECT_PRESENTATION_ONLY_CMD_NAMES_WAVE222: &[&str] = &[
    "click_live_pick_object_presentation_only_ok_prepare",
    "click_live_pick_object_presentation_only_ok_live",
    "click_live_pick_object_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_pick_object_presentation_only_method_names_residual_wave222() -> bool {
    LIVE_PICK_OBJECT_PRESENTATION_ONLY_METHOD_NAMES_WAVE222.len() == 5
        && residual_name_index(
            LIVE_PICK_OBJECT_PRESENTATION_ONLY_METHOD_NAMES_WAVE222,
            "find_object_at_position",
        ) == Some(0)
        && residual_name_index(
            LIVE_PICK_OBJECT_PRESENTATION_ONLY_METHOD_NAMES_WAVE222,
            "mouse_select",
        ) == Some(3)
        && residual_name_index(
            LIVE_PICK_OBJECT_PRESENTATION_ONLY_METHOD_NAMES_WAVE222,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_pick_object_presentation_only_nav_commands_residual_wave222() -> bool {
    LIVE_PICK_OBJECT_PRESENTATION_ONLY_NAV_STEPS_WAVE222.len() == 4
        && residual_name_index(
            LIVE_PICK_OBJECT_PRESENTATION_ONLY_NAV_STEPS_WAVE222,
            "REQUIRE_PICK_OBJECT_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_PICK_OBJECT_PRESENTATION_ONLY_NAV_STEPS_WAVE222,
            "LIVE_PICK_OBJECT_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PICK_OBJECT_PRESENTATION_ONLY_CMD_NAMES_WAVE222.len() == 3
}

/// Wave 222 composite residual honesty pack.
pub fn honesty_live_pick_object_presentation_only_residual_pack_wave222() -> bool {
    honesty_live_pick_object_presentation_only_method_names_residual_wave222()
        && honesty_live_pick_object_presentation_only_nav_commands_residual_wave222()
}

/// Source residual: find_object_at_position has no GameLogic parameter.
pub fn honesty_pick_object_presentation_only_source() -> bool {
    let eng = include_str!("../../cnc_game_engine.rs");
    let Some(i) = eng.find("fn find_object_at_position(") else {
        return false;
    };
    let rest = &eng[i..];
    // Stop at function body end (first `\n    }` after signature).
    let Some(sig_end) = rest.find('{') else {
        return false;
    };
    let after = &rest[sig_end..];
    let Some(close) = after.find("\n    }") else {
        return false;
    };
    let body = &rest[..sig_end + close + 6];
    body.contains("Wave 222")
        && body.contains("pick_object_id_at_world_from_presentation")
        && !body.contains("&GameLogic")
        && !body.contains("_game_logic")
        && eng.contains("find_object_at_position(clamped, true)")
        && !eng.contains("find_object_at_position(clamped, &self.game_logic")
        && !eng.contains("find_object_at_position(mouse_pos, &self.game_logic")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_pick_object_presentation_only_honesty() -> bool {
    honesty_live_pick_object_presentation_only_residual_pack_wave222()
        && honesty_pick_object_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_pick_object_presentation_only_method_names_residual_wave222());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_pick_object_presentation_only_nav_commands_residual_wave222());
    }

    #[test]
    fn wave222_composite_pack() {
        assert!(honesty_live_pick_object_presentation_only_residual_pack_wave222());
    }

    #[test]
    fn pick_object_sources() {
        assert!(honesty_pick_object_presentation_only_source());
    }

    #[test]
    fn simulate_live_pick_object_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_pick_object_presentation_only_honesty(),
            "pick-object presentation-only residual must latch"
        );
    }
}
