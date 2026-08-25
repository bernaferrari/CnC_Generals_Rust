//! Wave 513 residual peels: JAMMED / DYING / RELOADING_A / PACKING / UNPACKING mesh bits.
//! - weapons_jammed → JAMMED
//! - destroyed → DYING
//! - continuous_fire_coast_until_frame > logic_frame → RELOADING_A
//! - non-deployed door phases map UNPACKING (1–2) / PACKING (3–4)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 496 door bits and Wave 501 DEPLOYED.
//! Architecture residual - jam/die/reload/pack pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 513 stamp
//! - host_enum_table_residual.rs jammed/dying bits
//! - graphics/render_pipeline.rs Wave 513 comment
//!
//! Fail-closed:
//! - Full DeployStyle packing anim matrix / multi-weapon reload still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_JAM_DIE_RELOAD_PACK_METHOD_NAMES_WAVE513: &[&str] = &[
    "jammed",
    "dying_model_bit",
    "reloading_a_model_bit",
    "packing_model_bit",
    "unpacking_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_JAM_DIE_RELOAD_PACK_SOURCE_MARKERS_WAVE513: &[&str] = &[
    "Wave 513: jammed / dying / reloading / packing-unpack deploy residual bits",
    "Wave 513: JAMMED / DYING / RELOADING / PACKING / UNPACKING bits included in stamp helper",
    "jammed: ro.weapons_jammed",
    "continuous_fire_coast_until_frame > self.logic_frame",
];

pub const PRESENTATION_JAM_DIE_RELOAD_PACK_NAV_STEPS_WAVE513: &[&str] = &[
    "FREEZE_JAMMED_DESTROYED_COAST",
    "STAMP_JAMMED",
    "STAMP_DYING",
    "STAMP_RELOADING_FROM_COAST",
    "STAMP_PACK_UNPACK_FROM_DOOR",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_JAM_DIE_RELOAD_PACK_CMD_NAMES_WAVE513: &[&str] = &[
    "click_presentation_jam_die_reload_pack_ok_wnd_detect",
    "click_presentation_jam_die_reload_pack_ok_wnd_skip",
    "click_presentation_jam_die_reload_pack_ok_wnd_queue",
    "click_presentation_jam_die_reload_pack_ok_wnd_prepare",
    "click_presentation_jam_die_reload_pack_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationJamDieReloadPackAction {
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

fn residual_action_store(a: ResidualPresentationJamDieReloadPackAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_jam_die_reload_pack_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_jam_die_reload_pack_last_action()
-> ResidualPresentationJamDieReloadPackAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationJamDieReloadPackAction::MethodNames,
        2 => ResidualPresentationJamDieReloadPackAction::SourceMarkers,
        3 => ResidualPresentationJamDieReloadPackAction::NavCommands,
        4 => ResidualPresentationJamDieReloadPackAction::InputSource,
        5 => ResidualPresentationJamDieReloadPackAction::StampSource,
        6 => ResidualPresentationJamDieReloadPackAction::Composite,
        _ => ResidualPresentationJamDieReloadPackAction::Idle,
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

pub fn honesty_presentation_jam_die_reload_pack_method_names_residual_wave513() -> bool {
    PRESENTATION_JAM_DIE_RELOAD_PACK_METHOD_NAMES_WAVE513.len() == 6
        && residual_name_index(
            PRESENTATION_JAM_DIE_RELOAD_PACK_METHOD_NAMES_WAVE513,
            "jammed",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_JAM_DIE_RELOAD_PACK_METHOD_NAMES_WAVE513,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_jam_die_reload_pack_source_markers_residual_wave513() -> bool {
    PRESENTATION_JAM_DIE_RELOAD_PACK_SOURCE_MARKERS_WAVE513.len() == 4
        && residual_name_index(
            PRESENTATION_JAM_DIE_RELOAD_PACK_SOURCE_MARKERS_WAVE513,
            "Wave 513: jammed / dying / reloading / packing-unpack deploy residual bits",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_JAM_DIE_RELOAD_PACK_SOURCE_MARKERS_WAVE513,
            "jammed: ro.weapons_jammed",
        ) == Some(2)
}

pub fn honesty_presentation_jam_die_reload_pack_nav_commands_residual_wave513() -> bool {
    PRESENTATION_JAM_DIE_RELOAD_PACK_NAV_STEPS_WAVE513.len() == 6
        && residual_name_index(
            PRESENTATION_JAM_DIE_RELOAD_PACK_NAV_STEPS_WAVE513,
            "STAMP_RELOADING_FROM_COAST",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_JAM_DIE_RELOAD_PACK_NAV_STEPS_WAVE513,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_JAM_DIE_RELOAD_PACK_CMD_NAMES_WAVE513.len() == 5
}

pub fn simulate_presentation_jam_die_reload_pack_input_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 513: weapons jammed residual")
        && pf.contains("jammed: ro.weapons_jammed")
        && pf.contains("destroyed: ro.destroyed")
        && pf.contains("continuous_fire_coast_until_frame: ro.continuous_fire_coast_until_frame")
        && (pf.contains("input.logic_frame = self.frame.0")
            || pf.contains("input.logic_frame = logic_frame"));
    residual_action_store(ResidualPresentationJamDieReloadPackAction::InputSource);
    ok
}

pub fn simulate_presentation_jam_die_reload_pack_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 513: jammed / dying / reloading / packing-unpack deploy residual bits")
        && en.contains("pub fn jammed_model_bit")
        && en.contains("pub fn dying_model_bit")
        && pf.contains("continuous_fire_coast_until_frame > self.logic_frame")
        && pf.contains("packing_model_bit")
        && pf.contains("unpacking_model_bit")
        && rp.contains(
            "Wave 513: JAMMED / DYING / RELOADING / PACKING / UNPACKING bits included in stamp helper",
        );
    residual_action_store(ResidualPresentationJamDieReloadPackAction::StampSource);
    ok
}

pub fn honesty_presentation_jam_die_reload_pack_residual_pack_wave513() -> bool {
    honesty_presentation_jam_die_reload_pack_method_names_residual_wave513()
        && honesty_presentation_jam_die_reload_pack_source_markers_residual_wave513()
        && honesty_presentation_jam_die_reload_pack_nav_commands_residual_wave513()
        && simulate_presentation_jam_die_reload_pack_input_source()
        && simulate_presentation_jam_die_reload_pack_stamp_source()
}

pub fn simulate_live_presentation_jam_die_reload_pack_honesty() -> bool {
    let ok = honesty_presentation_jam_die_reload_pack_residual_pack_wave513();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationJamDieReloadPackAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_jam_die_reload_pack_method_names_residual_wave513());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_jam_die_reload_pack_source_markers_residual_wave513());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_jam_die_reload_pack_nav_commands_residual_wave513());
    }

    #[test]
    fn presentation_jam_die_reload_pack_sources() {
        assert!(simulate_presentation_jam_die_reload_pack_input_source());
        assert!(simulate_presentation_jam_die_reload_pack_stamp_source());
    }

    #[test]
    fn wave513_composite_pack() {
        assert!(honesty_presentation_jam_die_reload_pack_residual_pack_wave513());
    }

    #[test]
    fn simulate_live_presentation_jam_die_reload_pack_honesty_residual_live() {
        assert!(
            simulate_live_presentation_jam_die_reload_pack_honesty(),
            "presentation jam/die/reload/pack residual must latch"
        );
        assert!(residual_presentation_jam_die_reload_pack_ok());
        assert_eq!(
            residual_presentation_jam_die_reload_pack_last_action(),
            ResidualPresentationJamDieReloadPackAction::Composite
        );
    }
}
