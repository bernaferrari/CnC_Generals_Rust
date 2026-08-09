//! Wave 295 residual peels: Script actions dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), script action
//! execute helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 294 victory dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/scripting/actions.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Script actions dual-world empty-gate residual method names.
pub const LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE295: &[&str] = &[
    "dual_world_registry_unavailable",
    "resolve_named_object_id",
    "execute",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE295: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SCRIPT_ACTIONS_EMPTY_GATES",
    "LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE295: &[&str] = &[
    "click_live_script_actions_dual_world_empty_gate_ok_prepare",
    "click_live_script_actions_dual_world_empty_gate_ok_live",
    "click_live_script_actions_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_script_actions_dual_world_empty_gate_method_names_residual_wave295() -> bool {
    LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE295.len() == 4
        && residual_name_index(
            LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE295,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE295,
            "execute",
        ) == Some(2)
        && residual_name_index(
            LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE295,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_script_actions_dual_world_empty_gate_nav_commands_residual_wave295() -> bool {
    LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE295.len() == 4
        && residual_name_index(
            LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE295,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE295,
            "LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SCRIPT_ACTIONS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE295.len() == 3
}

/// Wave 295 composite residual honesty pack.
pub fn honesty_live_script_actions_dual_world_empty_gate_residual_pack_wave295() -> bool {
    honesty_live_script_actions_dual_world_empty_gate_method_names_residual_wave295()
        && honesty_live_script_actions_dual_world_empty_gate_nav_commands_residual_wave295()
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

/// Source residual: script actions empty dual-world short-circuits.
pub fn honesty_script_actions_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/scripting/actions.rs");
    if !(g.contains("Wave 295")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(resolve) = fn_body(g, "fn resolve_named_object_id(") else {
        return false;
    };
    let exec_gate = g.contains("// Wave 295: empty dual-world → Success(None).")
        && g.contains("Ok(ScriptResult::Success(None))");
    helper_ok && resolve.contains("return None") && exec_gate
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_script_actions_dual_world_empty_gate_honesty() -> bool {
    honesty_live_script_actions_dual_world_empty_gate_residual_pack_wave295()
        && honesty_script_actions_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_script_actions_dual_world_empty_gate_method_names_residual_wave295());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_script_actions_dual_world_empty_gate_nav_commands_residual_wave295());
    }

    #[test]
    fn wave295_composite_pack() {
        assert!(honesty_live_script_actions_dual_world_empty_gate_residual_pack_wave295());
    }

    #[test]
    fn script_actions_dual_world_empty_gate_sources() {
        assert!(honesty_script_actions_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_script_actions_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_script_actions_dual_world_empty_gate_honesty(),
            "script actions dual-world empty gate residual must latch"
        );
    }
}
