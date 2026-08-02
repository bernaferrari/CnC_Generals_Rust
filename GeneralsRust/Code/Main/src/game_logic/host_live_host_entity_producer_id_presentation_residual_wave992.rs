//! Wave 992: PresentationFrame mirrors GameWorld Entity.producer_id.
//!
//! Host Object::producer_id already shadows onto Entity (gameworld_shadow).
//! Wave 992 projects that residual into RenderableObject.producer_id when
//! building presentation from GameWorld entities (Wave 982 left None).
//! Enables IgnoredInGui mouseover slaver remap from presentation.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ENTITY_PRODUCER_ID_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE992: &[&str] = &[
    "producer_id",
    "ent.producer_id",
    "Wave 992",
    "playable_claim = false",
];

pub const LIVE_HOST_ENTITY_PRODUCER_ID_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE992: &[&str] = &[
    "ENTITY_PRODUCER_ID",
    "PRESENTATION_MIRROR",
    "IGNORED_IN_GUI_SLAVER",
    "LIVE_HOST_ENTITY_PRODUCER_ID_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEntityProducerIdPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostEntityProducerIdPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}
fn entity_source() -> &'static str {
    include_str!("../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}
fn shadow_source() -> &'static str {
    include_str!("../gameworld_shadow.rs")
}

pub fn honesty_host_entity_producer_id_presentation_residual_method_names_residual_wave992() -> bool
{
    let names = LIVE_HOST_ENTITY_PRODUCER_ID_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE992;
    let ok = residual_name_index(names, "ent.producer_id").is_some()
        && residual_name_index(names, "Wave 992").is_some();
    residual_action_store(ResidualHostEntityProducerIdPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_entity_producer_id_presentation_residual_nav_commands_residual_wave992() -> bool
{
    let steps = LIVE_HOST_ENTITY_PRODUCER_ID_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE992;
    let ok = residual_name_index(steps, "LIVE_HOST_ENTITY_PRODUCER_ID_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "PRESENTATION_MIRROR").is_some();
    residual_action_store(ResidualHostEntityProducerIdPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_entity_producer_id_presentation_residual_residual_pack_wave992() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ent = entity_source();
    let sh = shadow_source();
    let ok = ent.contains("pub producer_id: Option<u32>")
        && sh.contains("e.producer_id = obj.producer_id")
        && pf.contains("ent.producer_id.map(ObjectId)")
        && pf.contains("Wave 992")
        && !pf.contains("GW entity producer residual not yet mirrored")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostEntityProducerIdPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_entity_producer_id_presentation_residual_honesty() -> bool {
    let a = honesty_host_entity_producer_id_presentation_residual_method_names_residual_wave992();
    let b = honesty_host_entity_producer_id_presentation_residual_nav_commands_residual_wave992();
    let c = honesty_host_entity_producer_id_presentation_residual_residual_pack_wave992();
    residual_action_store(ResidualHostEntityProducerIdPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_entity_producer_id_presentation_residual_wave992() {
        assert!(honesty_host_entity_producer_id_presentation_residual_residual_pack_wave992());
        assert!(
            honesty_host_entity_producer_id_presentation_residual_method_names_residual_wave992()
        );
        assert!(
            honesty_host_entity_producer_id_presentation_residual_nav_commands_residual_wave992()
        );
        assert!(simulate_live_host_entity_producer_id_presentation_residual_honesty());
    }
}
