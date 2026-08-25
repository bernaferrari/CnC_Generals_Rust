//! Wave 551 residual peels: time-frozen residual is centralized through
//! `presentation_or_boot_time_frozen` — presentation freeze owns the flag when
//! installed; boot residual without freeze uses the host match timing latch.
//! Call sites: host tick, shell visual delta, render time delta, camera shake.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 550 visual speed presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `camera_drain.rs` presentation_or_boot_time_frozen / call sites
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_TIME_FROZEN_PRESENTATION_HELPER_METHOD_NAMES_WAVE551: &[&str] = &[
    "presentation_or_boot_time_frozen",
    "last_presentation_frame",
    "time_frozen_for_simulation",
    "host_match_time_frozen",
    "Wave 551",
    "playable_claim = false",
];

pub const LIVE_TIME_FROZEN_PRESENTATION_HELPER_NAV_STEPS_WAVE551: &[&str] = &[
    "REQUIRE_TIME_FROZEN_PRESENTATION_HELPER",
    "REQUIRE_NO_INLINE_TIME_FROZEN_DUAL_READ",
    "LIVE_TIME_FROZEN_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_TIME_FROZEN_PRESENTATION_HELPER_CMD_NAMES_WAVE551: &[&str] = &[
    "time_frozen_presentation_helper",
    "presentation_time_frozen_owns",
    "boot_host_match_time_frozen",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualTimeFrozenPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualTimeFrozenPresentationHelperAction {
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

fn residual_action_store(action: ResidualTimeFrozenPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_time_frozen_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_time_frozen_presentation_helper_last_action()
-> ResidualTimeFrozenPresentationHelperAction {
    ResidualTimeFrozenPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

pub fn honesty_time_frozen_presentation_helper_method_names_residual_wave551() -> bool {
    let names = LIVE_TIME_FROZEN_PRESENTATION_HELPER_METHOD_NAMES_WAVE551;
    let ok = residual_name_index(names, "presentation_or_boot_time_frozen").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "time_frozen_for_simulation").is_some()
        && residual_name_index(names, "host_match_time_frozen").is_some()
        && residual_name_index(names, "Wave 551").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualTimeFrozenPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_time_frozen_presentation_helper_source_markers_residual_wave551() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn presentation_or_boot_time_frozen(") else {
        residual_action_store(ResidualTimeFrozenPresentationHelperAction::SourceMarkers);
        return false;
    };
    let helper_ok = body.contains("Wave 551")
        && body.contains("pres.time_frozen_for_simulation")
        && body.contains("self.host_match_time_frozen");
    let calls = eng.matches("presentation_or_boot_time_frozen()").count();
    let raw = eng
        .matches("self.game_logic.is_time_frozen_for_simulation()")
        .count();
    let ok = helper_ok && calls >= 4 && raw == 0 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualTimeFrozenPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_time_frozen_presentation_helper_nav_commands_residual_wave551() -> bool {
    let steps = LIVE_TIME_FROZEN_PRESENTATION_HELPER_NAV_STEPS_WAVE551;
    let cmds = RUNTIME_HOST_LIVE_TIME_FROZEN_PRESENTATION_HELPER_CMD_NAMES_WAVE551;
    let ok = residual_name_index(steps, "REQUIRE_TIME_FROZEN_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_NO_INLINE_TIME_FROZEN_DUAL_READ").is_some()
        && residual_name_index(steps, "LIVE_TIME_FROZEN_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "time_frozen_presentation_helper").is_some()
        && residual_name_index(cmds, "presentation_time_frozen_owns").is_some()
        && residual_name_index(cmds, "boot_host_match_time_frozen").is_some();
    residual_action_store(ResidualTimeFrozenPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_time_frozen_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 551")
        && eng.contains("fn presentation_or_boot_time_frozen")
        && eng.contains("presentation_or_boot_time_frozen()");
    residual_action_store(ResidualTimeFrozenPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_time_frozen_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn presentation_or_boot_time_frozen(") else {
        residual_action_store(ResidualTimeFrozenPresentationHelperAction::DispatchSource);
        return false;
    };
    let ok = body.contains("Wave 551")
        && body.contains("pres.time_frozen_for_simulation")
        && body.contains("self.host_match_time_frozen")
        && eng.matches("presentation_or_boot_time_frozen()").count() >= 4;
    residual_action_store(ResidualTimeFrozenPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_time_frozen_presentation_helper_residual_pack_wave551() -> bool {
    honesty_time_frozen_presentation_helper_method_names_residual_wave551()
        && honesty_time_frozen_presentation_helper_source_markers_residual_wave551()
        && honesty_time_frozen_presentation_helper_nav_commands_residual_wave551()
        && simulate_time_frozen_presentation_helper_collect_source()
        && simulate_time_frozen_presentation_helper_dispatch_source()
}

pub fn simulate_live_time_frozen_presentation_helper_honesty() -> bool {
    let ok = honesty_time_frozen_presentation_helper_residual_pack_wave551();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualTimeFrozenPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_time_frozen_presentation_helper_method_names_residual_wave551());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_time_frozen_presentation_helper_source_markers_residual_wave551());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_time_frozen_presentation_helper_nav_commands_residual_wave551());
    }

    #[test]
    fn time_frozen_presentation_helper_sources() {
        assert!(simulate_time_frozen_presentation_helper_collect_source());
        assert!(simulate_time_frozen_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave551_composite_pack() {
        assert!(honesty_time_frozen_presentation_helper_residual_pack_wave551());
    }

    #[test]
    fn simulate_live_time_frozen_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_time_frozen_presentation_helper_honesty(),
            "time frozen presentation helper residual must latch"
        );
        assert!(residual_time_frozen_presentation_helper_ok());
        assert_eq!(
            residual_time_frozen_presentation_helper_last_action(),
            ResidualTimeFrozenPresentationHelperAction::Composite
        );
    }
}
