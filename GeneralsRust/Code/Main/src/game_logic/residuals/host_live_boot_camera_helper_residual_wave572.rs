//! Wave 572 residual peels: boot camera residual is centralized through
//! `apply_boot_camera_residual`. Presentation freeze continues to use
//! `apply_presentation_camera_residual` + `drain_live_camera_request_queues`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 571 popup/music helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` apply_boot_camera_residual /
//!   apply_presentation_camera_residual / apply_pending_script_camera_requests
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_BOOT_CAMERA_HELPER_METHOD_NAMES_WAVE572: &[&str] = &[
    "apply_boot_camera_residual",
    "apply_presentation_camera_residual",
    "drain_live_camera_request_queues",
    "host_drain_live_camera_request_queues",
    "apply_pending_script_camera_requests",
    "Wave 572",
    "playable_claim = false",
];

pub const LIVE_BOOT_CAMERA_HELPER_NAV_STEPS_WAVE572: &[&str] = &[
    "REQUIRE_BOOT_CAMERA_HELPER",
    "REQUIRE_PRES_CAMERA_HELPER",
    "LIVE_BOOT_CAMERA_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_BOOT_CAMERA_HELPER_CMD_NAMES_WAVE572: &[&str] = &[
    "boot_camera_helper",
    "pres_camera_helper",
    "camera_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualBootCameraHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualBootCameraHelperAction {
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

fn residual_action_store(action: ResidualBootCameraHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_boot_camera_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_boot_camera_helper_last_action() -> ResidualBootCameraHelperAction {
    ResidualBootCameraHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_boot_camera_helper_method_names_residual_wave572() -> bool {
    let names = LIVE_BOOT_CAMERA_HELPER_METHOD_NAMES_WAVE572;
    let ok = residual_name_index(names, "apply_boot_camera_residual").is_some()
        && residual_name_index(names, "apply_presentation_camera_residual").is_some()
        && residual_name_index(names, "drain_live_camera_request_queues").is_some()
        && residual_name_index(names, "apply_pending_script_camera_requests").is_some()
        && residual_name_index(names, "Wave 572").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualBootCameraHelperAction::MethodNames);
    ok
}

pub fn honesty_boot_camera_helper_source_markers_residual_wave572() -> bool {
    let eng = eng_source();
    let Some(pending) = fn_body(eng, "fn apply_pending_script_camera_requests(") else {
        residual_action_store(ResidualBootCameraHelperAction::SourceMarkers);
        return false;
    };
    let Some(boot) = fn_body(eng, "fn apply_boot_camera_residual(") else {
        residual_action_store(ResidualBootCameraHelperAction::SourceMarkers);
        return false;
    };
    let Some(pres) = fn_body(eng, "fn apply_presentation_camera_residual(") else {
        residual_action_store(ResidualBootCameraHelperAction::SourceMarkers);
        return false;
    };
    let pending_ok = pending.contains("Wave 572")
        && pending.contains("apply_presentation_camera_residual")
        && pending.contains("drain_live_camera_request_queues")
        && pending.contains("apply_boot_camera_residual()")
        // no direct live take inside dispatcher after peel
        && !pending.contains("take_camera_focus_request");
    // 2026-08-15: Wave 899 peeled live take_* dual-reads out of boot camera.
    // Boot now applies `host_match_camera_follow_position` only.
    let boot_ok = boot.contains("Wave 572")
        && boot.contains("host_match_camera_follow_position")
        && boot.contains("center_camera_on")
        && !boot.contains("take_camera_focus_request()")
        && !boot.contains("take_camera_zoom_request()");
    let pres_ok = pres.contains("camera_follow_position")
        && !pres.contains("camera_follow_target_position")
        && !pres.contains("take_camera_focus_request");
    let ok = pending_ok
        && boot_ok
        && pres_ok
        && eng.contains("self.apply_boot_camera_residual()")
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualBootCameraHelperAction::SourceMarkers);
    ok
}

pub fn honesty_boot_camera_helper_nav_commands_residual_wave572() -> bool {
    let steps = LIVE_BOOT_CAMERA_HELPER_NAV_STEPS_WAVE572;
    let cmds = RUNTIME_HOST_LIVE_BOOT_CAMERA_HELPER_CMD_NAMES_WAVE572;
    let ok = residual_name_index(steps, "REQUIRE_BOOT_CAMERA_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_PRES_CAMERA_HELPER").is_some()
        && residual_name_index(steps, "LIVE_BOOT_CAMERA_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "boot_camera_helper").is_some()
        && residual_name_index(cmds, "pres_camera_helper").is_some()
        && residual_name_index(cmds, "camera_residual").is_some();
    residual_action_store(ResidualBootCameraHelperAction::NavCommands);
    ok
}

pub fn simulate_boot_camera_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 572")
        && eng.contains("fn apply_boot_camera_residual")
        && eng.contains("fn apply_presentation_camera_residual");
    residual_action_store(ResidualBootCameraHelperAction::CollectSource);
    ok
}

pub fn simulate_boot_camera_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(pending) = fn_body(eng, "fn apply_pending_script_camera_requests(") else {
        residual_action_store(ResidualBootCameraHelperAction::DispatchSource);
        return false;
    };
    let ok = pending.contains("self.apply_boot_camera_residual()")
        && pending.contains("self.apply_presentation_camera_residual(&pres)")
        && pending.contains("self.drain_live_camera_request_queues()");
    residual_action_store(ResidualBootCameraHelperAction::DispatchSource);
    ok
}

pub fn honesty_boot_camera_helper_residual_pack_wave572() -> bool {
    honesty_boot_camera_helper_method_names_residual_wave572()
        && honesty_boot_camera_helper_source_markers_residual_wave572()
        && honesty_boot_camera_helper_nav_commands_residual_wave572()
        && simulate_boot_camera_helper_collect_source()
        && simulate_boot_camera_helper_dispatch_source()
}

pub fn simulate_live_boot_camera_helper_honesty() -> bool {
    let ok = honesty_boot_camera_helper_residual_pack_wave572();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualBootCameraHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_boot_camera_helper_method_names_residual_wave572());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_boot_camera_helper_source_markers_residual_wave572());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_boot_camera_helper_nav_commands_residual_wave572());
    }

    #[test]
    fn boot_camera_helper_sources() {
        assert!(simulate_boot_camera_helper_collect_source());
        assert!(simulate_boot_camera_helper_dispatch_source());
    }

    #[test]
    fn wave572_composite_pack() {
        assert!(honesty_boot_camera_helper_residual_pack_wave572());
    }

    #[test]
    fn simulate_live_boot_camera_helper_honesty_residual_live() {
        assert!(
            simulate_live_boot_camera_helper_honesty(),
            "boot camera helper residual must latch"
        );
        assert!(residual_boot_camera_helper_ok());
        assert_eq!(
            residual_boot_camera_helper_last_action(),
            ResidualBootCameraHelperAction::Composite
        );
    }
}
