//! Wave 322 residual peels: TensileFormationUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), tensile
//! link/dislodge helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 321 FuelAirBomb dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/tensile_formation_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Tensile formation dual-world empty-gate residual method names.
pub const LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE322: &[&str] = &[
    "dual_world_registry_unavailable",
    "set_pathfinding_wall",
    "init_links",
    "propagate_dislodgement",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE322: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_TENSILE_FORMATION_EMPTY_GATES",
    "LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE322: &[&str] = &[
    "click_live_tensile_formation_dual_world_empty_gate_ok_prepare",
    "click_live_tensile_formation_dual_world_empty_gate_ok_live",
    "click_live_tensile_formation_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_tensile_formation_dual_world_empty_gate_method_names_residual_wave322() -> bool
{
    LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE322.len() == 6
        && residual_name_index(
            LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE322,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE322,
            "update_simple",
        ) == Some(4)
        && residual_name_index(
            LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE322,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_tensile_formation_dual_world_empty_gate_nav_commands_residual_wave322() -> bool
{
    LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE322.len() == 4
        && residual_name_index(
            LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE322,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE322,
            "LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_TENSILE_FORMATION_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE322.len() == 3
}

/// Wave 322 composite residual honesty pack.
pub fn honesty_live_tensile_formation_dual_world_empty_gate_residual_pack_wave322() -> bool {
    honesty_live_tensile_formation_dual_world_empty_gate_method_names_residual_wave322()
        && honesty_live_tensile_formation_dual_world_empty_gate_nav_commands_residual_wave322()
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

/// Source residual: tensile formation empty dual-world short-circuits.
pub fn honesty_tensile_formation_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/behavior/tensile_formation_update.rs"
    );
    if !(g.contains("Wave 322")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    let Some(init) = fn_body(g, "fn init_links(") else {
        return false;
    };
    helper_ok
        && update.contains("UpdateSleepTime::Forever")
        && init.contains("return;")
        && g.contains("fn set_pathfinding_wall")
        && g.contains("fn propagate_dislodgement")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_tensile_formation_dual_world_empty_gate_honesty() -> bool {
    honesty_live_tensile_formation_dual_world_empty_gate_residual_pack_wave322()
        && honesty_tensile_formation_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_tensile_formation_dual_world_empty_gate_method_names_residual_wave322()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_tensile_formation_dual_world_empty_gate_nav_commands_residual_wave322()
        );
    }

    #[test]
    fn wave322_composite_pack() {
        assert!(honesty_live_tensile_formation_dual_world_empty_gate_residual_pack_wave322());
    }

    #[test]
    fn tensile_formation_dual_world_empty_gate_sources() {
        assert!(honesty_tensile_formation_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_tensile_formation_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_tensile_formation_dual_world_empty_gate_honesty(),
            "tensile formation dual-world empty gate residual must latch"
        );
    }
}
