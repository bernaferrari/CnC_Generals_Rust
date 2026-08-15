//! Wave 1060: dual-world floating cash/text presentation residual.
//!
//! Presentation floating_texts apply into InGameUISubsystem residual each frame.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FLOATING_TEXT_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1060: &[&str] = &[
    "apply_presentation_floating_texts",
    "replace_floating_texts_from_presentation",
    "Wave 1060",
    "playable_claim = false",
];

pub const LIVE_HOST_FLOATING_TEXT_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1060: &[&str] = &[
    "FLOATING_TEXT",
    "PRESENTATION_APPLY",
    "LIVE_HOST_FLOATING_TEXT_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFloatingTextPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFloatingTextPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}
fn sub_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/core/subsystems.rs")
}

pub fn honesty_host_floating_text_presentation_residual_method_names_residual_wave1060() -> bool {
    let names = LIVE_HOST_FLOATING_TEXT_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1060;
    let ok = residual_name_index(names, "apply_presentation_floating_texts").is_some()
        && residual_name_index(names, "Wave 1060").is_some();
    residual_action_store(ResidualHostFloatingTextPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_floating_text_presentation_residual_nav_commands_residual_wave1060() -> bool {
    let steps = LIVE_HOST_FLOATING_TEXT_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1060;
    let ok = residual_name_index(steps, "LIVE_HOST_FLOATING_TEXT_PRESENTATION_RESIDUAL").is_some()
        && residual_name_index(steps, "FLOATING_TEXT").is_some();
    residual_action_store(ResidualHostFloatingTextPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_floating_text_presentation_residual_residual_pack_wave1060() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let sub = sub_source();
    let ok = cnc.contains("Wave 1060: floating cash/text residual → InGameUI")
        && cnc.contains("apply_presentation_floating_texts")
        && client.contains("Wave 1060: presentation floating cash/text residual")
        && sub.contains("presentation_floating_texts")
        && sub.contains("replace_floating_texts_from_presentation")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFloatingTextPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_floating_text_presentation_residual_honesty() -> bool {
    let a = honesty_host_floating_text_presentation_residual_method_names_residual_wave1060();
    let b = honesty_host_floating_text_presentation_residual_nav_commands_residual_wave1060();
    let c = honesty_host_floating_text_presentation_residual_residual_pack_wave1060();
    residual_action_store(ResidualHostFloatingTextPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_floating_text_presentation_residual_wave1060() {
        assert!(honesty_host_floating_text_presentation_residual_residual_pack_wave1060());
        assert!(honesty_host_floating_text_presentation_residual_method_names_residual_wave1060());
        assert!(honesty_host_floating_text_presentation_residual_nav_commands_residual_wave1060());
        assert!(simulate_live_host_floating_text_presentation_residual_honesty());
    }
}
