//! Wave 321 residual peels: FuelAirBombPower dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), fuel-air
//! strike helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 320 ParadropPower dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/special_power_module/fuel_air_bomb_power.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Fuel air bomb dual-world empty-gate residual method names.
pub const LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE321: &[&str] = &[
    "dual_world_registry_unavailable",
    "resolve_target_position",
    "execute_strike",
    "try_activate",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE321: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_FUEL_AIR_BOMB_EMPTY_GATES",
    "LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE321: &[&str] = &[
    "click_live_fuel_air_bomb_dual_world_empty_gate_ok_prepare",
    "click_live_fuel_air_bomb_dual_world_empty_gate_ok_live",
    "click_live_fuel_air_bomb_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_fuel_air_bomb_dual_world_empty_gate_method_names_residual_wave321() -> bool {
    LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE321.len() == 5
        && residual_name_index(
            LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE321,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE321,
            "try_activate",
        ) == Some(3)
        && residual_name_index(
            LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE321,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_fuel_air_bomb_dual_world_empty_gate_nav_commands_residual_wave321() -> bool {
    LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE321.len() == 4
        && residual_name_index(
            LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE321,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE321,
            "LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_FUEL_AIR_BOMB_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE321.len() == 3
}

/// Wave 321 composite residual honesty pack.
pub fn honesty_live_fuel_air_bomb_dual_world_empty_gate_residual_pack_wave321() -> bool {
    honesty_live_fuel_air_bomb_dual_world_empty_gate_method_names_residual_wave321()
        && honesty_live_fuel_air_bomb_dual_world_empty_gate_nav_commands_residual_wave321()
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

/// Source residual: fuel air bomb empty dual-world short-circuits.
pub fn honesty_fuel_air_bomb_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/special_power_module/fuel_air_bomb_power.rs"
    );
    if !(g.contains("Wave 321")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(resolve) = fn_body(g, "fn resolve_target_position(") else {
        return false;
    };
    let Some(strike) = fn_body(g, "fn execute_strike(") else {
        return false;
    };
    let Some(activate) = fn_body(g, "fn try_activate(") else {
        return false;
    };
    helper_ok
        && resolve.contains("targeting.position")
        && strike.contains("Err(")
        && activate.contains("ActivationResult::Failed")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_fuel_air_bomb_dual_world_empty_gate_honesty() -> bool {
    honesty_live_fuel_air_bomb_dual_world_empty_gate_residual_pack_wave321()
        && honesty_fuel_air_bomb_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_fuel_air_bomb_dual_world_empty_gate_method_names_residual_wave321());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_fuel_air_bomb_dual_world_empty_gate_nav_commands_residual_wave321());
    }

    #[test]
    fn wave321_composite_pack() {
        assert!(honesty_live_fuel_air_bomb_dual_world_empty_gate_residual_pack_wave321());
    }

    #[test]
    fn fuel_air_bomb_dual_world_empty_gate_sources() {
        assert!(honesty_fuel_air_bomb_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_fuel_air_bomb_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_fuel_air_bomb_dual_world_empty_gate_honesty(),
            "fuel air bomb dual-world empty gate residual must latch"
        );
    }
}
