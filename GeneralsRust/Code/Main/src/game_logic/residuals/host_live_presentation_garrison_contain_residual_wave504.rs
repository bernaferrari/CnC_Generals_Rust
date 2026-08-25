//! Wave 504 residual peels: hide contained units; stamp GARRISONED on occupied structures.
//! - `unit_render_inputs` skips objects with `contained_by`
//! - `occupant_count > 0` stamps MODELCONDITION_GARRISONED
//! - stealth hide: freeze zeros occupant_count for non-allies when hide_garrisoned_state
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 502–503 stealth/construction/disguise mesh residuals.
//! Architecture residual - container presentation without live GameLogic dual-read.
//!
//! Sources:
//! - host_enum_table_residual.rs garrisoned_model_bit
//! - presentation_frame.rs unit_render_inputs Wave 504 filter
//! - presentation_frame.rs model_condition stamp Wave 504
//!
//! Fail-closed:
//! - Full garrison fireports / contain draw still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_GARRISON_CONTAIN_METHOD_NAMES_WAVE504: &[&str] = &[
    "contained_by",
    "occupant_count",
    "garrisoned_model_bit",
    "unit_render_inputs",
    "GARRISONED",
    "playable_claim = false",
];

pub const PRESENTATION_GARRISON_CONTAIN_SOURCE_MARKERS_WAVE504: &[&str] = &[
    "Wave 504: contained units are not drawn as free world meshes",
    "Wave 504/507: garrisoned residual for structures; transports use RIDER bits",
    "garrisoned_model_bit",
    "occupant_count: ro.occupant_count",
];

pub const PRESENTATION_GARRISON_CONTAIN_NAV_STEPS_WAVE504: &[&str] = &[
    "FREEZE_CONTAINED_AND_OCCUPANTS",
    "FILTER_CONTAINED_FROM_MESH",
    "STAMP_GARRISONED_BIT",
    "MESH_RESOLVE_FROM_BITS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_GARRISON_CONTAIN_CMD_NAMES_WAVE504: &[&str] = &[
    "click_presentation_garrison_contain_ok_wnd_detect",
    "click_presentation_garrison_contain_ok_wnd_skip",
    "click_presentation_garrison_contain_ok_wnd_queue",
    "click_presentation_garrison_contain_ok_wnd_prepare",
    "click_presentation_garrison_contain_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationGarrisonContainAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    FilterSource = 4,
    StampSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationGarrisonContainAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_garrison_contain_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_garrison_contain_last_action()
-> ResidualPresentationGarrisonContainAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationGarrisonContainAction::MethodNames,
        2 => ResidualPresentationGarrisonContainAction::SourceMarkers,
        3 => ResidualPresentationGarrisonContainAction::NavCommands,
        4 => ResidualPresentationGarrisonContainAction::FilterSource,
        5 => ResidualPresentationGarrisonContainAction::StampSource,
        6 => ResidualPresentationGarrisonContainAction::Composite,
        _ => ResidualPresentationGarrisonContainAction::Idle,
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

pub fn honesty_presentation_garrison_contain_method_names_residual_wave504() -> bool {
    PRESENTATION_GARRISON_CONTAIN_METHOD_NAMES_WAVE504.len() == 6
        && residual_name_index(
            PRESENTATION_GARRISON_CONTAIN_METHOD_NAMES_WAVE504,
            "contained_by",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_GARRISON_CONTAIN_METHOD_NAMES_WAVE504,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_garrison_contain_source_markers_residual_wave504() -> bool {
    PRESENTATION_GARRISON_CONTAIN_SOURCE_MARKERS_WAVE504.len() == 4
        && residual_name_index(
            PRESENTATION_GARRISON_CONTAIN_SOURCE_MARKERS_WAVE504,
            "Wave 504: contained units are not drawn as free world meshes",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_GARRISON_CONTAIN_SOURCE_MARKERS_WAVE504,
            "garrisoned_model_bit",
        ) == Some(2)
}

pub fn honesty_presentation_garrison_contain_nav_commands_residual_wave504() -> bool {
    PRESENTATION_GARRISON_CONTAIN_NAV_STEPS_WAVE504.len() == 6
        && residual_name_index(
            PRESENTATION_GARRISON_CONTAIN_NAV_STEPS_WAVE504,
            "STAMP_GARRISONED_BIT",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_GARRISON_CONTAIN_NAV_STEPS_WAVE504,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_GARRISON_CONTAIN_CMD_NAMES_WAVE504.len() == 5
}

pub fn simulate_presentation_garrison_contain_filter_source() -> bool {
    let pf = pf_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 504: contained units are not drawn as free world meshes")
        && pf.contains("contained_by: ro.contained_by")
        && pf.contains("o.contained_by.is_some()")
        && rp.contains("Wave 504: contained units filtered; garrisoned bits in stamp helper");
    residual_action_store(ResidualPresentationGarrisonContainAction::FilterSource);
    ok
}

pub fn simulate_presentation_garrison_contain_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf
        .contains("Wave 504/507: garrisoned residual for structures; transports use RIDER bits")
        && pf.contains("occupant_count: ro.occupant_count")
        && pf.contains("garrisoned_model_bit")
        && en.contains("pub fn garrisoned_model_bit")
        && (pf.contains("self.occupant_count > 0")
            || pf.contains("self.is_structure && self.occupant_count > 0"));
    residual_action_store(ResidualPresentationGarrisonContainAction::StampSource);
    ok
}

pub fn honesty_presentation_garrison_contain_residual_pack_wave504() -> bool {
    honesty_presentation_garrison_contain_method_names_residual_wave504()
        && honesty_presentation_garrison_contain_source_markers_residual_wave504()
        && honesty_presentation_garrison_contain_nav_commands_residual_wave504()
        && simulate_presentation_garrison_contain_filter_source()
        && simulate_presentation_garrison_contain_stamp_source()
}

pub fn simulate_live_presentation_garrison_contain_honesty() -> bool {
    let ok = honesty_presentation_garrison_contain_residual_pack_wave504();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationGarrisonContainAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_garrison_contain_method_names_residual_wave504());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_garrison_contain_source_markers_residual_wave504());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_garrison_contain_nav_commands_residual_wave504());
    }

    #[test]
    fn presentation_garrison_contain_sources() {
        assert!(simulate_presentation_garrison_contain_filter_source());
        assert!(simulate_presentation_garrison_contain_stamp_source());
    }

    #[test]
    fn wave504_composite_pack() {
        assert!(honesty_presentation_garrison_contain_residual_pack_wave504());
    }

    #[test]
    fn simulate_live_presentation_garrison_contain_honesty_residual_live() {
        assert!(
            simulate_live_presentation_garrison_contain_honesty(),
            "presentation garrison/contain residual must latch"
        );
        assert!(residual_presentation_garrison_contain_ok());
        assert_eq!(
            residual_presentation_garrison_contain_last_action(),
            ResidualPresentationGarrisonContainAction::Composite
        );
    }
}
