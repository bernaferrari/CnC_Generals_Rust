//! Wave 309 residual peels: JetAIUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), jet AI
//! command/update helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 308 StatusBitsUpgrade dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/ai_update/jet_ai_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Jet AI dual-world empty-gate residual method names.
pub const LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE309: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_object",
    "handle_command",
    "do_landing_command",
    "prune_dead_targeters",
    "update",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE309: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_JET_AI_EMPTY_GATES",
    "LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE309: &[&str] = &[
    "click_live_jet_ai_dual_world_empty_gate_ok_prepare",
    "click_live_jet_ai_dual_world_empty_gate_ok_live",
    "click_live_jet_ai_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_jet_ai_dual_world_empty_gate_method_names_residual_wave309() -> bool {
    LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE309.len() == 7
        && residual_name_index(
            LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE309,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE309,
            "update",
        ) == Some(5)
        && residual_name_index(
            LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE309,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_jet_ai_dual_world_empty_gate_nav_commands_residual_wave309() -> bool {
    LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE309.len() == 4
        && residual_name_index(
            LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE309,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE309,
            "LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_JET_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE309.len() == 3
}

/// Wave 309 composite residual honesty pack.
pub fn honesty_live_jet_ai_dual_world_empty_gate_residual_pack_wave309() -> bool {
    honesty_live_jet_ai_dual_world_empty_gate_method_names_residual_wave309()
        && honesty_live_jet_ai_dual_world_empty_gate_nav_commands_residual_wave309()
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

/// Source residual: jet AI empty dual-world short-circuits.
pub fn honesty_jet_ai_dual_world_empty_gate_source() -> bool {
    let g =
        include_str!("../../../GameEngine/GameLogic/src/object/update/ai_update/jet_ai_update.rs");
    if !(g.contains("Wave 309")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(get) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update(") else {
        return false;
    };
    // prefer the JetAIUpdate::update that gates afterburner path
    helper_ok
        && get.contains("return None")
        && update.contains("return;")
        && update.contains("afterburner_sound")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_jet_ai_dual_world_empty_gate_honesty() -> bool {
    honesty_live_jet_ai_dual_world_empty_gate_residual_pack_wave309()
        && honesty_jet_ai_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_jet_ai_dual_world_empty_gate_method_names_residual_wave309());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_jet_ai_dual_world_empty_gate_nav_commands_residual_wave309());
    }

    #[test]
    fn wave309_composite_pack() {
        assert!(honesty_live_jet_ai_dual_world_empty_gate_residual_pack_wave309());
    }

    #[test]
    fn jet_ai_dual_world_empty_gate_sources() {
        assert!(honesty_jet_ai_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_jet_ai_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_jet_ai_dual_world_empty_gate_honesty(),
            "jet ai dual-world empty gate residual must latch"
        );
    }
}
