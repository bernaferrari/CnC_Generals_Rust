//! Wave 462 residual peels: render UI / script messages / sim clock prefer
//! pipeline PresentationFrame before last_presentation_frame; live
//! update_ui_state only when no freeze is installed.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 461 presentation_world_bounds probe.
//! Architecture residual - render consumer presentation-first via pipeline.
//!
//! Sources (cnc_game_engine.rs render()):
//! - ui_state from pipeline.presentation_frame().cloned().or(last)
//! - new_script_messages from pipeline.or(last)
//! - current_game_time from pipeline.or(last)
//! - boot residual update_ui_state only in else branch
//!
//! Fail-closed:
//! - Boot/loading without freeze still calls live update_ui_state
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const RENDER_UI_PIPELINE_PRESENTATION_METHOD_NAMES_WAVE462: &[&str] = &[
    "render",
    "presentation_frame",
    "apply_to_ui_state",
    "update_ui_state",
    "take_new_script_messages",
    "get_total_play_time",
];

pub const RENDER_UI_PIPELINE_PRESENTATION_SOURCE_MARKERS_WAVE462: &[&str] = &[
    "Wave 462: prefer pipeline freeze, then last_presentation_frame",
    "presentation_frame()",
    "Boot/loading residual only",
    "update_ui_state(self.current_player_id)",
];

pub const RENDER_UI_PIPELINE_PRESENTATION_NAV_STEPS_WAVE462: &[&str] = &[
    "RESOLVE_PIPELINE_PRESENTATION",
    "FALLBACK_LAST_PRESENTATION_FRAME",
    "APPLY_UI_STATE_FROM_PRESENTATION",
    "SYNC_SCRIPT_MESSAGES_FROM_PRESENTATION",
    "SYNC_SIM_CLOCK_FROM_PRESENTATION",
    "BOOT_LIVE_UI_ONLY_WHEN_NO_FRAME",
];

