//! Wave 228 residual peels: RMB command classification prefers
//! `PresentationTargetHint` on `MouseCommandContext` when installed (no live
//! `get_object(target)` dual-read for target identity). Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 227 construct spawn pose residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `command_system.rs` classify_right_click_target_from_presentation
//! - `cnc_game_engine.rs` presentation_target_hint
//!
//! Fail-closed:
//! - Not full C++ RMB context matrix (service pads, hero abilities, …)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// RMB target presentation-only residual method names.
pub const LIVE_RMB_TARGET_PRESENTATION_ONLY_METHOD_NAMES_WAVE228: &[&str] = &[
    "PresentationTargetHint",
    "target_presentation",
    "classify_right_click_target_from_presentation",
    "presentation_target_hint",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_RMB_TARGET_PRESENTATION_ONLY_NAV_STEPS_WAVE228: &[&str] = &[
    "REQUIRE_RMB_TARGET_PRESENTATION_ONLY",
    "REQUIRE_TARGET_PRESENTATION_HINT",
    "LIVE_RMB_TARGET_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_RMB_TARGET_PRESENTATION_ONLY_CMD_NAMES_WAVE228: &[&str] = &[
    "click_live_rmb_target_presentation_only_ok_prepare",
    "click_live_rmb_target_presentation_only_ok_live",
    "click_live_rmb_target_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_rmb_target_presentation_only_method_names_residual_wave228() -> bool {
    LIVE_RMB_TARGET_PRESENTATION_ONLY_METHOD_NAMES_WAVE228.len() == 5
        && residual_name_index(
            LIVE_RMB_TARGET_PRESENTATION_ONLY_METHOD_NAMES_WAVE228,
            "PresentationTargetHint",
        ) == Some(0)
        && residual_name_index(
            LIVE_RMB_TARGET_PRESENTATION_ONLY_METHOD_NAMES_WAVE228,
            "presentation_target_hint",
        ) == Some(3)
        && residual_name_index(
            LIVE_RMB_TARGET_PRESENTATION_ONLY_METHOD_NAMES_WAVE228,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_rmb_target_presentation_only_nav_commands_residual_wave228() -> bool {
    LIVE_RMB_TARGET_PRESENTATION_ONLY_NAV_STEPS_WAVE228.len() == 4
        && residual_name_index(
            LIVE_RMB_TARGET_PRESENTATION_ONLY_NAV_STEPS_WAVE228,
            "REQUIRE_RMB_TARGET_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_RMB_TARGET_PRESENTATION_ONLY_NAV_STEPS_WAVE228,
            "LIVE_RMB_TARGET_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_RMB_TARGET_PRESENTATION_ONLY_CMD_NAMES_WAVE228.len() == 3
}

/// Wave 228 composite residual honesty pack.
pub fn honesty_live_rmb_target_presentation_only_residual_pack_wave228() -> bool {
    honesty_live_rmb_target_presentation_only_method_names_residual_wave228()
        && honesty_live_rmb_target_presentation_only_nav_commands_residual_wave228()
}

/// Source residual: hint type + classify path + engine helper.
pub fn honesty_rmb_target_presentation_only_source() -> bool {
    let cs = include_str!("../../command_system.rs");
    let eng = include_str!("../../cnc_game_engine.rs");
    cs.contains("struct PresentationTargetHint")
        && cs.contains("target_presentation: Option<PresentationTargetHint>")
        && cs.contains("fn classify_right_click_target_from_presentation")
        && cs.contains("Wave 228: prefer presentation-frozen target identity when installed")
        && eng.contains("fn presentation_target_hint")
        && eng.contains("target_object.and_then(|id| self.presentation_target_hint(id))")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_rmb_target_presentation_only_honesty() -> bool {
    honesty_live_rmb_target_presentation_only_residual_pack_wave228()
        && honesty_rmb_target_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_rmb_target_presentation_only_method_names_residual_wave228());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_rmb_target_presentation_only_nav_commands_residual_wave228());
    }

    #[test]
    fn wave228_composite_pack() {
        assert!(honesty_live_rmb_target_presentation_only_residual_pack_wave228());
    }

    #[test]
    fn rmb_target_sources() {
        assert!(honesty_rmb_target_presentation_only_source());
    }

    #[test]
    fn simulate_live_rmb_target_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_rmb_target_presentation_only_honesty(),
            "rmb target presentation-only residual must latch"
        );
    }
}
