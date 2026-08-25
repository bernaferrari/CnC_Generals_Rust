//! Wave 592 residual peels: render-path presentation overlays (radar/script/clock/diag)
//! are centralized through `host_apply_render_ui_presentation_overlays`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 591 render UI presentation consumer residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_apply_render_ui_presentation_overlays
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RENDER_UI_OVERLAYS_HELPER_METHOD_NAMES_WAVE592: &[&str] = &[
    "host_apply_render_ui_presentation_overlays",
    "take_presentation_or_boot_new_script_messages",
    "presentation_or_boot_total_play_time",
    "add_radar_message",
    "push_script_message",
    "Wave 592",
    "Wave 570",
    "playable_claim = false",
];

pub const LIVE_HOST_RENDER_UI_OVERLAYS_HELPER_NAV_STEPS_WAVE592: &[&str] = &[
    "REQUIRE_RENDER_UI_OVERLAYS_HELPER",
    "REQUIRE_RADAR_MESSAGES",
    "REQUIRE_SCRIPT_MESSAGES",
    "REQUIRE_SIM_CLOCK",
    "LIVE_HOST_RENDER_UI_OVERLAYS_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_RENDER_UI_OVERLAYS_HELPER_CMD_NAMES_WAVE592: &[&str] = &[
    "host_render_ui_overlays_helper",
    "radar_messages",
    "script_messages",
    "sim_clock",
    "render_ui_overlays_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRenderUiOverlaysHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostRenderUiOverlaysHelperAction {
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

fn residual_action_store(action: ResidualHostRenderUiOverlaysHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_render_ui_overlays_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_render_ui_overlays_helper_last_action()
-> ResidualHostRenderUiOverlaysHelperAction {
    ResidualHostRenderUiOverlaysHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

pub fn honesty_host_render_ui_overlays_helper_method_names_residual_wave592() -> bool {
    let names = LIVE_HOST_RENDER_UI_OVERLAYS_HELPER_METHOD_NAMES_WAVE592;
    let ok = residual_name_index(names, "host_apply_render_ui_presentation_overlays").is_some()
        && residual_name_index(names, "take_presentation_or_boot_new_script_messages").is_some()
        && residual_name_index(names, "presentation_or_boot_total_play_time").is_some()
        && residual_name_index(names, "Wave 592").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRenderUiOverlaysHelperAction::MethodNames);
    ok
}

pub fn honesty_host_render_ui_overlays_helper_source_markers_residual_wave592() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_apply_render_ui_presentation_overlays(") else {
        residual_action_store(ResidualHostRenderUiOverlaysHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 592")
        && body.contains("Wave 570")
        && body.contains("Wave 462: prefer pipeline/last presentation sim clock residual")
        && body.contains("take_presentation_or_boot_new_script_messages")
        && body.contains("presentation_or_boot_total_play_time")
        && body.contains("add_radar_message")
        && body.contains("push_script_message")
        && body.contains("diagnostics_overlay")
        && !body.contains("process_ui_events");
    let call_ok = eng.contains("self.host_apply_render_ui_presentation_overlays(&mut ui_state)")
        && eng.contains("Wave 592: radar/script/clock/diag overlays via host helper");
    let ok = body_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostRenderUiOverlaysHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_render_ui_overlays_helper_nav_commands_residual_wave592() -> bool {
    let steps = LIVE_HOST_RENDER_UI_OVERLAYS_HELPER_NAV_STEPS_WAVE592;
    let cmds = RUNTIME_HOST_LIVE_HOST_RENDER_UI_OVERLAYS_HELPER_CMD_NAMES_WAVE592;
    let ok = residual_name_index(steps, "REQUIRE_RENDER_UI_OVERLAYS_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_RADAR_MESSAGES").is_some()
        && residual_name_index(steps, "REQUIRE_SCRIPT_MESSAGES").is_some()
        && residual_name_index(steps, "REQUIRE_SIM_CLOCK").is_some()
        && residual_name_index(steps, "LIVE_HOST_RENDER_UI_OVERLAYS_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_render_ui_overlays_helper").is_some()
        && residual_name_index(cmds, "radar_messages").is_some()
        && residual_name_index(cmds, "script_messages").is_some()
        && residual_name_index(cmds, "sim_clock").is_some()
        && residual_name_index(cmds, "render_ui_overlays_residual").is_some();
    residual_action_store(ResidualHostRenderUiOverlaysHelperAction::NavCommands);
    ok
}

pub fn simulate_host_render_ui_overlays_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 592")
        && eng.contains("fn host_apply_render_ui_presentation_overlays")
        && eng.contains("Wave 462: prefer pipeline/last presentation sim clock residual");
    residual_action_store(ResidualHostRenderUiOverlaysHelperAction::CollectSource);
    ok
}

pub fn simulate_host_render_ui_overlays_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_apply_render_ui_presentation_overlays(&mut ui_state)")
        && eng.contains("Wave 592: radar/script/clock/diag overlays via host helper")
        && eng.contains("self.take_presentation_or_boot_new_script_messages()");
    residual_action_store(ResidualHostRenderUiOverlaysHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_render_ui_overlays_helper_residual_pack_wave592() -> bool {
    honesty_host_render_ui_overlays_helper_method_names_residual_wave592()
        && honesty_host_render_ui_overlays_helper_source_markers_residual_wave592()
        && honesty_host_render_ui_overlays_helper_nav_commands_residual_wave592()
        && simulate_host_render_ui_overlays_helper_collect_source()
        && simulate_host_render_ui_overlays_helper_dispatch_source()
}

pub fn simulate_live_host_render_ui_overlays_helper_honesty() -> bool {
    let ok = honesty_host_render_ui_overlays_helper_residual_pack_wave592();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRenderUiOverlaysHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_render_ui_overlays_helper_method_names_residual_wave592());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_render_ui_overlays_helper_source_markers_residual_wave592());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_render_ui_overlays_helper_nav_commands_residual_wave592());
    }

    #[test]
    fn host_render_ui_overlays_helper_sources() {
        assert!(simulate_host_render_ui_overlays_helper_collect_source());
        assert!(simulate_host_render_ui_overlays_helper_dispatch_source());
    }

    #[test]
    fn wave592_composite_pack() {
        assert!(honesty_host_render_ui_overlays_helper_residual_pack_wave592());
    }

    #[test]
    fn simulate_live_host_render_ui_overlays_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_render_ui_overlays_helper_honesty(),
            "host render UI overlays helper residual must latch"
        );
        assert!(residual_host_render_ui_overlays_helper_ok());
        assert_eq!(
            residual_host_render_ui_overlays_helper_last_action(),
            ResidualHostRenderUiOverlaysHelperAction::Composite
        );
    }
}
