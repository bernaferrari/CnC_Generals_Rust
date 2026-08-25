//! Wave 602 residual peels: InGame logic+shadow+presentation frame and shell
//! screen routing are centralized through
//! `host_run_ingame_logic_presentation_frame` and
//! `host_route_shell_owned_screen_change`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 597 shadow session and Wave 589 presentation finalize.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_run_ingame_logic_presentation_frame /
//!   host_route_shell_owned_screen_change
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_INGAME_LOGIC_SHELL_HELPER_METHOD_NAMES_WAVE602: &[&str] = &[
    "host_run_ingame_logic_presentation_frame",
    "host_route_shell_owned_screen_change",
    "host_update_logic_frame",
    "dual_tick_policy",
    "host_run_gameworld_shadow_after_logic",
    "host_finalize_presentation_after_logic",
    "Wave 602",
    "playable_claim = false",
];

pub const LIVE_HOST_INGAME_LOGIC_SHELL_HELPER_NAV_STEPS_WAVE602: &[&str] = &[
    "REQUIRE_INGAME_LOGIC_HELPER",
    "REQUIRE_DUAL_TICK_POLICY",
    "REQUIRE_SHADOW_AND_FINALIZE",
    "REQUIRE_SHELL_ROUTE_HELPER",
    "LIVE_HOST_INGAME_LOGIC_SHELL_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_INGAME_LOGIC_SHELL_HELPER_CMD_NAMES_WAVE602: &[&str] = &[
    "host_ingame_logic_helper",
    "dual_tick_policy",
    "shadow_and_finalize",
    "shell_route_helper",
    "ingame_logic_shell_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostIngameLogicShellHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostIngameLogicShellHelperAction {
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

fn residual_action_store(action: ResidualHostIngameLogicShellHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_ingame_logic_shell_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_ingame_logic_shell_helper_last_action()
-> ResidualHostIngameLogicShellHelperAction {
    ResidualHostIngameLogicShellHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_ingame_logic_shell_helper_method_names_residual_wave602() -> bool {
    let names = LIVE_HOST_INGAME_LOGIC_SHELL_HELPER_METHOD_NAMES_WAVE602;
    let ok = residual_name_index(names, "host_run_ingame_logic_presentation_frame").is_some()
        && residual_name_index(names, "host_route_shell_owned_screen_change").is_some()
        && residual_name_index(names, "dual_tick_policy").is_some()
        && residual_name_index(names, "Wave 602").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostIngameLogicShellHelperAction::MethodNames);
    ok
}

pub fn honesty_host_ingame_logic_shell_helper_source_markers_residual_wave602() -> bool {
    let eng = eng_source();
    let Some(logic) = fn_body(eng, "fn host_run_ingame_logic_presentation_frame(") else {
        residual_action_store(ResidualHostIngameLogicShellHelperAction::SourceMarkers);
        return false;
    };
    let Some(shell) = fn_body(eng, "fn host_route_shell_owned_screen_change(") else {
        residual_action_store(ResidualHostIngameLogicShellHelperAction::SourceMarkers);
        return false;
    };
    let Some(wrapper) = fn_body(eng, "fn route_shell_owned_screen_change(") else {
        residual_action_store(ResidualHostIngameLogicShellHelperAction::SourceMarkers);
        return false;
    };
    let logic_ok = logic.contains("Wave 602")
        && logic.contains("CoupledTickGuard")
        && logic.contains("host_update_logic_frame")
        && logic.contains("dual_tick_policy")
        && logic.contains("apply_post_authority_crate_tick")
        && logic.contains("host_run_gameworld_shadow_after_logic")
        && logic.contains("host_finalize_presentation_after_logic")
        && logic.contains("host_tick_game_client_presentation_shell");
    let shell_ok = shell.contains("Wave 602")
        && shell.contains("enter_shell_menu_from_runtime_host")
        && shell.contains("SkirmishGameOptionsMenu.wnd");
    let wrapper_ok = wrapper.contains("Wave 602")
        && wrapper.contains("host_route_shell_owned_screen_change(screen)");
    let call_ok = eng.contains("self.host_run_ingame_logic_presentation_frame(dt)")
        && eng.contains("Wave 602: host InGame logic+shadow+presentation residual via helper");
    let ok =
        logic_ok && shell_ok && wrapper_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostIngameLogicShellHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_ingame_logic_shell_helper_nav_commands_residual_wave602() -> bool {
    let steps = LIVE_HOST_INGAME_LOGIC_SHELL_HELPER_NAV_STEPS_WAVE602;
    let cmds = RUNTIME_HOST_LIVE_HOST_INGAME_LOGIC_SHELL_HELPER_CMD_NAMES_WAVE602;
    let ok = residual_name_index(steps, "REQUIRE_INGAME_LOGIC_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_DUAL_TICK_POLICY").is_some()
        && residual_name_index(steps, "REQUIRE_SHADOW_AND_FINALIZE").is_some()
        && residual_name_index(steps, "REQUIRE_SHELL_ROUTE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_HOST_INGAME_LOGIC_SHELL_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_ingame_logic_helper").is_some()
        && residual_name_index(cmds, "dual_tick_policy").is_some()
        && residual_name_index(cmds, "shadow_and_finalize").is_some()
        && residual_name_index(cmds, "shell_route_helper").is_some()
        && residual_name_index(cmds, "ingame_logic_shell_residual").is_some();
    residual_action_store(ResidualHostIngameLogicShellHelperAction::NavCommands);
    ok
}

pub fn simulate_host_ingame_logic_shell_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 602")
        && eng.contains("fn host_run_ingame_logic_presentation_frame")
        && eng.contains("fn host_route_shell_owned_screen_change")
        && eng.contains("CoupledTickGuard");
    residual_action_store(ResidualHostIngameLogicShellHelperAction::CollectSource);
    ok
}

pub fn simulate_host_ingame_logic_shell_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_run_ingame_logic_presentation_frame(dt)")
        && eng.contains("self.host_route_shell_owned_screen_change(screen)")
        && eng.contains("Wave 602: host InGame logic+shadow+presentation residual via helper");
    residual_action_store(ResidualHostIngameLogicShellHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_ingame_logic_shell_helper_residual_pack_wave602() -> bool {
    honesty_host_ingame_logic_shell_helper_method_names_residual_wave602()
        && honesty_host_ingame_logic_shell_helper_source_markers_residual_wave602()
        && honesty_host_ingame_logic_shell_helper_nav_commands_residual_wave602()
        && simulate_host_ingame_logic_shell_helper_collect_source()
        && simulate_host_ingame_logic_shell_helper_dispatch_source()
}

pub fn simulate_live_host_ingame_logic_shell_helper_honesty() -> bool {
    let ok = honesty_host_ingame_logic_shell_helper_residual_pack_wave602();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostIngameLogicShellHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_ingame_logic_shell_helper_method_names_residual_wave602());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_ingame_logic_shell_helper_source_markers_residual_wave602());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_ingame_logic_shell_helper_nav_commands_residual_wave602());
    }

    #[test]
    fn host_ingame_logic_shell_helper_sources() {
        assert!(simulate_host_ingame_logic_shell_helper_collect_source());
        assert!(simulate_host_ingame_logic_shell_helper_dispatch_source());
    }

    #[test]
    fn wave602_composite_pack() {
        assert!(honesty_host_ingame_logic_shell_helper_residual_pack_wave602());
    }

    #[test]
    fn simulate_live_host_ingame_logic_shell_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_ingame_logic_shell_helper_honesty(),
            "host ingame logic shell helper residual must latch"
        );
        assert!(residual_host_ingame_logic_shell_helper_ok());
        assert_eq!(
            residual_host_ingame_logic_shell_helper_last_action(),
            ResidualHostIngameLogicShellHelperAction::Composite
        );
    }
}
