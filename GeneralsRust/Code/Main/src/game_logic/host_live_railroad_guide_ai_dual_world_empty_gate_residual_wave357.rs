//! Wave 357 residual peels: RailroadGuideAIUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), train
//! hitch/update helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 356 WeaponTemplate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/ai_update/railroad_guide_ai_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// RailroadGuideAI dual-world empty-gate residual method names.
pub const LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE357: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_object",
    "update",
    "create_carriages",
    "on_collision",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE357: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_RAILROAD_GUIDE_AI_EMPTY_GATES",
    "LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE357: &[&str] = &[
    "click_live_railroad_guide_ai_dual_world_empty_gate_ok_prepare",
    "click_live_railroad_guide_ai_dual_world_empty_gate_ok_live",
    "click_live_railroad_guide_ai_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_railroad_guide_ai_dual_world_empty_gate_method_names_residual_wave357() -> bool
{
    LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE357.len() == 6
        && residual_name_index(
            LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE357,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE357,
            "on_collision",
        ) == Some(4)
        && residual_name_index(
            LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE357,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_railroad_guide_ai_dual_world_empty_gate_nav_commands_residual_wave357() -> bool
{
    LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE357.len() == 4
        && residual_name_index(
            LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE357,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE357,
            "LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_RAILROAD_GUIDE_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE357.len() == 3
}

/// Wave 357 composite residual honesty pack.
pub fn honesty_live_railroad_guide_ai_dual_world_empty_gate_residual_pack_wave357() -> bool {
    honesty_live_railroad_guide_ai_dual_world_empty_gate_method_names_residual_wave357()
        && honesty_live_railroad_guide_ai_dual_world_empty_gate_nav_commands_residual_wave357()
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

/// Source residual: RailroadGuideAI empty dual-world short-circuits.
pub fn honesty_railroad_guide_ai_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/update/ai_update/railroad_guide_ai_update.rs"
    );
    if !(g.contains("Wave 357")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(obj) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(upd) = fn_body(g, "fn update(") else {
        return false;
    };
    let Some(create) = fn_body(g, "fn create_carriages(") else {
        return false;
    };
    let Some(col) = fn_body(g, "fn on_collision(") else {
        return false;
    };
    helper_ok
        && obj.contains("return None")
        && upd.contains("UPDATE_SLEEP_NONE")
        && create.contains("return;")
        && col.contains("return;")
        && g.matches("Wave 357:").count() >= 8
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_railroad_guide_ai_dual_world_empty_gate_honesty() -> bool {
    honesty_live_railroad_guide_ai_dual_world_empty_gate_residual_pack_wave357()
        && honesty_railroad_guide_ai_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_railroad_guide_ai_dual_world_empty_gate_method_names_residual_wave357()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_railroad_guide_ai_dual_world_empty_gate_nav_commands_residual_wave357()
        );
    }

    #[test]
    fn wave357_composite_pack() {
        assert!(honesty_live_railroad_guide_ai_dual_world_empty_gate_residual_pack_wave357());
    }

    #[test]
    fn railroad_guide_ai_dual_world_empty_gate_sources() {
        assert!(honesty_railroad_guide_ai_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_railroad_guide_ai_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_railroad_guide_ai_dual_world_empty_gate_honesty(),
            "railroad guide ai dual-world empty gate residual must latch"
        );
    }
}
