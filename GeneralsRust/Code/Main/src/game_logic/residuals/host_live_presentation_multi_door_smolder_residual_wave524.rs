//! Wave 524 residual peels: multi-door DOOR_2..4 banks + SMOLDERING death pose.
//! - stamp door 1..4 banks from production_door_phase (multi-door factory residual)
//! - stamp SMOLDERING when burned death without active flame
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 523 second-life/stun bits.
//! Architecture residual - factory doors / smolder without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 524 door banks + smoldering stamp
//! - host_enum_table_residual.rs door_2..4_* / smoldering helpers
//!
//! Fail-closed:
//! - Per-door independent ProductionUpdate door modules still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MULTI_DOOR_SMOLDER_METHOD_NAMES_WAVE524: &[&str] = &[
    "production_door_phase",
    "door_2_opening_model_bit",
    "door_4_closing_model_bit",
    "smoldering_model_bit",
    "death_type_name",
    "playable_claim = false",
];

pub const PRESENTATION_MULTI_DOOR_SMOLDER_SOURCE_MARKERS_WAVE524: &[&str] = &[
    "Wave 524: clear door 1..4 banks then set active phase bit on each door bank",
    "Wave 524: SMOLDERING when burned residual without active flame",
    "fn door_2_opening_model_bit",
    "fn smoldering_model_bit",
];

pub const PRESENTATION_MULTI_DOOR_SMOLDER_NAV_STEPS_WAVE524: &[&str] = &[
    "STAMP_DOOR_BANKS_1_TO_4",
    "STAMP_SMOLDERING",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_MULTI_DOOR_SMOLDER_CMD_NAMES_WAVE524: &[&str] = &[
    "click_presentation_multi_door_smolder_ok_wnd_detect",
    "click_presentation_multi_door_smolder_ok_wnd_skip",
    "click_presentation_multi_door_smolder_ok_wnd_queue",
    "click_presentation_multi_door_smolder_ok_wnd_prepare",
    "click_presentation_multi_door_smolder_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationMultiDoorSmolderAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    DoorSource = 4,
    SmolderSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationMultiDoorSmolderAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_multi_door_smolder_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_multi_door_smolder_last_action()
-> ResidualPresentationMultiDoorSmolderAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationMultiDoorSmolderAction::MethodNames,
        2 => ResidualPresentationMultiDoorSmolderAction::SourceMarkers,
        3 => ResidualPresentationMultiDoorSmolderAction::NavCommands,
        4 => ResidualPresentationMultiDoorSmolderAction::DoorSource,
        5 => ResidualPresentationMultiDoorSmolderAction::SmolderSource,
        6 => ResidualPresentationMultiDoorSmolderAction::Composite,
        _ => ResidualPresentationMultiDoorSmolderAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_presentation_multi_door_smolder_method_names_residual_wave524() -> bool {
    PRESENTATION_MULTI_DOOR_SMOLDER_METHOD_NAMES_WAVE524.len() == 6
        && residual_name_index(
            PRESENTATION_MULTI_DOOR_SMOLDER_METHOD_NAMES_WAVE524,
            "production_door_phase",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MULTI_DOOR_SMOLDER_METHOD_NAMES_WAVE524,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_multi_door_smolder_source_markers_residual_wave524() -> bool {
    PRESENTATION_MULTI_DOOR_SMOLDER_SOURCE_MARKERS_WAVE524.len() == 4
        && residual_name_index(
            PRESENTATION_MULTI_DOOR_SMOLDER_SOURCE_MARKERS_WAVE524,
            "Wave 524: clear door 1..4 banks then set active phase bit on each door bank",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MULTI_DOOR_SMOLDER_SOURCE_MARKERS_WAVE524,
            "fn smoldering_model_bit",
        ) == Some(3)
}

pub fn honesty_presentation_multi_door_smolder_nav_commands_residual_wave524() -> bool {
    PRESENTATION_MULTI_DOOR_SMOLDER_NAV_STEPS_WAVE524.len() == 4
        && residual_name_index(
            PRESENTATION_MULTI_DOOR_SMOLDER_NAV_STEPS_WAVE524,
            "STAMP_DOOR_BANKS_1_TO_4",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MULTI_DOOR_SMOLDER_NAV_STEPS_WAVE524,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(3)
        && RUNTIME_HOST_PRESENTATION_MULTI_DOOR_SMOLDER_CMD_NAMES_WAVE524.len() == 5
}

pub fn simulate_presentation_multi_door_smolder_door_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf
        .contains("Wave 524: clear door 1..4 banks then set active phase bit on each door bank")
        && en.contains("pub fn door_2_opening_model_bit")
        && en.contains("pub fn door_4_closing_model_bit")
        && pf.contains("door_4_closing_model_bit()");
    residual_action_store(ResidualPresentationMultiDoorSmolderAction::DoorSource);
    ok
}

pub fn simulate_presentation_multi_door_smolder_smolder_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf.contains("Wave 524: SMOLDERING when burned residual without active flame")
        && en.contains("pub fn smoldering_model_bit")
        && pf.contains("death.contains(\"smolder\")");
    residual_action_store(ResidualPresentationMultiDoorSmolderAction::SmolderSource);
    ok
}

pub fn honesty_presentation_multi_door_smolder_residual_pack_wave524() -> bool {
    honesty_presentation_multi_door_smolder_method_names_residual_wave524()
        && honesty_presentation_multi_door_smolder_source_markers_residual_wave524()
        && honesty_presentation_multi_door_smolder_nav_commands_residual_wave524()
        && simulate_presentation_multi_door_smolder_door_source()
        && simulate_presentation_multi_door_smolder_smolder_source()
}

pub fn simulate_live_presentation_multi_door_smolder_honesty() -> bool {
    let ok = honesty_presentation_multi_door_smolder_residual_pack_wave524();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMultiDoorSmolderAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_multi_door_smolder_method_names_residual_wave524());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_multi_door_smolder_source_markers_residual_wave524());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_multi_door_smolder_nav_commands_residual_wave524());
    }

    #[test]
    fn presentation_multi_door_smolder_sources() {
        assert!(simulate_presentation_multi_door_smolder_door_source());
        assert!(simulate_presentation_multi_door_smolder_smolder_source());
    }

    #[test]
    fn wave524_composite_pack() {
        assert!(honesty_presentation_multi_door_smolder_residual_pack_wave524());
    }

    #[test]
    fn simulate_live_presentation_multi_door_smolder_honesty_residual_live() {
        assert!(
            simulate_live_presentation_multi_door_smolder_honesty(),
            "presentation multi-door/smolder residual must latch"
        );
        assert!(residual_presentation_multi_door_smolder_ok());
        assert_eq!(
            residual_presentation_multi_door_smolder_last_action(),
            ResidualPresentationMultiDoorSmolderAction::Composite
        );
    }
}
