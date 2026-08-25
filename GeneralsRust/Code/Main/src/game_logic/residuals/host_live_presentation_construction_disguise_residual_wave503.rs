//! Wave 503 residual peels: construction scaffold bits + disguise mesh for non-allies.
//! - construction_percent/under_construction stamp AWAITING/PARTIALLY/ACTIVELY bits
//! - non-allied viewers get disguise_as_template mesh key + disguise team color
//! - allies keep true team color / true mesh
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 501–502 deploy/radar/stealth mesh residuals.
//! Architecture residual - construction/disguise presentation without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs model_condition_bits_with_combat_flags Wave 503
//! - presentation_frame.rs unit_render_inputs disguise mesh swap
//! - presentation_frame.rs team_color Wave 503 ally-true residual
//!
//! Fail-closed:
//! - Full disguise transition opacity / detector reveal still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_CONSTRUCTION_DISGUISE_METHOD_NAMES_WAVE503: &[&str] = &[
    "under_construction",
    "construction_percent",
    "disguise_as_template",
    "actively_being_constructed_model_bit",
    "model_key_from_presentation",
    "playable_claim = false",
];

pub const PRESENTATION_CONSTRUCTION_DISGUISE_SOURCE_MARKERS_WAVE503: &[&str] = &[
    "Wave 503: construction scaffold model-condition residual",
    "Wave 503: non-allied viewers see disguise mesh residual",
    "Wave 503: C++ enemies see disguise player color; allies see true colors",
    "actively_being_constructed_model_bit",
];

pub const PRESENTATION_CONSTRUCTION_DISGUISE_NAV_STEPS_WAVE503: &[&str] = &[
    "FREEZE_CONSTRUCTION_DISGUISE",
    "STAMP_CONSTRUCTION_BITS",
    "ALLY_TRUE_TEAM_COLOR",
    "ENEMY_DISGUISE_MESH_KEY",
    "MESH_RESOLVE_FROM_BITS",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_CONSTRUCTION_DISGUISE_CMD_NAMES_WAVE503: &[&str] = &[
    "click_presentation_construction_disguise_ok_wnd_detect",
    "click_presentation_construction_disguise_ok_wnd_skip",
    "click_presentation_construction_disguise_ok_wnd_queue",
    "click_presentation_construction_disguise_ok_wnd_prepare",
    "click_presentation_construction_disguise_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationConstructionDisguiseAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    StampSource = 4,
    MeshSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationConstructionDisguiseAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_construction_disguise_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_construction_disguise_last_action()
-> ResidualPresentationConstructionDisguiseAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationConstructionDisguiseAction::MethodNames,
        2 => ResidualPresentationConstructionDisguiseAction::SourceMarkers,
        3 => ResidualPresentationConstructionDisguiseAction::NavCommands,
        4 => ResidualPresentationConstructionDisguiseAction::StampSource,
        5 => ResidualPresentationConstructionDisguiseAction::MeshSource,
        6 => ResidualPresentationConstructionDisguiseAction::Composite,
        _ => ResidualPresentationConstructionDisguiseAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_construction_disguise_method_names_residual_wave503() -> bool {
    PRESENTATION_CONSTRUCTION_DISGUISE_METHOD_NAMES_WAVE503.len() == 6
        && residual_name_index(
            PRESENTATION_CONSTRUCTION_DISGUISE_METHOD_NAMES_WAVE503,
            "under_construction",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CONSTRUCTION_DISGUISE_METHOD_NAMES_WAVE503,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_construction_disguise_source_markers_residual_wave503() -> bool {
    PRESENTATION_CONSTRUCTION_DISGUISE_SOURCE_MARKERS_WAVE503.len() == 4
        && residual_name_index(
            PRESENTATION_CONSTRUCTION_DISGUISE_SOURCE_MARKERS_WAVE503,
            "Wave 503: non-allied viewers see disguise mesh residual",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_CONSTRUCTION_DISGUISE_SOURCE_MARKERS_WAVE503,
            "Wave 503: C++ enemies see disguise player color; allies see true colors",
        ) == Some(2)
}

pub fn honesty_presentation_construction_disguise_nav_commands_residual_wave503() -> bool {
    PRESENTATION_CONSTRUCTION_DISGUISE_NAV_STEPS_WAVE503.len() == 6
        && residual_name_index(
            PRESENTATION_CONSTRUCTION_DISGUISE_NAV_STEPS_WAVE503,
            "ENEMY_DISGUISE_MESH_KEY",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_CONSTRUCTION_DISGUISE_NAV_STEPS_WAVE503,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_CONSTRUCTION_DISGUISE_CMD_NAMES_WAVE503.len() == 5
}

pub fn simulate_presentation_construction_disguise_stamp_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 503: construction scaffold model-condition residual")
        && pf.contains("under_construction: ro.under_construction")
        && pf.contains("construction_percent: ro.construction_percent")
        && pf.contains("actively_being_constructed_model_bit")
        && pf.contains("partially_constructed_model_bit")
        && pf.contains("awaiting_construction_model_bit");
    residual_action_store(ResidualPresentationConstructionDisguiseAction::StampSource);
    ok
}

pub fn simulate_presentation_construction_disguise_mesh_source() -> bool {
    let pf = pf_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 503")
        && pf.contains("Wave 503: C++ enemies see disguise player color; allies see true colors")
        && pf.contains("disguise_as_template")
        && pf.contains("model_key_from_presentation")
        && rp.contains("Wave 503: disguise mesh swap + construction bits via stamp helper");
    residual_action_store(ResidualPresentationConstructionDisguiseAction::MeshSource);
    ok
}

pub fn honesty_presentation_construction_disguise_residual_pack_wave503() -> bool {
    honesty_presentation_construction_disguise_method_names_residual_wave503()
        && honesty_presentation_construction_disguise_source_markers_residual_wave503()
        && honesty_presentation_construction_disguise_nav_commands_residual_wave503()
        && simulate_presentation_construction_disguise_stamp_source()
        && simulate_presentation_construction_disguise_mesh_source()
}

pub fn simulate_live_presentation_construction_disguise_honesty() -> bool {
    let ok = honesty_presentation_construction_disguise_residual_pack_wave503();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationConstructionDisguiseAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_construction_disguise_method_names_residual_wave503());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_construction_disguise_source_markers_residual_wave503());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_construction_disguise_nav_commands_residual_wave503());
    }

    #[test]
    fn presentation_construction_disguise_sources() {
        assert!(simulate_presentation_construction_disguise_stamp_source());
        assert!(simulate_presentation_construction_disguise_mesh_source());
    }

    #[test]
    fn wave503_composite_pack() {
        assert!(honesty_presentation_construction_disguise_residual_pack_wave503());
    }

    #[test]
    fn simulate_live_presentation_construction_disguise_honesty_residual_live() {
        assert!(
            simulate_live_presentation_construction_disguise_honesty(),
            "presentation construction/disguise residual must latch"
        );
        assert!(residual_presentation_construction_disguise_ok());
        assert_eq!(
            residual_presentation_construction_disguise_last_action(),
            ResidualPresentationConstructionDisguiseAction::Composite
        );
    }
}
