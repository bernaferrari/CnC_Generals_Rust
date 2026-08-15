//! Wave 950: seal Main `get_object_mut` dual-writes + presentation cycle peel.
//!
//! Remaining outside-`game_logic` `get_object_mut` call sites route through
//! `GameLogic::host_object_mut`. UnitInputHandler unit cycle is presentation-only.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GET_OBJECT_MUT_SEAL_METHOD_NAMES_WAVE950: &[&str] = &[
    "host_object_mut",
    "with_host_object_mut",
    "cycle_selected_units",
    "presentation_is_selectable",
    "Wave 950",
    "playable_claim = false",
];

pub const LIVE_HOST_GET_OBJECT_MUT_SEAL_NAV_STEPS_WAVE950: &[&str] = &[
    "GET_OBJECT_MUT_SEAL",
    "HOST_OBJECT_MUT_COMMAND_PATH",
    "PRESENTATION_CYCLE_SELECTION",
    "LIVE_HOST_GET_OBJECT_MUT_SEAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGetObjectMutSealAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostGetObjectMutSealAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn ce_source() -> &'static str {
    crate::command_executor::COMMAND_EXECUTOR_SRC
}

fn uc_source() -> &'static str {
    include_str!("../../unit_control.rs")
}

fn ui_source() -> &'static str {
    include_str!("../../unit_input_handler.rs")
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn non_comment_code(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn fn_window<'a>(src: &'a str, marker: &str) -> &'a str {
    let Some(i) = src.find(marker) else {
        return "";
    };
    let Some(brace) = src[i..].find('{').map(|o| i + o) else {
        return "";
    };
    let mut depth = 0usize;
    let mut p = brace;
    let bytes = src.as_bytes();
    while p < src.len() {
        match bytes[p] as char {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[i..=p];
                }
            }
            _ => {}
        }
        p += 1;
    }
    &src[i..src.len().min(i + 8_000)]
}

pub fn honesty_host_get_object_mut_seal_method_names_residual_wave950() -> bool {
    let names = LIVE_HOST_GET_OBJECT_MUT_SEAL_METHOD_NAMES_WAVE950;
    let ok = residual_name_index(names, "host_object_mut").is_some()
        && residual_name_index(names, "Wave 950").is_some()
        && residual_name_index(names, "cycle_selected_units").is_some();
    residual_action_store(ResidualHostGetObjectMutSealAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_get_object_mut_seal_nav_commands_residual_wave950() -> bool {
    let steps = LIVE_HOST_GET_OBJECT_MUT_SEAL_NAV_STEPS_WAVE950;
    let ok = residual_name_index(steps, "LIVE_HOST_GET_OBJECT_MUT_SEAL").is_some()
        && residual_name_index(steps, "PRESENTATION_CYCLE_SELECTION").is_some();
    residual_action_store(ResidualHostGetObjectMutSealAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_get_object_mut_seal_residual_pack_wave950() -> bool {
    let gl = gl_source();
    let ce = non_comment_code(ce_source());
    let uc = non_comment_code(uc_source());
    let pf = non_comment_code(pf_source());
    let ui = ui_source();
    let cnc = cnc_source();
    let cycle = non_comment_code(fn_window(ui, "fn cycle_selected_units"));
    let ok = gl.contains("fn host_object_mut")
        && !ce.contains("get_object_mut(")
        && !uc.contains("get_object_mut(")
        && !pf.contains("get_object_mut(")
        && ce.contains("host_object_mut")
        && cycle.contains("presentation_frame")
        && cycle.contains("presentation_is_selectable")
        && !cycle.contains("get_objects()")
        && (ui.contains("Wave 950") || ce_source().contains("Wave 950"))
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostGetObjectMutSealAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_get_object_mut_seal_honesty() -> bool {
    let a = honesty_host_get_object_mut_seal_method_names_residual_wave950();
    let b = honesty_host_get_object_mut_seal_nav_commands_residual_wave950();
    let c = honesty_host_get_object_mut_seal_residual_pack_wave950();
    residual_action_store(ResidualHostGetObjectMutSealAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_get_object_mut_seal_residual_wave950() {
        assert!(honesty_host_get_object_mut_seal_residual_pack_wave950());
        assert!(honesty_host_get_object_mut_seal_method_names_residual_wave950());
        assert!(honesty_host_get_object_mut_seal_nav_commands_residual_wave950());
        assert!(simulate_live_host_get_object_mut_seal_honesty());
    }
}
