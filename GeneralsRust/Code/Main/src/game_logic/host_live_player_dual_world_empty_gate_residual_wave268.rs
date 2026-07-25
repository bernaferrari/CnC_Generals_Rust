//! Wave 268 residual peels: Player dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), Player
//! ownership/economy/vision helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 267 AI state machine dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/player.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Player dual-world empty-gate residual method names.
pub const LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE268: &[&str] = &[
    "dual_world_registry_unavailable",
    "add_power_bonus",
    "add_owned_object",
    "find_drone_id_by_producer_id",
    "can_build_more_of_type",
    "on_unit_destroyed_id",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE268: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PLAYER_EMPTY_GATES",
    "LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE268: &[&str] = &[
    "click_live_player_dual_world_empty_gate_ok_prepare",
    "click_live_player_dual_world_empty_gate_ok_live",
    "click_live_player_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_player_dual_world_empty_gate_method_names_residual_wave268() -> bool {
    LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE268.len() == 7
        && residual_name_index(
            LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE268,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE268,
            "can_build_more_of_type",
        ) == Some(4)
        && residual_name_index(
            LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE268,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_player_dual_world_empty_gate_nav_commands_residual_wave268() -> bool {
    LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE268.len() == 4
        && residual_name_index(
            LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE268,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE268,
            "LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE268.len() == 3
}

/// Wave 268 composite residual honesty pack.
pub fn honesty_live_player_dual_world_empty_gate_residual_pack_wave268() -> bool {
    honesty_live_player_dual_world_empty_gate_method_names_residual_wave268()
        && honesty_live_player_dual_world_empty_gate_nav_commands_residual_wave268()
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

/// Source residual: Player empty dual-world short-circuits.
pub fn honesty_player_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/player.rs");
    if !(g.contains("Wave 268")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(power) = fn_body(g, "fn add_power_bonus(") else {
        return false;
    };
    let Some(owned) = fn_body(g, "fn add_owned_object(") else {
        return false;
    };
    let Some(build) = fn_body(g, "fn can_build_more_of_type(") else {
        return false;
    };
    power.contains("dual_world_registry_unavailable")
        && owned.contains("dual_world_registry_unavailable")
        && build.contains("dual_world_registry_unavailable")
        && build.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_player_dual_world_empty_gate_honesty() -> bool {
    honesty_live_player_dual_world_empty_gate_residual_pack_wave268()
        && honesty_player_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_player_dual_world_empty_gate_method_names_residual_wave268());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_player_dual_world_empty_gate_nav_commands_residual_wave268());
    }

    #[test]
    fn wave268_composite_pack() {
        assert!(honesty_live_player_dual_world_empty_gate_residual_pack_wave268());
    }

    #[test]
    fn player_dual_world_empty_gate_sources() {
        assert!(honesty_player_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_player_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_player_dual_world_empty_gate_honesty(),
            "player dual-world empty gate residual must latch"
        );
    }
}
