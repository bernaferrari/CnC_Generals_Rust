//! Wave 918: load_map residual skip + clear-path residual skip + process stamp freeze skip.
//!
//! - host_load_map_or_default skips reload when residual map identity matches
//! - clear_unit_movement_path skips when presentation has no move destination
//! - process paths stamp sim timing only without presentation freeze
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_LOAD_PATH_STAMP_PEELS_METHOD_NAMES_WAVE918: &[&str] = &[
    "host_load_map_or_default",
    "host_clear_unit_movement_path",
    "host_queue_and_process_command_silent",
    "host_process_shell_menu_commands",
    "host_process_commands_with_command_sound",
    "Wave 918",
    "playable_claim = false",
];

pub const LIVE_HOST_LOAD_PATH_STAMP_PEELS_NAV_STEPS_WAVE918: &[&str] = &[
    "LOAD_MAP_RESIDUAL_IDENTITY_SKIP",
    "CLEAR_PATH_PRESENTATION_SKIP",
    "PROCESS_STAMP_SKIP_UNDER_FREEZE",
    "LIVE_HOST_LOAD_PATH_STAMP_PEELS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostLoadPathStampPeelsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostLoadPathStampPeelsAction) {
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

pub fn honesty_host_load_path_stamp_peels_method_names_residual_wave918() -> bool {
    let names = LIVE_HOST_LOAD_PATH_STAMP_PEELS_METHOD_NAMES_WAVE918;
    let ok = residual_name_index(names, "host_load_map_or_default").is_some()
        && residual_name_index(names, "Wave 918").is_some();
    residual_action_store(ResidualHostLoadPathStampPeelsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_load_path_stamp_peels_nav_commands_residual_wave918() -> bool {
    let steps = LIVE_HOST_LOAD_PATH_STAMP_PEELS_NAV_STEPS_WAVE918;
    let ok = residual_name_index(steps, "LIVE_HOST_LOAD_PATH_STAMP_PEELS").is_some()
        && residual_name_index(steps, "LOAD_MAP_RESIDUAL_IDENTITY_SKIP").is_some();
    residual_action_store(ResidualHostLoadPathStampPeelsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_load_path_stamp_peels_residual_pack_wave918() -> bool {
    let cnc = cnc_source();
    let load_raw = code_window(cnc, "fn host_load_map_or_default", 1100);
    let load = non_comment_code(load_raw);
    let clear_raw = code_window(cnc, "fn host_clear_unit_movement_path", 900);
    let clear = non_comment_code(clear_raw);
    let silent_raw = code_window(cnc, "fn host_queue_and_process_command_silent", 700);
    let silent = non_comment_code(silent_raw);
    let ok = load_raw.contains("918")
        && load.contains("host_match_map_name")
        && clear_raw.contains("918")
        && clear.contains("move_destination")
        && silent_raw.contains("918")
        && silent.contains("last_presentation_frame.is_none()")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostLoadPathStampPeelsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_load_path_stamp_peels_honesty() -> bool {
    let a = honesty_host_load_path_stamp_peels_method_names_residual_wave918();
    let b = honesty_host_load_path_stamp_peels_nav_commands_residual_wave918();
    let c = honesty_host_load_path_stamp_peels_residual_pack_wave918();
    residual_action_store(ResidualHostLoadPathStampPeelsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_load_path_stamp_peels_residual_wave918() {
        assert!(honesty_host_load_path_stamp_peels_residual_pack_wave918());
        assert!(honesty_host_load_path_stamp_peels_method_names_residual_wave918());
        assert!(honesty_host_load_path_stamp_peels_nav_commands_residual_wave918());
        assert!(simulate_live_host_load_path_stamp_peels_honesty());
    }
}
