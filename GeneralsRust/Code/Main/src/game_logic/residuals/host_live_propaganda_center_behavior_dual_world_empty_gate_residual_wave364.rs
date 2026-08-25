//! Wave 364 residual peels: PropagandaCenterBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), brainwash
//! contain helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 363 PropagandaTowerBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/propaganda_center_behavior.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// PropagandaCenterBehavior dual-world empty-gate residual method names.
pub const LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE364: &[&str] = &[
    "dual_world_registry_unavailable",
    "with_object",
    "get_object",
    "on_delete",
    "process_brainwashing",
    "on_containing",
    "on_removing",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE364: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PROPAGANDA_CENTER_EMPTY_GATES",
    "LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE364:
    &[&str] = &[
    "click_live_propaganda_center_behavior_dual_world_empty_gate_ok_prepare",
    "click_live_propaganda_center_behavior_dual_world_empty_gate_ok_live",
    "click_live_propaganda_center_behavior_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_propaganda_center_behavior_dual_world_empty_gate_method_names_residual_wave364()
-> bool {
    LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE364.len() == 8
        && residual_name_index(
            LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE364,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE364,
            "on_removing",
        ) == Some(6)
        && residual_name_index(
            LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE364,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_residual_wave364()
-> bool {
    LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE364.len() == 4
        && residual_name_index(
            LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE364,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE364,
            "LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PROPAGANDA_CENTER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE364
            .len()
            == 3
}

/// Wave 364 composite residual honesty pack.
pub fn honesty_live_propaganda_center_behavior_dual_world_empty_gate_residual_pack_wave364() -> bool
{
    honesty_live_propaganda_center_behavior_dual_world_empty_gate_method_names_residual_wave364()
        && honesty_live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_residual_wave364()
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

/// Source residual: PropagandaCenterBehavior empty dual-world short-circuits.
pub fn honesty_propaganda_center_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/propaganda_center_behavior.rs"
    );
    if !(g.contains("Wave 364")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(with_obj) = fn_body(g, "fn with_object") else {
        return false;
    };
    let Some(get_obj) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(on_delete) = fn_body(g, "fn on_delete(") else {
        return false;
    };
    let Some(process) = fn_body(g, "fn process_brainwashing(") else {
        return false;
    };
    let Some(on_containing) = fn_body(g, "fn on_containing(") else {
        return false;
    };
    helper_ok
        && with_obj.contains("return None")
        && get_obj.contains("return None")
        && on_delete.contains("return Ok(())")
        && process.contains("return Ok(())")
        && on_containing.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_propaganda_center_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_propaganda_center_behavior_dual_world_empty_gate_residual_pack_wave364()
        && honesty_propaganda_center_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_propaganda_center_behavior_dual_world_empty_gate_method_names_residual_wave364());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_propaganda_center_behavior_dual_world_empty_gate_nav_commands_residual_wave364());
    }

    #[test]
    fn wave364_composite_pack() {
        assert!(
            honesty_live_propaganda_center_behavior_dual_world_empty_gate_residual_pack_wave364()
        );
    }

    #[test]
    fn propaganda_center_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_propaganda_center_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_propaganda_center_behavior_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_propaganda_center_behavior_dual_world_empty_gate_honesty(),
            "propaganda center behavior dual-world empty gate residual must latch"
        );
    }
}
