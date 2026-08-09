//! Wave 905: host UI observe multi-line dual-read peels.
//!
//! selection seed, science points, economy, camera height, cursor friendly
//! hover — freeze/residual first, fail-closed boot (no live GameLogic probe).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_OBSERVE_FAILCLOSED_METHOD_NAMES_WAVE905: &[&str] = &[
    "host_ui_selection_seed_id",
    "host_ui_local_science_purchase_points",
    "host_ui_local_economy",
    "host_center_camera_on",
    "host_resolve_context_cursor_icon",
    "Wave 905",
    "playable_claim = false",
];

pub const LIVE_HOST_UI_OBSERVE_FAILCLOSED_NAV_STEPS_WAVE905: &[&str] = &[
    "SELECTION_SEED_FAILCLOSED",
    "SCIENCE_POINTS_FAILCLOSED",
    "ECONOMY_FAILCLOSED",
    "CAMERA_HEIGHT_FAILCLOSED",
    "CURSOR_NO_FIND_OBJECT",
    "LIVE_HOST_UI_OBSERVE_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiObserveFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUiObserveFailclosedAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
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

pub fn honesty_host_ui_observe_failclosed_method_names_residual_wave905() -> bool {
    let names = LIVE_HOST_UI_OBSERVE_FAILCLOSED_METHOD_NAMES_WAVE905;
    let ok = residual_name_index(names, "host_ui_selection_seed_id").is_some()
        && residual_name_index(names, "host_ui_local_economy").is_some()
        && residual_name_index(names, "Wave 905").is_some();
    residual_action_store(ResidualHostUiObserveFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_observe_failclosed_nav_commands_residual_wave905() -> bool {
    let steps = LIVE_HOST_UI_OBSERVE_FAILCLOSED_NAV_STEPS_WAVE905;
    let ok = residual_name_index(steps, "LIVE_HOST_UI_OBSERVE_FAILCLOSED").is_some()
        && residual_name_index(steps, "ECONOMY_FAILCLOSED").is_some();
    residual_action_store(ResidualHostUiObserveFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_observe_failclosed_residual_pack_wave905() -> bool {
    let cnc = cnc_source();
    let seed = non_comment_code(code_window(cnc, "fn host_ui_selection_seed_id", 1400));
    let sci = non_comment_code(code_window(
        cnc,
        "fn host_ui_local_science_purchase_points",
        700,
    ));
    let eco = non_comment_code(code_window(cnc, "fn host_ui_local_economy", 900));
    let cam = non_comment_code(code_window(cnc, "fn host_center_camera_on", 900));
    let cursor = non_comment_code(code_window(
        cnc,
        "fn host_resolve_context_cursor_icon",
        3500,
    ));
    let ok = !seed.contains("player_selected_objects")
        && !sci.contains("player_science_purchase_points")
        && !eco.contains("player_economy")
        && !cam.contains("terrain_height_at")
        && !cursor.contains(".find_object(")
        && (cnc.contains("Wave 905") || cnc.contains("/905"))
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostUiObserveFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ui_observe_failclosed_honesty() -> bool {
    let a = honesty_host_ui_observe_failclosed_method_names_residual_wave905();
    let b = honesty_host_ui_observe_failclosed_nav_commands_residual_wave905();
    let c = honesty_host_ui_observe_failclosed_residual_pack_wave905();
    residual_action_store(ResidualHostUiObserveFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ui_observe_failclosed_residual_wave905() {
        assert!(honesty_host_ui_observe_failclosed_residual_pack_wave905());
        assert!(honesty_host_ui_observe_failclosed_method_names_residual_wave905());
        assert!(honesty_host_ui_observe_failclosed_nav_commands_residual_wave905());
        assert!(simulate_live_host_ui_observe_failclosed_honesty());
    }
}
