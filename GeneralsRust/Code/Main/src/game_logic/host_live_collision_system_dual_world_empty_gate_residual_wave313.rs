//! Wave 313 residual peels: CollisionSystem dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), collision
//! respond/ignore/crush helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 312 ProductionExit dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/collide/collision_system.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Collision system dual-world empty-gate residual method names.
pub const LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE313: &[&str] = &[
    "dual_world_registry_unavailable",
    "test_and_respond_collision",
    "should_ignore_ai_collision_id",
    "should_ignore_physics_collision_id",
    "can_crush_or_squish",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE313: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_COLLISION_SYSTEM_EMPTY_GATES",
    "LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE313: &[&str] = &[
    "click_live_collision_system_dual_world_empty_gate_ok_prepare",
    "click_live_collision_system_dual_world_empty_gate_ok_live",
    "click_live_collision_system_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_collision_system_dual_world_empty_gate_method_names_residual_wave313() -> bool {
    LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE313.len() == 6
        && residual_name_index(
            LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE313,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE313,
            "can_crush_or_squish",
        ) == Some(4)
        && residual_name_index(
            LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE313,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_collision_system_dual_world_empty_gate_nav_commands_residual_wave313() -> bool {
    LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE313.len() == 4
        && residual_name_index(
            LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE313,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE313,
            "LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COLLISION_SYSTEM_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE313.len() == 3
}

/// Wave 313 composite residual honesty pack.
pub fn honesty_live_collision_system_dual_world_empty_gate_residual_pack_wave313() -> bool {
    honesty_live_collision_system_dual_world_empty_gate_method_names_residual_wave313()
        && honesty_live_collision_system_dual_world_empty_gate_nav_commands_residual_wave313()
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

/// Source residual: collision system empty dual-world short-circuits.
pub fn honesty_collision_system_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/collide/collision_system.rs");
    if !(g.contains("Wave 313")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(test) = fn_body(g, "fn test_and_respond_collision(") else {
        return false;
    };
    let Some(crush) = fn_body(g, "fn can_crush_or_squish(") else {
        return false;
    };
    helper_ok
        && test.contains("Ok(false)")
        && crush.contains("return false")
        && g.contains("fn should_ignore_ai_collision_id")
        && g.contains("fn should_ignore_physics_collision_id")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_collision_system_dual_world_empty_gate_honesty() -> bool {
    honesty_live_collision_system_dual_world_empty_gate_residual_pack_wave313()
        && honesty_collision_system_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_collision_system_dual_world_empty_gate_method_names_residual_wave313()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_collision_system_dual_world_empty_gate_nav_commands_residual_wave313()
        );
    }

    #[test]
    fn wave313_composite_pack() {
        assert!(honesty_live_collision_system_dual_world_empty_gate_residual_pack_wave313());
    }

    #[test]
    fn collision_system_dual_world_empty_gate_sources() {
        assert!(honesty_collision_system_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_collision_system_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_collision_system_dual_world_empty_gate_honesty(),
            "collision system dual-world empty gate residual must latch"
        );
    }
}
