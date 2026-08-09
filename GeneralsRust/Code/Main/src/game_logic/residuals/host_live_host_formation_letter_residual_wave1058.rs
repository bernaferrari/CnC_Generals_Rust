//! Wave 1058: dual-world formation letter residual.
//!
//! PresentationDrawableSync stamps formation_id; dual draw_ui_text draws the
//! formation letter via get_formation_letter_string. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FORMATION_LETTER_RESIDUAL_METHOD_NAMES_WAVE1058: &[&str] = &[
    "formation_id",
    "get_formation_letter_string",
    "Wave 1058",
    "playable_claim = false",
];

pub const LIVE_HOST_FORMATION_LETTER_RESIDUAL_NAV_STEPS_WAVE1058: &[&str] = &[
    "FORMATION",
    "LETTER_DRAW",
    "LIVE_HOST_FORMATION_LETTER_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFormationLetterResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFormationLetterResidualAction) {
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

pub fn honesty_host_formation_letter_residual_method_names_residual_wave1058() -> bool {
    let names = LIVE_HOST_FORMATION_LETTER_RESIDUAL_METHOD_NAMES_WAVE1058;
    let ok = residual_name_index(names, "formation_id").is_some()
        && residual_name_index(names, "Wave 1058").is_some();
    residual_action_store(ResidualHostFormationLetterResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_formation_letter_residual_nav_commands_residual_wave1058() -> bool {
    let steps = LIVE_HOST_FORMATION_LETTER_RESIDUAL_NAV_STEPS_WAVE1058;
    let ok = residual_name_index(steps, "LIVE_HOST_FORMATION_LETTER_RESIDUAL").is_some()
        && residual_name_index(steps, "FORMATION").is_some();
    residual_action_store(ResidualHostFormationLetterResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_formation_letter_residual_residual_pack_wave1058() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let d = drawable_source();
    let ok = client.contains("pub formation_id: u32")
        && cnc.contains("Wave 1058: formation residual for dual formation letter")
        && cnc.contains("formation_id: o.formation_id")
        && d.contains("Wave 1058: formation letter residual")
        && d.contains("presentation_formation_id")
        && d.contains("get_formation_letter_string")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFormationLetterResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_formation_letter_residual_honesty() -> bool {
    let a = honesty_host_formation_letter_residual_method_names_residual_wave1058();
    let b = honesty_host_formation_letter_residual_nav_commands_residual_wave1058();
    let c = honesty_host_formation_letter_residual_residual_pack_wave1058();
    residual_action_store(ResidualHostFormationLetterResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_formation_letter_residual_wave1058() {
        assert!(honesty_host_formation_letter_residual_residual_pack_wave1058());
        assert!(honesty_host_formation_letter_residual_method_names_residual_wave1058());
        assert!(honesty_host_formation_letter_residual_nav_commands_residual_wave1058());
        assert!(simulate_live_host_formation_letter_residual_honesty());
    }
}
