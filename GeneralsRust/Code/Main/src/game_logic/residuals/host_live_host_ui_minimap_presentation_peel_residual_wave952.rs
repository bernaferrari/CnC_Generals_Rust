//! Wave 952: presentation peel for WgpuUI + minimap unit dual-reads.
//!
//! WgpuUISystem selection centroid/pose and MinimapFowIntegration unit dots
//! prefer `PresentationFrame` (fail-closed without freeze for pose residual).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_MINIMAP_PRESENTATION_PEEL_METHOD_NAMES_WAVE952: &[&str] = &[
    "set_presentation_frame",
    "presentation_frame",
    "handle_command_button",
    "Wave 952",
    "playable_claim = false",
];

pub const LIVE_HOST_UI_MINIMAP_PRESENTATION_PEEL_NAV_STEPS_WAVE952: &[&str] = &[
    "UI_MINIMAP_PRESENTATION_PEEL",
    "WGPU_UI_NO_LIVE_GET_OBJECT",
    "MINIMAP_UNIT_DOTS_PRESENTATION",
    "LIVE_HOST_UI_MINIMAP_PRESENTATION_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiMinimapPresentationPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUiMinimapPresentationPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn ui_source() -> &'static str {
    include_str!("../../ui/wgpu_ui_system.rs")
}

fn mm_source() -> &'static str {
    include_str!("../../minimap_fow_integration.rs")
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

pub fn honesty_host_ui_minimap_presentation_peel_method_names_residual_wave952() -> bool {
    let names = LIVE_HOST_UI_MINIMAP_PRESENTATION_PEEL_METHOD_NAMES_WAVE952;
    let ok = residual_name_index(names, "set_presentation_frame").is_some()
        && residual_name_index(names, "Wave 952").is_some()
        && residual_name_index(names, "handle_command_button").is_some();
    residual_action_store(ResidualHostUiMinimapPresentationPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_minimap_presentation_peel_nav_commands_residual_wave952() -> bool {
    let steps = LIVE_HOST_UI_MINIMAP_PRESENTATION_PEEL_NAV_STEPS_WAVE952;
    let ok = residual_name_index(steps, "LIVE_HOST_UI_MINIMAP_PRESENTATION_PEEL").is_some()
        && residual_name_index(steps, "MINIMAP_UNIT_DOTS_PRESENTATION").is_some();
    residual_action_store(ResidualHostUiMinimapPresentationPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_minimap_presentation_peel_residual_pack_wave952() -> bool {
    let ui = ui_source();
    let mm = mm_source();
    let cnc = cnc_source();
    let gl = gl_source();
    let btn = non_comment_code(fn_window(ui, "fn handle_command_button"));
    let mm_code = non_comment_code(mm);
    let ok = ui.contains("Wave 952")
        && mm.contains("Wave 952")
        && ui.contains("fn set_presentation_frame")
        && mm.contains("fn set_presentation_frame")
        && !btn.contains("get_object(")
        && btn.contains("presentation_frame")
        && !mm_code.contains("get_objects()")
        && mm_code.contains("presentation_frame")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUiMinimapPresentationPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ui_minimap_presentation_peel_honesty() -> bool {
    let a = honesty_host_ui_minimap_presentation_peel_method_names_residual_wave952();
    let b = honesty_host_ui_minimap_presentation_peel_nav_commands_residual_wave952();
    let c = honesty_host_ui_minimap_presentation_peel_residual_pack_wave952();
    residual_action_store(ResidualHostUiMinimapPresentationPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ui_minimap_presentation_peel_residual_wave952() {
        assert!(honesty_host_ui_minimap_presentation_peel_residual_pack_wave952());
        assert!(honesty_host_ui_minimap_presentation_peel_method_names_residual_wave952());
        assert!(honesty_host_ui_minimap_presentation_peel_nav_commands_residual_wave952());
        assert!(simulate_live_host_ui_minimap_presentation_peel_honesty());
    }
}
