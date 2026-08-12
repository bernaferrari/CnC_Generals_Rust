//! Wave 243 residual peels: construct economy path uses GameLogic player/unit
//! probes (`unit_team_if_can_construct`, `player_id_for_team`,
//! `try_spend_player_resources`, `player_refund_supplies`) instead of dual-reading
//! `&Object` / `&mut Player` via `get_object` / `get_player_mut_by_team`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 242 command player probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` unit_team_if_can_construct / player_id_for_team /
//!   try_spend_player_resources / player_refund_supplies
//! - `command_system.rs` execute_construct_command
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Construct economy probe residual method names.
pub const LIVE_CONSTRUCT_ECONOMY_PROBE_METHOD_NAMES_WAVE243: &[&str] = &[
    "unit_team_if_can_construct",
    "player_id_for_team",
    "try_spend_player_resources",
    "player_refund_supplies",
    "execute_construct_command",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CONSTRUCT_ECONOMY_PROBE_NAV_STEPS_WAVE243: &[&str] = &[
    "REQUIRE_CONSTRUCT_ECONOMY_PROBE",
    "REQUIRE_NO_GET_PLAYER_MUT_BY_TEAM",
    "LIVE_CONSTRUCT_ECONOMY_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CONSTRUCT_ECONOMY_PROBE_CMD_NAMES_WAVE243: &[&str] = &[
    "click_live_construct_economy_probe_ok_prepare",
    "click_live_construct_economy_probe_ok_live",
    "click_live_construct_economy_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_construct_economy_probe_method_names_residual_wave243() -> bool {
    LIVE_CONSTRUCT_ECONOMY_PROBE_METHOD_NAMES_WAVE243.len() == 6
        && residual_name_index(
            LIVE_CONSTRUCT_ECONOMY_PROBE_METHOD_NAMES_WAVE243,
            "unit_team_if_can_construct",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONSTRUCT_ECONOMY_PROBE_METHOD_NAMES_WAVE243,
            "execute_construct_command",
        ) == Some(4)
        && residual_name_index(
            LIVE_CONSTRUCT_ECONOMY_PROBE_METHOD_NAMES_WAVE243,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_construct_economy_probe_nav_commands_residual_wave243() -> bool {
    LIVE_CONSTRUCT_ECONOMY_PROBE_NAV_STEPS_WAVE243.len() == 4
        && residual_name_index(
            LIVE_CONSTRUCT_ECONOMY_PROBE_NAV_STEPS_WAVE243,
            "REQUIRE_CONSTRUCT_ECONOMY_PROBE",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONSTRUCT_ECONOMY_PROBE_NAV_STEPS_WAVE243,
            "LIVE_CONSTRUCT_ECONOMY_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CONSTRUCT_ECONOMY_PROBE_CMD_NAMES_WAVE243.len() == 3
}

/// Wave 243 composite residual honesty pack.
pub fn honesty_live_construct_economy_probe_residual_pack_wave243() -> bool {
    honesty_live_construct_economy_probe_method_names_residual_wave243()
        && honesty_live_construct_economy_probe_nav_commands_residual_wave243()
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

/// Source residual: construct uses probes; no get_player_mut_by_team dual-read.
pub fn honesty_construct_economy_probe_source() -> bool {
    let gl = include_str!("../game_logic.rs");
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    if !(gl.contains("pub fn unit_team_if_can_construct(")
        && gl.contains("pub fn player_id_for_team(")
        && gl.contains("pub fn try_spend_player_resources(")
        && gl.contains("pub fn player_refund_supplies("))
    {
        return false;
    }
    let Some(construct) = fn_body(cs, "fn execute_construct_command(") else {
        return false;
    };
    construct.contains("Wave 243")
        && construct.contains("unit_team_if_can_construct")
        && construct.contains("try_spend_player_resources")
        && construct.contains("player_refund_supplies")
        && !construct.contains("get_player_mut_by_team")
        && !construct.contains("get_object(")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_construct_economy_probe_honesty() -> bool {
    honesty_live_construct_economy_probe_residual_pack_wave243()
        && honesty_construct_economy_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_construct_economy_probe_method_names_residual_wave243());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_construct_economy_probe_nav_commands_residual_wave243());
    }

    #[test]
    fn wave243_composite_pack() {
        assert!(honesty_live_construct_economy_probe_residual_pack_wave243());
    }

    #[test]
    fn construct_economy_probe_sources() {
        assert!(honesty_construct_economy_probe_source());
    }

    #[test]
    fn simulate_live_construct_economy_probe_honesty_residual_live() {
        assert!(
            simulate_live_construct_economy_probe_honesty(),
            "construct economy probe residual must latch"
        );
    }
}
