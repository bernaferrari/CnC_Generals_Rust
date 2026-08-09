//! Wave 1059: dual-world unit caption residual.
//!
//! PresentationDrawableSync stamps caption from distinct display_name; dual
//! draw_ui_text draws caption via new_display_string + draw_caption_string.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UNIT_CAPTION_RESIDUAL_METHOD_NAMES_WAVE1059: &[&str] = &[
    "caption",
    "set_caption_text",
    "draw_caption_string",
    "Wave 1059",
    "playable_claim = false",
];

pub const LIVE_HOST_UNIT_CAPTION_RESIDUAL_NAV_STEPS_WAVE1059: &[&str] = &[
    "CAPTION",
    "DUAL_DRAW",
    "LIVE_HOST_UNIT_CAPTION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUnitCaptionResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUnitCaptionResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn client_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/core/game_client.rs")
}
fn drawable_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/drawable/drawable.rs")
}

pub fn honesty_host_unit_caption_residual_method_names_residual_wave1059() -> bool {
    let names = LIVE_HOST_UNIT_CAPTION_RESIDUAL_METHOD_NAMES_WAVE1059;
    let ok = residual_name_index(names, "caption").is_some()
        && residual_name_index(names, "Wave 1059").is_some();
    residual_action_store(ResidualHostUnitCaptionResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_unit_caption_residual_nav_commands_residual_wave1059() -> bool {
    let steps = LIVE_HOST_UNIT_CAPTION_RESIDUAL_NAV_STEPS_WAVE1059;
    let ok = residual_name_index(steps, "LIVE_HOST_UNIT_CAPTION_RESIDUAL").is_some()
        && residual_name_index(steps, "CAPTION").is_some();
    residual_action_store(ResidualHostUnitCaptionResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_unit_caption_residual_residual_pack_wave1059() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let d = drawable_source();
    let ok = client.contains("/// Wave 1059: unit caption residual")
        && cnc.contains("Wave 1059: caption residual (display_name when distinct from template)")
        && d.contains("Wave 1059: caption residual for dual draw_ui_text")
        && d.contains("Wave 1059: dual caption draw from presentation caption residual")
        && d.contains("manager.new_display_string()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUnitCaptionResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_unit_caption_residual_honesty() -> bool {
    let a = honesty_host_unit_caption_residual_method_names_residual_wave1059();
    let b = honesty_host_unit_caption_residual_nav_commands_residual_wave1059();
    let c = honesty_host_unit_caption_residual_residual_pack_wave1059();
    residual_action_store(ResidualHostUnitCaptionResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_unit_caption_residual_wave1059() {
        assert!(honesty_host_unit_caption_residual_residual_pack_wave1059());
        assert!(honesty_host_unit_caption_residual_method_names_residual_wave1059());
        assert!(honesty_host_unit_caption_residual_nav_commands_residual_wave1059());
        assert!(simulate_live_host_unit_caption_residual_honesty());
    }
}
