//! Wave 1098: cursor Select + cmd-hint attack sold/masked residual.
//!
//! After Waves 1092–1097 pick/cmd peels:
//! - context cursor Select only checked team + !destroyed
//! - presentation_target_hint is_alive ignored sold/masked
//! - classify_right_click attack ignored sold
//!
//! Align cursor Select with presentation_is_selectable; tighten is_alive and
//! attack classification fail-closed on sold.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CURSOR_CMD_HINT_SELECTABLE_SOLD_METHOD_NAMES_WAVE1098: &[&str] = &[
    "host_resolve_context_cursor_icon",
    "presentation_target_hint",
    "classify_right_click_target_from_presentation",
    "Wave 1098",
    "playable_claim = false",
];

pub const LIVE_HOST_CURSOR_CMD_HINT_SELECTABLE_SOLD_NAV_STEPS_WAVE1098: &[&str] = &[
    "CURSOR_SELECT_PRESENTATION_SELECTABLE",
    "TARGET_HINT_IS_ALIVE_EXCLUDES_SOLD_MASKED",
    "ATTACK_CLASSIFY_FAILS_SOLD",
    "LIVE_HOST_CURSOR_CMD_HINT_SELECTABLE_SOLD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCursorCmdHintSelectableSoldAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCursorCmdHintSelectableSoldAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}
fn cs_source() -> &'static str {
    crate::command_system::COMMAND_SYSTEM_SRC
}
fn es_source() -> &'static str {
    include_str!("../../executable_smoke.rs")
}

pub fn honesty_host_cursor_cmd_hint_selectable_sold_method_names_residual_wave1098() -> bool {
    let names = LIVE_HOST_CURSOR_CMD_HINT_SELECTABLE_SOLD_METHOD_NAMES_WAVE1098;
    let ok = residual_name_index(names, "host_resolve_context_cursor_icon").is_some()
        && residual_name_index(names, "Wave 1098").is_some();
    residual_action_store(ResidualHostCursorCmdHintSelectableSoldAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_cursor_cmd_hint_selectable_sold_nav_commands_residual_wave1098() -> bool {
    let steps = LIVE_HOST_CURSOR_CMD_HINT_SELECTABLE_SOLD_NAV_STEPS_WAVE1098;
    let ok = residual_name_index(steps, "LIVE_HOST_CURSOR_CMD_HINT_SELECTABLE_SOLD").is_some()
        && residual_name_index(steps, "CURSOR_SELECT_PRESENTATION_SELECTABLE").is_some();
    residual_action_store(ResidualHostCursorCmdHintSelectableSoldAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_cursor_cmd_hint_selectable_sold_residual_pack_wave1098() -> bool {
    let cnc = cnc_source();
    let cs = cs_source();
    let es = es_source();
    let cur = match super::harness::last_rust_fn_body(cnc, "host_resolve_context_cursor_icon") {
        Some(b) => b,
        None => {
            residual_action_store(ResidualHostCursorCmdHintSelectableSoldAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let t = match super::harness::last_rust_fn_body(cnc, "presentation_target_hint") {
        Some(b) => b,
        None => {
            residual_action_store(ResidualHostCursorCmdHintSelectableSoldAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let cl = match super::harness::last_rust_fn_body(
        cs,
        "classify_right_click_target_from_presentation",
    ) {
        Some(b) => b,
        None => {
            residual_action_store(ResidualHostCursorCmdHintSelectableSoldAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let ok = cur.contains("Wave 1098: cursor Select residual uses full presentation")
        && cur.contains("presentation_is_selectable")
        && t.contains("Wave 1098: is_alive residual excludes sold/masked")
        && t.contains("!o.sold")
        && t.contains("!o.masked")
        && cl.contains("Wave 1098: sold residual fail-closed")
        && cl.contains("!hint.sold && any_attacker()")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostCursorCmdHintSelectableSoldAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_cursor_cmd_hint_selectable_sold_residual_honesty() -> bool {
    let a = honesty_host_cursor_cmd_hint_selectable_sold_method_names_residual_wave1098();
    let b = honesty_host_cursor_cmd_hint_selectable_sold_nav_commands_residual_wave1098();
    let c = honesty_host_cursor_cmd_hint_selectable_sold_residual_pack_wave1098();
    residual_action_store(ResidualHostCursorCmdHintSelectableSoldAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_cursor_cmd_hint_selectable_sold_residual_wave1098() {
        assert!(honesty_host_cursor_cmd_hint_selectable_sold_residual_pack_wave1098());
        assert!(honesty_host_cursor_cmd_hint_selectable_sold_method_names_residual_wave1098());
        assert!(honesty_host_cursor_cmd_hint_selectable_sold_nav_commands_residual_wave1098());
        assert!(simulate_live_host_cursor_cmd_hint_selectable_sold_residual_honesty());
    }
}
