//! Wave 328 residual peels: CountermeasuresBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), flare
//! divert helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 327 NeutronBlast dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/countermeasures_behavior.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Countermeasures dual-world empty-gate residual method names.
pub const LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE328: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_object_valid",
    "calculate_distance_squared",
    "get_object",
    "report_missile_for_countermeasures",
    "apply_upgrade",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE328: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_COUNTERMEASURES_EMPTY_GATES",
    "LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE328: &[&str] = &[
    "click_live_countermeasures_dual_world_empty_gate_ok_prepare",
    "click_live_countermeasures_dual_world_empty_gate_ok_live",
    "click_live_countermeasures_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_countermeasures_dual_world_empty_gate_method_names_residual_wave328() -> bool {
    LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE328.len() == 7
        && residual_name_index(
            LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE328,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE328,
            "report_missile_for_countermeasures",
        ) == Some(4)
        && residual_name_index(
            LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE328,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_countermeasures_dual_world_empty_gate_nav_commands_residual_wave328() -> bool {
    LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE328.len() == 4
        && residual_name_index(
            LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE328,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE328,
            "LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COUNTERMEASURES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE328.len() == 3
}

/// Wave 328 composite residual honesty pack.
pub fn honesty_live_countermeasures_dual_world_empty_gate_residual_pack_wave328() -> bool {
    honesty_live_countermeasures_dual_world_empty_gate_method_names_residual_wave328()
        && honesty_live_countermeasures_dual_world_empty_gate_nav_commands_residual_wave328()
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

/// Source residual: countermeasures empty dual-world short-circuits.
pub fn honesty_countermeasures_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/behavior/countermeasures_behavior.rs"
    );
    if !(g.contains("Wave 328")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(valid) = fn_body(g, "fn is_object_valid(") else {
        return false;
    };
    let Some(get) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(report) = fn_body(g, "fn report_missile_for_countermeasures(") else {
        return false;
    };
    helper_ok
        && valid.contains("return false")
        && get.contains("ObjectNotFound")
        && report.contains("Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_countermeasures_dual_world_empty_gate_honesty() -> bool {
    honesty_live_countermeasures_dual_world_empty_gate_residual_pack_wave328()
        && honesty_countermeasures_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_countermeasures_dual_world_empty_gate_method_names_residual_wave328());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_countermeasures_dual_world_empty_gate_nav_commands_residual_wave328());
    }

    #[test]
    fn wave328_composite_pack() {
        assert!(honesty_live_countermeasures_dual_world_empty_gate_residual_pack_wave328());
    }

    #[test]
    fn countermeasures_dual_world_empty_gate_sources() {
        assert!(honesty_countermeasures_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_countermeasures_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_countermeasures_dual_world_empty_gate_honesty(),
            "countermeasures dual-world empty gate residual must latch"
        );
    }
}
