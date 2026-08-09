//! Wave 290 residual peels: Async AI player dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), async AI
//! state/threat/task helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 289 weapon.rs dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/async_player.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Async player dual-world empty-gate residual method names.
pub const LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290: &[&str] = &[
    "dual_world_registry_unavailable",
    "update_game_state",
    "evaluate_threats",
    "generate_tasks_from_strategy",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE290: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_ASYNC_PLAYER_EMPTY_GATES",
    "LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE290: &[&str] = &[
    "click_live_async_player_dual_world_empty_gate_ok_prepare",
    "click_live_async_player_dual_world_empty_gate_ok_live",
    "click_live_async_player_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_async_player_dual_world_empty_gate_method_names_residual_wave290() -> bool {
    LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290.len() == 5
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290,
            "generate_tasks_from_strategy",
        ) == Some(3)
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_async_player_dual_world_empty_gate_nav_commands_residual_wave290() -> bool {
    LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE290.len() == 4
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE290,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE290,
            "LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE290.len() == 3
}

/// Wave 290 composite residual honesty pack.
pub fn honesty_live_async_player_dual_world_empty_gate_residual_pack_wave290() -> bool {
    honesty_live_async_player_dual_world_empty_gate_method_names_residual_wave290()
        && honesty_live_async_player_dual_world_empty_gate_nav_commands_residual_wave290()
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

/// Source residual: async player empty dual-world short-circuits.
pub fn honesty_async_player_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/ai/async_player.rs");
    if !(g.contains("Wave 290")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(state) = fn_body(g, "fn update_game_state(") else {
        return false;
    };
    let Some(tasks) = fn_body(g, "fn generate_tasks_from_strategy(") else {
        return false;
    };
    helper_ok && state.contains("Ok(())") && tasks.contains("Ok(Vec::new())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_async_player_dual_world_empty_gate_honesty() -> bool {
    honesty_live_async_player_dual_world_empty_gate_residual_pack_wave290()
        && honesty_async_player_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_async_player_dual_world_empty_gate_method_names_residual_wave290());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_async_player_dual_world_empty_gate_nav_commands_residual_wave290());
    }

    #[test]
    fn wave290_composite_pack() {
        assert!(honesty_live_async_player_dual_world_empty_gate_residual_pack_wave290());
    }

    #[test]
    fn async_player_dual_world_empty_gate_sources() {
        assert!(honesty_async_player_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_async_player_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_async_player_dual_world_empty_gate_honesty(),
            "async player dual-world empty gate residual must latch"
        );
    }
}
