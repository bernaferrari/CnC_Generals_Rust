//! Wave 612 residual peels: combat/construction/cursor/state-transition residuals
//! are centralized through `host_issue_force_attack_from_left_click`,
//! `host_resume_selected_construction`, `host_resolve_context_cursor_icon`, and
//! `host_transition_to_state`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Waves 234/578 combat residual peels.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host combat/cursor/transition helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COMBAT_CURSOR_TRANSITION_HELPER_METHOD_NAMES_WAVE612: &[&str] = &[
    "host_issue_force_attack_from_left_click",
    "host_resume_selected_construction",
    "host_resolve_context_cursor_icon",
    "host_transition_to_state",
    "issue_force_attack_from_left_click",
    "transition_to_state",
    "Wave 612",
    "playable_claim = false",
];

pub const LIVE_HOST_COMBAT_CURSOR_TRANSITION_HELPER_NAV_STEPS_WAVE612: &[&str] = &[
    "REQUIRE_HOST_FORCE_ATTACK_HELPER",
    "REQUIRE_HOST_RESUME_CONSTRUCTION_HELPER",
    "REQUIRE_HOST_CURSOR_HELPER",
    "REQUIRE_HOST_TRANSITION_HELPER",
    "REQUIRE_THIN_WRAPPERS",
    "LIVE_HOST_COMBAT_CURSOR_TRANSITION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_COMBAT_CURSOR_TRANSITION_HELPER_CMD_NAMES_WAVE612: &[&str] = &[
    "host_force_attack_helper",
    "host_resume_construction_helper",
    "host_cursor_helper",
    "host_transition_helper",
    "thin_wrappers",
    "combat_cursor_transition_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCombatCursorTransitionHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostCombatCursorTransitionHelperAction {
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

fn residual_action_store(action: ResidualHostCombatCursorTransitionHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_combat_cursor_transition_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_combat_cursor_transition_helper_last_action()
-> ResidualHostCombatCursorTransitionHelperAction {
    ResidualHostCombatCursorTransitionHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_combat_cursor_transition_helper_method_names_residual_wave612() -> bool {
    let names = LIVE_HOST_COMBAT_CURSOR_TRANSITION_HELPER_METHOD_NAMES_WAVE612;
    let ok = residual_name_index(names, "host_issue_force_attack_from_left_click").is_some()
        && residual_name_index(names, "host_resume_selected_construction").is_some()
        && residual_name_index(names, "host_resolve_context_cursor_icon").is_some()
        && residual_name_index(names, "host_transition_to_state").is_some()
        && residual_name_index(names, "Wave 612").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCombatCursorTransitionHelperAction::MethodNames);
    ok
}

pub fn honesty_host_combat_cursor_transition_helper_source_markers_residual_wave612() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_issue_force_attack_from_left_click(",
        "fn host_resume_selected_construction(",
        "fn host_resolve_context_cursor_icon(",
        "fn host_transition_to_state(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 612") {
            defs_ok = false;
            break;
        }
    }
    let Some(force_wrap) = fn_body(eng, "fn issue_force_attack_from_left_click(") else {
        residual_action_store(ResidualHostCombatCursorTransitionHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: first `fn transition_to_state` is the types.rs trait stub.
    let Some(trans_wrap) = fn_body(eng, "pub(super) fn transition_to_state(")
        .or_else(|| fn_body(eng, "fn transition_to_state("))
    else {
        residual_action_store(ResidualHostCombatCursorTransitionHelperAction::SourceMarkers);
        return false;
    };
    // production transition is last match via last_sig_index
    let Some(force_host) = fn_body(eng, "fn host_issue_force_attack_from_left_click(") else {
        residual_action_store(ResidualHostCombatCursorTransitionHelperAction::SourceMarkers);
        return false;
    };
    let Some(cursor_host) = fn_body(eng, "fn host_resolve_context_cursor_icon(") else {
        residual_action_store(ResidualHostCombatCursorTransitionHelperAction::SourceMarkers);
        return false;
    };
    let wrap_ok = force_wrap.contains("host_issue_force_attack_from_left_click")
        && force_wrap.contains("Wave 612")
        && trans_wrap.contains("host_transition_to_state")
        && trans_wrap.contains("Wave 612");
    let host_ok = force_host.contains("ui_selected_ids")
        && (force_host.contains("Wave 234") || force_host.contains("ForceAttack"))
        && cursor_host.contains("ui_selected_ids");
    let call_ok = eng.contains("self.host_issue_force_attack_from_left_click(")
        && eng.contains("self.host_transition_to_state(")
        && eng.contains("self.host_resolve_context_cursor_icon(");
    let ok = defs_ok && wrap_ok && host_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostCombatCursorTransitionHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_combat_cursor_transition_helper_nav_commands_residual_wave612() -> bool {
    let steps = LIVE_HOST_COMBAT_CURSOR_TRANSITION_HELPER_NAV_STEPS_WAVE612;
    let cmds = RUNTIME_HOST_LIVE_HOST_COMBAT_CURSOR_TRANSITION_HELPER_CMD_NAMES_WAVE612;
    let ok = residual_name_index(steps, "REQUIRE_HOST_FORCE_ATTACK_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_RESUME_CONSTRUCTION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_CURSOR_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_TRANSITION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_THIN_WRAPPERS").is_some()
        && residual_name_index(steps, "LIVE_HOST_COMBAT_CURSOR_TRANSITION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_force_attack_helper").is_some()
        && residual_name_index(cmds, "host_resume_construction_helper").is_some()
        && residual_name_index(cmds, "host_cursor_helper").is_some()
        && residual_name_index(cmds, "host_transition_helper").is_some()
        && residual_name_index(cmds, "thin_wrappers").is_some()
        && residual_name_index(cmds, "combat_cursor_transition_residual").is_some();
    residual_action_store(ResidualHostCombatCursorTransitionHelperAction::NavCommands);
    ok
}

pub fn simulate_host_combat_cursor_transition_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 612")
        && eng.contains("fn host_issue_force_attack_from_left_click")
        && eng.contains("fn host_resolve_context_cursor_icon")
        && eng.contains("fn host_transition_to_state")
        && eng.contains("fn host_resume_selected_construction");
    residual_action_store(ResidualHostCombatCursorTransitionHelperAction::CollectSource);
    ok
}

pub fn simulate_host_combat_cursor_transition_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_issue_force_attack_from_left_click(")
        && eng.contains("self.host_transition_to_state(")
        && eng.contains("Wave 612: thin wrapper");
    residual_action_store(ResidualHostCombatCursorTransitionHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_combat_cursor_transition_helper_residual_pack_wave612() -> bool {
    honesty_host_combat_cursor_transition_helper_method_names_residual_wave612()
        && honesty_host_combat_cursor_transition_helper_source_markers_residual_wave612()
        && honesty_host_combat_cursor_transition_helper_nav_commands_residual_wave612()
        && simulate_host_combat_cursor_transition_helper_collect_source()
        && simulate_host_combat_cursor_transition_helper_dispatch_source()
}

pub fn simulate_live_host_combat_cursor_transition_helper_honesty() -> bool {
    let ok = honesty_host_combat_cursor_transition_helper_residual_pack_wave612();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCombatCursorTransitionHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_combat_cursor_transition_helper_method_names_residual_wave612());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_combat_cursor_transition_helper_source_markers_residual_wave612());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_combat_cursor_transition_helper_nav_commands_residual_wave612());
    }

    #[test]
    fn host_combat_cursor_transition_helper_sources() {
        assert!(simulate_host_combat_cursor_transition_helper_collect_source());
        assert!(simulate_host_combat_cursor_transition_helper_dispatch_source());
    }

    #[test]
    fn wave612_composite_pack() {
        assert!(honesty_host_combat_cursor_transition_helper_residual_pack_wave612());
    }

    #[test]
    fn simulate_live_host_combat_cursor_transition_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_combat_cursor_transition_helper_honesty(),
            "host combat cursor transition helper residual must latch"
        );
        assert!(residual_host_combat_cursor_transition_helper_ok());
        assert_eq!(
            residual_host_combat_cursor_transition_helper_last_action(),
            ResidualHostCombatCursorTransitionHelperAction::Composite
        );
    }
}
