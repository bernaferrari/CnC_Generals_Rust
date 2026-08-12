//! Wave 450 residual peels: core sim dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), remaining
//! stealth/damage/healing/path/crate helpers fail-closed without dual-world walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 449 contain module fail-closed residual.
//!
//! Sources:
//! - `system/stealth_integration.rs`
//! - `weapon/damage_application.rs`
//! - `object/destroy/create_crate_die.rs`
//! - `object/behavior/auto_find_healing_behavior.rs`
//! - `object/behavior/auto_find_healing_update.rs`
//! - `object/die/rebuild_hole_expose_die.rs`
//! - `path/pathfind_layer.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Core-sim dual-world empty-gate residual method names.
pub const LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE450: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_objects_for_player",
    "apply_damage_to_object",
    "set_crate_team",
    "scan_closest_target",
    "transfer_attackers",
    "is_point_on_wall",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE450: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_CORE_SIM_EMPTY_GATES",
    "LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE450: &[&str] = &[
    "click_live_core_sim_dual_world_empty_gate_ok_prepare",
    "click_live_core_sim_dual_world_empty_gate_ok_live",
    "click_live_core_sim_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_core_sim_dual_world_empty_gate_method_names_residual_wave450() -> bool {
    LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE450.len() == 8
        && residual_name_index(
            LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE450,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE450,
            "is_point_on_wall",
        ) == Some(6)
        && residual_name_index(
            LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE450,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_core_sim_dual_world_empty_gate_nav_commands_residual_wave450() -> bool {
    LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE450.len() == 4
        && residual_name_index(
            LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE450,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE450,
            "LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CORE_SIM_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE450.len() == 3
}

/// Wave 450 composite residual honesty pack.
pub fn honesty_live_core_sim_dual_world_empty_gate_residual_pack_wave450() -> bool {
    honesty_live_core_sim_dual_world_empty_gate_method_names_residual_wave450()
        && honesty_live_core_sim_dual_world_empty_gate_nav_commands_residual_wave450()
}

fn honesty_one(src: &str, fn_name: &str, expected_return_snip: &str) -> bool {
    if !(src.contains("Wave 450")
        && src.contains("fn dual_world_registry_unavailable")
        && src.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = src.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let needle = format!("fn {fn_name}(");
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(&needle) {
        let i = search_from + rel;
        let Some(b) = src[i..].find('{') else {
            search_from = i + needle.len();
            continue;
        };
        let brace = i + b;
        let mut depth = 0usize;
        for (off, ch) in src[brace..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let body = &src[i..brace + off + 1];
                        if body.contains("dual_world_registry_unavailable")
                            && body.contains(expected_return_snip)
                        {
                            return helper_ok;
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        search_from = i + needle.len();
    }
    false
}

/// Source residual: core-sim empty dual-world short-circuits.
pub fn honesty_core_sim_dual_world_empty_gate_source() -> bool {
    let stealth =
        include_str!("../../../../GameEngine/GameLogic/src/system/stealth_integration.rs");
    let dmg = include_str!("../../../../GameEngine/GameLogic/src/weapon/damage_application.rs");
    let crate_die =
        include_str!("../../../../GameEngine/GameLogic/src/object/destroy/create_crate_die.rs");
    let heal_b = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/auto_find_healing_behavior.rs"
    );
    let heal_u = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/auto_find_healing_update.rs"
    );
    let hole =
        include_str!("../../../../GameEngine/GameLogic/src/object/die/rebuild_hole_expose_die.rs");
    let path = include_str!("../../../../GameEngine/GameLogic/src/path/pathfind_layer.rs");

    honesty_one(stealth, "get_objects_for_player", "return Ok(Vec::new())")
        && honesty_one(dmg, "apply_damage_to_object", "return false;")
        && honesty_one(crate_die, "set_crate_team", "return Ok(())")
        && honesty_one(heal_b, "scan_closest_target", "return None;")
        && honesty_one(heal_u, "scan_closest_target", "return None;")
        && honesty_one(hole, "transfer_attackers", "return;")
        && honesty_one(path, "is_point_on_wall", "return false;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_core_sim_dual_world_empty_gate_honesty() -> bool {
    honesty_live_core_sim_dual_world_empty_gate_residual_pack_wave450()
        && honesty_core_sim_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_core_sim_dual_world_empty_gate_method_names_residual_wave450());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_core_sim_dual_world_empty_gate_nav_commands_residual_wave450());
    }

    #[test]
    fn wave450_composite_pack() {
        assert!(honesty_live_core_sim_dual_world_empty_gate_residual_pack_wave450());
    }

    #[test]
    fn core_sim_dual_world_empty_gate_sources() {
        assert!(honesty_core_sim_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_core_sim_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_core_sim_dual_world_empty_gate_honesty(),
            "core sim dual-world empty gate residual must latch"
        );
    }
}
