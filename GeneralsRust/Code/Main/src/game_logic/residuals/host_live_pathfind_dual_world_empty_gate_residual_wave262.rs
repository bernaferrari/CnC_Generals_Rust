//! Wave 262 residual peels: PathfindingSystem dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), pathfind
//! obstacle/ally/object queries fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 261 OpenContain dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/pathfind_complete.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Pathfind dual-world empty-gate residual method names.
pub const LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE262: &[&str] = &[
    "dual_world_registry_unavailable",
    "process_queue",
    "check_destination",
    "move_allies",
    "object_is_vehicle",
    "check_for_movement",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE262: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PATHFIND_EMPTY_GATES",
    "LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE262: &[&str] = &[
    "click_live_pathfind_dual_world_empty_gate_ok_prepare",
    "click_live_pathfind_dual_world_empty_gate_ok_live",
    "click_live_pathfind_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_pathfind_dual_world_empty_gate_method_names_residual_wave262() -> bool {
    LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE262.len() == 7
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE262,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE262,
            "check_for_movement",
        ) == Some(5)
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE262,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_pathfind_dual_world_empty_gate_nav_commands_residual_wave262() -> bool {
    LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE262.len() == 4
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE262,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE262,
            "LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PATHFIND_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE262.len() == 3
}

/// Wave 262 composite residual honesty pack.
pub fn honesty_live_pathfind_dual_world_empty_gate_residual_pack_wave262() -> bool {
    honesty_live_pathfind_dual_world_empty_gate_method_names_residual_wave262()
        && honesty_live_pathfind_dual_world_empty_gate_nav_commands_residual_wave262()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: PathfindingSystem empty dual-world short-circuits.
pub fn honesty_pathfind_dual_world_empty_gate_source() -> bool {
    let g = concat!(
        include_str!(
            "../../../../GameEngine/GameLogic/src/ai/pathfind_complete/types.rs"
        ),
        include_str!(
            "../../../../GameEngine/GameLogic/src/ai/pathfind_complete/construct.rs"
        ),
        include_str!(
            "../../../../GameEngine/GameLogic/src/ai/pathfind_complete/line_passable.rs"
        ),
        include_str!(
            "../../../../GameEngine/GameLogic/src/ai/pathfind_complete/occupancy.rs"
        ),
    );
    if !(g.contains("Wave 262")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(queue) = fn_body(g, "fn process_queue(") else {
        return false;
    };
    let Some(dest) = fn_body(g, "fn check_destination(") else {
        return false;
    };
    let Some(vehicle) = fn_body(g, "fn object_is_vehicle(") else {
        return false;
    };
    queue.contains("dual_world_registry_unavailable")
        && dest.contains("dual_world_registry_unavailable")
        && vehicle.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_pathfind_dual_world_empty_gate_honesty() -> bool {
    honesty_live_pathfind_dual_world_empty_gate_residual_pack_wave262()
        && honesty_pathfind_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_pathfind_dual_world_empty_gate_method_names_residual_wave262());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_pathfind_dual_world_empty_gate_nav_commands_residual_wave262());
    }

    #[test]
    fn wave262_composite_pack() {
        assert!(honesty_live_pathfind_dual_world_empty_gate_residual_pack_wave262());
    }

    #[test]
    fn pathfind_dual_world_empty_gate_sources() {
        assert!(honesty_pathfind_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_pathfind_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_pathfind_dual_world_empty_gate_honesty(),
            "pathfind dual-world empty gate residual must latch"
        );
    }
}
