//! Wave 230 residual peels: command_system move/attack/force-attack/stop/guard
//! execute paths mutate units via GameLogic `unit_command_*` APIs (no direct
//! `get_object_mut` in those execute bodies). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 229 RMB selected presentation-only residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` unit_command_move_to / attack / force_attack / stop / guard_*
//! - `command_system.rs` execute_move/attack/force_attack/stop/guard_command
//!
//! Fail-closed:
//! - Not full CommandExecutor scatter/repair/enter matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Command unit authority API residual method names.
pub const LIVE_COMMAND_UNIT_AUTHORITY_API_METHOD_NAMES_WAVE230: &[&str] = &[
    "unit_command_move_to",
    "unit_command_attack",
    "unit_command_force_attack",
    "unit_command_stop",
    "unit_command_guard_position",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_UNIT_AUTHORITY_API_NAV_STEPS_WAVE230: &[&str] = &[
    "REQUIRE_COMMAND_UNIT_AUTHORITY_API",
    "REQUIRE_EXECUTE_MOVE_ATTACK_STOP_GUARD",
    "LIVE_COMMAND_UNIT_AUTHORITY_API",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_UNIT_AUTHORITY_API_CMD_NAMES_WAVE230: &[&str] = &[
    "click_live_command_unit_authority_api_ok_prepare",
    "click_live_command_unit_authority_api_ok_live",
    "click_live_command_unit_authority_api_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_unit_authority_api_method_names_residual_wave230() -> bool {
    LIVE_COMMAND_UNIT_AUTHORITY_API_METHOD_NAMES_WAVE230.len() == 6
        && residual_name_index(
            LIVE_COMMAND_UNIT_AUTHORITY_API_METHOD_NAMES_WAVE230,
            "unit_command_move_to",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_UNIT_AUTHORITY_API_METHOD_NAMES_WAVE230,
            "unit_command_stop",
        ) == Some(3)
        && residual_name_index(
            LIVE_COMMAND_UNIT_AUTHORITY_API_METHOD_NAMES_WAVE230,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_unit_authority_api_nav_commands_residual_wave230() -> bool {
    LIVE_COMMAND_UNIT_AUTHORITY_API_NAV_STEPS_WAVE230.len() == 4
        && residual_name_index(
            LIVE_COMMAND_UNIT_AUTHORITY_API_NAV_STEPS_WAVE230,
            "REQUIRE_COMMAND_UNIT_AUTHORITY_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_UNIT_AUTHORITY_API_NAV_STEPS_WAVE230,
            "LIVE_COMMAND_UNIT_AUTHORITY_API",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_UNIT_AUTHORITY_API_CMD_NAMES_WAVE230.len() == 3
}

/// Wave 230 composite residual honesty pack.
pub fn honesty_live_command_unit_authority_api_residual_pack_wave230() -> bool {
    honesty_live_command_unit_authority_api_method_names_residual_wave230()
        && honesty_live_command_unit_authority_api_nav_commands_residual_wave230()
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

/// Source residual: GameLogic APIs + execute bodies call them without get_object_mut.
pub fn honesty_command_unit_authority_api_source() -> bool {
    let gl = include_str!("../game_logic.rs");
    let cs = include_str!("../../command_system.rs");
    for api in [
        "pub fn unit_command_move_to",
        "pub fn unit_command_attack",
        "pub fn unit_command_force_attack",
        "pub fn unit_command_stop",
        "pub fn unit_command_guard_position",
        "pub fn unit_command_guard_object",
    ] {
        if !gl.contains(api) {
            return false;
        }
    }
    for (name, token) in [
        ("fn execute_move_command", "unit_command_move_to"),
        ("fn execute_attack_command", "unit_command_attack"),
        (
            "fn execute_force_attack_command",
            "unit_command_force_attack",
        ),
        ("fn execute_stop_command", "unit_command_stop"),
        ("fn execute_guard_command", "unit_command_guard_"),
    ] {
        let Some(body) = fn_body(cs, name) else {
            return false;
        };
        if !body.contains("Wave 230") || !body.contains(token) || body.contains("get_object_mut") {
            return false;
        }
    }
    true
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_command_unit_authority_api_honesty() -> bool {
    honesty_live_command_unit_authority_api_residual_pack_wave230()
        && honesty_command_unit_authority_api_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_unit_authority_api_method_names_residual_wave230());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_unit_authority_api_nav_commands_residual_wave230());
    }

    #[test]
    fn wave230_composite_pack() {
        assert!(honesty_live_command_unit_authority_api_residual_pack_wave230());
    }

    #[test]
    fn command_unit_sources() {
        assert!(honesty_command_unit_authority_api_source());
    }

    #[test]
    fn simulate_live_command_unit_authority_api_honesty_residual_live() {
        assert!(
            simulate_live_command_unit_authority_api_honesty(),
            "command unit authority API residual must latch"
        );
    }
}