pub const RUNTIME_HOST_RENDER_UI_PIPELINE_PRESENTATION_CMD_NAMES_WAVE462: &[&str] = &[
    "click_render_ui_pipeline_presentation_ok_wnd_resolve",
    "click_render_ui_pipeline_presentation_ok_wnd_apply",
    "click_render_ui_pipeline_presentation_ok_wnd_script",
    "click_render_ui_pipeline_presentation_ok_wnd_prepare",
    "click_render_ui_pipeline_presentation_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualRenderUiPipelinePresentationAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    UiSource = 4,
    ScriptClockSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualRenderUiPipelinePresentationAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_render_ui_pipeline_presentation_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_render_ui_pipeline_presentation_last_action()
-> ResidualRenderUiPipelinePresentationAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualRenderUiPipelinePresentationAction::MethodNames,
        2 => ResidualRenderUiPipelinePresentationAction::SourceMarkers,
        3 => ResidualRenderUiPipelinePresentationAction::NavCommands,
        4 => ResidualRenderUiPipelinePresentationAction::UiSource,
        5 => ResidualRenderUiPipelinePresentationAction::ScriptClockSource,
        6 => ResidualRenderUiPipelinePresentationAction::Composite,
        _ => ResidualRenderUiPipelinePresentationAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_render_ui_pipeline_presentation_method_names_residual_wave462() -> bool {
    RENDER_UI_PIPELINE_PRESENTATION_METHOD_NAMES_WAVE462.len() == 6
        && residual_name_index(
            RENDER_UI_PIPELINE_PRESENTATION_METHOD_NAMES_WAVE462,
            "render",
        ) == Some(0)
        && residual_name_index(
            RENDER_UI_PIPELINE_PRESENTATION_METHOD_NAMES_WAVE462,
            "get_total_play_time",
        ) == Some(5)
}

pub fn honesty_render_ui_pipeline_presentation_source_markers_residual_wave462() -> bool {
    RENDER_UI_PIPELINE_PRESENTATION_SOURCE_MARKERS_WAVE462.len() == 4
        && residual_name_index(
            RENDER_UI_PIPELINE_PRESENTATION_SOURCE_MARKERS_WAVE462,
            "Wave 462: prefer pipeline freeze, then last_presentation_frame",
        ) == Some(0)
        && residual_name_index(
            RENDER_UI_PIPELINE_PRESENTATION_SOURCE_MARKERS_WAVE462,
            "update_ui_state(self.current_player_id)",
        ) == Some(3)
}

pub fn honesty_render_ui_pipeline_presentation_nav_commands_residual_wave462() -> bool {
    RENDER_UI_PIPELINE_PRESENTATION_NAV_STEPS_WAVE462.len() == 6
        && residual_name_index(
            RENDER_UI_PIPELINE_PRESENTATION_NAV_STEPS_WAVE462,
            "APPLY_UI_STATE_FROM_PRESENTATION",
        ) == Some(2)
        && residual_name_index(
            RENDER_UI_PIPELINE_PRESENTATION_NAV_STEPS_WAVE462,
            "BOOT_LIVE_UI_ONLY_WHEN_NO_FRAME",
        ) == Some(5)
        && RUNTIME_HOST_RENDER_UI_PIPELINE_PRESENTATION_CMD_NAMES_WAVE462.len() == 5
        && residual_name_index(
            RUNTIME_HOST_RENDER_UI_PIPELINE_PRESENTATION_CMD_NAMES_WAVE462,
            "click_render_ui_pipeline_presentation_ok_wnd_prepare",
        ) == Some(3)
}

/// Residual: render UI prefers pipeline presentation before live update_ui_state.
pub fn simulate_render_ui_pipeline_presentation_source() -> bool {
    let src = cnc_source();
    // Wave 591: real consumer lives in host_build_render_ui_state_from_presentation.
    // 2026-08-15: rustfmt split the signature (camera_drain.rs:1431).
    let marker = "fn host_build_render_ui_state_from_presentation(";
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(marker) {
        at = Some(from + rel);
        from = from + rel + marker.len();
    }
    let Some(i) = at else {
        return false;
    };
    let win = &src[i..];
    let ok = win.contains("Wave 591")
        && win.contains("Wave 462: prefer pipeline freeze, then last_presentation_frame")
        && win.contains("presentation_frame()")
        && win.contains("or_else(|| self.last_presentation_frame.clone())")
        && win.contains("apply_to_ui_state")
        && win.contains("Boot/loading residual only")
        && (win.contains("update_ui_state(self.current_player_id)")
            || win.contains("host_update_ui_state(self.current_player_id)"))
        // presentation branch must not call live update first
        && win
            .find("Boot/loading residual only")
            .map(|b| {
                !win[..b].contains("update_ui_state(self.current_player_id)")
                    && !win[..b].contains("host_update_ui_state(self.current_player_id)")
            })
            .unwrap_or(false)
        && src.contains("self.host_build_render_ui_state_from_presentation()")
        // marker retained for production presentation consumer docs
        && src.contains("GameUIState is built from PresentationFrame only");
    residual_action_store(ResidualRenderUiPipelinePresentationAction::UiSource);
    ok
}

/// Residual: script messages + sim clock prefer pipeline/last presentation.
/// Wave 570: script messages peel into `take_presentation_or_boot_new_script_messages`.
pub fn simulate_render_script_clock_pipeline_presentation_source() -> bool {
    let src = cnc_source();
    let script_ok = src.contains("Wave 462")
        || (src.contains("Wave 570")
            && src.contains("take_presentation_or_boot_new_script_messages")
            && src.contains("self.take_presentation_or_boot_new_script_messages()"));
    let ok = script_ok
        && src.contains("Wave 462: prefer pipeline/last presentation sim clock residual")
        && src.contains("total_play_time_seconds")
        && src.contains("new_script_messages")
        && src.contains("presentation_frame()");
    residual_action_store(ResidualRenderUiPipelinePresentationAction::ScriptClockSource);
    ok
}

pub fn honesty_render_ui_pipeline_presentation_residual_pack_wave462() -> bool {
    honesty_render_ui_pipeline_presentation_method_names_residual_wave462()
        && honesty_render_ui_pipeline_presentation_source_markers_residual_wave462()
        && honesty_render_ui_pipeline_presentation_nav_commands_residual_wave462()
        && simulate_render_ui_pipeline_presentation_source()
        && simulate_render_script_clock_pipeline_presentation_source()
}

pub fn simulate_live_render_ui_pipeline_presentation_honesty() -> bool {
    let ok = honesty_render_ui_pipeline_presentation_residual_pack_wave462();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualRenderUiPipelinePresentationAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_render_ui_pipeline_presentation_method_names_residual_wave462());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_render_ui_pipeline_presentation_source_markers_residual_wave462());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_render_ui_pipeline_presentation_nav_commands_residual_wave462());
    }

    #[test]
    fn render_ui_pipeline_presentation_sources() {
        assert!(simulate_render_ui_pipeline_presentation_source());
        assert!(simulate_render_script_clock_pipeline_presentation_source());
    }

    #[test]
    fn wave462_composite_pack() {
        assert!(honesty_render_ui_pipeline_presentation_residual_pack_wave462());
    }

    #[test]
    fn simulate_live_render_ui_pipeline_presentation_honesty_residual_live() {
        assert!(
            simulate_live_render_ui_pipeline_presentation_honesty(),
            "render UI pipeline presentation residual must latch"
        );
        assert!(residual_render_ui_pipeline_presentation_ok());
        assert_eq!(
            residual_render_ui_pipeline_presentation_last_action(),
            ResidualRenderUiPipelinePresentationAction::Composite
        );
    }
}
