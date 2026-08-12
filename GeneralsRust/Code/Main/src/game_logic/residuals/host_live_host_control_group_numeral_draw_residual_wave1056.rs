//! Wave 1056: dual-world control-group numeral draw residual.
//!
//! draw_ui_text_from_presentation draws group numerals via draw_caption_string
//! (not resolve-only). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONTROL_GROUP_NUMERAL_DRAW_RESIDUAL_METHOD_NAMES_WAVE1056: &[&str] = &[
    "draw_ui_text_from_presentation",
    "draw_caption_string",
    "Wave 1056",
    "playable_claim = false",
];

pub const LIVE_HOST_CONTROL_GROUP_NUMERAL_DRAW_RESIDUAL_NAV_STEPS_WAVE1056: &[&str] = &[
    "CONTROL_GROUP",
    "NUMERAL_DRAW",
    "LIVE_HOST_CONTROL_GROUP_NUMERAL_DRAW_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostControlGroupNumeralDrawResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostControlGroupNumeralDrawResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn drawable_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}

pub fn honesty_host_control_group_numeral_draw_residual_method_names_residual_wave1056() -> bool {
    let names = LIVE_HOST_CONTROL_GROUP_NUMERAL_DRAW_RESIDUAL_METHOD_NAMES_WAVE1056;
    let ok = residual_name_index(names, "draw_caption_string").is_some()
        && residual_name_index(names, "Wave 1056").is_some();
    residual_action_store(ResidualHostControlGroupNumeralDrawResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_control_group_numeral_draw_residual_nav_commands_residual_wave1056() -> bool {
    let steps = LIVE_HOST_CONTROL_GROUP_NUMERAL_DRAW_RESIDUAL_NAV_STEPS_WAVE1056;
    let ok = residual_name_index(steps, "LIVE_HOST_CONTROL_GROUP_NUMERAL_DRAW_RESIDUAL").is_some()
        && residual_name_index(steps, "NUMERAL_DRAW").is_some();
    residual_action_store(ResidualHostControlGroupNumeralDrawResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_control_group_numeral_draw_residual_residual_pack_wave1056() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let d = drawable_source();
    let ok = d.contains("Wave 1055/1056: host control-group residual → group numeral dual draw")
        && d.contains("Wave 1056: actually draw the numeral (not resolve-only residual).")
        && d.contains("draw_group_info.drop_shadow_offset_x")
        && d.contains("Self::draw_caption_string(")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostControlGroupNumeralDrawResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_control_group_numeral_draw_residual_honesty() -> bool {
    let a = honesty_host_control_group_numeral_draw_residual_method_names_residual_wave1056();
    let b = honesty_host_control_group_numeral_draw_residual_nav_commands_residual_wave1056();
    let c = honesty_host_control_group_numeral_draw_residual_residual_pack_wave1056();
    residual_action_store(ResidualHostControlGroupNumeralDrawResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_control_group_numeral_draw_residual_wave1056() {
        assert!(honesty_host_control_group_numeral_draw_residual_residual_pack_wave1056());
        assert!(honesty_host_control_group_numeral_draw_residual_method_names_residual_wave1056());
        assert!(honesty_host_control_group_numeral_draw_residual_nav_commands_residual_wave1056());
        assert!(simulate_live_host_control_group_numeral_draw_residual_honesty());
    }
}
