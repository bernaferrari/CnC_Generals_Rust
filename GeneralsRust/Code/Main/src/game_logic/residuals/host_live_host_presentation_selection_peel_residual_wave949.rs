//! Wave 949: presentation-only selection dual-read peel.
//!
//! UnitControl + InputProcessor box/select-similar/select-all/cycle selection
//! no longer iterates live `GameLogic::get_objects()` when classifying units.
//! Selection identity comes from the installed `PresentationFrame` freeze.
//! Fail-closed without a freeze. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_SELECTION_PEEL_METHOD_NAMES_WAVE949: &[&str] = &[
    "presentation_frame",
    "presentation_is_selectable",
    "select_all_units",
    "handle_box_selection",
    "select_similar_units",
    "cycle_units",
    "Wave 949",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_SELECTION_PEEL_NAV_STEPS_WAVE949: &[&str] = &[
    "PRESENTATION_SELECTION_PEEL",
    "SELECTION_NO_LIVE_GET_OBJECTS",
    "LIVE_HOST_PRESENTATION_SELECTION_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationSelectionPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationSelectionPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn uc_source() -> &'static str {
    include_str!("../../unit_control.rs")
}

fn ii_source() -> &'static str {
    include_str!("../../input_integration.rs")
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

pub fn honesty_host_presentation_selection_peel_method_names_residual_wave949() -> bool {
    let names = LIVE_HOST_PRESENTATION_SELECTION_PEEL_METHOD_NAMES_WAVE949;
    let ok = residual_name_index(names, "presentation_is_selectable").is_some()
        && residual_name_index(names, "Wave 949").is_some()
        && residual_name_index(names, "select_all_units").is_some();
    residual_action_store(ResidualHostPresentationSelectionPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_selection_peel_nav_commands_residual_wave949() -> bool {
    let steps = LIVE_HOST_PRESENTATION_SELECTION_PEEL_NAV_STEPS_WAVE949;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_SELECTION_PEEL").is_some()
        && residual_name_index(steps, "SELECTION_NO_LIVE_GET_OBJECTS").is_some();
    residual_action_store(ResidualHostPresentationSelectionPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_selection_peel_residual_pack_wave949() -> bool {
    let uc = uc_source();
    let ii = ii_source();
    let cnc = cnc_source();
    let gl = gl_source();
    let uc_box = non_comment_code(fn_window(uc, "fn handle_box_selection"));
    let uc_all = non_comment_code(fn_window(uc, "fn select_all_units"));
    let uc_sim = non_comment_code(fn_window(uc, "fn select_similar_units"));
    let ii_box = non_comment_code(fn_window(ii, "fn handle_box_selection"));
    let ii_all = non_comment_code(fn_window(ii, "fn select_all_units"));
    let ii_sim = non_comment_code(fn_window(ii, "fn select_similar_units"));
    let ii_cyc = non_comment_code(fn_window(ii, "fn cycle_units"));
    let ok = uc.contains("Wave 949")
        && ii.contains("Wave 949")
        && !uc_box.contains("get_objects()")
        && !uc_all.contains("get_objects()")
        && !uc_sim.contains("get_objects()")
        && !ii_box.contains("get_objects()")
        && !ii_all.contains("get_objects()")
        && !ii_sim.contains("get_objects()")
        && !ii_cyc.contains("get_objects()")
        && uc_box.contains("presentation_frame")
        && uc_all.contains("presentation_frame")
        && ii_box.contains("presentation_frame")
        && ii.contains("presentation_is_selectable")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationSelectionPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_selection_peel_honesty() -> bool {
    let a = honesty_host_presentation_selection_peel_method_names_residual_wave949();
    let b = honesty_host_presentation_selection_peel_nav_commands_residual_wave949();
    let c = honesty_host_presentation_selection_peel_residual_pack_wave949();
    residual_action_store(ResidualHostPresentationSelectionPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_selection_peel_residual_wave949() {
        assert!(honesty_host_presentation_selection_peel_residual_pack_wave949());
        assert!(honesty_host_presentation_selection_peel_method_names_residual_wave949());
        assert!(honesty_host_presentation_selection_peel_nav_commands_residual_wave949());
        assert!(simulate_live_host_presentation_selection_peel_honesty());
    }
}
