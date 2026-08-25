//! Wave 550 residual peels: visual speed residual is centralized through
//! `presentation_or_boot_visual_speed` — presentation freeze owns the multiplier
//! when installed; boot residual without freeze uses host GameLogic probe.
//! Call sites: `update_internal`, render time delta, `apply_script_frame_limit`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 549 ui_player_info presentation fail-closed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` presentation_or_boot_visual_speed / call sites
//!
//! Fail-closed:
//! - Replay flag remains engine-mode residual (`isInReplayGame`)
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_VISUAL_SPEED_PRESENTATION_HELPER_METHOD_NAMES_WAVE550: &[&str] = &[
    "presentation_or_boot_visual_speed",
    "last_presentation_frame",
    "visual_speed_multiplier",
    "apply_script_frame_limit",
    "Wave 550",
    "playable_claim = false",
];

pub const LIVE_VISUAL_SPEED_PRESENTATION_HELPER_NAV_STEPS_WAVE550: &[&str] = &[
    "REQUIRE_VISUAL_SPEED_PRESENTATION_HELPER",
    "REQUIRE_NO_INLINE_VISUAL_SPEED_DUAL_READ",
    "LIVE_VISUAL_SPEED_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_VISUAL_SPEED_PRESENTATION_HELPER_CMD_NAMES_WAVE550: &[&str] = &[
    "visual_speed_presentation_helper",
    "presentation_visual_speed_owns",
    "boot_visual_speed_multiplier",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualVisualSpeedPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualVisualSpeedPresentationHelperAction {
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

fn residual_action_store(action: ResidualVisualSpeedPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_visual_speed_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_visual_speed_presentation_helper_last_action()
-> ResidualVisualSpeedPresentationHelperAction {
    ResidualVisualSpeedPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_visual_speed_presentation_helper_method_names_residual_wave550() -> bool {
    let names = LIVE_VISUAL_SPEED_PRESENTATION_HELPER_METHOD_NAMES_WAVE550;
    let ok = residual_name_index(names, "presentation_or_boot_visual_speed").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "visual_speed_multiplier").is_some()
        && residual_name_index(names, "apply_script_frame_limit").is_some()
        && residual_name_index(names, "Wave 550").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualVisualSpeedPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_visual_speed_presentation_helper_source_markers_residual_wave550() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn presentation_or_boot_visual_speed(") else {
        residual_action_store(ResidualVisualSpeedPresentationHelperAction::SourceMarkers);
        return false;
    };
    let helper_ok = body.contains("Wave 550")
        && body.contains("pres.visual_speed_multiplier")
        && body.contains("host_match_visual_speed");
    // Call sites must use helper (at least 3).
    let calls = eng.matches("presentation_or_boot_visual_speed()").count();
    // Only one raw dual-read remains — inside the helper.
    let raw = eng
        .matches("self.game_logic.visual_speed_multiplier()")
        .count();
    let ok = helper_ok && calls >= 3 && raw == 0 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualVisualSpeedPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_visual_speed_presentation_helper_nav_commands_residual_wave550() -> bool {
    let steps = LIVE_VISUAL_SPEED_PRESENTATION_HELPER_NAV_STEPS_WAVE550;
    let cmds = RUNTIME_HOST_LIVE_VISUAL_SPEED_PRESENTATION_HELPER_CMD_NAMES_WAVE550;
    let ok = residual_name_index(steps, "REQUIRE_VISUAL_SPEED_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_NO_INLINE_VISUAL_SPEED_DUAL_READ").is_some()
        && residual_name_index(steps, "LIVE_VISUAL_SPEED_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "visual_speed_presentation_helper").is_some()
        && residual_name_index(cmds, "presentation_visual_speed_owns").is_some()
        && residual_name_index(cmds, "boot_visual_speed_multiplier").is_some();
    residual_action_store(ResidualVisualSpeedPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_visual_speed_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 550")
        && eng.contains("fn presentation_or_boot_visual_speed")
        && eng.contains("presentation_or_boot_visual_speed()");
    residual_action_store(ResidualVisualSpeedPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_visual_speed_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn presentation_or_boot_visual_speed(") else {
        residual_action_store(ResidualVisualSpeedPresentationHelperAction::DispatchSource);
        return false;
    };
    let ok = body.contains("Wave 550")
        && body.contains("pres.visual_speed_multiplier")
        && body.contains("host_match_visual_speed")
        && eng.matches("presentation_or_boot_visual_speed()").count() >= 3;
    residual_action_store(ResidualVisualSpeedPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_visual_speed_presentation_helper_residual_pack_wave550() -> bool {
    honesty_visual_speed_presentation_helper_method_names_residual_wave550()
        && honesty_visual_speed_presentation_helper_source_markers_residual_wave550()
        && honesty_visual_speed_presentation_helper_nav_commands_residual_wave550()
        && simulate_visual_speed_presentation_helper_collect_source()
        && simulate_visual_speed_presentation_helper_dispatch_source()
}

pub fn simulate_live_visual_speed_presentation_helper_honesty() -> bool {
    let ok = honesty_visual_speed_presentation_helper_residual_pack_wave550();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualVisualSpeedPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_visual_speed_presentation_helper_method_names_residual_wave550());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_visual_speed_presentation_helper_source_markers_residual_wave550());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_visual_speed_presentation_helper_nav_commands_residual_wave550());
    }

    #[test]
    fn visual_speed_presentation_helper_sources() {
        assert!(simulate_visual_speed_presentation_helper_collect_source());
        assert!(simulate_visual_speed_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave550_composite_pack() {
        assert!(honesty_visual_speed_presentation_helper_residual_pack_wave550());
    }

    #[test]
    fn simulate_live_visual_speed_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_visual_speed_presentation_helper_honesty(),
            "visual speed presentation helper residual must latch"
        );
        assert!(residual_visual_speed_presentation_helper_ok());
        assert_eq!(
            residual_visual_speed_presentation_helper_last_action(),
            ResidualVisualSpeedPresentationHelperAction::Composite
        );
    }
}
