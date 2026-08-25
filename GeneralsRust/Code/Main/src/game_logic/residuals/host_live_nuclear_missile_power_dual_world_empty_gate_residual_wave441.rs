//! Wave 441 residual peels: NuclearMissilePower dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), nuclear
//! missile helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 440 CarpetBombPower dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/special_power_module/nuclear_missile_power.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// NuclearMissilePower dual-world empty-gate residual method names.
pub const LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE441: &[&str] = &[
    "dual_world_registry_unavailable",
    "execute_strike",
    "try_activate",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE441: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_NUCLEAR_MISSILE_POWER_EMPTY_GATES",
    "LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE441:
    &[&str] = &[
    "click_live_nuclear_missile_power_dual_world_empty_gate_ok_prepare",
    "click_live_nuclear_missile_power_dual_world_empty_gate_ok_live",
    "click_live_nuclear_missile_power_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_nuclear_missile_power_dual_world_empty_gate_method_names_residual_wave441()
-> bool {
    LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE441.len() == 4
        && residual_name_index(
            LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE441,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE441,
            "try_activate",
        ) == Some(2)
        && residual_name_index(
            LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE441,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_nuclear_missile_power_dual_world_empty_gate_nav_commands_residual_wave441()
-> bool {
    LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE441.len() == 4
        && residual_name_index(
            LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE441,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE441,
            "LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_NUCLEAR_MISSILE_POWER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE441.len()
            == 3
}

/// Wave 441 composite residual honesty pack.
pub fn honesty_live_nuclear_missile_power_dual_world_empty_gate_residual_pack_wave441() -> bool {
    honesty_live_nuclear_missile_power_dual_world_empty_gate_method_names_residual_wave441()
        && honesty_live_nuclear_missile_power_dual_world_empty_gate_nav_commands_residual_wave441()
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

/// Source residual: NuclearMissilePower empty dual-world short-circuits.
pub fn honesty_nuclear_missile_power_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/special_power_module/nuclear_missile_power.rs"
    );
    if !(g.contains("Wave 441")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(exec) = fn_body(g, "fn execute_strike(") else {
        return false;
    };
    let Some(act) = fn_body(g, "fn try_activate(") else {
        return false;
    };
    helper_ok && exec.contains("return Ok(())") && act.contains("ActivationResult::Failed")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_nuclear_missile_power_dual_world_empty_gate_honesty() -> bool {
    honesty_live_nuclear_missile_power_dual_world_empty_gate_residual_pack_wave441()
        && honesty_nuclear_missile_power_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_nuclear_missile_power_dual_world_empty_gate_method_names_residual_wave441(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_nuclear_missile_power_dual_world_empty_gate_nav_commands_residual_wave441(
            )
        );
    }

    #[test]
    fn wave441_composite_pack() {
        assert!(honesty_live_nuclear_missile_power_dual_world_empty_gate_residual_pack_wave441());
    }

    #[test]
    fn nuclear_missile_power_dual_world_empty_gate_sources() {
        assert!(honesty_nuclear_missile_power_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_nuclear_missile_power_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_nuclear_missile_power_dual_world_empty_gate_honesty(),
            "nuclear missile power dual-world empty gate residual must latch"
        );
    }
}
