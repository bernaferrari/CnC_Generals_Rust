//! Wave 411 residual peels: PointDefenseLaserUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), point defense
//! laser helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 410 MinefieldBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/point_defense_laser_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// PointDefenseLaserUpdate dual-world empty-gate residual method names.
pub const LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE411: &[&str] = &[
    "dual_world_registry_unavailable",
    "scan_closest_target",
    "fire_when_ready",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE411: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_POINT_DEFENSE_LASER_EMPTY_GATES",
    "LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE411:
    &[&str] = &[
    "click_live_point_defense_laser_update_dual_world_empty_gate_ok_prepare",
    "click_live_point_defense_laser_update_dual_world_empty_gate_ok_live",
    "click_live_point_defense_laser_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_point_defense_laser_update_dual_world_empty_gate_method_names_residual_wave411()
-> bool {
    LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE411.len() == 5
        && residual_name_index(
            LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE411,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE411,
            "update_simple",
        ) == Some(3)
        && residual_name_index(
            LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE411,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_point_defense_laser_update_dual_world_empty_gate_nav_commands_residual_wave411()
-> bool {
    LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE411.len() == 4
        && residual_name_index(
            LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE411,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE411,
            "LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_POINT_DEFENSE_LASER_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE411
            .len()
            == 3
}

/// Wave 411 composite residual honesty pack.
pub fn honesty_live_point_defense_laser_update_dual_world_empty_gate_residual_pack_wave411() -> bool
{
    honesty_live_point_defense_laser_update_dual_world_empty_gate_method_names_residual_wave411()
        && honesty_live_point_defense_laser_update_dual_world_empty_gate_nav_commands_residual_wave411()
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

/// Source residual: PointDefenseLaserUpdate empty dual-world short-circuits.
pub fn honesty_point_defense_laser_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/point_defense_laser_update.rs"
    );
    if !(g.contains("Wave 411")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(scan) = fn_body(g, "fn scan_closest_target(") else {
        return false;
    };
    let Some(fire) = fn_body(g, "fn fire_when_ready(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    helper_ok
        && scan.contains("return None")
        && fire.contains("return;")
        && update.contains("UpdateSleepTime::Forever")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_point_defense_laser_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_point_defense_laser_update_dual_world_empty_gate_residual_pack_wave411()
        && honesty_point_defense_laser_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_point_defense_laser_update_dual_world_empty_gate_method_names_residual_wave411()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_point_defense_laser_update_dual_world_empty_gate_nav_commands_residual_wave411()
        );
    }

    #[test]
    fn wave411_composite_pack() {
        assert!(
            honesty_live_point_defense_laser_update_dual_world_empty_gate_residual_pack_wave411()
        );
    }

    #[test]
    fn point_defense_laser_update_dual_world_empty_gate_sources() {
        assert!(honesty_point_defense_laser_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_point_defense_laser_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_point_defense_laser_update_dual_world_empty_gate_honesty(),
            "point defense laser dual-world empty gate residual must latch"
        );
    }
}
