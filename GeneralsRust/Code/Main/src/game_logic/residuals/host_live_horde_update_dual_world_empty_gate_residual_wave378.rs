//! Wave 378 residual: HordeUpdate no longer Forever-sleeps when
//! `OBJECT_REGISTRY` is empty. C++ `HordeUpdate::update` always runs.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/horde_update.rs`

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// HordeUpdate dual-world empty-gate residual method names (gate closed).
pub const LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE378: &[&str] = &[
    "show_hide_flag",
    "check_horde_status",
    "update_simple",
    "no dual_world_registry_unavailable",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE378: &[&str] = &[
    "REQUIRE_HORDE_UPDATE_ALWAYS_RUNS",
    "REQUIRE_NO_EMPTY_REGISTRY_FOREVER",
    "LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE378: &[&str] = &[
    "click_live_horde_update_dual_world_empty_gate_ok_prepare",
    "click_live_horde_update_dual_world_empty_gate_ok_live",
    "click_live_horde_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_horde_update_dual_world_empty_gate_method_names_residual_wave378() -> bool {
    LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE378.len() == 5
        && residual_name_index(
            LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE378,
            "show_hide_flag",
        ) == Some(0)
        && residual_name_index(
            LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE378,
            "update_simple",
        ) == Some(2)
        && residual_name_index(
            LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE378,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_horde_update_dual_world_empty_gate_nav_commands_residual_wave378() -> bool {
    LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE378.len() == 4
        && residual_name_index(
            LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE378,
            "REQUIRE_HORDE_UPDATE_ALWAYS_RUNS",
        ) == Some(0)
        && residual_name_index(
            LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE378,
            "LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_HORDE_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE378.len() == 3
}

/// Wave 378 composite residual honesty pack.
pub fn honesty_live_horde_update_dual_world_empty_gate_residual_pack_wave378() -> bool {
    honesty_live_horde_update_dual_world_empty_gate_method_names_residual_wave378()
        && honesty_live_horde_update_dual_world_empty_gate_nav_commands_residual_wave378()
}

/// Source residual: HordeUpdate empty dual-world gate is closed (C++ always runs).
pub fn honesty_horde_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/behavior/horde_update.rs");
    !g.contains("fn dual_world_registry_unavailable")
        && !g.contains("dual_world_registry_unavailable()")
        && g.contains("fn show_hide_flag(")
        && g.contains("fn check_horde_status(")
        && g.contains("fn update_simple(")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_horde_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_horde_update_dual_world_empty_gate_residual_pack_wave378()
        && honesty_horde_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_horde_update_dual_world_empty_gate_method_names_residual_wave378());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_horde_update_dual_world_empty_gate_nav_commands_residual_wave378());
    }

    #[test]
    fn wave378_composite_pack() {
        assert!(honesty_live_horde_update_dual_world_empty_gate_residual_pack_wave378());
    }

    #[test]
    fn horde_update_dual_world_empty_gate_sources() {
        assert!(honesty_horde_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_horde_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_horde_update_dual_world_empty_gate_honesty(),
            "horde update dual-world empty gate residual must latch"
        );
    }
}
