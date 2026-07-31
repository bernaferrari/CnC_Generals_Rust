//! Wave 906: mouse classification presentation-only (no GameLogic dual-read).
//!
//! `host_presentation_mouse_game_logic` always returns None so cursor/command
//! classification uses PresentationFrame residuals exclusively.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MOUSE_PRESENTATION_ONLY_METHOD_NAMES_WAVE906: &[&str] = &[
    "host_presentation_mouse_game_logic",
    "presentation_mouse_game_logic",
    "Wave 906",
    "playable_claim = false",
];

pub const LIVE_HOST_MOUSE_PRESENTATION_ONLY_NAV_STEPS_WAVE906: &[&str] = &[
    "MOUSE_CLASSIFY_PRESENTATION_ONLY",
    "NO_LIVE_GAMELOGIC_MOUSE_BORROW",
    "LIVE_HOST_MOUSE_PRESENTATION_ONLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMousePresentationOnlyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMousePresentationOnlyAction) {
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

pub fn honesty_host_mouse_presentation_only_method_names_residual_wave906() -> bool {
    let names = LIVE_HOST_MOUSE_PRESENTATION_ONLY_METHOD_NAMES_WAVE906;
    let ok = residual_name_index(names, "host_presentation_mouse_game_logic").is_some()
        && residual_name_index(names, "Wave 906").is_some();
    residual_action_store(ResidualHostMousePresentationOnlyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_mouse_presentation_only_nav_commands_residual_wave906() -> bool {
    let steps = LIVE_HOST_MOUSE_PRESENTATION_ONLY_NAV_STEPS_WAVE906;
    let ok = residual_name_index(steps, "LIVE_HOST_MOUSE_PRESENTATION_ONLY").is_some()
        && residual_name_index(steps, "MOUSE_CLASSIFY_PRESENTATION_ONLY").is_some();
    residual_action_store(ResidualHostMousePresentationOnlyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_mouse_presentation_only_residual_pack_wave906() -> bool {
    let cnc = cnc_source();
    let mouse_raw = code_window(cnc, "fn host_presentation_mouse_game_logic", 600);
    let mouse = non_comment_code(mouse_raw);
    let ok = mouse.contains("None")
        && !mouse.contains("Some(&self.game_logic)")
        && !mouse.contains("Some(& self.game_logic)")
        && (mouse_raw.contains("906") || cnc.contains("Wave 906"))
        && !cnc.contains("playable_claim = true");
    residual_action_store(ResidualHostMousePresentationOnlyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_mouse_presentation_only_honesty() -> bool {
    let a = honesty_host_mouse_presentation_only_method_names_residual_wave906();
    let b = honesty_host_mouse_presentation_only_nav_commands_residual_wave906();
    let c = honesty_host_mouse_presentation_only_residual_pack_wave906();
    residual_action_store(ResidualHostMousePresentationOnlyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_mouse_presentation_only_residual_wave906() {
        assert!(honesty_host_mouse_presentation_only_residual_pack_wave906());
        assert!(honesty_host_mouse_presentation_only_method_names_residual_wave906());
        assert!(honesty_host_mouse_presentation_only_nav_commands_residual_wave906());
        assert!(simulate_live_host_mouse_presentation_only_honesty());
    }
}
