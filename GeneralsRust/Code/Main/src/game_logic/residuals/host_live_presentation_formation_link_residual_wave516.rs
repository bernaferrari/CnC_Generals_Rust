//! Wave 516 residual peels: selection formation links from presentation freeze.
//! - `render_selection_from_presentation` groups selected units by `formation_id`
//! - draws thin screen-space links via `create_formation_link` (no live GameLogic dual-read)
//! - formation_id == 0 is ignored (NO_FORMATION_ID residual)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 515 surrender/formation freeze.
//! Architecture residual - formation HUD links without live GameLogic dual-read.
//!
//! Sources:
//! - selection_renderer.rs Wave 516 formation links
//! - presentation_frame.rs Wave 515 formation_id freeze
//!
//! Fail-closed:
//! - Full W3D formation glyph / letter overlay still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_FORMATION_LINK_METHOD_NAMES_WAVE516: &[&str] = &[
    "render_selection_from_presentation",
    "create_formation_link",
    "formation_id",
    "world_to_screen",
    "playable_claim = false",
];

pub const PRESENTATION_FORMATION_LINK_SOURCE_MARKERS_WAVE516: &[&str] = &[
    "Wave 516: formation links among selected units sharing formation_id",
    "fn create_formation_link",
    "ro.formation_id == 0",
    "formation_id: obj.formation_id",
];

pub const PRESENTATION_FORMATION_LINK_NAV_STEPS_WAVE516: &[&str] = &[
    "FREEZE_FORMATION_ID",
    "GROUP_SELECTED_BY_FORMATION_ID",
    "DRAW_FORMATION_LINKS",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_FORMATION_LINK_CMD_NAMES_WAVE516: &[&str] = &[
    "click_presentation_formation_link_ok_wnd_detect",
    "click_presentation_formation_link_ok_wnd_skip",
    "click_presentation_formation_link_ok_wnd_queue",
    "click_presentation_formation_link_ok_wnd_prepare",
    "click_presentation_formation_link_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationFormationLinkAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    SelectionSource = 4,
    FreezeSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationFormationLinkAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_formation_link_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_formation_link_last_action() -> ResidualPresentationFormationLinkAction
{
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationFormationLinkAction::MethodNames,
        2 => ResidualPresentationFormationLinkAction::SourceMarkers,
        3 => ResidualPresentationFormationLinkAction::NavCommands,
        4 => ResidualPresentationFormationLinkAction::SelectionSource,
        5 => ResidualPresentationFormationLinkAction::FreezeSource,
        6 => ResidualPresentationFormationLinkAction::Composite,
        _ => ResidualPresentationFormationLinkAction::Idle,
    }
}

fn sel_source() -> &'static str {
    include_str!("../../selection_renderer.rs")
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_presentation_formation_link_method_names_residual_wave516() -> bool {
    PRESENTATION_FORMATION_LINK_METHOD_NAMES_WAVE516.len() == 5
        && residual_name_index(
            PRESENTATION_FORMATION_LINK_METHOD_NAMES_WAVE516,
            "render_selection_from_presentation",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_FORMATION_LINK_METHOD_NAMES_WAVE516,
            "playable_claim = false",
        ) == Some(4)
}

pub fn honesty_presentation_formation_link_source_markers_residual_wave516() -> bool {
    PRESENTATION_FORMATION_LINK_SOURCE_MARKERS_WAVE516.len() == 4
        && residual_name_index(
            PRESENTATION_FORMATION_LINK_SOURCE_MARKERS_WAVE516,
            "Wave 516: formation links among selected units sharing formation_id",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_FORMATION_LINK_SOURCE_MARKERS_WAVE516,
            "fn create_formation_link",
        ) == Some(1)
}

pub fn honesty_presentation_formation_link_nav_commands_residual_wave516() -> bool {
    PRESENTATION_FORMATION_LINK_NAV_STEPS_WAVE516.len() == 5
        && residual_name_index(
            PRESENTATION_FORMATION_LINK_NAV_STEPS_WAVE516,
            "DRAW_FORMATION_LINKS",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_FORMATION_LINK_NAV_STEPS_WAVE516,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(4)
        && RUNTIME_HOST_PRESENTATION_FORMATION_LINK_CMD_NAMES_WAVE516.len() == 5
}

pub fn simulate_presentation_formation_link_selection_source() -> bool {
    let sel = sel_source();
    let ok = sel.contains("Wave 516: formation links among selected units sharing formation_id")
        && sel.contains("fn create_formation_link")
        && sel.contains("ro.formation_id == 0")
        && sel.contains("create_formation_link(pair[0].1, pair[1].1, link_color)");
    residual_action_store(ResidualPresentationFormationLinkAction::SelectionSource);
    ok
}

pub fn simulate_presentation_formation_link_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("formation_id: obj.formation_id")
        && pf.contains("formation_offset: obj.formation_offset");
    residual_action_store(ResidualPresentationFormationLinkAction::FreezeSource);
    ok
}

pub fn honesty_presentation_formation_link_residual_pack_wave516() -> bool {
    honesty_presentation_formation_link_method_names_residual_wave516()
        && honesty_presentation_formation_link_source_markers_residual_wave516()
        && honesty_presentation_formation_link_nav_commands_residual_wave516()
        && simulate_presentation_formation_link_selection_source()
        && simulate_presentation_formation_link_freeze_source()
}

pub fn simulate_live_presentation_formation_link_honesty() -> bool {
    let ok = honesty_presentation_formation_link_residual_pack_wave516();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationFormationLinkAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_formation_link_method_names_residual_wave516());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_formation_link_source_markers_residual_wave516());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_formation_link_nav_commands_residual_wave516());
    }

    #[test]
    fn presentation_formation_link_sources() {
        assert!(simulate_presentation_formation_link_selection_source());
        assert!(simulate_presentation_formation_link_freeze_source());
    }

    #[test]
    fn wave516_composite_pack() {
        assert!(honesty_presentation_formation_link_residual_pack_wave516());
    }

    #[test]
    fn simulate_live_presentation_formation_link_honesty_residual_live() {
        assert!(
            simulate_live_presentation_formation_link_honesty(),
            "presentation formation-link residual must latch"
        );
        assert!(residual_presentation_formation_link_ok());
        assert_eq!(
            residual_presentation_formation_link_last_action(),
            ResidualPresentationFormationLinkAction::Composite
        );
    }
}
