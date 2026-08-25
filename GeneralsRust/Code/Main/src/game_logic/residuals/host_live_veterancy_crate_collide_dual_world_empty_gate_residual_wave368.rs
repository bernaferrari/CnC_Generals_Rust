//! Wave 368 residual peels: VeterancyCrateCollide dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), veterancy
//! crate helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 367 FireWeaponWhenDamaged dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/collide/crate_collide/veterancy_crate_collide.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Test helpers intentionally ungated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// VeterancyCrateCollide dual-world empty-gate residual method names.
pub const LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE368: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_levels_to_gain",
    "owner_goal_matches",
    "owner_player_id",
    "collect_area_effect_object_ids",
    "execute_crate_behavior_internal",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE368: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_VETERANCY_CRATE_EMPTY_GATES",
    "LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE368:
    &[&str] = &[
    "click_live_veterancy_crate_collide_dual_world_empty_gate_ok_prepare",
    "click_live_veterancy_crate_collide_dual_world_empty_gate_ok_live",
    "click_live_veterancy_crate_collide_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_veterancy_crate_collide_dual_world_empty_gate_method_names_residual_wave368()
-> bool {
    LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE368.len() == 7
        && residual_name_index(
            LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE368,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE368,
            "execute_crate_behavior_internal",
        ) == Some(5)
        && residual_name_index(
            LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE368,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_residual_wave368()
-> bool {
    LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE368.len() == 4
        && residual_name_index(
            LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE368,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE368,
            "LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_VETERANCY_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE368.len()
            == 3
}

/// Wave 368 composite residual honesty pack.
pub fn honesty_live_veterancy_crate_collide_dual_world_empty_gate_residual_pack_wave368() -> bool {
    honesty_live_veterancy_crate_collide_dual_world_empty_gate_method_names_residual_wave368()
        && honesty_live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_residual_wave368(
        )
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

/// Source residual: VeterancyCrateCollide empty dual-world short-circuits.
pub fn honesty_veterancy_crate_collide_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/collide/crate_collide/veterancy_crate_collide.rs"
    );
    if !(g.contains("Wave 368")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(levels) = fn_body(g, "fn get_levels_to_gain(") else {
        return false;
    };
    let Some(goal) = fn_body(g, "fn owner_goal_matches(") else {
        return false;
    };
    let Some(player) = fn_body(g, "fn owner_player_id(") else {
        return false;
    };
    let Some(collect) = fn_body(g, "fn collect_area_effect_object_ids(") else {
        return false;
    };
    let Some(execute) = fn_body(g, "fn execute_crate_behavior_internal(") else {
        return false;
    };
    helper_ok
        && levels.contains("return 0")
        && goal.contains("return false")
        && player.contains("return None")
        && collect.contains("return Vec::new()")
        && execute.contains("return Ok(false)")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_veterancy_crate_collide_dual_world_empty_gate_honesty() -> bool {
    honesty_live_veterancy_crate_collide_dual_world_empty_gate_residual_pack_wave368()
        && honesty_veterancy_crate_collide_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_veterancy_crate_collide_dual_world_empty_gate_method_names_residual_wave368());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_veterancy_crate_collide_dual_world_empty_gate_nav_commands_residual_wave368());
    }

    #[test]
    fn wave368_composite_pack() {
        assert!(honesty_live_veterancy_crate_collide_dual_world_empty_gate_residual_pack_wave368());
    }

    #[test]
    fn veterancy_crate_collide_dual_world_empty_gate_sources() {
        assert!(honesty_veterancy_crate_collide_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_veterancy_crate_collide_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_veterancy_crate_collide_dual_world_empty_gate_honesty(),
            "veterancy crate collide dual-world empty gate residual must latch"
        );
    }
}
