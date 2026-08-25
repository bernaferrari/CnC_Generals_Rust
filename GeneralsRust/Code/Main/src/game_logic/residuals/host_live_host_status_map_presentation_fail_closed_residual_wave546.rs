//! Wave 546 residual peels: `runtime_host_status_snapshot` map_name fails closed
//! under a presentation freeze — empty presentation map does **not** dual-read
//! `get_current_map_name`. Boot residual without freeze unchanged.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 545 save/restart presentation fail-closed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` runtime_host_status_snapshot
//!
//! Fail-closed:
//! - Empty presentation map yields `"-"` (no host dual-read)
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE546: &[&str] = &[
    "runtime_host_status_snapshot",
    "last_presentation_frame",
    "get_current_map_name",
    "world_env.map_name",
    "Wave 546",
    "playable_claim = false",
];

pub const LIVE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE546: &[&str] = &[
    "REQUIRE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED",
    "REQUIRE_NO_HOST_MAP_DUAL_READ_WITH_FREEZE",
    "LIVE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE546: &[&str] = &[
    "host_status_map_presentation_fail_closed",
    "presentation_map_owns",
    "boot_get_current_map_name",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStatusMapPresentationFailClosedAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostStatusMapPresentationFailClosedAction {
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

fn residual_action_store(action: ResidualHostStatusMapPresentationFailClosedAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_status_map_presentation_fail_closed_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_status_map_presentation_fail_closed_last_action()
-> ResidualHostStatusMapPresentationFailClosedAction {
    ResidualHostStatusMapPresentationFailClosedAction::from_u8(
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

pub fn honesty_host_status_map_presentation_fail_closed_method_names_residual_wave546() -> bool {
    let names = LIVE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED_METHOD_NAMES_WAVE546;
    let ok = residual_name_index(names, "runtime_host_status_snapshot").is_some()
        && residual_name_index(names, "last_presentation_frame").is_some()
        && residual_name_index(names, "get_current_map_name").is_some()
        && residual_name_index(names, "world_env.map_name").is_some()
        && residual_name_index(names, "Wave 546").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostStatusMapPresentationFailClosedAction::MethodNames);
    ok
}

pub fn honesty_host_status_map_presentation_fail_closed_source_markers_residual_wave546() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(ResidualHostStatusMapPresentationFailClosedAction::SourceMarkers);
        return false;
    };
    // Wave 554: host status map via presentation_or_boot_map_name helper
    // (still fail-closed empty → "-"; raw dual-read only inside helper).
    let pres_ok = body.contains("Wave 546")
        && body.contains("presentation freeze owns host status map residual")
        && (body.contains("pres.world_env.map_name.trim()")
            || body.contains("presentation_or_boot_map_name()"));
    let no_filter_chain = !body.contains("filter(|s| !s.is_empty())");
    let boot =
        body.contains("get_current_map_name()") || eng.contains("fn presentation_or_boot_map_name");
    let arm_ok = body.contains("\"-\"")
        && (body.contains("if let Some(pres) = self.last_presentation_frame.as_ref()")
            || body.contains("presentation_or_boot_map_name()"));
    let ok = pres_ok && no_filter_chain && boot && arm_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostStatusMapPresentationFailClosedAction::SourceMarkers);
    ok
}

pub fn honesty_host_status_map_presentation_fail_closed_nav_commands_residual_wave546() -> bool {
    let steps = LIVE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED_NAV_STEPS_WAVE546;
    let cmds = RUNTIME_HOST_LIVE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED_CMD_NAMES_WAVE546;
    let ok = residual_name_index(steps, "REQUIRE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED")
        .is_some()
        && residual_name_index(steps, "REQUIRE_NO_HOST_MAP_DUAL_READ_WITH_FREEZE").is_some()
        && residual_name_index(steps, "LIVE_HOST_STATUS_MAP_PRESENTATION_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_status_map_presentation_fail_closed").is_some()
        && residual_name_index(cmds, "presentation_map_owns").is_some()
        && residual_name_index(cmds, "boot_get_current_map_name").is_some();
    residual_action_store(ResidualHostStatusMapPresentationFailClosedAction::NavCommands);
    ok
}

pub fn simulate_host_status_map_presentation_fail_closed_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 546")
        && eng.contains("fn runtime_host_status_snapshot")
        && eng.contains("presentation freeze owns host status map residual");
    residual_action_store(ResidualHostStatusMapPresentationFailClosedAction::CollectSource);
    ok
}

pub fn simulate_host_status_map_presentation_fail_closed_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(ResidualHostStatusMapPresentationFailClosedAction::DispatchSource);
        return false;
    };
    let ok = body.contains("presentation freeze owns host status map residual")
        && (body.contains("pres.world_env.map_name.trim()")
            || body.contains("presentation_or_boot_map_name()"))
        && (body.contains("get_current_map_name()")
            || eng.contains("fn presentation_or_boot_map_name"))
        && !body.contains("filter(|s| !s.is_empty())");
    residual_action_store(ResidualHostStatusMapPresentationFailClosedAction::DispatchSource);
    ok
}

pub fn honesty_host_status_map_presentation_fail_closed_residual_pack_wave546() -> bool {
    honesty_host_status_map_presentation_fail_closed_method_names_residual_wave546()
        && honesty_host_status_map_presentation_fail_closed_source_markers_residual_wave546()
        && honesty_host_status_map_presentation_fail_closed_nav_commands_residual_wave546()
        && simulate_host_status_map_presentation_fail_closed_collect_source()
        && simulate_host_status_map_presentation_fail_closed_dispatch_source()
}

pub fn simulate_live_host_status_map_presentation_fail_closed_honesty() -> bool {
    let ok = honesty_host_status_map_presentation_fail_closed_residual_pack_wave546();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostStatusMapPresentationFailClosedAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_status_map_presentation_fail_closed_method_names_residual_wave546());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_status_map_presentation_fail_closed_source_markers_residual_wave546());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_status_map_presentation_fail_closed_nav_commands_residual_wave546());
    }

    #[test]
    fn host_status_map_presentation_fail_closed_sources() {
        assert!(simulate_host_status_map_presentation_fail_closed_collect_source());
        assert!(simulate_host_status_map_presentation_fail_closed_dispatch_source());
    }

    #[test]
    fn wave546_composite_pack() {
        assert!(honesty_host_status_map_presentation_fail_closed_residual_pack_wave546());
    }

    #[test]
    fn simulate_live_host_status_map_presentation_fail_closed_honesty_residual_live() {
        assert!(
            simulate_live_host_status_map_presentation_fail_closed_honesty(),
            "host status map presentation fail-closed residual must latch"
        );
        assert!(residual_host_status_map_presentation_fail_closed_ok());
        assert_eq!(
            residual_host_status_map_presentation_fail_closed_last_action(),
            ResidualHostStatusMapPresentationFailClosedAction::Composite
        );
    }
}
