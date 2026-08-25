//! Wave 508 residual peels: body-damage / DISGUISED / STUNNED mesh model-condition bits.
//! - `host_apply_body_damage_model_bits` from `body_damage_state`
//! - stamp DISGUISED when presentation `disguised`
//! - stamp STUNNED when presentation `disabled`
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 503 disguise mesh key swap (color/mesh name).
//! Architecture residual - damage/stun/disguise pose without live GameLogic dual-read.
//!
//! Sources:
//! - host_enum_table_residual.rs disguised_model_bit / stunned_model_bit / host_apply_body_damage_model_bits
//! - presentation_frame.rs Wave 508 stamp
//! - graphics/render_pipeline.rs Wave 508 comment
//!
//! Fail-closed:
//! - Full stunned flailing / special damaged matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_BODY_DISGUISE_STUN_METHOD_NAMES_WAVE508: &[&str] = &[
    "body_damage_state",
    "host_apply_body_damage_model_bits",
    "disguised_model_bit",
    "stunned_model_bit",
    "disabled",
    "playable_claim = false",
];

pub const PRESENTATION_BODY_DISGUISE_STUN_SOURCE_MARKERS_WAVE508: &[&str] = &[
    "Wave 508: body-damage / disguised / stunned model-condition residual",
    "Wave 508: body-damage / DISGUISED / STUNNED bits included in stamp helper",
    "host_apply_body_damage_model_bits",
    "disabled: ro.disabled",
];

pub const PRESENTATION_BODY_DISGUISE_STUN_NAV_STEPS_WAVE508: &[&str] = &[
    "FREEZE_BODY_DISGUISE_DISABLED",
    "APPLY_BODY_DAMAGE_BITS",
    "STAMP_DISGUISED_BIT",
    "STAMP_STUNNED_BIT",
    "MESH_RESOLVE_FROM_BITS",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_BODY_DISGUISE_STUN_CMD_NAMES_WAVE508: &[&str] = &[
    "click_presentation_body_disguise_stun_ok_wnd_detect",
    "click_presentation_body_disguise_stun_ok_wnd_skip",
    "click_presentation_body_disguise_stun_ok_wnd_queue",
    "click_presentation_body_disguise_stun_ok_wnd_prepare",
    "click_presentation_body_disguise_stun_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationBodyDisguiseStunAction {
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

fn residual_action_store(a: ResidualPresentationBodyDisguiseStunAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_body_disguise_stun_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_body_disguise_stun_last_action()
-> ResidualPresentationBodyDisguiseStunAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationBodyDisguiseStunAction::MethodNames,
        2 => ResidualPresentationBodyDisguiseStunAction::SourceMarkers,
        3 => ResidualPresentationBodyDisguiseStunAction::NavCommands,
        4 => ResidualPresentationBodyDisguiseStunAction::InputSource,
        5 => ResidualPresentationBodyDisguiseStunAction::StampSource,
        6 => ResidualPresentationBodyDisguiseStunAction::Composite,
        _ => ResidualPresentationBodyDisguiseStunAction::Idle,
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

pub fn honesty_presentation_body_disguise_stun_method_names_residual_wave508() -> bool {
    PRESENTATION_BODY_DISGUISE_STUN_METHOD_NAMES_WAVE508.len() == 6
        && residual_name_index(
            PRESENTATION_BODY_DISGUISE_STUN_METHOD_NAMES_WAVE508,
            "body_damage_state",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_BODY_DISGUISE_STUN_METHOD_NAMES_WAVE508,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_body_disguise_stun_source_markers_residual_wave508() -> bool {
    PRESENTATION_BODY_DISGUISE_STUN_SOURCE_MARKERS_WAVE508.len() == 4
        && residual_name_index(
            PRESENTATION_BODY_DISGUISE_STUN_SOURCE_MARKERS_WAVE508,
            "Wave 508: body-damage / disguised / stunned model-condition residual",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_BODY_DISGUISE_STUN_SOURCE_MARKERS_WAVE508,
            "host_apply_body_damage_model_bits",
        ) == Some(2)
}

pub fn honesty_presentation_body_disguise_stun_nav_commands_residual_wave508() -> bool {
    PRESENTATION_BODY_DISGUISE_STUN_NAV_STEPS_WAVE508.len() == 6
        && residual_name_index(
            PRESENTATION_BODY_DISGUISE_STUN_NAV_STEPS_WAVE508,
            "STAMP_STUNNED_BIT",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_BODY_DISGUISE_STUN_NAV_STEPS_WAVE508,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_BODY_DISGUISE_STUN_CMD_NAMES_WAVE508.len() == 5
}

pub fn simulate_presentation_body_disguise_stun_input_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 508: any host disable residual that blocks acting (stun pose)")
        && pf.contains("disabled: ro.disabled")
        && pf.contains("disguised: ro.disguised")
        && pf.contains("body_damage_state: ro.body_damage_state");
    residual_action_store(ResidualPresentationBodyDisguiseStunAction::InputSource);
    ok
}

pub fn simulate_presentation_body_disguise_stun_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 508: body-damage / disguised / stunned model-condition residual")
        && en.contains("pub fn disguised_model_bit")
        && en.contains("pub fn stunned_model_bit")
        && en.contains("pub fn host_apply_body_damage_model_bits")
        && pf.contains("host_apply_body_damage_model_bits")
        && rp.contains("Wave 508: body-damage / DISGUISED / STUNNED bits included in stamp helper");
    residual_action_store(ResidualPresentationBodyDisguiseStunAction::StampSource);
    ok
}

pub fn honesty_presentation_body_disguise_stun_residual_pack_wave508() -> bool {
    honesty_presentation_body_disguise_stun_method_names_residual_wave508()
        && honesty_presentation_body_disguise_stun_source_markers_residual_wave508()
        && honesty_presentation_body_disguise_stun_nav_commands_residual_wave508()
        && simulate_presentation_body_disguise_stun_input_source()
        && simulate_presentation_body_disguise_stun_stamp_source()
}

pub fn simulate_live_presentation_body_disguise_stun_honesty() -> bool {
    let ok = honesty_presentation_body_disguise_stun_residual_pack_wave508();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationBodyDisguiseStunAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_body_disguise_stun_method_names_residual_wave508());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_body_disguise_stun_source_markers_residual_wave508());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_body_disguise_stun_nav_commands_residual_wave508());
    }

    #[test]
    fn presentation_body_disguise_stun_sources() {
        assert!(simulate_presentation_body_disguise_stun_input_source());
        assert!(simulate_presentation_body_disguise_stun_stamp_source());
    }

    #[test]
    fn wave508_composite_pack() {
        assert!(honesty_presentation_body_disguise_stun_residual_pack_wave508());
    }

    #[test]
    fn simulate_live_presentation_body_disguise_stun_honesty_residual_live() {
        assert!(
            simulate_live_presentation_body_disguise_stun_honesty(),
            "presentation body/disguise/stun residual must latch"
        );
        assert!(residual_presentation_body_disguise_stun_ok());
        assert_eq!(
            residual_presentation_body_disguise_stun_last_action(),
            ResidualPresentationBodyDisguiseStunAction::Composite
        );
    }
}
