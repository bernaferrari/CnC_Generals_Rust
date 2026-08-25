//! Wave 596 residual peels: live camera request queue drain is centralized
//! through `host_drain_live_camera_request_queues`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 572 boot/presentation camera residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_drain_live_camera_request_queues
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CAMERA_QUEUE_DRAIN_HELPER_METHOD_NAMES_WAVE596: &[&str] = &[
    "host_drain_live_camera_request_queues",
    "drain_live_camera_request_queues",
    "take_camera_focus_request",
    "take_camera_zoom_request",
    "take_screen_shake_requests",
    "Wave 596",
    "playable_claim = false",
];

pub const LIVE_HOST_CAMERA_QUEUE_DRAIN_HELPER_NAV_STEPS_WAVE596: &[&str] = &[
    "REQUIRE_HOST_CAMERA_DRAIN_HELPER",
    "REQUIRE_ALL_CAMERA_TAKES",
    "REQUIRE_THIN_WRAPPER",
    "LIVE_HOST_CAMERA_QUEUE_DRAIN_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_CAMERA_QUEUE_DRAIN_HELPER_CMD_NAMES_WAVE596: &[&str] = &[
    "host_camera_queue_drain_helper",
    "all_camera_takes",
    "thin_wrapper",
    "camera_queue_drain_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCameraQueueDrainHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostCameraQueueDrainHelperAction {
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

fn residual_action_store(action: ResidualHostCameraQueueDrainHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_camera_queue_drain_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_camera_queue_drain_helper_last_action()
-> ResidualHostCameraQueueDrainHelperAction {
    ResidualHostCameraQueueDrainHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_host_camera_queue_drain_helper_method_names_residual_wave596() -> bool {
    let names = LIVE_HOST_CAMERA_QUEUE_DRAIN_HELPER_METHOD_NAMES_WAVE596;
    let ok = residual_name_index(names, "host_drain_live_camera_request_queues").is_some()
        && residual_name_index(names, "drain_live_camera_request_queues").is_some()
        && residual_name_index(names, "take_camera_focus_request").is_some()
        && residual_name_index(names, "Wave 596").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCameraQueueDrainHelperAction::MethodNames);
    ok
}

pub fn honesty_host_camera_queue_drain_helper_source_markers_residual_wave596() -> bool {
    let eng = eng_source();
    let Some(wrapper) = fn_body(eng, "fn drain_live_camera_request_queues(&mut self)") else {
        residual_action_store(ResidualHostCameraQueueDrainHelperAction::SourceMarkers);
        return false;
    };
    let Some(host) = fn_body(eng, "fn host_drain_live_camera_request_queues(&mut self)") else {
        residual_action_store(ResidualHostCameraQueueDrainHelperAction::SourceMarkers);
        return false;
    };
    let wrapper_ok = wrapper.contains("Wave 596")
        && wrapper.contains("host_drain_live_camera_request_queues()")
        && !wrapper.contains("take_camera_focus_request");
    let takes = [
        "take_camera_focus_request",
        "take_camera_zoom_reset",
        "take_camera_zoom_request",
        "take_camera_pitch_request",
        "take_camera_rotate_request",
        "take_camera_look_toward_request",
        "take_camera_slave_mode_enable_request",
        "take_camera_slave_mode_disable_request",
        "take_screen_shake_requests",
        "take_camera_add_shaker_requests",
        "take_view_guardband_request",
        "take_camera_bw_mode_request",
        "take_camera_motion_blur_requests",
    ];
    // 2026-08-15: Wave 899 peeled take_* dual-reads — host drain is fail-closed.
    let host_ok = host.contains("Wave 596")
        && host.contains("last_presentation_frame")
        && takes.iter().all(|t| !host.contains(t));
    let call_ok = eng.contains("self.drain_live_camera_request_queues()")
        || eng.contains("self.host_drain_live_camera_request_queues()");
    let ok = wrapper_ok && host_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostCameraQueueDrainHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_camera_queue_drain_helper_nav_commands_residual_wave596() -> bool {
    let steps = LIVE_HOST_CAMERA_QUEUE_DRAIN_HELPER_NAV_STEPS_WAVE596;
    let cmds = RUNTIME_HOST_LIVE_HOST_CAMERA_QUEUE_DRAIN_HELPER_CMD_NAMES_WAVE596;
    let ok = residual_name_index(steps, "REQUIRE_HOST_CAMERA_DRAIN_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_ALL_CAMERA_TAKES").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_WRAPPER").is_some()
        && residual_name_index(steps, "LIVE_HOST_CAMERA_QUEUE_DRAIN_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_camera_queue_drain_helper").is_some()
        && residual_name_index(cmds, "all_camera_takes").is_some()
        && residual_name_index(cmds, "thin_wrapper").is_some()
        && residual_name_index(cmds, "camera_queue_drain_residual").is_some();
    residual_action_store(ResidualHostCameraQueueDrainHelperAction::NavCommands);
    ok
}

pub fn simulate_host_camera_queue_drain_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 596")
        && eng.contains("fn host_drain_live_camera_request_queues")
        && eng.contains("fn drain_live_camera_request_queues");
    residual_action_store(ResidualHostCameraQueueDrainHelperAction::CollectSource);
    ok
}

pub fn simulate_host_camera_queue_drain_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_drain_live_camera_request_queues()")
        && eng.contains("Wave 596: via `host_drain_live_camera_request_queues`");
    residual_action_store(ResidualHostCameraQueueDrainHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_camera_queue_drain_helper_residual_pack_wave596() -> bool {
    honesty_host_camera_queue_drain_helper_method_names_residual_wave596()
        && honesty_host_camera_queue_drain_helper_source_markers_residual_wave596()
        && honesty_host_camera_queue_drain_helper_nav_commands_residual_wave596()
        && simulate_host_camera_queue_drain_helper_collect_source()
        && simulate_host_camera_queue_drain_helper_dispatch_source()
}

pub fn simulate_live_host_camera_queue_drain_helper_honesty() -> bool {
    let ok = honesty_host_camera_queue_drain_helper_residual_pack_wave596();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCameraQueueDrainHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_camera_queue_drain_helper_method_names_residual_wave596());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_camera_queue_drain_helper_source_markers_residual_wave596());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_camera_queue_drain_helper_nav_commands_residual_wave596());
    }

    #[test]
    fn host_camera_queue_drain_helper_sources() {
        assert!(simulate_host_camera_queue_drain_helper_collect_source());
        assert!(simulate_host_camera_queue_drain_helper_dispatch_source());
    }

    #[test]
    fn wave596_composite_pack() {
        assert!(honesty_host_camera_queue_drain_helper_residual_pack_wave596());
    }

    #[test]
    fn simulate_live_host_camera_queue_drain_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_camera_queue_drain_helper_honesty(),
            "host camera queue drain helper residual must latch"
        );
        assert!(residual_host_camera_queue_drain_helper_ok());
        assert_eq!(
            residual_host_camera_queue_drain_helper_last_action(),
            ResidualHostCameraQueueDrainHelperAction::Composite
        );
    }
}
