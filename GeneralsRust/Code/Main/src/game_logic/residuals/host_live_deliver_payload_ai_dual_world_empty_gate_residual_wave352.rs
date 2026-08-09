//! Wave 352 residual peels: DeliverPayloadAIUpdate dual-world empty short-circuits.
//! Host-only (not in GameLogic + empty OBJECT_REGISTRY) still fail-closed.
//! In-game C++ path runs (`TheGameLogic::is_in_game()` → no skip).
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 351 DozerAI dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/ai_update/deliver_payload_ai_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// DeliverPayloadAI dual-world empty-gate residual method names.
pub const LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE352: &[&str] = &[
    "dual_world_registry_unavailable",
    "ai_move_to_position",
    "update_delivering",
    "deliver_payload",
    "is_close_enough_to_target",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE352: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_DELIVER_PAYLOAD_AI_EMPTY_GATES",
    "LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE352: &[&str] = &[
    "click_live_deliver_payload_ai_dual_world_empty_gate_ok_prepare",
    "click_live_deliver_payload_ai_dual_world_empty_gate_ok_live",
    "click_live_deliver_payload_ai_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_deliver_payload_ai_dual_world_empty_gate_method_names_residual_wave352() -> bool
{
    LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE352.len() == 6
        && residual_name_index(
            LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE352,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE352,
            "is_close_enough_to_target",
        ) == Some(4)
        && residual_name_index(
            LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE352,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_deliver_payload_ai_dual_world_empty_gate_nav_commands_residual_wave352() -> bool
{
    LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE352.len() == 4
        && residual_name_index(
            LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE352,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE352,
            "LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_DELIVER_PAYLOAD_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE352.len() == 3
}

/// Wave 352 composite residual honesty pack.
pub fn honesty_live_deliver_payload_ai_dual_world_empty_gate_residual_pack_wave352() -> bool {
    honesty_live_deliver_payload_ai_dual_world_empty_gate_method_names_residual_wave352()
        && honesty_live_deliver_payload_ai_dual_world_empty_gate_nav_commands_residual_wave352()
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

/// Source residual: DeliverPayloadAI empty dual-world short-circuits.
pub fn honesty_deliver_payload_ai_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/update/ai_update/deliver_payload_ai_update.rs"
    );
    if !(g.contains("Wave 352")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains("fn dual_world_registry_unavailable")
        && g.contains("if TheGameLogic::is_in_game()")
        && g.contains("OBJECT_REGISTRY.is_empty()");
    let Some(mv) = fn_body(g, "fn ai_move_to_position(") else {
        return false;
    };
    let Some(upd) = fn_body(g, "fn update_delivering(") else {
        return false;
    };
    let Some(del) = fn_body(g, "fn deliver_payload(") else {
        return false;
    };
    let Some(close) = fn_body(g, "fn is_close_enough_to_target(") else {
        return false;
    };
    helper_ok
        && mv.contains("return;")
        && upd.contains("StateReturnType::Continue")
        && del.contains("return;")
        && close.contains("return false")
        && g.matches("Wave 352:").count() >= 15
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_deliver_payload_ai_dual_world_empty_gate_honesty() -> bool {
    honesty_live_deliver_payload_ai_dual_world_empty_gate_residual_pack_wave352()
        && honesty_deliver_payload_ai_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_deliver_payload_ai_dual_world_empty_gate_method_names_residual_wave352()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_deliver_payload_ai_dual_world_empty_gate_nav_commands_residual_wave352()
        );
    }

    #[test]
    fn wave352_composite_pack() {
        assert!(honesty_live_deliver_payload_ai_dual_world_empty_gate_residual_pack_wave352());
    }

    #[test]
    fn deliver_payload_ai_dual_world_empty_gate_sources() {
        assert!(honesty_deliver_payload_ai_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_deliver_payload_ai_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_deliver_payload_ai_dual_world_empty_gate_honesty(),
            "deliver payload ai dual-world empty gate residual must latch"
        );
    }
}
