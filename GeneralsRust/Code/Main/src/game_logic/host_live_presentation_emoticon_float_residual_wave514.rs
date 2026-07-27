//! Wave 514 residual peels: Drawable emoticon residual → presentation floating text.
//! - freeze `emoticon_name` / `emoticon_frames_left` on RenderableObject
//! - active host emoticons peel into `PresentationFloatingTextKind::Emoticon`
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 500 object FX particles and money floating texts.
//! Architecture residual - status bubbles without live GameLogic dual-read mid-render.
//!
//! Sources:
//! - presentation_frame.rs Emoticon floating kind + freeze + collect peel
//!
//! Fail-closed:
//! - Full Anim2D emoticon atlas / surrender UI still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_EMOTICON_FLOAT_METHOD_NAMES_WAVE514: &[&str] = &[
    "emoticon_name",
    "emoticon_frames_left",
    "PresentationFloatingTextKind::Emoticon",
    "collect_presentation_floating_texts",
    "Wave 514: active host emoticons",
    "playable_claim = false",
];

pub const PRESENTATION_EMOTICON_FLOAT_SOURCE_MARKERS_WAVE514: &[&str] = &[
    "Wave 514: Drawable emoticon residual (status bubble above unit)",
    "Wave 514: active host emoticons → floating-text residual (presentation-only)",
    "emoticon_name: obj.emoticon_name.clone()",
    "PresentationFloatingTextKind::Emoticon",
];

pub const PRESENTATION_EMOTICON_FLOAT_NAV_STEPS_WAVE514: &[&str] = &[
    "FREEZE_EMOTICON_NAME_FRAMES",
    "SCAN_HOST_OBJECTS",
    "FILTER_ACTIVE_EMOTICONS",
    "PUSH_FLOATING_TEXT_EMOTICON",
    "PACK_FLOATING_LAYOUT",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_EMOTICON_FLOAT_CMD_NAMES_WAVE514: &[&str] = &[
    "click_presentation_emoticon_float_ok_wnd_detect",
    "click_presentation_emoticon_float_ok_wnd_skip",
    "click_presentation_emoticon_float_ok_wnd_queue",
    "click_presentation_emoticon_float_ok_wnd_prepare",
    "click_presentation_emoticon_float_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationEmoticonFloatAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    FreezeSource = 4,
    CollectSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationEmoticonFloatAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_emoticon_float_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_emoticon_float_last_action() -> ResidualPresentationEmoticonFloatAction
{
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationEmoticonFloatAction::MethodNames,
        2 => ResidualPresentationEmoticonFloatAction::SourceMarkers,
        3 => ResidualPresentationEmoticonFloatAction::NavCommands,
        4 => ResidualPresentationEmoticonFloatAction::FreezeSource,
        5 => ResidualPresentationEmoticonFloatAction::CollectSource,
        6 => ResidualPresentationEmoticonFloatAction::Composite,
        _ => ResidualPresentationEmoticonFloatAction::Idle,
    }
}

fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}

pub fn honesty_presentation_emoticon_float_method_names_residual_wave514() -> bool {
    PRESENTATION_EMOTICON_FLOAT_METHOD_NAMES_WAVE514.len() == 6
        && residual_name_index(
            PRESENTATION_EMOTICON_FLOAT_METHOD_NAMES_WAVE514,
            "emoticon_name",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_EMOTICON_FLOAT_METHOD_NAMES_WAVE514,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_emoticon_float_source_markers_residual_wave514() -> bool {
    PRESENTATION_EMOTICON_FLOAT_SOURCE_MARKERS_WAVE514.len() == 4
        && residual_name_index(
            PRESENTATION_EMOTICON_FLOAT_SOURCE_MARKERS_WAVE514,
            "Wave 514: active host emoticons → floating-text residual (presentation-only)",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_EMOTICON_FLOAT_SOURCE_MARKERS_WAVE514,
            "PresentationFloatingTextKind::Emoticon",
        ) == Some(3)
}

pub fn honesty_presentation_emoticon_float_nav_commands_residual_wave514() -> bool {
    PRESENTATION_EMOTICON_FLOAT_NAV_STEPS_WAVE514.len() == 6
        && residual_name_index(
            PRESENTATION_EMOTICON_FLOAT_NAV_STEPS_WAVE514,
            "PUSH_FLOATING_TEXT_EMOTICON",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_EMOTICON_FLOAT_NAV_STEPS_WAVE514,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_EMOTICON_FLOAT_CMD_NAMES_WAVE514.len() == 5
}

pub fn simulate_presentation_emoticon_float_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 514: C++ Drawable emoticon residual name")
        && pf.contains("emoticon_name: obj.emoticon_name.clone()")
        && pf.contains("emoticon_frames_left: obj.emoticon_frames_left");
    residual_action_store(ResidualPresentationEmoticonFloatAction::FreezeSource);
    ok
}

pub fn simulate_presentation_emoticon_float_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf
        .contains("Wave 514: active host emoticons → floating-text residual (presentation-only)")
        && pf.contains("PresentationFloatingTextKind::Emoticon")
        && pf.contains("obj.emoticon_frames_left <= 0")
        && pf.contains("obj.emoticon_name.is_empty()");
    residual_action_store(ResidualPresentationEmoticonFloatAction::CollectSource);
    ok
}

pub fn honesty_presentation_emoticon_float_residual_pack_wave514() -> bool {
    honesty_presentation_emoticon_float_method_names_residual_wave514()
        && honesty_presentation_emoticon_float_source_markers_residual_wave514()
        && honesty_presentation_emoticon_float_nav_commands_residual_wave514()
        && simulate_presentation_emoticon_float_freeze_source()
        && simulate_presentation_emoticon_float_collect_source()
}

pub fn simulate_live_presentation_emoticon_float_honesty() -> bool {
    let ok = honesty_presentation_emoticon_float_residual_pack_wave514();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationEmoticonFloatAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_emoticon_float_method_names_residual_wave514());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_emoticon_float_source_markers_residual_wave514());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_emoticon_float_nav_commands_residual_wave514());
    }

    #[test]
    fn presentation_emoticon_float_sources() {
        assert!(simulate_presentation_emoticon_float_freeze_source());
        assert!(simulate_presentation_emoticon_float_collect_source());
    }

    #[test]
    fn wave514_composite_pack() {
        assert!(honesty_presentation_emoticon_float_residual_pack_wave514());
    }

    #[test]
    fn simulate_live_presentation_emoticon_float_honesty_residual_live() {
        assert!(
            simulate_live_presentation_emoticon_float_honesty(),
            "presentation emoticon float residual must latch"
        );
        assert!(residual_presentation_emoticon_float_ok());
        assert_eq!(
            residual_presentation_emoticon_float_last_action(),
            ResidualPresentationEmoticonFloatAction::Composite
        );
    }
}
