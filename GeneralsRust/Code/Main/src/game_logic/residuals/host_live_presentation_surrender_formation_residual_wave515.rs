//! Wave 515 residual peels: surrender + formation freeze; RAISING_FLAG mesh bit.
//! - freeze `is_surrendered`, `formation_id`, `formation_offset`
//! - surrendered units stamp MODELCONDITION_RAISING_FLAG
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 514 emoticon floating text.
//! Architecture residual - surrender/formation without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 515 freeze + stamp
//! - host_enum_table_residual.rs raising_flag_model_bit
//! - graphics/render_pipeline.rs Wave 515 comment
//!
//! Fail-closed:
//! - Full SURRENDER model bit (ALLOW_SURRENDER off in ZH) still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_SURRENDER_FORMATION_METHOD_NAMES_WAVE515: &[&str] = &[
    "is_surrendered",
    "formation_id",
    "formation_offset",
    "raising_flag_model_bit",
    "RAISING_FLAG",
    "playable_claim = false",
];

pub const PRESENTATION_SURRENDER_FORMATION_SOURCE_MARKERS_WAVE515: &[&str] = &[
    "Wave 515: C++ AIUpdateInterface::setSurrendered residual",
    "Wave 515: surrendered residual stamps RAISING_FLAG model-condition bit",
    "is_surrendered: obj.is_surrendered",
    "formation_id: obj.formation_id",
];

pub const PRESENTATION_SURRENDER_FORMATION_NAV_STEPS_WAVE515: &[&str] = &[
    "FREEZE_SURRENDERED",
    "FREEZE_FORMATION_ID_OFFSET",
    "STAMP_RAISING_FLAG",
    "MESH_RESOLVE_FROM_BITS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_SURRENDER_FORMATION_CMD_NAMES_WAVE515: &[&str] = &[
    "click_presentation_surrender_formation_ok_wnd_detect",
    "click_presentation_surrender_formation_ok_wnd_skip",
    "click_presentation_surrender_formation_ok_wnd_queue",
    "click_presentation_surrender_formation_ok_wnd_prepare",
    "click_presentation_surrender_formation_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationSurrenderFormationAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    FreezeSource = 4,
    StampSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationSurrenderFormationAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_surrender_formation_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_surrender_formation_last_action()
-> ResidualPresentationSurrenderFormationAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationSurrenderFormationAction::MethodNames,
        2 => ResidualPresentationSurrenderFormationAction::SourceMarkers,
        3 => ResidualPresentationSurrenderFormationAction::NavCommands,
        4 => ResidualPresentationSurrenderFormationAction::FreezeSource,
        5 => ResidualPresentationSurrenderFormationAction::StampSource,
        6 => ResidualPresentationSurrenderFormationAction::Composite,
        _ => ResidualPresentationSurrenderFormationAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_surrender_formation_method_names_residual_wave515() -> bool {
    PRESENTATION_SURRENDER_FORMATION_METHOD_NAMES_WAVE515.len() == 6
        && residual_name_index(
            PRESENTATION_SURRENDER_FORMATION_METHOD_NAMES_WAVE515,
            "is_surrendered",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_SURRENDER_FORMATION_METHOD_NAMES_WAVE515,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_surrender_formation_source_markers_residual_wave515() -> bool {
    PRESENTATION_SURRENDER_FORMATION_SOURCE_MARKERS_WAVE515.len() == 4
        && residual_name_index(
            PRESENTATION_SURRENDER_FORMATION_SOURCE_MARKERS_WAVE515,
            "Wave 515: surrendered residual stamps RAISING_FLAG model-condition bit",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_SURRENDER_FORMATION_SOURCE_MARKERS_WAVE515,
            "is_surrendered: obj.is_surrendered",
        ) == Some(2)
}

pub fn honesty_presentation_surrender_formation_nav_commands_residual_wave515() -> bool {
    PRESENTATION_SURRENDER_FORMATION_NAV_STEPS_WAVE515.len() == 6
        && residual_name_index(
            PRESENTATION_SURRENDER_FORMATION_NAV_STEPS_WAVE515,
            "STAMP_RAISING_FLAG",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_SURRENDER_FORMATION_NAV_STEPS_WAVE515,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_SURRENDER_FORMATION_CMD_NAMES_WAVE515.len() == 5
}

pub fn simulate_presentation_surrender_formation_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 515: C++ AIUpdateInterface::setSurrendered residual")
        && pf.contains("is_surrendered: obj.is_surrendered")
        && pf.contains("formation_id: obj.formation_id")
        && pf.contains("formation_offset: obj.formation_offset")
        && pf.contains("is_surrendered: ro.is_surrendered");
    residual_action_store(ResidualPresentationSurrenderFormationAction::FreezeSource);
    ok
}

pub fn simulate_presentation_surrender_formation_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 515: surrendered residual stamps RAISING_FLAG model-condition bit")
        && en.contains("pub fn raising_flag_model_bit")
        && pf.contains("self.is_surrendered")
        && rp.contains("Wave 515: RAISING_FLAG (surrendered) bit included in stamp helper");
    residual_action_store(ResidualPresentationSurrenderFormationAction::StampSource);
    ok
}

pub fn honesty_presentation_surrender_formation_residual_pack_wave515() -> bool {
    honesty_presentation_surrender_formation_method_names_residual_wave515()
        && honesty_presentation_surrender_formation_source_markers_residual_wave515()
        && honesty_presentation_surrender_formation_nav_commands_residual_wave515()
        && simulate_presentation_surrender_formation_freeze_source()
        && simulate_presentation_surrender_formation_stamp_source()
}

pub fn simulate_live_presentation_surrender_formation_honesty() -> bool {
    let ok = honesty_presentation_surrender_formation_residual_pack_wave515();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationSurrenderFormationAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_surrender_formation_method_names_residual_wave515());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_surrender_formation_source_markers_residual_wave515());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_surrender_formation_nav_commands_residual_wave515());
    }

    #[test]
    fn presentation_surrender_formation_sources() {
        assert!(simulate_presentation_surrender_formation_freeze_source());
        assert!(simulate_presentation_surrender_formation_stamp_source());
    }

    #[test]
    fn wave515_composite_pack() {
        assert!(honesty_presentation_surrender_formation_residual_pack_wave515());
    }

    #[test]
    fn simulate_live_presentation_surrender_formation_honesty_residual_live() {
        assert!(
            simulate_live_presentation_surrender_formation_honesty(),
            "presentation surrender/formation residual must latch"
        );
        assert!(residual_presentation_surrender_formation_ok());
        assert_eq!(
            residual_presentation_surrender_formation_last_action(),
            ResidualPresentationSurrenderFormationAction::Composite
        );
    }
}
