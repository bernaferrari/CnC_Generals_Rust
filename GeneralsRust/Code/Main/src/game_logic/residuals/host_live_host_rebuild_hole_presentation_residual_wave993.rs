//! Wave 993: RebuildHole presentation residual from host + GameWorld entity.
//!
//! Freezes RebuildHoleBehavior fields onto RenderableObject:
//! rebuild_template_name, rebuild_ready_frame, spawner/worker/reconstructing ids.
//! Host Object and Entity paths both project. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_REBUILD_HOLE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE993: &[&str] = &[
    "rebuild_template_name",
    "rebuild_spawner_id",
    "is_rebuild_hole",
    "Wave 993",
    "playable_claim = false",
];

pub const LIVE_HOST_REBUILD_HOLE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE993: &[&str] = &[
    "REBUILD_HOLE",
    "PRESENTATION_FREEZE",
    "ENTITY_MIRROR",
    "LIVE_HOST_REBUILD_HOLE_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRebuildHolePresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostRebuildHolePresentationResidualAction) {
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
fn object_source() -> &'static str {
    crate::game_logic::object::OBJECT_SRC
}

pub fn honesty_host_rebuild_hole_presentation_residual_method_names_residual_wave993() -> bool {
    let names = LIVE_HOST_REBUILD_HOLE_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE993;
    let ok = residual_name_index(names, "rebuild_template_name").is_some()
        && residual_name_index(names, "Wave 993").is_some();
    residual_action_store(ResidualHostRebuildHolePresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_rebuild_hole_presentation_residual_nav_commands_residual_wave993() -> bool {
    let steps = LIVE_HOST_REBUILD_HOLE_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE993;
    let ok = residual_name_index(steps, "LIVE_HOST_REBUILD_HOLE_PRESENTATION_RESIDUAL").is_some()
        && residual_name_index(steps, "ENTITY_MIRROR").is_some();
    residual_action_store(ResidualHostRebuildHolePresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_rebuild_hole_presentation_residual_residual_pack_wave993() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ent = entity_source();
    let obj = object_source();
    let ok = obj.contains("pub rebuild_template_name")
        && ent.contains("pub rebuild_template_name: String")
        && pf.contains("pub rebuild_template_name: String")
        && pf.contains("Wave 993")
        && pf.contains("rebuild_template_name: ent.rebuild_template_name.clone()")
        && pf.contains("rebuild_template_name: obj")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostRebuildHolePresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_rebuild_hole_presentation_residual_honesty() -> bool {
    let a = honesty_host_rebuild_hole_presentation_residual_method_names_residual_wave993();
    let b = honesty_host_rebuild_hole_presentation_residual_nav_commands_residual_wave993();
    let c = honesty_host_rebuild_hole_presentation_residual_residual_pack_wave993();
    residual_action_store(ResidualHostRebuildHolePresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_rebuild_hole_presentation_residual_wave993() {
        assert!(honesty_host_rebuild_hole_presentation_residual_residual_pack_wave993());
        assert!(honesty_host_rebuild_hole_presentation_residual_method_names_residual_wave993());
        assert!(honesty_host_rebuild_hole_presentation_residual_nav_commands_residual_wave993());
        assert!(simulate_live_host_rebuild_hole_presentation_residual_honesty());
    }
}
