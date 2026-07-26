//! Wave 426 residual peels: Pathfind dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), attack-LOS
//! obstacle scans fail-closed without dual-world factory walks.
//! Destination/goal helpers stay ungated (they fall back to TheGameLogic).
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 425 AIManager dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/pathfind.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Pathfind dual-world empty-gate residual method names.
pub const LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE426: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_attack_view_blocked_by_obstacle",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE426: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PATHFIND_EMPTY_GATES",
    "LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE426: &[&str] = &[
    "click_live_pathfind_dual_world_empty_gate_ok_prepare",
    "click_live_pathfind_dual_world_empty_gate_ok_live",
    "click_live_pathfind_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_pathfind_dual_world_empty_gate_method_names_residual_wave426() -> bool {
    LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE426.len() == 3
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE426,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE426,
            "is_attack_view_blocked_by_obstacle",
        ) == Some(1)
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE426,
            "playable_claim = false",
        ) == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_pathfind_dual_world_empty_gate_nav_commands_residual_wave426() -> bool {
    LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE426.len() == 4
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE426,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE426,
            "LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE426.len() == 3
}

/// Wave 426 composite residual honesty pack.
pub fn honesty_live_pathfind_dual_world_empty_gate_residual_pack_wave426() -> bool {
    honesty_live_pathfind_dual_world_empty_gate_method_names_residual_wave426()
        && honesty_live_pathfind_dual_world_empty_gate_nav_commands_residual_wave426()
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

/// Source residual: Pathfind empty dual-world short-circuits.
pub fn honesty_pathfind_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/ai/pathfind.rs");
    if !(g.contains("Wave 426")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(attack) = fn_body(g, "fn is_attack_view_blocked_by_obstacle(") else {
        return false;
    };
    // Host-fallback helpers must remain ungated.
    let adjust_start = g.find("fn adjust_destination_for_object(").unwrap_or(0);
    let adjust_slice = g.get(adjust_start..adjust_start + 280).unwrap_or("");
    let goal_start = g.find("fn update_goal_for_object(").unwrap_or(0);
    let goal_slice = g.get(goal_start..goal_start + 280).unwrap_or("");
    helper_ok
        && attack.contains("return false")
        && !adjust_slice.contains("dual_world_registry_unavailable")
        && !goal_slice.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_pathfind_dual_world_empty_gate_honesty_wave426() -> bool {
    honesty_live_pathfind_dual_world_empty_gate_residual_pack_wave426()
        && honesty_pathfind_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_pathfind_dual_world_empty_gate_method_names_residual_wave426());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_pathfind_dual_world_empty_gate_nav_commands_residual_wave426());
    }

    #[test]
    fn wave426_composite_pack() {
        assert!(honesty_live_pathfind_dual_world_empty_gate_residual_pack_wave426());
    }

    #[test]
    fn pathfind_dual_world_empty_gate_sources() {
        assert!(honesty_pathfind_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_pathfind_dual_world_empty_gate_honesty_wave426_residual_live() {
        assert!(
            simulate_live_pathfind_dual_world_empty_gate_honesty_wave426(),
            "pathfind dual-world empty gate residual must latch"
        );
    }
}
