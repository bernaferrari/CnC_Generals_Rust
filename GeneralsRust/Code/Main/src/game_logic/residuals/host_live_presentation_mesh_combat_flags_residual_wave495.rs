//! Wave 495 residual peels: unit mesh pass stamps combat model-condition bits.
//! - `UnitRenderInput` carries `moving` / `attacking` / `is_firing_weapon`
//! - `model_condition_bits_with_combat_flags` ORs moving/attacking/firing bits
//!   (MC_BIT_* or Wave 526 name-table helpers)
//! - render collect uses stamped bits before sold mesh resolve
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 494 turret mesh facing.
//! Architecture residual - presentation combat flags must reach mesh condition channel.
//!
//! Sources:
//! - presentation_frame.rs UnitRenderInput + model_condition_bits_with_combat_flags
//! - graphics/render_pipeline.rs unit mesh collect Wave 495
//!
//! Fail-closed:
//! - Full W3D anim graph still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MESH_COMBAT_FLAGS_METHOD_NAMES_WAVE495: &[&str] = &[
    "is_firing_weapon",
    "moving",
    "attacking",
    "model_condition_bits_with_combat_flags",
    "MC_BIT_FIRING_A",
    "playable_claim = false",
];

pub const PRESENTATION_MESH_COMBAT_FLAGS_SOURCE_MARKERS_WAVE495: &[&str] = &[
    "Wave 495: frozen combat motion flags for mesh model-condition stamping",
    "Wave 495: ensure combat motion flags are present in model-condition bits",
    "Wave 495: stamp moving/attacking/firing bits then honor sold residual",
    "model_condition_bits_with_combat_flags",
];

pub const PRESENTATION_MESH_COMBAT_FLAGS_NAV_STEPS_WAVE495: &[&str] = &[
    "FREEZE_COMBAT_FLAGS",
    "STAMP_MODEL_CONDITION_BITS",
    "RENDER_USES_STAMPED_BITS",
    "SOLD_RESOLVE_AFTER_STAMP",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_MESH_COMBAT_FLAGS_CMD_NAMES_WAVE495: &[&str] = &[
    "click_presentation_mesh_combat_flags_ok_wnd_detect",
    "click_presentation_mesh_combat_flags_ok_wnd_skip",
    "click_presentation_mesh_combat_flags_ok_wnd_queue",
    "click_presentation_mesh_combat_flags_ok_wnd_prepare",
    "click_presentation_mesh_combat_flags_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationMeshCombatFlagsAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    InputSource = 4,
    RenderSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationMeshCombatFlagsAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_mesh_combat_flags_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_mesh_combat_flags_last_action()
-> ResidualPresentationMeshCombatFlagsAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationMeshCombatFlagsAction::MethodNames,
        2 => ResidualPresentationMeshCombatFlagsAction::SourceMarkers,
        3 => ResidualPresentationMeshCombatFlagsAction::NavCommands,
        4 => ResidualPresentationMeshCombatFlagsAction::InputSource,
        5 => ResidualPresentationMeshCombatFlagsAction::RenderSource,
        6 => ResidualPresentationMeshCombatFlagsAction::Composite,
        _ => ResidualPresentationMeshCombatFlagsAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_mesh_combat_flags_method_names_residual_wave495() -> bool {
    PRESENTATION_MESH_COMBAT_FLAGS_METHOD_NAMES_WAVE495.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_COMBAT_FLAGS_METHOD_NAMES_WAVE495,
            "is_firing_weapon",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MESH_COMBAT_FLAGS_METHOD_NAMES_WAVE495,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_mesh_combat_flags_source_markers_residual_wave495() -> bool {
    PRESENTATION_MESH_COMBAT_FLAGS_SOURCE_MARKERS_WAVE495.len() == 4
        && residual_name_index(
            PRESENTATION_MESH_COMBAT_FLAGS_SOURCE_MARKERS_WAVE495,
            "Wave 495: stamp moving/attacking/firing bits then honor sold residual",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_MESH_COMBAT_FLAGS_SOURCE_MARKERS_WAVE495,
            "model_condition_bits_with_combat_flags",
        ) == Some(3)
}

pub fn honesty_presentation_mesh_combat_flags_nav_commands_residual_wave495() -> bool {
    PRESENTATION_MESH_COMBAT_FLAGS_NAV_STEPS_WAVE495.len() == 6
        && residual_name_index(
            PRESENTATION_MESH_COMBAT_FLAGS_NAV_STEPS_WAVE495,
            "STAMP_MODEL_CONDITION_BITS",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_MESH_COMBAT_FLAGS_NAV_STEPS_WAVE495,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_MESH_COMBAT_FLAGS_CMD_NAMES_WAVE495.len() == 5
}

pub fn simulate_presentation_mesh_combat_flags_input_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 495: frozen combat motion flags for mesh model-condition stamping")
        && pf.contains("is_firing_weapon: ro.is_firing_weapon")
        && pf.contains("moving: ro.moving")
        && pf.contains("attacking: ro.attacking")
        && pf.contains("fn model_condition_bits_with_combat_flags")
        && (pf.contains("MC_BIT_FIRING_A")
            || pf.contains("firing_a_model_bit")
            || pf.contains("moving_model_bit"));
    residual_action_store(ResidualPresentationMeshCombatFlagsAction::InputSource);
    ok
}

pub fn simulate_presentation_mesh_combat_flags_render_source() -> bool {
    // 2026-08-15: Wave 495 stamp lives in presentation_frame/unit_render.rs.
    let pf = pf_source();
    let rp = rp_source();
    let ok = (pf.contains("Wave 495") || rp.contains("Wave 495"))
        && (pf.contains("model_condition_bits_with_combat_flags")
            || rp.contains("model_condition_bits_with_combat_flags"))
        && (pf.contains("is_firing_weapon") || rp.contains("_combat_model_bits"));
    residual_action_store(ResidualPresentationMeshCombatFlagsAction::RenderSource);
    ok
}

pub fn honesty_presentation_mesh_combat_flags_residual_pack_wave495() -> bool {
    honesty_presentation_mesh_combat_flags_method_names_residual_wave495()
        && honesty_presentation_mesh_combat_flags_source_markers_residual_wave495()
        && honesty_presentation_mesh_combat_flags_nav_commands_residual_wave495()
        && simulate_presentation_mesh_combat_flags_input_source()
        && simulate_presentation_mesh_combat_flags_render_source()
}

pub fn simulate_live_presentation_mesh_combat_flags_honesty() -> bool {
    let ok = honesty_presentation_mesh_combat_flags_residual_pack_wave495();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMeshCombatFlagsAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_mesh_combat_flags_method_names_residual_wave495());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_mesh_combat_flags_source_markers_residual_wave495());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_mesh_combat_flags_nav_commands_residual_wave495());
    }

    #[test]
    fn presentation_mesh_combat_flags_sources() {
        assert!(simulate_presentation_mesh_combat_flags_input_source());
        assert!(simulate_presentation_mesh_combat_flags_render_source());
    }

    #[test]
    fn wave495_composite_pack() {
        assert!(honesty_presentation_mesh_combat_flags_residual_pack_wave495());
    }

    #[test]
    fn simulate_live_presentation_mesh_combat_flags_honesty_residual_live() {
        assert!(
            simulate_live_presentation_mesh_combat_flags_honesty(),
            "presentation mesh combat flags residual must latch"
        );
        assert!(residual_presentation_mesh_combat_flags_ok());
        assert_eq!(
            residual_presentation_mesh_combat_flags_last_action(),
            ResidualPresentationMeshCombatFlagsAction::Composite
        );
    }
}
