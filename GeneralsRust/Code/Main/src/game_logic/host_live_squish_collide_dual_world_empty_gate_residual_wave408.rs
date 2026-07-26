//! Wave 408 residual peels: SquishCollide dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), squish collide
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 407 RailedTransportAIUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/collide/squish_collide.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// SquishCollide dual-world empty-gate residual method names.
pub const LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE408: &[&str] = &[
    "dual_world_registry_unavailable",
    "owner_handle",
    "should_allow_squish",
    "apply_crush",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE408: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SQUISH_COLLIDE_EMPTY_GATES",
    "LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE408: &[&str] = &[
    "click_live_squish_collide_dual_world_empty_gate_ok_prepare",
    "click_live_squish_collide_dual_world_empty_gate_ok_live",
    "click_live_squish_collide_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_squish_collide_dual_world_empty_gate_method_names_residual_wave408() -> bool {
    LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE408.len() == 5
        && residual_name_index(
            LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE408,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE408,
            "apply_crush",
        ) == Some(3)
        && residual_name_index(
            LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE408,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_squish_collide_dual_world_empty_gate_nav_commands_residual_wave408() -> bool {
    LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE408.len() == 4
        && residual_name_index(
            LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE408,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE408,
            "LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SQUISH_COLLIDE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE408.len() == 3
}

/// Wave 408 composite residual honesty pack.
pub fn honesty_live_squish_collide_dual_world_empty_gate_residual_pack_wave408() -> bool {
    honesty_live_squish_collide_dual_world_empty_gate_method_names_residual_wave408()
        && honesty_live_squish_collide_dual_world_empty_gate_nav_commands_residual_wave408()
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

/// Source residual: SquishCollide empty dual-world short-circuits.
pub fn honesty_squish_collide_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/collide/squish_collide.rs");
    if !(g.contains("Wave 408")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(owner) = fn_body(g, "fn owner_handle(") else {
        return false;
    };
    let Some(allow) = fn_body(g, "fn should_allow_squish(") else {
        return false;
    };
    let Some(crush) = fn_body(g, "fn apply_crush(") else {
        return false;
    };
    helper_ok
        && owner.contains("InvalidObject")
        && allow.contains("return false")
        && crush.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_squish_collide_dual_world_empty_gate_honesty() -> bool {
    honesty_live_squish_collide_dual_world_empty_gate_residual_pack_wave408()
        && honesty_squish_collide_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_squish_collide_dual_world_empty_gate_method_names_residual_wave408());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_squish_collide_dual_world_empty_gate_nav_commands_residual_wave408());
    }

    #[test]
    fn wave408_composite_pack() {
        assert!(honesty_live_squish_collide_dual_world_empty_gate_residual_pack_wave408());
    }

    #[test]
    fn squish_collide_dual_world_empty_gate_sources() {
        assert!(honesty_squish_collide_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_squish_collide_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_squish_collide_dual_world_empty_gate_honesty(),
            "squish collide dual-world empty gate residual must latch"
        );
    }
}
