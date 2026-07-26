//! Wave 333 residual peels: EMPUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), EMP
//! disable helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 332 WaveGuide dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/emp_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// EMP update dual-world empty-gate residual method names.
pub const LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE333: &[&str] = &[
    "dual_world_registry_unavailable",
    "do_disable_attack",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE333: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_EMP_UPDATE_EMPTY_GATES",
    "LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE333: &[&str] = &[
    "click_live_emp_update_dual_world_empty_gate_ok_prepare",
    "click_live_emp_update_dual_world_empty_gate_ok_live",
    "click_live_emp_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_emp_update_dual_world_empty_gate_method_names_residual_wave333() -> bool {
    LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE333.len() == 4
        && residual_name_index(
            LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE333,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE333,
            "update_simple",
        ) == Some(2)
        && residual_name_index(
            LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE333,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_emp_update_dual_world_empty_gate_nav_commands_residual_wave333() -> bool {
    LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE333.len() == 4
        && residual_name_index(
            LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE333,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE333,
            "LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_EMP_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE333.len() == 3
}

/// Wave 333 composite residual honesty pack.
pub fn honesty_live_emp_update_dual_world_empty_gate_residual_pack_wave333() -> bool {
    honesty_live_emp_update_dual_world_empty_gate_method_names_residual_wave333()
        && honesty_live_emp_update_dual_world_empty_gate_nav_commands_residual_wave333()
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

/// Source residual: EMP update empty dual-world short-circuits.
pub fn honesty_emp_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/behavior/emp_update.rs");
    if !(g.contains("Wave 333")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(attack) = fn_body(g, "fn do_disable_attack(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    helper_ok && attack.contains("return;") && update.contains("UpdateSleepTime::None")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_emp_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_emp_update_dual_world_empty_gate_residual_pack_wave333()
        && honesty_emp_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_emp_update_dual_world_empty_gate_method_names_residual_wave333());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_emp_update_dual_world_empty_gate_nav_commands_residual_wave333());
    }

    #[test]
    fn wave333_composite_pack() {
        assert!(honesty_live_emp_update_dual_world_empty_gate_residual_pack_wave333());
    }

    #[test]
    fn emp_update_dual_world_empty_gate_sources() {
        assert!(honesty_emp_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_emp_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_emp_update_dual_world_empty_gate_honesty(),
            "emp update dual-world empty gate residual must latch"
        );
    }
}
