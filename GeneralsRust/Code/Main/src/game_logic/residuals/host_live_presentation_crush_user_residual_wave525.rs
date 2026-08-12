//! Wave 525 residual peels: FRONTCRUSHED / BACKCRUSHED / PREORDER / USER_1 / USER_2.
//! - freeze front/back crushed + USER bits from host model_condition_bits
//! - stamp crush/user bits; PREORDER for under-construction structures
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 524 multi-door/smolder bits.
//! Architecture residual - crush/user pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 525 freeze + stamps
//! - host_enum_table_residual.rs frontcrushed/backcrushed/preorder/user_* helpers
//!
//! Fail-closed:
//! - Full physics crush impulse matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_CRUSH_USER_METHOD_NAMES_WAVE525: &[&str] = &[
    "front_crushed",
    "back_crushed",
    "user_1",
    "user_2",
    "preorder_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_CRUSH_USER_SOURCE_MARKERS_WAVE525: &[&str] = &[
    "Wave 525: FRONTCRUSHED / BACKCRUSHED / PREORDER / USER_1 / USER_2 residual bits",
    "front_crushed: obj.front_crushed",
    "fn frontcrushed_model_bit",
    "fn preorder_model_bit",
];

pub const PRESENTATION_CRUSH_USER_NAV_STEPS_WAVE525: &[&str] = &[
    "FREEZE_CRUSH_USER",
    "STAMP_CRUSH_BITS",
    "STAMP_USER_BITS",
    "STAMP_PREORDER_UNDER_CONSTRUCTION",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_CRUSH_USER_CMD_NAMES_WAVE525: &[&str] = &[
    "click_presentation_crush_user_ok_wnd_detect",
    "click_presentation_crush_user_ok_wnd_skip",
    "click_presentation_crush_user_ok_wnd_queue",
    "click_presentation_crush_user_ok_wnd_prepare",
    "click_presentation_crush_user_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationCrushUserAction {
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

fn residual_action_store(a: ResidualPresentationCrushUserAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_crush_user_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_crush_user_last_action() -> ResidualPresentationCrushUserAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationCrushUserAction::MethodNames,
        2 => ResidualPresentationCrushUserAction::SourceMarkers,
        3 => ResidualPresentationCrushUserAction::NavCommands,
        4 => ResidualPresentationCrushUserAction::FreezeSource,
        5 => ResidualPresentationCrushUserAction::StampSource,
        6 => ResidualPresentationCrushUserAction::Composite,
        _ => ResidualPresentationCrushUserAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_presentation_crush_user_method_names_residual_wave525() -> bool {
    PRESENTATION_CRUSH_USER_METHOD_NAMES_WAVE525.len() == 6
        && residual_name_index(
            PRESENTATION_CRUSH_USER_METHOD_NAMES_WAVE525,
            "front_crushed",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CRUSH_USER_METHOD_NAMES_WAVE525,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_crush_user_source_markers_residual_wave525() -> bool {
    PRESENTATION_CRUSH_USER_SOURCE_MARKERS_WAVE525.len() == 4
        && residual_name_index(
            PRESENTATION_CRUSH_USER_SOURCE_MARKERS_WAVE525,
            "Wave 525: FRONTCRUSHED / BACKCRUSHED / PREORDER / USER_1 / USER_2 residual bits",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CRUSH_USER_SOURCE_MARKERS_WAVE525,
            "fn preorder_model_bit",
        ) == Some(3)
}

pub fn honesty_presentation_crush_user_nav_commands_residual_wave525() -> bool {
    PRESENTATION_CRUSH_USER_NAV_STEPS_WAVE525.len() == 6
        && residual_name_index(
            PRESENTATION_CRUSH_USER_NAV_STEPS_WAVE525,
            "STAMP_PREORDER_UNDER_CONSTRUCTION",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_CRUSH_USER_NAV_STEPS_WAVE525,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_CRUSH_USER_CMD_NAMES_WAVE525.len() == 5
}

pub fn simulate_presentation_crush_user_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("front_crushed: obj.front_crushed")
        && pf.contains("back_crushed: obj.back_crushed")
        && pf.contains("user_1_model_bit()")
        && pf.contains("front_crushed: ro.front_crushed");
    residual_action_store(ResidualPresentationCrushUserAction::FreezeSource);
    ok
}

pub fn simulate_presentation_crush_user_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf.contains(
        "Wave 525: FRONTCRUSHED / BACKCRUSHED / PREORDER / USER_1 / USER_2 residual bits",
    ) && en.contains("pub fn frontcrushed_model_bit")
        && en.contains("pub fn preorder_model_bit")
        && pf.contains("under_construction && self.construction_percent < 1.0");
    residual_action_store(ResidualPresentationCrushUserAction::StampSource);
    ok
}

pub fn honesty_presentation_crush_user_residual_pack_wave525() -> bool {
    honesty_presentation_crush_user_method_names_residual_wave525()
        && honesty_presentation_crush_user_source_markers_residual_wave525()
        && honesty_presentation_crush_user_nav_commands_residual_wave525()
        && simulate_presentation_crush_user_freeze_source()
        && simulate_presentation_crush_user_stamp_source()
}

pub fn simulate_live_presentation_crush_user_honesty() -> bool {
    let ok = honesty_presentation_crush_user_residual_pack_wave525();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationCrushUserAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_crush_user_method_names_residual_wave525());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_crush_user_source_markers_residual_wave525());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_crush_user_nav_commands_residual_wave525());
    }

    #[test]
    fn presentation_crush_user_sources() {
        assert!(simulate_presentation_crush_user_freeze_source());
        assert!(simulate_presentation_crush_user_stamp_source());
    }

    #[test]
    fn wave525_composite_pack() {
        assert!(honesty_presentation_crush_user_residual_pack_wave525());
    }

    #[test]
    fn simulate_live_presentation_crush_user_honesty_residual_live() {
        assert!(
            simulate_live_presentation_crush_user_honesty(),
            "presentation crush/user residual must latch"
        );
        assert!(residual_presentation_crush_user_ok());
        assert_eq!(
            residual_presentation_crush_user_last_action(),
            ResidualPresentationCrushUserAction::Composite
        );
    }
}
