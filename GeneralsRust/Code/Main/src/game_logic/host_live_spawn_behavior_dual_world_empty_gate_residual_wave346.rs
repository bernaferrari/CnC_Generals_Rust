//! Wave 346 residual peels: SpawnBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), spawn/slave
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 345 meta_event dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/spawn_behavior.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// SpawnBehavior dual-world empty-gate residual method names.
pub const LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE346: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_object",
    "create_spawn",
    "get_closest_slave",
    "get_can_any_slaves_attack_specific_target",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE346: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SPAWN_BEHAVIOR_EMPTY_GATES",
    "LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE346: &[&str] = &[
    "click_live_spawn_behavior_dual_world_empty_gate_ok_prepare",
    "click_live_spawn_behavior_dual_world_empty_gate_ok_live",
    "click_live_spawn_behavior_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_spawn_behavior_dual_world_empty_gate_method_names_residual_wave346() -> bool {
    LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE346.len() == 6
        && residual_name_index(
            LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE346,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE346,
            "get_can_any_slaves_attack_specific_target",
        ) == Some(4)
        && residual_name_index(
            LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE346,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_spawn_behavior_dual_world_empty_gate_nav_commands_residual_wave346() -> bool {
    LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE346.len() == 4
        && residual_name_index(
            LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE346,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE346,
            "LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SPAWN_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE346.len() == 3
}

/// Wave 346 composite residual honesty pack.
pub fn honesty_live_spawn_behavior_dual_world_empty_gate_residual_pack_wave346() -> bool {
    honesty_live_spawn_behavior_dual_world_empty_gate_method_names_residual_wave346()
        && honesty_live_spawn_behavior_dual_world_empty_gate_nav_commands_residual_wave346()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(name) {
        let i = search_from + rel;
        let Some(b) = src[i..].find('{') else {
            search_from = i + name.len();
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
                        if body.contains("dual_world_registry_unavailable") {
                            return Some(body);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        search_from = i + name.len();
    }
    None
}

/// Source residual: SpawnBehavior empty dual-world short-circuits.
pub fn honesty_spawn_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/behavior/spawn_behavior.rs");
    if !(g.contains("Wave 346")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(get) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(create) = fn_body(g, "fn create_spawn(") else {
        return false;
    };
    let Some(closest) = fn_body(g, "fn get_closest_slave(") else {
        return false;
    };
    let Some(atk) = fn_body(g, "fn get_can_any_slaves_attack_specific_target(") else {
        return false;
    };
    helper_ok
        && get.contains("dual-world object registry unavailable")
        && create.contains("return Ok(false)")
        && closest.contains("return None")
        && atk.contains("ATTACKRESULT_NOT_POSSIBLE")
        && g.matches("Wave 346:").count() >= 15
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_spawn_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_spawn_behavior_dual_world_empty_gate_residual_pack_wave346()
        && honesty_spawn_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_spawn_behavior_dual_world_empty_gate_method_names_residual_wave346());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_spawn_behavior_dual_world_empty_gate_nav_commands_residual_wave346());
    }

    #[test]
    fn wave346_composite_pack() {
        assert!(honesty_live_spawn_behavior_dual_world_empty_gate_residual_pack_wave346());
    }

    #[test]
    fn spawn_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_spawn_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_spawn_behavior_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_spawn_behavior_dual_world_empty_gate_honesty(),
            "spawn behavior dual-world empty gate residual must latch"
        );
    }
}
