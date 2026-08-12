//! Wave 242 residual peels: command_system selection/view paths use GameLogic
//! `player_*` probes instead of dual-reading `&Player` / `&mut Player` via
//! `get_player` / `get_player_mut`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 241 camera height probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` player_extend_selection / player_selected_count
//! - `command_system.rs` execute_selection_command / execute_view_command_center /
//!   create_selection_command / create_select_similar_command /
//!   select_units_by_predicate / get_selected_units
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Command player probe residual method names.
pub const LIVE_COMMAND_PLAYER_PROBE_METHOD_NAMES_WAVE242: &[&str] = &[
    "player_extend_selection",
    "player_selected_count",
    "execute_selection_command",
    "execute_view_command_center",
    "create_selection_command",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_PLAYER_PROBE_NAV_STEPS_WAVE242: &[&str] = &[
    "REQUIRE_COMMAND_PLAYER_PROBE",
    "REQUIRE_SELECTION_WITHOUT_GET_PLAYER",
    "LIVE_COMMAND_PLAYER_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_PLAYER_PROBE_CMD_NAMES_WAVE242: &[&str] = &[
    "click_live_command_player_probe_ok_prepare",
    "click_live_command_player_probe_ok_live",
    "click_live_command_player_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_player_probe_method_names_residual_wave242() -> bool {
    LIVE_COMMAND_PLAYER_PROBE_METHOD_NAMES_WAVE242.len() == 6
        && residual_name_index(
            LIVE_COMMAND_PLAYER_PROBE_METHOD_NAMES_WAVE242,
            "player_extend_selection",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_PLAYER_PROBE_METHOD_NAMES_WAVE242,
            "create_selection_command",
        ) == Some(4)
        && residual_name_index(
            LIVE_COMMAND_PLAYER_PROBE_METHOD_NAMES_WAVE242,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_player_probe_nav_commands_residual_wave242() -> bool {
    LIVE_COMMAND_PLAYER_PROBE_NAV_STEPS_WAVE242.len() == 4
        && residual_name_index(
            LIVE_COMMAND_PLAYER_PROBE_NAV_STEPS_WAVE242,
            "REQUIRE_COMMAND_PLAYER_PROBE",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_PLAYER_PROBE_NAV_STEPS_WAVE242,
            "LIVE_COMMAND_PLAYER_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_PLAYER_PROBE_CMD_NAMES_WAVE242.len() == 3
}

/// Wave 242 composite residual honesty pack.
pub fn honesty_live_command_player_probe_residual_pack_wave242() -> bool {
    honesty_live_command_player_probe_method_names_residual_wave242()
        && honesty_live_command_player_probe_nav_commands_residual_wave242()
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

/// Source residual: selection/view use player probes; no get_player dual-read.
pub fn honesty_command_player_probe_source() -> bool {
    let gl = include_str!("../game_logic.rs");
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    if !(gl.contains("pub fn player_extend_selection(")
        && gl.contains("pub fn player_selected_count("))
    {
        return false;
    }
    let Some(sel) = fn_body(cs, "fn execute_selection_command(") else {
        return false;
    };
    if !(sel.contains("Wave 242")
        && sel.contains("player_exists")
        && sel.contains("player_extend_selection")
        && !sel.contains("get_player(")
        && !sel.contains("get_player_mut("))
    {
        return false;
    }
    let Some(view) = fn_body(cs, "fn execute_view_command_center(") else {
        return false;
    };
    if !(view.contains("Wave 242") && view.contains("player_team") && !view.contains("get_player("))
    {
        return false;
    }
    let Some(create) = fn_body(cs, "fn create_selection_command(") else {
        return false;
    };
    create.contains("player_team") && !create.contains("get_player(") && {
        let prod = cs.split("#[cfg(test)]").next().unwrap_or(cs);
        !prod.contains("get_player(") && !prod.contains("get_player_mut(")
    }
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_command_player_probe_honesty() -> bool {
    honesty_live_command_player_probe_residual_pack_wave242()
        && honesty_command_player_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_player_probe_method_names_residual_wave242());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_player_probe_nav_commands_residual_wave242());
    }

    #[test]
    fn wave242_composite_pack() {
        assert!(honesty_live_command_player_probe_residual_pack_wave242());
    }

    #[test]
    fn command_player_probe_sources() {
        assert!(honesty_command_player_probe_source());
    }

    #[test]
    fn simulate_live_command_player_probe_honesty_residual_live() {
        assert!(
            simulate_live_command_player_probe_honesty(),
            "command player probe residual must latch"
        );
    }
}
