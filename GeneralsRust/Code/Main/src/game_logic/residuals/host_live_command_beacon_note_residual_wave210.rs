//! Wave 210 residual peels: PlaceBeacon command notes host `recent_beacons`
//! via `note_beacon_placed` so presentation freezes new-beacon bloom without
//! relying solely on mid-frame beacon manager dual-read. Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 209 OS-input command path residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `execute_place_beacon` → `note_beacon_placed`
//! - `GameLogic::note_beacon_placed` / `recent_beacons`
//!
//! Fail-closed:
//! - Not full multiplayer beacon stream / GameWorld beacon entity store
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Beacon note residual method names.
pub const LIVE_COMMAND_BEACON_NOTE_METHOD_NAMES_WAVE210: &[&str] = &[
    "execute_place_beacon",
    "note_beacon_placed",
    "recent_beacons",
    "place_beacon",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_BEACON_NOTE_NAV_STEPS_WAVE210: &[&str] = &[
    "REQUIRE_PLACE_BEACON_NOTES_HOST",
    "REQUIRE_NOTE_BEACON_API",
    "LIVE_PLACE_BEACON_NOTES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_BEACON_NOTE_CMD_NAMES_WAVE210: &[&str] = &[
    "click_live_command_beacon_note_ok_prepare",
    "click_live_command_beacon_note_ok_live",
    "click_live_command_beacon_note_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_live_command_beacon_note_method_names_residual_wave210() -> bool {
    LIVE_COMMAND_BEACON_NOTE_METHOD_NAMES_WAVE210.len() == 5
        && residual_name_index(
            LIVE_COMMAND_BEACON_NOTE_METHOD_NAMES_WAVE210,
            "execute_place_beacon",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_BEACON_NOTE_METHOD_NAMES_WAVE210,
            "note_beacon_placed",
        ) == Some(1)
        && residual_name_index(
            LIVE_COMMAND_BEACON_NOTE_METHOD_NAMES_WAVE210,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_beacon_note_nav_commands_residual_wave210() -> bool {
    LIVE_COMMAND_BEACON_NOTE_NAV_STEPS_WAVE210.len() == 4
        && residual_name_index(
            LIVE_COMMAND_BEACON_NOTE_NAV_STEPS_WAVE210,
            "REQUIRE_PLACE_BEACON_NOTES_HOST",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_BEACON_NOTE_NAV_STEPS_WAVE210,
            "LIVE_PLACE_BEACON_NOTES",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_BEACON_NOTE_CMD_NAMES_WAVE210.len() == 3
}

/// Wave 210 composite residual honesty pack.
pub fn honesty_live_command_beacon_note_residual_pack_wave210() -> bool {
    honesty_live_command_beacon_note_method_names_residual_wave210()
        && honesty_live_command_beacon_note_nav_commands_residual_wave210()
}

/// Source residual: execute_place_beacon calls note_beacon_placed.
pub fn honesty_place_beacon_notes_host_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = match ce.find("fn execute_place_beacon") {
        Some(i) => i,
        None => return false,
    };
    let body = &ce[i..];
    body.contains("note_beacon_placed")
        && body.contains("place_beacon")
        && body.contains("try_eva_beacon_detected")
}

/// Source residual: note_beacon_placed pushes recent_beacons.
pub fn honesty_note_beacon_api_source() -> bool {
    let src = super::host_logic_scan_src();
    let i = match src.find("fn note_beacon_placed") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..];
    body.contains("recent_beacons.push")
}

/// Live residual: note_beacon_placed surfaces in recent_beacons drain.
pub fn simulate_live_command_beacon_note_honesty() -> bool {
    use crate::game_logic::GameLogic;
    use glam::Vec3;

    if !honesty_live_command_beacon_note_residual_pack_wave210() {
        return false;
    }
    if !honesty_place_beacon_notes_host_source() {
        return false;
    }
    if !honesty_note_beacon_api_source() {
        return false;
    }

    let mut logic = GameLogic::new();
    let pos = Vec3::new(11.0, 0.0, 22.0);
    logic.note_beacon_placed(pos);
    let recent = logic.recent_beacons();
    recent.iter().any(|p| (*p - pos).length() < 0.01)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_beacon_note_method_names_residual_wave210());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_beacon_note_nav_commands_residual_wave210());
    }

    #[test]
    fn wave210_composite_pack() {
        assert!(honesty_live_command_beacon_note_residual_pack_wave210());
    }

    #[test]
    fn beacon_note_sources() {
        assert!(honesty_place_beacon_notes_host_source());
        assert!(honesty_note_beacon_api_source());
    }

    #[test]
    fn simulate_live_command_beacon_note_honesty_residual_live() {
        assert!(
            simulate_live_command_beacon_note_honesty(),
            "beacon note residual must latch"
        );
    }
}
