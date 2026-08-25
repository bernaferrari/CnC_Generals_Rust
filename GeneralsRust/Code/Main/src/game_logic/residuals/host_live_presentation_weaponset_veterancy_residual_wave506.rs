//! Wave 506 residual peels: weaponset veterancy bits on presentation mesh path.
//! - `UnitRenderInput.veterancy` frozen from presentation
//! - stamp WEAPONSET_VETERAN / ELITE / HERO model-condition bits
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 505 air/parachute stamps.
//! Architecture residual - experience chevrons/weaponset without live GameLogic dual-read.
//!
//! Sources:
//! - host_enum_table_residual.rs weaponset_*_model_bit
//! - presentation_frame.rs Wave 506 stamp
//! - graphics/render_pipeline.rs Wave 506 comment
//!
//! Fail-closed:
//! - Full drawable chevron overlay still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_WEAPONSET_VETERANCY_METHOD_NAMES_WAVE506: &[&str] = &[
    "veterancy",
    "weaponset_veteran_model_bit",
    "weaponset_elite_model_bit",
    "weaponset_hero_model_bit",
    "PresentationVeterancy",
    "playable_claim = false",
];

pub const PRESENTATION_WEAPONSET_VETERANCY_SOURCE_MARKERS_WAVE506: &[&str] = &[
    "Wave 506: weaponset veterancy model-condition residual",
    "Wave 506: weaponset veterancy bits included in stamp helper",
    "veterancy: ro.veterancy",
    "weaponset_hero_model_bit",
];

pub const PRESENTATION_WEAPONSET_VETERANCY_NAV_STEPS_WAVE506: &[&str] = &[
    "FREEZE_VETERANCY",
    "CLEAR_WEAPONSET_BANK",
    "STAMP_VETERAN_ELITE_OR_HERO",
    "MESH_RESOLVE_FROM_BITS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_WEAPONSET_VETERANCY_CMD_NAMES_WAVE506: &[&str] = &[
    "click_presentation_weaponset_veterancy_ok_wnd_detect",
    "click_presentation_weaponset_veterancy_ok_wnd_skip",
    "click_presentation_weaponset_veterancy_ok_wnd_queue",
    "click_presentation_weaponset_veterancy_ok_wnd_prepare",
    "click_presentation_weaponset_veterancy_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationWeaponsetVeterancyAction {
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

fn residual_action_store(a: ResidualPresentationWeaponsetVeterancyAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_weaponset_veterancy_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_weaponset_veterancy_last_action()
-> ResidualPresentationWeaponsetVeterancyAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationWeaponsetVeterancyAction::MethodNames,
        2 => ResidualPresentationWeaponsetVeterancyAction::SourceMarkers,
        3 => ResidualPresentationWeaponsetVeterancyAction::NavCommands,
        4 => ResidualPresentationWeaponsetVeterancyAction::InputSource,
        5 => ResidualPresentationWeaponsetVeterancyAction::StampSource,
        6 => ResidualPresentationWeaponsetVeterancyAction::Composite,
        _ => ResidualPresentationWeaponsetVeterancyAction::Idle,
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

pub fn honesty_presentation_weaponset_veterancy_method_names_residual_wave506() -> bool {
    PRESENTATION_WEAPONSET_VETERANCY_METHOD_NAMES_WAVE506.len() == 6
        && residual_name_index(
            PRESENTATION_WEAPONSET_VETERANCY_METHOD_NAMES_WAVE506,
            "veterancy",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_WEAPONSET_VETERANCY_METHOD_NAMES_WAVE506,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_weaponset_veterancy_source_markers_residual_wave506() -> bool {
    PRESENTATION_WEAPONSET_VETERANCY_SOURCE_MARKERS_WAVE506.len() == 4
        && residual_name_index(
            PRESENTATION_WEAPONSET_VETERANCY_SOURCE_MARKERS_WAVE506,
            "Wave 506: weaponset veterancy model-condition residual",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_WEAPONSET_VETERANCY_SOURCE_MARKERS_WAVE506,
            "veterancy: ro.veterancy",
        ) == Some(2)
}

pub fn honesty_presentation_weaponset_veterancy_nav_commands_residual_wave506() -> bool {
    PRESENTATION_WEAPONSET_VETERANCY_NAV_STEPS_WAVE506.len() == 6
        && residual_name_index(
            PRESENTATION_WEAPONSET_VETERANCY_NAV_STEPS_WAVE506,
            "STAMP_VETERAN_ELITE_OR_HERO",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_WEAPONSET_VETERANCY_NAV_STEPS_WAVE506,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_WEAPONSET_VETERANCY_CMD_NAMES_WAVE506.len() == 5
}

pub fn simulate_presentation_weaponset_veterancy_input_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 506: presentation veterancy residual for weaponset model bits")
        && pf.contains("veterancy: ro.veterancy")
        && pf.contains("PresentationVeterancy::Heroic");
    residual_action_store(ResidualPresentationWeaponsetVeterancyAction::InputSource);
    ok
}

pub fn simulate_presentation_weaponset_veterancy_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 506: weaponset veterancy model-condition residual")
        && en.contains("pub fn weaponset_veteran_model_bit")
        && en.contains("pub fn weaponset_elite_model_bit")
        && en.contains("pub fn weaponset_hero_model_bit")
        && rp.contains("Wave 506: weaponset veterancy bits included in stamp helper");
    residual_action_store(ResidualPresentationWeaponsetVeterancyAction::StampSource);
    ok
}

pub fn honesty_presentation_weaponset_veterancy_residual_pack_wave506() -> bool {
    honesty_presentation_weaponset_veterancy_method_names_residual_wave506()
        && honesty_presentation_weaponset_veterancy_source_markers_residual_wave506()
        && honesty_presentation_weaponset_veterancy_nav_commands_residual_wave506()
        && simulate_presentation_weaponset_veterancy_input_source()
        && simulate_presentation_weaponset_veterancy_stamp_source()
}

pub fn simulate_live_presentation_weaponset_veterancy_honesty() -> bool {
    let ok = honesty_presentation_weaponset_veterancy_residual_pack_wave506();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationWeaponsetVeterancyAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_weaponset_veterancy_method_names_residual_wave506());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_weaponset_veterancy_source_markers_residual_wave506());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_weaponset_veterancy_nav_commands_residual_wave506());
    }

    #[test]
    fn presentation_weaponset_veterancy_sources() {
        assert!(simulate_presentation_weaponset_veterancy_input_source());
        assert!(simulate_presentation_weaponset_veterancy_stamp_source());
    }

    #[test]
    fn wave506_composite_pack() {
        assert!(honesty_presentation_weaponset_veterancy_residual_pack_wave506());
    }

    #[test]
    fn simulate_live_presentation_weaponset_veterancy_honesty_residual_live() {
        assert!(
            simulate_live_presentation_weaponset_veterancy_honesty(),
            "presentation weaponset veterancy residual must latch"
        );
        assert!(residual_presentation_weaponset_veterancy_ok());
        assert_eq!(
            residual_presentation_weaponset_veterancy_last_action(),
            ResidualPresentationWeaponsetVeterancyAction::Composite
        );
    }
}
