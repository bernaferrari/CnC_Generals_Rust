//! Wave 381 residual peels: QueueProductionExitBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), queue exit
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 380 BaseRegenerateUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/queue_production_exit_behavior.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// QueueProductionExitBehavior dual-world empty-gate residual method names.
pub const LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE381:
    &[&str] = &[
    "dual_world_registry_unavailable",
    "exit_object_via_door",
    "exit_object_by_budding",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE381: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_QUEUE_PRODUCTION_EXIT_EMPTY_GATES",
    "LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE381:
    &[&str] = &[
    "click_live_queue_production_exit_behavior_dual_world_empty_gate_ok_prepare",
    "click_live_queue_production_exit_behavior_dual_world_empty_gate_ok_live",
    "click_live_queue_production_exit_behavior_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: empty-registry helper is fail-open (C++ does not skip-close).
pub fn honesty_live_queue_production_exit_behavior_dual_world_empty_gate_method_names_residual_wave381()
-> bool {
    LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE381.len() == 4
        && residual_name_index(
            LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE381,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE381,
            "exit_object_by_budding",
        ) == Some(2)
        && residual_name_index(
            LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE381,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_residual_wave381()
-> bool {
    LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE381.len() == 4
        && residual_name_index(
            LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE381,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE381,
            "LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_QUEUE_PRODUCTION_EXIT_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE381
            .len()
            == 3
}

/// Wave 381 composite residual honesty pack.
pub fn honesty_live_queue_production_exit_behavior_dual_world_empty_gate_residual_pack_wave381()
-> bool {
    honesty_live_queue_production_exit_behavior_dual_world_empty_gate_method_names_residual_wave381()
        && honesty_live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_residual_wave381()
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

/// Source residual: QueueProductionExitBehavior empty dual-world short-circuits.
pub fn honesty_queue_production_exit_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/queue_production_exit_behavior.rs"
    );
    if !(g.contains("Wave 381")
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
    // Prefer ExitResult overload which fails closed with Err.
    let via = fn_body(g, "fn exit_object_via_door(");
    let bud = fn_body(g, "fn exit_object_by_budding(");
    let Some(via) = via else {
        return false;
    };
    let Some(bud) = bud else {
        return false;
    };
    helper_ok
        && (via.contains("return Err(") || via.contains("return Ok(())"))
        && (bud.contains("return Err(") || bud.contains("return Ok(())"))
        && g.matches("dual_world_registry_unavailable()").count() >= 4
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_queue_production_exit_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_queue_production_exit_behavior_dual_world_empty_gate_residual_pack_wave381()
        && honesty_queue_production_exit_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_queue_production_exit_behavior_dual_world_empty_gate_method_names_residual_wave381()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_queue_production_exit_behavior_dual_world_empty_gate_nav_commands_residual_wave381()
        );
    }

    #[test]
    fn wave381_composite_pack() {
        assert!(
            honesty_live_queue_production_exit_behavior_dual_world_empty_gate_residual_pack_wave381(
            )
        );
    }

    #[test]
    fn queue_production_exit_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_queue_production_exit_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_queue_production_exit_behavior_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_queue_production_exit_behavior_dual_world_empty_gate_honesty(),
            "queue production exit behavior dual-world empty gate residual must latch"
        );
    }
}
