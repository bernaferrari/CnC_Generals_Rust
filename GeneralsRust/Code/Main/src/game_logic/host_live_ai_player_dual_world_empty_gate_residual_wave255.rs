//! Wave 255 residual peels: AIPlayer dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), AI player
//! factory/economy/military walks fail-closed without dual-world factory
//! resolution. Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 254 AIStates dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/ai_player.rs` dual_world_registry_unavailable + early outs
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AIPlayer dual-world empty-gate residual method names.
pub const LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE255: &[&str] = &[
    "dual_world_registry_unavailable",
    "validate_factory",
    "find_supply_center",
    "process_base_building",
    "select_attack_target",
    "count_player_harvesters",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE255: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_PLAYER_EMPTY_GATES",
    "LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE255: &[&str] = &[
    "click_live_ai_player_dual_world_empty_gate_ok_prepare",
    "click_live_ai_player_dual_world_empty_gate_ok_live",
    "click_live_ai_player_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_player_dual_world_empty_gate_method_names_residual_wave255() -> bool {
    LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE255.len() == 7
        && residual_name_index(
            LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE255,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE255,
            "process_base_building",
        ) == Some(3)
        && residual_name_index(
            LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE255,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_player_dual_world_empty_gate_nav_commands_residual_wave255() -> bool {
    LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE255.len() == 4
        && residual_name_index(
            LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE255,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE255,
            "LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE255.len() == 3
}

/// Wave 255 composite residual honesty pack.
pub fn honesty_live_ai_player_dual_world_empty_gate_residual_pack_wave255() -> bool {
    honesty_live_ai_player_dual_world_empty_gate_method_names_residual_wave255()
        && honesty_live_ai_player_dual_world_empty_gate_nav_commands_residual_wave255()
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

/// Source residual: AIPlayer empty dual-world short-circuits.
pub fn honesty_ai_player_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/ai/ai_player.rs");
    if !(g.contains("Wave 255")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(vf) = fn_body(g, "fn validate_factory(") else {
        return false;
    };
    let Some(pbb) = fn_body(g, "fn process_base_building(") else {
        return false;
    };
    let Some(sat) = fn_body(g, "fn select_attack_target(") else {
        return false;
    };
    vf.contains("dual_world_registry_unavailable")
        && pbb.contains("dual_world_registry_unavailable")
        && sat.contains("dual_world_registry_unavailable")
        && sat.contains("Ok(None)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_player_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_player_dual_world_empty_gate_residual_pack_wave255()
        && honesty_ai_player_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ai_player_dual_world_empty_gate_method_names_residual_wave255());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ai_player_dual_world_empty_gate_nav_commands_residual_wave255());
    }

    #[test]
    fn wave255_composite_pack() {
        assert!(honesty_live_ai_player_dual_world_empty_gate_residual_pack_wave255());
    }

    #[test]
    fn ai_player_dual_world_empty_gate_sources() {
        assert!(honesty_ai_player_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_player_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_player_dual_world_empty_gate_honesty(),
            "ai player dual-world empty gate residual must latch"
        );
    }
}
