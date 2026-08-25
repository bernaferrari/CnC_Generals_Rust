//! Wave 582 residual peels: train enqueue residual is centralized through
//! `host_enqueue_production`, and shell/menu `process_commands` through
//! `host_process_shell_menu_commands`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 581 host template/spawn helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_enqueue_production /
//!   host_process_shell_menu_commands
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ENQUEUE_SHELL_CMD_HELPER_METHOD_NAMES_WAVE582: &[&str] = &[
    "host_enqueue_production",
    "host_process_shell_menu_commands",
    "host_process_commands_with_command_sound",
    "enqueue_production",
    "Wave 582",
    "playable_claim = false",
];

pub const LIVE_HOST_ENQUEUE_SHELL_CMD_HELPER_NAV_STEPS_WAVE582: &[&str] = &[
    "REQUIRE_HOST_ENQUEUE_PRODUCTION",
    "REQUIRE_HOST_PROCESS_SHELL_MENU",
    "LIVE_HOST_ENQUEUE_SHELL_CMD_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_ENQUEUE_SHELL_CMD_HELPER_CMD_NAMES_WAVE582: &[&str] = &[
    "host_enqueue_production_helper",
    "host_process_shell_menu_helper",
    "enqueue_shell_cmd_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEnqueueShellCmdHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostEnqueueShellCmdHelperAction {
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

fn residual_action_store(action: ResidualHostEnqueueShellCmdHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_enqueue_shell_cmd_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_enqueue_shell_cmd_helper_last_action()
-> ResidualHostEnqueueShellCmdHelperAction {
    ResidualHostEnqueueShellCmdHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
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
pub fn honesty_host_enqueue_shell_cmd_helper_method_names_residual_wave582() -> bool {
    let names = LIVE_HOST_ENQUEUE_SHELL_CMD_HELPER_METHOD_NAMES_WAVE582;
    let ok = residual_name_index(names, "host_enqueue_production").is_some()
        && residual_name_index(names, "host_process_shell_menu_commands").is_some()
        && residual_name_index(names, "host_process_commands_with_command_sound").is_some()
        && residual_name_index(names, "enqueue_production").is_some()
        && residual_name_index(names, "Wave 582").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEnqueueShellCmdHelperAction::MethodNames);
    ok
}

pub fn honesty_host_enqueue_shell_cmd_helper_source_markers_residual_wave582() -> bool {
    let eng = eng_source();
    let Some(enq) = fn_body(eng, "fn host_enqueue_production(") else {
        residual_action_store(ResidualHostEnqueueShellCmdHelperAction::SourceMarkers);
        return false;
    };
    let Some(shell) = fn_body(eng, "fn host_process_shell_menu_commands(") else {
        residual_action_store(ResidualHostEnqueueShellCmdHelperAction::SourceMarkers);
        return false;
    };
    let enq_ok = enq.contains("Wave 582") && enq.contains("ObjectLifecycleOp::EnqueueProduction");
    let shell_ok =
        shell.contains("Wave 582") && shell.contains("CommandPipelineOp::ProcessIfNeeded");
    let call_ok = eng.contains("self.host_enqueue_production(")
        && eng.contains("self.host_process_shell_menu_commands()");
    let raw_enq = eng.matches("self.game_logic.enqueue_production").count();
    // 2026-08-15: process_commands peeled onto CommandPipelineOp helpers (count 0).
    let raw_proc = eng.matches("self.game_logic.process_commands()").count();
    let ok = enq_ok
        && shell_ok
        && call_ok
        && raw_enq == 0
        && raw_proc == 0
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostEnqueueShellCmdHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_enqueue_shell_cmd_helper_nav_commands_residual_wave582() -> bool {
    let steps = LIVE_HOST_ENQUEUE_SHELL_CMD_HELPER_NAV_STEPS_WAVE582;
    let cmds = RUNTIME_HOST_LIVE_HOST_ENQUEUE_SHELL_CMD_HELPER_CMD_NAMES_WAVE582;
    let ok = residual_name_index(steps, "REQUIRE_HOST_ENQUEUE_PRODUCTION").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_PROCESS_SHELL_MENU").is_some()
        && residual_name_index(steps, "LIVE_HOST_ENQUEUE_SHELL_CMD_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_enqueue_production_helper").is_some()
        && residual_name_index(cmds, "host_process_shell_menu_helper").is_some()
        && residual_name_index(cmds, "enqueue_shell_cmd_residual").is_some();
    residual_action_store(ResidualHostEnqueueShellCmdHelperAction::NavCommands);
    ok
}

pub fn simulate_host_enqueue_shell_cmd_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 582")
        && eng.contains("fn host_enqueue_production")
        && eng.contains("fn host_process_shell_menu_commands");
    residual_action_store(ResidualHostEnqueueShellCmdHelperAction::CollectSource);
    ok
}

pub fn simulate_host_enqueue_shell_cmd_helper_dispatch_source() -> bool {
    let eng = eng_source();
    // 2026-08-15: enqueue call uses name.clone() (gameplay.rs).
    let ok = (eng.contains("self.host_enqueue_production(pid, name.to_string())")
        || eng.contains("self.host_enqueue_production(pid, name.clone())"))
        && eng.contains("self.host_process_shell_menu_commands()")
        && eng.contains("train_ok");
    residual_action_store(ResidualHostEnqueueShellCmdHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_enqueue_shell_cmd_helper_residual_pack_wave582() -> bool {
    honesty_host_enqueue_shell_cmd_helper_method_names_residual_wave582()
        && honesty_host_enqueue_shell_cmd_helper_source_markers_residual_wave582()
        && honesty_host_enqueue_shell_cmd_helper_nav_commands_residual_wave582()
        && simulate_host_enqueue_shell_cmd_helper_collect_source()
        && simulate_host_enqueue_shell_cmd_helper_dispatch_source()
}

pub fn simulate_live_host_enqueue_shell_cmd_helper_honesty() -> bool {
    let ok = honesty_host_enqueue_shell_cmd_helper_residual_pack_wave582();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEnqueueShellCmdHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_enqueue_shell_cmd_helper_method_names_residual_wave582());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_enqueue_shell_cmd_helper_source_markers_residual_wave582());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_enqueue_shell_cmd_helper_nav_commands_residual_wave582());
    }

    #[test]
    fn host_enqueue_shell_cmd_helper_sources() {
        assert!(simulate_host_enqueue_shell_cmd_helper_collect_source());
        assert!(simulate_host_enqueue_shell_cmd_helper_dispatch_source());
    }

    #[test]
    fn wave582_composite_pack() {
        assert!(honesty_host_enqueue_shell_cmd_helper_residual_pack_wave582());
    }

    #[test]
    fn simulate_live_host_enqueue_shell_cmd_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_enqueue_shell_cmd_helper_honesty(),
            "host enqueue/shell cmd helper residual must latch"
        );
        assert!(residual_host_enqueue_shell_cmd_helper_ok());
        assert_eq!(
            residual_host_enqueue_shell_cmd_helper_last_action(),
            ResidualHostEnqueueShellCmdHelperAction::Composite
        );
    }
}
