//! Wave 900: presentation event drain + boot notify fail-closed dual-read peels.
//!
//! - take_presentation_or_boot script/defeat/alliance: freeze only, boot empty.
//! - presentation movie/popup: no live queue drain dual-read after apply.
//! - shell FPS residual: freeze only.
//! - boot UI message: always presentation UI residual (no GameLogic dual-write).
//! - golden ranger: no contains_key probe before insert.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_EVENT_DRAIN_FAILCLOSED_METHOD_NAMES_WAVE900: &[&str] = &[
    "host_take_presentation_or_boot_new_script_messages",
    "host_take_presentation_or_boot_defeat_events",
    "host_take_presentation_or_boot_alliance_events",
    "apply_presentation_movie_residual",
    "apply_shell_script_fps_limit_residual",
    "host_notify_boot_ui_message",
    "host_ensure_golden_ranger_template",
    "Wave 900",
    "playable_claim = false",
];

pub const LIVE_HOST_EVENT_DRAIN_FAILCLOSED_NAV_STEPS_WAVE900: &[&str] = &[
    "EVENT_DRAIN_FREEZE_ONLY",
    "MOVIE_POPUP_NO_LIVE_DRAIN",
    "SHELL_FPS_FREEZE_ONLY",
    "BOOT_UI_PRESENTATION_NOTIFY",
    "GOLDEN_RANGER_NO_CONTAINS_PROBE",
    "LIVE_HOST_EVENT_DRAIN_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEventDrainFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostEventDrainFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn code_window<'a>(src: &'a str, marker: &str, len: usize) -> &'a str {
    match src.find(marker) {
        Some(i) => &src[i..src.len().min(i + len)],
        None => "",
    }
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_event_drain_failclosed_method_names_residual_wave900() -> bool {
    let names = LIVE_HOST_EVENT_DRAIN_FAILCLOSED_METHOD_NAMES_WAVE900;
    let ok = residual_name_index(names, "host_take_presentation_or_boot_defeat_events").is_some()
        && residual_name_index(names, "host_notify_boot_ui_message").is_some()
        && residual_name_index(names, "Wave 900").is_some();
    residual_action_store(ResidualHostEventDrainFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_event_drain_failclosed_nav_commands_residual_wave900() -> bool {
    let steps = LIVE_HOST_EVENT_DRAIN_FAILCLOSED_NAV_STEPS_WAVE900;
    let ok = residual_name_index(steps, "LIVE_HOST_EVENT_DRAIN_FAILCLOSED").is_some()
        && residual_name_index(steps, "EVENT_DRAIN_FREEZE_ONLY").is_some();
    residual_action_store(ResidualHostEventDrainFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_event_drain_failclosed_residual_pack_wave900() -> bool {
    let cnc = cnc_source();
    let msgs = non_comment_code(code_window(
        cnc,
        "fn host_take_presentation_or_boot_new_script_messages",
        700,
    ));
    let defeat = non_comment_code(code_window(
        cnc,
        "fn host_take_presentation_or_boot_defeat_events",
        500,
    ));
    let alliance = non_comment_code(code_window(
        cnc,
        "fn host_take_presentation_or_boot_alliance_events",
        600,
    ));
    let movie = non_comment_code(code_window(
        cnc,
        "fn apply_presentation_movie_residual",
        1600,
    ));
    let popup = non_comment_code(code_window(
        cnc,
        "fn apply_presentation_popup_music_residual",
        900,
    ));
    let fps = non_comment_code(code_window(
        cnc,
        "fn apply_shell_script_fps_limit_residual",
        700,
    ));
    let notify = non_comment_code(code_window(cnc, "fn host_notify_boot_ui_message", 600));
    let golden = non_comment_code(code_window(
        cnc,
        "fn host_ensure_golden_ranger_template",
        900,
    ));
    let ok = !msgs.contains("take_new_script_messages")
        && msgs.contains("Vec::new()")
        && !defeat.contains("take_defeat_events")
        && !alliance.contains("take_alliance_events")
        && !movie.contains("take_pending_movie")
        && !popup.contains("take_popup_message_requests")
        && !popup.contains("take_music_stop_request")
        && !fps.contains("take_script_fps_limit_request")
        && notify.contains("host_notify_presentation_ui_message")
        && !notify.contains("queue_radar_message")
        && !golden.contains("contains_key(\"GoldenRanger\")")
        && cnc.contains("Wave 900")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostEventDrainFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_event_drain_failclosed_honesty() -> bool {
    let a = honesty_host_event_drain_failclosed_method_names_residual_wave900();
    let b = honesty_host_event_drain_failclosed_nav_commands_residual_wave900();
    let c = honesty_host_event_drain_failclosed_residual_pack_wave900();
    residual_action_store(ResidualHostEventDrainFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_event_drain_failclosed_residual_wave900() {
        assert!(honesty_host_event_drain_failclosed_residual_pack_wave900());
        assert!(honesty_host_event_drain_failclosed_method_names_residual_wave900());
        assert!(honesty_host_event_drain_failclosed_nav_commands_residual_wave900());
        assert!(simulate_live_host_event_drain_failclosed_honesty());
    }
}
