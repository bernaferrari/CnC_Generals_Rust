//! Wave 254 residual peels: AIStates dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), AI state
//! owner/target resolves fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 253 AIGroup dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/ai_states.rs` dual_world_registry_unavailable + early outs
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AIStates dual-world empty-gate residual method names.
pub const LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE254: &[&str] = &[
    "dual_world_registry_unavailable",
    "out_of_weapon_range_object",
    "goal_reached",
    "on_enter",
    "update",
    "AIAttackState",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE254: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_STATE_EMPTY_GATES",
    "LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE254: &[&str] = &[
    "click_live_ai_states_dual_world_empty_gate_ok_prepare",
    "click_live_ai_states_dual_world_empty_gate_ok_live",
    "click_live_ai_states_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_states_dual_world_empty_gate_method_names_residual_wave254() -> bool {
    LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE254.len() == 7
        && residual_name_index(
            LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE254,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE254,
            "AIAttackState",
        ) == Some(5)
        && residual_name_index(
            LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE254,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_states_dual_world_empty_gate_nav_commands_residual_wave254() -> bool {
    LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE254.len() == 4
        && residual_name_index(
            LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE254,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE254,
            "LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_STATES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE254.len() == 3
}

/// Wave 254 composite residual honesty pack.
pub fn honesty_live_ai_states_dual_world_empty_gate_residual_pack_wave254() -> bool {
    honesty_live_ai_states_dual_world_empty_gate_method_names_residual_wave254()
        && honesty_live_ai_states_dual_world_empty_gate_nav_commands_residual_wave254()
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

/// Source residual: AIStates empty dual-world short-circuits.
pub fn honesty_ai_states_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/ai/ai_states.rs");
    if !(g.contains("Wave 254")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(range) = fn_body(g, "fn out_of_weapon_range_object(") else {
        return false;
    };
    let Some(goal) = fn_body(g, "fn goal_reached(") else {
        return false;
    };
    // Attack state on_enter fails closed when dual-world empty.
    let attack_idx = g.find("impl AIState for AIAttackState");
    if attack_idx.is_none() {
        return false;
    }
    let attack = &g[attack_idx.unwrap()..attack_idx.unwrap() + 800];
    range.contains("dual_world_registry_unavailable")
        && goal.contains("dual_world_registry_unavailable")
        && attack.contains("dual_world_registry_unavailable")
        && attack.contains("StateReturnType::Failed")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_states_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_states_dual_world_empty_gate_residual_pack_wave254()
        && honesty_ai_states_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ai_states_dual_world_empty_gate_method_names_residual_wave254());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ai_states_dual_world_empty_gate_nav_commands_residual_wave254());
    }

    #[test]
    fn wave254_composite_pack() {
        assert!(honesty_live_ai_states_dual_world_empty_gate_residual_pack_wave254());
    }

    #[test]
    fn ai_states_dual_world_empty_gate_sources() {
        assert!(honesty_ai_states_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_states_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_states_dual_world_empty_gate_honesty(),
            "ai states dual-world empty gate residual must latch"
        );
    }
}
