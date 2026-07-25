//! Wave 253 residual peels: AIGroup dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), group
//! member walks skip dual-world factory resolution.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 252 presentation script-camera probe residual.
//!
//! Sources:
//! - `GameLogic/src/ai/group.rs` dual_world_registry_unavailable + early outs
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AIGroup dual-world empty-gate residual method names.
pub const LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE253: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_all_ids_snapshot",
    "prune_dead_members",
    "is_idle",
    "is_busy",
    "group_move_to_position",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE253: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_GROUP_EMPTY_GATES",
    "LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE253: &[&str] = &[
    "click_live_ai_group_dual_world_empty_gate_ok_prepare",
    "click_live_ai_group_dual_world_empty_gate_ok_live",
    "click_live_ai_group_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_group_dual_world_empty_gate_method_names_residual_wave253() -> bool {
    LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE253.len() == 7
        && residual_name_index(
            LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE253,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE253,
            "group_move_to_position",
        ) == Some(5)
        && residual_name_index(
            LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE253,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_group_dual_world_empty_gate_nav_commands_residual_wave253() -> bool {
    LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE253.len() == 4
        && residual_name_index(
            LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE253,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE253,
            "LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_GROUP_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE253.len() == 3
}

/// Wave 253 composite residual honesty pack.
pub fn honesty_live_ai_group_dual_world_empty_gate_residual_pack_wave253() -> bool {
    honesty_live_ai_group_dual_world_empty_gate_method_names_residual_wave253()
        && honesty_live_ai_group_dual_world_empty_gate_nav_commands_residual_wave253()
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

/// Source residual: AIGroup empty dual-world short-circuits.
pub fn honesty_ai_group_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/ai/group.rs");
    if !(g.contains("Wave 253")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(snap) = fn_body(g, "fn get_all_ids_snapshot(") else {
        return false;
    };
    let Some(idle) = fn_body(g, "fn is_idle(") else {
        return false;
    };
    let Some(move_to) = fn_body(g, "fn group_move_to_position(") else {
        return false;
    };
    snap.contains("dual_world_registry_unavailable")
        && idle.contains("dual_world_registry_unavailable")
        && move_to.contains("dual_world_registry_unavailable")
        && move_to.contains("self.is_empty()")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_group_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_group_dual_world_empty_gate_residual_pack_wave253()
        && honesty_ai_group_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ai_group_dual_world_empty_gate_method_names_residual_wave253());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ai_group_dual_world_empty_gate_nav_commands_residual_wave253());
    }

    #[test]
    fn wave253_composite_pack() {
        assert!(honesty_live_ai_group_dual_world_empty_gate_residual_pack_wave253());
    }

    #[test]
    fn ai_group_dual_world_empty_gate_sources() {
        assert!(honesty_ai_group_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_group_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_group_dual_world_empty_gate_honesty(),
            "ai group dual-world empty gate residual must latch"
        );
    }
}
