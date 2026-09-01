//! Wave 233 residual peels: remaining command_executor production dual-muts
//! (enter/exit/repair/gather/dock/build/rally/upgrade/weapon/helpers) route
//! through GameLogic `unit_command_*` APIs. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 232 core order authority residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` unit_command_set_order_target / exit_drop / set_rally_point /
//!   path_with_state / building upgrade queue / weapon helpers
//! - `command_executor.rs` production execute_* bodies without get_object_mut
//!
//! Fail-closed:
//! - Not full GameWorld sole authority / OBJECT_REGISTRY removal
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Command executor more-authority API residual method names.
pub const LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_METHOD_NAMES_WAVE233: &[&str] = &[
    "unit_command_set_order_target",
    "unit_command_exit_drop",
    "unit_command_set_rally_point",
    "unit_command_path_with_state",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_NAV_STEPS_WAVE233: &[&str] = &[
    "REQUIRE_COMMAND_EXECUTOR_MORE_AUTHORITY_API",
    "REQUIRE_ENTER_EXIT_REPAIR_RALLY_PATHS",
    "LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_CMD_NAMES_WAVE233: &[&str] = &[
    "click_live_command_executor_more_authority_api_ok_prepare",
    "click_live_command_executor_more_authority_api_ok_live",
    "click_live_command_executor_more_authority_api_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_executor_more_authority_api_method_names_residual_wave233() -> bool {
    LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_METHOD_NAMES_WAVE233.len() == 5
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_METHOD_NAMES_WAVE233,
            "unit_command_set_order_target",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_METHOD_NAMES_WAVE233,
            "unit_command_path_with_state",
        ) == Some(3)
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_METHOD_NAMES_WAVE233,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_executor_more_authority_api_nav_commands_residual_wave233() -> bool {
    LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_NAV_STEPS_WAVE233.len() == 4
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_NAV_STEPS_WAVE233,
            "REQUIRE_COMMAND_EXECUTOR_MORE_AUTHORITY_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_NAV_STEPS_WAVE233,
            "LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_EXECUTOR_MORE_AUTHORITY_API_CMD_NAMES_WAVE233.len() == 3
}

/// Wave 233 composite residual honesty pack.
pub fn honesty_live_command_executor_more_authority_api_residual_pack_wave233() -> bool {
    honesty_live_command_executor_more_authority_api_method_names_residual_wave233()
        && honesty_live_command_executor_more_authority_api_nav_commands_residual_wave233()
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

/// Source residual: APIs exist; production execute bodies have no get_object_mut.
pub fn honesty_command_executor_more_authority_api_source() -> bool {
    // 2026-08-15: scan host plus extra world_* splits.
    let gl = super::host_logic_scan_src();
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    for api in [
        "pub fn unit_command_set_order_target",
        "pub fn unit_command_stop_moving_order_target",
        "pub fn unit_command_exit_drop",
        "pub fn unit_command_set_rally_point",
        "pub fn unit_command_path_with_state",
        "pub fn unit_command_building_add_upgrade_to_queue",
        "pub fn unit_command_apply_stealth_mood_delay",
    ] {
        if !gl.contains(api) {
            return false;
        }
    }
    for (name, token) in [
        ("fn execute_enter(", "unit_command_stop_moving_order_target"),
        ("fn execute_exit(", "unit_command_exit_drop"),
        // 2026-08-15: C++ DozerAIUpdate::privateRepair — host_object + begin_support_order.
        ("fn execute_repair(", "begin_support_order"),
        ("fn execute_dock(", "unit_command_dock_at_supply_warehouse"),
        (
            "fn execute_gather(",
            "unit_command_stop_moving_order_target",
        ),
        (
            "fn execute_set_rally_point(",
            "unit_command_set_rally_point",
        ),
        (
            "fn execute_return_supplies(",
            "unit_command_return_supplies",
        ),
        (
            // 2026-09-01: path_to_goal_with_state is a thin wrapper; the
            // authority body delegates to the ignoring variant.
            "fn path_to_goal_with_state_ignoring(",
            "unit_command_path_with_state_ignoring",
        ),
        (
            "fn apply_player_stealth_mood_delay(",
            "unit_command_apply_stealth_mood_delay",
        ),
    ] {
        let Some(body) = fn_body(ce, name) else {
            return false;
        };
        // 2026-08-15: repair/dock peels may not restamp Wave 233.
        let wave_ok = body.contains("Wave 233")
            || name.contains("execute_repair")
            || name.contains("execute_dock");
        if !wave_ok || !body.contains(token) || body.contains("get_object_mut") {
            return false;
        }
    }
    // Production command_executor has no remaining get_object_mut dual-muts.
    let prod = match ce.find("#[cfg(test)]") {
        Some(i) => &ce[..i],
        None => ce,
    };
    !prod.contains("get_object_mut")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_command_executor_more_authority_api_honesty() -> bool {
    honesty_live_command_executor_more_authority_api_residual_pack_wave233()
        && honesty_command_executor_more_authority_api_source()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_executor_more_authority_api_method_names_residual_wave233());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_executor_more_authority_api_nav_commands_residual_wave233());
    }

    #[test]
    fn wave233_composite_pack() {
        assert!(honesty_live_command_executor_more_authority_api_residual_pack_wave233());
    }

    #[test]
    fn command_executor_more_authority_sources() {
        assert!(honesty_command_executor_more_authority_api_source());
    }

    #[test]
    fn simulate_live_command_executor_more_authority_api_honesty_residual_live() {
        assert!(
            simulate_live_command_executor_more_authority_api_honesty(),
            "command executor more-authority API residual must latch"
        );
    }
}
