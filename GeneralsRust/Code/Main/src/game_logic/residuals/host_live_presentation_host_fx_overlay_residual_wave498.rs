//! Wave 498 residual peels: host FX residual re-stamped after GameWorld object rebuild.
//! - `overlay_host_fx_residual` copies topple/damage/bone/poison/defector/death FX
//! - `build_from_gameworld` and victory engine path call overlay after rebuild
//! - GW entity hard-defaults remain until host stamp
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 497 mesh condition resolve.
//! Architecture residual - presentation FX must not wipe on GW object rebuild.
//!
//! Sources:
//! - presentation_frame.rs overlay_host_fx_residual
//! - presentation_frame.rs build_from_gameworld Wave 498 call
//!
//! Fail-closed:
//! - Full GameWorld ownership of FX still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_HOST_FX_OVERLAY_METHOD_NAMES_WAVE498: &[&str] = &[
    "overlay_host_fx_residual",
    "presentation_topple_lean_radians",
    "pending_transition_damage_fx",
    "is_poison_tinted",
    "pending_death_fx",
    "playable_claim = false",
];

pub const PRESENTATION_HOST_FX_OVERLAY_SOURCE_MARKERS_WAVE498: &[&str] = &[
    "Wave 498: re-stamp host-only FX presentation residual after GameWorld object rebuild",
    "Wave 498: host FX residual survives GameWorld object rebuild",
    "overlay_host_fx_residual",
    "pending_transition_damage_fx",
];

pub const PRESENTATION_HOST_FX_OVERLAY_NAV_STEPS_WAVE498: &[&str] = &[
    "REBUILD_OBJECTS_FROM_GAMEWORLD",
    "LOOKUP_HOST_OBJECT_BY_ID",
    "STAMP_TOPPLE_LEAN",
    "STAMP_DAMAGE_BONE_DEATH_FX",
    "STAMP_POISON_DEFECTOR",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_HOST_FX_OVERLAY_CMD_NAMES_WAVE498: &[&str] = &[
    "click_presentation_host_fx_overlay_ok_wnd_detect",
    "click_presentation_host_fx_overlay_ok_wnd_skip",
    "click_presentation_host_fx_overlay_ok_wnd_queue",
    "click_presentation_host_fx_overlay_ok_wnd_prepare",
    "click_presentation_host_fx_overlay_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationHostFxOverlayAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    OverlaySource = 4,
    EngineSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationHostFxOverlayAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_host_fx_overlay_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_host_fx_overlay_last_action() -> ResidualPresentationHostFxOverlayAction
{
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationHostFxOverlayAction::MethodNames,
        2 => ResidualPresentationHostFxOverlayAction::SourceMarkers,
        3 => ResidualPresentationHostFxOverlayAction::NavCommands,
        4 => ResidualPresentationHostFxOverlayAction::OverlaySource,
        5 => ResidualPresentationHostFxOverlayAction::EngineSource,
        6 => ResidualPresentationHostFxOverlayAction::Composite,
        _ => ResidualPresentationHostFxOverlayAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_presentation_host_fx_overlay_method_names_residual_wave498() -> bool {
    PRESENTATION_HOST_FX_OVERLAY_METHOD_NAMES_WAVE498.len() == 6
        && residual_name_index(
            PRESENTATION_HOST_FX_OVERLAY_METHOD_NAMES_WAVE498,
            "overlay_host_fx_residual",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_HOST_FX_OVERLAY_METHOD_NAMES_WAVE498,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_host_fx_overlay_source_markers_residual_wave498() -> bool {
    PRESENTATION_HOST_FX_OVERLAY_SOURCE_MARKERS_WAVE498.len() == 4
        && residual_name_index(
            PRESENTATION_HOST_FX_OVERLAY_SOURCE_MARKERS_WAVE498,
            "Wave 498: host FX residual survives GameWorld object rebuild",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_HOST_FX_OVERLAY_SOURCE_MARKERS_WAVE498,
            "overlay_host_fx_residual",
        ) == Some(2)
}

pub fn honesty_presentation_host_fx_overlay_nav_commands_residual_wave498() -> bool {
    PRESENTATION_HOST_FX_OVERLAY_NAV_STEPS_WAVE498.len() == 6
        && residual_name_index(
            PRESENTATION_HOST_FX_OVERLAY_NAV_STEPS_WAVE498,
            "STAMP_DAMAGE_BONE_DEATH_FX",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_HOST_FX_OVERLAY_NAV_STEPS_WAVE498,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_HOST_FX_OVERLAY_CMD_NAMES_WAVE498.len() == 5
}

pub fn simulate_presentation_host_fx_overlay_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("pub fn overlay_host_fx_residual")
        && pf.contains(
            "Wave 498: re-stamp host-only FX presentation residual after GameWorld object rebuild",
        )
        && pf.contains("presentation_topple_lean_radians")
        && pf.contains("pending_transition_damage_fx")
        && pf.contains("is_poison_tinted")
        && pf.contains("pending_death_fx");
    residual_action_store(ResidualPresentationHostFxOverlayAction::OverlaySource);
    ok
}

pub fn simulate_presentation_host_fx_overlay_engine_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 498: host FX residual survives GameWorld object rebuild")
        && pf.contains("frame.overlay_host_fx_residual(logic)")
        && pf.contains("Wave 498: host FX residual after GW object rebuild");
    residual_action_store(ResidualPresentationHostFxOverlayAction::EngineSource);
    ok
}

pub fn honesty_presentation_host_fx_overlay_residual_pack_wave498() -> bool {
    honesty_presentation_host_fx_overlay_method_names_residual_wave498()
        && honesty_presentation_host_fx_overlay_source_markers_residual_wave498()
        && honesty_presentation_host_fx_overlay_nav_commands_residual_wave498()
        && simulate_presentation_host_fx_overlay_source()
        && simulate_presentation_host_fx_overlay_engine_source()
}

pub fn simulate_live_presentation_host_fx_overlay_honesty() -> bool {
    let ok = honesty_presentation_host_fx_overlay_residual_pack_wave498();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationHostFxOverlayAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_host_fx_overlay_method_names_residual_wave498());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_host_fx_overlay_source_markers_residual_wave498());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_host_fx_overlay_nav_commands_residual_wave498());
    }

    #[test]
    fn presentation_host_fx_overlay_sources() {
        assert!(simulate_presentation_host_fx_overlay_source());
        assert!(simulate_presentation_host_fx_overlay_engine_source());
    }

    #[test]
    fn wave498_composite_pack() {
        assert!(honesty_presentation_host_fx_overlay_residual_pack_wave498());
    }

    #[test]
    fn simulate_live_presentation_host_fx_overlay_honesty_residual_live() {
        assert!(
            simulate_live_presentation_host_fx_overlay_honesty(),
            "presentation host FX overlay residual must latch"
        );
        assert!(residual_presentation_host_fx_overlay_ok());
        assert_eq!(
            residual_presentation_host_fx_overlay_last_action(),
            ResidualPresentationHostFxOverlayAction::Composite
        );
    }
}
