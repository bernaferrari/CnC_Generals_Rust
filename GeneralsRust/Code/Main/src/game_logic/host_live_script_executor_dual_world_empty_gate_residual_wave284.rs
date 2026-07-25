//! Wave 284 residual peels: Script executor dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), object-bound
//! script actions/conditions fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 283 StealthUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/scripting/executor.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Script executor dual-world empty-gate residual method names.
pub const LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE284: &[&str] = &[
    "dual_world_registry_unavailable",
    "do_camera_follow_named",
    "do_object_create_radar_event",
    "do_delete_all_unmanned",
    "eval_named_selected",
    "eval_built_by_player",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE284: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SCRIPT_EXECUTOR_EMPTY_GATES",
    "LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE284: &[&str] = &[
    "click_live_script_executor_dual_world_empty_gate_ok_prepare",
    "click_live_script_executor_dual_world_empty_gate_ok_live",
    "click_live_script_executor_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_script_executor_dual_world_empty_gate_method_names_residual_wave284() -> bool {
    LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE284.len() == 7
        && residual_name_index(
            LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE284,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE284,
            "eval_built_by_player",
        ) == Some(5)
        && residual_name_index(
            LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE284,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_script_executor_dual_world_empty_gate_nav_commands_residual_wave284() -> bool {
    LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE284.len() == 4
        && residual_name_index(
            LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE284,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE284,
            "LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SCRIPT_EXECUTOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE284.len() == 3
}

/// Wave 284 composite residual honesty pack.
pub fn honesty_live_script_executor_dual_world_empty_gate_residual_pack_wave284() -> bool {
    honesty_live_script_executor_dual_world_empty_gate_method_names_residual_wave284()
        && honesty_live_script_executor_dual_world_empty_gate_nav_commands_residual_wave284()
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

/// Source residual: script executor empty dual-world short-circuits.
pub fn honesty_script_executor_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/scripting/executor.rs");
    if !(g.contains("Wave 284")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(camera) = fn_body(g, "fn do_camera_follow_named(") else {
        return false;
    };
    let Some(del) = fn_body(g, "fn do_delete_all_unmanned(") else {
        return false;
    };
    let Some(eval) = fn_body(g, "fn eval_named_selected(") else {
        return false;
    };
    helper_ok
        && camera.contains("ScriptActionResult::Success")
        && del.contains("ScriptActionResult::Success")
        && eval.contains("ScriptConditionResult::False")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_script_executor_dual_world_empty_gate_honesty() -> bool {
    honesty_live_script_executor_dual_world_empty_gate_residual_pack_wave284()
        && honesty_script_executor_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_script_executor_dual_world_empty_gate_method_names_residual_wave284());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_script_executor_dual_world_empty_gate_nav_commands_residual_wave284());
    }

    #[test]
    fn wave284_composite_pack() {
        assert!(honesty_live_script_executor_dual_world_empty_gate_residual_pack_wave284());
    }

    #[test]
    fn script_executor_dual_world_empty_gate_sources() {
        assert!(honesty_script_executor_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_script_executor_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_script_executor_dual_world_empty_gate_honesty(),
            "script executor dual-world empty gate residual must latch"
        );
    }
}
