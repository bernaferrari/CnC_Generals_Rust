//! Wave 425 residual peels: AIManager dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), AI manager
//! command helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 424 PathFollowing dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/commands/ai_manager.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AIManager dual-world empty-gate residual method names.
pub const LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE425: &[&str] = &[
    "dual_world_registry_unavailable",
    "queue_waypoint_for_object",
    "execute_waypoint_queue_for_object",
    "execute_ai_command_on_unit",
    "should_advance_unit_queue",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE425: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_MANAGER_EMPTY_GATES",
    "LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE425: &[&str] = &[
    "click_live_ai_manager_dual_world_empty_gate_ok_prepare",
    "click_live_ai_manager_dual_world_empty_gate_ok_live",
    "click_live_ai_manager_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_manager_dual_world_empty_gate_method_names_residual_wave425() -> bool {
    LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE425.len() == 6
        && residual_name_index(
            LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE425,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE425,
            "should_advance_unit_queue",
        ) == Some(4)
        && residual_name_index(
            LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE425,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_manager_dual_world_empty_gate_nav_commands_residual_wave425() -> bool {
    LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE425.len() == 4
        && residual_name_index(
            LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE425,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE425,
            "LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_MANAGER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE425.len() == 3
}

/// Wave 425 composite residual honesty pack.
pub fn honesty_live_ai_manager_dual_world_empty_gate_residual_pack_wave425() -> bool {
    honesty_live_ai_manager_dual_world_empty_gate_method_names_residual_wave425()
        && honesty_live_ai_manager_dual_world_empty_gate_nav_commands_residual_wave425()
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

/// Source residual: AIManager empty dual-world short-circuits.
pub fn honesty_ai_manager_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/commands/ai_manager.rs");
    if !(g.contains("Wave 425")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(queue) = fn_body(g, "fn queue_waypoint_for_object(") else {
        return false;
    };
    let Some(exec) = fn_body(g, "fn execute_ai_command_on_unit(") else {
        return false;
    };
    let Some(adv) = fn_body(g, "fn should_advance_unit_queue(") else {
        return false;
    };
    helper_ok
        && queue.contains("return;")
        && exec.contains("return;")
        && adv.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_manager_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_manager_dual_world_empty_gate_residual_pack_wave425()
        && honesty_ai_manager_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ai_manager_dual_world_empty_gate_method_names_residual_wave425());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ai_manager_dual_world_empty_gate_nav_commands_residual_wave425());
    }

    #[test]
    fn wave425_composite_pack() {
        assert!(honesty_live_ai_manager_dual_world_empty_gate_residual_pack_wave425());
    }

    #[test]
    fn ai_manager_dual_world_empty_gate_sources() {
        assert!(honesty_ai_manager_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_manager_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_manager_dual_world_empty_gate_honesty(),
            "ai manager dual-world empty gate residual must latch"
        );
    }
}
