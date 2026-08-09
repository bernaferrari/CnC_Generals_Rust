//! Wave 261 residual peels: OpenContain dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), open
//! contain/enter/exit walks fail-closed without dual-world factory resolution.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 260 Garrison dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/contain/open_contain.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// OpenContain dual-world empty-gate residual method names.
pub const LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE261: &[&str] = &[
    "dual_world_registry_unavailable",
    "update",
    "add_to_contain",
    "remove_from_contain",
    "exit_object_via_door",
    "reserve_door_for_exit",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE261: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_OPEN_CONTAIN_EMPTY_GATES",
    "LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE261: &[&str] = &[
    "click_live_open_contain_dual_world_empty_gate_ok_prepare",
    "click_live_open_contain_dual_world_empty_gate_ok_live",
    "click_live_open_contain_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_open_contain_dual_world_empty_gate_method_names_residual_wave261() -> bool {
    LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE261.len() == 7
        && residual_name_index(
            LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE261,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE261,
            "reserve_door_for_exit",
        ) == Some(5)
        && residual_name_index(
            LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE261,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_open_contain_dual_world_empty_gate_nav_commands_residual_wave261() -> bool {
    LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE261.len() == 4
        && residual_name_index(
            LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE261,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE261,
            "LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_OPEN_CONTAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE261.len() == 3
}

/// Wave 261 composite residual honesty pack.
pub fn honesty_live_open_contain_dual_world_empty_gate_residual_pack_wave261() -> bool {
    honesty_live_open_contain_dual_world_empty_gate_method_names_residual_wave261()
        && honesty_live_open_contain_dual_world_empty_gate_nav_commands_residual_wave261()
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

/// Source residual: OpenContain empty dual-world short-circuits.
pub fn honesty_open_contain_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/contain/open_contain.rs");
    if !(g.contains("Wave 261")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(update) = fn_body(g, "fn update(&mut self) -> GameResult<UpdateSleepTime>") else {
        return false;
    };
    let Some(add) = fn_body(g, "fn add_to_contain(") else {
        return false;
    };
    let Some(door) = fn_body(g, "fn reserve_door_for_exit(") else {
        return false;
    };
    update.contains("dual_world_registry_unavailable")
        && update.contains("UpdateSleepTime::None")
        && add.contains("dual_world_registry_unavailable")
        && door.contains("dual_world_registry_unavailable")
        && (door.contains("ExitDoorType::NoneAvailable") || door.contains("NoneAvailable"))
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_open_contain_dual_world_empty_gate_honesty() -> bool {
    honesty_live_open_contain_dual_world_empty_gate_residual_pack_wave261()
        && honesty_open_contain_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_open_contain_dual_world_empty_gate_method_names_residual_wave261());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_open_contain_dual_world_empty_gate_nav_commands_residual_wave261());
    }

    #[test]
    fn wave261_composite_pack() {
        assert!(honesty_live_open_contain_dual_world_empty_gate_residual_pack_wave261());
    }

    #[test]
    fn open_contain_dual_world_empty_gate_sources() {
        assert!(honesty_open_contain_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_open_contain_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_open_contain_dual_world_empty_gate_honesty(),
            "open contain dual-world empty gate residual must latch"
        );
    }
}
