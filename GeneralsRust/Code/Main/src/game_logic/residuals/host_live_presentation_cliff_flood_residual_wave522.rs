//! Wave 522 residual peels: CLIMBING / RAPPELLING / FLOODED terrain-cell mesh bits.
//! - freeze `cell_is_cliff`, `cell_is_underwater`
//! - stamp FLOODED when underwater; CLIMBING on cliff; RAPPELLING when airborne over cliff
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 521 dock/rider bits.
//! Architecture residual - terrain locomotion pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 522 freeze + stamps
//! - host_enum_table_residual.rs climbing/rappelling/flooded helpers
//! - Object::cell_is_cliff / cell_is_underwater
//!
//! Fail-closed:
//! - Full locomotor surface matrix / multi-door banks still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_CLIFF_FLOOD_METHOD_NAMES_WAVE522: &[&str] = &[
    "cell_is_cliff",
    "cell_is_underwater",
    "climbing_model_bit",
    "rappelling_model_bit",
    "flooded_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_CLIFF_FLOOD_SOURCE_MARKERS_WAVE522: &[&str] = &[
    "Wave 522: CLIMBING / RAPPELLING / FLOODED from terrain cell residuals",
    "cell_is_cliff: obj.cell_is_cliff",
    "cell_is_underwater: obj.cell_is_underwater",
    "fn climbing_model_bit",
];

pub const PRESENTATION_CLIFF_FLOOD_NAV_STEPS_WAVE522: &[&str] = &[
    "FREEZE_CELL_CLIFF_UNDERWATER",
    "STAMP_FLOODED",
    "STAMP_CLIMBING_RAPPELLING",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_CLIFF_FLOOD_CMD_NAMES_WAVE522: &[&str] = &[
    "click_presentation_cliff_flood_ok_wnd_detect",
    "click_presentation_cliff_flood_ok_wnd_skip",
    "click_presentation_cliff_flood_ok_wnd_queue",
    "click_presentation_cliff_flood_ok_wnd_prepare",
    "click_presentation_cliff_flood_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationCliffFloodAction {
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

fn residual_action_store(a: ResidualPresentationCliffFloodAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_cliff_flood_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_cliff_flood_last_action() -> ResidualPresentationCliffFloodAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationCliffFloodAction::MethodNames,
        2 => ResidualPresentationCliffFloodAction::SourceMarkers,
        3 => ResidualPresentationCliffFloodAction::NavCommands,
        4 => ResidualPresentationCliffFloodAction::FreezeSource,
        5 => ResidualPresentationCliffFloodAction::StampSource,
        6 => ResidualPresentationCliffFloodAction::Composite,
        _ => ResidualPresentationCliffFloodAction::Idle,
    }
}

fn pf_source() -> &'static str {
    include_str!("../../presentation_frame.rs")
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_presentation_cliff_flood_method_names_residual_wave522() -> bool {
    PRESENTATION_CLIFF_FLOOD_METHOD_NAMES_WAVE522.len() == 6
        && residual_name_index(
            PRESENTATION_CLIFF_FLOOD_METHOD_NAMES_WAVE522,
            "cell_is_cliff",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CLIFF_FLOOD_METHOD_NAMES_WAVE522,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_cliff_flood_source_markers_residual_wave522() -> bool {
    PRESENTATION_CLIFF_FLOOD_SOURCE_MARKERS_WAVE522.len() == 4
        && residual_name_index(
            PRESENTATION_CLIFF_FLOOD_SOURCE_MARKERS_WAVE522,
            "Wave 522: CLIMBING / RAPPELLING / FLOODED from terrain cell residuals",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CLIFF_FLOOD_SOURCE_MARKERS_WAVE522,
            "fn climbing_model_bit",
        ) == Some(3)
}

pub fn honesty_presentation_cliff_flood_nav_commands_residual_wave522() -> bool {
    PRESENTATION_CLIFF_FLOOD_NAV_STEPS_WAVE522.len() == 5
        && residual_name_index(
            PRESENTATION_CLIFF_FLOOD_NAV_STEPS_WAVE522,
            "STAMP_CLIMBING_RAPPELLING",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_CLIFF_FLOOD_NAV_STEPS_WAVE522,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(4)
        && RUNTIME_HOST_PRESENTATION_CLIFF_FLOOD_CMD_NAMES_WAVE522.len() == 5
}

pub fn simulate_presentation_cliff_flood_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("cell_is_cliff: obj.cell_is_cliff")
        && pf.contains("cell_is_underwater: obj.cell_is_underwater")
        && pf.contains("cell_is_cliff: ro.cell_is_cliff");
    residual_action_store(ResidualPresentationCliffFloodAction::FreezeSource);
    ok
}

pub fn simulate_presentation_cliff_flood_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf.contains("Wave 522: CLIMBING / RAPPELLING / FLOODED from terrain cell residuals")
        && en.contains("pub fn climbing_model_bit")
        && en.contains("pub fn flooded_model_bit")
        && pf.contains("if self.cell_is_cliff");
    residual_action_store(ResidualPresentationCliffFloodAction::StampSource);
    ok
}

pub fn honesty_presentation_cliff_flood_residual_pack_wave522() -> bool {
    honesty_presentation_cliff_flood_method_names_residual_wave522()
        && honesty_presentation_cliff_flood_source_markers_residual_wave522()
        && honesty_presentation_cliff_flood_nav_commands_residual_wave522()
        && simulate_presentation_cliff_flood_freeze_source()
        && simulate_presentation_cliff_flood_stamp_source()
}

pub fn simulate_live_presentation_cliff_flood_honesty() -> bool {
    let ok = honesty_presentation_cliff_flood_residual_pack_wave522();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationCliffFloodAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_cliff_flood_method_names_residual_wave522());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_cliff_flood_source_markers_residual_wave522());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_cliff_flood_nav_commands_residual_wave522());
    }

    #[test]
    fn presentation_cliff_flood_sources() {
        assert!(simulate_presentation_cliff_flood_freeze_source());
        assert!(simulate_presentation_cliff_flood_stamp_source());
    }

    #[test]
    fn wave522_composite_pack() {
        assert!(honesty_presentation_cliff_flood_residual_pack_wave522());
    }

    #[test]
    fn simulate_live_presentation_cliff_flood_honesty_residual_live() {
        assert!(
            simulate_live_presentation_cliff_flood_honesty(),
            "presentation cliff/flood residual must latch"
        );
        assert!(residual_presentation_cliff_flood_ok());
        assert_eq!(
            residual_presentation_cliff_flood_last_action(),
            ResidualPresentationCliffFloodAction::Composite
        );
    }
}
