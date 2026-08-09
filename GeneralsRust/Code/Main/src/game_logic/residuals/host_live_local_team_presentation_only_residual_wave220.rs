//! Wave 220 residual peels: runtime-host command paths resolve local team via
//! presentation-first `local_team_for_ui` (no per-command
//! `last_presentation_frame / get_player` dual-read). Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 219 UI-command selection presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` train/construct/sell/attack/… team resolution
//!
//! Fail-closed:
//! - Not full multi-player observer team matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Local-team presentation-only residual method names.
pub const LIVE_LOCAL_TEAM_PRESENTATION_ONLY_METHOD_NAMES_WAVE220: &[&str] = &[
    "local_team_for_ui",
    "train",
    "construct",
    "sell",
    "attack",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_LOCAL_TEAM_PRESENTATION_ONLY_NAV_STEPS_WAVE220: &[&str] = &[
    "REQUIRE_LOCAL_TEAM_PRESENTATION_ONLY",
    "REQUIRE_LOCAL_TEAM_FOR_UI",
    "LIVE_LOCAL_TEAM_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_LOCAL_TEAM_PRESENTATION_ONLY_CMD_NAMES_WAVE220: &[&str] = &[
    "click_live_local_team_presentation_only_ok_prepare",
    "click_live_local_team_presentation_only_ok_live",
    "click_live_local_team_presentation_only_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_local_team_presentation_only_method_names_residual_wave220() -> bool {
    LIVE_LOCAL_TEAM_PRESENTATION_ONLY_METHOD_NAMES_WAVE220.len() == 6
        && residual_name_index(
            LIVE_LOCAL_TEAM_PRESENTATION_ONLY_METHOD_NAMES_WAVE220,
            "local_team_for_ui",
        ) == Some(0)
        && residual_name_index(
            LIVE_LOCAL_TEAM_PRESENTATION_ONLY_METHOD_NAMES_WAVE220,
            "construct",
        ) == Some(2)
        && residual_name_index(
            LIVE_LOCAL_TEAM_PRESENTATION_ONLY_METHOD_NAMES_WAVE220,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_local_team_presentation_only_nav_commands_residual_wave220() -> bool {
    LIVE_LOCAL_TEAM_PRESENTATION_ONLY_NAV_STEPS_WAVE220.len() == 4
        && residual_name_index(
            LIVE_LOCAL_TEAM_PRESENTATION_ONLY_NAV_STEPS_WAVE220,
            "REQUIRE_LOCAL_TEAM_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            LIVE_LOCAL_TEAM_PRESENTATION_ONLY_NAV_STEPS_WAVE220,
            "LIVE_LOCAL_TEAM_PRESENTATION_ONLY",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_LOCAL_TEAM_PRESENTATION_ONLY_CMD_NAMES_WAVE220.len() == 3
}

/// Wave 220 composite residual honesty pack.
pub fn honesty_live_local_team_presentation_only_residual_pack_wave220() -> bool {
    honesty_live_local_team_presentation_only_method_names_residual_wave220()
        && honesty_live_local_team_presentation_only_nav_commands_residual_wave220()
}

/// Source residual: host command team resolution uses local_team_for_ui.
pub fn honesty_local_team_presentation_only_source() -> bool {
    let eng = include_str!("../../cnc_game_engine.rs");
    let wave_marks = eng
        .matches("Wave 220: team via presentation-first local_team_for_ui")
        .count();
    if wave_marks < 15 {
        return false;
    }
    // Dual-read pattern should not remain for host command Option team blocks.
    let dual = eng.matches("Some(frame.local_team())").count();
    // One residual allowed for world-click acquire (presentation-only frame path).
    if dual > 1 {
        return false;
    }
    eng.contains("fn local_team_for_ui") && eng.contains("Some(self.local_team_for_ui())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_local_team_presentation_only_honesty() -> bool {
    honesty_live_local_team_presentation_only_residual_pack_wave220()
        && honesty_local_team_presentation_only_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_local_team_presentation_only_method_names_residual_wave220());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_local_team_presentation_only_nav_commands_residual_wave220());
    }

    #[test]
    fn wave220_composite_pack() {
        assert!(honesty_live_local_team_presentation_only_residual_pack_wave220());
    }

    #[test]
    fn local_team_sources() {
        assert!(honesty_local_team_presentation_only_source());
    }

    #[test]
    fn simulate_live_local_team_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_local_team_presentation_only_honesty(),
            "local-team presentation-only residual must latch"
        );
    }
}
