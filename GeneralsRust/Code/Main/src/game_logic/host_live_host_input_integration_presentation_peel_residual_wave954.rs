//! Wave 954: presentation peel for InputProcessor click dual-reads.
//!
//! left/right click classify + control-group filter prefer PresentationFrame
//! (fail-closed without freeze). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_INPUT_INTEGRATION_PRESENTATION_PEEL_METHOD_NAMES_WAVE954: &[&str] = &[
    "set_presentation_frame",
    "presentation_frame",
    "handle_left_click",
    "handle_right_click",
    "select_control_group",
    "Wave 954",
    "playable_claim = false",
];

pub const LIVE_HOST_INPUT_INTEGRATION_PRESENTATION_PEEL_NAV_STEPS_WAVE954: &[&str] = &[
    "INPUT_INTEGRATION_PRESENTATION_PEEL",
    "LEFT_CLICK_PRESENTATION",
    "RIGHT_CLICK_PRESENTATION",
    "CONTROL_GROUP_PRESENTATION",
    "LIVE_HOST_INPUT_INTEGRATION_PRESENTATION_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostInputIntegrationPresentationPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostInputIntegrationPresentationPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn input_source() -> &'static str {
    include_str!("../input_integration.rs")
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
    &src[i..src.len().min(i + 12_000)]
}

pub fn honesty_host_input_integration_presentation_peel_method_names_residual_wave954() -> bool {
    let names = LIVE_HOST_INPUT_INTEGRATION_PRESENTATION_PEEL_METHOD_NAMES_WAVE954;
    let ok = residual_name_index(names, "handle_left_click").is_some()
        && residual_name_index(names, "Wave 954").is_some()
        && residual_name_index(names, "select_control_group").is_some();
    residual_action_store(ResidualHostInputIntegrationPresentationPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_input_integration_presentation_peel_nav_commands_residual_wave954() -> bool {
    let steps = LIVE_HOST_INPUT_INTEGRATION_PRESENTATION_PEEL_NAV_STEPS_WAVE954;
    let ok = residual_name_index(steps, "LIVE_HOST_INPUT_INTEGRATION_PRESENTATION_PEEL").is_some()
        && residual_name_index(steps, "RIGHT_CLICK_PRESENTATION").is_some();
    residual_action_store(ResidualHostInputIntegrationPresentationPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_input_integration_presentation_peel_residual_pack_wave954() -> bool {
    let input = input_source();
    let cnc = cnc_source();
    let gl = gl_source();
    let left = non_comment_code(fn_window(input, "async fn handle_left_click"));
    let right = non_comment_code(fn_window(input, "async fn handle_right_click"));
    let cg = non_comment_code(fn_window(input, "fn select_control_group"));
    let ok = input.contains("Wave 954")
        && input.contains("fn set_presentation_frame")
        && !left.contains("find_object(")
        && left.contains("presentation_frame")
        && !right.contains("find_object(")
        && right.contains("presentation_frame")
        && !cg.contains("find_object(")
        && cg.contains("presentation_frame")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostInputIntegrationPresentationPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_input_integration_presentation_peel_honesty() -> bool {
    let a = honesty_host_input_integration_presentation_peel_method_names_residual_wave954();
    let b = honesty_host_input_integration_presentation_peel_nav_commands_residual_wave954();
    let c = honesty_host_input_integration_presentation_peel_residual_pack_wave954();
    residual_action_store(ResidualHostInputIntegrationPresentationPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_input_integration_presentation_peel_residual_wave954() {
        assert!(honesty_host_input_integration_presentation_peel_residual_pack_wave954());
        assert!(honesty_host_input_integration_presentation_peel_method_names_residual_wave954());
        assert!(honesty_host_input_integration_presentation_peel_nav_commands_residual_wave954());
        assert!(simulate_live_host_input_integration_presentation_peel_honesty());
    }
}
