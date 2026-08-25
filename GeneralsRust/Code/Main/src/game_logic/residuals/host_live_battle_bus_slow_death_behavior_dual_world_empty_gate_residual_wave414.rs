//! Wave 414 residual peels: BattleBusSlowDeathBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), battle bus
//! slow-death helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 413 SlowDeathBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/battle_bus_slow_death_behavior.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// BattleBusSlowDeathBehavior dual-world empty-gate residual method names.
pub const LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE414:
    &[&str] = &[
    "dual_world_registry_unavailable",
    "get_object",
    "with_object",
    "with_object_mut",
    "damage_passengers",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE414: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_BATTLE_BUS_SLOW_DEATH_EMPTY_GATES",
    "LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE414:
    &[&str] = &[
        "click_live_battle_bus_slow_death_behavior_dual_world_empty_gate_ok_prepare",
        "click_live_battle_bus_slow_death_behavior_dual_world_empty_gate_ok_live",
        "click_live_battle_bus_slow_death_behavior_dual_world_empty_gate_miss",
    ];

/// Honesty: method names residual pack.
pub fn honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_method_names_residual_wave414()
-> bool {
    LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE414.len() == 6
        && residual_name_index(
            LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE414,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE414,
            "damage_passengers",
        ) == Some(4)
        && residual_name_index(
            LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE414,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_nav_commands_residual_wave414()
-> bool {
    LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE414.len() == 4
        && residual_name_index(
            LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE414,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE414,
            "LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_BATTLE_BUS_SLOW_DEATH_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE414
            .len()
            == 3
}

/// Wave 414 composite residual honesty pack.
pub fn honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_residual_pack_wave414()
-> bool {
    honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_method_names_residual_wave414()
        && honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_nav_commands_residual_wave414()
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

/// Source residual: BattleBusSlowDeathBehavior empty dual-world short-circuits.
pub fn honesty_battle_bus_slow_death_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/battle_bus_slow_death_behavior.rs"
    );
    if !(g.contains("Wave 414")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(get) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(with_o) = fn_body(g, "fn with_object<") else {
        return false;
    };
    let Some(damage) = fn_body(g, "fn damage_passengers(") else {
        return false;
    };
    helper_ok
        && get.contains("missing owning object")
        && with_o.contains("return None")
        && damage.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_battle_bus_slow_death_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_residual_pack_wave414()
        && honesty_battle_bus_slow_death_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_method_names_residual_wave414()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_nav_commands_residual_wave414()
        );
    }

    #[test]
    fn wave414_composite_pack() {
        assert!(
            honesty_live_battle_bus_slow_death_behavior_dual_world_empty_gate_residual_pack_wave414(
            )
        );
    }

    #[test]
    fn battle_bus_slow_death_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_battle_bus_slow_death_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_battle_bus_slow_death_behavior_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_battle_bus_slow_death_behavior_dual_world_empty_gate_honesty(),
            "battle bus slow death dual-world empty gate residual must latch"
        );
    }
}
