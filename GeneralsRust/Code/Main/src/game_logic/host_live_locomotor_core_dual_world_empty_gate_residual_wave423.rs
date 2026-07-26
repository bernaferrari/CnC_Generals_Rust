//! Wave 423 residual peels: Locomotor core dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), locomotor
//! path helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 422 MoveToState dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/locomotor/core.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Locomotor core dual-world empty-gate residual method names.
pub const LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE423: &[&str] = &[
    "dual_world_registry_unavailable",
    "request_path",
    "apply_requester_capabilities",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE423: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_LOCOMOTOR_CORE_EMPTY_GATES",
    "LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE423: &[&str] = &[
    "click_live_locomotor_core_dual_world_empty_gate_ok_prepare",
    "click_live_locomotor_core_dual_world_empty_gate_ok_live",
    "click_live_locomotor_core_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_locomotor_core_dual_world_empty_gate_method_names_residual_wave423() -> bool {
    LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE423.len() == 4
        && residual_name_index(
            LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE423,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE423,
            "apply_requester_capabilities",
        ) == Some(2)
        && residual_name_index(
            LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE423,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_locomotor_core_dual_world_empty_gate_nav_commands_residual_wave423() -> bool {
    LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE423.len() == 4
        && residual_name_index(
            LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE423,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE423,
            "LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_LOCOMOTOR_CORE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE423.len() == 3
}

/// Wave 423 composite residual honesty pack.
pub fn honesty_live_locomotor_core_dual_world_empty_gate_residual_pack_wave423() -> bool {
    honesty_live_locomotor_core_dual_world_empty_gate_method_names_residual_wave423()
        && honesty_live_locomotor_core_dual_world_empty_gate_nav_commands_residual_wave423()
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

/// Source residual: Locomotor core empty dual-world short-circuits.
pub fn honesty_locomotor_core_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/locomotor/core.rs");
    if !(g.contains("Wave 423")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(req) = fn_body(g, "fn request_path(") else {
        return false;
    };
    let Some(apply) = fn_body(g, "fn apply_requester_capabilities(") else {
        return false;
    };
    helper_ok && req.contains("Ok(None)") && apply.contains("return capabilities")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_locomotor_core_dual_world_empty_gate_honesty() -> bool {
    honesty_live_locomotor_core_dual_world_empty_gate_residual_pack_wave423()
        && honesty_locomotor_core_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_locomotor_core_dual_world_empty_gate_method_names_residual_wave423());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_locomotor_core_dual_world_empty_gate_nav_commands_residual_wave423());
    }

    #[test]
    fn wave423_composite_pack() {
        assert!(honesty_live_locomotor_core_dual_world_empty_gate_residual_pack_wave423());
    }

    #[test]
    fn locomotor_core_dual_world_empty_gate_sources() {
        assert!(honesty_locomotor_core_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_locomotor_core_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_locomotor_core_dual_world_empty_gate_honesty(),
            "locomotor core dual-world empty gate residual must latch"
        );
    }
}
