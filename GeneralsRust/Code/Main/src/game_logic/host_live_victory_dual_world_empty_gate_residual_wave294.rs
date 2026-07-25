//! Wave 294 residual peels: Victory conditions dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), victory
//! progress helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 293 AIBuildList dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/scripting/victory.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Victory dual-world empty-gate residual method names.
pub const LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294: &[&str] = &[
    "dual_world_registry_unavailable",
    "calculate_destruction_progress",
    "calculate_structure_destruction_progress",
    "calculate_rescue_progress",
    "calculate_kill_progress",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE294: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_VICTORY_EMPTY_GATES",
    "LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE294: &[&str] = &[
    "click_live_victory_dual_world_empty_gate_ok_prepare",
    "click_live_victory_dual_world_empty_gate_ok_live",
    "click_live_victory_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_victory_dual_world_empty_gate_method_names_residual_wave294() -> bool {
    LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294.len() == 6
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294,
            "calculate_kill_progress",
        ) == Some(4)
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE294,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_victory_dual_world_empty_gate_nav_commands_residual_wave294() -> bool {
    LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE294.len() == 4
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE294,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE294,
            "LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_VICTORY_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE294.len() == 3
}

/// Wave 294 composite residual honesty pack.
pub fn honesty_live_victory_dual_world_empty_gate_residual_pack_wave294() -> bool {
    honesty_live_victory_dual_world_empty_gate_method_names_residual_wave294()
        && honesty_live_victory_dual_world_empty_gate_nav_commands_residual_wave294()
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

/// Source residual: victory empty dual-world short-circuits.
pub fn honesty_victory_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/scripting/victory.rs");
    if !(g.contains("Wave 294")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(kill) = fn_body(g, "fn calculate_kill_progress(") else {
        return false;
    };
    let Some(dest) = fn_body(g, "fn calculate_destruction_progress(") else {
        return false;
    };
    helper_ok && kill.contains("Ok(0.0)") && dest.contains("Ok(0.0)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_victory_dual_world_empty_gate_honesty() -> bool {
    honesty_live_victory_dual_world_empty_gate_residual_pack_wave294()
        && honesty_victory_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_victory_dual_world_empty_gate_method_names_residual_wave294());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_victory_dual_world_empty_gate_nav_commands_residual_wave294());
    }

    #[test]
    fn wave294_composite_pack() {
        assert!(honesty_live_victory_dual_world_empty_gate_residual_pack_wave294());
    }

    #[test]
    fn victory_dual_world_empty_gate_sources() {
        assert!(honesty_victory_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_victory_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_victory_dual_world_empty_gate_honesty(),
            "victory dual-world empty gate residual must latch"
        );
    }
}
