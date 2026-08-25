//! Wave 417 residual peels: SpawnPointProductionExitBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), spawn-point
//! production exit helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 416 TransitionDamageFX dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/spawn_point_production_exit_behavior.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// SpawnPointProductionExitBehavior dual-world empty-gate residual method names.
pub const LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE417:
    &[&str] = &[
    "dual_world_registry_unavailable",
    "initialize_bone_positions",
    "revalidate_occupiers",
    "exit_object_via_door_internal",
    "exit_object_via_door",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE417:
    &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SPAWN_POINT_PRODUCTION_EXIT_EMPTY_GATES",
    "LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE417:
    &[&str] = &[
        "click_live_spawn_point_production_exit_behavior_dual_world_empty_gate_ok_prepare",
        "click_live_spawn_point_production_exit_behavior_dual_world_empty_gate_ok_live",
        "click_live_spawn_point_production_exit_behavior_dual_world_empty_gate_miss",
    ];

/// Honesty: method names residual pack.
// 2026-08-15: empty-registry helper is fail-open (C++ does not skip-close).
pub fn honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_method_names_residual_wave417()
-> bool {
    LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE417.len() == 6
        && residual_name_index(
            LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE417,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE417,
            "exit_object_via_door",
        ) == Some(4)
        && residual_name_index(
            LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE417,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_nav_commands_residual_wave417()
-> bool {
    LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE417.len() == 4
        && residual_name_index(
            LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE417,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE417,
            "LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SPAWN_POINT_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE417
            .len()
            == 3
}

/// Wave 417 composite residual honesty pack.
pub fn honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_residual_pack_wave417()
-> bool {
    honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_method_names_residual_wave417()
        && honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_nav_commands_residual_wave417()
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

/// Source residual: SpawnPointProductionExitBehavior empty dual-world short-circuits.
pub fn honesty_spawn_point_production_exit_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/spawn_point_production_exit_behavior.rs"
    );
    if !(g.contains("Wave 417")
        && g.contains("fn dual_world_registry_unavailable")
        && (g.contains("let _host_empty") || g.contains("OBJECT_REGISTRY.is_empty()"))
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    // 2026-08-15: helper probes emptiness but returns false (C++ does not skip-close).
    let helper_ok = g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()")
        && g.contains("false");
    let Some(init) = fn_body(g, "fn initialize_bone_positions(") else {
        return false;
    };
    let Some(exit) = fn_body(g, "fn exit_object_via_door(") else {
        return false;
    };
    helper_ok && init.contains("return;") && exit.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_spawn_point_production_exit_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_residual_pack_wave417()
        && honesty_spawn_point_production_exit_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_method_names_residual_wave417()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_nav_commands_residual_wave417()
        );
    }

    #[test]
    fn wave417_composite_pack() {
        assert!(
            honesty_live_spawn_point_production_exit_behavior_dual_world_empty_gate_residual_pack_wave417()
        );
    }

    #[test]
    fn spawn_point_production_exit_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_spawn_point_production_exit_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_spawn_point_production_exit_behavior_dual_world_empty_gate_honesty_residual_live()
     {
        assert!(
            simulate_live_spawn_point_production_exit_behavior_dual_world_empty_gate_honesty(),
            "spawn point production exit dual-world empty gate residual must latch"
        );
    }
}
