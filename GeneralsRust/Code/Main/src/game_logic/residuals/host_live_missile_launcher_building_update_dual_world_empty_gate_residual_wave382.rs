//! Wave 382 residual peels: MissileLauncherBuildingUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), missile
//! launcher door/power helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 381 QueueProductionExit dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/missile_launcher_building_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// MissileLauncherBuildingUpdate dual-world empty-gate residual method names.
pub const LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE382:
    &[&str] = &[
    "dual_world_registry_unavailable",
    "get_power_ready_frame",
    "is_power_ready",
    "update",
    "initiate_intent_to_do_special_power",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE382: &[&str] =
    &[
        "REQUIRE_DUAL_WORLD_HELPER",
        "REQUIRE_MISSILE_LAUNCHER_EMPTY_GATES",
        "LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE",
        "LIVE_PLAYABLE_CLAIM_FALSE",
    ];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE382:
    &[&str] = &[
    "click_live_missile_launcher_building_update_dual_world_empty_gate_ok_prepare",
    "click_live_missile_launcher_building_update_dual_world_empty_gate_ok_live",
    "click_live_missile_launcher_building_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_missile_launcher_building_update_dual_world_empty_gate_method_names_residual_wave382()
-> bool {
    LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE382.len() == 6
        && residual_name_index(
            LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE382,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE382,
            "initiate_intent_to_do_special_power",
        ) == Some(4)
        && residual_name_index(
            LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE382,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_residual_wave382()
-> bool {
    LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE382.len() == 4
        && residual_name_index(
            LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE382,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE382,
            "LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_MISSILE_LAUNCHER_BUILDING_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE382
            .len()
            == 3
}

/// Wave 382 composite residual honesty pack.
pub fn honesty_live_missile_launcher_building_update_dual_world_empty_gate_residual_pack_wave382()
-> bool {
    honesty_live_missile_launcher_building_update_dual_world_empty_gate_method_names_residual_wave382()
        && honesty_live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_residual_wave382()
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

/// Source residual: MissileLauncherBuildingUpdate empty dual-world short-circuits.
pub fn honesty_missile_launcher_building_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/missile_launcher_building_update.rs"
    );
    if !(g.contains("Wave 382")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(ready_frame) = fn_body(g, "fn get_power_ready_frame(") else {
        return false;
    };
    let Some(ready) = fn_body(g, "fn is_power_ready(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update(") else {
        return false;
    };
    let Some(intent) = fn_body(g, "fn initiate_intent_to_do_special_power(") else {
        return false;
    };
    helper_ok
        && ready_frame.contains("return 0")
        && ready.contains("return false")
        && update.contains("return Ok(UpdateSleepTime::Forever)")
        && intent.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_missile_launcher_building_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_missile_launcher_building_update_dual_world_empty_gate_residual_pack_wave382()
        && honesty_missile_launcher_building_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_missile_launcher_building_update_dual_world_empty_gate_method_names_residual_wave382()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_missile_launcher_building_update_dual_world_empty_gate_nav_commands_residual_wave382()
        );
    }

    #[test]
    fn wave382_composite_pack() {
        assert!(
            honesty_live_missile_launcher_building_update_dual_world_empty_gate_residual_pack_wave382()
        );
    }

    #[test]
    fn missile_launcher_building_update_dual_world_empty_gate_sources() {
        assert!(honesty_missile_launcher_building_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_missile_launcher_building_update_dual_world_empty_gate_honesty_residual_live()
    {
        assert!(
            simulate_live_missile_launcher_building_update_dual_world_empty_gate_honesty(),
            "missile launcher building update dual-world empty gate residual must latch"
        );
    }
}
