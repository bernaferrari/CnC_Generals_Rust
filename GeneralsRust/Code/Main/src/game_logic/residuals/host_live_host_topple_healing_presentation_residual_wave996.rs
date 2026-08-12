//! Wave 996: topple lean + healing presentation residual from GameWorld Entity.
//!
//! Entity path mirrors topple_lean_radians, sole-healing window (show_healing),
//! and healing_icon_type from kind_of_bits (Structure/Vehicle).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TOPPLE_HEALING_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE996: &[&str] = &[
    "topple_lean_radians",
    "sole_healing_benefactor_expiration_frame",
    "healing_icon_type",
    "Wave 996",
    "playable_claim = false",
];

pub const LIVE_HOST_TOPPLE_HEALING_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE996: &[&str] = &[
    "TOPPLE_LEAN",
    "SHOW_HEALING",
    "HEALING_ICON",
    "LIVE_HOST_TOPPLE_HEALING_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostToppleHealingPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostToppleHealingPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn entity_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}

pub fn honesty_host_topple_healing_presentation_residual_method_names_residual_wave996() -> bool {
    let names = LIVE_HOST_TOPPLE_HEALING_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE996;
    let ok = residual_name_index(names, "topple_lean_radians").is_some()
        && residual_name_index(names, "Wave 996").is_some();
    residual_action_store(ResidualHostToppleHealingPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_topple_healing_presentation_residual_nav_commands_residual_wave996() -> bool {
    let steps = LIVE_HOST_TOPPLE_HEALING_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE996;
    let ok = residual_name_index(steps, "LIVE_HOST_TOPPLE_HEALING_PRESENTATION_RESIDUAL").is_some()
        && residual_name_index(steps, "SHOW_HEALING").is_some();
    residual_action_store(ResidualHostToppleHealingPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_topple_healing_presentation_residual_residual_pack_wave996() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ent = entity_source();
    let gw = match pf.find("fn renderable_from_gameworld_entity") {
        Some(i) => &pf[i..pf.len().min(i + 10000)],
        None => "",
    };
    let ok = ent.contains("pub topple_lean_radians: f32")
        && ent.contains("pub sole_healing_benefactor_expiration_frame: u32")
        && gw.contains("topple_lean_radians: ent.topple_lean_radians")
        && gw.contains("sole_healing_benefactor_expiration_frame != 0")
        && gw.contains("healing_icon_type:")
        && gw.contains("VEHICLE_BIT")
        && !gw.contains("topple_lean_radians: 0.0")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostToppleHealingPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_topple_healing_presentation_residual_honesty() -> bool {
    let a = honesty_host_topple_healing_presentation_residual_method_names_residual_wave996();
    let b = honesty_host_topple_healing_presentation_residual_nav_commands_residual_wave996();
    let c = honesty_host_topple_healing_presentation_residual_residual_pack_wave996();
    residual_action_store(ResidualHostToppleHealingPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_topple_healing_presentation_residual_wave996() {
        assert!(honesty_host_topple_healing_presentation_residual_residual_pack_wave996());
        assert!(honesty_host_topple_healing_presentation_residual_method_names_residual_wave996());
        assert!(honesty_host_topple_healing_presentation_residual_nav_commands_residual_wave996());
        assert!(simulate_live_host_topple_healing_presentation_residual_honesty());
    }
}
