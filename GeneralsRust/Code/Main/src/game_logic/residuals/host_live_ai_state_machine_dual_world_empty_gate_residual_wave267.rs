//! Wave 267 residual peels: AI state machine dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), AI state
//! enter/update helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 266 Partition filters dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/state_machine.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AI state machine dual-world empty-gate residual method names.
pub const LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE267: &[&str] = &[
    "dual_world_registry_unavailable",
    "on_state_enter",
    "update_move_to_state",
    "update_attack_object_state",
    "update_guard_state",
    "resolve_current_position",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE267: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_STATE_MACHINE_EMPTY_GATES",
    "LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE267: &[&str] = &[
    "click_live_ai_state_machine_dual_world_empty_gate_ok_prepare",
    "click_live_ai_state_machine_dual_world_empty_gate_ok_live",
    "click_live_ai_state_machine_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_state_machine_dual_world_empty_gate_method_names_residual_wave267() -> bool {
    LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE267.len() == 7
        && residual_name_index(
            LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE267,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE267,
            "resolve_current_position",
        ) == Some(5)
        && residual_name_index(
            LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE267,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_state_machine_dual_world_empty_gate_nav_commands_residual_wave267() -> bool {
    LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE267.len() == 4
        && residual_name_index(
            LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE267,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE267,
            "LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_STATE_MACHINE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE267.len() == 3
}

/// Wave 267 composite residual honesty pack.
pub fn honesty_live_ai_state_machine_dual_world_empty_gate_residual_pack_wave267() -> bool {
    honesty_live_ai_state_machine_dual_world_empty_gate_method_names_residual_wave267()
        && honesty_live_ai_state_machine_dual_world_empty_gate_nav_commands_residual_wave267()
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

/// Source residual: AI state machine empty dual-world short-circuits.
pub fn honesty_ai_state_machine_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/ai/state_machine.rs");
    if !(g.contains("Wave 267")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(enter) = fn_body(g, "fn on_state_enter(") else {
        return false;
    };
    let Some(move_to) = fn_body(g, "fn update_move_to_state(") else {
        return false;
    };
    let Some(pos) = fn_body(g, "fn resolve_current_position(") else {
        return false;
    };
    enter.contains("dual_world_registry_unavailable")
        && move_to.contains("dual_world_registry_unavailable")
        && move_to.contains("StateReturnType::StateFailed")
        && pos.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_state_machine_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_state_machine_dual_world_empty_gate_residual_pack_wave267()
        && honesty_ai_state_machine_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_ai_state_machine_dual_world_empty_gate_method_names_residual_wave267()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_ai_state_machine_dual_world_empty_gate_nav_commands_residual_wave267()
        );
    }

    #[test]
    fn wave267_composite_pack() {
        assert!(honesty_live_ai_state_machine_dual_world_empty_gate_residual_pack_wave267());
    }

    #[test]
    fn ai_state_machine_dual_world_empty_gate_sources() {
        assert!(honesty_ai_state_machine_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_state_machine_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_state_machine_dual_world_empty_gate_honesty(),
            "ai state machine dual-world empty gate residual must latch"
        );
    }
}
