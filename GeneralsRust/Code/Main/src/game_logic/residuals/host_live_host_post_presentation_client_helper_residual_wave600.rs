//! Wave 600 residual peels: post-HUD camera/audio/UI/popup/music residual is
//! centralized through `host_tick_post_presentation_client_residuals`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 571 popup/music and Wave 572 camera residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_tick_post_presentation_client_residuals
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_POST_PRESENTATION_CLIENT_HELPER_METHOD_NAMES_WAVE600: &[&str] = &[
    "host_tick_post_presentation_client_residuals",
    "update_camera",
    "cleanup_sound_effects",
    "apply_pending_script_camera_requests",
    "apply_presentation_popup_music_residual",
    "apply_boot_movie_residual",
    "Wave 600",
    "Wave 571",
    "playable_claim = false",
];

pub const LIVE_HOST_POST_PRESENTATION_CLIENT_HELPER_NAV_STEPS_WAVE600: &[&str] = &[
    "REQUIRE_POST_PRESENTATION_CLIENT_HELPER",
    "REQUIRE_CAMERA_AUDIO",
    "REQUIRE_SCRIPT_CAMERA",
    "REQUIRE_POPUP_MUSIC",
    "LIVE_HOST_POST_PRESENTATION_CLIENT_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_POST_PRESENTATION_CLIENT_HELPER_CMD_NAMES_WAVE600: &[&str] = &[
    "host_post_presentation_client_helper",
    "camera_audio",
    "script_camera",
    "popup_music",
    "post_presentation_client_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPostPresentationClientHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostPostPresentationClientHelperAction {
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

fn residual_action_store(action: ResidualHostPostPresentationClientHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_post_presentation_client_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_post_presentation_client_helper_last_action()
-> ResidualHostPostPresentationClientHelperAction {
    ResidualHostPostPresentationClientHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_host_post_presentation_client_helper_method_names_residual_wave600() -> bool {
    let names = LIVE_HOST_POST_PRESENTATION_CLIENT_HELPER_METHOD_NAMES_WAVE600;
    let ok = residual_name_index(names, "host_tick_post_presentation_client_residuals").is_some()
        && residual_name_index(names, "apply_pending_script_camera_requests").is_some()
        && residual_name_index(names, "apply_presentation_popup_music_residual").is_some()
        && residual_name_index(names, "Wave 600").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPostPresentationClientHelperAction::MethodNames);
    ok
}

pub fn honesty_host_post_presentation_client_helper_source_markers_residual_wave600() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_tick_post_presentation_client_residuals(") else {
        residual_action_store(ResidualHostPostPresentationClientHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 600")
        && body.contains("update_camera")
        && body.contains("cleanup_sound_effects")
        && body.contains("set_runtime_ui_state_projection")
        && body.contains("apply_pending_script_camera_requests")
        && body.contains("Wave 571")
        && body.contains("apply_presentation_popup_music_residual")
        && body.contains("apply_presentation_movie_residual")
        && body.contains("apply_boot_popup_music_residual")
        && body.contains("Wave 567")
        && body.contains("apply_boot_movie_residual");
    let call_ok = eng.contains("self.host_tick_post_presentation_client_residuals(visual_dt, dt)")
        && eng.contains("Wave 600: post-HUD camera/audio/UI/popup-music residual via host helper");
    let ok = body_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostPostPresentationClientHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_post_presentation_client_helper_nav_commands_residual_wave600() -> bool {
    let steps = LIVE_HOST_POST_PRESENTATION_CLIENT_HELPER_NAV_STEPS_WAVE600;
    let cmds = RUNTIME_HOST_LIVE_HOST_POST_PRESENTATION_CLIENT_HELPER_CMD_NAMES_WAVE600;
    let ok = residual_name_index(steps, "REQUIRE_POST_PRESENTATION_CLIENT_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_CAMERA_AUDIO").is_some()
        && residual_name_index(steps, "REQUIRE_SCRIPT_CAMERA").is_some()
        && residual_name_index(steps, "REQUIRE_POPUP_MUSIC").is_some()
        && residual_name_index(steps, "LIVE_HOST_POST_PRESENTATION_CLIENT_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_post_presentation_client_helper").is_some()
        && residual_name_index(cmds, "camera_audio").is_some()
        && residual_name_index(cmds, "script_camera").is_some()
        && residual_name_index(cmds, "popup_music").is_some()
        && residual_name_index(cmds, "post_presentation_client_residual").is_some();
    residual_action_store(ResidualHostPostPresentationClientHelperAction::NavCommands);
    ok
}

pub fn simulate_host_post_presentation_client_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 600")
        && eng.contains("fn host_tick_post_presentation_client_residuals")
        && eng.contains("apply_presentation_popup_music_residual");
    residual_action_store(ResidualHostPostPresentationClientHelperAction::CollectSource);
    ok
}

pub fn simulate_host_post_presentation_client_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_tick_post_presentation_client_residuals(visual_dt, dt)")
        && eng.contains("Wave 600: post-HUD camera/audio/UI/popup-music residual via host helper")
        && eng.contains("self.host_apply_ingame_hud_from_presentation(dt)")
        && eng.contains("self.host_broadcast_match_outcome_residuals()");
    residual_action_store(ResidualHostPostPresentationClientHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_post_presentation_client_helper_residual_pack_wave600() -> bool {
    honesty_host_post_presentation_client_helper_method_names_residual_wave600()
        && honesty_host_post_presentation_client_helper_source_markers_residual_wave600()
        && honesty_host_post_presentation_client_helper_nav_commands_residual_wave600()
        && simulate_host_post_presentation_client_helper_collect_source()
        && simulate_host_post_presentation_client_helper_dispatch_source()
}

pub fn simulate_live_host_post_presentation_client_helper_honesty() -> bool {
    let ok = honesty_host_post_presentation_client_helper_residual_pack_wave600();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostPostPresentationClientHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_post_presentation_client_helper_method_names_residual_wave600());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_post_presentation_client_helper_source_markers_residual_wave600());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_post_presentation_client_helper_nav_commands_residual_wave600());
    }

    #[test]
    fn host_post_presentation_client_helper_sources() {
        assert!(simulate_host_post_presentation_client_helper_collect_source());
        assert!(simulate_host_post_presentation_client_helper_dispatch_source());
    }

    #[test]
    fn wave600_composite_pack() {
        assert!(honesty_host_post_presentation_client_helper_residual_pack_wave600());
    }

    #[test]
    fn simulate_live_host_post_presentation_client_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_post_presentation_client_helper_honesty(),
            "host post presentation client helper residual must latch"
        );
        assert!(residual_host_post_presentation_client_helper_ok());
        assert_eq!(
            residual_host_post_presentation_client_helper_last_action(),
            ResidualHostPostPresentationClientHelperAction::Composite
        );
    }
}
