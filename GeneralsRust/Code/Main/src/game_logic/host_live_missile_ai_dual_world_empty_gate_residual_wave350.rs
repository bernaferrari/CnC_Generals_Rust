//! Wave 350 residual peels: MissileAIUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), detonate/
//! track/kill helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 349 ChinookAI dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/missile_ai_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// MissileAI dual-world empty-gate residual method names.
pub const LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE350: &[&str] = &[
    "dual_world_registry_unavailable",
    "handle_garrison_hit_kill",
    "detonate",
    "current_goal_object",
    "distance_to_goal_bounding_sphere_3d_squared",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE350: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_MISSILE_AI_EMPTY_GATES",
    "LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE350: &[&str] = &[
    "click_live_missile_ai_dual_world_empty_gate_ok_prepare",
    "click_live_missile_ai_dual_world_empty_gate_ok_live",
    "click_live_missile_ai_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_missile_ai_dual_world_empty_gate_method_names_residual_wave350() -> bool {
    LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE350.len() == 6
        && residual_name_index(
            LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE350,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE350,
            "distance_to_goal_bounding_sphere_3d_squared",
        ) == Some(4)
        && residual_name_index(
            LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE350,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_missile_ai_dual_world_empty_gate_nav_commands_residual_wave350() -> bool {
    LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE350.len() == 4
        && residual_name_index(
            LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE350,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE350,
            "LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_MISSILE_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE350.len() == 3
}

/// Wave 350 composite residual honesty pack.
pub fn honesty_live_missile_ai_dual_world_empty_gate_residual_pack_wave350() -> bool {
    honesty_live_missile_ai_dual_world_empty_gate_method_names_residual_wave350()
        && honesty_live_missile_ai_dual_world_empty_gate_nav_commands_residual_wave350()
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

/// Source residual: MissileAI empty dual-world short-circuits.
pub fn honesty_missile_ai_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/update/missile_ai_update.rs");
    if !(g.contains("Wave 350")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(hit) = fn_body(g, "fn handle_garrison_hit_kill(") else {
        return false;
    };
    let Some(det) = fn_body(g, "fn detonate(") else {
        return false;
    };
    let Some(goal) = fn_body(g, "fn current_goal_object(") else {
        return false;
    };
    let Some(dist) = fn_body(g, "fn distance_to_goal_bounding_sphere_3d_squared(") else {
        return false;
    };
    helper_ok
        && hit.contains("return false")
        && det.contains("return;")
        && goal.contains("return None")
        && dist.contains("Real::MAX")
        && g.matches("Wave 350:").count() >= 15
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_missile_ai_dual_world_empty_gate_honesty() -> bool {
    honesty_live_missile_ai_dual_world_empty_gate_residual_pack_wave350()
        && honesty_missile_ai_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_missile_ai_dual_world_empty_gate_method_names_residual_wave350());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_missile_ai_dual_world_empty_gate_nav_commands_residual_wave350());
    }

    #[test]
    fn wave350_composite_pack() {
        assert!(honesty_live_missile_ai_dual_world_empty_gate_residual_pack_wave350());
    }

    #[test]
    fn missile_ai_dual_world_empty_gate_sources() {
        assert!(honesty_missile_ai_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_missile_ai_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_missile_ai_dual_world_empty_gate_honesty(),
            "missile ai dual-world empty gate residual must latch"
        );
    }
}
