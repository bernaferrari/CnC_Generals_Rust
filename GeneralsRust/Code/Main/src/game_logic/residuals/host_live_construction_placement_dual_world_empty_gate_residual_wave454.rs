//! Wave 454 residual peels: construction placement dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), object-collision
//! probes inside `validate_placement` are skipped after terrain/shroud checks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 453 upgrade/behavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/production/construction.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Terrain/off-map checks still run before the empty-registry skip

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Construction placement dual-world empty-gate residual method names.
pub const LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE454: &[&str] = &[
    "dual_world_registry_unavailable",
    "validate_placement",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE454: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PLACEMENT_COLLISION_SKIP",
    "LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE454:
    &[&str] = &[
    "click_live_construction_placement_dual_world_empty_gate_ok_prepare",
    "click_live_construction_placement_dual_world_empty_gate_ok_live",
    "click_live_construction_placement_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_construction_placement_dual_world_empty_gate_method_names_residual_wave454()
-> bool {
    LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE454.len() == 3
        && residual_name_index(
            LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE454,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE454,
            "validate_placement",
        ) == Some(1)
        && residual_name_index(
            LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE454,
            "playable_claim = false",
        ) == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_construction_placement_dual_world_empty_gate_nav_commands_residual_wave454()
-> bool {
    LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE454.len() == 4
        && residual_name_index(
            LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE454,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE454,
            "LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CONSTRUCTION_PLACEMENT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE454.len()
            == 3
}

/// Wave 454 composite residual honesty pack.
pub fn honesty_live_construction_placement_dual_world_empty_gate_residual_pack_wave454() -> bool {
    honesty_live_construction_placement_dual_world_empty_gate_method_names_residual_wave454()
        && honesty_live_construction_placement_dual_world_empty_gate_nav_commands_residual_wave454()
}

/// Source residual: construction placement empty dual-world short-circuits.
pub fn honesty_construction_placement_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/production/construction.rs");
    if !(g.contains("Wave 454")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    // Collision skip must be inside validate_placement and not replace terrain checks.
    let i = match g.find("fn validate_placement") {
        Some(i) => i,
        None => return false,
    };
    let body = &g[i..g.len().min(i + 12000)];
    let skip_ok = body.contains("if !dual_world_registry_unavailable()")
        && body.contains("for object_id in nearby");
    // Terrain check still present before skip
    let terrain_before = body.find("TheTerrainLogic").is_some()
        && body.find("TheTerrainLogic").unwrap()
            < body
                .find("if !dual_world_registry_unavailable()")
                .unwrap_or(usize::MAX);
    helper_ok && skip_ok && terrain_before
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_construction_placement_dual_world_empty_gate_honesty() -> bool {
    honesty_live_construction_placement_dual_world_empty_gate_residual_pack_wave454()
        && honesty_construction_placement_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_construction_placement_dual_world_empty_gate_method_names_residual_wave454(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_construction_placement_dual_world_empty_gate_nav_commands_residual_wave454(
            )
        );
    }

    #[test]
    fn wave454_composite_pack() {
        assert!(honesty_live_construction_placement_dual_world_empty_gate_residual_pack_wave454());
    }

    #[test]
    fn construction_placement_dual_world_empty_gate_sources() {
        assert!(honesty_construction_placement_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_construction_placement_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_construction_placement_dual_world_empty_gate_honesty(),
            "construction placement dual-world empty gate residual must latch"
        );
    }
}
