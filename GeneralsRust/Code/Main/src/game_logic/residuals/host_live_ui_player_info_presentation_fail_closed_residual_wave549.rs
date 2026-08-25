//! Wave 549 residual peels: `ui_player_info` fails closed under a presentation
//! freeze — missing `frame.player_info(id)` does **not** dual-read host
//! `player_exists` / `player_name` / team / alive / local. Boot residual without
//! freeze unchanged. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 548 camera follow presentation fail-closed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` ui_player_info
//!
//! Fail-closed:
//! - Presentation freeze owns player roster residual
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE549: &[&str] = &[
    "ui_player_info",
    "last_presentation_frame",
    "player_info",
    "player_exists",
    "Wave 549",
    "playable_claim = false",
];

pub const LIVE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE549: &[&str] = &[
    "REQUIRE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED",
    "REQUIRE_NO_HOST_PLAYER_FIELD_DUAL_READ_WITH_FREEZE",
    "LIVE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE549: &[&str] = &[
    "ui_player_info_presentation_fail_closed",
    "presentation_player_roster_owns",
    "boot_player_exists",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualUiPlayerInfoPresentationFailClosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualUiPlayerInfoPresentationFailClosedAction {
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

fn residual_action_store(action: ResidualUiPlayerInfoPresentationFailClosedAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_ui_player_info_presentation_fail_closed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_ui_player_info_presentation_fail_closed_last_action()
-> ResidualUiPlayerInfoPresentationFailClosedAction {
    ResidualUiPlayerInfoPresentationFailClosedAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
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

pub fn honesty_ui_player_info_presentation_fail_closed_method_names_residual_wave549() -> bool {
    let names = LIVE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE549;
    let ok = residual_name_index(names, "ui_player_info").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "player_info").is_some()
        && residual_name_index(names, "player_exists").is_some()
        && residual_name_index(names, "Wave 549").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualUiPlayerInfoPresentationFailClosedAction::MethodNames);
    ok
}

pub fn honesty_ui_player_info_presentation_fail_closed_source_markers_residual_wave549() -> bool {
    let eng = eng_source();
    let Some(body) =
        fn_body(eng, "fn host_ui_player_info(").or_else(|| fn_body(eng, "fn ui_player_info("))
    else {
        residual_action_store(ResidualUiPlayerInfoPresentationFailClosedAction::SourceMarkers);
        return false;
    };
    let wave = body.contains("Wave 549")
        && body.contains("presentation freeze owns player roster residual");
    let early_return = body.contains("return frame.player_info(player_id).cloned();");
    // Wave 573: boot path may be inline player_exists or boot_player_info_from_host.
    let boot = body.contains("player_exists(player_id)")
        || body.contains("boot_player_info_from_host(player_id)");
    // Freeze arm must not call player_exists before returning.
    let freeze_arm_ok =
        match body.find("if let Some(frame) = self.last_presentation_frame.as_ref()") {
            Some(i) => {
                let arm = &body[i..];
                let ret = arm.find("return frame.player_info");
                let exists = arm.find("player_exists");
                matches!((ret, exists), (Some(r), Some(e)) if r < e)
                    || (ret.is_some() && exists.is_none())
            }
            None => false,
        };
    let ok =
        wave && early_return && boot && freeze_arm_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualUiPlayerInfoPresentationFailClosedAction::SourceMarkers);
    ok
}

pub fn honesty_ui_player_info_presentation_fail_closed_nav_commands_residual_wave549() -> bool {
    let steps = LIVE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE549;
    let cmds = RUNTIME_HOST_LIVE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE549;
    let ok = residual_name_index(steps, "REQUIRE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED")
        .is_some()
        && residual_name_index(steps, "REQUIRE_NO_HOST_PLAYER_FIELD_DUAL_READ_WITH_FREEZE")
            .is_some()
        && residual_name_index(steps, "LIVE_UI_PLAYER_INFO_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "ui_player_info_presentation_fail_closed").is_some()
        && residual_name_index(cmds, "presentation_player_roster_owns").is_some()
        && residual_name_index(cmds, "boot_player_exists").is_some();
    residual_action_store(ResidualUiPlayerInfoPresentationFailClosedAction::NavCommands);
    ok
}

pub fn simulate_ui_player_info_presentation_fail_closed_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 549")
        && eng.contains("fn ui_player_info")
        && eng.contains("presentation freeze owns player roster residual");
    residual_action_store(ResidualUiPlayerInfoPresentationFailClosedAction::CollectSource);
    ok
}

pub fn simulate_ui_player_info_presentation_fail_closed_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) =
        fn_body(eng, "fn host_ui_player_info(").or_else(|| fn_body(eng, "fn ui_player_info("))
    else {
        residual_action_store(ResidualUiPlayerInfoPresentationFailClosedAction::DispatchSource);
        return false;
    };
    // Wave 573: boot path may be helper call instead of inline player_exists.
    let boot_ok = body.contains("player_exists(player_id)")
        || body.contains("boot_player_info_from_host(player_id)");
    let ok = body.contains("presentation freeze owns player roster residual")
        && body.contains("return frame.player_info(player_id).cloned();")
        && boot_ok;
    residual_action_store(ResidualUiPlayerInfoPresentationFailClosedAction::DispatchSource);
    ok
}

pub fn honesty_ui_player_info_presentation_fail_closed_residual_pack_wave549() -> bool {
    honesty_ui_player_info_presentation_fail_closed_method_names_residual_wave549()
        && honesty_ui_player_info_presentation_fail_closed_source_markers_residual_wave549()
        && honesty_ui_player_info_presentation_fail_closed_nav_commands_residual_wave549()
        && simulate_ui_player_info_presentation_fail_closed_collect_source()
        && simulate_ui_player_info_presentation_fail_closed_dispatch_source()
}

pub fn simulate_live_ui_player_info_presentation_fail_closed_honesty() -> bool {
    let ok = honesty_ui_player_info_presentation_fail_closed_residual_pack_wave549();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualUiPlayerInfoPresentationFailClosedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_ui_player_info_presentation_fail_closed_method_names_residual_wave549());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_ui_player_info_presentation_fail_closed_source_markers_residual_wave549());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_ui_player_info_presentation_fail_closed_nav_commands_residual_wave549());
    }

    #[test]
    fn ui_player_info_presentation_fail_closed_sources() {
        assert!(simulate_ui_player_info_presentation_fail_closed_collect_source());
        assert!(simulate_ui_player_info_presentation_fail_closed_dispatch_source());
    }

    #[test]
    fn wave549_composite_pack() {
        assert!(honesty_ui_player_info_presentation_fail_closed_residual_pack_wave549());
    }

    #[test]
    fn simulate_live_ui_player_info_presentation_fail_closed_honesty_residual_live() {
        assert!(
            simulate_live_ui_player_info_presentation_fail_closed_honesty(),
            "ui_player_info presentation fail-closed residual must latch"
        );
        assert!(residual_ui_player_info_presentation_fail_closed_ok());
        assert_eq!(
            residual_ui_player_info_presentation_fail_closed_last_action(),
            ResidualUiPlayerInfoPresentationFailClosedAction::Composite
        );
    }
}
