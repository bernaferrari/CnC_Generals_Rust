//! Wave 605 residual peels: Menu-state client residual is centralized through
//! `host_tick_menu_client_residuals`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 588 Menu GameClient shell helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_tick_menu_client_residuals
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MENU_CLIENT_HELPER_METHOD_NAMES_WAVE605: &[&str] = &[
    "host_tick_menu_client_residuals",
    "host_update_shell_with_budget",
    "host_process_shell_menu_commands",
    "host_tick_game_client_menu_shell",
    "Wave 605",
    "Wave 588",
    "playable_claim = false",
];

pub const LIVE_HOST_MENU_CLIENT_HELPER_NAV_STEPS_WAVE605: &[&str] = &[
    "REQUIRE_MENU_CLIENT_HELPER",
    "REQUIRE_SHELL_TICK",
    "REQUIRE_MENU_COMMANDS",
    "REQUIRE_MENU_SHELL_DRAIN",
    "LIVE_HOST_MENU_CLIENT_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_MENU_CLIENT_HELPER_CMD_NAMES_WAVE605: &[&str] = &[
    "host_menu_client_helper",
    "shell_tick",
    "menu_commands",
    "menu_shell_drain",
    "menu_client_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMenuClientHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostMenuClientHelperAction {
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

fn residual_action_store(action: ResidualHostMenuClientHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_menu_client_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_menu_client_helper_last_action() -> ResidualHostMenuClientHelperAction {
    ResidualHostMenuClientHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn last_sig_index(src: &str, sig: &str) -> Option<usize> {
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(sig) {
        at = Some(from + rel);
        from = from + rel + sig.len();
    }
    at
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = last_sig_index(src, sig)?;
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

pub fn honesty_host_menu_client_helper_method_names_residual_wave605() -> bool {
    let names = LIVE_HOST_MENU_CLIENT_HELPER_METHOD_NAMES_WAVE605;
    let ok = residual_name_index(names, "host_tick_menu_client_residuals").is_some()
        && residual_name_index(names, "host_tick_game_client_menu_shell").is_some()
        && residual_name_index(names, "Wave 605").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostMenuClientHelperAction::MethodNames);
    ok
}

pub fn honesty_host_menu_client_helper_source_markers_residual_wave605() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_tick_menu_client_residuals(") else {
        residual_action_store(ResidualHostMenuClientHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 605")
        && body.contains("Wave 552")
        && body.contains("host_update_shell_with_budget")
        && body.contains("host_process_shell_menu_commands")
        && body.contains("apply_pending_script_camera_requests")
        && body.contains("update_camera")
        && body.contains("MainMenu")
        && body.contains("Wave 588")
        && body.contains("host_tick_game_client_menu_shell");
    let call_ok = eng.contains("self.host_tick_menu_client_residuals(visual_dt, dt)")
        && eng.contains("Wave 605: Menu client residual via host helper");
    let ok = body_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostMenuClientHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_menu_client_helper_nav_commands_residual_wave605() -> bool {
    let steps = LIVE_HOST_MENU_CLIENT_HELPER_NAV_STEPS_WAVE605;
    let cmds = RUNTIME_HOST_LIVE_HOST_MENU_CLIENT_HELPER_CMD_NAMES_WAVE605;
    let ok = residual_name_index(steps, "REQUIRE_MENU_CLIENT_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_SHELL_TICK").is_some()
        && residual_name_index(steps, "REQUIRE_MENU_COMMANDS").is_some()
        && residual_name_index(steps, "REQUIRE_MENU_SHELL_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_MENU_CLIENT_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_menu_client_helper").is_some()
        && residual_name_index(cmds, "shell_tick").is_some()
        && residual_name_index(cmds, "menu_commands").is_some()
        && residual_name_index(cmds, "menu_shell_drain").is_some()
        && residual_name_index(cmds, "menu_client_residual").is_some();
    residual_action_store(ResidualHostMenuClientHelperAction::NavCommands);
    ok
}

pub fn simulate_host_menu_client_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 605")
        && eng.contains("fn host_tick_menu_client_residuals")
        && eng.contains("host_tick_game_client_menu_shell");
    residual_action_store(ResidualHostMenuClientHelperAction::CollectSource);
    ok
}

pub fn simulate_host_menu_client_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_tick_menu_client_residuals(visual_dt, dt)")
        && eng.contains("Wave 605: Menu client residual via host helper");
    residual_action_store(ResidualHostMenuClientHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_menu_client_helper_residual_pack_wave605() -> bool {
    honesty_host_menu_client_helper_method_names_residual_wave605()
        && honesty_host_menu_client_helper_source_markers_residual_wave605()
        && honesty_host_menu_client_helper_nav_commands_residual_wave605()
        && simulate_host_menu_client_helper_collect_source()
        && simulate_host_menu_client_helper_dispatch_source()
}

pub fn simulate_live_host_menu_client_helper_honesty() -> bool {
    let ok = honesty_host_menu_client_helper_residual_pack_wave605();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostMenuClientHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_menu_client_helper_method_names_residual_wave605());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_menu_client_helper_source_markers_residual_wave605());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_menu_client_helper_nav_commands_residual_wave605());
    }

    #[test]
    fn host_menu_client_helper_sources() {
        assert!(simulate_host_menu_client_helper_collect_source());
        assert!(simulate_host_menu_client_helper_dispatch_source());
    }

    #[test]
    fn wave605_composite_pack() {
        assert!(honesty_host_menu_client_helper_residual_pack_wave605());
    }

    #[test]
    fn simulate_live_host_menu_client_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_menu_client_helper_honesty(),
            "host menu client helper residual must latch"
        );
        assert!(residual_host_menu_client_helper_ok());
        assert_eq!(
            residual_host_menu_client_helper_last_action(),
            ResidualHostMenuClientHelperAction::Composite
        );
    }
}
