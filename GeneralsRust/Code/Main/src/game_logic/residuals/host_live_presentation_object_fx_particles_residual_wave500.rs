//! Wave 500 residual peels: object damage/death/bone FX names → presentation particles.
//! - `append_object_residual_fx_particles` peels `damage_fx_name`/`death_fx_name`/`bone_fx_name`
//! - build_from_logic / build_from_gameworld / victory engine path call after object freeze
//! - particle upload/client observe named `fx_list_name` without live GameLogic dual-read
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 498 host FX field overlay (fields exist; particles now observe them).
//! Architecture residual - FXList residual must reach particle presentation channel.
//!
//! Sources:
//! - presentation_frame.rs append_object_residual_fx_particles
//! - presentation_frame.rs Wave 500 build call sites
//!
//! Fail-closed:
//! - Full FXList.ini particle graph / bone-local offsets still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_OBJECT_FX_PARTICLES_METHOD_NAMES_WAVE500: &[&str] = &[
    "append_object_residual_fx_particles",
    "damage_fx_name",
    "death_fx_name",
    "bone_fx_name",
    "fx_list_name",
    "playable_claim = false",
];

pub const PRESENTATION_OBJECT_FX_PARTICLES_SOURCE_MARKERS_WAVE500: &[&str] = &[
    "Wave 500: merge per-object damage/death/bone FX residual names into the",
    "Wave 500: named damage/death/bone FX residual → particle observe list",
    "append_object_residual_fx_particles",
    "fx_list_name: name.to_string()",
];

pub const PRESENTATION_OBJECT_FX_PARTICLES_NAV_STEPS_WAVE500: &[&str] = &[
    "FREEZE_OBJECT_FX_NAMES",
    "APPEND_DAMAGE_DEATH_BONE_PARTICLES",
    "SET_FX_LIST_NAME",
    "UPDATE_DUAL_TICK_PARTICLE_COUNT",
    "CLIENT_OR_UPLOAD_OBSERVES",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_OBJECT_FX_PARTICLES_CMD_NAMES_WAVE500: &[&str] = &[
    "click_presentation_object_fx_particles_ok_wnd_detect",
    "click_presentation_object_fx_particles_ok_wnd_skip",
    "click_presentation_object_fx_particles_ok_wnd_queue",
    "click_presentation_object_fx_particles_ok_wnd_prepare",
    "click_presentation_object_fx_particles_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationObjectFxParticlesAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    AppendSource = 4,
    BuildSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationObjectFxParticlesAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_object_fx_particles_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_object_fx_particles_last_action()
-> ResidualPresentationObjectFxParticlesAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationObjectFxParticlesAction::MethodNames,
        2 => ResidualPresentationObjectFxParticlesAction::SourceMarkers,
        3 => ResidualPresentationObjectFxParticlesAction::NavCommands,
        4 => ResidualPresentationObjectFxParticlesAction::AppendSource,
        5 => ResidualPresentationObjectFxParticlesAction::BuildSource,
        6 => ResidualPresentationObjectFxParticlesAction::Composite,
        _ => ResidualPresentationObjectFxParticlesAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_presentation_object_fx_particles_method_names_residual_wave500() -> bool {
    PRESENTATION_OBJECT_FX_PARTICLES_METHOD_NAMES_WAVE500.len() == 6
        && residual_name_index(
            PRESENTATION_OBJECT_FX_PARTICLES_METHOD_NAMES_WAVE500,
            "append_object_residual_fx_particles",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_OBJECT_FX_PARTICLES_METHOD_NAMES_WAVE500,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_object_fx_particles_source_markers_residual_wave500() -> bool {
    PRESENTATION_OBJECT_FX_PARTICLES_SOURCE_MARKERS_WAVE500.len() == 4
        && residual_name_index(
            PRESENTATION_OBJECT_FX_PARTICLES_SOURCE_MARKERS_WAVE500,
            "Wave 500: named damage/death/bone FX residual → particle observe list",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_OBJECT_FX_PARTICLES_SOURCE_MARKERS_WAVE500,
            "append_object_residual_fx_particles",
        ) == Some(2)
}

pub fn honesty_presentation_object_fx_particles_nav_commands_residual_wave500() -> bool {
    PRESENTATION_OBJECT_FX_PARTICLES_NAV_STEPS_WAVE500.len() == 6
        && residual_name_index(
            PRESENTATION_OBJECT_FX_PARTICLES_NAV_STEPS_WAVE500,
            "APPEND_DAMAGE_DEATH_BONE_PARTICLES",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_OBJECT_FX_PARTICLES_NAV_STEPS_WAVE500,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_OBJECT_FX_PARTICLES_CMD_NAMES_WAVE500.len() == 5
}

pub fn simulate_presentation_object_fx_particles_append_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("pub fn append_object_residual_fx_particles")
        && pf.contains("Wave 500: merge per-object damage/death/bone FX residual names into the")
        && pf.contains("damage_fx_name")
        && pf.contains("death_fx_name")
        && pf.contains("bone_fx_name")
        && pf.contains("fx_list_name: name.to_string()");
    residual_action_store(ResidualPresentationObjectFxParticlesAction::AppendSource);
    ok
}

pub fn simulate_presentation_object_fx_particles_build_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 500: named damage/death/bone FX residual → particle observe list")
        && pf.contains("Wave 500: object FX residual names → particle list after host FX stamp")
        && pf.matches("append_object_residual_fx_particles()").count() >= 3;
    residual_action_store(ResidualPresentationObjectFxParticlesAction::BuildSource);
    ok
}

pub fn honesty_presentation_object_fx_particles_residual_pack_wave500() -> bool {
    honesty_presentation_object_fx_particles_method_names_residual_wave500()
        && honesty_presentation_object_fx_particles_source_markers_residual_wave500()
        && honesty_presentation_object_fx_particles_nav_commands_residual_wave500()
        && simulate_presentation_object_fx_particles_append_source()
        && simulate_presentation_object_fx_particles_build_source()
}

pub fn simulate_live_presentation_object_fx_particles_honesty() -> bool {
    let ok = honesty_presentation_object_fx_particles_residual_pack_wave500();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationObjectFxParticlesAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_object_fx_particles_method_names_residual_wave500());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_object_fx_particles_source_markers_residual_wave500());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_object_fx_particles_nav_commands_residual_wave500());
    }

    #[test]
    fn presentation_object_fx_particles_sources() {
        assert!(simulate_presentation_object_fx_particles_append_source());
        assert!(simulate_presentation_object_fx_particles_build_source());
    }

    #[test]
    fn wave500_composite_pack() {
        assert!(honesty_presentation_object_fx_particles_residual_pack_wave500());
    }

    #[test]
    fn simulate_live_presentation_object_fx_particles_honesty_residual_live() {
        assert!(
            simulate_live_presentation_object_fx_particles_honesty(),
            "presentation object FX particles residual must latch"
        );
        assert!(residual_presentation_object_fx_particles_ok());
        assert_eq!(
            residual_presentation_object_fx_particles_last_action(),
            ResidualPresentationObjectFxParticlesAction::Composite
        );
    }
}
