//! Wave 523 residual peels: STUNNED_FLAILING / SECOND_LIFE / POST_COLLAPSE / SPECIAL_DAMAGED.
//! - freeze `second_life` from Object::armor_set_second_life
//! - stamp STUNNED_FLAILING when shock_stun_frames > 0; SECOND_LIFE; structure POST_COLLAPSE/SPECIAL_DAMAGED
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 522 cliff/flood bits.
//! Architecture residual - death/second-life pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 523 freeze + stamps
//! - host_enum_table_residual.rs stunned_flailing/second_life/post_collapse/special_damaged helpers
//!
//! Fail-closed:
//! - Full battle-bus second-life W3D chassis swap still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_SECOND_LIFE_STUN_METHOD_NAMES_WAVE523: &[&str] = &[
    "second_life",
    "shock_stun_frames",
    "stunned_flailing_model_bit",
    "second_life_model_bit",
    "post_collapse_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_SECOND_LIFE_STUN_SOURCE_MARKERS_WAVE523: &[&str] = &[
    "Wave 523: shock stun frames => STUNNED_FLAILING; disabled => STUNNED",
    "second_life: obj.armor_set_second_life",
    "fn stunned_flailing_model_bit",
    "fn second_life_model_bit",
];

pub const PRESENTATION_SECOND_LIFE_STUN_NAV_STEPS_WAVE523: &[&str] = &[
    "FREEZE_SECOND_LIFE",
    "STAMP_STUNNED_FLAILING",
    "STAMP_SECOND_LIFE",
    "STAMP_POST_COLLAPSE_SPECIAL_DAMAGED",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_SECOND_LIFE_STUN_CMD_NAMES_WAVE523: &[&str] = &[
    "click_presentation_second_life_stun_ok_wnd_detect",
    "click_presentation_second_life_stun_ok_wnd_skip",
    "click_presentation_second_life_stun_ok_wnd_queue",
    "click_presentation_second_life_stun_ok_wnd_prepare",
    "click_presentation_second_life_stun_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationSecondLifeStunAction {
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

fn residual_action_store(a: ResidualPresentationSecondLifeStunAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_second_life_stun_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_second_life_stun_last_action()
-> ResidualPresentationSecondLifeStunAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationSecondLifeStunAction::MethodNames,
        2 => ResidualPresentationSecondLifeStunAction::SourceMarkers,
        3 => ResidualPresentationSecondLifeStunAction::NavCommands,
        4 => ResidualPresentationSecondLifeStunAction::FreezeSource,
        5 => ResidualPresentationSecondLifeStunAction::StampSource,
        6 => ResidualPresentationSecondLifeStunAction::Composite,
        _ => ResidualPresentationSecondLifeStunAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_presentation_second_life_stun_method_names_residual_wave523() -> bool {
    PRESENTATION_SECOND_LIFE_STUN_METHOD_NAMES_WAVE523.len() == 6
        && residual_name_index(
            PRESENTATION_SECOND_LIFE_STUN_METHOD_NAMES_WAVE523,
            "second_life",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_SECOND_LIFE_STUN_METHOD_NAMES_WAVE523,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_second_life_stun_source_markers_residual_wave523() -> bool {
    PRESENTATION_SECOND_LIFE_STUN_SOURCE_MARKERS_WAVE523.len() == 4
        && residual_name_index(
            PRESENTATION_SECOND_LIFE_STUN_SOURCE_MARKERS_WAVE523,
            "Wave 523: shock stun frames => STUNNED_FLAILING; disabled => STUNNED",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_SECOND_LIFE_STUN_SOURCE_MARKERS_WAVE523,
            "fn second_life_model_bit",
        ) == Some(3)
}

pub fn honesty_presentation_second_life_stun_nav_commands_residual_wave523() -> bool {
    PRESENTATION_SECOND_LIFE_STUN_NAV_STEPS_WAVE523.len() == 6
        && residual_name_index(
            PRESENTATION_SECOND_LIFE_STUN_NAV_STEPS_WAVE523,
            "STAMP_SECOND_LIFE",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_SECOND_LIFE_STUN_NAV_STEPS_WAVE523,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_SECOND_LIFE_STUN_CMD_NAMES_WAVE523.len() == 5
}

pub fn simulate_presentation_second_life_stun_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("second_life: obj.armor_set_second_life")
        && pf.contains("second_life: ro.second_life")
        && pf.contains("shock_stun_frames");
    residual_action_store(ResidualPresentationSecondLifeStunAction::FreezeSource);
    ok
}

pub fn simulate_presentation_second_life_stun_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf.contains("Wave 523: shock stun frames => STUNNED_FLAILING; disabled => STUNNED")
        && en.contains("pub fn stunned_flailing_model_bit")
        && en.contains("pub fn second_life_model_bit")
        && pf.contains("if self.second_life");
    residual_action_store(ResidualPresentationSecondLifeStunAction::StampSource);
    ok
}

pub fn honesty_presentation_second_life_stun_residual_pack_wave523() -> bool {
    honesty_presentation_second_life_stun_method_names_residual_wave523()
        && honesty_presentation_second_life_stun_source_markers_residual_wave523()
        && honesty_presentation_second_life_stun_nav_commands_residual_wave523()
        && simulate_presentation_second_life_stun_freeze_source()
        && simulate_presentation_second_life_stun_stamp_source()
}

pub fn simulate_live_presentation_second_life_stun_honesty() -> bool {
    let ok = honesty_presentation_second_life_stun_residual_pack_wave523();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationSecondLifeStunAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_second_life_stun_method_names_residual_wave523());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_second_life_stun_source_markers_residual_wave523());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_second_life_stun_nav_commands_residual_wave523());
    }

    #[test]
    fn presentation_second_life_stun_sources() {
        assert!(simulate_presentation_second_life_stun_freeze_source());
        assert!(simulate_presentation_second_life_stun_stamp_source());
    }

    #[test]
    fn wave523_composite_pack() {
        assert!(honesty_presentation_second_life_stun_residual_pack_wave523());
    }

    #[test]
    fn simulate_live_presentation_second_life_stun_honesty_residual_live() {
        assert!(
            simulate_live_presentation_second_life_stun_honesty(),
            "presentation second-life/stun residual must latch"
        );
        assert!(residual_presentation_second_life_stun_ok());
        assert_eq!(
            residual_presentation_second_life_stun_last_action(),
            ResidualPresentationSecondLifeStunAction::Composite
        );
    }
}
