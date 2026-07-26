//! Wave 315 residual peels: StructureToppleUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), topple
//! phase/FX/crush helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 314 MaxHealthUpgrade dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/structure_topple_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Structure topple dual-world empty-gate residual method names.
pub const LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE315: &[&str] = &[
    "dual_world_registry_unavailable",
    "do_phase_stuff",
    "begin_structure_topple",
    "do_topple_delay_burst_fx",
    "do_topple_done_stuff",
    "do_angle_fx",
    "apply_crushing_damage",
    "update_simple",
    "on_die",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE315: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_STRUCTURE_TOPPLE_EMPTY_GATES",
    "LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE315: &[&str] = &[
    "click_live_structure_topple_dual_world_empty_gate_ok_prepare",
    "click_live_structure_topple_dual_world_empty_gate_ok_live",
    "click_live_structure_topple_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_structure_topple_dual_world_empty_gate_method_names_residual_wave315() -> bool {
    LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE315.len() == 10
        && residual_name_index(
            LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE315,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE315,
            "update_simple",
        ) == Some(7)
        && residual_name_index(
            LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE315,
            "playable_claim = false",
        ) == Some(9)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_structure_topple_dual_world_empty_gate_nav_commands_residual_wave315() -> bool {
    LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE315.len() == 4
        && residual_name_index(
            LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE315,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE315,
            "LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_STRUCTURE_TOPPLE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE315.len() == 3
}

/// Wave 315 composite residual honesty pack.
pub fn honesty_live_structure_topple_dual_world_empty_gate_residual_pack_wave315() -> bool {
    honesty_live_structure_topple_dual_world_empty_gate_method_names_residual_wave315()
        && honesty_live_structure_topple_dual_world_empty_gate_nav_commands_residual_wave315()
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

/// Source residual: structure topple empty dual-world short-circuits.
pub fn honesty_structure_topple_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/behavior/structure_topple_update.rs"
    );
    if !(g.contains("Wave 315")
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
    let Some(on_die) = fn_body(g, "fn on_die(") else {
        return false;
    };
    helper_ok
        && update.contains("UpdateSleepTime::Forever")
        && on_die.contains("Ok(())")
        && g.contains("fn begin_structure_topple")
        && g.contains("fn apply_crushing_damage")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_structure_topple_dual_world_empty_gate_honesty() -> bool {
    honesty_live_structure_topple_dual_world_empty_gate_residual_pack_wave315()
        && honesty_structure_topple_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_structure_topple_dual_world_empty_gate_method_names_residual_wave315()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_structure_topple_dual_world_empty_gate_nav_commands_residual_wave315()
        );
    }

    #[test]
    fn wave315_composite_pack() {
        assert!(honesty_live_structure_topple_dual_world_empty_gate_residual_pack_wave315());
    }

    #[test]
    fn structure_topple_dual_world_empty_gate_sources() {
        assert!(honesty_structure_topple_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_structure_topple_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_structure_topple_dual_world_empty_gate_honesty(),
            "structure topple dual-world empty gate residual must latch"
        );
    }
}
