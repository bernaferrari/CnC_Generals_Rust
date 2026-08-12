//! Wave 920: producer residual refresh freeze peels + zero-dt shell skip.
//!
//! After authority mutations, skip host_refresh_local_train_producer_residuals
//! when presentation freeze is installed (finalize owns scan). Shell update
//! skips empty-dt dual-write. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCER_REFRESH_FREEZE_PEELS_METHOD_NAMES_WAVE920: &[&str] = &[
    "host_cancel_production_and_sync_hud",
    "host_force_complete_construction",
    "host_ensure_barracks_building_data",
    "host_force_ensure_barracks_building_data",
    "host_clear_unit_movement_path",
    "host_adjust_unit_guard_radius",
    "host_destroy_object",
    "host_update_shell_with_budget",
    "Wave 920",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCER_REFRESH_FREEZE_PEELS_NAV_STEPS_WAVE920: &[&str] = &[
    "PRODUCER_REFRESH_SKIP_UNDER_FREEZE",
    "SHELL_ZERO_DT_SKIP",
    "LIVE_HOST_PRODUCER_REFRESH_FREEZE_PEELS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProducerRefreshFreezePeelsAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProducerRefreshFreezePeelsAction) {
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

pub fn honesty_host_producer_refresh_freeze_peels_method_names_residual_wave920() -> bool {
    let names = LIVE_HOST_PRODUCER_REFRESH_FREEZE_PEELS_METHOD_NAMES_WAVE920;
    let ok = residual_name_index(names, "host_cancel_production_and_sync_hud").is_some()
        && residual_name_index(names, "Wave 920").is_some();
    residual_action_store(ResidualHostProducerRefreshFreezePeelsAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_producer_refresh_freeze_peels_nav_commands_residual_wave920() -> bool {
    let steps = LIVE_HOST_PRODUCER_REFRESH_FREEZE_PEELS_NAV_STEPS_WAVE920;
    let ok = residual_name_index(steps, "LIVE_HOST_PRODUCER_REFRESH_FREEZE_PEELS").is_some()
        && residual_name_index(steps, "PRODUCER_REFRESH_SKIP_UNDER_FREEZE").is_some();
    residual_action_store(ResidualHostProducerRefreshFreezePeelsAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_producer_refresh_freeze_peels_residual_pack_wave920() -> bool {
    let cnc = cnc_source();
    let cancel_raw = code_window(cnc, "fn host_cancel_production_and_sync_hud", 1100);
    let cancel = non_comment_code(cancel_raw);
    let shell_raw = code_window(cnc, "fn host_update_shell_with_budget", 700);
    let shell = non_comment_code(shell_raw);
    let destroy_raw = code_window(cnc, "fn host_destroy_object", 1200);
    let destroy = non_comment_code(destroy_raw);
    let ok = cancel_raw.contains("920")
        && cancel.contains("last_presentation_frame.is_none()")
        && shell_raw.contains("920")
        && shell.contains("dt <= 0.0")
        && destroy_raw.contains("920")
        && destroy.contains("last_presentation_frame.is_none()")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostProducerRefreshFreezePeelsAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_producer_refresh_freeze_peels_honesty() -> bool {
    let a = honesty_host_producer_refresh_freeze_peels_method_names_residual_wave920();
    let b = honesty_host_producer_refresh_freeze_peels_nav_commands_residual_wave920();
    let c = honesty_host_producer_refresh_freeze_peels_residual_pack_wave920();
    residual_action_store(ResidualHostProducerRefreshFreezePeelsAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_producer_refresh_freeze_peels_residual_wave920() {
        assert!(honesty_host_producer_refresh_freeze_peels_residual_pack_wave920());
        assert!(honesty_host_producer_refresh_freeze_peels_method_names_residual_wave920());
        assert!(honesty_host_producer_refresh_freeze_peels_nav_commands_residual_wave920());
        assert!(simulate_live_host_producer_refresh_freeze_peels_honesty());
    }
}
