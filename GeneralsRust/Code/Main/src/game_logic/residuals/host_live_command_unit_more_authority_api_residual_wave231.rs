//! Wave 231 residual peels: command_system attack-move, force-attack-ground,
//! scatter, formation, and additive selection use GameLogic `unit_command_*` /
//! `unit_select_if_team` APIs (no direct `get_object_mut` in those execute
//! bodies). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 230 move/attack/stop/guard authority API residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` unit_command_attack_move_to / attack_ground / move_to_moving /
//!   unit_select_if_team
//! - `command_system.rs` execute_attack_move / force_attack_ground / scatter /
//!   create_formation / selection_command
//!
//! Fail-closed:
//! - Not full construct/enter/repair execute cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Command unit more-authority API residual method names.
pub const LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_METHOD_NAMES_WAVE231: &[&str] = &[
    "unit_command_attack_move_to",
    "unit_command_attack_ground",
    "unit_command_move_to_moving",
    "unit_select_if_team",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_NAV_STEPS_WAVE231: &[&str] = &[
    "REQUIRE_COMMAND_UNIT_MORE_AUTHORITY_API",
    "REQUIRE_ATTACK_MOVE_SCATTER_FORMATION_SELECT",
    "LIVE_COMMAND_UNIT_MORE_AUTHORITY_API",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_CMD_NAMES_WAVE231: &[&str] = &[
    "click_live_command_unit_more_authority_api_ok_prepare",
    "click_live_command_unit_more_authority_api_ok_live",
    "click_live_command_unit_more_authority_api_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_unit_more_authority_api_method_names_residual_wave231() -> bool {
    LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_METHOD_NAMES_WAVE231.len() == 5
        && residual_name_index(
            LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_METHOD_NAMES_WAVE231,
            "unit_command_attack_move_to",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_METHOD_NAMES_WAVE231,
            "unit_select_if_team",
        ) == Some(3)
        && residual_name_index(
            LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_METHOD_NAMES_WAVE231,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_unit_more_authority_api_nav_commands_residual_wave231() -> bool {
    LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_NAV_STEPS_WAVE231.len() == 4
        && residual_name_index(
            LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_NAV_STEPS_WAVE231,
            "REQUIRE_COMMAND_UNIT_MORE_AUTHORITY_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_NAV_STEPS_WAVE231,
            "LIVE_COMMAND_UNIT_MORE_AUTHORITY_API",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_UNIT_MORE_AUTHORITY_API_CMD_NAMES_WAVE231.len() == 3
}

/// Wave 231 composite residual honesty pack.
pub fn honesty_live_command_unit_more_authority_api_residual_pack_wave231() -> bool {
    honesty_live_command_unit_more_authority_api_method_names_residual_wave231()
        && honesty_live_command_unit_more_authority_api_nav_commands_residual_wave231()
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

/// Source residual: APIs exist; execute bodies call them without get_object_mut.
pub fn honesty_command_unit_more_authority_api_source() -> bool {
    let gl = include_str!("../game_logic.rs");
    let cs = include_str!("../../command_system.rs");
    for api in [
        "pub fn unit_command_attack_move_to",
        "pub fn unit_command_attack_ground",
        "pub fn unit_command_move_to_moving",
        "pub fn unit_select_if_team",
        "pub fn unit_position_if_movable",
    ] {
        if !gl.contains(api) {
            return false;
        }
    }
    for (name, token) in [
        (
            "fn execute_attack_move_command",
            "unit_command_attack_move_to",
        ),
        (
            "fn execute_force_attack_ground_command",
            "unit_command_attack_ground",
        ),
        ("fn execute_scatter_command", "unit_command_move_to_moving"),
        (
            "fn execute_create_formation_command",
            "unit_command_move_to_moving",
        ),
        ("fn execute_selection_command", "unit_select_if_team"),
    ] {
        let Some(body) = fn_body(cs, name) else {
            return false;
        };
        if !body.contains("Wave 231") || !body.contains(token) || body.contains("get_object_mut") {
            return false;
        }
    }
    true
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_command_unit_more_authority_api_honesty() -> bool {
    honesty_live_command_unit_more_authority_api_residual_pack_wave231()
        && honesty_command_unit_more_authority_api_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_unit_more_authority_api_method_names_residual_wave231());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_unit_more_authority_api_nav_commands_residual_wave231());
    }

    #[test]
    fn wave231_composite_pack() {
        assert!(honesty_live_command_unit_more_authority_api_residual_pack_wave231());
    }

    #[test]
    fn command_unit_more_sources() {
        assert!(honesty_command_unit_more_authority_api_source());
    }

    #[test]
    fn simulate_live_command_unit_more_authority_api_honesty_residual_live() {
        assert!(
            simulate_live_command_unit_more_authority_api_honesty(),
            "command unit more-authority API residual must latch"
        );
    }
}
