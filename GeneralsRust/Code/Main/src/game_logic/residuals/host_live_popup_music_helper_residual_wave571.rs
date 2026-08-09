//! Wave 571 residual peels: popup/music residual is centralized through
//! `apply_presentation_popup_music_residual` (freeze apply + drain) and
//! `apply_boot_popup_music_residual` (boot take). Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 570 script message helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` apply_presentation/boot_popup_music_residual
//! - `presentation_frame.rs` pending_popup_messages / pending_music_stop
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_POPUP_MUSIC_HELPER_METHOD_NAMES_WAVE571: &[&str] = &[
    "apply_presentation_popup_music_residual",
    "apply_boot_popup_music_residual",
    "pending_popup_messages",
    "pending_music_stop",
    "Wave 571",
    "playable_claim = false",
];

pub const LIVE_POPUP_MUSIC_HELPER_NAV_STEPS_WAVE571: &[&str] = &[
    "REQUIRE_POPUP_MUSIC_PRES_HELPER",
    "REQUIRE_POPUP_MUSIC_BOOT_HELPER",
    "LIVE_POPUP_MUSIC_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_POPUP_MUSIC_HELPER_CMD_NAMES_WAVE571: &[&str] = &[
    "popup_music_pres_helper",
    "popup_music_boot_helper",
    "popup_music_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPopupMusicHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPopupMusicHelperAction {
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

fn residual_action_store(action: ResidualPopupMusicHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_popup_music_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_popup_music_helper_last_action() -> ResidualPopupMusicHelperAction {
    ResidualPopupMusicHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn pf_source() -> &'static str {
    include_str!("../../presentation_frame.rs")
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

pub fn honesty_popup_music_helper_method_names_residual_wave571() -> bool {
    let names = LIVE_POPUP_MUSIC_HELPER_METHOD_NAMES_WAVE571;
    let ok = residual_name_index(names, "apply_presentation_popup_music_residual").is_some()
        && residual_name_index(names, "apply_boot_popup_music_residual").is_some()
        && residual_name_index(names, "pending_popup_messages").is_some()
        && residual_name_index(names, "pending_music_stop").is_some()
        && residual_name_index(names, "Wave 571").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPopupMusicHelperAction::MethodNames);
    ok
}

pub fn honesty_popup_music_helper_source_markers_residual_wave571() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("pending_popup_messages") && pf.contains("pending_music_stop");
    let Some(pres) = fn_body(eng, "fn apply_presentation_popup_music_residual(") else {
        residual_action_store(ResidualPopupMusicHelperAction::SourceMarkers);
        return false;
    };
    let Some(boot) = fn_body(eng, "fn apply_boot_popup_music_residual(") else {
        residual_action_store(ResidualPopupMusicHelperAction::SourceMarkers);
        return false;
    };
    let pres_ok = pres.contains("Wave 571")
        && pres.contains("pending_popup_messages")
        && pres.contains("pending_music_stop")
        && pres.contains("take_popup_message_requests()")
        && pres.contains("take_music_stop_request()");
    let boot_ok = boot.contains("Wave 571")
        && boot.contains("take_popup_message_requests()")
        && boot.contains("take_music_stop_request()");
    let call_ok = eng.contains("self.apply_presentation_popup_music_residual(&pres)")
        && eng.contains("self.apply_boot_popup_music_residual()");
    let raw_p = eng
        .matches("self.game_logic.take_popup_message_requests()")
        .count();
    let raw_m = eng
        .matches("self.game_logic.take_music_stop_request()")
        .count();
    let ok = field_ok
        && pres_ok
        && boot_ok
        && call_ok
        && raw_p == 2
        && raw_m == 2
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualPopupMusicHelperAction::SourceMarkers);
    ok
}

pub fn honesty_popup_music_helper_nav_commands_residual_wave571() -> bool {
    let steps = LIVE_POPUP_MUSIC_HELPER_NAV_STEPS_WAVE571;
    let cmds = RUNTIME_HOST_LIVE_POPUP_MUSIC_HELPER_CMD_NAMES_WAVE571;
    let ok = residual_name_index(steps, "REQUIRE_POPUP_MUSIC_PRES_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_POPUP_MUSIC_BOOT_HELPER").is_some()
        && residual_name_index(steps, "LIVE_POPUP_MUSIC_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "popup_music_pres_helper").is_some()
        && residual_name_index(cmds, "popup_music_boot_helper").is_some()
        && residual_name_index(cmds, "popup_music_residual").is_some();
    residual_action_store(ResidualPopupMusicHelperAction::NavCommands);
    ok
}

pub fn simulate_popup_music_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 571")
        && eng.contains("fn apply_presentation_popup_music_residual")
        && eng.contains("fn apply_boot_popup_music_residual");
    residual_action_store(ResidualPopupMusicHelperAction::CollectSource);
    ok
}

pub fn simulate_popup_music_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.apply_presentation_popup_music_residual(&pres)")
        && eng.contains("self.apply_boot_popup_music_residual()")
        && eng.contains("apply_presentation_movie_residual")
        && eng.contains("apply_boot_movie_residual");
    residual_action_store(ResidualPopupMusicHelperAction::DispatchSource);
    ok
}

pub fn honesty_popup_music_helper_residual_pack_wave571() -> bool {
    honesty_popup_music_helper_method_names_residual_wave571()
        && honesty_popup_music_helper_source_markers_residual_wave571()
        && honesty_popup_music_helper_nav_commands_residual_wave571()
        && simulate_popup_music_helper_collect_source()
        && simulate_popup_music_helper_dispatch_source()
}

pub fn simulate_live_popup_music_helper_honesty() -> bool {
    let ok = honesty_popup_music_helper_residual_pack_wave571();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPopupMusicHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_popup_music_helper_method_names_residual_wave571());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_popup_music_helper_source_markers_residual_wave571());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_popup_music_helper_nav_commands_residual_wave571());
    }

    #[test]
    fn popup_music_helper_sources() {
        assert!(simulate_popup_music_helper_collect_source());
        assert!(simulate_popup_music_helper_dispatch_source());
    }

    #[test]
    fn wave571_composite_pack() {
        assert!(honesty_popup_music_helper_residual_pack_wave571());
    }

    #[test]
    fn simulate_live_popup_music_helper_honesty_residual_live() {
        assert!(
            simulate_live_popup_music_helper_honesty(),
            "popup/music helper residual must latch"
        );
        assert!(residual_popup_music_helper_ok());
        assert_eq!(
            residual_popup_music_helper_last_action(),
            ResidualPopupMusicHelperAction::Composite
        );
    }
}
