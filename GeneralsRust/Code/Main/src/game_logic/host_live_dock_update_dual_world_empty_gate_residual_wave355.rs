//! Wave 355 residual peels: DockUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), dock
//! approach/enter/exit helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 354 POWTruckAI dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/production/dock_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// DockUpdate dual-world empty-gate residual method names.
pub const LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE355: &[&str] = &[
    "dual_world_registry_unavailable",
    "load_dock_positions",
    "compute_approach_position",
    "update",
    "resolve_dock_object",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE355: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_DOCK_UPDATE_EMPTY_GATES",
    "LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE355: &[&str] = &[
    "click_live_dock_update_dual_world_empty_gate_ok_prepare",
    "click_live_dock_update_dual_world_empty_gate_ok_live",
    "click_live_dock_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_dock_update_dual_world_empty_gate_method_names_residual_wave355() -> bool {
    LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE355.len() == 6
        && residual_name_index(
            LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE355,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE355,
            "resolve_dock_object",
        ) == Some(4)
        && residual_name_index(
            LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE355,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_dock_update_dual_world_empty_gate_nav_commands_residual_wave355() -> bool {
    LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE355.len() == 4
        && residual_name_index(
            LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE355,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE355,
            "LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_DOCK_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE355.len() == 3
}

/// Wave 355 composite residual honesty pack.
pub fn honesty_live_dock_update_dual_world_empty_gate_residual_pack_wave355() -> bool {
    honesty_live_dock_update_dual_world_empty_gate_method_names_residual_wave355()
        && honesty_live_dock_update_dual_world_empty_gate_nav_commands_residual_wave355()
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

/// Source residual: DockUpdate empty dual-world short-circuits.
pub fn honesty_dock_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/production/dock_update.rs");
    if !(g.contains("Wave 355")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(load) = fn_body(g, "fn load_dock_positions(") else {
        return false;
    };
    let Some(pos) = fn_body(g, "fn compute_approach_position(") else {
        return false;
    };
    let Some(upd) = fn_body(g, "fn update(") else {
        return false;
    };
    let Some(res) = fn_body(g, "fn resolve_dock_object(") else {
        return false;
    };
    helper_ok
        && load.contains("return;")
        && pos.contains("Coord3D::ZERO")
        && upd.contains("return Ok(())")
        && res.contains("return None")
        && g.matches("Wave 355:").count() >= 10
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_dock_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_dock_update_dual_world_empty_gate_residual_pack_wave355()
        && honesty_dock_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_dock_update_dual_world_empty_gate_method_names_residual_wave355());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_dock_update_dual_world_empty_gate_nav_commands_residual_wave355());
    }

    #[test]
    fn wave355_composite_pack() {
        assert!(honesty_live_dock_update_dual_world_empty_gate_residual_pack_wave355());
    }

    #[test]
    fn dock_update_dual_world_empty_gate_sources() {
        assert!(honesty_dock_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_dock_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_dock_update_dual_world_empty_gate_honesty(),
            "dock update dual-world empty gate residual must latch"
        );
    }
}
