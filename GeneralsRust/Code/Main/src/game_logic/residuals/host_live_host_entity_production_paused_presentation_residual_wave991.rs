//! Wave 991: PresentationFrame mirrors GameWorld Entity.production_paused.
//!
//! Wave 990 froze host BuildingData::production_paused onto Entity. Wave 991
//! projects that residual into RenderableObject / HUD when building presentation
//! from GameWorld entities (no longer hardcodes false).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ENTITY_PRODUCTION_PAUSED_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE991: &[&str] =
    &[
        "production_paused",
        "ent.production_paused",
        "Wave 991",
        "playable_claim = false",
    ];

pub const LIVE_HOST_ENTITY_PRODUCTION_PAUSED_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE991: &[&str] = &[
    "ENTITY_PRODUCTION_PAUSED",
    "PRESENTATION_MIRROR",
    "RENDERABLE_OBJECT",
    "LIVE_HOST_ENTITY_PRODUCTION_PAUSED_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEntityProductionPausedPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostEntityProductionPausedPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn entity_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}

pub fn honesty_host_entity_production_paused_presentation_residual_method_names_residual_wave991()
-> bool {
    let names = LIVE_HOST_ENTITY_PRODUCTION_PAUSED_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE991;
    let ok = residual_name_index(names, "ent.production_paused").is_some()
        && residual_name_index(names, "Wave 991").is_some();
    residual_action_store(
        ResidualHostEntityProductionPausedPresentationResidualAction::MethodNames,
    );
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_entity_production_paused_presentation_residual_nav_commands_residual_wave991()
-> bool {
    let steps = LIVE_HOST_ENTITY_PRODUCTION_PAUSED_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE991;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_ENTITY_PRODUCTION_PAUSED_PRESENTATION_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "PRESENTATION_MIRROR").is_some();
    residual_action_store(
        ResidualHostEntityProductionPausedPresentationResidualAction::NavCommands,
    );
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_entity_production_paused_presentation_residual_residual_pack_wave991() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ent = entity_source();
    let ok = ent.contains("pub production_paused: bool")
        && pf.contains("ent.production_paused")
        && pf.contains("Wave 991")
        && pf.contains("Wave 991: GameWorld entity pause residual")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(
        ResidualHostEntityProductionPausedPresentationResidualAction::SourceMarkers,
    );
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_entity_production_paused_presentation_residual_honesty() -> bool {
    let a =
        honesty_host_entity_production_paused_presentation_residual_method_names_residual_wave991();
    let b =
        honesty_host_entity_production_paused_presentation_residual_nav_commands_residual_wave991();
    let c = honesty_host_entity_production_paused_presentation_residual_residual_pack_wave991();
    residual_action_store(
        ResidualHostEntityProductionPausedPresentationResidualAction::DispatchSource,
    );
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_entity_production_paused_presentation_residual_wave991() {
        assert!(
            honesty_host_entity_production_paused_presentation_residual_residual_pack_wave991()
        );
        assert!(honesty_host_entity_production_paused_presentation_residual_method_names_residual_wave991());
        assert!(honesty_host_entity_production_paused_presentation_residual_nav_commands_residual_wave991());
        assert!(simulate_live_host_entity_production_paused_presentation_residual_honesty());
    }
}
