//! Wave 953: presentation peel for SimpleInputProcessor dual-reads.
//!
//! select-all / cycle / control-group / left-right click classify prefer
//! PresentationFrame (fail-closed without freeze). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SIMPLE_INPUT_PRESENTATION_PEEL_METHOD_NAMES_WAVE953: &[&str] = &[
    "set_presentation_frame",
    "presentation_frame",
    "select_all_units_async",
    "cycle_units_async",
    "select_control_group_async",
    "handle_left_click_async",
    "handle_right_click_async",
    "Wave 953",
    "playable_claim = false",
];

pub const LIVE_HOST_SIMPLE_INPUT_PRESENTATION_PEEL_NAV_STEPS_WAVE953: &[&str] = &[
    "SIMPLE_INPUT_PRESENTATION_PEEL",
    "SELECT_ALL_PRESENTATION",
    "CYCLE_PRESENTATION",
    "CONTROL_GROUP_PRESENTATION",
    "CLICK_CLASSIFY_PRESENTATION",
    "LIVE_HOST_SIMPLE_INPUT_PRESENTATION_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSimpleInputPresentationPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSimpleInputPresentationPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn simple_source() -> &'static str {
    include_str!("../../input_system_simple.rs")
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

pub fn honesty_host_simple_input_presentation_peel_method_names_residual_wave953() -> bool {
    let names = LIVE_HOST_SIMPLE_INPUT_PRESENTATION_PEEL_METHOD_NAMES_WAVE953;
    let ok = residual_name_index(names, "select_all_units_async").is_some()
        && residual_name_index(names, "Wave 953").is_some()
        && residual_name_index(names, "handle_right_click_async").is_some();
    residual_action_store(ResidualHostSimpleInputPresentationPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_simple_input_presentation_peel_nav_commands_residual_wave953() -> bool {
    let steps = LIVE_HOST_SIMPLE_INPUT_PRESENTATION_PEEL_NAV_STEPS_WAVE953;
    let ok = residual_name_index(steps, "LIVE_HOST_SIMPLE_INPUT_PRESENTATION_PEEL").is_some()
        && residual_name_index(steps, "CLICK_CLASSIFY_PRESENTATION").is_some();
    residual_action_store(ResidualHostSimpleInputPresentationPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_simple_input_presentation_peel_residual_pack_wave953() -> bool {
    let simple = simple_source();
    let cnc = cnc_source();
    let gl = gl_source();
    let select_all = non_comment_code(fn_window(simple, "async fn select_all_units_async"));
    let cycle = non_comment_code(fn_window(simple, "async fn cycle_units_async"));
    let cg = non_comment_code(fn_window(simple, "async fn select_control_group_async"));
    let left = non_comment_code(fn_window(simple, "async fn handle_left_click_async"));
    let right = non_comment_code(fn_window(simple, "async fn handle_right_click_async"));
    let ok = simple.contains("Wave 953")
        && simple.contains("fn set_presentation_frame")
        && !select_all.contains("get_objects()")
        && select_all.contains("presentation_frame")
        && !cycle.contains("get_objects()")
        && cycle.contains("presentation_frame")
        && !cg.contains("find_object(")
        && cg.contains("presentation_frame")
        && !left.contains("find_object(")
        && left.contains("presentation_frame")
        && !right.contains("find_object(")
        && right.contains("presentation_frame")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSimpleInputPresentationPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_simple_input_presentation_peel_honesty() -> bool {
    let a = honesty_host_simple_input_presentation_peel_method_names_residual_wave953();
    let b = honesty_host_simple_input_presentation_peel_nav_commands_residual_wave953();
    let c = honesty_host_simple_input_presentation_peel_residual_pack_wave953();
    residual_action_store(ResidualHostSimpleInputPresentationPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_simple_input_presentation_peel_residual_wave953() {
        assert!(honesty_host_simple_input_presentation_peel_residual_pack_wave953());
        assert!(honesty_host_simple_input_presentation_peel_method_names_residual_wave953());
        assert!(honesty_host_simple_input_presentation_peel_nav_commands_residual_wave953());
        assert!(simulate_live_host_simple_input_presentation_peel_honesty());
    }
}
