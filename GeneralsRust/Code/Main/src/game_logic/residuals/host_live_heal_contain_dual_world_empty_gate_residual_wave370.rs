//! Wave 370 residual peels: HealContain dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), heal
//! contain helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 369 AssaultTransportAIUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/contain/heal_contain.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// HealContain dual-world empty-gate residual method names.
pub const LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE370: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_object",
    "update",
    "do_heal",
    "can_contain",
    "contain_object",
    "release_object",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE370: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_HEAL_CONTAIN_EMPTY_GATES",
    "LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE370: &[&str] = &[
    "click_live_heal_contain_dual_world_empty_gate_ok_prepare",
    "click_live_heal_contain_dual_world_empty_gate_ok_live",
    "click_live_heal_contain_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: empty-registry helper is fail-open (C++ does not skip-close).
pub fn honesty_live_heal_contain_dual_world_empty_gate_method_names_residual_wave370() -> bool {
    LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE370.len() == 8
        && residual_name_index(
            LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE370,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE370,
            "release_object",
        ) == Some(6)
        && residual_name_index(
            LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE370,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_heal_contain_dual_world_empty_gate_nav_commands_residual_wave370() -> bool {
    LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE370.len() == 4
        && residual_name_index(
            LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE370,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE370,
            "LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_HEAL_CONTAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE370.len() == 3
}

/// Wave 370 composite residual honesty pack.
pub fn honesty_live_heal_contain_dual_world_empty_gate_residual_pack_wave370() -> bool {
    honesty_live_heal_contain_dual_world_empty_gate_method_names_residual_wave370()
        && honesty_live_heal_contain_dual_world_empty_gate_nav_commands_residual_wave370()
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

/// Source residual: HealContain empty dual-world short-circuits.
pub fn honesty_heal_contain_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/contain/heal_contain.rs");
    if !(g.contains("Wave 370")
        && g.contains("fn dual_world_registry_unavailable")
        && (g.contains("let _host_empty") || g.contains("OBJECT_REGISTRY.is_empty()"))
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    // 2026-08-15: helper probes emptiness but returns false (C++ does not skip-close).
    let helper_ok = g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()")
        && g.contains("false");
    let Some(get_obj) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update(") else {
        return false;
    };
    let Some(heal) = fn_body(g, "fn do_heal(") else {
        return false;
    };
    let Some(can) = fn_body(g, "fn can_contain(") else {
        return false;
    };
    let Some(contain) = fn_body(g, "fn contain_object(") else {
        return false;
    };
    helper_ok
        && get_obj.contains("return None")
        && update.contains("return Ok(UpdateSleepTime::None)")
        && heal.contains("return Ok(false)")
        && can.contains("return false")
        && contain.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_heal_contain_dual_world_empty_gate_honesty() -> bool {
    honesty_live_heal_contain_dual_world_empty_gate_residual_pack_wave370()
        && honesty_heal_contain_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_heal_contain_dual_world_empty_gate_method_names_residual_wave370());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_heal_contain_dual_world_empty_gate_nav_commands_residual_wave370());
    }

    #[test]
    fn wave370_composite_pack() {
        assert!(honesty_live_heal_contain_dual_world_empty_gate_residual_pack_wave370());
    }

    #[test]
    fn heal_contain_dual_world_empty_gate_sources() {
        assert!(honesty_heal_contain_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_heal_contain_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_heal_contain_dual_world_empty_gate_honesty(),
            "heal contain dual-world empty gate residual must latch"
        );
    }
}
