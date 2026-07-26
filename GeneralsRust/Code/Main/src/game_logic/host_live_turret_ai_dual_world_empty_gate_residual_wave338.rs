//! Wave 338 residual peels: TurretAI dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), turret
//! aim/update helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 337 Economy dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/turret_ai_full.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Turret AI dual-world empty-gate residual method names.
pub const LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE338: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_aimed_at_target",
    "is_owners_cur_weapon_on_turret",
    "turret_ai_update",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE338: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_TURRET_AI_EMPTY_GATES",
    "LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE338: &[&str] = &[
    "click_live_turret_ai_dual_world_empty_gate_ok_prepare",
    "click_live_turret_ai_dual_world_empty_gate_ok_live",
    "click_live_turret_ai_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_turret_ai_dual_world_empty_gate_method_names_residual_wave338() -> bool {
    LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE338.len() == 5
        && residual_name_index(
            LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE338,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE338,
            "turret_ai_update",
        ) == Some(3)
        && residual_name_index(
            LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE338,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_turret_ai_dual_world_empty_gate_nav_commands_residual_wave338() -> bool {
    LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE338.len() == 4
        && residual_name_index(
            LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE338,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE338,
            "LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_TURRET_AI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE338.len() == 3
}

/// Wave 338 composite residual honesty pack.
pub fn honesty_live_turret_ai_dual_world_empty_gate_residual_pack_wave338() -> bool {
    honesty_live_turret_ai_dual_world_empty_gate_method_names_residual_wave338()
        && honesty_live_turret_ai_dual_world_empty_gate_nav_commands_residual_wave338()
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

/// Source residual: turret AI empty dual-world short-circuits.
pub fn honesty_turret_ai_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/ai/turret_ai_full.rs");
    if !(g.contains("Wave 338")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(aim) = fn_body(g, "fn is_aimed_at_target(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn turret_ai_update(") else {
        return false;
    };
    helper_ok
        && aim.contains("return false")
        && update.contains("UpdateSleepTime::Sleep30")
        && g.contains("fn is_owners_cur_weapon_on_turret")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_turret_ai_dual_world_empty_gate_honesty() -> bool {
    honesty_live_turret_ai_dual_world_empty_gate_residual_pack_wave338()
        && honesty_turret_ai_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_turret_ai_dual_world_empty_gate_method_names_residual_wave338());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_turret_ai_dual_world_empty_gate_nav_commands_residual_wave338());
    }

    #[test]
    fn wave338_composite_pack() {
        assert!(honesty_live_turret_ai_dual_world_empty_gate_residual_pack_wave338());
    }

    #[test]
    fn turret_ai_dual_world_empty_gate_sources() {
        assert!(honesty_turret_ai_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_turret_ai_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_turret_ai_dual_world_empty_gate_honesty(),
            "turret ai dual-world empty gate residual must latch"
        );
    }
}
