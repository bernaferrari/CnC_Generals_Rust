//! Wave 312 residual peels: ProductionExitStrategy dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), spawn_unit
//! strategies fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 311 FlightDeck dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/production/exit_strategies.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Exit strategies dual-world empty-gate residual method names.
pub const LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE312: &[&str] = &[
    "dual_world_registry_unavailable",
    "spawn_unit_default",
    "spawn_unit_queue",
    "spawn_unit_supply_center",
    "spawn_unit_spawn_point",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE312: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_EXIT_STRATEGIES_EMPTY_GATES",
    "LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE312: &[&str] = &[
    "click_live_exit_strategies_dual_world_empty_gate_ok_prepare",
    "click_live_exit_strategies_dual_world_empty_gate_ok_live",
    "click_live_exit_strategies_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_exit_strategies_dual_world_empty_gate_method_names_residual_wave312() -> bool {
    LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE312.len() == 6
        && residual_name_index(
            LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE312,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE312,
            "spawn_unit_spawn_point",
        ) == Some(4)
        && residual_name_index(
            LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE312,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_exit_strategies_dual_world_empty_gate_nav_commands_residual_wave312() -> bool {
    LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE312.len() == 4
        && residual_name_index(
            LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE312,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE312,
            "LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_EXIT_STRATEGIES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE312.len() == 3
}

/// Wave 312 composite residual honesty pack.
pub fn honesty_live_exit_strategies_dual_world_empty_gate_residual_pack_wave312() -> bool {
    honesty_live_exit_strategies_dual_world_empty_gate_method_names_residual_wave312()
        && honesty_live_exit_strategies_dual_world_empty_gate_nav_commands_residual_wave312()
}

fn count_spawn_gates(src: &str) -> usize {
    let needle = "Wave 312: empty dual-world";
    src.matches(needle).count()
}

/// Source residual: exit strategies empty dual-world short-circuits.
pub fn honesty_exit_strategies_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/production/exit_strategies.rs");
    if !(g.contains("Wave 312")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    helper_ok
        && count_spawn_gates(g) == 4
        && g.contains("dual-world object registry unavailable")
        && g.contains("impl ProductionExitStrategy for DefaultProductionExit")
        && g.contains("impl ProductionExitStrategy for QueueProductionExit")
        && g.contains("impl ProductionExitStrategy for SupplyCenterProductionExit")
        && g.contains("impl ProductionExitStrategy for SpawnPointProductionExit")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_exit_strategies_dual_world_empty_gate_honesty() -> bool {
    honesty_live_exit_strategies_dual_world_empty_gate_residual_pack_wave312()
        && honesty_exit_strategies_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_exit_strategies_dual_world_empty_gate_method_names_residual_wave312());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_exit_strategies_dual_world_empty_gate_nav_commands_residual_wave312());
    }

    #[test]
    fn wave312_composite_pack() {
        assert!(honesty_live_exit_strategies_dual_world_empty_gate_residual_pack_wave312());
    }

    #[test]
    fn exit_strategies_dual_world_empty_gate_sources() {
        assert!(honesty_exit_strategies_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_exit_strategies_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_exit_strategies_dual_world_empty_gate_honesty(),
            "exit strategies dual-world empty gate residual must latch"
        );
    }
}
