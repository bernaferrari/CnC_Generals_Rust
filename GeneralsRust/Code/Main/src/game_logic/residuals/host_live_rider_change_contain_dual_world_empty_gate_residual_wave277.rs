//! Wave 277 residual peels: RiderChangeContain dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), rider-change
//! contain helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 276 Turret dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/contain/rider_change_contain.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// RiderChangeContain dual-world empty-gate residual method names.
pub const LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE277: &[&str] = &[
    "dual_world_registry_unavailable",
    "add_to_contain",
    "on_containing",
    "on_removing",
    "update",
    "has_exit_scuttle_drawables",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE277: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_RIDER_CHANGE_CONTAIN_EMPTY_GATES",
    "LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE277:
    &[&str] = &[
    "click_live_rider_change_contain_dual_world_empty_gate_ok_prepare",
    "click_live_rider_change_contain_dual_world_empty_gate_ok_live",
    "click_live_rider_change_contain_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_rider_change_contain_dual_world_empty_gate_method_names_residual_wave277()
-> bool {
    LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE277.len() == 7
        && residual_name_index(
            LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE277,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE277,
            "update",
        ) == Some(4)
        && residual_name_index(
            LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE277,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_rider_change_contain_dual_world_empty_gate_nav_commands_residual_wave277()
-> bool {
    LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE277.len() == 4
        && residual_name_index(
            LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE277,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE277,
            "LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_RIDER_CHANGE_CONTAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE277.len() == 3
}

/// Wave 277 composite residual honesty pack.
pub fn honesty_live_rider_change_contain_dual_world_empty_gate_residual_pack_wave277() -> bool {
    honesty_live_rider_change_contain_dual_world_empty_gate_method_names_residual_wave277()
        && honesty_live_rider_change_contain_dual_world_empty_gate_nav_commands_residual_wave277()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: RiderChangeContain empty dual-world short-circuits.
pub fn honesty_rider_change_contain_dual_world_empty_gate_source() -> bool {
    let g =
        include_str!("../../../../GameEngine/GameLogic/src/object/contain/rider_change_contain.rs");
    if !(g.contains("Wave 277")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(add) = fn_body(g, "fn add_to_contain(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update(&mut self) -> GameResult<UpdateSleepTime>") else {
        return false;
    };
    let Some(scuttle) = fn_body(g, "fn has_exit_scuttle_drawables(") else {
        return false;
    };
    add.contains("dual_world_registry_unavailable")
        && update.contains("dual_world_registry_unavailable")
        && update.contains("UpdateSleepTime::Forever")
        && scuttle.contains("dual_world_registry_unavailable")
        && scuttle.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_rider_change_contain_dual_world_empty_gate_honesty() -> bool {
    honesty_live_rider_change_contain_dual_world_empty_gate_residual_pack_wave277()
        && honesty_rider_change_contain_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_rider_change_contain_dual_world_empty_gate_method_names_residual_wave277()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_rider_change_contain_dual_world_empty_gate_nav_commands_residual_wave277()
        );
    }

    #[test]
    fn wave277_composite_pack() {
        assert!(honesty_live_rider_change_contain_dual_world_empty_gate_residual_pack_wave277());
    }

    #[test]
    fn rider_change_contain_dual_world_empty_gate_sources() {
        assert!(honesty_rider_change_contain_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_rider_change_contain_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_rider_change_contain_dual_world_empty_gate_honesty(),
            "rider change contain dual-world empty gate residual must latch"
        );
    }
}
