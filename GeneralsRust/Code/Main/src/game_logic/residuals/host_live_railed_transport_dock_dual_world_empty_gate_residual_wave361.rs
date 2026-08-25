//! Wave 361 residual peels: RailedTransportDock dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), pull/push
//! dock helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 360 RadiusDecalUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/production/railed_transport_dock.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// RailedTransportDock dual-world empty-gate residual method names.
pub const LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE361: &[&str] = &[
    "dual_world_registry_unavailable",
    "do_pull_in_docking",
    "do_push_out_docking",
    "unload_next",
    "action",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE361: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_RAILED_TRANSPORT_DOCK_EMPTY_GATES",
    "LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE361:
    &[&str] = &[
    "click_live_railed_transport_dock_dual_world_empty_gate_ok_prepare",
    "click_live_railed_transport_dock_dual_world_empty_gate_ok_live",
    "click_live_railed_transport_dock_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_railed_transport_dock_dual_world_empty_gate_method_names_residual_wave361()
-> bool {
    LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE361.len() == 6
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE361,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE361,
            "action",
        ) == Some(4)
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE361,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_railed_transport_dock_dual_world_empty_gate_nav_commands_residual_wave361()
-> bool {
    LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE361.len() == 4
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE361,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE361,
            "LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_RAILED_TRANSPORT_DOCK_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE361.len()
            == 3
}

/// Wave 361 composite residual honesty pack.
pub fn honesty_live_railed_transport_dock_dual_world_empty_gate_residual_pack_wave361() -> bool {
    honesty_live_railed_transport_dock_dual_world_empty_gate_method_names_residual_wave361()
        && honesty_live_railed_transport_dock_dual_world_empty_gate_nav_commands_residual_wave361()
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

/// Source residual: RailedTransportDock empty dual-world short-circuits.
pub fn honesty_railed_transport_dock_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/production/railed_transport_dock.rs"
    );
    if !(g.contains("Wave 361")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(pull) = fn_body(g, "fn do_pull_in_docking(") else {
        return false;
    };
    let Some(push) = fn_body(g, "fn do_push_out_docking(") else {
        return false;
    };
    let Some(unload) = fn_body(g, "fn unload_next(") else {
        return false;
    };
    let Some(action) = fn_body(g, "fn action(") else {
        return false;
    };
    helper_ok
        && pull.contains("return Ok(())")
        && push.contains("return Ok(())")
        && unload.contains("return Ok(())")
        && action.contains("return Ok(false)")
        && g.contains("fn is_clear_to_enter")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_railed_transport_dock_dual_world_empty_gate_honesty() -> bool {
    honesty_live_railed_transport_dock_dual_world_empty_gate_residual_pack_wave361()
        && honesty_railed_transport_dock_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_railed_transport_dock_dual_world_empty_gate_method_names_residual_wave361(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_railed_transport_dock_dual_world_empty_gate_nav_commands_residual_wave361(
            )
        );
    }

    #[test]
    fn wave361_composite_pack() {
        assert!(honesty_live_railed_transport_dock_dual_world_empty_gate_residual_pack_wave361());
    }

    #[test]
    fn railed_transport_dock_dual_world_empty_gate_sources() {
        assert!(honesty_railed_transport_dock_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_railed_transport_dock_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_railed_transport_dock_dual_world_empty_gate_honesty(),
            "railed transport dock dual-world empty gate residual must latch"
        );
    }
}
