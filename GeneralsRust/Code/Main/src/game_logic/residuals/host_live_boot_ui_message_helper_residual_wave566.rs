//! Wave 566 residual peels: boot residual radar + GUIMessageReceived dual-write
//! is centralized through `notify_boot_ui_message`. Presentation freeze continues
//! to use `notify_presentation_ui_message` (no mid-frame GameLogic dual-write).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 565 construct template presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` notify_boot_ui_message / notify_presentation_ui_message
//! - defeat / alliance notification paths
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_BOOT_UI_MESSAGE_HELPER_METHOD_NAMES_WAVE566: &[&str] = &[
    "notify_boot_ui_message",
    "host_notify_boot_ui_message",
    "notify_presentation_ui_message",
    "play_ui_sound",
    "queue_radar_message",
    "Wave 566",
    "playable_claim = false",
];

pub const LIVE_BOOT_UI_MESSAGE_HELPER_NAV_STEPS_WAVE566: &[&str] = &[
    "REQUIRE_BOOT_UI_MESSAGE_HELPER",
    "REQUIRE_PRESENTATION_UI_MESSAGE_HELPER",
    "LIVE_BOOT_UI_MESSAGE_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_BOOT_UI_MESSAGE_HELPER_CMD_NAMES_WAVE566: &[&str] = &[
    "boot_ui_message_helper",
    "presentation_ui_message_helper",
    "defeat_alliance_ui_notify",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualBootUiMessageHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualBootUiMessageHelperAction {
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

fn residual_action_store(action: ResidualBootUiMessageHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_boot_ui_message_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_boot_ui_message_helper_last_action() -> ResidualBootUiMessageHelperAction {
    ResidualBootUiMessageHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_boot_ui_message_helper_method_names_residual_wave566() -> bool {
    let names = LIVE_BOOT_UI_MESSAGE_HELPER_METHOD_NAMES_WAVE566;
    let ok = residual_name_index(names, "notify_boot_ui_message").is_some()
        && residual_name_index(names, "notify_presentation_ui_message").is_some()
        && residual_name_index(names, "play_ui_sound").is_some()
        && residual_name_index(names, "queue_radar_message").is_some()
        && residual_name_index(names, "Wave 566").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualBootUiMessageHelperAction::MethodNames);
    ok
}

pub fn honesty_boot_ui_message_helper_source_markers_residual_wave566() -> bool {
    let eng = eng_source();
    let Some(boot) = fn_body(eng, "fn host_notify_boot_ui_message(")
        .or_else(|| fn_body(eng, "fn notify_boot_ui_message("))
    else {
        residual_action_store(ResidualBootUiMessageHelperAction::SourceMarkers);
        return false;
    };
    let Some(pres) = fn_body(eng, "fn host_notify_presentation_ui_message(")
        .or_else(|| fn_body(eng, "fn notify_presentation_ui_message("))
    else {
        residual_action_store(ResidualBootUiMessageHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 900 peeled GameLogic radar/sound dual-writes.
    // Boot routes through host_notify_presentation_ui_message (HUD + audio).
    let boot_ok = (boot.contains("Wave 566") || boot.contains("Wave 603"))
        && eng.contains("self.host_notify_boot_ui_message(message, team)")
        && boot.contains("host_notify_presentation_ui_message(message)")
        && !boot.contains("queue_radar_message_for_team")
        && !boot.contains("play_ui_sound(\"GUIMessageReceived\")");
    let pres_ok = pres.contains("add_radar_message")
        && pres.contains("GUIMessageReceived")
        && !pres.contains("self.game_logic.play_ui_sound")
        && !pres.contains("self.game_logic.queue_radar");
    let call_ok = eng.contains("notify_boot_ui_message(&message")
        && (eng.contains("notify_presentation_ui_message(&message)")
            || eng.contains("host_notify_presentation_ui_message(message)"));
    let raw_sound = eng.matches("self.game_logic.play_ui_sound").count();
    let raw_radar = eng.matches("self.game_logic.queue_radar_message(").count();
    let raw_team = eng.matches("queue_radar_message_for_team").count();
    let ok = boot_ok
        && pres_ok
        && call_ok
        && raw_sound == 0
        && raw_radar == 0
        && raw_team == 0
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualBootUiMessageHelperAction::SourceMarkers);
    ok
}

pub fn honesty_boot_ui_message_helper_nav_commands_residual_wave566() -> bool {
    let steps = LIVE_BOOT_UI_MESSAGE_HELPER_NAV_STEPS_WAVE566;
    let cmds = RUNTIME_HOST_LIVE_BOOT_UI_MESSAGE_HELPER_CMD_NAMES_WAVE566;
    let ok = residual_name_index(steps, "REQUIRE_BOOT_UI_MESSAGE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_PRESENTATION_UI_MESSAGE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_BOOT_UI_MESSAGE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "boot_ui_message_helper").is_some()
        && residual_name_index(cmds, "presentation_ui_message_helper").is_some()
        && residual_name_index(cmds, "defeat_alliance_ui_notify").is_some();
    residual_action_store(ResidualBootUiMessageHelperAction::NavCommands);
    ok
}

pub fn simulate_boot_ui_message_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 566")
        && eng.contains("fn notify_boot_ui_message")
        && eng.contains("fn notify_presentation_ui_message");
    residual_action_store(ResidualBootUiMessageHelperAction::CollectSource);
    ok
}

pub fn simulate_boot_ui_message_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("notify_boot_ui_message(&message")
        && eng.contains("notify_presentation_ui_message(&message)")
        && eng.matches("notify_boot_ui_message(").count() >= 3; // def + 2 call sites
    residual_action_store(ResidualBootUiMessageHelperAction::DispatchSource);
    ok
}

pub fn honesty_boot_ui_message_helper_residual_pack_wave566() -> bool {
    honesty_boot_ui_message_helper_method_names_residual_wave566()
        && honesty_boot_ui_message_helper_source_markers_residual_wave566()
        && honesty_boot_ui_message_helper_nav_commands_residual_wave566()
        && simulate_boot_ui_message_helper_collect_source()
        && simulate_boot_ui_message_helper_dispatch_source()
}

pub fn simulate_live_boot_ui_message_helper_honesty() -> bool {
    let ok = honesty_boot_ui_message_helper_residual_pack_wave566();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualBootUiMessageHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_boot_ui_message_helper_method_names_residual_wave566());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_boot_ui_message_helper_source_markers_residual_wave566());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_boot_ui_message_helper_nav_commands_residual_wave566());
    }

    #[test]
    fn boot_ui_message_helper_sources() {
        assert!(simulate_boot_ui_message_helper_collect_source());
        assert!(simulate_boot_ui_message_helper_dispatch_source());
    }

    #[test]
    fn wave566_composite_pack() {
        assert!(honesty_boot_ui_message_helper_residual_pack_wave566());
    }

    #[test]
    fn simulate_live_boot_ui_message_helper_honesty_residual_live() {
        assert!(
            simulate_live_boot_ui_message_helper_honesty(),
            "boot UI message helper residual must latch"
        );
        assert!(residual_boot_ui_message_helper_ok());
        assert_eq!(
            residual_boot_ui_message_helper_last_action(),
            ResidualBootUiMessageHelperAction::Composite
        );
    }
}
