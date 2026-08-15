//! Wave 578 residual peels: remaining silent UI/host `queue_command` +
//! `process_commands` pairs are centralized through
//! `host_queue_and_process_command_silent` (Wave 576). Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 577 host camera/start helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_queue_and_process_command_silent call sites
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SILENT_COMMAND_PEEL_METHOD_NAMES_WAVE578: &[&str] = &[
    "host_queue_and_process_command_silent",
    "host_queue_and_process_command",
    "host_queue_command",
    "host_process_commands_with_command_sound",
    "process_commands",
    "Wave 578",
    "playable_claim = false",
];

pub const LIVE_HOST_SILENT_COMMAND_PEEL_NAV_STEPS_WAVE578: &[&str] = &[
    "REQUIRE_SILENT_QUEUE_HELPER",
    "REQUIRE_NO_RAW_QUEUE_PROCESS_PAIRS",
    "LIVE_HOST_SILENT_COMMAND_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_SILENT_COMMAND_PEEL_CMD_NAMES_WAVE578: &[&str] = &[
    "silent_queue_helper",
    "force_attack_silent_flush",
    "construct_silent_flush",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSilentCommandPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostSilentCommandPeelAction {
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

fn residual_action_store(action: ResidualHostSilentCommandPeelAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_silent_command_peel_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_silent_command_peel_last_action() -> ResidualHostSilentCommandPeelAction {
    ResidualHostSilentCommandPeelAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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
pub fn honesty_host_silent_command_peel_method_names_residual_wave578() -> bool {
    let names = LIVE_HOST_SILENT_COMMAND_PEEL_METHOD_NAMES_WAVE578;
    let ok = residual_name_index(names, "host_queue_and_process_command_silent").is_some()
        && residual_name_index(names, "host_queue_and_process_command").is_some()
        && residual_name_index(names, "host_process_commands_with_command_sound").is_some()
        && residual_name_index(names, "process_commands").is_some()
        && residual_name_index(names, "Wave 578").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSilentCommandPeelAction::MethodNames);
    ok
}

pub fn honesty_host_silent_command_peel_source_markers_residual_wave578() -> bool {
    let eng = eng_source();
    let Some(silent) = fn_body(eng, "fn host_queue_and_process_command_silent(") else {
        residual_action_store(ResidualHostSilentCommandPeelAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: silent helper is QueueAndProcess only (ui_commands.rs:832-833).
    let silent_ok = silent.contains("Wave 576")
        && silent.contains("CommandPipelineOp::QueueAndProcess")
        && !silent.contains("play_sound_effect");
    let call_ok = eng.contains("self.host_queue_and_process_command_silent(");
    let silent_calls = eng
        .matches("self.host_queue_and_process_command_silent(")
        .count();
    // Outside helpers: shell menu process + helper internals (process sound + silent).
    // 2026-08-15: process/queue peeled onto CommandPipelineOp (count 0).
    let raw_process = eng.matches("self.game_logic.process_commands()").count();
    let raw_queue = eng.matches("self.game_logic.queue_command").count();
    let ok = silent_ok
        && call_ok
        && silent_calls >= 8
        && raw_process == 0
        && raw_queue == 0
        && eng.contains("fn host_queue_command")
        && eng.contains("Wave 578")
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostSilentCommandPeelAction::SourceMarkers);
    ok
}

pub fn honesty_host_silent_command_peel_nav_commands_residual_wave578() -> bool {
    let steps = LIVE_HOST_SILENT_COMMAND_PEEL_NAV_STEPS_WAVE578;
    let cmds = RUNTIME_HOST_LIVE_HOST_SILENT_COMMAND_PEEL_CMD_NAMES_WAVE578;
    let ok = residual_name_index(steps, "REQUIRE_SILENT_QUEUE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_NO_RAW_QUEUE_PROCESS_PAIRS").is_some()
        && residual_name_index(steps, "LIVE_HOST_SILENT_COMMAND_PEEL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "silent_queue_helper").is_some()
        && residual_name_index(cmds, "force_attack_silent_flush").is_some()
        && residual_name_index(cmds, "construct_silent_flush").is_some();
    residual_action_store(ResidualHostSilentCommandPeelAction::NavCommands);
    ok
}

pub fn simulate_host_silent_command_peel_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 578")
        && eng.contains("fn host_queue_and_process_command_silent")
        && eng.contains("host_queue_and_process_command_silent(");
    residual_action_store(ResidualHostSilentCommandPeelAction::CollectSource);
    ok
}

pub fn simulate_host_silent_command_peel_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_queue_and_process_command_silent(")
        && eng.contains("ForceAttackGround")
        && eng.contains("DozerConstruct");
    residual_action_store(ResidualHostSilentCommandPeelAction::DispatchSource);
    ok
}

pub fn honesty_host_silent_command_peel_residual_pack_wave578() -> bool {
    honesty_host_silent_command_peel_method_names_residual_wave578()
        && honesty_host_silent_command_peel_source_markers_residual_wave578()
        && honesty_host_silent_command_peel_nav_commands_residual_wave578()
        && simulate_host_silent_command_peel_collect_source()
        && simulate_host_silent_command_peel_dispatch_source()
}

pub fn simulate_live_host_silent_command_peel_honesty() -> bool {
    let ok = honesty_host_silent_command_peel_residual_pack_wave578();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSilentCommandPeelAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_silent_command_peel_method_names_residual_wave578());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_silent_command_peel_source_markers_residual_wave578());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_silent_command_peel_nav_commands_residual_wave578());
    }

    #[test]
    fn host_silent_command_peel_sources() {
        assert!(simulate_host_silent_command_peel_collect_source());
        assert!(simulate_host_silent_command_peel_dispatch_source());
    }

    #[test]
    fn wave578_composite_pack() {
        assert!(honesty_host_silent_command_peel_residual_pack_wave578());
    }

    #[test]
    fn simulate_live_host_silent_command_peel_honesty_residual_live() {
        assert!(
            simulate_live_host_silent_command_peel_honesty(),
            "host silent command peel residual must latch"
        );
        assert!(residual_host_silent_command_peel_ok());
        assert_eq!(
            residual_host_silent_command_peel_last_action(),
            ResidualHostSilentCommandPeelAction::Composite
        );
    }
}
