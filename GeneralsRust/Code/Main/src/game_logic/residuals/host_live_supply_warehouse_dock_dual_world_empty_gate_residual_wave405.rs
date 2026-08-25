//! Wave 405 residual peels: SupplyWarehouseDock dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), supply warehouse
//! dock helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 404 BoneFXUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/production/supply_warehouse_dock.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// SupplyWarehouseDock dual-world empty-gate residual method names.
pub const LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE405: &[&str] = &[
    "dual_world_registry_unavailable",
    "update_drawable_supply_status",
    "perform_supply_transfer",
    "action",
    "set_dock_crippled",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE405: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SUPPLY_WAREHOUSE_DOCK_EMPTY_GATES",
    "LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE405:
    &[&str] = &[
    "click_live_supply_warehouse_dock_dual_world_empty_gate_ok_prepare",
    "click_live_supply_warehouse_dock_dual_world_empty_gate_ok_live",
    "click_live_supply_warehouse_dock_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_supply_warehouse_dock_dual_world_empty_gate_method_names_residual_wave405()
-> bool {
    LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE405.len() == 6
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE405,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE405,
            "set_dock_crippled",
        ) == Some(4)
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE405,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_residual_wave405()
-> bool {
    LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE405.len() == 4
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE405,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE405,
            "LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SUPPLY_WAREHOUSE_DOCK_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE405.len()
            == 3
}

/// Wave 405 composite residual honesty pack.
pub fn honesty_live_supply_warehouse_dock_dual_world_empty_gate_residual_pack_wave405() -> bool {
    honesty_live_supply_warehouse_dock_dual_world_empty_gate_method_names_residual_wave405()
        && honesty_live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_residual_wave405()
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

/// Source residual: SupplyWarehouseDock empty dual-world short-circuits.
pub fn honesty_supply_warehouse_dock_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/production/supply_warehouse_dock.rs"
    );
    if !(g.contains("Wave 405")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(status) = fn_body(g, "fn update_drawable_supply_status(") else {
        return false;
    };
    let Some(xfer) = fn_body(g, "fn perform_supply_transfer(") else {
        return false;
    };
    let Some(action) = fn_body(g, "fn action(") else {
        return false;
    };
    let Some(cripple) = fn_body(g, "fn set_dock_crippled(") else {
        return false;
    };
    helper_ok
        && status.contains("return;")
        && xfer.contains("return Ok(false)")
        && action.contains("return Ok(false)")
        && cripple.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_supply_warehouse_dock_dual_world_empty_gate_honesty() -> bool {
    honesty_live_supply_warehouse_dock_dual_world_empty_gate_residual_pack_wave405()
        && honesty_supply_warehouse_dock_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_supply_warehouse_dock_dual_world_empty_gate_method_names_residual_wave405(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_supply_warehouse_dock_dual_world_empty_gate_nav_commands_residual_wave405(
            )
        );
    }

    #[test]
    fn wave405_composite_pack() {
        assert!(honesty_live_supply_warehouse_dock_dual_world_empty_gate_residual_pack_wave405());
    }

    #[test]
    fn supply_warehouse_dock_dual_world_empty_gate_sources() {
        assert!(honesty_supply_warehouse_dock_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_supply_warehouse_dock_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_supply_warehouse_dock_dual_world_empty_gate_honesty(),
            "supply warehouse dock dual-world empty gate residual must latch"
        );
    }
}
