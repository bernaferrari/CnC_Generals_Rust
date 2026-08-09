//! Wave 398 residual peels: AI Groups dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), AI group
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 397 AI Dock dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/groups.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AI Groups dual-world empty-gate residual method names.
pub const LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE398: &[&str] = &[
    "dual_world_registry_unavailable",
    "toggle_overcharge",
    "recalculate_group_properties",
    "coordinate_focus_fire",
    "coordinate_defensive_tactics",
    "coordinate_support_actions",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE398: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_GROUPS_EMPTY_GATES",
    "LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE398: &[&str] = &[
    "click_live_ai_groups_dual_world_empty_gate_ok_prepare",
    "click_live_ai_groups_dual_world_empty_gate_ok_live",
    "click_live_ai_groups_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_groups_dual_world_empty_gate_method_names_residual_wave398() -> bool {
    LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE398.len() == 7
        && residual_name_index(
            LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE398,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE398,
            "coordinate_support_actions",
        ) == Some(5)
        && residual_name_index(
            LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE398,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_groups_dual_world_empty_gate_nav_commands_residual_wave398() -> bool {
    LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE398.len() == 4
        && residual_name_index(
            LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE398,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE398,
            "LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_GROUPS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE398.len() == 3
}

/// Wave 398 composite residual honesty pack.
pub fn honesty_live_ai_groups_dual_world_empty_gate_residual_pack_wave398() -> bool {
    honesty_live_ai_groups_dual_world_empty_gate_method_names_residual_wave398()
        && honesty_live_ai_groups_dual_world_empty_gate_nav_commands_residual_wave398()
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

/// Source residual: AI Groups empty dual-world short-circuits.
pub fn honesty_ai_groups_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/ai/groups.rs");
    if !(g.contains("Wave 398")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(toggle) = fn_body(g, "fn toggle_overcharge(") else {
        return false;
    };
    let Some(recalc) = fn_body(g, "fn recalculate_group_properties(") else {
        return false;
    };
    let Some(focus) = fn_body(g, "fn coordinate_focus_fire(") else {
        return false;
    };
    let Some(support) = fn_body(g, "fn coordinate_support_actions(") else {
        return false;
    };
    helper_ok
        && toggle.contains("return;")
        && recalc.contains("return;")
        && focus.contains("return Ok(())")
        && support.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_groups_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_groups_dual_world_empty_gate_residual_pack_wave398()
        && honesty_ai_groups_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ai_groups_dual_world_empty_gate_method_names_residual_wave398());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ai_groups_dual_world_empty_gate_nav_commands_residual_wave398());
    }

    #[test]
    fn wave398_composite_pack() {
        assert!(honesty_live_ai_groups_dual_world_empty_gate_residual_pack_wave398());
    }

    #[test]
    fn ai_groups_dual_world_empty_gate_sources() {
        assert!(honesty_ai_groups_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_groups_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_groups_dual_world_empty_gate_honesty(),
            "ai groups dual-world empty gate residual must latch"
        );
    }
}
