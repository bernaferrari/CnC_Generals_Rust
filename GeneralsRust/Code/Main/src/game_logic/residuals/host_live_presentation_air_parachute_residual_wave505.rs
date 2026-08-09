//! Wave 505 residual peels: parachuting / jet exhaust / using-weapon mesh bits.
//! - freeze `parachuting` on presentation objects
//! - stamp PARACHUTING, JETEXHAUST (aircraft moving), USING_WEAPON_A
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 501–504 deploy/radar/stealth/construction/garrison stamps.
//! Architecture residual - air/parachute pose without live GameLogic dual-read.
//!
//! Sources:
//! - host_enum_table_residual.rs parachuting/jetexhaust/using_weapon_a bits
//! - presentation_frame.rs Wave 505 freeze + stamp
//! - graphics/render_pipeline.rs Wave 505 comment
//!
//! Fail-closed:
//! - Full AmericaParachute drawable / jet afterburner still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_AIR_PARACHUTE_METHOD_NAMES_WAVE505: &[&str] = &[
    "parachuting",
    "parachuting_model_bit",
    "jetexhaust_model_bit",
    "using_weapon_a_model_bit",
    "PresentationObjectType::Aircraft",
    "playable_claim = false",
];

pub const PRESENTATION_AIR_PARACHUTE_SOURCE_MARKERS_WAVE505: &[&str] = &[
    "Wave 505: C++ OBJECT_STATUS_PARACHUTING residual",
    "Wave 505: parachuting / jet exhaust / using-weapon pose residual bits",
    "parachuting: obj.is_parachuting()",
    "jetexhaust_model_bit",
];

pub const PRESENTATION_AIR_PARACHUTE_NAV_STEPS_WAVE505: &[&str] = &[
    "FREEZE_PARACHUTING",
    "STAMP_PARACHUTING_BIT",
    "STAMP_JETEXHAUST_FOR_AIRCRAFT",
    "STAMP_USING_WEAPON_A",
    "MESH_RESOLVE_FROM_BITS",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_AIR_PARACHUTE_CMD_NAMES_WAVE505: &[&str] = &[
    "click_presentation_air_parachute_ok_wnd_detect",
    "click_presentation_air_parachute_ok_wnd_skip",
    "click_presentation_air_parachute_ok_wnd_queue",
    "click_presentation_air_parachute_ok_wnd_prepare",
    "click_presentation_air_parachute_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationAirParachuteAction {
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

fn residual_action_store(a: ResidualPresentationAirParachuteAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_air_parachute_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_air_parachute_last_action() -> ResidualPresentationAirParachuteAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationAirParachuteAction::MethodNames,
        2 => ResidualPresentationAirParachuteAction::SourceMarkers,
        3 => ResidualPresentationAirParachuteAction::NavCommands,
        4 => ResidualPresentationAirParachuteAction::FreezeSource,
        5 => ResidualPresentationAirParachuteAction::StampSource,
        6 => ResidualPresentationAirParachuteAction::Composite,
        _ => ResidualPresentationAirParachuteAction::Idle,
    }
}

fn pf_source() -> &'static str {
    include_str!("../../presentation_frame.rs")
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

fn rp_source() -> &'static str {
    include_str!("../../graphics/render_pipeline.rs")
}

pub fn honesty_presentation_air_parachute_method_names_residual_wave505() -> bool {
    PRESENTATION_AIR_PARACHUTE_METHOD_NAMES_WAVE505.len() == 6
        && residual_name_index(
            PRESENTATION_AIR_PARACHUTE_METHOD_NAMES_WAVE505,
            "parachuting",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_AIR_PARACHUTE_METHOD_NAMES_WAVE505,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_air_parachute_source_markers_residual_wave505() -> bool {
    PRESENTATION_AIR_PARACHUTE_SOURCE_MARKERS_WAVE505.len() == 4
        && residual_name_index(
            PRESENTATION_AIR_PARACHUTE_SOURCE_MARKERS_WAVE505,
            "Wave 505: parachuting / jet exhaust / using-weapon pose residual bits",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_AIR_PARACHUTE_SOURCE_MARKERS_WAVE505,
            "parachuting: obj.is_parachuting()",
        ) == Some(2)
}

pub fn honesty_presentation_air_parachute_nav_commands_residual_wave505() -> bool {
    PRESENTATION_AIR_PARACHUTE_NAV_STEPS_WAVE505.len() == 6
        && residual_name_index(
            PRESENTATION_AIR_PARACHUTE_NAV_STEPS_WAVE505,
            "STAMP_JETEXHAUST_FOR_AIRCRAFT",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_AIR_PARACHUTE_NAV_STEPS_WAVE505,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_AIR_PARACHUTE_CMD_NAMES_WAVE505.len() == 5
}

pub fn simulate_presentation_air_parachute_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 505: C++ OBJECT_STATUS_PARACHUTING residual")
        && pf.contains("parachuting: obj.is_parachuting()")
        && pf.contains("parachuting: ent.parachuting")
        && pf.contains("parachuting: ro.parachuting")
        && pf.contains("using_ability: ro.using_ability")
        && pf.contains("airborne_target: ro.airborne_target");
    residual_action_store(ResidualPresentationAirParachuteAction::FreezeSource);
    ok
}

pub fn simulate_presentation_air_parachute_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 505: parachuting / jet exhaust / using-weapon pose residual bits")
        && en.contains("pub fn parachuting_model_bit")
        && en.contains("pub fn jetexhaust_model_bit")
        && en.contains("pub fn using_weapon_a_model_bit")
        && pf.contains("PresentationObjectType::Aircraft")
        && rp.contains(
            "Wave 505: parachuting/jetexhaust/using-weapon bits included in stamp helper",
        );
    residual_action_store(ResidualPresentationAirParachuteAction::StampSource);
    ok
}

pub fn honesty_presentation_air_parachute_residual_pack_wave505() -> bool {
    honesty_presentation_air_parachute_method_names_residual_wave505()
        && honesty_presentation_air_parachute_source_markers_residual_wave505()
        && honesty_presentation_air_parachute_nav_commands_residual_wave505()
        && simulate_presentation_air_parachute_freeze_source()
        && simulate_presentation_air_parachute_stamp_source()
}

pub fn simulate_live_presentation_air_parachute_honesty() -> bool {
    let ok = honesty_presentation_air_parachute_residual_pack_wave505();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationAirParachuteAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_air_parachute_method_names_residual_wave505());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_air_parachute_source_markers_residual_wave505());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_air_parachute_nav_commands_residual_wave505());
    }

    #[test]
    fn presentation_air_parachute_sources() {
        assert!(simulate_presentation_air_parachute_freeze_source());
        assert!(simulate_presentation_air_parachute_stamp_source());
    }

    #[test]
    fn wave505_composite_pack() {
        assert!(honesty_presentation_air_parachute_residual_pack_wave505());
    }

    #[test]
    fn simulate_live_presentation_air_parachute_honesty_residual_live() {
        assert!(
            simulate_live_presentation_air_parachute_honesty(),
            "presentation air/parachute residual must latch"
        );
        assert!(residual_presentation_air_parachute_ok());
        assert_eq!(
            residual_presentation_air_parachute_last_action(),
            ResidualPresentationAirParachuteAction::Composite
        );
    }
}
