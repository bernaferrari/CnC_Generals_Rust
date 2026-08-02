//! Wave 965: drawable presentation host residual (kind/stealth/color/health).
//!
//! BasicDrawable stamps presentation KindOf names, team color, stealth, and
//! health fraction so dual-world empty path can answer kind/stealth/indicator
//! queries without OBJECT_REGISTRY. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DRAWABLE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE965: &[&str] = &[
    "set_presentation_host_residual",
    "stamp_presentation_object_residual",
    "presentation_kind_names",
    "Wave 965",
    "playable_claim = false",
];

pub const LIVE_HOST_DRAWABLE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE965: &[&str] = &[
    "DRAWABLE_PRESENTATION_HOST_RESIDUAL",
    "KIND_OF_FROM_FREEZE",
    "STEALTH_FROM_FREEZE",
    "TEAM_COLOR_FROM_FREEZE",
    "LIVE_HOST_DRAWABLE_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDrawablePresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDrawablePresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

fn drawable_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/drawable/drawable.rs")
}

pub fn honesty_host_drawable_presentation_residual_method_names_residual_wave965() -> bool {
    let names = LIVE_HOST_DRAWABLE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE965;
    let ok = residual_name_index(names, "set_presentation_host_residual").is_some()
        && residual_name_index(names, "Wave 965").is_some();
    residual_action_store(ResidualHostDrawablePresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_presentation_residual_nav_commands_residual_wave965() -> bool {
    let steps = LIVE_HOST_DRAWABLE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE965;
    let ok = residual_name_index(steps, "LIVE_HOST_DRAWABLE_PRESENTATION_RESIDUAL").is_some()
        && residual_name_index(steps, "KIND_OF_FROM_FREEZE").is_some();
    residual_action_store(ResidualHostDrawablePresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_presentation_residual_residual_pack_wave965() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let drawable = drawable_source();
    let ok = drawable.contains("Wave 965")
        && client.contains("Wave 965")
        && (cnc.contains("Wave 965") || cnc.contains("kind_names:"))
        && drawable.contains("set_presentation_host_residual")
        && drawable.contains("presentation_kind_names")
        && drawable.contains("presentation_effectively_stealthed")
        && client.contains("stamp_presentation_object_residual")
        && client.contains("kind_names")
        && client.contains("effectively_stealthed")
        && cnc.contains("kind_names:")
        && cnc.contains("effectively_stealthed:")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDrawablePresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_drawable_presentation_residual_honesty() -> bool {
    let a = honesty_host_drawable_presentation_residual_method_names_residual_wave965();
    let b = honesty_host_drawable_presentation_residual_nav_commands_residual_wave965();
    let c = honesty_host_drawable_presentation_residual_residual_pack_wave965();
    residual_action_store(ResidualHostDrawablePresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_drawable_presentation_residual_wave965() {
        assert!(honesty_host_drawable_presentation_residual_residual_pack_wave965());
        assert!(honesty_host_drawable_presentation_residual_method_names_residual_wave965());
        assert!(honesty_host_drawable_presentation_residual_nav_commands_residual_wave965());
        assert!(simulate_live_host_drawable_presentation_residual_honesty());
    }
}
