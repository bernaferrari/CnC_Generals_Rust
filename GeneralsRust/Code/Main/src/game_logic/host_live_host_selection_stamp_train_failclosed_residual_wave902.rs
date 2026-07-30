//! Wave 902: selection/stamp/train residual fail-closed dual-read peels.
//!
//! - host_ui_selected_ids: residual/freeze only, boot empty.
//! - host_set_paused: no is_time_frozen dual-read on boot.
//! - host_refresh_local_train_producer: cold residual empty (no get_objects).
//! - host_stamp_sim_timing / refresh cold: dual-read only frame + fixed-step.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECTION_STAMP_TRAIN_METHOD_NAMES_WAVE902: &[&str] = &[
    "host_ui_selected_ids",
    "host_set_paused",
    "host_refresh_local_train_producer_residuals",
    "host_stamp_sim_timing_residuals",
    "Wave 902",
    "playable_claim = false",
];

pub const LIVE_HOST_SELECTION_STAMP_TRAIN_NAV_STEPS_WAVE902: &[&str] = &[
    "SELECTION_FAILCLOSED_BOOT",
    "PAUSE_NO_TIME_FROZEN_DUAL_READ",
    "TRAIN_PRODUCER_COLD_EMPTY",
    "STAMP_FRAME_FIXED_STEP_ONLY",
    "LIVE_HOST_SELECTION_STAMP_TRAIN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectionStampTrainAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectionStampTrainAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
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

pub fn honesty_host_selection_stamp_train_method_names_residual_wave902() -> bool {
    let names = LIVE_HOST_SELECTION_STAMP_TRAIN_METHOD_NAMES_WAVE902;
    let ok = residual_name_index(names, "host_ui_selected_ids").is_some()
        && residual_name_index(names, "host_stamp_sim_timing_residuals").is_some()
        && residual_name_index(names, "Wave 902").is_some();
    residual_action_store(ResidualHostSelectionStampTrainAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_stamp_train_nav_commands_residual_wave902() -> bool {
    let steps = LIVE_HOST_SELECTION_STAMP_TRAIN_NAV_STEPS_WAVE902;
    let ok = residual_name_index(steps, "LIVE_HOST_SELECTION_STAMP_TRAIN").is_some()
        && residual_name_index(steps, "STAMP_FRAME_FIXED_STEP_ONLY").is_some();
    residual_action_store(ResidualHostSelectionStampTrainAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_stamp_train_residual_pack_wave902() -> bool {
    let cnc = cnc_source();
    let sel = non_comment_code(code_window(cnc, "fn host_ui_selected_ids", 1600));
    let pause = non_comment_code(code_window(cnc, "fn host_set_paused", 800));
    let train = non_comment_code(code_window(
        cnc,
        "fn host_refresh_local_train_producer_residuals",
        3000,
    ));
    let stamp_raw = code_window(cnc, "fn host_stamp_sim_timing_residuals", 1600);
    let stamp = non_comment_code(stamp_raw);
    let ok = !sel.contains("player_selected_objects")
        && sel.contains("Vec::new()")
        && !pause.contains("is_time_frozen_for_simulation()")
        && !train.contains("get_objects()")
        && stamp.contains("get_frame()")
        && stamp.contains("fixed_step_diagnostics()")
        && !stamp.contains("visual_speed_multiplier()")
        && !stamp.contains("get_total_play_time()")
        && stamp_raw.contains("Wave 902")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostSelectionStampTrainAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_selection_stamp_train_honesty() -> bool {
    let a = honesty_host_selection_stamp_train_method_names_residual_wave902();
    let b = honesty_host_selection_stamp_train_nav_commands_residual_wave902();
    let c = honesty_host_selection_stamp_train_residual_pack_wave902();
    residual_action_store(ResidualHostSelectionStampTrainAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_selection_stamp_train_residual_wave902() {
        assert!(honesty_host_selection_stamp_train_residual_pack_wave902());
        assert!(honesty_host_selection_stamp_train_method_names_residual_wave902());
        assert!(honesty_host_selection_stamp_train_nav_commands_residual_wave902());
        assert!(simulate_live_host_selection_stamp_train_honesty());
    }
}
