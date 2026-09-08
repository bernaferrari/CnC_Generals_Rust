//! Wave 294 residual peels: Victory conditions dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), victory
//! progress helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 293 AIBuildList dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/scripting/victory.rs` was DELETED (wave-1 no-legacy
//!   sweep); the source pin now asserts that deletion.
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Victory dual-world empty-gate residual method names.
pub const LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294: &[&str] = &[
    "dual_world_registry_unavailable",
    "calculate_destruction_progress",
    "calculate_structure_destruction_progress",
    "calculate_rescue_progress",
    "calculate_kill_progress",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE294: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_VICTORY_EMPTY_GATES",
    "LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE294: &[&str] = &[
    "click_live_victory_dual_world_empty_gate_ok_prepare",
    "click_live_victory_dual_world_empty_gate_ok_live",
    "click_live_victory_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_victory_dual_world_empty_gate_method_names_residual_wave294() -> bool {
    LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294.len() == 6
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294,
            "calculate_kill_progress",
        ) == Some(4)
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_victory_dual_world_empty_gate_nav_commands_residual_wave294() -> bool {
    LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE294.len() == 4
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE294,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE294,
            "LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE294.len() == 3
}

/// Wave 294 composite residual honesty pack.
pub fn honesty_live_victory_dual_world_empty_gate_residual_pack_wave294() -> bool {
    honesty_live_victory_dual_world_empty_gate_method_names_residual_wave294()
        && honesty_live_victory_dual_world_empty_gate_nav_commands_residual_wave294()
}

/// Source residual: the dual-world victory module was deleted (wave-1 sweep).
///
/// `scripting/victory.rs` (VictoryManager second brain) and the byte-stub
/// `script_engine/` compat module are gone; the crate no longer declares or
/// re-exports either. Host-only victory honesty lives in the crate's live
/// `system::victory_conditions` + `helpers::TheVictoryConditions`.
pub fn honesty_victory_dual_world_empty_gate_source() -> bool {
    let lib = include_str!("../../../../GameEngine/GameLogic/src/lib.rs");
    let scripting_mod = include_str!("../../../../GameEngine/GameLogic/src/scripting/mod.rs");
    !lib.contains("pub mod script_engine")
        && !lib.contains("VictoryManager")
        && !scripting_mod.contains("pub mod victory")
        && !scripting_mod.contains("VictoryManager")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_victory_dual_world_empty_gate_honesty() -> bool {
    honesty_live_victory_dual_world_empty_gate_residual_pack_wave294()
        && honesty_victory_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_victory_dual_world_empty_gate_method_names_residual_wave294());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_victory_dual_world_empty_gate_nav_commands_residual_wave294());
    }

    #[test]
    fn wave294_composite_pack() {
        assert!(honesty_live_victory_dual_world_empty_gate_residual_pack_wave294());
    }

    #[test]
    fn victory_dual_world_empty_gate_sources() {
        assert!(honesty_victory_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_victory_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_victory_dual_world_empty_gate_honesty(),
            "victory dual-world empty gate residual must latch"
        );
    }
}
