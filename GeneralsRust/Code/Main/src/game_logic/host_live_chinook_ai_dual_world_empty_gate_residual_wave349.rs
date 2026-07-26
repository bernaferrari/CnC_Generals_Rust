//! Wave 349 residual peels: ChinookAIUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), combat-drop/
//! attack/supply helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 348 ScriptEngine dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/ai_update/chinook_ai_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// ChinookAI dual-world empty-gate residual method names.
pub const LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE349: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_potential_rappeller",
    "start_combat_drop",
    "update_combat_drop",
    "update",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE349: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_CHINOOK_AI_EMPTY_GATES",
    "LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE349: &[&str] = &[
    "click_live_chinook_ai_dual_world_empty_gate_ok_prepare",
    "click_live_chinook_ai_dual_world_empty_gate_ok_live",
    "click_live_chinook_ai_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_chinook_ai_dual_world_empty_gate_method_names_residual_wave349() -> bool {
    LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE349.len() == 6
        && residual_name_index(
            LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE349,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE349,
            "update",
        ) == Some(4)
        && residual_name_index(
            LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE349,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_chinook_ai_dual_world_empty_gate_nav_commands_residual_wave349() -> bool {
    LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE349.len() == 4
        && residual_name_index(
            LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE349,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE349,
            "LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CHINOOK_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE349.len() == 3
}

/// Wave 349 composite residual honesty pack.
pub fn honesty_live_chinook_ai_dual_world_empty_gate_residual_pack_wave349() -> bool {
    honesty_live_chinook_ai_dual_world_empty_gate_method_names_residual_wave349()
        && honesty_live_chinook_ai_dual_world_empty_gate_nav_commands_residual_wave349()
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

/// Source residual: ChinookAI empty dual-world short-circuits.
pub fn honesty_chinook_ai_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/update/ai_update/chinook_ai_update.rs"
    );
    if !(g.contains("Wave 349")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(rep) = fn_body(g, "fn get_potential_rappeller(") else {
        return false;
    };
    let Some(start) = fn_body(g, "fn start_combat_drop(") else {
        return false;
    };
    let Some(drop) = fn_body(g, "fn update_combat_drop(") else {
        return false;
    };
    let Some(upd) = fn_body(g, "fn update(") else {
        return false;
    };
    helper_ok
        && rep.contains("return None")
        && start.contains("return false")
        && drop.contains("return true")
        && upd.contains("return Ok(())")
        && g.matches("Wave 349:").count() >= 15
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_chinook_ai_dual_world_empty_gate_honesty() -> bool {
    honesty_live_chinook_ai_dual_world_empty_gate_residual_pack_wave349()
        && honesty_chinook_ai_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_chinook_ai_dual_world_empty_gate_method_names_residual_wave349());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_chinook_ai_dual_world_empty_gate_nav_commands_residual_wave349());
    }

    #[test]
    fn wave349_composite_pack() {
        assert!(honesty_live_chinook_ai_dual_world_empty_gate_residual_pack_wave349());
    }

    #[test]
    fn chinook_ai_dual_world_empty_gate_sources() {
        assert!(honesty_chinook_ai_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_chinook_ai_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_chinook_ai_dual_world_empty_gate_honesty(),
            "chinook ai dual-world empty gate residual must latch"
        );
    }
}
