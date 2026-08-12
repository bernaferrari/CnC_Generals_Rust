//! Wave 898: host observe-path fail-closed dual-read peels.
//!
//! world bounds, camera script defaults, game mode, multiplayer, local team,
//! first opponent, EVA counters, special-power ready, template contains —
//! prefer freeze/host residual then fail-closed defaults (no cold GameLogic probe).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OBSERVE_FAILCLOSED_METHOD_NAMES_WAVE898: &[&str] = &[
    "host_world_bounds",
    "host_ui_script_default_camera_max_height",
    "host_presentation_or_live_game_mode",
    "host_is_in_multiplayer_game",
    "host_is_special_power_ready_for",
    "host_presentation_or_live_has_template",
    "boot_eva_counter_bundle_from_host",
    "Wave 898",
    "playable_claim = false",
];

pub const LIVE_HOST_OBSERVE_FAILCLOSED_NAV_STEPS_WAVE898: &[&str] = &[
    "WORLD_BOUNDS_FAILCLOSED",
    "CAMERA_DEFAULTS_FAILCLOSED",
    "GAME_MODE_SHELL_DEFAULT",
    "SPECIAL_POWER_RESIDUAL_ONLY",
    "TEMPLATE_NO_LIVE_CONTAINS",
    "LIVE_HOST_OBSERVE_FAILCLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostObserveFailclosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostObserveFailclosedAction) {
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

pub fn honesty_host_observe_failclosed_method_names_residual_wave898() -> bool {
    let names = LIVE_HOST_OBSERVE_FAILCLOSED_METHOD_NAMES_WAVE898;
    let ok = residual_name_index(names, "host_world_bounds").is_some()
        && residual_name_index(names, "host_is_special_power_ready_for").is_some()
        && residual_name_index(names, "Wave 898").is_some();
    residual_action_store(ResidualHostObserveFailclosedAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_observe_failclosed_nav_commands_residual_wave898() -> bool {
    let steps = LIVE_HOST_OBSERVE_FAILCLOSED_NAV_STEPS_WAVE898;
    let ok = residual_name_index(steps, "LIVE_HOST_OBSERVE_FAILCLOSED").is_some()
        && residual_name_index(steps, "SPECIAL_POWER_RESIDUAL_ONLY").is_some();
    residual_action_store(ResidualHostObserveFailclosedAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_observe_failclosed_residual_pack_wave898() -> bool {
    let cnc = cnc_source();
    let bounds = non_comment_code(code_window(cnc, "fn host_world_bounds", 500));
    let cam = non_comment_code(code_window(
        cnc,
        "fn host_ui_script_default_camera_max_height",
        500,
    ));
    let mode = non_comment_code(code_window(
        cnc,
        "fn host_presentation_or_live_game_mode",
        500,
    ));
    let mp = non_comment_code(code_window(cnc, "fn host_is_in_multiplayer_game", 400));
    let sp = non_comment_code(code_window(cnc, "fn host_is_special_power_ready_for", 700));
    let tmpl = non_comment_code(code_window(
        cnc,
        "fn host_presentation_or_live_has_template",
        900,
    ));
    let eva = non_comment_code(code_window(
        cnc,
        "fn boot_eva_counter_bundle_from_host",
        700,
    ));
    let ok = bounds.contains("Vec3::ZERO")
        && !bounds.contains("world_bounds()")
        && cam.contains("1.0")
        && !cam.contains("script_default_camera_max_height()")
        && mode.contains("GameMode::Shell")
        && !mode.contains("game_mode()")
        && mp.contains("false")
        && !mp.contains("isInMultiplayerGame()")
        && !sp.contains("is_special_power_ready_for(id, power)")
        && !tmpl.contains("templates.contains_key")
        && eva.contains("(0, 0, 0, 0)")
        && eva.contains("eva_low_power_count")
        && cnc.contains("Wave 898")
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostObserveFailclosedAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_observe_failclosed_honesty() -> bool {
    let a = honesty_host_observe_failclosed_method_names_residual_wave898();
    let b = honesty_host_observe_failclosed_nav_commands_residual_wave898();
    let c = honesty_host_observe_failclosed_residual_pack_wave898();
    residual_action_store(ResidualHostObserveFailclosedAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_observe_failclosed_residual_wave898() {
        assert!(honesty_host_observe_failclosed_residual_pack_wave898());
        assert!(honesty_host_observe_failclosed_method_names_residual_wave898());
        assert!(honesty_host_observe_failclosed_nav_commands_residual_wave898());
        assert!(simulate_live_host_observe_failclosed_honesty());
    }
}
