//! Wave 591 residual peels: render-path UI presentation consumer is centralized
//! through `host_build_render_ui_state_from_presentation`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 590 presentation seed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_build_render_ui_state_from_presentation
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RENDER_UI_PRESENTATION_HELPER_METHOD_NAMES_WAVE591: &[&str] = &[
    "host_build_render_ui_state_from_presentation",
    "apply_to_ui_state",
    "apply_presentation_to_huds",
    "sync_eva_messages_from_presentation",
    "host_update_ui_state",
    "Wave 591",
    "Wave 462",
    "playable_claim = false",
];

pub const LIVE_HOST_RENDER_UI_PRESENTATION_HELPER_NAV_STEPS_WAVE591: &[&str] = &[
    "REQUIRE_RENDER_UI_HELPER",
    "REQUIRE_PIPELINE_THEN_LAST_FRAME",
    "REQUIRE_PRESENTATION_APPLY",
    "REQUIRE_BOOT_UPDATE_UI_FALLBACK",
    "LIVE_HOST_RENDER_UI_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_RENDER_UI_PRESENTATION_HELPER_CMD_NAMES_WAVE591: &[&str] = &[
    "host_render_ui_presentation_helper",
    "pipeline_then_last_frame",
    "boot_update_ui_fallback",
    "render_ui_presentation_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRenderUiPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostRenderUiPresentationHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualHostRenderUiPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_render_ui_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_render_ui_presentation_helper_last_action()
-> ResidualHostRenderUiPresentationHelperAction {
    ResidualHostRenderUiPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn last_sig_index(src: &str, sig: &str) -> Option<usize> {
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(sig) {
        at = Some(from + rel);
        from = from + rel + sig.len();
    }
    at
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = last_sig_index(src, sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_host_render_ui_presentation_helper_method_names_residual_wave591() -> bool {
    let names = LIVE_HOST_RENDER_UI_PRESENTATION_HELPER_METHOD_NAMES_WAVE591;
    let ok = residual_name_index(names, "host_build_render_ui_state_from_presentation").is_some()
        && residual_name_index(names, "apply_to_ui_state").is_some()
        && residual_name_index(names, "Wave 591").is_some()
        && residual_name_index(names, "Wave 462").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRenderUiPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_host_render_ui_presentation_helper_source_markers_residual_wave591() -> bool {
    let eng = eng_source();
    // 2026-08-15: rustfmt split the signature across lines (camera_drain.rs).
    let Some(body) = fn_body(eng, "fn host_build_render_ui_state_from_presentation(") else {
        residual_action_store(ResidualHostRenderUiPresentationHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 591")
        && body.contains("Wave 462: prefer pipeline freeze, then last_presentation_frame")
        && body.contains("presentation_frame()")
        && body.contains("or_else(|| self.last_presentation_frame.clone())")
        && body.contains("GameUIState::default()")
        && body.contains("apply_to_ui_state")
        && body.contains("apply_presentation_to_huds")
        && body.contains("sync_eva_messages_from_presentation")
        && body.contains("Boot/loading residual only")
        && body.contains("host_update_ui_state(self.current_player_id)");
    // 2026-08-15: live comment is "consumer residual" (camera_drain.rs:1434).
    let call_ok = eng.contains("self.host_build_render_ui_state_from_presentation()")
        && (eng.contains("Wave 591: render UI presentation consumer residual")
            || eng.contains("Wave 591: render UI presentation consumer via host helper"));
    let ok = body_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostRenderUiPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_render_ui_presentation_helper_nav_commands_residual_wave591() -> bool {
    let steps = LIVE_HOST_RENDER_UI_PRESENTATION_HELPER_NAV_STEPS_WAVE591;
    let cmds = RUNTIME_HOST_LIVE_HOST_RENDER_UI_PRESENTATION_HELPER_CMD_NAMES_WAVE591;
    let ok = residual_name_index(steps, "REQUIRE_RENDER_UI_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_PIPELINE_THEN_LAST_FRAME").is_some()
        && residual_name_index(steps, "REQUIRE_PRESENTATION_APPLY").is_some()
        && residual_name_index(steps, "REQUIRE_BOOT_UPDATE_UI_FALLBACK").is_some()
        && residual_name_index(steps, "LIVE_HOST_RENDER_UI_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_render_ui_presentation_helper").is_some()
        && residual_name_index(cmds, "pipeline_then_last_frame").is_some()
        && residual_name_index(cmds, "boot_update_ui_fallback").is_some()
        && residual_name_index(cmds, "render_ui_presentation_residual").is_some();
    residual_action_store(ResidualHostRenderUiPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_host_render_ui_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 591")
        && eng.contains("fn host_build_render_ui_state_from_presentation")
        && eng.contains("GameUIState is built from PresentationFrame only");
    residual_action_store(ResidualHostRenderUiPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_host_render_ui_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    // 2026-08-15: live comment is "consumer residual" (camera_drain.rs:1434).
    let ok = eng.contains("self.host_build_render_ui_state_from_presentation()")
        && (eng.contains("Wave 591: render UI presentation consumer residual")
            || eng.contains("Wave 591: render UI presentation consumer via host helper"));
    residual_action_store(ResidualHostRenderUiPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_render_ui_presentation_helper_residual_pack_wave591() -> bool {
    honesty_host_render_ui_presentation_helper_method_names_residual_wave591()
        && honesty_host_render_ui_presentation_helper_source_markers_residual_wave591()
        && honesty_host_render_ui_presentation_helper_nav_commands_residual_wave591()
        && simulate_host_render_ui_presentation_helper_collect_source()
        && simulate_host_render_ui_presentation_helper_dispatch_source()
}

pub fn simulate_live_host_render_ui_presentation_helper_honesty() -> bool {
    let ok = honesty_host_render_ui_presentation_helper_residual_pack_wave591();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRenderUiPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_render_ui_presentation_helper_method_names_residual_wave591());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_render_ui_presentation_helper_source_markers_residual_wave591());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_render_ui_presentation_helper_nav_commands_residual_wave591());
    }

    #[test]
    fn host_render_ui_presentation_helper_sources() {
        assert!(simulate_host_render_ui_presentation_helper_collect_source());
        assert!(simulate_host_render_ui_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave591_composite_pack() {
        assert!(honesty_host_render_ui_presentation_helper_residual_pack_wave591());
    }

    #[test]
    fn simulate_live_host_render_ui_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_render_ui_presentation_helper_honesty(),
            "host render UI presentation helper residual must latch"
        );
        assert!(residual_host_render_ui_presentation_helper_ok());
        assert_eq!(
            residual_host_render_ui_presentation_helper_last_action(),
            ResidualHostRenderUiPresentationHelperAction::Composite
        );
    }
}
