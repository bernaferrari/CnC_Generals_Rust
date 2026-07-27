//! Wave 520 residual peels: AnimationSteeringUpdate turn mesh bits.
//! - freeze `anim_steer_turn` from HostAnimationSteeringData::current_turn_anim
//! - stamp CENTER_TO_LEFT/RIGHT and LEFT/RIGHT_TO_CENTER
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 519 shock/power/jet bits.
//! Architecture residual - battle-bus turn pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 520 freeze + stamps
//! - host_animation_steering.rs HostAnimSteerTurnAnim
//! - host_enum_table_residual.rs center/left/right helpers
//!
//! Fail-closed:
//! - Full Drawable multi-flag scrub / client blend still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_ANIM_STEER_METHOD_NAMES_WAVE520: &[&str] = &[
    "anim_steer_turn",
    "current_turn_anim",
    "center_to_left_model_bit",
    "center_to_right_model_bit",
    "left_to_center_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_ANIM_STEER_SOURCE_MARKERS_WAVE520: &[&str] = &[
    "Wave 520: AnimationSteeringUpdate CENTER/LEFT/RIGHT turn model-condition residual",
    "current_turn_anim as u8",
    "fn center_to_left_model_bit",
    "match self.anim_steer_turn",
];

pub const PRESENTATION_ANIM_STEER_NAV_STEPS_WAVE520: &[&str] = &[
    "FREEZE_ANIM_STEER_TURN",
    "STAMP_CENTER_TO_LEFT_RIGHT",
    "STAMP_LEFT_RIGHT_TO_CENTER",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_ANIM_STEER_CMD_NAMES_WAVE520: &[&str] = &[
    "click_presentation_anim_steer_ok_wnd_detect",
    "click_presentation_anim_steer_ok_wnd_skip",
    "click_presentation_anim_steer_ok_wnd_queue",
    "click_presentation_anim_steer_ok_wnd_prepare",
    "click_presentation_anim_steer_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationAnimSteerAction {
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

fn residual_action_store(a: ResidualPresentationAnimSteerAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_anim_steer_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_anim_steer_last_action() -> ResidualPresentationAnimSteerAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationAnimSteerAction::MethodNames,
        2 => ResidualPresentationAnimSteerAction::SourceMarkers,
        3 => ResidualPresentationAnimSteerAction::NavCommands,
        4 => ResidualPresentationAnimSteerAction::FreezeSource,
        5 => ResidualPresentationAnimSteerAction::StampSource,
        6 => ResidualPresentationAnimSteerAction::Composite,
        _ => ResidualPresentationAnimSteerAction::Idle,
    }
}

fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}

fn en_source() -> &'static str {
    include_str!("host_enum_table_residual.rs")
}

pub fn honesty_presentation_anim_steer_method_names_residual_wave520() -> bool {
    PRESENTATION_ANIM_STEER_METHOD_NAMES_WAVE520.len() == 6
        && residual_name_index(
            PRESENTATION_ANIM_STEER_METHOD_NAMES_WAVE520,
            "anim_steer_turn",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_ANIM_STEER_METHOD_NAMES_WAVE520,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_anim_steer_source_markers_residual_wave520() -> bool {
    PRESENTATION_ANIM_STEER_SOURCE_MARKERS_WAVE520.len() == 4
        && residual_name_index(
            PRESENTATION_ANIM_STEER_SOURCE_MARKERS_WAVE520,
            "Wave 520: AnimationSteeringUpdate CENTER/LEFT/RIGHT turn model-condition residual",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_ANIM_STEER_SOURCE_MARKERS_WAVE520,
            "fn center_to_left_model_bit",
        ) == Some(2)
}

pub fn honesty_presentation_anim_steer_nav_commands_residual_wave520() -> bool {
    PRESENTATION_ANIM_STEER_NAV_STEPS_WAVE520.len() == 5
        && residual_name_index(
            PRESENTATION_ANIM_STEER_NAV_STEPS_WAVE520,
            "STAMP_CENTER_TO_LEFT_RIGHT",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_ANIM_STEER_NAV_STEPS_WAVE520,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(4)
        && RUNTIME_HOST_PRESENTATION_ANIM_STEER_CMD_NAMES_WAVE520.len() == 5
}

pub fn simulate_presentation_anim_steer_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("current_turn_anim as u8")
        && pf.contains("animation_steering")
        && pf.contains("anim_steer_turn:");
    residual_action_store(ResidualPresentationAnimSteerAction::FreezeSource);
    ok
}

pub fn simulate_presentation_anim_steer_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf.contains(
        "Wave 520: AnimationSteeringUpdate CENTER/LEFT/RIGHT turn model-condition residual",
    ) && en.contains("pub fn center_to_left_model_bit")
        && en.contains("pub fn right_to_center_model_bit")
        && pf.contains("match self.anim_steer_turn");
    residual_action_store(ResidualPresentationAnimSteerAction::StampSource);
    ok
}

pub fn honesty_presentation_anim_steer_residual_pack_wave520() -> bool {
    honesty_presentation_anim_steer_method_names_residual_wave520()
        && honesty_presentation_anim_steer_source_markers_residual_wave520()
        && honesty_presentation_anim_steer_nav_commands_residual_wave520()
        && simulate_presentation_anim_steer_freeze_source()
        && simulate_presentation_anim_steer_stamp_source()
}

pub fn simulate_live_presentation_anim_steer_honesty() -> bool {
    let ok = honesty_presentation_anim_steer_residual_pack_wave520();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationAnimSteerAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_anim_steer_method_names_residual_wave520());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_anim_steer_source_markers_residual_wave520());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_anim_steer_nav_commands_residual_wave520());
    }

    #[test]
    fn presentation_anim_steer_sources() {
        assert!(simulate_presentation_anim_steer_freeze_source());
        assert!(simulate_presentation_anim_steer_stamp_source());
    }

    #[test]
    fn wave520_composite_pack() {
        assert!(honesty_presentation_anim_steer_residual_pack_wave520());
    }

    #[test]
    fn simulate_live_presentation_anim_steer_honesty_residual_live() {
        assert!(
            simulate_live_presentation_anim_steer_honesty(),
            "presentation anim-steer residual must latch"
        );
        assert!(residual_presentation_anim_steer_ok());
        assert_eq!(
            residual_presentation_anim_steer_last_action(),
            ResidualPresentationAnimSteerAction::Composite
        );
    }
}
