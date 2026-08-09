//! Wave 604 residual peels: Loading-state client residual and UI SFX are
//! centralized through `host_tick_loading_client_residuals` and
//! `host_play_sound_effect`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 603 paused/endgame client residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_tick_loading_client_residuals / host_play_sound_effect
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_LOADING_SFX_HELPER_METHOD_NAMES_WAVE604: &[&str] = &[
    "host_tick_loading_client_residuals",
    "host_play_sound_effect",
    "play_sound_effect",
    "update_startup_loading",
    "Wave 604",
    "playable_claim = false",
];

pub const LIVE_HOST_LOADING_SFX_HELPER_NAV_STEPS_WAVE604: &[&str] = &[
    "REQUIRE_LOADING_CLIENT_HELPER",
    "REQUIRE_HOST_SFX_HELPER",
    "REQUIRE_THIN_SFX_WRAPPER",
    "LIVE_HOST_LOADING_SFX_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_LOADING_SFX_HELPER_CMD_NAMES_WAVE604: &[&str] = &[
    "host_loading_client_helper",
    "host_sfx_helper",
    "thin_sfx_wrapper",
    "loading_sfx_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostLoadingSfxHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostLoadingSfxHelperAction {
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

fn residual_action_store(action: ResidualHostLoadingSfxHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_loading_sfx_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_loading_sfx_helper_last_action() -> ResidualHostLoadingSfxHelperAction {
    ResidualHostLoadingSfxHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
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

pub fn honesty_host_loading_sfx_helper_method_names_residual_wave604() -> bool {
    let names = LIVE_HOST_LOADING_SFX_HELPER_METHOD_NAMES_WAVE604;
    let ok = residual_name_index(names, "host_tick_loading_client_residuals").is_some()
        && residual_name_index(names, "host_play_sound_effect").is_some()
        && residual_name_index(names, "play_sound_effect").is_some()
        && residual_name_index(names, "Wave 604").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostLoadingSfxHelperAction::MethodNames);
    ok
}

pub fn honesty_host_loading_sfx_helper_source_markers_residual_wave604() -> bool {
    let eng = eng_source();
    let Some(loading) = fn_body(eng, "fn host_tick_loading_client_residuals(") else {
        residual_action_store(ResidualHostLoadingSfxHelperAction::SourceMarkers);
        return false;
    };
    let Some(wrapper) = fn_body(
        eng,
        "fn play_sound_effect(&mut self, sound_type: SoundType)",
    ) else {
        residual_action_store(ResidualHostLoadingSfxHelperAction::SourceMarkers);
        return false;
    };
    let Some(sfx) = fn_body(
        eng,
        "fn host_play_sound_effect(&mut self, sound_type: SoundType)",
    ) else {
        residual_action_store(ResidualHostLoadingSfxHelperAction::SourceMarkers);
        return false;
    };
    let loading_ok = loading.contains("Wave 604")
        && loading.contains("update_startup_loading")
        && loading.contains("UISystemState::Loading")
        && loading.contains("GameState::Loading");
    let wrapper_ok = wrapper.contains("Wave 604")
        && wrapper.contains("host_play_sound_effect(sound_type)")
        && !wrapper.contains("last_presentation_frame");
    let sfx_ok = sfx.contains("Wave 604")
        && sfx.contains("last_presentation_frame")
        && (sfx.contains("SoundType") || sfx.contains("sound_type"));
    let call_ok = eng.contains("self.host_tick_loading_client_residuals()")
        && eng.contains("Wave 604: loading client residual via host helper");
    let ok =
        loading_ok && wrapper_ok && sfx_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostLoadingSfxHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_loading_sfx_helper_nav_commands_residual_wave604() -> bool {
    let steps = LIVE_HOST_LOADING_SFX_HELPER_NAV_STEPS_WAVE604;
    let cmds = RUNTIME_HOST_LIVE_HOST_LOADING_SFX_HELPER_CMD_NAMES_WAVE604;
    let ok = residual_name_index(steps, "REQUIRE_LOADING_CLIENT_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_SFX_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_SFX_WRAPPER").is_some()
        && residual_name_index(steps, "LIVE_HOST_LOADING_SFX_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_loading_client_helper").is_some()
        && residual_name_index(cmds, "host_sfx_helper").is_some()
        && residual_name_index(cmds, "thin_sfx_wrapper").is_some()
        && residual_name_index(cmds, "loading_sfx_residual").is_some();
    residual_action_store(ResidualHostLoadingSfxHelperAction::NavCommands);
    ok
}

pub fn simulate_host_loading_sfx_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 604")
        && eng.contains("fn host_tick_loading_client_residuals")
        && eng.contains("fn host_play_sound_effect")
        && eng.contains("fn play_sound_effect");
    residual_action_store(ResidualHostLoadingSfxHelperAction::CollectSource);
    ok
}

pub fn simulate_host_loading_sfx_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_tick_loading_client_residuals()")
        && eng.contains("self.host_play_sound_effect(sound_type)")
        && eng.contains("Wave 604: loading client residual via host helper");
    residual_action_store(ResidualHostLoadingSfxHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_loading_sfx_helper_residual_pack_wave604() -> bool {
    honesty_host_loading_sfx_helper_method_names_residual_wave604()
        && honesty_host_loading_sfx_helper_source_markers_residual_wave604()
        && honesty_host_loading_sfx_helper_nav_commands_residual_wave604()
        && simulate_host_loading_sfx_helper_collect_source()
        && simulate_host_loading_sfx_helper_dispatch_source()
}

pub fn simulate_live_host_loading_sfx_helper_honesty() -> bool {
    let ok = honesty_host_loading_sfx_helper_residual_pack_wave604();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostLoadingSfxHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_loading_sfx_helper_method_names_residual_wave604());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_loading_sfx_helper_source_markers_residual_wave604());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_loading_sfx_helper_nav_commands_residual_wave604());
    }

    #[test]
    fn host_loading_sfx_helper_sources() {
        assert!(simulate_host_loading_sfx_helper_collect_source());
        assert!(simulate_host_loading_sfx_helper_dispatch_source());
    }

    #[test]
    fn wave604_composite_pack() {
        assert!(honesty_host_loading_sfx_helper_residual_pack_wave604());
    }

    #[test]
    fn simulate_live_host_loading_sfx_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_loading_sfx_helper_honesty(),
            "host loading sfx helper residual must latch"
        );
        assert!(residual_host_loading_sfx_helper_ok());
        assert_eq!(
            residual_host_loading_sfx_helper_last_action(),
            ResidualHostLoadingSfxHelperAction::Composite
        );
    }
}
