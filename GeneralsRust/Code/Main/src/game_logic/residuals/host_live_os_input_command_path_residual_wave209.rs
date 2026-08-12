//! Wave 209 residual peels: OS window mouse path owns command intake
//! (`WindowEvent` → `handle_left_click` / `handle_right_click` → selection /
//! context commands). Full `GameClient::update` OS-input remains disconnected
//! by design (Main owns intake). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 208 golden mop-up honesty.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` winit MouseButton → handle_*_click
//! - handle_right_click → CommandSystem MouseCommandContext
//! - handle_left_click → selection / force-attack / map command
//!
//! Fail-closed:
//! - Not full retail WND shell menu click routing
//! - Not GameClient::update OS-input cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// OS-input command path residual method names.
pub const LIVE_OS_INPUT_COMMAND_PATH_METHOD_NAMES_WAVE209: &[&str] = &[
    "handle_left_click",
    "handle_right_click",
    "MouseButton::Left",
    "MouseCommandContext",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_OS_INPUT_COMMAND_PATH_NAV_STEPS_WAVE209: &[&str] = &[
    "REQUIRE_WINDOW_EVENT_MOUSE_TO_HANDLE_CLICK",
    "REQUIRE_RIGHT_CLICK_CONTEXT_COMMANDS",
    "REQUIRE_GAMECLIENT_UPDATE_OS_INPUT_DISCONNECTED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_OS_INPUT_COMMAND_PATH_CMD_NAMES_WAVE209: &[&str] = &[
    "click_live_os_input_command_path_ok_prepare",
    "click_live_os_input_command_path_ok_live",
    "click_live_os_input_command_path_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_os_input_command_path_method_names_residual_wave209() -> bool {
    LIVE_OS_INPUT_COMMAND_PATH_METHOD_NAMES_WAVE209.len() == 5
        && residual_name_index(
            LIVE_OS_INPUT_COMMAND_PATH_METHOD_NAMES_WAVE209,
            "handle_left_click",
        ) == Some(0)
        && residual_name_index(
            LIVE_OS_INPUT_COMMAND_PATH_METHOD_NAMES_WAVE209,
            "handle_right_click",
        ) == Some(1)
        && residual_name_index(
            LIVE_OS_INPUT_COMMAND_PATH_METHOD_NAMES_WAVE209,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_os_input_command_path_nav_commands_residual_wave209() -> bool {
    LIVE_OS_INPUT_COMMAND_PATH_NAV_STEPS_WAVE209.len() == 4
        && residual_name_index(
            LIVE_OS_INPUT_COMMAND_PATH_NAV_STEPS_WAVE209,
            "REQUIRE_WINDOW_EVENT_MOUSE_TO_HANDLE_CLICK",
        ) == Some(0)
        && residual_name_index(
            LIVE_OS_INPUT_COMMAND_PATH_NAV_STEPS_WAVE209,
            "REQUIRE_GAMECLIENT_UPDATE_OS_INPUT_DISCONNECTED",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_OS_INPUT_COMMAND_PATH_CMD_NAMES_WAVE209.len() == 3
}

/// Wave 209 composite residual honesty pack.
pub fn honesty_live_os_input_command_path_residual_pack_wave209() -> bool {
    honesty_live_os_input_command_path_method_names_residual_wave209()
        && honesty_live_os_input_command_path_nav_commands_residual_wave209()
}

/// Source residual: WindowEvent mouse pressed routes to handle_*_click.
pub fn honesty_window_event_mouse_to_handle_click_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    eng.contains("MouseButton::Left, ElementState::Pressed")
        && eng.contains("self.handle_left_click()")
        && eng.contains("MouseButton::Right, ElementState::Pressed")
        && eng.contains("self.handle_right_click(origin, physical_rmb_gesture)")
}

/// Source residual: right-click builds context commands via CommandSystem.
pub fn honesty_right_click_context_commands_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = match eng.find("fn handle_right_click") {
        Some(i) => i,
        None => return false,
    };
    let body = &eng[i..eng.len().min(i + 3500)];
    body.contains("MouseCommandContext")
        && (body.contains("queue_command")
            || body.contains("issue_")
            || body.contains("CommandSystem")
            || body.contains("command_system"))
}

/// Source residual: left-click owns selection / force-attack residual.
pub fn honesty_left_click_selection_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    let i = match eng.find("fn handle_left_click") {
        Some(i) => i,
        None => return false,
    };
    let body = &eng[i..eng.len().min(i + 3500)];
    body.contains("find_object_at_position")
        && (body.contains("select")
            || body.contains("toggle_select")
            || body.contains("issue_force_attack"))
}

/// Source residual: GameClient::update OS-input remains disconnected (Main owns intake).
pub fn honesty_gameclient_update_os_input_disconnected_source() -> bool {
    let eng = crate::cnc_game_engine::ENGINE_SRC;
    (eng.contains("Full GameClient::update() OS-input path") && eng.contains("Main owns input"))
        || (eng.contains("GameClient::update")
            && eng.contains("OS-input")
            && eng.contains("not used"))
}

/// Live residual: source honesty pack for OS-input command ownership.
pub fn simulate_live_os_input_command_path_honesty() -> bool {
    honesty_live_os_input_command_path_residual_pack_wave209()
        && honesty_window_event_mouse_to_handle_click_source()
        && honesty_right_click_context_commands_source()
        && honesty_left_click_selection_source()
        && honesty_gameclient_update_os_input_disconnected_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_os_input_command_path_method_names_residual_wave209());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_os_input_command_path_nav_commands_residual_wave209());
    }

    #[test]
    fn wave209_composite_pack() {
        assert!(honesty_live_os_input_command_path_residual_pack_wave209());
    }

    #[test]
    fn os_input_sources() {
        assert!(honesty_window_event_mouse_to_handle_click_source());
        assert!(honesty_right_click_context_commands_source());
        assert!(honesty_left_click_selection_source());
        assert!(honesty_gameclient_update_os_input_disconnected_source());
    }

    #[test]
    fn simulate_live_os_input_command_path_honesty_residual_live() {
        assert!(
            simulate_live_os_input_command_path_honesty(),
            "os-input command path residual must latch"
        );
    }
}
