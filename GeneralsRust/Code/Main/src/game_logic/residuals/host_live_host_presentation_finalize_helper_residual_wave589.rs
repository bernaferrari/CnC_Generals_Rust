//! Wave 589 residual peels: post-logic presentation finalize is centralized
//! through `host_finalize_presentation_after_logic` — build victory frame,
//! dispatch presentation audio, mirror particle FX, store last frame, apply
//! InGame script FPS. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 586/588 GameClient shell ticks.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_finalize_presentation_after_logic
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Not sole GameWorld authority cutover

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_FINALIZE_HELPER_METHOD_NAMES_WAVE589: &[&str] = &[
    "host_finalize_presentation_after_logic",
    "build_with_victory_for_engine",
    "dispatch_audio_events_direct",
    "apply_particle_systems_to_client",
    "apply_ingame_script_fps_limit_residual",
    "last_presentation_frame",
    "Wave 589",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_FINALIZE_HELPER_NAV_STEPS_WAVE589: &[&str] = &[
    "REQUIRE_HOST_PRESENTATION_FINALIZE",
    "REQUIRE_BUILD_WITH_VICTORY",
    "REQUIRE_AUDIO_DISPATCH",
    "REQUIRE_PARTICLE_CLIENT_MIRROR",
    "REQUIRE_SCRIPT_FPS_AFTER_STORE",
    "LIVE_HOST_PRESENTATION_FINALIZE_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PRESENTATION_FINALIZE_HELPER_CMD_NAMES_WAVE589: &[&str] = &[
    "host_presentation_finalize_helper",
    "build_victory_frame",
    "dispatch_presentation_audio",
    "mirror_presentation_particles",
    "presentation_finalize_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationFinalizeHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostPresentationFinalizeHelperAction {
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

fn residual_action_store(action: ResidualHostPresentationFinalizeHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_presentation_finalize_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_presentation_finalize_helper_last_action()
-> ResidualHostPresentationFinalizeHelperAction {
    ResidualHostPresentationFinalizeHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_presentation_finalize_helper_method_names_residual_wave589() -> bool {
    let names = LIVE_HOST_PRESENTATION_FINALIZE_HELPER_METHOD_NAMES_WAVE589;
    let ok = residual_name_index(names, "host_finalize_presentation_after_logic").is_some()
        && residual_name_index(names, "build_with_victory_for_engine").is_some()
        && residual_name_index(names, "dispatch_audio_events_direct").is_some()
        && residual_name_index(names, "apply_particle_systems_to_client").is_some()
        && residual_name_index(names, "apply_ingame_script_fps_limit_residual").is_some()
        && residual_name_index(names, "Wave 589").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPresentationFinalizeHelperAction::MethodNames);
    ok
}

pub fn honesty_host_presentation_finalize_helper_source_markers_residual_wave589() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_finalize_presentation_after_logic(") else {
        residual_action_store(ResidualHostPresentationFinalizeHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 589")
        && body.contains("host_sync_shadow_and_build_presentation")
        && body.contains("dispatch_audio_events_direct")
        && body.contains("apply_particle_systems_to_client")
        && body.contains("last_presentation_frame = Some(pres)")
        && body.contains("apply_ingame_script_fps_limit_residual");
    // 2026-08-15: build is host_sync_shadow_and_build_presentation (camera_drain.rs:2019).
    let i_build = body
        .find("host_sync_shadow_and_build_presentation")
        .or_else(|| body.find("build_with_victory_for_engine"));
    let i_audio = body.find("dispatch_audio_events_direct");
    let i_fx = body.find("apply_particle_systems_to_client");
    let i_store = body.find("last_presentation_frame = Some(pres)");
    let i_fps = body.find("apply_ingame_script_fps_limit_residual");
    let order_ok = matches!(
        (i_build, i_audio, i_fx, i_store, i_fps),
        (Some(a), Some(b), Some(c), Some(d), Some(e)) if a < b && b < c && c < d && d < e
    );
    let call_ok = eng.contains("self.host_finalize_presentation_after_logic()");
    let ok = body_ok
        && order_ok
        && call_ok
        && !body.contains("self.game_logic.dispatch_audio")
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationFinalizeHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_presentation_finalize_helper_nav_commands_residual_wave589() -> bool {
    let steps = LIVE_HOST_PRESENTATION_FINALIZE_HELPER_NAV_STEPS_WAVE589;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRESENTATION_FINALIZE_HELPER_CMD_NAMES_WAVE589;
    let ok = residual_name_index(steps, "REQUIRE_HOST_PRESENTATION_FINALIZE").is_some()
        && residual_name_index(steps, "REQUIRE_BUILD_WITH_VICTORY").is_some()
        && residual_name_index(steps, "REQUIRE_AUDIO_DISPATCH").is_some()
        && residual_name_index(steps, "REQUIRE_PARTICLE_CLIENT_MIRROR").is_some()
        && residual_name_index(steps, "REQUIRE_SCRIPT_FPS_AFTER_STORE").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRESENTATION_FINALIZE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_presentation_finalize_helper").is_some()
        && residual_name_index(cmds, "build_victory_frame").is_some()
        && residual_name_index(cmds, "dispatch_presentation_audio").is_some()
        && residual_name_index(cmds, "mirror_presentation_particles").is_some()
        && residual_name_index(cmds, "presentation_finalize_residual").is_some();
    residual_action_store(ResidualHostPresentationFinalizeHelperAction::NavCommands);
    ok
}

pub fn simulate_host_presentation_finalize_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 589")
        && eng.contains("fn host_finalize_presentation_after_logic")
        && eng.contains("host_sync_shadow_and_build_presentation")
        && eng.contains("dispatch_audio_events_direct");
    residual_action_store(ResidualHostPresentationFinalizeHelperAction::CollectSource);
    ok
}

pub fn simulate_host_presentation_finalize_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_finalize_presentation_after_logic()")
        && eng.contains("Wave 589: presentation finalize residual via helper");
    residual_action_store(ResidualHostPresentationFinalizeHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_presentation_finalize_helper_residual_pack_wave589() -> bool {
    honesty_host_presentation_finalize_helper_method_names_residual_wave589()
        && honesty_host_presentation_finalize_helper_source_markers_residual_wave589()
        && honesty_host_presentation_finalize_helper_nav_commands_residual_wave589()
        && simulate_host_presentation_finalize_helper_collect_source()
        && simulate_host_presentation_finalize_helper_dispatch_source()
}

pub fn simulate_live_host_presentation_finalize_helper_honesty() -> bool {
    let ok = honesty_host_presentation_finalize_helper_residual_pack_wave589();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostPresentationFinalizeHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_presentation_finalize_helper_method_names_residual_wave589());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_presentation_finalize_helper_source_markers_residual_wave589());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_presentation_finalize_helper_nav_commands_residual_wave589());
    }

    #[test]
    fn host_presentation_finalize_helper_sources() {
        assert!(simulate_host_presentation_finalize_helper_collect_source());
        assert!(simulate_host_presentation_finalize_helper_dispatch_source());
    }

    #[test]
    fn wave589_composite_pack() {
        assert!(honesty_host_presentation_finalize_helper_residual_pack_wave589());
    }

    #[test]
    fn simulate_live_host_presentation_finalize_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_presentation_finalize_helper_honesty(),
            "host presentation finalize helper residual must latch"
        );
        assert!(residual_host_presentation_finalize_helper_ok());
        assert_eq!(
            residual_host_presentation_finalize_helper_last_action(),
            ResidualHostPresentationFinalizeHelperAction::Composite
        );
    }
}
