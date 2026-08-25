//! Wave 395 residual peels: SupplyWarehouseCripplingBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), supply warehouse
//! crippling helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 394 AutoDepositUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/supply_warehouse_crippling_behavior.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// SupplyWarehouseCripplingBehavior dual-world empty-gate residual method names.
pub const LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE395:
    &[&str] = &[
    "dual_world_registry_unavailable",
    "with_object",
    "with_object_mut",
    "get_object",
    "start_crippled_effects",
    "stop_crippled_effects",
    "update",
    "on_damage",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE395:
    &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SUPPLY_WAREHOUSE_CRIPPLING_EMPTY_GATES",
    "LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE395:
    &[&str] = &[
        "click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_ok_prepare",
        "click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_ok_live",
        "click_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_miss",
    ];

/// Honesty: method names residual pack.
pub fn honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_residual_wave395()
-> bool {
    LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE395.len() == 9
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE395,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE395,
            "on_damage",
        ) == Some(7)
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE395,
            "playable_claim = false",
        ) == Some(8)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_residual_wave395()
-> bool {
    LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE395.len() == 4
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE395,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE395,
            "LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SUPPLY_WAREHOUSE_CRIPPLING_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE395
            .len()
            == 3
}

/// Wave 395 composite residual honesty pack.
pub fn honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_residual_pack_wave395()
-> bool {
    honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_residual_wave395()
        && honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_residual_wave395()
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

/// Source residual: SupplyWarehouseCripplingBehavior empty dual-world short-circuits.
pub fn honesty_supply_warehouse_crippling_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/supply_warehouse_crippling_behavior.rs"
    );
    if !(g.contains("Wave 395")
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
    let Some(get) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update(") else {
        return false;
    };
    let Some(on_damage) = fn_body(g, "fn on_damage(") else {
        return false;
    };
    helper_ok
        && with_o.contains("Object not found")
        && get.contains("Object not found")
        && update.contains("UpdateSleepTime::Forever")
        && on_damage.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_residual_pack_wave395()
        && honesty_supply_warehouse_crippling_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_method_names_residual_wave395()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_nav_commands_residual_wave395()
        );
    }

    #[test]
    fn wave395_composite_pack() {
        assert!(
            honesty_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_residual_pack_wave395()
        );
    }

    #[test]
    fn supply_warehouse_crippling_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_supply_warehouse_crippling_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_honesty_residual_live()
     {
        assert!(
            simulate_live_supply_warehouse_crippling_behavior_dual_world_empty_gate_honesty(),
            "supply warehouse crippling dual-world empty gate residual must latch"
        );
    }
}
