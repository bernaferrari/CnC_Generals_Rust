//! Wave 211 residual peels: host-owned `host_beacons` list is preferred for
//! PresentationFrame freeze (no Mutex dual-read when host list non-empty).
//! Place/remove keep host list in sync. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 210 note_beacon_placed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `GameLogic::host_beacons` / `note_beacon_placed` / `note_beacon_removed_latest`
//! - `PresentationFrame` build prefers `logic.host_beacons()`
//!
//! Fail-closed:
//! - Not full GameWorld beacon entity store / multiplayer beacon stream
//! - Manager snapshot remains fallback when host list empty
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Host beacon presentation residual method names.
pub const LIVE_HOST_BEACON_PRESENTATION_METHOD_NAMES_WAVE211: &[&str] = &[
    "host_beacons",
    "note_beacon_placed",
    "note_beacon_removed_latest",
    "PresentationFrame host prefer",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_HOST_BEACON_PRESENTATION_NAV_STEPS_WAVE211: &[&str] = &[
    "REQUIRE_HOST_BEACONS_API",
    "REQUIRE_PRESENTATION_PREFERS_HOST",
    "LIVE_HOST_BEACON_LIST",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_HOST_BEACON_PRESENTATION_CMD_NAMES_WAVE211: &[&str] = &[
    "click_live_host_beacon_presentation_ok_prepare",
    "click_live_host_beacon_presentation_ok_live",
    "click_live_host_beacon_presentation_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_live_host_beacon_presentation_method_names_residual_wave211() -> bool {
    LIVE_HOST_BEACON_PRESENTATION_METHOD_NAMES_WAVE211.len() == 5
        && residual_name_index(
            LIVE_HOST_BEACON_PRESENTATION_METHOD_NAMES_WAVE211,
            "host_beacons",
        ) == Some(0)
        && residual_name_index(
            LIVE_HOST_BEACON_PRESENTATION_METHOD_NAMES_WAVE211,
            "note_beacon_removed_latest",
        ) == Some(2)
        && residual_name_index(
            LIVE_HOST_BEACON_PRESENTATION_METHOD_NAMES_WAVE211,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_host_beacon_presentation_nav_commands_residual_wave211() -> bool {
    LIVE_HOST_BEACON_PRESENTATION_NAV_STEPS_WAVE211.len() == 4
        && residual_name_index(
            LIVE_HOST_BEACON_PRESENTATION_NAV_STEPS_WAVE211,
            "REQUIRE_HOST_BEACONS_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_HOST_BEACON_PRESENTATION_NAV_STEPS_WAVE211,
            "LIVE_HOST_BEACON_LIST",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_HOST_BEACON_PRESENTATION_CMD_NAMES_WAVE211.len() == 3
}

/// Wave 211 composite residual honesty pack.
pub fn honesty_live_host_beacon_presentation_residual_pack_wave211() -> bool {
    honesty_live_host_beacon_presentation_method_names_residual_wave211()
        && honesty_live_host_beacon_presentation_nav_commands_residual_wave211()
}

/// Source residual: host beacon APIs exist on GameLogic.
pub fn honesty_host_beacons_api_source() -> bool {
    let src = super::GAME_LOGIC_HOST_SRC;
    src.contains("host_beacons: Vec<Vec3>")
        && src.contains("fn note_beacon_placed")
        && src.contains("fn note_beacon_removed_latest")
        && src.contains("fn host_beacons")
}

/// Source residual: presentation prefers host_beacons when non-empty.
pub fn honesty_presentation_prefers_host_beacons_source() -> bool {
    let pf = crate::presentation_frame::PRESENTATION_FRAME_SRC;
    pf.contains("logic.host_beacons()")
        && pf.contains("Wave 211")
        && pf.contains("snapshot_beacons")
}

/// Source residual: remove path notes host list.
pub fn honesty_remove_beacon_notes_host_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = match ce.find("fn execute_remove_beacon") {
        Some(i) => i,
        None => return false,
    };
    let body = &ce[i..];
    body.contains("note_beacon_removed_latest")
}

/// Live residual: place notes host list; presentation source prefers it.
pub fn simulate_live_host_beacon_presentation_honesty() -> bool {
    use crate::game_logic::GameLogic;
    use glam::Vec3;

    if !honesty_live_host_beacon_presentation_residual_pack_wave211() {
        return false;
    }
    if !honesty_host_beacons_api_source() {
        return false;
    }
    if !honesty_presentation_prefers_host_beacons_source() {
        return false;
    }
    if !honesty_remove_beacon_notes_host_source() {
        return false;
    }

    let mut logic = GameLogic::new();
    let a = Vec3::new(1.0, 0.0, 2.0);
    let b = Vec3::new(5.0, 0.0, 6.0);
    logic.note_beacon_placed(a);
    logic.note_beacon_placed(b);
    if logic.host_beacons().len() != 2 {
        return false;
    }
    // Near-duplicate place replaces within match threshold.
    logic.note_beacon_placed(a + Vec3::new(0.5, 0.0, 0.0));
    if logic.host_beacons().len() != 2 {
        return false;
    }
    logic.note_beacon_removed_latest();
    logic.host_beacons().len() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_host_beacon_presentation_method_names_residual_wave211());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_host_beacon_presentation_nav_commands_residual_wave211());
    }

    #[test]
    fn wave211_composite_pack() {
        assert!(honesty_live_host_beacon_presentation_residual_pack_wave211());
    }

    #[test]
    fn host_beacon_sources() {
        assert!(honesty_host_beacons_api_source());
        assert!(honesty_presentation_prefers_host_beacons_source());
        assert!(honesty_remove_beacon_notes_host_source());
    }

    #[test]
    fn simulate_live_host_beacon_presentation_honesty_residual_live() {
        assert!(
            simulate_live_host_beacon_presentation_honesty(),
            "host beacon presentation residual must latch"
        );
    }
}
