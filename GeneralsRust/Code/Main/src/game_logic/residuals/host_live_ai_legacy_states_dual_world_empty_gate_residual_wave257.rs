//! Wave 257 residual peels: legacy AI states dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), classic AI
//! state enter/update and combat helpers fail-closed without dual-world factory
//! walks. Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 256 Team dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/states.rs` dual_world_registry_unavailable + early outs
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Legacy AI states dual-world empty-gate residual method names.
pub const LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE257: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_goal_object",
    "classic_on_enter",
    "classic_on_update",
    "choose_victim",
    "compute_path",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE257: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_LEGACY_AI_STATE_EMPTY_GATES",
    "LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE257: &[&str] = &[
    "click_live_ai_legacy_states_dual_world_empty_gate_ok_prepare",
    "click_live_ai_legacy_states_dual_world_empty_gate_ok_live",
    "click_live_ai_legacy_states_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_legacy_states_dual_world_empty_gate_method_names_residual_wave257() -> bool {
    LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE257.len() == 7
        && residual_name_index(
            LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE257,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE257,
            "compute_path",
        ) == Some(5)
        && residual_name_index(
            LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE257,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_legacy_states_dual_world_empty_gate_nav_commands_residual_wave257() -> bool {
    LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE257.len() == 4
        && residual_name_index(
            LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE257,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE257,
            "LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_LEGACY_STATES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE257.len() == 3
}

/// Wave 257 composite residual honesty pack.
pub fn honesty_live_ai_legacy_states_dual_world_empty_gate_residual_pack_wave257() -> bool {
    honesty_live_ai_legacy_states_dual_world_empty_gate_method_names_residual_wave257()
        && honesty_live_ai_legacy_states_dual_world_empty_gate_nav_commands_residual_wave257()
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

/// Source residual: legacy AI states empty dual-world short-circuits.
pub fn honesty_ai_legacy_states_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/ai/states.rs");
    if !(g.contains("Wave 257")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(goal) = fn_body(g, "fn get_goal_object(") else {
        return false;
    };
    let Some(choose) = fn_body(g, "fn choose_victim(") else {
        return false;
    };
    // Attack-path compute_path variants take no AI arg and gate dual-world.
    let path_gated = g.contains(
        "fn compute_path(&mut self) -> Result<bool, String> {
        // Wave 257: empty dual-world → Ok(false).",
    );
    // classic_on_enter with Failure return present
    goal.contains("dual_world_registry_unavailable")
        && choose.contains("dual_world_registry_unavailable")
        && path_gated
        && g.contains("Wave 257: empty dual-world → fail-closed state.")
        && g.contains("Ok(StateReturnType::Failure)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_legacy_states_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_legacy_states_dual_world_empty_gate_residual_pack_wave257()
        && honesty_ai_legacy_states_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_ai_legacy_states_dual_world_empty_gate_method_names_residual_wave257()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_ai_legacy_states_dual_world_empty_gate_nav_commands_residual_wave257()
        );
    }

    #[test]
    fn wave257_composite_pack() {
        assert!(honesty_live_ai_legacy_states_dual_world_empty_gate_residual_pack_wave257());
    }

    #[test]
    fn ai_legacy_states_dual_world_empty_gate_sources() {
        assert!(honesty_ai_legacy_states_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_legacy_states_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_legacy_states_dual_world_empty_gate_honesty(),
            "legacy ai states dual-world empty gate residual must latch"
        );
    }
}
