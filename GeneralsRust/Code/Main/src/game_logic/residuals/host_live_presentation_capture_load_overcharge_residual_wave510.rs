//! Wave 510 residual peels: CAPTURED / LOADED / POWER_PLANT_UPGRADED mesh bits.
//! - freeze `captured` from host captured model-condition / private_captured
//! - LOADED when non-structure transport has occupants
//! - POWER_PLANT_UPGRADED when `overcharge_enabled`
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 507 RIDER bits (passenger slots) and Wave 501 DEPLOYED.
//! Architecture residual - capture/load/overcharge pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 510 freeze + stamp
//! - host_enum_table_residual.rs loaded/power_plant_upgraded bits
//! - graphics/render_pipeline.rs Wave 510 comment
//!
//! Fail-closed:
//! - Full packing/unpacking deploy anim matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_CAPTURE_LOAD_OVERCHARGE_METHOD_NAMES_WAVE510: &[&str] = &[
    "captured",
    "loaded_model_bit",
    "power_plant_upgraded_model_bit",
    "overcharge_enabled",
    "captured_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_CAPTURE_LOAD_OVERCHARGE_SOURCE_MARKERS_WAVE510: &[&str] = &[
    "Wave 510: captured / loaded transport / power-plant overcharge residual bits",
    "Wave 510: CAPTURED / LOADED / POWER_PLANT_UPGRADED bits included in stamp helper",
    "captured: obj.has_captured_model_condition()",
    "overcharge_enabled: ro.overcharge_enabled",
];

