//! Wave 407 residual peels: RailedTransportAIUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), railed transport
//! AI helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 361 RailedTransportDock and Wave 406 OCLSpecialPower residuals.
//!
//! Sources:
//! - `GameLogic/src/object/update/ai_update/railed_transport_ai_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// RailedTransportAIUpdate dual-world empty-gate residual method names.
pub const LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE407: &[&str] = &[
    "dual_world_registry_unavailable",
    "update",
    "set_in_transit",
    "pick_and_move_to_initial_location",
    "private_execute_railed_transport",
    "private_evacuate",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE407: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_RAILED_TRANSPORT_AI_UPDATE_EMPTY_GATES",
    "LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE407:
    &[&str] = &[
    "click_live_railed_transport_ai_update_dual_world_empty_gate_ok_prepare",
    "click_live_railed_transport_ai_update_dual_world_empty_gate_ok_live",
    "click_live_railed_transport_ai_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_railed_transport_ai_update_dual_world_empty_gate_method_names_residual_wave407()
-> bool {
    LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE407.len() == 7
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE407,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE407,
            "private_evacuate",
        ) == Some(5)
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE407,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave407()
-> bool {
    LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE407.len() == 4
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE407,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE407,
            "LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_RAILED_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE407
            .len()
            == 3
}

/// Wave 407 composite residual honesty pack.
pub fn honesty_live_railed_transport_ai_update_dual_world_empty_gate_residual_pack_wave407() -> bool
{
    honesty_live_railed_transport_ai_update_dual_world_empty_gate_method_names_residual_wave407()
        && honesty_live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave407()
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

/// Source residual: RailedTransportAIUpdate empty dual-world short-circuits.
pub fn honesty_railed_transport_ai_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/update/ai_update/railed_transport_ai_update.rs"
    );
    if !(g.contains("Wave 407")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(update) = fn_body(g, "fn update(") else {
        return false;
    };
    let Some(transit) = fn_body(g, "fn set_in_transit(") else {
        return false;
    };
    let Some(pick) = fn_body(g, "fn pick_and_move_to_initial_location(") else {
        return false;
    };
    let Some(exec) = fn_body(g, "fn private_execute_railed_transport(") else {
        return false;
    };
    let Some(evac) = fn_body(g, "fn private_evacuate(") else {
        return false;
    };
    helper_ok
        && update.contains("return Ok(())")
        && transit.contains("return;")
        && pick.contains("return Ok(())")
        && exec.contains("return;")
        && evac.contains("return;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_railed_transport_ai_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_railed_transport_ai_update_dual_world_empty_gate_residual_pack_wave407()
        && honesty_railed_transport_ai_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_railed_transport_ai_update_dual_world_empty_gate_method_names_residual_wave407()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_railed_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave407()
        );
    }

    #[test]
    fn wave407_composite_pack() {
        assert!(
            honesty_live_railed_transport_ai_update_dual_world_empty_gate_residual_pack_wave407()
        );
    }

    #[test]
    fn railed_transport_ai_update_dual_world_empty_gate_sources() {
        assert!(honesty_railed_transport_ai_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_railed_transport_ai_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_railed_transport_ai_update_dual_world_empty_gate_honesty(),
            "railed transport ai update dual-world empty gate residual must latch"
        );
    }
}
