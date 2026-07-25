//! Wave 281 residual peels: GameLogic helpers dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), lookup/
//! spatial helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 280 TunnelContain dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/helpers.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - register_object/remove_object remain active (populate/clear registry)
//! - select/deselect remain active (operate on provided object refs)

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Helpers dual-world empty-gate residual method names.
pub const LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE281: &[&str] = &[
    "dual_world_registry_unavailable",
    "find_object_by_id",
    "get_objects_in_range",
    "get_closest_object",
    "find_objects_in_radius",
    "try_infiltration_event_id",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE281: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_HELPERS_EMPTY_GATES",
    "LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE281: &[&str] = &[
    "click_live_helpers_dual_world_empty_gate_ok_prepare",
    "click_live_helpers_dual_world_empty_gate_ok_live",
    "click_live_helpers_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_helpers_dual_world_empty_gate_method_names_residual_wave281() -> bool {
    LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE281.len() == 7
        && residual_name_index(
            LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE281,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE281,
            "try_infiltration_event_id",
        ) == Some(5)
        && residual_name_index(
            LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE281,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_helpers_dual_world_empty_gate_nav_commands_residual_wave281() -> bool {
    LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE281.len() == 4
        && residual_name_index(
            LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE281,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE281,
            "LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_HELPERS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE281.len() == 3
}

/// Wave 281 composite residual honesty pack.
pub fn honesty_live_helpers_dual_world_empty_gate_residual_pack_wave281() -> bool {
    honesty_live_helpers_dual_world_empty_gate_method_names_residual_wave281()
        && honesty_live_helpers_dual_world_empty_gate_nav_commands_residual_wave281()
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

/// Source residual: helpers empty dual-world short-circuits.
pub fn honesty_helpers_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/helpers.rs");
    if !(g.contains("Wave 281")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(find) = fn_body(g, "fn find_object_by_id(") else {
        return false;
    };
    let Some(range) = fn_body(g, "fn get_objects_in_range(") else {
        return false;
    };
    let Some(closest) = fn_body(g, "fn get_closest_object(") else {
        return false;
    };
    // register_object must not early-return solely on empty dual-world
    let register_ok = !g.contains(
        "fn register_object(\n        object: std::sync::Arc<std::sync::RwLock<crate::object::Object>>,\n    ) -> Result<(), GameError> {\n        // Wave 281:",
    );
    helper_ok
        && find.contains("return None")
        && range.contains("Vec::new()")
        && closest.contains("return None")
        && register_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_helpers_dual_world_empty_gate_honesty() -> bool {
    honesty_live_helpers_dual_world_empty_gate_residual_pack_wave281()
        && honesty_helpers_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_helpers_dual_world_empty_gate_method_names_residual_wave281());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_helpers_dual_world_empty_gate_nav_commands_residual_wave281());
    }

    #[test]
    fn wave281_composite_pack() {
        assert!(honesty_live_helpers_dual_world_empty_gate_residual_pack_wave281());
    }

    #[test]
    fn helpers_dual_world_empty_gate_sources() {
        assert!(honesty_helpers_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_helpers_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_helpers_dual_world_empty_gate_honesty(),
            "helpers dual-world empty gate residual must latch"
        );
    }
}
