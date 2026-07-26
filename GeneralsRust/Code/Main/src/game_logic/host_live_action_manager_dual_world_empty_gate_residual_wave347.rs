//! Wave 347 residual peels: ActionManager dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), action
//! validation helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 346 SpawnBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/action_manager.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// ActionManager dual-world empty-gate residual method names.
pub const LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE347: &[&str] = &[
    "dual_world_registry_unavailable",
    "count_special_objects_by_producer",
    "can_enter_object",
    "can_pick_up_prisoner_by_id",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE347: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_ACTION_MANAGER_EMPTY_GATES",
    "LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE347: &[&str] = &[
    "click_live_action_manager_dual_world_empty_gate_ok_prepare",
    "click_live_action_manager_dual_world_empty_gate_ok_live",
    "click_live_action_manager_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_action_manager_dual_world_empty_gate_method_names_residual_wave347() -> bool {
    LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE347.len() == 5
        && residual_name_index(
            LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE347,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE347,
            "can_pick_up_prisoner_by_id",
        ) == Some(3)
        && residual_name_index(
            LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE347,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_action_manager_dual_world_empty_gate_nav_commands_residual_wave347() -> bool {
    LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE347.len() == 4
        && residual_name_index(
            LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE347,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE347,
            "LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ACTION_MANAGER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE347.len() == 3
}

/// Wave 347 composite residual honesty pack.
pub fn honesty_live_action_manager_dual_world_empty_gate_residual_pack_wave347() -> bool {
    honesty_live_action_manager_dual_world_empty_gate_method_names_residual_wave347()
        && honesty_live_action_manager_dual_world_empty_gate_nav_commands_residual_wave347()
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

/// Source residual: ActionManager empty dual-world short-circuits.
pub fn honesty_action_manager_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/action_manager.rs");
    if !(g.contains("Wave 347")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(count) = fn_body(g, "fn count_special_objects_by_producer(") else {
        return false;
    };
    let Some(enter) = fn_body(g, "fn can_enter_object(") else {
        return false;
    };
    let Some(prisoner) = fn_body(g, "fn can_pick_up_prisoner_by_id(") else {
        return false;
    };
    helper_ok
        && count.contains("return 0")
        && enter.contains("return false")
        && prisoner.contains("return false")
        && g.contains("fn can_hijack_vehicle")
        && g.contains("fn can_sabotage_building")
        && g.contains("fn has_special_object_on_target")
        // no duplicate empty checks left beside the helper
        && g.matches("OBJECT_REGISTRY.is_empty()").count() == 1
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_action_manager_dual_world_empty_gate_honesty() -> bool {
    honesty_live_action_manager_dual_world_empty_gate_residual_pack_wave347()
        && honesty_action_manager_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_action_manager_dual_world_empty_gate_method_names_residual_wave347());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_action_manager_dual_world_empty_gate_nav_commands_residual_wave347());
    }

    #[test]
    fn wave347_composite_pack() {
        assert!(honesty_live_action_manager_dual_world_empty_gate_residual_pack_wave347());
    }

    #[test]
    fn action_manager_dual_world_empty_gate_sources() {
        assert!(honesty_action_manager_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_action_manager_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_action_manager_dual_world_empty_gate_honesty(),
            "action manager dual-world empty gate residual must latch"
        );
    }
}
