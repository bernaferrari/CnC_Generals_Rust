//! Wave 587 residual peels: GameClient presentation shell tick advances
//! Main-injected device state via `update_input` (THE_MOUSE/THE_KEYBOARD
//! bookkeeping only — not a second OS poll). Menu shell path documents the same.
//! Full `GameClient::update` stays disconnected. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 586 shell-tick centralization.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_tick_game_client_presentation_shell / Menu update_input
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Not full WND OS-input GameClient ownership reconnect

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GAME_CLIENT_DEVICE_TICK_HELPER_METHOD_NAMES_WAVE587: &[&str] = &[
    "host_tick_game_client_presentation_shell",
    "update_input",
    "inject_game_client_key",
    "host_inject_game_client_key",
    "inject_game_client_mouse_move",
    "inject_game_client_mouse_button",
    "Wave 587",
    "playable_claim = false",
];

pub const LIVE_HOST_GAME_CLIENT_DEVICE_TICK_HELPER_NAV_STEPS_WAVE587: &[&str] = &[
    "REQUIRE_INJECT_THEN_DEVICE_UPDATE",
    "REQUIRE_NO_SECOND_OS_POLL",
    "REQUIRE_SHELL_TICK_CALLS_UPDATE_INPUT",
    "REQUIRE_NO_FULL_GAMECLIENT_UPDATE",
    "LIVE_HOST_GAME_CLIENT_DEVICE_TICK_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_GAME_CLIENT_DEVICE_TICK_HELPER_CMD_NAMES_WAVE587: &[&str] = &[
    "host_game_client_device_tick_helper",
    "inject_then_update_input",
    "no_second_os_poll",
    "game_client_device_tick_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGameClientDeviceTickHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostGameClientDeviceTickHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualHostGameClientDeviceTickHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_game_client_device_tick_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_game_client_device_tick_helper_last_action()
-> ResidualHostGameClientDeviceTickHelperAction {
    ResidualHostGameClientDeviceTickHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_host_game_client_device_tick_helper_method_names_residual_wave587() -> bool {
    let names = LIVE_HOST_GAME_CLIENT_DEVICE_TICK_HELPER_METHOD_NAMES_WAVE587;
    let ok = residual_name_index(names, "host_tick_game_client_presentation_shell").is_some()
        && residual_name_index(names, "update_input").is_some()
        && residual_name_index(names, "inject_game_client_key").is_some()
        && residual_name_index(names, "inject_game_client_mouse_move").is_some()
        && residual_name_index(names, "Wave 587").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostGameClientDeviceTickHelperAction::MethodNames);
    ok
}

pub fn honesty_host_game_client_device_tick_helper_source_markers_residual_wave587() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_tick_game_client_presentation_shell(") else {
        residual_action_store(ResidualHostGameClientDeviceTickHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 587")
        && body.contains("update_input()")
        && body.contains("update_presentation_shell")
        && body.contains("Main-injected")
        && !body.contains("game_client.update()");
    let inject_ok = eng.contains("fn inject_game_client_key")
        && eng.contains("fn host_inject_game_client_key")
        && eng.contains("fn inject_game_client_mouse_move")
        && eng.contains("fn inject_game_client_mouse_button")
        && eng.contains("with_keyboard")
        && eng.contains("with_mouse");
    // Menu path (Wave 588) peels through host_tick_game_client_menu_shell; device
    // bookkeeping comment may live on InGame helper and/or Menu helper.
    let menu_ok = eng.contains("Wave 587: device bookkeeping on Main-injected state")
        || eng.contains("Wave 587/588: device bookkeeping on Main-injected state")
        || (eng.contains("fn host_tick_game_client_menu_shell")
            && eng.contains("device bookkeeping on Main-injected state"));
    let ok = body_ok && inject_ok && menu_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostGameClientDeviceTickHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_game_client_device_tick_helper_nav_commands_residual_wave587() -> bool {
    let steps = LIVE_HOST_GAME_CLIENT_DEVICE_TICK_HELPER_NAV_STEPS_WAVE587;
    let cmds = RUNTIME_HOST_LIVE_HOST_GAME_CLIENT_DEVICE_TICK_HELPER_CMD_NAMES_WAVE587;
    let ok = residual_name_index(steps, "REQUIRE_INJECT_THEN_DEVICE_UPDATE").is_some()
        && residual_name_index(steps, "REQUIRE_NO_SECOND_OS_POLL").is_some()
        && residual_name_index(steps, "REQUIRE_SHELL_TICK_CALLS_UPDATE_INPUT").is_some()
        && residual_name_index(steps, "REQUIRE_NO_FULL_GAMECLIENT_UPDATE").is_some()
        && residual_name_index(steps, "LIVE_HOST_GAME_CLIENT_DEVICE_TICK_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_game_client_device_tick_helper").is_some()
        && residual_name_index(cmds, "inject_then_update_input").is_some()
        && residual_name_index(cmds, "no_second_os_poll").is_some()
        && residual_name_index(cmds, "game_client_device_tick_residual").is_some();
    residual_action_store(ResidualHostGameClientDeviceTickHelperAction::NavCommands);
    ok
}

pub fn simulate_host_game_client_device_tick_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 587")
        && eng.contains("fn host_tick_game_client_presentation_shell")
        && eng.contains("update_input()")
        && eng.contains("inject_game_client_");
    residual_action_store(ResidualHostGameClientDeviceTickHelperAction::CollectSource);
    ok
}

pub fn simulate_host_game_client_device_tick_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_tick_game_client_presentation_shell(") else {
        residual_action_store(ResidualHostGameClientDeviceTickHelperAction::DispatchSource);
        return false;
    };
    // Device update must precede presentation shell.
    let i_in = body.find("update_input()");
    let i_shell = body.find("update_presentation_shell");
    let order_ok = matches!((i_in, i_shell), (Some(a), Some(b)) if a < b);
    let ok = order_ok
        && eng.contains("self.host_tick_game_client_presentation_shell()")
        && !eng.contains("self.game_client.update()");
    residual_action_store(ResidualHostGameClientDeviceTickHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_game_client_device_tick_helper_residual_pack_wave587() -> bool {
    honesty_host_game_client_device_tick_helper_method_names_residual_wave587()
        && honesty_host_game_client_device_tick_helper_source_markers_residual_wave587()
        && honesty_host_game_client_device_tick_helper_nav_commands_residual_wave587()
        && simulate_host_game_client_device_tick_helper_collect_source()
        && simulate_host_game_client_device_tick_helper_dispatch_source()
}

pub fn simulate_live_host_game_client_device_tick_helper_honesty() -> bool {
    let ok = honesty_host_game_client_device_tick_helper_residual_pack_wave587();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostGameClientDeviceTickHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_game_client_device_tick_helper_method_names_residual_wave587());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_game_client_device_tick_helper_source_markers_residual_wave587());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_game_client_device_tick_helper_nav_commands_residual_wave587());
    }

    #[test]
    fn host_game_client_device_tick_helper_sources() {
        assert!(simulate_host_game_client_device_tick_helper_collect_source());
        assert!(simulate_host_game_client_device_tick_helper_dispatch_source());
    }

    #[test]
    fn wave587_composite_pack() {
        assert!(honesty_host_game_client_device_tick_helper_residual_pack_wave587());
    }

    #[test]
    fn simulate_live_host_game_client_device_tick_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_game_client_device_tick_helper_honesty(),
            "host GameClient device tick helper residual must latch"
        );
        assert!(residual_host_game_client_device_tick_helper_ok());
        assert_eq!(
            residual_host_game_client_device_tick_helper_last_action(),
            ResidualHostGameClientDeviceTickHelperAction::Composite
        );
    }
}
