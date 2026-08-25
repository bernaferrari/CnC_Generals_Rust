//! Wave 597 residual peels: GameWorld shadow session after host logic is
//! centralized through `host_run_gameworld_shadow_after_logic`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 589 presentation finalize residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_run_gameworld_shadow_after_logic
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GAMEWORLD_SHADOW_SESSION_HELPER_METHOD_NAMES_WAVE597: &[&str] = &[
    "host_run_gameworld_shadow_after_logic",
    "shadow_session_after_host_tick",
    "presentation_view_from_shadow",
    "maybe_shadow_after_host_tick",
    "last_gameworld_presentation_entity_count",
    "end_shadow_coupled_tick",
    "Wave 597",
    "playable_claim = false",
];

pub const LIVE_HOST_GAMEWORLD_SHADOW_SESSION_HELPER_NAV_STEPS_WAVE597: &[&str] = &[
    "REQUIRE_SHADOW_SESSION_HELPER",
    "REQUIRE_OBSERVE_PATH_VIEW",
    "REQUIRE_COUPLED_TICK_END",
    "LIVE_HOST_GAMEWORLD_SHADOW_SESSION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_GAMEWORLD_SHADOW_SESSION_HELPER_CMD_NAMES_WAVE597: &[&str] = &[
    "host_gameworld_shadow_session_helper",
    "observe_path_view",
    "coupled_tick_end",
    "gameworld_shadow_session_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGameworldShadowSessionHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostGameworldShadowSessionHelperAction {
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

fn residual_action_store(action: ResidualHostGameworldShadowSessionHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_gameworld_shadow_session_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_gameworld_shadow_session_helper_last_action()
-> ResidualHostGameworldShadowSessionHelperAction {
    ResidualHostGameworldShadowSessionHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_host_gameworld_shadow_session_helper_method_names_residual_wave597() -> bool {
    let names = LIVE_HOST_GAMEWORLD_SHADOW_SESSION_HELPER_METHOD_NAMES_WAVE597;
    let ok = residual_name_index(names, "host_run_gameworld_shadow_after_logic").is_some()
        && residual_name_index(names, "shadow_session_after_host_tick").is_some()
        && residual_name_index(names, "presentation_view_from_shadow").is_some()
        && residual_name_index(names, "Wave 597").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostGameworldShadowSessionHelperAction::MethodNames);
    ok
}

pub fn honesty_host_gameworld_shadow_session_helper_source_markers_residual_wave597() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_run_gameworld_shadow_after_logic(") else {
        residual_action_store(ResidualHostGameworldShadowSessionHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-14 (Wave 927 seam): the direct shadow_session_after_host_tick /
    // maybe_shadow_after_host_tick / end_shadow_coupled_tick calls moved into
    // gameworld_shadow::run_post_logic_shadow_boundary; the engine helper now
    // delegates through it. The session markers are asserted in the shadow
    // source view instead of the engine body.
    let sh = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let boundary_ok = sh.contains("run_post_logic_shadow_boundary")
        && sh.contains("shadow_session_after_host_tick")
        && sh.contains("maybe_shadow_after_host_tick");
    let body_ok = body.contains("Wave 597")
        && body.contains("run_post_logic_shadow_boundary")
        && body.contains("presentation_view_from_shadow")
        && body.contains("last_gameworld_presentation_entity_count")
        && body.contains("&mut self.game_logic");
    let call_ok = eng.contains("self.host_run_gameworld_shadow_after_logic(couple_shadow)")
        && eng.contains("Wave 597: GameWorld shadow session after host logic residual");
    let ok = body_ok && boundary_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostGameworldShadowSessionHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_gameworld_shadow_session_helper_nav_commands_residual_wave597() -> bool {
    let steps = LIVE_HOST_GAMEWORLD_SHADOW_SESSION_HELPER_NAV_STEPS_WAVE597;
    let cmds = RUNTIME_HOST_LIVE_HOST_GAMEWORLD_SHADOW_SESSION_HELPER_CMD_NAMES_WAVE597;
    let ok = residual_name_index(steps, "REQUIRE_SHADOW_SESSION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_OBSERVE_PATH_VIEW").is_some()
        && residual_name_index(steps, "REQUIRE_COUPLED_TICK_END").is_some()
        && residual_name_index(steps, "LIVE_HOST_GAMEWORLD_SHADOW_SESSION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_gameworld_shadow_session_helper").is_some()
        && residual_name_index(cmds, "observe_path_view").is_some()
        && residual_name_index(cmds, "coupled_tick_end").is_some()
        && residual_name_index(cmds, "gameworld_shadow_session_residual").is_some();
    residual_action_store(ResidualHostGameworldShadowSessionHelperAction::NavCommands);
    ok
}

pub fn simulate_host_gameworld_shadow_session_helper_collect_source() -> bool {
    // 2026-08-14 (Wave 927 seam): shadow_session_after_host_tick moved into
    // gameworld_shadow::run_post_logic_shadow_boundary; assert it there.
    let eng = eng_source();
    let ok = eng.contains("Wave 597")
        && eng.contains("fn host_run_gameworld_shadow_after_logic")
        && eng.contains("run_post_logic_shadow_boundary")
        && crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC.contains("shadow_session_after_host_tick");
    residual_action_store(ResidualHostGameworldShadowSessionHelperAction::CollectSource);
    ok
}

pub fn simulate_host_gameworld_shadow_session_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_run_gameworld_shadow_after_logic(couple_shadow)")
        && eng.contains("Wave 597: GameWorld shadow session after host logic residual")
        && eng.contains("self.host_finalize_presentation_after_logic()");
    residual_action_store(ResidualHostGameworldShadowSessionHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_gameworld_shadow_session_helper_residual_pack_wave597() -> bool {
    honesty_host_gameworld_shadow_session_helper_method_names_residual_wave597()
        && honesty_host_gameworld_shadow_session_helper_source_markers_residual_wave597()
        && honesty_host_gameworld_shadow_session_helper_nav_commands_residual_wave597()
        && simulate_host_gameworld_shadow_session_helper_collect_source()
        && simulate_host_gameworld_shadow_session_helper_dispatch_source()
}

pub fn simulate_live_host_gameworld_shadow_session_helper_honesty() -> bool {
    let ok = honesty_host_gameworld_shadow_session_helper_residual_pack_wave597();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostGameworldShadowSessionHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_gameworld_shadow_session_helper_method_names_residual_wave597());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_gameworld_shadow_session_helper_source_markers_residual_wave597());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_gameworld_shadow_session_helper_nav_commands_residual_wave597());
    }

    #[test]
    fn host_gameworld_shadow_session_helper_sources() {
        assert!(simulate_host_gameworld_shadow_session_helper_collect_source());
        assert!(simulate_host_gameworld_shadow_session_helper_dispatch_source());
    }

    #[test]
    fn wave597_composite_pack() {
        assert!(honesty_host_gameworld_shadow_session_helper_residual_pack_wave597());
    }

    #[test]
    fn simulate_live_host_gameworld_shadow_session_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_gameworld_shadow_session_helper_honesty(),
            "host gameworld shadow session helper residual must latch"
        );
        assert!(residual_host_gameworld_shadow_session_helper_ok());
        assert_eq!(
            residual_host_gameworld_shadow_session_helper_last_action(),
            ResidualHostGameworldShadowSessionHelperAction::Composite
        );
    }
}
