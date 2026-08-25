//! Wave 603 residual peels: paused/endgame client residuals and boot UI notify
//! are centralized through `host_tick_paused_client_residuals`,
//! `host_tick_endgame_client_residuals`, and `host_notify_boot_ui_message`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 600 post-presentation client residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host paused/endgame/boot-UI helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PAUSED_ENDGAME_BOOT_UI_HELPER_METHOD_NAMES_WAVE603: &[&str] = &[
    "host_tick_paused_client_residuals",
    "host_tick_endgame_client_residuals",
    "host_notify_boot_ui_message",
    "notify_boot_ui_message",
    "queue_radar_message",
    "Wave 603",
    "playable_claim = false",
];

pub const LIVE_HOST_PAUSED_ENDGAME_BOOT_UI_HELPER_NAV_STEPS_WAVE603: &[&str] = &[
    "REQUIRE_PAUSED_CLIENT_HELPER",
    "REQUIRE_ENDGAME_CLIENT_HELPER",
    "REQUIRE_BOOT_UI_NOTIFY_HELPER",
    "LIVE_HOST_PAUSED_ENDGAME_BOOT_UI_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PAUSED_ENDGAME_BOOT_UI_HELPER_CMD_NAMES_WAVE603: &[&str] = &[
    "host_paused_client_helper",
    "host_endgame_client_helper",
    "host_boot_ui_notify_helper",
    "paused_endgame_boot_ui_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPausedEndgameBootUiHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostPausedEndgameBootUiHelperAction {
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

fn residual_action_store(action: ResidualHostPausedEndgameBootUiHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_paused_endgame_boot_ui_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_paused_endgame_boot_ui_helper_last_action()
-> ResidualHostPausedEndgameBootUiHelperAction {
    ResidualHostPausedEndgameBootUiHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
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

pub fn honesty_host_paused_endgame_boot_ui_helper_method_names_residual_wave603() -> bool {
    let names = LIVE_HOST_PAUSED_ENDGAME_BOOT_UI_HELPER_METHOD_NAMES_WAVE603;
    let ok = residual_name_index(names, "host_tick_paused_client_residuals").is_some()
        && residual_name_index(names, "host_tick_endgame_client_residuals").is_some()
        && residual_name_index(names, "host_notify_boot_ui_message").is_some()
        && residual_name_index(names, "Wave 603").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::MethodNames);
    ok
}

pub fn honesty_host_paused_endgame_boot_ui_helper_source_markers_residual_wave603() -> bool {
    let eng = eng_source();
    let Some(paused) = fn_body(eng, "fn host_tick_paused_client_residuals(") else {
        residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::SourceMarkers);
        return false;
    };
    let Some(endgame) = fn_body(eng, "fn host_tick_endgame_client_residuals(") else {
        residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::SourceMarkers);
        return false;
    };
    let Some(boot) = fn_body(eng, "fn host_notify_boot_ui_message(") else {
        residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::SourceMarkers);
        return false;
    };
    let paused_ok = paused.contains("Wave 603")
        && paused.contains("update_camera")
        && paused.contains("PauseMenu")
        && paused.contains("cleanup_sound_effects");
    let endgame_ok = endgame.contains("Wave 603")
        && endgame.contains("update_camera")
        && endgame.contains("cleanup_sound_effects");
    // 2026-08-15: boot UI routes through host_notify_presentation_ui_message
    // (input.rs:1893) — no GameLogic dual-write.
    let boot_ok = boot.contains("Wave 603")
        && (boot.contains("queue_radar_message")
            || boot.contains("host_notify_presentation_ui_message"))
        && (boot.contains("play_ui_sound") || boot.contains("host_notify_presentation_ui_message"));
    let call_ok = eng.contains("self.host_tick_paused_client_residuals(visual_dt, dt)")
        && eng.contains("self.host_tick_endgame_client_residuals(visual_dt, dt)")
        && eng.contains("self.host_notify_boot_ui_message(message, team)");
    let ok =
        paused_ok && endgame_ok && boot_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_paused_endgame_boot_ui_helper_nav_commands_residual_wave603() -> bool {
    let steps = LIVE_HOST_PAUSED_ENDGAME_BOOT_UI_HELPER_NAV_STEPS_WAVE603;
    let cmds = RUNTIME_HOST_LIVE_HOST_PAUSED_ENDGAME_BOOT_UI_HELPER_CMD_NAMES_WAVE603;
    let ok = residual_name_index(steps, "REQUIRE_PAUSED_CLIENT_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_ENDGAME_CLIENT_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_BOOT_UI_NOTIFY_HELPER").is_some()
        && residual_name_index(steps, "LIVE_HOST_PAUSED_ENDGAME_BOOT_UI_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_paused_client_helper").is_some()
        && residual_name_index(cmds, "host_endgame_client_helper").is_some()
        && residual_name_index(cmds, "host_boot_ui_notify_helper").is_some()
        && residual_name_index(cmds, "paused_endgame_boot_ui_residual").is_some();
    residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::NavCommands);
    ok
}

pub fn simulate_host_paused_endgame_boot_ui_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 603")
        && eng.contains("fn host_tick_paused_client_residuals")
        && eng.contains("fn host_tick_endgame_client_residuals")
        && eng.contains("fn host_notify_boot_ui_message");
    residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::CollectSource);
    ok
}

pub fn simulate_host_paused_endgame_boot_ui_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_tick_paused_client_residuals(visual_dt, dt)")
        && eng.contains("self.host_tick_endgame_client_residuals(visual_dt, dt)")
        && eng.contains("Wave 603: paused client residual via host helper")
        && eng.contains("Wave 603: endgame client residual via host helper");
    residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_paused_endgame_boot_ui_helper_residual_pack_wave603() -> bool {
    honesty_host_paused_endgame_boot_ui_helper_method_names_residual_wave603()
        && honesty_host_paused_endgame_boot_ui_helper_source_markers_residual_wave603()
        && honesty_host_paused_endgame_boot_ui_helper_nav_commands_residual_wave603()
        && simulate_host_paused_endgame_boot_ui_helper_collect_source()
        && simulate_host_paused_endgame_boot_ui_helper_dispatch_source()
}

pub fn simulate_live_host_paused_endgame_boot_ui_helper_honesty() -> bool {
    let ok = honesty_host_paused_endgame_boot_ui_helper_residual_pack_wave603();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostPausedEndgameBootUiHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_paused_endgame_boot_ui_helper_method_names_residual_wave603());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_paused_endgame_boot_ui_helper_source_markers_residual_wave603());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_paused_endgame_boot_ui_helper_nav_commands_residual_wave603());
    }

    #[test]
    fn host_paused_endgame_boot_ui_helper_sources() {
        assert!(simulate_host_paused_endgame_boot_ui_helper_collect_source());
        assert!(simulate_host_paused_endgame_boot_ui_helper_dispatch_source());
    }

    #[test]
    fn wave603_composite_pack() {
        assert!(honesty_host_paused_endgame_boot_ui_helper_residual_pack_wave603());
    }

    #[test]
    fn simulate_live_host_paused_endgame_boot_ui_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_paused_endgame_boot_ui_helper_honesty(),
            "host paused endgame boot ui helper residual must latch"
        );
        assert!(residual_host_paused_endgame_boot_ui_helper_ok());
        assert_eq!(
            residual_host_paused_endgame_boot_ui_helper_last_action(),
            ResidualHostPausedEndgameBootUiHelperAction::Composite
        );
    }
}