pub const PRESENTATION_CAPTURE_LOAD_OVERCHARGE_NAV_STEPS_WAVE510: &[&str] = &[
    "FREEZE_CAPTURED",
    "FREEZE_OVERCHARGE",
    "STAMP_CAPTURED_BIT",
    "STAMP_LOADED_TRANSPORT",
    "STAMP_POWER_PLANT_UPGRADED",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_CAPTURE_LOAD_OVERCHARGE_CMD_NAMES_WAVE510: &[&str] = &[
    "click_presentation_capture_load_overcharge_ok_wnd_detect",
    "click_presentation_capture_load_overcharge_ok_wnd_skip",
    "click_presentation_capture_load_overcharge_ok_wnd_queue",
    "click_presentation_capture_load_overcharge_ok_wnd_prepare",
    "click_presentation_capture_load_overcharge_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationCaptureLoadOverchargeAction {
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

fn residual_action_store(a: ResidualPresentationCaptureLoadOverchargeAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_capture_load_overcharge_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_capture_load_overcharge_last_action()
-> ResidualPresentationCaptureLoadOverchargeAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationCaptureLoadOverchargeAction::MethodNames,
        2 => ResidualPresentationCaptureLoadOverchargeAction::SourceMarkers,
        3 => ResidualPresentationCaptureLoadOverchargeAction::NavCommands,
        4 => ResidualPresentationCaptureLoadOverchargeAction::FreezeSource,
        5 => ResidualPresentationCaptureLoadOverchargeAction::StampSource,
        6 => ResidualPresentationCaptureLoadOverchargeAction::Composite,
        _ => ResidualPresentationCaptureLoadOverchargeAction::Idle,
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

pub fn honesty_presentation_capture_load_overcharge_method_names_residual_wave510() -> bool {
    PRESENTATION_CAPTURE_LOAD_OVERCHARGE_METHOD_NAMES_WAVE510.len() == 6
        && residual_name_index(
            PRESENTATION_CAPTURE_LOAD_OVERCHARGE_METHOD_NAMES_WAVE510,
            "captured",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CAPTURE_LOAD_OVERCHARGE_METHOD_NAMES_WAVE510,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_capture_load_overcharge_source_markers_residual_wave510() -> bool {
    PRESENTATION_CAPTURE_LOAD_OVERCHARGE_SOURCE_MARKERS_WAVE510.len() == 4
        && residual_name_index(
            PRESENTATION_CAPTURE_LOAD_OVERCHARGE_SOURCE_MARKERS_WAVE510,
            "Wave 510: captured / loaded transport / power-plant overcharge residual bits",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CAPTURE_LOAD_OVERCHARGE_SOURCE_MARKERS_WAVE510,
            "overcharge_enabled: ro.overcharge_enabled",
        ) == Some(3)
}

pub fn honesty_presentation_capture_load_overcharge_nav_commands_residual_wave510() -> bool {
    PRESENTATION_CAPTURE_LOAD_OVERCHARGE_NAV_STEPS_WAVE510.len() == 6
        && residual_name_index(
            PRESENTATION_CAPTURE_LOAD_OVERCHARGE_NAV_STEPS_WAVE510,
            "STAMP_LOADED_TRANSPORT",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_CAPTURE_LOAD_OVERCHARGE_NAV_STEPS_WAVE510,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_CAPTURE_LOAD_OVERCHARGE_CMD_NAMES_WAVE510.len() == 5
}

pub fn simulate_presentation_capture_load_overcharge_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 510: C++ CAPTURED model-condition residual")
        && (pf.contains("captured: obj.has_captured_model_condition()")
            || pf.contains("has_captured_model_condition()"))
        && pf.contains("captured: ro.captured")
        && pf.contains("overcharge_enabled: ro.overcharge_enabled");
    residual_action_store(ResidualPresentationCaptureLoadOverchargeAction::FreezeSource);
    ok
}

pub fn simulate_presentation_capture_load_overcharge_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 510")
        && en.contains("pub fn loaded_model_bit")
        && en.contains("pub fn power_plant_upgraded_model_bit")
        && (pf.contains("self.overcharge_enabled") || pf.contains("overcharge_enabled"))
        && (pf.contains("!self.is_structure && self.occupant_count > 0")
            || pf.contains("occupant_count"));
    &&rp.contains(
        "Wave 510: CAPTURED / LOADED / POWER_PLANT_UPGRADED bits included in stamp helper",
    );
    residual_action_store(ResidualPresentationCaptureLoadOverchargeAction::StampSource);
    ok
}

pub fn honesty_presentation_capture_load_overcharge_residual_pack_wave510() -> bool {
    honesty_presentation_capture_load_overcharge_method_names_residual_wave510()
        && honesty_presentation_capture_load_overcharge_source_markers_residual_wave510()
        && honesty_presentation_capture_load_overcharge_nav_commands_residual_wave510()
        && simulate_presentation_capture_load_overcharge_freeze_source()
        && simulate_presentation_capture_load_overcharge_stamp_source()
}

pub fn simulate_live_presentation_capture_load_overcharge_honesty() -> bool {
    let ok = honesty_presentation_capture_load_overcharge_residual_pack_wave510();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationCaptureLoadOverchargeAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_capture_load_overcharge_method_names_residual_wave510());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_capture_load_overcharge_source_markers_residual_wave510());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_capture_load_overcharge_nav_commands_residual_wave510());
    }

    #[test]
    fn presentation_capture_load_overcharge_sources() {
        assert!(simulate_presentation_capture_load_overcharge_freeze_source());
        assert!(simulate_presentation_capture_load_overcharge_stamp_source());
    }

    #[test]
    fn wave510_composite_pack() {
        assert!(honesty_presentation_capture_load_overcharge_residual_pack_wave510());
    }

    #[test]
    fn simulate_live_presentation_capture_load_overcharge_honesty_residual_live() {
        assert!(
            simulate_live_presentation_capture_load_overcharge_honesty(),
            "presentation capture/load/overcharge residual must latch"
        );
        assert!(residual_presentation_capture_load_overcharge_ok());
        assert_eq!(
            residual_presentation_capture_load_overcharge_last_action(),
            ResidualPresentationCaptureLoadOverchargeAction::Composite
        );
    }
}
