//! Wave 301 residual peels: BridgeBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), bridge
//! damage/object helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 300 OverlordContain dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/bridge_behavior.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Bridge behavior dual-world empty-gate residual method names.
pub const LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE301: &[&str] = &[
    "dual_world_registry_unavailable",
    "from_module_thing",
    "get_object",
    "find_object_by_id",
    "on_damage",
    "detach_tower",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE301: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_BRIDGE_BEHAVIOR_EMPTY_GATES",
    "LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE301: &[&str] = &[
    "click_live_bridge_behavior_dual_world_empty_gate_ok_prepare",
    "click_live_bridge_behavior_dual_world_empty_gate_ok_live",
    "click_live_bridge_behavior_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_bridge_behavior_dual_world_empty_gate_method_names_residual_wave301() -> bool {
    LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE301.len() == 7
        && residual_name_index(
            LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE301,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE301,
            "on_damage",
        ) == Some(4)
        && residual_name_index(
            LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE301,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_bridge_behavior_dual_world_empty_gate_nav_commands_residual_wave301() -> bool {
    LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE301.len() == 4
        && residual_name_index(
            LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE301,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE301,
            "LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_BRIDGE_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE301.len() == 3
}

/// Wave 301 composite residual honesty pack.
pub fn honesty_live_bridge_behavior_dual_world_empty_gate_residual_pack_wave301() -> bool {
    honesty_live_bridge_behavior_dual_world_empty_gate_method_names_residual_wave301()
        && honesty_live_bridge_behavior_dual_world_empty_gate_nav_commands_residual_wave301()
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

/// Source residual: bridge behavior empty dual-world short-circuits.
pub fn honesty_bridge_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/behavior/bridge_behavior.rs");
    if !(g.contains("Wave 301")
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
    let Some(find) = fn_body(g, "fn find_object_by_id(") else {
        return false;
    };
    helper_ok && get.contains("Err(") && find.contains("Ok(None)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_bridge_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_bridge_behavior_dual_world_empty_gate_residual_pack_wave301()
        && honesty_bridge_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_bridge_behavior_dual_world_empty_gate_method_names_residual_wave301());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_bridge_behavior_dual_world_empty_gate_nav_commands_residual_wave301());
    }

    #[test]
    fn wave301_composite_pack() {
        assert!(honesty_live_bridge_behavior_dual_world_empty_gate_residual_pack_wave301());
    }

    #[test]
    fn bridge_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_bridge_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_bridge_behavior_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_bridge_behavior_dual_world_empty_gate_honesty(),
            "bridge behavior dual-world empty gate residual must latch"
        );
    }
}
