//! Wave 307 residual peels: GrantStealthBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), grant-stealth
//! scan/update helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 306 AutoHeal dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/grant_stealth_behavior.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Grant stealth dual-world empty-gate residual method names.
pub const LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE307: &[&str] = &[
    "dual_world_registry_unavailable",
    "grant_stealth_to_object",
    "scan_for_objects",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE307: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_GRANT_STEALTH_EMPTY_GATES",
    "LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE307: &[&str] = &[
    "click_live_grant_stealth_dual_world_empty_gate_ok_prepare",
    "click_live_grant_stealth_dual_world_empty_gate_ok_live",
    "click_live_grant_stealth_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_grant_stealth_dual_world_empty_gate_method_names_residual_wave307() -> bool {
    LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE307.len() == 5
        && residual_name_index(
            LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE307,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE307,
            "update_simple",
        ) == Some(3)
        && residual_name_index(
            LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE307,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_grant_stealth_dual_world_empty_gate_nav_commands_residual_wave307() -> bool {
    LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE307.len() == 4
        && residual_name_index(
            LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE307,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE307,
            "LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_GRANT_STEALTH_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE307.len() == 3
}

/// Wave 307 composite residual honesty pack.
pub fn honesty_live_grant_stealth_dual_world_empty_gate_residual_pack_wave307() -> bool {
    honesty_live_grant_stealth_dual_world_empty_gate_method_names_residual_wave307()
        && honesty_live_grant_stealth_dual_world_empty_gate_nav_commands_residual_wave307()
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

/// Source residual: grant stealth empty dual-world short-circuits.
pub fn honesty_grant_stealth_dual_world_empty_gate_source() -> bool {
    let g =
        include_str!("../../../../GameEngine/GameLogic/src/object/behavior/grant_stealth_behavior.rs");
    if !(g.contains("Wave 307")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    let Some(scan) = fn_body(g, "fn scan_for_objects(") else {
        return false;
    };
    helper_ok && update.contains("UPDATE_SLEEP_FOREVER") && scan.contains("return;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_grant_stealth_dual_world_empty_gate_honesty() -> bool {
    honesty_live_grant_stealth_dual_world_empty_gate_residual_pack_wave307()
        && honesty_grant_stealth_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_grant_stealth_dual_world_empty_gate_method_names_residual_wave307());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_grant_stealth_dual_world_empty_gate_nav_commands_residual_wave307());
    }

    #[test]
    fn wave307_composite_pack() {
        assert!(honesty_live_grant_stealth_dual_world_empty_gate_residual_pack_wave307());
    }

    #[test]
    fn grant_stealth_dual_world_empty_gate_sources() {
        assert!(honesty_grant_stealth_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_grant_stealth_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_grant_stealth_dual_world_empty_gate_honesty(),
            "grant stealth dual-world empty gate residual must latch"
        );
    }
}
