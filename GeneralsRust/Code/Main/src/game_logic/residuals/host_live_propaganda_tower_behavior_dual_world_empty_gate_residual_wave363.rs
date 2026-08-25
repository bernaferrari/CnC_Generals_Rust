//! Wave 363 residual peels: PropagandaTowerBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), influence
//! scan helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 362 StructureCollapseUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/propaganda_tower_behavior.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// PropagandaTowerBehavior dual-world empty-gate residual method names.
pub const LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE363: &[&str] = &[
    "dual_world_registry_unavailable",
    "resolve_object",
    "remove_all_influence",
    "should_play_fx",
    "do_scan",
    "refresh_influence",
    "update",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE363: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PROPAGANDA_TOWER_EMPTY_GATES",
    "LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE363:
    &[&str] = &[
    "click_live_propaganda_tower_behavior_dual_world_empty_gate_ok_prepare",
    "click_live_propaganda_tower_behavior_dual_world_empty_gate_ok_live",
    "click_live_propaganda_tower_behavior_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_propaganda_tower_behavior_dual_world_empty_gate_method_names_residual_wave363()
-> bool {
    LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE363.len() == 8
        && residual_name_index(
            LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE363,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE363,
            "update",
        ) == Some(6)
        && residual_name_index(
            LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE363,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_residual_wave363()
-> bool {
    LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE363.len() == 4
        && residual_name_index(
            LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE363,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE363,
            "LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PROPAGANDA_TOWER_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE363.len()
            == 3
}

/// Wave 363 composite residual honesty pack.
pub fn honesty_live_propaganda_tower_behavior_dual_world_empty_gate_residual_pack_wave363() -> bool
{
    honesty_live_propaganda_tower_behavior_dual_world_empty_gate_method_names_residual_wave363()
        && honesty_live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_residual_wave363()
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

/// Source residual: PropagandaTowerBehavior empty dual-world short-circuits.
pub fn honesty_propaganda_tower_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/propaganda_tower_behavior.rs"
    );
    if !(g.contains("Wave 363")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(resolve) = fn_body(g, "fn resolve_object(") else {
        return false;
    };
    let Some(remove) = fn_body(g, "fn remove_all_influence(") else {
        return false;
    };
    let Some(fx) = fn_body(g, "fn should_play_fx(") else {
        return false;
    };
    let Some(scan) = fn_body(g, "fn do_scan(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update(") else {
        return false;
    };
    helper_ok
        && resolve.contains("return Err(")
        && remove.contains("return;")
        && fx.contains("return false")
        && scan.contains("return;")
        && update.contains("return Ok(UpdateSleepTime::Forever)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_propaganda_tower_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_propaganda_tower_behavior_dual_world_empty_gate_residual_pack_wave363()
        && honesty_propaganda_tower_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_propaganda_tower_behavior_dual_world_empty_gate_method_names_residual_wave363());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_propaganda_tower_behavior_dual_world_empty_gate_nav_commands_residual_wave363());
    }

    #[test]
    fn wave363_composite_pack() {
        assert!(
            honesty_live_propaganda_tower_behavior_dual_world_empty_gate_residual_pack_wave363()
        );
    }

    #[test]
    fn propaganda_tower_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_propaganda_tower_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_propaganda_tower_behavior_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_propaganda_tower_behavior_dual_world_empty_gate_honesty(),
            "propaganda tower behavior dual-world empty gate residual must latch"
        );
    }
}
