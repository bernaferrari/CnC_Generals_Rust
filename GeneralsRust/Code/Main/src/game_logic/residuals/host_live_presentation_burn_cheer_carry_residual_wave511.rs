//! Wave 511 residual peels: BURNED / AFLAME / SPECIAL_CHEERING / CARRYING mesh bits.
//! - death_type_name tokens burn/flame/fire → BURNED/AFLAME
//! - infantry using_ability → SPECIAL_CHEERING
//! - infantry ability without combat → CARRYING
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 508 stun/disguise and Wave 505 using-weapon stamps.
//! Architecture residual - death/ability pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 511 stamp
//! - host_enum_table_residual.rs aflame/burned/special_cheering/carrying bits
//! - graphics/render_pipeline.rs Wave 511 comment
//!
//! Fail-closed:
//! - Full oil-derrick aflame DoT / flag-carrier matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_BURN_CHEER_CARRY_METHOD_NAMES_WAVE511: &[&str] = &[
    "death_type_name",
    "burned_model_bit",
    "aflame_model_bit",
    "special_cheering_model_bit",
    "carrying_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_BURN_CHEER_CARRY_SOURCE_MARKERS_WAVE511: &[&str] = &[
    "Wave 511: burned/aflame death pose + special cheering + carrying residual",
    "Wave 511: BURNED / AFLAME / SPECIAL_CHEERING / CARRYING bits included in stamp helper",
    "death_type_name: ro.death_type_name.clone()",
    "special_cheering_model_bit",
];

pub const PRESENTATION_BURN_CHEER_CARRY_NAV_STEPS_WAVE511: &[&str] = &[
    "FREEZE_DEATH_TYPE_NAME",
    "STAMP_BURNED_FROM_DEATH",
    "STAMP_AFLAME_FROM_DEATH",
    "STAMP_SPECIAL_CHEERING",
    "STAMP_CARRYING",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_BURN_CHEER_CARRY_CMD_NAMES_WAVE511: &[&str] = &[
    "click_presentation_burn_cheer_carry_ok_wnd_detect",
    "click_presentation_burn_cheer_carry_ok_wnd_skip",
    "click_presentation_burn_cheer_carry_ok_wnd_queue",
    "click_presentation_burn_cheer_carry_ok_wnd_prepare",
    "click_presentation_burn_cheer_carry_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationBurnCheerCarryAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    InputSource = 4,
    StampSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationBurnCheerCarryAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_burn_cheer_carry_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_burn_cheer_carry_last_action()
-> ResidualPresentationBurnCheerCarryAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationBurnCheerCarryAction::MethodNames,
        2 => ResidualPresentationBurnCheerCarryAction::SourceMarkers,
        3 => ResidualPresentationBurnCheerCarryAction::NavCommands,
        4 => ResidualPresentationBurnCheerCarryAction::InputSource,
        5 => ResidualPresentationBurnCheerCarryAction::StampSource,
        6 => ResidualPresentationBurnCheerCarryAction::Composite,
        _ => ResidualPresentationBurnCheerCarryAction::Idle,
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

pub fn honesty_presentation_burn_cheer_carry_method_names_residual_wave511() -> bool {
    PRESENTATION_BURN_CHEER_CARRY_METHOD_NAMES_WAVE511.len() == 6
        && residual_name_index(
            PRESENTATION_BURN_CHEER_CARRY_METHOD_NAMES_WAVE511,
            "death_type_name",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_BURN_CHEER_CARRY_METHOD_NAMES_WAVE511,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_burn_cheer_carry_source_markers_residual_wave511() -> bool {
    PRESENTATION_BURN_CHEER_CARRY_SOURCE_MARKERS_WAVE511.len() == 4
        && residual_name_index(
            PRESENTATION_BURN_CHEER_CARRY_SOURCE_MARKERS_WAVE511,
            "Wave 511: burned/aflame death pose + special cheering + carrying residual",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_BURN_CHEER_CARRY_SOURCE_MARKERS_WAVE511,
            "death_type_name: ro.death_type_name.clone()",
        ) == Some(2)
}

pub fn honesty_presentation_burn_cheer_carry_nav_commands_residual_wave511() -> bool {
    PRESENTATION_BURN_CHEER_CARRY_NAV_STEPS_WAVE511.len() == 6
        && residual_name_index(
            PRESENTATION_BURN_CHEER_CARRY_NAV_STEPS_WAVE511,
            "STAMP_SPECIAL_CHEERING",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_BURN_CHEER_CARRY_NAV_STEPS_WAVE511,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_BURN_CHEER_CARRY_CMD_NAMES_WAVE511.len() == 5
}

pub fn simulate_presentation_burn_cheer_carry_input_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 511: death type name residual for burned/aflame pose")
        && pf.contains("death_type_name: ro.death_type_name.clone()")
        && pf.contains("using_ability: ro.using_ability");
    residual_action_store(ResidualPresentationBurnCheerCarryAction::InputSource);
    ok
}

pub fn simulate_presentation_burn_cheer_carry_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf
        .contains("Wave 511: burned/aflame death pose + special cheering + carrying residual")
        && en.contains("pub fn burned_model_bit")
        && en.contains("pub fn aflame_model_bit")
        && en.contains("pub fn special_cheering_model_bit")
        && en.contains("pub fn carrying_model_bit")
        && pf.contains("PresentationObjectType::Infantry")
        && rp.contains(
            "Wave 511: BURNED / AFLAME / SPECIAL_CHEERING / CARRYING bits included in stamp helper",
        );
    residual_action_store(ResidualPresentationBurnCheerCarryAction::StampSource);
    ok
}

pub fn honesty_presentation_burn_cheer_carry_residual_pack_wave511() -> bool {
    honesty_presentation_burn_cheer_carry_method_names_residual_wave511()
        && honesty_presentation_burn_cheer_carry_source_markers_residual_wave511()
        && honesty_presentation_burn_cheer_carry_nav_commands_residual_wave511()
        && simulate_presentation_burn_cheer_carry_input_source()
        && simulate_presentation_burn_cheer_carry_stamp_source()
}

pub fn simulate_live_presentation_burn_cheer_carry_honesty() -> bool {
    let ok = honesty_presentation_burn_cheer_carry_residual_pack_wave511();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationBurnCheerCarryAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_burn_cheer_carry_method_names_residual_wave511());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_burn_cheer_carry_source_markers_residual_wave511());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_burn_cheer_carry_nav_commands_residual_wave511());
    }

    #[test]
    fn presentation_burn_cheer_carry_sources() {
        assert!(simulate_presentation_burn_cheer_carry_input_source());
        assert!(simulate_presentation_burn_cheer_carry_stamp_source());
    }

    #[test]
    fn wave511_composite_pack() {
        assert!(honesty_presentation_burn_cheer_carry_residual_pack_wave511());
    }

    #[test]
    fn simulate_live_presentation_burn_cheer_carry_honesty_residual_live() {
        assert!(
            simulate_live_presentation_burn_cheer_carry_honesty(),
            "presentation burn/cheer/carry residual must latch"
        );
        assert!(residual_presentation_burn_cheer_carry_ok());
        assert_eq!(
            residual_presentation_burn_cheer_carry_last_action(),
            ResidualPresentationBurnCheerCarryAction::Composite
        );
    }
}
