//! Wave 369 residual peels: AssaultTransportAIUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), assault
//! transport AI helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 368 VeterancyCrateCollide dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/ai_update/assault_transport_ai_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AssaultTransportAIUpdate dual-world empty-gate residual method names.
pub const LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE369: &[&str] = &[
    "dual_world_registry_unavailable",
    "update",
    "prune_missing_members",
    "add_new_members",
    "is_attack_pointless",
    "retrieve_members",
    "give_final_orders",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE369: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_ASSAULT_TRANSPORT_EMPTY_GATES",
    "LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE369:
    &[&str] = &[
    "click_live_assault_transport_ai_update_dual_world_empty_gate_ok_prepare",
    "click_live_assault_transport_ai_update_dual_world_empty_gate_ok_live",
    "click_live_assault_transport_ai_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_assault_transport_ai_update_dual_world_empty_gate_method_names_residual_wave369()
-> bool {
    LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE369.len() == 8
        && residual_name_index(
            LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE369,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE369,
            "give_final_orders",
        ) == Some(6)
        && residual_name_index(
            LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE369,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave369()
-> bool {
    LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE369.len() == 4
        && residual_name_index(
            LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE369,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE369,
            "LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ASSAULT_TRANSPORT_AI_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE369
            .len()
            == 3
}

/// Wave 369 composite residual honesty pack.
pub fn honesty_live_assault_transport_ai_update_dual_world_empty_gate_residual_pack_wave369() -> bool
{
    honesty_live_assault_transport_ai_update_dual_world_empty_gate_method_names_residual_wave369()
        && honesty_live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave369()
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

/// Source residual: AssaultTransportAIUpdate empty dual-world short-circuits.
pub fn honesty_assault_transport_ai_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/update/ai_update/assault_transport_ai_update.rs"
    );
    if !(g.contains("Wave 369")
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
    let Some(prune) = fn_body(g, "fn prune_missing_members(") else {
        return false;
    };
    let Some(add) = fn_body(g, "fn add_new_members(") else {
        return false;
    };
    let Some(pointless) = fn_body(g, "fn is_attack_pointless(") else {
        return false;
    };
    let Some(orders) = fn_body(g, "fn give_final_orders(") else {
        return false;
    };
    helper_ok
        && update.contains("return Ok(())")
        && prune.contains("return;")
        && add.contains("return;")
        && pointless.contains("return false")
        && orders.contains("return;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_assault_transport_ai_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_assault_transport_ai_update_dual_world_empty_gate_residual_pack_wave369()
        && honesty_assault_transport_ai_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_assault_transport_ai_update_dual_world_empty_gate_method_names_residual_wave369());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_assault_transport_ai_update_dual_world_empty_gate_nav_commands_residual_wave369());
    }

    #[test]
    fn wave369_composite_pack() {
        assert!(
            honesty_live_assault_transport_ai_update_dual_world_empty_gate_residual_pack_wave369()
        );
    }

    #[test]
    fn assault_transport_ai_update_dual_world_empty_gate_sources() {
        assert!(honesty_assault_transport_ai_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_assault_transport_ai_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_assault_transport_ai_update_dual_world_empty_gate_honesty(),
            "assault transport ai update dual-world empty gate residual must latch"
        );
    }
}
