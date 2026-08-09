//! Wave 293 residual peels: AIBuildList dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), build-list
//! query helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 292 skirmish conditions dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/ai_build_list.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AI build list dual-world empty-gate residual method names.
pub const LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE293: &[&str] = &[
    "dual_world_registry_unavailable",
    "player_has_building",
    "get_player_base_center",
    "get_builder_position",
    "find_idle_builders",
    "count_player_units",
    "count_nearby_buildings",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE293: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_BUILD_LIST_EMPTY_GATES",
    "LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE293: &[&str] = &[
    "click_live_ai_build_list_dual_world_empty_gate_ok_prepare",
    "click_live_ai_build_list_dual_world_empty_gate_ok_live",
    "click_live_ai_build_list_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_build_list_dual_world_empty_gate_method_names_residual_wave293() -> bool {
    LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE293.len() == 8
        && residual_name_index(
            LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE293,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE293,
            "find_idle_builders",
        ) == Some(4)
        && residual_name_index(
            LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE293,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_build_list_dual_world_empty_gate_nav_commands_residual_wave293() -> bool {
    LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE293.len() == 4
        && residual_name_index(
            LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE293,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE293,
            "LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_BUILD_LIST_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE293.len() == 3
}

/// Wave 293 composite residual honesty pack.
pub fn honesty_live_ai_build_list_dual_world_empty_gate_residual_pack_wave293() -> bool {
    honesty_live_ai_build_list_dual_world_empty_gate_method_names_residual_wave293()
        && honesty_live_ai_build_list_dual_world_empty_gate_nav_commands_residual_wave293()
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

/// Source residual: AI build list empty dual-world short-circuits.
pub fn honesty_ai_build_list_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/ai/ai_build_list.rs");
    if !(g.contains("Wave 293")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(has) = fn_body(g, "fn player_has_building(") else {
        return false;
    };
    let Some(idle) = fn_body(g, "fn find_idle_builders(") else {
        return false;
    };
    helper_ok && has.contains("Ok(false)") && idle.contains("Ok(Vec::new())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_build_list_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_build_list_dual_world_empty_gate_residual_pack_wave293()
        && honesty_ai_build_list_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ai_build_list_dual_world_empty_gate_method_names_residual_wave293());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ai_build_list_dual_world_empty_gate_nav_commands_residual_wave293());
    }

    #[test]
    fn wave293_composite_pack() {
        assert!(honesty_live_ai_build_list_dual_world_empty_gate_residual_pack_wave293());
    }

    #[test]
    fn ai_build_list_dual_world_empty_gate_sources() {
        assert!(honesty_ai_build_list_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_build_list_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_build_list_dual_world_empty_gate_honesty(),
            "ai build list dual-world empty gate residual must latch"
        );
    }
}
