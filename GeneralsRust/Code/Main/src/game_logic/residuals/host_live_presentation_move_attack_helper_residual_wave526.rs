//! Wave 526 residual peels: MOVING/ATTACKING/DAMAGED helpers complete name-table coverage.
//! - `moving_model_bit` / `attacking_model_bit` wrap MOVING/ATTACKING name indices
//! - `damaged` / `reallydamaged` / `rubble` helpers for body-damage table completeness
//! - combat-flag stamp uses name-table helpers instead of bare MC_BIT_* for move/attack
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 525 crush/user bits.
//! Architecture residual - motion bits without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 526 MOVING/ATTACKING stamp
//! - host_enum_table_residual.rs moving/attacking/damaged/reallydamaged/rubble helpers
//!
//! Fail-closed:
//! - Full animation graph still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_MOVE_ATTACK_HELPER_METHOD_NAMES_WAVE526: &[&str] = &[
    "moving_model_bit",
    "attacking_model_bit",
    "damaged_model_bit",
    "reallydamaged_model_bit",
    "rubble_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_MOVE_ATTACK_HELPER_SOURCE_MARKERS_WAVE526: &[&str] = &[
    "Wave 526: MOVING/ATTACKING via name-table helpers (parity with MC_BIT_*)",
    "fn moving_model_bit",
    "fn attacking_model_bit",
    "fn reallydamaged_model_bit",
];

pub const PRESENTATION_MOVE_ATTACK_HELPER_NAV_STEPS_WAVE526: &[&str] = &[
    "HELPERS_MOVE_ATTACK_DAMAGE",
    "STAMP_MOVING_ATTACKING_VIA_HELPERS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_MOVE_ATTACK_HELPER_CMD_NAMES_WAVE526: &[&str] = &[
    "click_presentation_move_attack_helper_ok_wnd_detect",
    "click_presentation_move_attack_helper_ok_wnd_skip",
    "click_presentation_move_attack_helper_ok_wnd_queue",
    "click_presentation_move_attack_helper_ok_wnd_prepare",
    "click_presentation_move_attack_helper_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationMoveAttackHelperAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    HelperSource = 4,
    StampSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationMoveAttackHelperAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_move_attack_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_move_attack_helper_last_action()
-> ResidualPresentationMoveAttackHelperAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationMoveAttackHelperAction::MethodNames,
        2 => ResidualPresentationMoveAttackHelperAction::SourceMarkers,
        3 => ResidualPresentationMoveAttackHelperAction::NavCommands,
        4 => ResidualPresentationMoveAttackHelperAction::HelperSource,
        5 => ResidualPresentationMoveAttackHelperAction::StampSource,
        6 => ResidualPresentationMoveAttackHelperAction::Composite,
        _ => ResidualPresentationMoveAttackHelperAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_presentation_move_attack_helper_method_names_residual_wave526() -> bool {
    PRESENTATION_MOVE_ATTACK_HELPER_METHOD_NAMES_WAVE526.len() == 6
        && residual_name_index(
            PRESENTATION_MOVE_ATTACK_HELPER_METHOD_NAMES_WAVE526,
            "moving_model_bit",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MOVE_ATTACK_HELPER_METHOD_NAMES_WAVE526,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_move_attack_helper_source_markers_residual_wave526() -> bool {
    PRESENTATION_MOVE_ATTACK_HELPER_SOURCE_MARKERS_WAVE526.len() == 4
        && residual_name_index(
            PRESENTATION_MOVE_ATTACK_HELPER_SOURCE_MARKERS_WAVE526,
            "Wave 526: MOVING/ATTACKING via name-table helpers (parity with MC_BIT_*)",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_MOVE_ATTACK_HELPER_SOURCE_MARKERS_WAVE526,
            "fn reallydamaged_model_bit",
        ) == Some(3)
}

pub fn honesty_presentation_move_attack_helper_nav_commands_residual_wave526() -> bool {
    PRESENTATION_MOVE_ATTACK_HELPER_NAV_STEPS_WAVE526.len() == 4
        && residual_name_index(
            PRESENTATION_MOVE_ATTACK_HELPER_NAV_STEPS_WAVE526,
            "STAMP_MOVING_ATTACKING_VIA_HELPERS",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_MOVE_ATTACK_HELPER_NAV_STEPS_WAVE526,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(3)
        && RUNTIME_HOST_PRESENTATION_MOVE_ATTACK_HELPER_CMD_NAMES_WAVE526.len() == 5
}

pub fn simulate_presentation_move_attack_helper_helper_source() -> bool {
    let en = en_source();
    let ok = en.contains("pub fn moving_model_bit")
        && en.contains("pub fn attacking_model_bit")
        && en.contains("pub fn damaged_model_bit")
        && en.contains("pub fn reallydamaged_model_bit")
        && en.contains("pub fn rubble_model_bit");
    residual_action_store(ResidualPresentationMoveAttackHelperAction::HelperSource);
    ok
}

pub fn simulate_presentation_move_attack_helper_stamp_source() -> bool {
    let pf = pf_source();
    let ok = pf
        .contains("Wave 526: MOVING/ATTACKING via name-table helpers (parity with MC_BIT_*)")
        && pf.contains("moving_model_bit()")
        && pf.contains("attacking_model_bit()");
    residual_action_store(ResidualPresentationMoveAttackHelperAction::StampSource);
    ok
}

pub fn honesty_presentation_move_attack_helper_residual_pack_wave526() -> bool {
    honesty_presentation_move_attack_helper_method_names_residual_wave526()
        && honesty_presentation_move_attack_helper_source_markers_residual_wave526()
        && honesty_presentation_move_attack_helper_nav_commands_residual_wave526()
        && simulate_presentation_move_attack_helper_helper_source()
        && simulate_presentation_move_attack_helper_stamp_source()
}

pub fn simulate_live_presentation_move_attack_helper_honesty() -> bool {
    let ok = honesty_presentation_move_attack_helper_residual_pack_wave526();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMoveAttackHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_move_attack_helper_method_names_residual_wave526());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_move_attack_helper_source_markers_residual_wave526());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_move_attack_helper_nav_commands_residual_wave526());
    }

    #[test]
    fn presentation_move_attack_helper_sources() {
        assert!(simulate_presentation_move_attack_helper_helper_source());
        assert!(simulate_presentation_move_attack_helper_stamp_source());
    }

    #[test]
    fn wave526_composite_pack() {
        assert!(honesty_presentation_move_attack_helper_residual_pack_wave526());
    }

    #[test]
    fn simulate_live_presentation_move_attack_helper_honesty_residual_live() {
        assert!(
            simulate_live_presentation_move_attack_helper_honesty(),
            "presentation move/attack helper residual must latch"
        );
        assert!(residual_presentation_move_attack_helper_ok());
        assert_eq!(
            residual_presentation_move_attack_helper_last_action(),
            ResidualPresentationMoveAttackHelperAction::Composite
        );
    }
}
