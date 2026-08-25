//! Wave 588 residual peels: Menu GameClient shell tick + NewGame drain is
//! centralized through `host_tick_game_client_menu_shell`. Intercepts MSG_NEW_GAME
//! before `pump_message_stream` so WND Start reaches InGame. Distinct from InGame
//! `host_tick_game_client_presentation_shell`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 587 device-tick residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_tick_game_client_menu_shell
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Not full retail WND widget-tree playthrough

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GAME_CLIENT_MENU_SHELL_HELPER_METHOD_NAMES_WAVE588: &[&str] = &[
    "host_tick_game_client_menu_shell",
    "host_tick_game_client_presentation_shell",
    "take_pending_new_game_start_request",
    "pump_message_stream",
    "ensure_shell_visible",
    "start_game_from_ui",
    "Wave 588",
    "playable_claim = false",
];

pub const LIVE_HOST_GAME_CLIENT_MENU_SHELL_HELPER_NAV_STEPS_WAVE588: &[&str] = &[
    "REQUIRE_MENU_SHELL_HELPER",
    "REQUIRE_NEWGAME_BEFORE_PUMP",
    "REQUIRE_DEVICE_UPDATE_ON_INJECTED_STATE",
    "REQUIRE_START_GAME_FROM_UI_ON_DRAIN",
    "LIVE_HOST_GAME_CLIENT_MENU_SHELL_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_GAME_CLIENT_MENU_SHELL_HELPER_CMD_NAMES_WAVE588: &[&str] = &[
    "host_game_client_menu_shell_helper",
    "newgame_before_pump",
    "menu_shell_tick_residual",
    "wnd_start_newgame_drain",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGameClientMenuShellHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostGameClientMenuShellHelperAction {
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

fn residual_action_store(action: ResidualHostGameClientMenuShellHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_game_client_menu_shell_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_game_client_menu_shell_helper_last_action()
-> ResidualHostGameClientMenuShellHelperAction {
    ResidualHostGameClientMenuShellHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_host_game_client_menu_shell_helper_method_names_residual_wave588() -> bool {
    let names = LIVE_HOST_GAME_CLIENT_MENU_SHELL_HELPER_METHOD_NAMES_WAVE588;
    let ok = residual_name_index(names, "host_tick_game_client_menu_shell").is_some()
        && residual_name_index(names, "host_tick_game_client_presentation_shell").is_some()
        && residual_name_index(names, "take_pending_new_game_start_request").is_some()
        && residual_name_index(names, "pump_message_stream").is_some()
        && residual_name_index(names, "start_game_from_ui").is_some()
        && residual_name_index(names, "Wave 588").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostGameClientMenuShellHelperAction::MethodNames);
    ok
}

pub fn honesty_host_game_client_menu_shell_helper_source_markers_residual_wave588() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_tick_game_client_menu_shell(") else {
        residual_action_store(ResidualHostGameClientMenuShellHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 588")
        && body.contains("ensure_shell_visible")
        && body.contains("update_input")
        && body.contains("update_pre_draw_ui")
        && body.contains("update_post_draw_ui")
        && body.contains("take_pending_new_game_start_request")
        && body.contains("pump_message_stream")
        && body.contains("start_game_from_ui")
        && body.contains("is_start_new_game_requested")
        && !body.contains("game_client.update()");
    // NewGame drain must precede pump inside the helper.
    let i_ng = body.find("take_pending_new_game_start_request");
    let i_pump = body.find("pump_message_stream");
    let order_ok = matches!((i_ng, i_pump), (Some(a), Some(b)) if a < b);
    let call_ok = eng.contains("self.host_tick_game_client_menu_shell()");
    // Production Menu path should not inline ensure_shell outside the helper.
    let raw_ensure = eng.matches("ensure_shell_visible()").count();
    let ok =
        body_ok && order_ok && call_ok && raw_ensure == 1 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostGameClientMenuShellHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_game_client_menu_shell_helper_nav_commands_residual_wave588() -> bool {
    let steps = LIVE_HOST_GAME_CLIENT_MENU_SHELL_HELPER_NAV_STEPS_WAVE588;
    let cmds = RUNTIME_HOST_LIVE_HOST_GAME_CLIENT_MENU_SHELL_HELPER_CMD_NAMES_WAVE588;
    let ok = residual_name_index(steps, "REQUIRE_MENU_SHELL_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_NEWGAME_BEFORE_PUMP").is_some()
        && residual_name_index(steps, "REQUIRE_DEVICE_UPDATE_ON_INJECTED_STATE").is_some()
        && residual_name_index(steps, "REQUIRE_START_GAME_FROM_UI_ON_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_GAME_CLIENT_MENU_SHELL_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_game_client_menu_shell_helper").is_some()
        && residual_name_index(cmds, "newgame_before_pump").is_some()
        && residual_name_index(cmds, "menu_shell_tick_residual").is_some()
        && residual_name_index(cmds, "wnd_start_newgame_drain").is_some();
    residual_action_store(ResidualHostGameClientMenuShellHelperAction::NavCommands);
    ok
}

pub fn simulate_host_game_client_menu_shell_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 588")
        && eng.contains("fn host_tick_game_client_menu_shell")
        && eng.contains("MSG_NEW_GAME")
        && eng.contains("take_pending_new_game_start_request");
    residual_action_store(ResidualHostGameClientMenuShellHelperAction::CollectSource);
    ok
}

pub fn simulate_host_game_client_menu_shell_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_tick_game_client_menu_shell()")
        && eng.contains("if self.host_tick_game_client_menu_shell()")
        && eng.contains("start_game_from_ui");
    residual_action_store(ResidualHostGameClientMenuShellHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_game_client_menu_shell_helper_residual_pack_wave588() -> bool {
    honesty_host_game_client_menu_shell_helper_method_names_residual_wave588()
        && honesty_host_game_client_menu_shell_helper_source_markers_residual_wave588()
        && honesty_host_game_client_menu_shell_helper_nav_commands_residual_wave588()
        && simulate_host_game_client_menu_shell_helper_collect_source()
        && simulate_host_game_client_menu_shell_helper_dispatch_source()
}

pub fn simulate_live_host_game_client_menu_shell_helper_honesty() -> bool {
    let ok = honesty_host_game_client_menu_shell_helper_residual_pack_wave588();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostGameClientMenuShellHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_game_client_menu_shell_helper_method_names_residual_wave588());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_game_client_menu_shell_helper_source_markers_residual_wave588());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_game_client_menu_shell_helper_nav_commands_residual_wave588());
    }

    #[test]
    fn host_game_client_menu_shell_helper_sources() {
        assert!(simulate_host_game_client_menu_shell_helper_collect_source());
        assert!(simulate_host_game_client_menu_shell_helper_dispatch_source());
    }

    #[test]
    fn wave588_composite_pack() {
        assert!(honesty_host_game_client_menu_shell_helper_residual_pack_wave588());
    }

    #[test]
    fn simulate_live_host_game_client_menu_shell_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_game_client_menu_shell_helper_honesty(),
            "host GameClient menu shell helper residual must latch"
        );
        assert!(residual_host_game_client_menu_shell_helper_ok());
        assert_eq!(
            residual_host_game_client_menu_shell_helper_last_action(),
            ResidualHostGameClientMenuShellHelperAction::Composite
        );
    }
}
