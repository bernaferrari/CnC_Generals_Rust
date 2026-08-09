//! Wave 343 residual peels: script evaluator dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), condition
//! evaluators fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 342 SpecialPowerTemplate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/scripting/evaluator.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Script evaluator dual-world empty-gate residual method names.
pub const LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE343: &[&str] = &[
    "dual_world_registry_unavailable",
    "evaluate_condition",
    "evaluate_named_destroyed_condition",
    "evaluate_special_power_condition",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE343: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SCRIPT_EVALUATOR_EMPTY_GATES",
    "LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE343: &[&str] = &[
    "click_live_script_evaluator_dual_world_empty_gate_ok_prepare",
    "click_live_script_evaluator_dual_world_empty_gate_ok_live",
    "click_live_script_evaluator_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_script_evaluator_dual_world_empty_gate_method_names_residual_wave343() -> bool {
    LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE343.len() == 5
        && residual_name_index(
            LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE343,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE343,
            "evaluate_special_power_condition",
        ) == Some(3)
        && residual_name_index(
            LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE343,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_script_evaluator_dual_world_empty_gate_nav_commands_residual_wave343() -> bool {
    LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE343.len() == 4
        && residual_name_index(
            LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE343,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE343,
            "LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SCRIPT_EVALUATOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE343.len() == 3
}

/// Wave 343 composite residual honesty pack.
pub fn honesty_live_script_evaluator_dual_world_empty_gate_residual_pack_wave343() -> bool {
    honesty_live_script_evaluator_dual_world_empty_gate_method_names_residual_wave343()
        && honesty_live_script_evaluator_dual_world_empty_gate_nav_commands_residual_wave343()
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

/// Source residual: script evaluator empty dual-world short-circuits.
pub fn honesty_script_evaluator_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/scripting/evaluator.rs");
    if !(g.contains("Wave 343")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(cond) = fn_body(g, "fn evaluate_condition(") else {
        return false;
    };
    let Some(dest) = fn_body(g, "fn evaluate_named_destroyed_condition(") else {
        return false;
    };
    let Some(sp) = fn_body(g, "fn evaluate_special_power_condition(") else {
        return false;
    };
    helper_ok
        && cond.contains("return Ok(false)")
        && dest.contains("return Ok(false)")
        && sp.contains("return Ok(false)")
        // Broad coverage of nested named/team condition evaluators.
        && g.matches("Wave 343: empty dual-world → Ok(false).").count() >= 20
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_script_evaluator_dual_world_empty_gate_honesty() -> bool {
    honesty_live_script_evaluator_dual_world_empty_gate_residual_pack_wave343()
        && honesty_script_evaluator_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_script_evaluator_dual_world_empty_gate_method_names_residual_wave343()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_script_evaluator_dual_world_empty_gate_nav_commands_residual_wave343()
        );
    }

    #[test]
    fn wave343_composite_pack() {
        assert!(honesty_live_script_evaluator_dual_world_empty_gate_residual_pack_wave343());
    }

    #[test]
    fn script_evaluator_dual_world_empty_gate_sources() {
        assert!(honesty_script_evaluator_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_script_evaluator_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_script_evaluator_dual_world_empty_gate_honesty(),
            "script evaluator dual-world empty gate residual must latch"
        );
    }
}
