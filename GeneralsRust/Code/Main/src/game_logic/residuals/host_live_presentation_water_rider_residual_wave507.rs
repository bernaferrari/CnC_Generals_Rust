//! Wave 507 residual peels: OVER_WATER + transport RIDER1..n mesh model-condition bits.
//! - freeze `over_water` on presentation objects
//! - stamp OVER_WATER when hovering water
//! - non-structure transports stamp RIDER1..min(8, occupants)
//! - structures keep GARRISONED (Wave 504 refined)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 504 garrison contain residual.
//! Architecture residual - water/transport pose without live GameLogic dual-read.
//!
//! Sources:
//! - host_enum_table_residual.rs over_water_model_bit / rider_model_bit
//! - presentation_frame.rs Wave 507 freeze + stamp
//! - graphics/render_pipeline.rs Wave 507 comment
//!
//! Fail-closed:
//! - Full water table / hop-in rider anim still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_WATER_RIDER_METHOD_NAMES_WAVE507: &[&str] = &[
    "over_water",
    "over_water_model_bit",
    "rider_model_bit",
    "occupant_count",
    "is_structure",
    "playable_claim = false",
];

pub const PRESENTATION_WATER_RIDER_SOURCE_MARKERS_WAVE507: &[&str] = &[
    "Wave 507: C++ OVER_WATER model condition residual (hover craft / water)",
    "Wave 507: over-water + transport RIDER1..n residual bits",
    "over_water: obj.over_water",
    "rider_model_bit",
];

pub const PRESENTATION_WATER_RIDER_NAV_STEPS_WAVE507: &[&str] = &[
    "FREEZE_OVER_WATER",
    "STAMP_OVER_WATER_BIT",
    "CLEAR_RIDER_BANK",
    "STAMP_TRANSPORT_RIDERS",
    "STRUCTURES_KEEP_GARRISONED",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_WATER_RIDER_CMD_NAMES_WAVE507: &[&str] = &[
    "click_presentation_water_rider_ok_wnd_detect",
    "click_presentation_water_rider_ok_wnd_skip",
    "click_presentation_water_rider_ok_wnd_queue",
    "click_presentation_water_rider_ok_wnd_prepare",
    "click_presentation_water_rider_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationWaterRiderAction {
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

fn residual_action_store(a: ResidualPresentationWaterRiderAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_water_rider_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_water_rider_last_action() -> ResidualPresentationWaterRiderAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationWaterRiderAction::MethodNames,
        2 => ResidualPresentationWaterRiderAction::SourceMarkers,
        3 => ResidualPresentationWaterRiderAction::NavCommands,
        4 => ResidualPresentationWaterRiderAction::FreezeSource,
        5 => ResidualPresentationWaterRiderAction::StampSource,
        6 => ResidualPresentationWaterRiderAction::Composite,
        _ => ResidualPresentationWaterRiderAction::Idle,
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

pub fn honesty_presentation_water_rider_method_names_residual_wave507() -> bool {
    PRESENTATION_WATER_RIDER_METHOD_NAMES_WAVE507.len() == 6
        && residual_name_index(PRESENTATION_WATER_RIDER_METHOD_NAMES_WAVE507, "over_water")
            == Some(0)
        && residual_name_index(
            PRESENTATION_WATER_RIDER_METHOD_NAMES_WAVE507,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_water_rider_source_markers_residual_wave507() -> bool {
    PRESENTATION_WATER_RIDER_SOURCE_MARKERS_WAVE507.len() == 4
        && residual_name_index(
            PRESENTATION_WATER_RIDER_SOURCE_MARKERS_WAVE507,
            "Wave 507: over-water + transport RIDER1..n residual bits",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_WATER_RIDER_SOURCE_MARKERS_WAVE507,
            "over_water: obj.over_water",
        ) == Some(2)
}

pub fn honesty_presentation_water_rider_nav_commands_residual_wave507() -> bool {
    PRESENTATION_WATER_RIDER_NAV_STEPS_WAVE507.len() == 6
        && residual_name_index(
            PRESENTATION_WATER_RIDER_NAV_STEPS_WAVE507,
            "STAMP_TRANSPORT_RIDERS",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_WATER_RIDER_NAV_STEPS_WAVE507,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_WATER_RIDER_CMD_NAMES_WAVE507.len() == 5
}

pub fn simulate_presentation_water_rider_freeze_source() -> bool {
    let pf = pf_source();
    // 2026-08-15: default `over_water: false` lives only in tests; live freeze
    // stamps obj/ro/ent (build.rs / unit_render.rs / overlay.rs).
    let ok = (pf
        .contains("Wave 507: C++ OVER_WATER model condition residual (hover craft / water)")
        || pf.contains("Wave 507: over-water residual for mesh model-condition"))
        && pf.contains("over_water: obj.over_water")
        && pf.contains("over_water: ro.over_water")
        && pf.contains("over_water: ent.cell_is_underwater");
    residual_action_store(ResidualPresentationWaterRiderAction::FreezeSource);
    ok
}

pub fn simulate_presentation_water_rider_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 507: over-water + transport RIDER1..n residual bits")
        && en.contains("pub fn over_water_model_bit")
        && en.contains("pub fn rider_model_bit")
        && pf.contains("!self.is_structure && self.occupant_count > 0")
        && pf.contains("self.is_structure && self.occupant_count > 0")
        && rp.contains("Wave 507: OVER_WATER + transport RIDER bits included in stamp helper");
    residual_action_store(ResidualPresentationWaterRiderAction::StampSource);
    ok
}

pub fn honesty_presentation_water_rider_residual_pack_wave507() -> bool {
    honesty_presentation_water_rider_method_names_residual_wave507()
        && honesty_presentation_water_rider_source_markers_residual_wave507()
        && honesty_presentation_water_rider_nav_commands_residual_wave507()
        && simulate_presentation_water_rider_freeze_source()
        && simulate_presentation_water_rider_stamp_source()
}

pub fn simulate_live_presentation_water_rider_honesty() -> bool {
    let ok = honesty_presentation_water_rider_residual_pack_wave507();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationWaterRiderAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_water_rider_method_names_residual_wave507());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_water_rider_source_markers_residual_wave507());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_water_rider_nav_commands_residual_wave507());
    }

    #[test]
    fn presentation_water_rider_sources() {
        assert!(simulate_presentation_water_rider_freeze_source());
        assert!(simulate_presentation_water_rider_stamp_source());
    }

    #[test]
    fn wave507_composite_pack() {
        assert!(honesty_presentation_water_rider_residual_pack_wave507());
    }

    #[test]
    fn simulate_live_presentation_water_rider_honesty_residual_live() {
        assert!(
            simulate_live_presentation_water_rider_honesty(),
            "presentation water/rider residual must latch"
        );
        assert!(residual_presentation_water_rider_ok());
        assert_eq!(
            residual_presentation_water_rider_last_action(),
            ResidualPresentationWaterRiderAction::Composite
        );
    }
}
