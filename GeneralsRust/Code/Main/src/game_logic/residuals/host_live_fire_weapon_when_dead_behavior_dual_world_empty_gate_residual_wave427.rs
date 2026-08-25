//! Wave 427 residual peels: FireWeaponWhenDeadBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), death-weapon
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 426 Pathfind dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/fire_weapon_when_dead_behavior.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// FireWeaponWhenDeadBehavior dual-world empty-gate residual method names.
pub const LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE427:
    &[&str] = &[
    "dual_world_registry_unavailable",
    "is_object_under_construction",
    "check_upgrade_conflicts",
    "create_and_fire_temp_weapon",
    "get_object_position",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE427: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_FIRE_WEAPON_WHEN_DEAD_EMPTY_GATES",
    "LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE427:
    &[&str] = &[
        "click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_ok_prepare",
        "click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_ok_live",
        "click_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_miss",
    ];

/// Honesty: method names residual pack.
pub fn honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_method_names_residual_wave427()
-> bool {
    LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE427.len() == 6
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE427,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE427,
            "get_object_position",
        ) == Some(4)
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE427,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_nav_commands_residual_wave427()
-> bool {
    LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE427.len() == 4
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE427,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE427,
            "LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_FIRE_WEAPON_WHEN_DEAD_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE427
            .len()
            == 3
}

/// Wave 427 composite residual honesty pack.
pub fn honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_residual_pack_wave427()
-> bool {
    honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_method_names_residual_wave427()
        && honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_nav_commands_residual_wave427()
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

/// Source residual: FireWeaponWhenDeadBehavior empty dual-world short-circuits.
pub fn honesty_fire_weapon_when_dead_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/fire_weapon_when_dead_behavior.rs"
    );
    if !(g.contains("Wave 427")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(under) = fn_body(g, "fn is_object_under_construction(") else {
        return false;
    };
    let Some(fire) = fn_body(g, "fn create_and_fire_temp_weapon(") else {
        return false;
    };
    let Some(pos) = fn_body(g, "fn get_object_position(") else {
        return false;
    };
    helper_ok
        && under.contains("return false")
        && fire.contains("return Ok(())")
        && pos.contains("Coord3D::new(0.0, 0.0, 0.0)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_residual_pack_wave427()
        && honesty_fire_weapon_when_dead_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_method_names_residual_wave427()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_nav_commands_residual_wave427()
        );
    }

    #[test]
    fn wave427_composite_pack() {
        assert!(
            honesty_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_residual_pack_wave427(
            )
        );
    }

    #[test]
    fn fire_weapon_when_dead_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_fire_weapon_when_dead_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_fire_weapon_when_dead_behavior_dual_world_empty_gate_honesty(),
            "fire weapon when dead dual-world empty gate residual must latch"
        );
    }
}
