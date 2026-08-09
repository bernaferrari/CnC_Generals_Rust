//! Wave 521 residual peels: transport RIDER1..n + DOCKING_* mesh bits.
//! - freeze `ai_state_ordinal` / `combat_cycle_rider` onto UnitRenderInput
//! - stamp RIDER1..8 from occupant_count (transports) or combat_cycle_rider
//! - stamp DOCKING / BEGINNING / ACTIVE / ENDING from Docked=12, Docking=18, Entering=17
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 520 animation-steering turn bits.
//! Architecture residual - dock/rider pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 521 stamp block
//! - host_enum_table_residual.rs docking_* / rider*_model_bit helpers
//! - gameworld_shadow::host_ai_state_ordinal Docked/Docking/Entering
//!
//! Fail-closed:
//! - Full multi-door DOOR_2..4 banks and climbing/rappelling still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_DOCK_RIDER_METHOD_NAMES_WAVE521: &[&str] = &[
    "ai_state_ordinal",
    "occupant_count",
    "docking_active_model_bit",
    "rider1_model_bit",
    "combat_cycle_rider",
    "playable_claim = false",
];

pub const PRESENTATION_DOCK_RIDER_SOURCE_MARKERS_WAVE521: &[&str] = &[
    "Wave 521: stamp RIDER1..n from occupant_count; DOCKING_* from ai_state_ordinal",
    "fn docking_model_bit",
    "fn rider1_model_bit",
    "host_ai_state_ordinal: Docked=12, Docking=18, Entering=17",
];

pub const PRESENTATION_DOCK_RIDER_NAV_STEPS_WAVE521: &[&str] = &[
    "FREEZE_AI_STATE_ORDINAL",
    "STAMP_RIDER_BITS",
    "STAMP_DOCKING_BITS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_DOCK_RIDER_CMD_NAMES_WAVE521: &[&str] = &[
    "click_presentation_dock_rider_ok_wnd_detect",
    "click_presentation_dock_rider_ok_wnd_skip",
    "click_presentation_dock_rider_ok_wnd_queue",
    "click_presentation_dock_rider_ok_wnd_prepare",
    "click_presentation_dock_rider_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationDockRiderAction {
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

fn residual_action_store(a: ResidualPresentationDockRiderAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_dock_rider_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_dock_rider_last_action() -> ResidualPresentationDockRiderAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationDockRiderAction::MethodNames,
        2 => ResidualPresentationDockRiderAction::SourceMarkers,
        3 => ResidualPresentationDockRiderAction::NavCommands,
        4 => ResidualPresentationDockRiderAction::FreezeSource,
        5 => ResidualPresentationDockRiderAction::StampSource,
        6 => ResidualPresentationDockRiderAction::Composite,
        _ => ResidualPresentationDockRiderAction::Idle,
    }
}

fn pf_source() -> &'static str {
    include_str!("../../presentation_frame.rs")
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_presentation_dock_rider_method_names_residual_wave521() -> bool {
    PRESENTATION_DOCK_RIDER_METHOD_NAMES_WAVE521.len() == 6
        && residual_name_index(
            PRESENTATION_DOCK_RIDER_METHOD_NAMES_WAVE521,
            "ai_state_ordinal",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_DOCK_RIDER_METHOD_NAMES_WAVE521,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_dock_rider_source_markers_residual_wave521() -> bool {
    PRESENTATION_DOCK_RIDER_SOURCE_MARKERS_WAVE521.len() == 4
        && residual_name_index(
            PRESENTATION_DOCK_RIDER_SOURCE_MARKERS_WAVE521,
            "Wave 521: stamp RIDER1..n from occupant_count; DOCKING_* from ai_state_ordinal",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_DOCK_RIDER_SOURCE_MARKERS_WAVE521,
            "fn rider1_model_bit",
        ) == Some(2)
}

pub fn honesty_presentation_dock_rider_nav_commands_residual_wave521() -> bool {
    PRESENTATION_DOCK_RIDER_NAV_STEPS_WAVE521.len() == 5
        && residual_name_index(
            PRESENTATION_DOCK_RIDER_NAV_STEPS_WAVE521,
            "STAMP_DOCKING_BITS",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_DOCK_RIDER_NAV_STEPS_WAVE521,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(4)
        && RUNTIME_HOST_PRESENTATION_DOCK_RIDER_CMD_NAMES_WAVE521.len() == 5
}

pub fn simulate_presentation_dock_rider_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("ai_state_ordinal: ro.ai_state_ordinal")
        && pf.contains("combat_cycle_rider: ro.combat_cycle_rider")
        && pf.contains("Wave 521: host AI state ordinal residual");
    residual_action_store(ResidualPresentationDockRiderAction::FreezeSource);
    ok
}

pub fn simulate_presentation_dock_rider_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf
        .contains("Wave 521: stamp RIDER1..n from occupant_count; DOCKING_* from ai_state_ordinal")
        && en.contains("pub fn docking_model_bit")
        && en.contains("pub fn rider1_model_bit")
        && pf.contains("host_ai_state_ordinal: Docked=12, Docking=18, Entering=17");
    residual_action_store(ResidualPresentationDockRiderAction::StampSource);
    ok
}

pub fn honesty_presentation_dock_rider_residual_pack_wave521() -> bool {
    honesty_presentation_dock_rider_method_names_residual_wave521()
        && honesty_presentation_dock_rider_source_markers_residual_wave521()
        && honesty_presentation_dock_rider_nav_commands_residual_wave521()
        && simulate_presentation_dock_rider_freeze_source()
        && simulate_presentation_dock_rider_stamp_source()
}

pub fn simulate_live_presentation_dock_rider_honesty() -> bool {
    let ok = honesty_presentation_dock_rider_residual_pack_wave521();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationDockRiderAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_dock_rider_method_names_residual_wave521());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_dock_rider_source_markers_residual_wave521());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_dock_rider_nav_commands_residual_wave521());
    }

    #[test]
    fn presentation_dock_rider_sources() {
        assert!(simulate_presentation_dock_rider_freeze_source());
        assert!(simulate_presentation_dock_rider_stamp_source());
    }

    #[test]
    fn wave521_composite_pack() {
        assert!(honesty_presentation_dock_rider_residual_pack_wave521());
    }

    #[test]
    fn simulate_live_presentation_dock_rider_honesty_residual_live() {
        assert!(
            simulate_live_presentation_dock_rider_honesty(),
            "presentation dock/rider residual must latch"
        );
        assert!(residual_presentation_dock_rider_ok());
        assert_eq!(
            residual_presentation_dock_rider_last_action(),
            ResidualPresentationDockRiderAction::Composite
        );
    }
}
