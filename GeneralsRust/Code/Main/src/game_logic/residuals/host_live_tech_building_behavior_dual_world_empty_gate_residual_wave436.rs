//! Wave 436 residual peels: TechBuildingBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), tech building
//! object helpers fail-closed without dual-world factory walks.
//! `get_object` stays ungated (TheGameLogic fallback).
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 435 OverchargeBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/tech_building_behavior.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// TechBuildingBehavior dual-world empty-gate residual method names.
pub const LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE436: &[&str] = &[
    "dual_world_registry_unavailable",
    "with_object",
    "with_object_mut",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE436: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_TECH_BUILDING_BEHAVIOR_EMPTY_GATES",
    "LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE436:
    &[&str] = &[
    "click_live_tech_building_behavior_dual_world_empty_gate_ok_prepare",
    "click_live_tech_building_behavior_dual_world_empty_gate_ok_live",
    "click_live_tech_building_behavior_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_tech_building_behavior_dual_world_empty_gate_method_names_residual_wave436()
-> bool {
    LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE436.len() == 4
        && residual_name_index(
            LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE436,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE436,
            "with_object_mut",
        ) == Some(2)
        && residual_name_index(
            LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE436,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_tech_building_behavior_dual_world_empty_gate_nav_commands_residual_wave436()
-> bool {
    LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE436.len() == 4
        && residual_name_index(
            LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE436,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE436,
            "LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_TECH_BUILDING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE436.len()
            == 3
}

/// Wave 436 composite residual honesty pack.
pub fn honesty_live_tech_building_behavior_dual_world_empty_gate_residual_pack_wave436() -> bool {
    honesty_live_tech_building_behavior_dual_world_empty_gate_method_names_residual_wave436()
        && honesty_live_tech_building_behavior_dual_world_empty_gate_nav_commands_residual_wave436()
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

/// Source residual: TechBuildingBehavior empty dual-world short-circuits.
pub fn honesty_tech_building_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/tech_building_behavior.rs"
    );
    if !(g.contains("Wave 436")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(with_o) = fn_body(g, "fn with_object<") else {
        return false;
    };
    let Some(with_m) = fn_body(g, "fn with_object_mut<") else {
        return false;
    };
    let get_start = g.find("fn get_object(").unwrap_or(0);
    let get_slice = g.get(get_start..get_start + 280).unwrap_or("");
    helper_ok
        && with_o.contains("Object not found")
        && with_m.contains("Object not found")
        && !get_slice.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_tech_building_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_tech_building_behavior_dual_world_empty_gate_residual_pack_wave436()
        && honesty_tech_building_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_tech_building_behavior_dual_world_empty_gate_method_names_residual_wave436(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_tech_building_behavior_dual_world_empty_gate_nav_commands_residual_wave436(
            )
        );
    }

    #[test]
    fn wave436_composite_pack() {
        assert!(honesty_live_tech_building_behavior_dual_world_empty_gate_residual_pack_wave436());
    }

    #[test]
    fn tech_building_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_tech_building_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_tech_building_behavior_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_tech_building_behavior_dual_world_empty_gate_honesty(),
            "tech building behavior dual-world empty gate residual must latch"
        );
    }
}
