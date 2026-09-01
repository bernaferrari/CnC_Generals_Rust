//! Wave 232 residual peels: command_executor core unit orders (move/attack/stop/
//! guard/attack-move/scatter/selection/formation/patrol/cheer/construct) route
//! through GameLogic `unit_command_*` APIs. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 231 command_system more-authority residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` unit_command_move_* / attack_* / stop / guard_full / patrol /
//!   cheer / set_formation / begin_construct
//! - `command_executor.rs` execute_move/attack/stop/guard/scatter/selection/...
//! - `command_system.rs` execute_construct_command → unit_command_begin_construct
//!
//! Fail-closed:
//! - Not full enter/repair/hero ability execute cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Command executor authority API residual method names.
pub const LIVE_COMMAND_EXECUTOR_AUTHORITY_API_METHOD_NAMES_WAVE232: &[&str] = &[
    "unit_command_move_free",
    "unit_command_attack",
    "unit_command_stop",
    "unit_command_guard_full",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_EXECUTOR_AUTHORITY_API_NAV_STEPS_WAVE232: &[&str] = &[
    "REQUIRE_COMMAND_EXECUTOR_AUTHORITY_API",
    "REQUIRE_MOVE_ATTACK_STOP_GUARD_ROUTES",
    "LIVE_COMMAND_EXECUTOR_AUTHORITY_API",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_EXECUTOR_AUTHORITY_API_CMD_NAMES_WAVE232: &[&str] = &[
    "click_live_command_executor_authority_api_ok_prepare",
    "click_live_command_executor_authority_api_ok_live",
    "click_live_command_executor_authority_api_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_executor_authority_api_method_names_residual_wave232() -> bool {
    LIVE_COMMAND_EXECUTOR_AUTHORITY_API_METHOD_NAMES_WAVE232.len() == 5
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_AUTHORITY_API_METHOD_NAMES_WAVE232,
            "unit_command_move_free",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_AUTHORITY_API_METHOD_NAMES_WAVE232,
            "unit_command_guard_full",
        ) == Some(3)
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_AUTHORITY_API_METHOD_NAMES_WAVE232,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_executor_authority_api_nav_commands_residual_wave232() -> bool {
    LIVE_COMMAND_EXECUTOR_AUTHORITY_API_NAV_STEPS_WAVE232.len() == 4
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_AUTHORITY_API_NAV_STEPS_WAVE232,
            "REQUIRE_COMMAND_EXECUTOR_AUTHORITY_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_AUTHORITY_API_NAV_STEPS_WAVE232,
            "LIVE_COMMAND_EXECUTOR_AUTHORITY_API",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_EXECUTOR_AUTHORITY_API_CMD_NAMES_WAVE232.len() == 3
}

/// Wave 232 composite residual honesty pack.
pub fn honesty_live_command_executor_authority_api_residual_pack_wave232() -> bool {
    honesty_live_command_executor_authority_api_method_names_residual_wave232()
        && honesty_live_command_executor_authority_api_nav_commands_residual_wave232()
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

/// Source residual: APIs exist; core execute bodies call them without get_object_mut.
pub fn honesty_command_executor_authority_api_source() -> bool {
    // 2026-08-15: scan host plus extra world_* splits.
    let gl = super::host_logic_scan_src();
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    for api in [
        "pub fn unit_command_move_free",
        "pub fn unit_command_move_to_waypoints",
        "pub fn unit_command_force_move_to",
        "pub fn unit_command_attack_move_to_ex",
        "pub fn unit_command_guard_full",
        "pub fn unit_command_patrol",
        "pub fn unit_command_cheer",
        "pub fn unit_command_begin_construct",
        "pub fn unit_command_set_formation",
    ] {
        if !gl.contains(api) {
            return false;
        }
    }
    for (name, token) in [
        ("fn execute_move(", "unit_command_move_free"),
        // 2026-09-01: execute_move_to is a thin wrapper; the authority body
        // (Wave 232 stamp + unit_command_move_to_waypoints) lives in
        // execute_move_to_with_voice.
        ("fn execute_move_to_with_voice(", "unit_command_move_to_waypoints"),
        ("fn execute_force_move(", "unit_command_force_move_to"),
        ("fn execute_attack(", "unit_command_attack"),
        ("fn execute_force_attack(", "unit_command_force_attack"),
        ("fn execute_stop(", "unit_command_stop"),
        ("fn execute_attack_move(", "unit_command_attack_move_to_ex"),
        ("fn execute_scatter(", "unit_command_move_to_moving"),
        // 2026-08-15: selection is player-owned (unit_select_if_player / select_objects),
        // not faction-team unit_select_if_team. C++ SelectionManager is per-player.
        ("fn execute_selection(", "unit_select_if_player"),
        ("fn execute_guard(", "unit_command_guard_full"),
        ("fn execute_patrol(", "unit_command_patrol"),
        ("fn execute_cheer(", "unit_command_cheer"),
        ("fn execute_create_formation(", "unit_command_set_formation"),
        ("fn execute_attack_ground(", "unit_command_attack_ground_ex"),
    ] {
        let Some(body) = fn_body(ce, name) else {
            return false;
        };
        // 2026-08-15: execute_selection peeled to player-owned select (no Wave 232 stamp).
        let wave_ok = body.contains("Wave 232") || name.contains("execute_selection");
        if !wave_ok || !body.contains(token) || body.contains("get_object_mut") {
            return false;
        }
    }
    let Some(construct) = fn_body(cs, "fn execute_construct_command") else {
        return false;
    };
    construct.contains("Wave 232")
        && construct.contains("unit_command_begin_construct")
        && !construct.contains("get_object_mut")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_command_executor_authority_api_honesty() -> bool {
    honesty_live_command_executor_authority_api_residual_pack_wave232()
        && honesty_command_executor_authority_api_source()
}

mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_executor_authority_api_method_names_residual_wave232());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_executor_authority_api_nav_commands_residual_wave232());
    }

    #[test]
    fn wave232_composite_pack() {
        assert!(honesty_live_command_executor_authority_api_residual_pack_wave232());
    }

    #[test]
    fn command_executor_authority_sources() {
        assert!(honesty_command_executor_authority_api_source());
    }

    #[test]
    fn simulate_live_command_executor_authority_api_honesty_residual_live() {
        assert!(
            simulate_live_command_executor_authority_api_honesty(),
            "command executor authority API residual must latch"
        );
    }
}
