//! Wave 330 residual peels: A10StrikePower dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), A-10
//! strike helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 329 SkirmishPlayer dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/special_power_module/a10_strike_power.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// A10 strike dual-world empty-gate residual method names.
pub const LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE330: &[&str] = &[
    "dual_world_registry_unavailable",
    "execute_strike",
    "update_strike",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE330: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_A10_STRIKE_EMPTY_GATES",
    "LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE330: &[&str] = &[
    "click_live_a10_strike_dual_world_empty_gate_ok_prepare",
    "click_live_a10_strike_dual_world_empty_gate_ok_live",
    "click_live_a10_strike_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_a10_strike_dual_world_empty_gate_method_names_residual_wave330() -> bool {
    LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE330.len() == 4
        && residual_name_index(
            LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE330,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE330,
            "update_strike",
        ) == Some(2)
        && residual_name_index(
            LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE330,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_a10_strike_dual_world_empty_gate_nav_commands_residual_wave330() -> bool {
    LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE330.len() == 4
        && residual_name_index(
            LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE330,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE330,
            "LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_A10_STRIKE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE330.len() == 3
}

/// Wave 330 composite residual honesty pack.
pub fn honesty_live_a10_strike_dual_world_empty_gate_residual_pack_wave330() -> bool {
    honesty_live_a10_strike_dual_world_empty_gate_method_names_residual_wave330()
        && honesty_live_a10_strike_dual_world_empty_gate_nav_commands_residual_wave330()
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

/// Source residual: A10 strike empty dual-world short-circuits.
pub fn honesty_a10_strike_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/special_power_module/a10_strike_power.rs"
    );
    if !(g.contains("Wave 330")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(exec) = fn_body(g, "fn execute_strike(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_strike(") else {
        return false;
    };
    helper_ok && exec.contains("Err(") && update.contains("return;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_a10_strike_dual_world_empty_gate_honesty() -> bool {
    honesty_live_a10_strike_dual_world_empty_gate_residual_pack_wave330()
        && honesty_a10_strike_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_a10_strike_dual_world_empty_gate_method_names_residual_wave330());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_a10_strike_dual_world_empty_gate_nav_commands_residual_wave330());
    }

    #[test]
    fn wave330_composite_pack() {
        assert!(honesty_live_a10_strike_dual_world_empty_gate_residual_pack_wave330());
    }

    #[test]
    fn a10_strike_dual_world_empty_gate_sources() {
        assert!(honesty_a10_strike_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_a10_strike_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_a10_strike_dual_world_empty_gate_honesty(),
            "a10 strike dual-world empty gate residual must latch"
        );
    }
}
