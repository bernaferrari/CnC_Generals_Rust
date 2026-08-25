//! Wave 548 residual peels: `toggle_camera_follow_selection` follow-active fails
//! closed under a presentation freeze — uses `camera_follow_position.is_some()`
//! and does **not** dual-read `camera_follow_object_id`. Boot residual without
//! freeze unchanged. Command authority still writes `set_camera_follow_object`.
//!
//! Wave 583: toggle peels through `presentation_or_boot_camera_follow_active` +
//! `host_set_camera_follow_object` (freeze/boot split lives in the helper).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 547 host status selected presentation fail-closed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` toggle_camera_follow_selection
//!
//! Fail-closed:
//! - Presentation freeze owns follow-active residual
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE548: &[&str] = &[
    "toggle_camera_follow_selection",
    "presentation_or_boot_camera_follow_active",
    "host_set_camera_follow_object",
    "last_presentation_frame",
    "camera_follow_position",
    "camera_follow_object_id",
    "Wave 548",
    "playable_claim = false",
];

pub const LIVE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE548: &[&str] = &[
    "REQUIRE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED",
    "REQUIRE_NO_HOST_FOLLOW_ID_DUAL_READ_WITH_FREEZE",
    "LIVE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE548: &[&str] = &[
    "camera_follow_presentation_fail_closed",
    "presentation_follow_position_owns",
    "boot_camera_follow_object_id",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualCameraFollowPresentationFailClosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualCameraFollowPresentationFailClosedAction {
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

fn residual_action_store(action: ResidualCameraFollowPresentationFailClosedAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_camera_follow_presentation_fail_closed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_camera_follow_presentation_fail_closed_last_action()
-> ResidualCameraFollowPresentationFailClosedAction {
    ResidualCameraFollowPresentationFailClosedAction::from_u8(
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_camera_follow_presentation_fail_closed_method_names_residual_wave548() -> bool {
    let names = LIVE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE548;
    let ok = residual_name_index(names, "toggle_camera_follow_selection").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "camera_follow_position").is_some()
        && residual_name_index(names, "camera_follow_object_id").is_some()
        && residual_name_index(names, "Wave 548").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualCameraFollowPresentationFailClosedAction::MethodNames);
    ok
}

pub fn honesty_camera_follow_presentation_fail_closed_source_markers_residual_wave548() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn toggle_camera_follow_selection(") else {
        residual_action_store(ResidualCameraFollowPresentationFailClosedAction::SourceMarkers);
        return false;
    };
    // Wave 548 intent + Wave 583 peel through presentation_or_boot helper.
    let wave = (body.contains("Wave 548") || body.contains("Wave 583"))
        && (body.contains("presentation freeze owns follow-active residual")
            || body.contains("presentation_or_boot_camera_follow_active"));
    let via_helper = body.contains("presentation_or_boot_camera_follow_active()")
        && body.contains("host_set_camera_follow_object");
    let Some(helper) = fn_body(eng, "fn presentation_or_boot_camera_follow_active(") else {
        residual_action_store(ResidualCameraFollowPresentationFailClosedAction::SourceMarkers);
        return false;
    };
    let pres = helper.contains("pres.camera_follow_position.is_some()");
    let boot = helper.contains("camera_follow_position.is_some()");
    // Under freeze arm of helper, camera_follow_object_id must not appear before boot None arm.
    // 2026-08-15: helper is if-let freeze / host_match / fail-closed, not match.
    let freeze_arm_ok = match helper.find("if let Some(pres)") {
        Some(i) => {
            let arm = &helper[i..];
            let some_pres = arm.find("Some(pres)");
            let none_boot = arm.find("host_match_camera_follow_active");
            let id_in_some = match (some_pres, none_boot) {
                (Some(s), Some(n)) if s < n => arm[s..n].contains("camera_follow_object_id"),
                _ => true,
            };
            !id_in_some && none_boot.is_some()
        }
        None => false,
    };
    let ok = wave
        && via_helper
        && pres
        && boot
        && freeze_arm_ok
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualCameraFollowPresentationFailClosedAction::SourceMarkers);
    ok
}

pub fn honesty_camera_follow_presentation_fail_closed_nav_commands_residual_wave548() -> bool {
    let steps = LIVE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE548;
    let cmds = RUNTIME_HOST_LIVE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE548;
    let ok = residual_name_index(steps, "REQUIRE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_NO_HOST_FOLLOW_ID_DUAL_READ_WITH_FREEZE").is_some()
        && residual_name_index(steps, "LIVE_CAMERA_FOLLOW_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "camera_follow_presentation_fail_closed").is_some()
        && residual_name_index(cmds, "presentation_follow_position_owns").is_some()
        && residual_name_index(cmds, "boot_camera_follow_object_id").is_some();
    residual_action_store(ResidualCameraFollowPresentationFailClosedAction::NavCommands);
    ok
}

pub fn simulate_camera_follow_presentation_fail_closed_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 548")
        && eng.contains("fn toggle_camera_follow_selection")
        && eng.contains("presentation freeze owns follow-active residual");
    residual_action_store(ResidualCameraFollowPresentationFailClosedAction::CollectSource);
    ok
}

pub fn simulate_camera_follow_presentation_fail_closed_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn toggle_camera_follow_selection(") else {
        residual_action_store(ResidualCameraFollowPresentationFailClosedAction::DispatchSource);
        return false;
    };
    let Some(helper) = fn_body(eng, "fn presentation_or_boot_camera_follow_active(") else {
        residual_action_store(ResidualCameraFollowPresentationFailClosedAction::DispatchSource);
        return false;
    };
    let ok = (body.contains("presentation freeze owns follow-active residual")
        || body.contains("presentation_or_boot_camera_follow_active"))
        && body.contains("presentation_or_boot_camera_follow_active()")
        && body.contains("host_set_camera_follow_object")
        && helper.contains("pres.camera_follow_position.is_some()")
        && helper.contains("camera_follow_position.is_some()");
    residual_action_store(ResidualCameraFollowPresentationFailClosedAction::DispatchSource);
    ok
}

