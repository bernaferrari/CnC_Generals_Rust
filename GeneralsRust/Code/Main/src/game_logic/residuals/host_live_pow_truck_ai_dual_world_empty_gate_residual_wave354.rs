//! Wave 354 residual peels: POWTruckAIUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), prisoner
//! pickup/return helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 353 SpecialPowerModule dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/pow_truck_ai_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// POWTruckAI dual-world empty-gate residual method names.
pub const LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE354: &[&str] = &[
    "dual_world_registry_unavailable",
    "handle_pick_up_prisoner",
    "find_best_target",
    "validate_target",
    "unload_prisoners_to_prison",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE354: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_POW_TRUCK_AI_EMPTY_GATES",
    "LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE354: &[&str] = &[
    "click_live_pow_truck_ai_dual_world_empty_gate_ok_prepare",
    "click_live_pow_truck_ai_dual_world_empty_gate_ok_live",
    "click_live_pow_truck_ai_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_pow_truck_ai_dual_world_empty_gate_method_names_residual_wave354() -> bool {
    LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE354.len() == 6
        && residual_name_index(
            LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE354,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE354,
            "unload_prisoners_to_prison",
        ) == Some(4)
        && residual_name_index(
            LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE354,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_pow_truck_ai_dual_world_empty_gate_nav_commands_residual_wave354() -> bool {
    LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE354.len() == 4
        && residual_name_index(
            LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE354,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE354,
            "LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_POW_TRUCK_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE354.len() == 3
}

/// Wave 354 composite residual honesty pack.
pub fn honesty_live_pow_truck_ai_dual_world_empty_gate_residual_pack_wave354() -> bool {
    honesty_live_pow_truck_ai_dual_world_empty_gate_method_names_residual_wave354()
        && honesty_live_pow_truck_ai_dual_world_empty_gate_nav_commands_residual_wave354()
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

/// Source residual: POWTruckAI empty dual-world short-circuits.
pub fn honesty_pow_truck_ai_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/pow_truck_ai_update.rs");
    if !(g.contains("Wave 354")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(pick) = fn_body(g, "fn handle_pick_up_prisoner(") else {
        return false;
    };
    let Some(best) = fn_body(g, "fn find_best_target(") else {
        return false;
    };
    let Some(val) = fn_body(g, "fn validate_target(") else {
        return false;
    };
    let Some(unload) = fn_body(g, "fn unload_prisoners_to_prison(") else {
        return false;
    };
    helper_ok
        && pick.contains("return Ok(())")
        && best.contains("return None")
        && val.contains("dual-world object registry unavailable")
        && unload.contains("return;")
        && g.matches("Wave 354:").count() >= 10
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_pow_truck_ai_dual_world_empty_gate_honesty() -> bool {
    honesty_live_pow_truck_ai_dual_world_empty_gate_residual_pack_wave354()
        && honesty_pow_truck_ai_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_pow_truck_ai_dual_world_empty_gate_method_names_residual_wave354());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_pow_truck_ai_dual_world_empty_gate_nav_commands_residual_wave354());
    }

    #[test]
    fn wave354_composite_pack() {
        assert!(honesty_live_pow_truck_ai_dual_world_empty_gate_residual_pack_wave354());
    }

    #[test]
    fn pow_truck_ai_dual_world_empty_gate_sources() {
        assert!(honesty_pow_truck_ai_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_pow_truck_ai_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_pow_truck_ai_dual_world_empty_gate_honesty(),
            "pow truck ai dual-world empty gate residual must latch"
        );
    }
}
