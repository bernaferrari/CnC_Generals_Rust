//! Wave 432 residual peels: UnitExit dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), unit exit
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 431 SubObjectsUpgrade dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/production/unit_exit.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// UnitExit dual-world empty-gate residual method names.
pub const LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE432: &[&str] = &[
    "dual_world_registry_unavailable",
    "update_frame",
    "calculate_exit_path",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE432: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_UNIT_EXIT_EMPTY_GATES",
    "LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE432: &[&str] = &[
    "click_live_unit_exit_dual_world_empty_gate_ok_prepare",
    "click_live_unit_exit_dual_world_empty_gate_ok_live",
    "click_live_unit_exit_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_unit_exit_dual_world_empty_gate_method_names_residual_wave432() -> bool {
    LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE432.len() == 4
        && residual_name_index(
            LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE432,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE432,
            "calculate_exit_path",
        ) == Some(2)
        && residual_name_index(
            LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE432,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_unit_exit_dual_world_empty_gate_nav_commands_residual_wave432() -> bool {
    LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE432.len() == 4
        && residual_name_index(
            LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE432,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE432,
            "LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_UNIT_EXIT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE432.len() == 3
}

/// Wave 432 composite residual honesty pack.
pub fn honesty_live_unit_exit_dual_world_empty_gate_residual_pack_wave432() -> bool {
    honesty_live_unit_exit_dual_world_empty_gate_method_names_residual_wave432()
        && honesty_live_unit_exit_dual_world_empty_gate_nav_commands_residual_wave432()
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

/// Source residual: UnitExit empty dual-world short-circuits.
pub fn honesty_unit_exit_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/production/unit_exit.rs");
    if !(g.contains("Wave 432")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(update) = fn_body(g, "fn update_frame(") else {
        return false;
    };
    let Some(path) = fn_body(g, "fn calculate_exit_path(") else {
        return false;
    };
    helper_ok && update.contains("Vec::new()") && path.contains("return None")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_unit_exit_dual_world_empty_gate_honesty() -> bool {
    honesty_live_unit_exit_dual_world_empty_gate_residual_pack_wave432()
        && honesty_unit_exit_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_unit_exit_dual_world_empty_gate_method_names_residual_wave432());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_unit_exit_dual_world_empty_gate_nav_commands_residual_wave432());
    }

    #[test]
    fn wave432_composite_pack() {
        assert!(honesty_live_unit_exit_dual_world_empty_gate_residual_pack_wave432());
    }

    #[test]
    fn unit_exit_dual_world_empty_gate_sources() {
        assert!(honesty_unit_exit_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_unit_exit_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_unit_exit_dual_world_empty_gate_honesty(),
            "unit exit dual-world empty gate residual must latch"
        );
    }
}
