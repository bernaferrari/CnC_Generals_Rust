//! Wave 263 residual peels: AI module dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), AI group
//! and targeting helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 262 Pathfind dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/mod.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AI mod dual-world empty-gate residual method names.
pub const LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE263: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_idle",
    "group_move_to_position",
    "find_closest_enemy",
    "compute_individual_destination",
    "get_attitude",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE263: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_MOD_EMPTY_GATES",
    "LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE263: &[&str] = &[
    "click_live_ai_mod_dual_world_empty_gate_ok_prepare",
    "click_live_ai_mod_dual_world_empty_gate_ok_live",
    "click_live_ai_mod_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_mod_dual_world_empty_gate_method_names_residual_wave263() -> bool {
    LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE263.len() == 7
        && residual_name_index(
            LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE263,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE263,
            "get_attitude",
        ) == Some(5)
        && residual_name_index(
            LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE263,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_mod_dual_world_empty_gate_nav_commands_residual_wave263() -> bool {
    LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE263.len() == 4
        && residual_name_index(
            LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE263,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE263,
            "LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_MOD_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE263.len() == 3
}

/// Wave 263 composite residual honesty pack.
pub fn honesty_live_ai_mod_dual_world_empty_gate_residual_pack_wave263() -> bool {
    honesty_live_ai_mod_dual_world_empty_gate_method_names_residual_wave263()
        && honesty_live_ai_mod_dual_world_empty_gate_nav_commands_residual_wave263()
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

/// Source residual: AI module empty dual-world short-circuits.
pub fn honesty_ai_mod_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/ai/mod.rs");
    if !(g.contains("Wave 263")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(idle) = fn_body(g, "fn is_idle(") else {
        return false;
    };
    let Some(move_to) = fn_body(g, "fn group_move_to_position(") else {
        return false;
    };
    let Some(dest) = fn_body(g, "fn compute_individual_destination(") else {
        return false;
    };
    idle.contains("dual_world_registry_unavailable")
        && move_to.contains("dual_world_registry_unavailable")
        && dest.contains("dual_world_registry_unavailable")
        && dest.contains("*group_dest")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_mod_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_mod_dual_world_empty_gate_residual_pack_wave263()
        && honesty_ai_mod_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ai_mod_dual_world_empty_gate_method_names_residual_wave263());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ai_mod_dual_world_empty_gate_nav_commands_residual_wave263());
    }

    #[test]
    fn wave263_composite_pack() {
        assert!(honesty_live_ai_mod_dual_world_empty_gate_residual_pack_wave263());
    }

    #[test]
    fn ai_mod_dual_world_empty_gate_sources() {
        assert!(honesty_ai_mod_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_mod_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_mod_dual_world_empty_gate_honesty(),
            "ai mod dual-world empty gate residual must latch"
        );
    }
}
