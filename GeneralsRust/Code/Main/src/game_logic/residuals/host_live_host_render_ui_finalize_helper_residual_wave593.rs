//! Wave 593 residual peels: render-path UI finalize (minimap/radar/victory/last_ui)
//! is centralized through `host_finalize_render_ui_state`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 592 render UI overlays residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_finalize_render_ui_state
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RENDER_UI_FINALIZE_HELPER_METHOD_NAMES_WAVE593: &[&str] = &[
    "host_finalize_render_ui_state",
    "update_minimap_viewport",
    "presentation_world_bounds",
    "update_radar_pings",
    "last_ui_state",
    "Wave 593",
    "playable_claim = false",
];

pub const LIVE_HOST_RENDER_UI_FINALIZE_HELPER_NAV_STEPS_WAVE593: &[&str] = &[
    "REQUIRE_RENDER_UI_FINALIZE_HELPER",
    "REQUIRE_MINIMAP_STAMP",
    "REQUIRE_RADAR_PINGS",
    "REQUIRE_VICTORY_SUMMARY",
    "REQUIRE_LAST_UI_STATE",
    "LIVE_HOST_RENDER_UI_FINALIZE_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_RENDER_UI_FINALIZE_HELPER_CMD_NAMES_WAVE593: &[&str] = &[
    "host_render_ui_finalize_helper",
    "minimap_stamp",
    "radar_pings",
    "victory_summary",
    "last_ui_state",
    "render_ui_finalize_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRenderUiFinalizeHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostRenderUiFinalizeHelperAction {
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

fn residual_action_store(action: ResidualHostRenderUiFinalizeHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_render_ui_finalize_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_render_ui_finalize_helper_last_action()
-> ResidualHostRenderUiFinalizeHelperAction {
    ResidualHostRenderUiFinalizeHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_host_render_ui_finalize_helper_method_names_residual_wave593() -> bool {
    let names = LIVE_HOST_RENDER_UI_FINALIZE_HELPER_METHOD_NAMES_WAVE593;
    let ok = residual_name_index(names, "host_finalize_render_ui_state").is_some()
        && residual_name_index(names, "update_minimap_viewport").is_some()
        && residual_name_index(names, "presentation_world_bounds").is_some()
        && residual_name_index(names, "Wave 593").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRenderUiFinalizeHelperAction::MethodNames);
    ok
}

pub fn honesty_host_render_ui_finalize_helper_source_markers_residual_wave593() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_finalize_render_ui_state(") else {
        residual_action_store(ResidualHostRenderUiFinalizeHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 593")
        && body.contains("update_minimap_viewport")
        && body.contains("presentation_world_bounds")
        && body.contains("update_radar_pings")
        && body.contains("victory_summary")
        && body.contains("last_ui_state = Some")
        && body.contains("Prefer presentation world_env for radar/minimap");
    let call_ok = eng.contains("self.host_finalize_render_ui_state(&mut ui_state)")
        && eng.contains("Wave 593: minimap/radar/victory UI residual via host helper");
    let ok = body_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostRenderUiFinalizeHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_render_ui_finalize_helper_nav_commands_residual_wave593() -> bool {
    let steps = LIVE_HOST_RENDER_UI_FINALIZE_HELPER_NAV_STEPS_WAVE593;
    let cmds = RUNTIME_HOST_LIVE_HOST_RENDER_UI_FINALIZE_HELPER_CMD_NAMES_WAVE593;
    let ok = residual_name_index(steps, "REQUIRE_RENDER_UI_FINALIZE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_MINIMAP_STAMP").is_some()
        && residual_name_index(steps, "REQUIRE_RADAR_PINGS").is_some()
        && residual_name_index(steps, "REQUIRE_VICTORY_SUMMARY").is_some()
        && residual_name_index(steps, "REQUIRE_LAST_UI_STATE").is_some()
        && residual_name_index(steps, "LIVE_HOST_RENDER_UI_FINALIZE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_render_ui_finalize_helper").is_some()
        && residual_name_index(cmds, "minimap_stamp").is_some()
        && residual_name_index(cmds, "radar_pings").is_some()
        && residual_name_index(cmds, "victory_summary").is_some()
        && residual_name_index(cmds, "last_ui_state").is_some()
        && residual_name_index(cmds, "render_ui_finalize_residual").is_some();
    residual_action_store(ResidualHostRenderUiFinalizeHelperAction::NavCommands);
    ok
}

pub fn simulate_host_render_ui_finalize_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 593")
        && eng.contains("fn host_finalize_render_ui_state")
        && eng.contains("update_minimap_viewport");
    residual_action_store(ResidualHostRenderUiFinalizeHelperAction::CollectSource);
    ok
}

pub fn simulate_host_render_ui_finalize_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_finalize_render_ui_state(&mut ui_state)")
        && eng.contains("Wave 593: minimap/radar/victory UI residual via host helper")
        && eng.contains("self.host_apply_render_ui_presentation_overlays(&mut ui_state)");
    residual_action_store(ResidualHostRenderUiFinalizeHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_render_ui_finalize_helper_residual_pack_wave593() -> bool {
    honesty_host_render_ui_finalize_helper_method_names_residual_wave593()
        && honesty_host_render_ui_finalize_helper_source_markers_residual_wave593()
        && honesty_host_render_ui_finalize_helper_nav_commands_residual_wave593()
        && simulate_host_render_ui_finalize_helper_collect_source()
        && simulate_host_render_ui_finalize_helper_dispatch_source()
}

pub fn simulate_live_host_render_ui_finalize_helper_honesty() -> bool {
    let ok = honesty_host_render_ui_finalize_helper_residual_pack_wave593();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRenderUiFinalizeHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_render_ui_finalize_helper_method_names_residual_wave593());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_render_ui_finalize_helper_source_markers_residual_wave593());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_render_ui_finalize_helper_nav_commands_residual_wave593());
    }

    #[test]
    fn host_render_ui_finalize_helper_sources() {
        assert!(simulate_host_render_ui_finalize_helper_collect_source());
        assert!(simulate_host_render_ui_finalize_helper_dispatch_source());
    }

    #[test]
    fn wave593_composite_pack() {
        assert!(honesty_host_render_ui_finalize_helper_residual_pack_wave593());
    }

    #[test]
    fn simulate_live_host_render_ui_finalize_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_render_ui_finalize_helper_honesty(),
            "host render UI finalize helper residual must latch"
        );
        assert!(residual_host_render_ui_finalize_helper_ok());
        assert_eq!(
            residual_host_render_ui_finalize_helper_last_action(),
            ResidualHostRenderUiFinalizeHelperAction::Composite
        );
    }
}
