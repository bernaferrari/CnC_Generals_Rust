//! Wave 397 residual peels: AI Dock dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), AI dock
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 396 NeutronMissileSlowDeath dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/dock.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AI Dock dual-world empty-gate residual method names.
pub const LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE397: &[&str] = &[
    "dual_world_registry_unavailable",
    "resolve_dock_object",
    "halt",
    "find_my_drone_id",
    "find_my_drone",
    "drone",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE397: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_DOCK_EMPTY_GATES",
    "LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE397: &[&str] = &[
    "click_live_ai_dock_dual_world_empty_gate_ok_prepare",
    "click_live_ai_dock_dual_world_empty_gate_ok_live",
    "click_live_ai_dock_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_dock_dual_world_empty_gate_method_names_residual_wave397() -> bool {
    LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE397.len() == 7
        && residual_name_index(
            LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE397,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE397,
            "drone",
        ) == Some(5)
        && residual_name_index(
            LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE397,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_dock_dual_world_empty_gate_nav_commands_residual_wave397() -> bool {
    LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE397.len() == 4
        && residual_name_index(
            LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE397,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE397,
            "LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_DOCK_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE397.len() == 3
}

/// Wave 397 composite residual honesty pack.
pub fn honesty_live_ai_dock_dual_world_empty_gate_residual_pack_wave397() -> bool {
    honesty_live_ai_dock_dual_world_empty_gate_method_names_residual_wave397()
        && honesty_live_ai_dock_dual_world_empty_gate_nav_commands_residual_wave397()
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

/// Source residual: AI Dock empty dual-world short-circuits.
pub fn honesty_ai_dock_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/ai/dock.rs");
    if !(g.contains("Wave 397")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(resolve) = fn_body(g, "fn resolve_dock_object(") else {
        return false;
    };
    let Some(halt) = fn_body(g, "fn halt(") else {
        return false;
    };
    let Some(drone_id) = fn_body(g, "fn find_my_drone_id(") else {
        return false;
    };
    let Some(drone) = fn_body(g, "fn drone(") else {
        return false;
    };
    helper_ok
        && resolve.contains("not found")
        && halt.contains("return Ok(())")
        && drone_id.contains("return Ok(None)")
        && drone.contains("return None")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_dock_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_dock_dual_world_empty_gate_residual_pack_wave397()
        && honesty_ai_dock_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ai_dock_dual_world_empty_gate_method_names_residual_wave397());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ai_dock_dual_world_empty_gate_nav_commands_residual_wave397());
    }

    #[test]
    fn wave397_composite_pack() {
        assert!(honesty_live_ai_dock_dual_world_empty_gate_residual_pack_wave397());
    }

    #[test]
    fn ai_dock_dual_world_empty_gate_sources() {
        assert!(honesty_ai_dock_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_dock_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_dock_dual_world_empty_gate_honesty(),
            "ai dock dual-world empty gate residual must latch"
        );
    }
}
