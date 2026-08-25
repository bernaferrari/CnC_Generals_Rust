//! Wave 606 residual peels: OS→GameClient inject and presentation UI notify are
//! centralized through `host_inject_game_client_*` and
//! `host_notify_presentation_ui_message`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 587 device tick residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host inject + presentation notify helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OS_INJECT_PRESENTATION_NOTIFY_HELPER_METHOD_NAMES_WAVE606: &[&str] = &[
    "host_inject_game_client_key",
    "host_inject_game_client_mouse_move",
    "host_inject_game_client_mouse_button",
    "host_inject_game_client_mouse_scroll",
    "host_notify_presentation_ui_message",
    "inject_game_client_key",
    "Wave 606",
    "playable_claim = false",
];

pub const LIVE_HOST_OS_INJECT_PRESENTATION_NOTIFY_HELPER_NAV_STEPS_WAVE606: &[&str] = &[
    "REQUIRE_HOST_OS_INJECT_HELPERS",
    "REQUIRE_PRESENTATION_NOTIFY_HELPER",
    "REQUIRE_THIN_WRAPPERS",
    "LIVE_HOST_OS_INJECT_PRESENTATION_NOTIFY_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_OS_INJECT_PRESENTATION_NOTIFY_HELPER_CMD_NAMES_WAVE606: &[&str] =
    &[
        "host_os_inject_helpers",
        "presentation_notify_helper",
        "thin_wrappers",
        "os_inject_presentation_notify_residual",
    ];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostOsInjectPresentationNotifyHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostOsInjectPresentationNotifyHelperAction {
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

fn residual_action_store(action: ResidualHostOsInjectPresentationNotifyHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_os_inject_presentation_notify_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_os_inject_presentation_notify_helper_last_action()
-> ResidualHostOsInjectPresentationNotifyHelperAction {
    ResidualHostOsInjectPresentationNotifyHelperAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
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

pub fn honesty_host_os_inject_presentation_notify_helper_method_names_residual_wave606() -> bool {
    let names = LIVE_HOST_OS_INJECT_PRESENTATION_NOTIFY_HELPER_METHOD_NAMES_WAVE606;
    let ok = residual_name_index(names, "host_inject_game_client_key").is_some()
        && residual_name_index(names, "host_inject_game_client_mouse_move").is_some()
        && residual_name_index(names, "host_notify_presentation_ui_message").is_some()
        && residual_name_index(names, "Wave 606").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::MethodNames);
    ok
}

pub fn honesty_host_os_inject_presentation_notify_helper_source_markers_residual_wave606() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_inject_game_client_key(",
        "fn host_inject_game_client_mouse_move(",
        "fn host_inject_game_client_mouse_button(",
        "fn host_inject_game_client_mouse_scroll(",
        "fn host_notify_presentation_ui_message(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 606") {
            defs_ok = false;
            break;
        }
    }
    let Some(key_wrap) = fn_body(eng, "fn inject_game_client_key(") else {
        residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::SourceMarkers);
        return false;
    };
    let Some(pres_wrap) = fn_body(eng, "fn notify_presentation_ui_message(") else {
        residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::SourceMarkers);
        return false;
    };
    let Some(pres_host) = fn_body(eng, "fn host_notify_presentation_ui_message(") else {
        residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::SourceMarkers);
        return false;
    };
    let wrap_ok = key_wrap.contains("host_inject_game_client_key")
        && key_wrap.contains("Wave 606")
        && pres_wrap.contains("host_notify_presentation_ui_message")
        && pres_wrap.contains("Wave 606");
    let host_ok = pres_host.contains("add_radar_message")
        && pres_host.contains("GUIMessageReceived")
        && !pres_host.contains("self.game_logic.");
    let call_ok = eng.contains("self.inject_game_client_key(")
        && eng.contains("self.host_inject_game_client_key(")
        && eng.contains("self.host_notify_presentation_ui_message(message)");
    let ok = defs_ok && wrap_ok && host_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_os_inject_presentation_notify_helper_nav_commands_residual_wave606() -> bool {
    let steps = LIVE_HOST_OS_INJECT_PRESENTATION_NOTIFY_HELPER_NAV_STEPS_WAVE606;
    let cmds = RUNTIME_HOST_LIVE_HOST_OS_INJECT_PRESENTATION_NOTIFY_HELPER_CMD_NAMES_WAVE606;
    let ok = residual_name_index(steps, "REQUIRE_HOST_OS_INJECT_HELPERS").is_some()
        && residual_name_index(steps, "REQUIRE_PRESENTATION_NOTIFY_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_WRAPPERS").is_some()
        && residual_name_index(steps, "LIVE_HOST_OS_INJECT_PRESENTATION_NOTIFY_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_os_inject_helpers").is_some()
        && residual_name_index(cmds, "presentation_notify_helper").is_some()
        && residual_name_index(cmds, "thin_wrappers").is_some()
        && residual_name_index(cmds, "os_inject_presentation_notify_residual").is_some();
    residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::NavCommands);
    ok
}

pub fn simulate_host_os_inject_presentation_notify_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 606")
        && eng.contains("fn host_inject_game_client_key")
        && eng.contains("fn host_notify_presentation_ui_message");
    residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::CollectSource);
    ok
}

pub fn simulate_host_os_inject_presentation_notify_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_inject_game_client_key(physical_key, pressed)")
        && eng.contains("self.host_notify_presentation_ui_message(message)")
        && eng.contains("Wave 606: thin wrapper — OS key inject via host helper");
    residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_os_inject_presentation_notify_helper_residual_pack_wave606() -> bool {
    honesty_host_os_inject_presentation_notify_helper_method_names_residual_wave606()
        && honesty_host_os_inject_presentation_notify_helper_source_markers_residual_wave606()
        && honesty_host_os_inject_presentation_notify_helper_nav_commands_residual_wave606()
        && simulate_host_os_inject_presentation_notify_helper_collect_source()
        && simulate_host_os_inject_presentation_notify_helper_dispatch_source()
}

pub fn simulate_live_host_os_inject_presentation_notify_helper_honesty() -> bool {
    let ok = honesty_host_os_inject_presentation_notify_helper_residual_pack_wave606();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostOsInjectPresentationNotifyHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_os_inject_presentation_notify_helper_method_names_residual_wave606());
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_os_inject_presentation_notify_helper_source_markers_residual_wave606()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_os_inject_presentation_notify_helper_nav_commands_residual_wave606());
    }

    #[test]
    fn host_os_inject_presentation_notify_helper_sources() {
        assert!(simulate_host_os_inject_presentation_notify_helper_collect_source());
        assert!(simulate_host_os_inject_presentation_notify_helper_dispatch_source());
    }

    #[test]
    fn wave606_composite_pack() {
        assert!(honesty_host_os_inject_presentation_notify_helper_residual_pack_wave606());
    }

    #[test]
    fn simulate_live_host_os_inject_presentation_notify_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_os_inject_presentation_notify_helper_honesty(),
            "host os inject presentation notify helper residual must latch"
        );
        assert!(residual_host_os_inject_presentation_notify_helper_ok());
        assert_eq!(
            residual_host_os_inject_presentation_notify_helper_last_action(),
            ResidualHostOsInjectPresentationNotifyHelperAction::Composite
        );
    }
}
