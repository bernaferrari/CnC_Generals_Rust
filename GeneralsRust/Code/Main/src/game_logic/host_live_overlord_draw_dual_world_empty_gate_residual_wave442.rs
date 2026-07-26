//! Wave 442 residual peels: Overlord draw modules dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), overlord
//! truck/tank/aircraft rider draw helpers fail-closed without dual-world walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 441 NuclearMissilePower dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/draw/w3d_overlord_truck_draw.rs`
//! - `GameLogic/src/object/draw/w3d_overlord_tank_draw.rs`
//! - `GameLogic/src/object/draw/w3d_overlord_aircraft_draw.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Base draw/set_hidden still runs before the empty-registry gate

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Overlord draw dual-world empty-gate residual method names.
pub const LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE442: &[&str] = &[
    "dual_world_registry_unavailable",
    "do_draw_module",
    "set_hidden",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE442: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_OVERLORD_DRAW_EMPTY_GATES",
    "LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE442: &[&str] = &[
    "click_live_overlord_draw_dual_world_empty_gate_ok_prepare",
    "click_live_overlord_draw_dual_world_empty_gate_ok_live",
    "click_live_overlord_draw_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_overlord_draw_dual_world_empty_gate_method_names_residual_wave442() -> bool {
    LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE442.len() == 4
        && residual_name_index(
            LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE442,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE442,
            "set_hidden",
        ) == Some(2)
        && residual_name_index(
            LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE442,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_overlord_draw_dual_world_empty_gate_nav_commands_residual_wave442() -> bool {
    LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE442.len() == 4
        && residual_name_index(
            LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE442,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE442,
            "LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_OVERLORD_DRAW_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE442.len() == 3
}

/// Wave 442 composite residual honesty pack.
pub fn honesty_live_overlord_draw_dual_world_empty_gate_residual_pack_wave442() -> bool {
    honesty_live_overlord_draw_dual_world_empty_gate_method_names_residual_wave442()
        && honesty_live_overlord_draw_dual_world_empty_gate_nav_commands_residual_wave442()
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

fn honesty_one_overlord_draw_source(path_src: &str) -> bool {
    if !(path_src.contains("Wave 442")
        && path_src.contains("fn dual_world_registry_unavailable")
        && path_src.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = path_src.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(draw) = fn_body(path_src, "fn do_draw_module(") else {
        return false;
    };
    let Some(hide) = fn_body(path_src, "fn set_hidden(") else {
        return false;
    };
    // Base draw/hide must still run before empty-registry short-circuit.
    let draw_base_first = match (
        draw.find("self.base.do_draw_module"),
        draw.find("dual_world_registry_unavailable"),
    ) {
        (Some(base), Some(gate)) => base < gate,
        _ => false,
    };
    let hide_base_pos = hide
        .find("self.base.set_hidden")
        .or_else(|| hide.find("DrawModule::set_hidden"));
    let hide_base_first = match (hide_base_pos, hide.find("dual_world_registry_unavailable")) {
        (Some(base), Some(gate)) => base < gate,
        _ => false,
    };
    helper_ok
        && draw.contains("return;")
        && hide.contains("return;")
        && draw_base_first
        && hide_base_first
}

/// Source residual: overlord draw empty dual-world short-circuits.
pub fn honesty_overlord_draw_dual_world_empty_gate_source() -> bool {
    let truck =
        include_str!("../../../GameEngine/GameLogic/src/object/draw/w3d_overlord_truck_draw.rs");
    let tank =
        include_str!("../../../GameEngine/GameLogic/src/object/draw/w3d_overlord_tank_draw.rs");
    let aircraft =
        include_str!("../../../GameEngine/GameLogic/src/object/draw/w3d_overlord_aircraft_draw.rs");
    honesty_one_overlord_draw_source(truck)
        && honesty_one_overlord_draw_source(tank)
        && honesty_one_overlord_draw_source(aircraft)
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_overlord_draw_dual_world_empty_gate_honesty() -> bool {
    honesty_live_overlord_draw_dual_world_empty_gate_residual_pack_wave442()
        && honesty_overlord_draw_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_overlord_draw_dual_world_empty_gate_method_names_residual_wave442());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_overlord_draw_dual_world_empty_gate_nav_commands_residual_wave442());
    }

    #[test]
    fn wave442_composite_pack() {
        assert!(honesty_live_overlord_draw_dual_world_empty_gate_residual_pack_wave442());
    }

    #[test]
    fn overlord_draw_dual_world_empty_gate_sources() {
        assert!(honesty_overlord_draw_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_overlord_draw_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_overlord_draw_dual_world_empty_gate_honesty(),
            "overlord draw dual-world empty gate residual must latch"
        );
    }
}
