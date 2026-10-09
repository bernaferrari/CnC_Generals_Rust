//! Wave 258 residual: explicit Unit adapters retain empty-registry gates.
//! Native mood queries use their constructor-bound Object; candidate lookup
//! still depends on registered targets. This receipt is not world-isolation proof.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 257 legacy AI states dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/unit/registry.rs` adapter gate
//! - `GameLogic/src/object/unit/ai_runtime.rs` mood dispatch and adapter
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Unit dual-world empty-gate residual method names.
pub const LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE258: &[&str] = &[
    "dual_world_registry_unavailable",
    "find_enemy_in_container",
    "get_next_mood_target_id",
    "ai_guard_object",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE258: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_UNIT_EMPTY_GATES",
    "LIVE_UNIT_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE258: &[&str] = &[
    "click_live_unit_dual_world_empty_gate_ok_prepare",
    "click_live_unit_dual_world_empty_gate_ok_live",
    "click_live_unit_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_unit_dual_world_empty_gate_method_names_residual_wave258() -> bool {
    LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE258.len() == 5
        && residual_name_index(
            LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE258,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE258,
            "get_next_mood_target_id",
        ) == Some(2)
        && residual_name_index(
            LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE258,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_unit_dual_world_empty_gate_nav_commands_residual_wave258() -> bool {
    LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE258.len() == 4
        && residual_name_index(
            LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE258,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE258,
            "LIVE_UNIT_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_UNIT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE258.len() == 3
}

/// Wave 258 composite residual honesty pack.
pub fn honesty_live_unit_dual_world_empty_gate_residual_pack_wave258() -> bool {
    honesty_live_unit_dual_world_empty_gate_method_names_residual_wave258()
        && honesty_live_unit_dual_world_empty_gate_nav_commands_residual_wave258()
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

/// Source residual: standalone Unit empty dual-world short-circuits.
/// The native mood query uses its constructor-bound Object; this receipt
/// covers the explicit legacy adapter, not native query parity.
pub fn honesty_unit_dual_world_empty_gate_source() -> bool {
    let registry = include_str!("../../../../GameEngine/GameLogic/src/object/unit/registry.rs");
    let runtime = include_str!("../../../../GameEngine/GameLogic/src/object/unit/ai_runtime.rs");
    let Some(gate) = fn_body(registry, "fn dual_world_registry_unavailable(") else {
        return false;
    };
    let Some(legacy) = fn_body(runtime, "fn get_legacy_mood_target(") else {
        return false;
    };
    let Some(dispatch) = fn_body(runtime, "fn get_next_mood_target_for_state(") else {
        return false;
    };
    gate.contains("OBJECT_REGISTRY.is_empty()")
        && legacy.contains("dual_world_registry_unavailable")
        && legacy.contains("get_unit_arc(self.unit_id)")
        && legacy.contains("return INVALID_ID")
        && dispatch.contains("if self.owner.is_none()")
        && dispatch.contains("return self.get_legacy_mood_target(")
        && dispatch.contains("self.owner.as_ref().and_then(Weak::upgrade)")
        && !dispatch.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_unit_dual_world_empty_gate_honesty() -> bool {
    honesty_live_unit_dual_world_empty_gate_residual_pack_wave258()
        && honesty_unit_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_unit_dual_world_empty_gate_method_names_residual_wave258());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_unit_dual_world_empty_gate_nav_commands_residual_wave258());
    }

    #[test]
    fn wave258_composite_pack() {
        assert!(honesty_live_unit_dual_world_empty_gate_residual_pack_wave258());
    }

    #[test]
    fn unit_dual_world_empty_gate_sources() {
        assert!(honesty_unit_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_unit_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_unit_dual_world_empty_gate_honesty(),
            "unit dual-world empty gate residual must latch"
        );
    }
}
