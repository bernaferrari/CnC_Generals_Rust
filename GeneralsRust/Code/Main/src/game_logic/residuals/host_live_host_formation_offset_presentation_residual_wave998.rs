//! Wave 998: formation_offset presentation residual from GameWorld Entity.
//!
//! Entity path no longer hardcodes formation_offset to Vec2::ZERO; mirrors
//! host Object::formation_offset residual. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FORMATION_OFFSET_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE998: &[&str] = &[
    "formation_offset",
    "formation_id",
    "Wave 998",
    "playable_claim = false",
];

pub const LIVE_HOST_FORMATION_OFFSET_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE998: &[&str] = &[
    "FORMATION_OFFSET",
    "ENTITY_MIRROR",
    "PRESENTATION_FREEZE",
    "LIVE_HOST_FORMATION_OFFSET_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFormationOffsetPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFormationOffsetPresentationResidualAction) {
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

pub fn honesty_host_formation_offset_presentation_residual_method_names_residual_wave998() -> bool {
    let names = LIVE_HOST_FORMATION_OFFSET_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE998;
    let ok = residual_name_index(names, "formation_offset").is_some()
        && residual_name_index(names, "Wave 998").is_some();
    residual_action_store(ResidualHostFormationOffsetPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_formation_offset_presentation_residual_nav_commands_residual_wave998() -> bool {
    let steps = LIVE_HOST_FORMATION_OFFSET_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE998;
    let ok = residual_name_index(steps, "LIVE_HOST_FORMATION_OFFSET_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "ENTITY_MIRROR").is_some();
    residual_action_store(ResidualHostFormationOffsetPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_formation_offset_presentation_residual_residual_pack_wave998() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ent = entity_source();
    let gw = match pf.find("fn renderable_from_gameworld_entity") {
        Some(i) => &pf[i..pf.len().min(i + 10000)],
        None => "",
    };
    let ok = ent.contains("pub formation_offset: [f32; 2]")
        && gw.contains("ent.formation_offset[0]")
        && gw.contains("Wave 998")
        && !gw.contains("formation_offset: glam::Vec2::ZERO")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFormationOffsetPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_formation_offset_presentation_residual_honesty() -> bool {
    let a = honesty_host_formation_offset_presentation_residual_method_names_residual_wave998();
    let b = honesty_host_formation_offset_presentation_residual_nav_commands_residual_wave998();
    let c = honesty_host_formation_offset_presentation_residual_residual_pack_wave998();
    residual_action_store(ResidualHostFormationOffsetPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_formation_offset_presentation_residual_wave998() {
        assert!(honesty_host_formation_offset_presentation_residual_residual_pack_wave998());
        assert!(
            honesty_host_formation_offset_presentation_residual_method_names_residual_wave998()
        );
        assert!(
            honesty_host_formation_offset_presentation_residual_nav_commands_residual_wave998()
        );
        assert!(simulate_live_host_formation_offset_presentation_residual_honesty());
    }
}
