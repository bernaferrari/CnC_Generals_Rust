//! Wave 367 residual peels: FireWeaponWhenDamagedBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), damage-fire
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 366 POWTruckBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/fire_weapon_when_damaged_behavior_new.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Snapshot `xfer` intentionally ungated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// FireWeaponWhenDamagedBehavior dual-world empty-gate residual method names.
pub const LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE367:
    &[&str] = &[
    "dual_world_registry_unavailable",
    "set_wake_frame",
    "on_damage",
    "update_simple",
    "apply_upgrade",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE367:
    &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_FIRE_WEAPON_WHEN_DAMAGED_EMPTY_GATES",
    "LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE367:
    &[&str] = &[
    "click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_ok_prepare",
    "click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_ok_live",
    "click_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_residual_wave367()
-> bool {
    LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE367.len() == 6
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE367,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE367,
            "apply_upgrade",
        ) == Some(4)
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE367,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_residual_wave367()
-> bool {
    LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE367.len() == 4
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE367,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE367,
            "LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_FIRE_WEAPON_WHEN_DAMAGED_BEHAVIOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE367
            .len()
            == 3
}

/// Wave 367 composite residual honesty pack.
pub fn honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_residual_pack_wave367()
-> bool {
    honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_residual_wave367()
        && honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_residual_wave367()
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

/// Source residual: FireWeaponWhenDamagedBehavior empty dual-world short-circuits.
pub fn honesty_fire_weapon_when_damaged_behavior_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/fire_weapon_when_damaged_behavior_new.rs"
    );
    if !(g.contains("Wave 367")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(wake) = fn_body(g, "fn set_wake_frame(") else {
        return false;
    };
    let Some(damage) = fn_body(g, "fn on_damage(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    let Some(upgrade) = fn_body(g, "fn apply_upgrade(") else {
        return false;
    };
    // xfer must remain ungated
    let xfer_ok = !g.contains("// Wave 367: empty dual-world → Ok(()).\n        if dual_world_registry_unavailable() {\n            return Ok(());\n        }\n\n        let mut version");
    helper_ok
        && wake.contains("return;")
        && damage.contains("return Ok(())")
        && update.contains("return UPDATE_SLEEP_FOREVER")
        && upgrade.contains("return false")
        && xfer_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_honesty() -> bool {
    honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_residual_pack_wave367()
        && honesty_fire_weapon_when_damaged_behavior_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_method_names_residual_wave367()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_nav_commands_residual_wave367()
        );
    }

    #[test]
    fn wave367_composite_pack() {
        assert!(
            honesty_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_residual_pack_wave367()
        );
    }

    #[test]
    fn fire_weapon_when_damaged_behavior_dual_world_empty_gate_sources() {
        assert!(honesty_fire_weapon_when_damaged_behavior_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_honesty_residual_live()
    {
        assert!(
            simulate_live_fire_weapon_when_damaged_behavior_dual_world_empty_gate_honesty(),
            "fire weapon when damaged behavior dual-world empty gate residual must latch"
        );
    }
}
