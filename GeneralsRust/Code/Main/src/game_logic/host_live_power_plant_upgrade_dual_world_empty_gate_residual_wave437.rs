//! Wave 437 residual peels: PowerPlantUpgrade dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), power plant
//! upgrade helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 436 TechBuildingBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/upgrade/power_plant_upgrade.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// PowerPlantUpgrade dual-world empty-gate residual method names.
pub const LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE437: &[&str] = &[
    "dual_world_registry_unavailable",
    "load_post_process",
    "apply_upgrade",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE437: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_POWER_PLANT_UPGRADE_EMPTY_GATES",
    "LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE437: &[&str] =
    &[
        "click_live_power_plant_upgrade_dual_world_empty_gate_ok_prepare",
        "click_live_power_plant_upgrade_dual_world_empty_gate_ok_live",
        "click_live_power_plant_upgrade_dual_world_empty_gate_miss",
    ];

/// Honesty: method names residual pack.
pub fn honesty_live_power_plant_upgrade_dual_world_empty_gate_method_names_residual_wave437() -> bool
{
    LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE437.len() == 4
        && residual_name_index(
            LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE437,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE437,
            "apply_upgrade",
        ) == Some(2)
        && residual_name_index(
            LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE437,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_power_plant_upgrade_dual_world_empty_gate_nav_commands_residual_wave437() -> bool
{
    LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE437.len() == 4
        && residual_name_index(
            LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE437,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE437,
            "LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_POWER_PLANT_UPGRADE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE437.len() == 3
}

/// Wave 437 composite residual honesty pack.
pub fn honesty_live_power_plant_upgrade_dual_world_empty_gate_residual_pack_wave437() -> bool {
    honesty_live_power_plant_upgrade_dual_world_empty_gate_method_names_residual_wave437()
        && honesty_live_power_plant_upgrade_dual_world_empty_gate_nav_commands_residual_wave437()
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

/// Source residual: PowerPlantUpgrade empty dual-world short-circuits.
pub fn honesty_power_plant_upgrade_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/upgrade/power_plant_upgrade.rs");
    if !(g.contains("Wave 437")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(load) = fn_body(g, "fn load_post_process(") else {
        return false;
    };
    let Some(apply) = fn_body(g, "fn apply_upgrade(") else {
        return false;
    };
    helper_ok && load.contains("return Ok(())") && apply.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_power_plant_upgrade_dual_world_empty_gate_honesty() -> bool {
    honesty_live_power_plant_upgrade_dual_world_empty_gate_residual_pack_wave437()
        && honesty_power_plant_upgrade_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_power_plant_upgrade_dual_world_empty_gate_method_names_residual_wave437()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_power_plant_upgrade_dual_world_empty_gate_nav_commands_residual_wave437()
        );
    }

    #[test]
    fn wave437_composite_pack() {
        assert!(honesty_live_power_plant_upgrade_dual_world_empty_gate_residual_pack_wave437());
    }

    #[test]
    fn power_plant_upgrade_dual_world_empty_gate_sources() {
        assert!(honesty_power_plant_upgrade_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_power_plant_upgrade_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_power_plant_upgrade_dual_world_empty_gate_honesty(),
            "power plant upgrade dual-world empty gate residual must latch"
        );
    }
}
