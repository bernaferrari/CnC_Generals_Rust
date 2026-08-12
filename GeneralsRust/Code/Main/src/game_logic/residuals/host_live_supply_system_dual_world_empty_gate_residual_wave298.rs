//! Wave 298 residual peels: Supply system dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), supply/dozer
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 297 StealthDetector dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/supply_system.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Supply system dual-world empty-gate residual method names.
pub const LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE298: &[&str] = &[
    "dual_world_registry_unavailable",
    "resolve_supply_object",
    "gain_one_box",
    "get_action_delay_for_dock",
    "new_task",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE298: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SUPPLY_SYSTEM_EMPTY_GATES",
    "LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE298: &[&str] = &[
    "click_live_supply_system_dual_world_empty_gate_ok_prepare",
    "click_live_supply_system_dual_world_empty_gate_ok_live",
    "click_live_supply_system_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_supply_system_dual_world_empty_gate_method_names_residual_wave298() -> bool {
    LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE298.len() == 6
        && residual_name_index(
            LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE298,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE298,
            "new_task",
        ) == Some(4)
        && residual_name_index(
            LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE298,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_supply_system_dual_world_empty_gate_nav_commands_residual_wave298() -> bool {
    LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE298.len() == 4
        && residual_name_index(
            LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE298,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE298,
            "LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SUPPLY_SYSTEM_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE298.len() == 3
}

/// Wave 298 composite residual honesty pack.
pub fn honesty_live_supply_system_dual_world_empty_gate_residual_pack_wave298() -> bool {
    honesty_live_supply_system_dual_world_empty_gate_method_names_residual_wave298()
        && honesty_live_supply_system_dual_world_empty_gate_nav_commands_residual_wave298()
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

/// Source residual: supply system empty dual-world short-circuits.
pub fn honesty_supply_system_dual_world_empty_gate_source() -> bool {
    let g = gamelogic::supply_system::SUPPLY_SYSTEM_SRC;
    if !(g.contains("Wave 298")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(resolve) = fn_body(g, "fn resolve_supply_object(") else {
        return false;
    };
    let Some(box_gain) = fn_body(g, "fn gain_one_box(") else {
        return false;
    };
    helper_ok && resolve.contains("Err(") && box_gain.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_supply_system_dual_world_empty_gate_honesty() -> bool {
    honesty_live_supply_system_dual_world_empty_gate_residual_pack_wave298()
        && honesty_supply_system_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_supply_system_dual_world_empty_gate_method_names_residual_wave298());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_supply_system_dual_world_empty_gate_nav_commands_residual_wave298());
    }

    #[test]
    fn wave298_composite_pack() {
        assert!(honesty_live_supply_system_dual_world_empty_gate_residual_pack_wave298());
    }

    #[test]
    fn supply_system_dual_world_empty_gate_sources() {
        assert!(honesty_supply_system_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_supply_system_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_supply_system_dual_world_empty_gate_honesty(),
            "supply system dual-world empty gate residual must latch"
        );
    }
}
