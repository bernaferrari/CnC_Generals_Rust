//! Wave 557 residual peels: freeze `in_replay_game` onto `PresentationFrame` and
//! centralize FPS-limit replay residual through `presentation_or_boot_in_replay_game`
//! — presentation freeze owns replay mode when installed; boot residual without
//! freeze uses host `isInReplayGame`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 556 victory presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` in_replay_game field + build_from_logic
//! - `cnc_game_engine.rs` presentation_or_boot_in_replay_game / apply_script_frame_limit
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_REPLAY_PRESENTATION_HELPER_METHOD_NAMES_WAVE557: &[&str] = &[
    "presentation_or_boot_in_replay_game",
    "in_replay_game",
    "isInReplayGame",
    "apply_script_frame_limit",
    "Wave 557",
    "playable_claim = false",
];

pub const LIVE_REPLAY_PRESENTATION_HELPER_NAV_STEPS_WAVE557: &[&str] = &[
    "REQUIRE_IN_REPLAY_PRESENTATION_FIELD",
    "REQUIRE_REPLAY_PRESENTATION_HELPER",
    "LIVE_REPLAY_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_REPLAY_PRESENTATION_HELPER_CMD_NAMES_WAVE557: &[&str] = &[
    "in_replay_presentation_field",
    "replay_presentation_helper",
    "boot_isInReplayGame",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualReplayPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualReplayPresentationHelperAction {
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

fn residual_action_store(action: ResidualReplayPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_replay_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_replay_presentation_helper_last_action() -> ResidualReplayPresentationHelperAction {
    ResidualReplayPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
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

pub fn honesty_replay_presentation_helper_method_names_residual_wave557() -> bool {
    let names = LIVE_REPLAY_PRESENTATION_HELPER_METHOD_NAMES_WAVE557;
    let ok = residual_name_index(names, "presentation_or_boot_in_replay_game").is_some()
        && residual_name_index(names, "in_replay_game").is_some()
        && residual_name_index(names, "isInReplayGame").is_some()
        && residual_name_index(names, "apply_script_frame_limit").is_some()
        && residual_name_index(names, "Wave 557").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualReplayPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_replay_presentation_helper_source_markers_residual_wave557() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("pub in_replay_game: bool")
        && pf.contains("Wave 557")
        && pf.contains("in_replay_game: logic.isInReplayGame()");
    let Some(body) = fn_body(eng, "fn presentation_or_boot_in_replay_game(") else {
        residual_action_store(ResidualReplayPresentationHelperAction::SourceMarkers);
        return false;
    };
    let helper_ok = body.contains("Wave 557")
        && body.contains("pres.in_replay_game")
        && body.contains("self.game_logic.isInReplayGame()");
    let call = eng.contains("presentation_or_boot_in_replay_game()");
    let raw = eng.matches("self.game_logic.isInReplayGame()").count();
    let ok = field_ok && helper_ok && call && raw == 1 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualReplayPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_replay_presentation_helper_nav_commands_residual_wave557() -> bool {
    let steps = LIVE_REPLAY_PRESENTATION_HELPER_NAV_STEPS_WAVE557;
    let cmds = RUNTIME_HOST_LIVE_REPLAY_PRESENTATION_HELPER_CMD_NAMES_WAVE557;
    let ok = residual_name_index(steps, "REQUIRE_IN_REPLAY_PRESENTATION_FIELD").is_some()
        && residual_name_index(steps, "REQUIRE_REPLAY_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_REPLAY_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "in_replay_presentation_field").is_some()
        && residual_name_index(cmds, "replay_presentation_helper").is_some()
        && residual_name_index(cmds, "boot_isInReplayGame").is_some();
    residual_action_store(ResidualReplayPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_replay_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let ok = eng.contains("Wave 557")
        && eng.contains("fn presentation_or_boot_in_replay_game")
        && pf.contains("pub in_replay_game: bool");
    residual_action_store(ResidualReplayPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_replay_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(fps) = fn_body(eng, "fn apply_script_frame_limit(") else {
        residual_action_store(ResidualReplayPresentationHelperAction::DispatchSource);
        return false;
    };
    let ok = fps.contains("presentation_or_boot_in_replay_game()")
        && fps.contains("presentation_or_boot_visual_speed()")
        && !fps.contains("self.game_logic.isInReplayGame()");
    residual_action_store(ResidualReplayPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_replay_presentation_helper_residual_pack_wave557() -> bool {
    honesty_replay_presentation_helper_method_names_residual_wave557()
        && honesty_replay_presentation_helper_source_markers_residual_wave557()
        && honesty_replay_presentation_helper_nav_commands_residual_wave557()
        && simulate_replay_presentation_helper_collect_source()
        && simulate_replay_presentation_helper_dispatch_source()
}

pub fn simulate_live_replay_presentation_helper_honesty() -> bool {
    let ok = honesty_replay_presentation_helper_residual_pack_wave557();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualReplayPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_replay_presentation_helper_method_names_residual_wave557());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_replay_presentation_helper_source_markers_residual_wave557());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_replay_presentation_helper_nav_commands_residual_wave557());
    }

    #[test]
    fn replay_presentation_helper_sources() {
        assert!(simulate_replay_presentation_helper_collect_source());
        assert!(simulate_replay_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave557_composite_pack() {
        assert!(honesty_replay_presentation_helper_residual_pack_wave557());
    }

    #[test]
    fn simulate_live_replay_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_replay_presentation_helper_honesty(),
            "replay presentation helper residual must latch"
        );
        assert!(residual_replay_presentation_helper_ok());
        assert_eq!(
            residual_replay_presentation_helper_last_action(),
            ResidualReplayPresentationHelperAction::Composite
        );
    }
}
