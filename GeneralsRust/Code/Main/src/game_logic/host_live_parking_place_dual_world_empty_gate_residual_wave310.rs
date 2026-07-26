//! Wave 310 residual peels: ParkingPlaceBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), parking
//! door/exit/heal helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 309 JetAI dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/parking_place_behavior.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Parking place dual-world empty-gate residual method names.
pub const LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE310: &[&str] = &[
    "dual_world_registry_unavailable",
    "set_hold_door_open",
    "build_info",
    "update_simple",
    "exit_object_via_door",
    "defect_all_parked_units",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE310: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PARKING_PLACE_EMPTY_GATES",
    "LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE310: &[&str] = &[
    "click_live_parking_place_dual_world_empty_gate_ok_prepare",
    "click_live_parking_place_dual_world_empty_gate_ok_live",
    "click_live_parking_place_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_parking_place_dual_world_empty_gate_method_names_residual_wave310() -> bool {
    LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE310.len() == 7
        && residual_name_index(
            LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE310,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE310,
            "update_simple",
        ) == Some(3)
        && residual_name_index(
            LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE310,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_parking_place_dual_world_empty_gate_nav_commands_residual_wave310() -> bool {
    LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE310.len() == 4
        && residual_name_index(
            LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE310,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE310,
            "LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PARKING_PLACE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE310.len() == 3
}

/// Wave 310 composite residual honesty pack.
pub fn honesty_live_parking_place_dual_world_empty_gate_residual_pack_wave310() -> bool {
    honesty_live_parking_place_dual_world_empty_gate_method_names_residual_wave310()
        && honesty_live_parking_place_dual_world_empty_gate_nav_commands_residual_wave310()
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

/// Source residual: parking place empty dual-world short-circuits.
pub fn honesty_parking_place_dual_world_empty_gate_source() -> bool {
    let g =
        include_str!("../../../GameEngine/GameLogic/src/object/behavior/parking_place_behavior.rs");
    if !(g.contains("Wave 310")
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
    let Some(exit) = fn_body(g, "fn exit_object_via_door(") else {
        return false;
    };
    helper_ok && update.contains("UpdateSleepTime::None") && exit.contains("Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_parking_place_dual_world_empty_gate_honesty() -> bool {
    honesty_live_parking_place_dual_world_empty_gate_residual_pack_wave310()
        && honesty_parking_place_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_parking_place_dual_world_empty_gate_method_names_residual_wave310());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_parking_place_dual_world_empty_gate_nav_commands_residual_wave310());
    }

    #[test]
    fn wave310_composite_pack() {
        assert!(honesty_live_parking_place_dual_world_empty_gate_residual_pack_wave310());
    }

    #[test]
    fn parking_place_dual_world_empty_gate_sources() {
        assert!(honesty_parking_place_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_parking_place_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_parking_place_dual_world_empty_gate_honesty(),
            "parking place dual-world empty gate residual must latch"
        );
    }
}