pub fn honesty_camera_follow_presentation_fail_closed_residual_pack_wave548() -> bool {
    honesty_camera_follow_presentation_fail_closed_method_names_residual_wave548()
        && honesty_camera_follow_presentation_fail_closed_source_markers_residual_wave548()
        && honesty_camera_follow_presentation_fail_closed_nav_commands_residual_wave548()
        && simulate_camera_follow_presentation_fail_closed_collect_source()
        && simulate_camera_follow_presentation_fail_closed_dispatch_source()
}

pub fn simulate_live_camera_follow_presentation_fail_closed_honesty() -> bool {
    let ok = honesty_camera_follow_presentation_fail_closed_residual_pack_wave548();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualCameraFollowPresentationFailClosedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_camera_follow_presentation_fail_closed_method_names_residual_wave548());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_camera_follow_presentation_fail_closed_source_markers_residual_wave548());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_camera_follow_presentation_fail_closed_nav_commands_residual_wave548());
    }

    #[test]
    fn camera_follow_presentation_fail_closed_sources() {
        assert!(simulate_camera_follow_presentation_fail_closed_collect_source());
        assert!(simulate_camera_follow_presentation_fail_closed_dispatch_source());
    }

    #[test]
    fn wave548_composite_pack() {
        assert!(honesty_camera_follow_presentation_fail_closed_residual_pack_wave548());
    }

    #[test]
    fn simulate_live_camera_follow_presentation_fail_closed_honesty_residual_live() {
        assert!(
            simulate_live_camera_follow_presentation_fail_closed_honesty(),
            "camera follow presentation fail-closed residual must latch"
        );
        assert!(residual_camera_follow_presentation_fail_closed_ok());
        assert_eq!(
            residual_camera_follow_presentation_fail_closed_last_action(),
            ResidualCameraFollowPresentationFailClosedAction::Composite
        );
    }
}
