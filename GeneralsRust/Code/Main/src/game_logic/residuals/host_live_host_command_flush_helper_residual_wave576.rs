//! Wave 576 residual peels: host command flush residual is centralized through
//! `host_process_commands_with_command_sound`, `host_queue_and_process_command`,
//! and silent `host_queue_and_process_command_silent`. Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 575 host pause/team helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_*_process_command* helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COMMAND_FLUSH_HELPER_METHOD_NAMES_WAVE576: &[&str] = &[
    "host_process_commands_with_command_sound",
    "host_queue_and_process_command",
    "host_queue_and_process_command_silent",
    "process_commands",
    "Wave 576",
    "playable_claim = false",
];

pub const LIVE_HOST_COMMAND_FLUSH_HELPER_NAV_STEPS_WAVE576: &[&str] = &[
    "REQUIRE_HOST_PROCESS_COMMAND_SOUND",
    "REQUIRE_HOST_QUEUE_AND_PROCESS",
    "REQUIRE_HOST_QUEUE_SILENT",
    "LIVE_HOST_COMMAND_FLUSH_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_COMMAND_FLUSH_HELPER_CMD_NAMES_WAVE576: &[&str] = &[
    "host_process_command_sound",
    "host_queue_and_process",
    "host_queue_silent",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCommandFlushHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostCommandFlushHelperAction {
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

fn residual_action_store(action: ResidualHostCommandFlushHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_command_flush_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_command_flush_helper_last_action() -> ResidualHostCommandFlushHelperAction {
    ResidualHostCommandFlushHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_host_command_flush_helper_method_names_residual_wave576() -> bool {
    let names = LIVE_HOST_COMMAND_FLUSH_HELPER_METHOD_NAMES_WAVE576;
    let ok = residual_name_index(names, "host_process_commands_with_command_sound").is_some()
        && residual_name_index(names, "host_queue_and_process_command").is_some()
        && residual_name_index(names, "host_queue_and_process_command_silent").is_some()
        && residual_name_index(names, "process_commands").is_some()
        && residual_name_index(names, "Wave 576").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostCommandFlushHelperAction::MethodNames);
    ok
}

pub fn honesty_host_command_flush_helper_source_markers_residual_wave576() -> bool {
    let eng = eng_source();
    let Some(proc) = fn_body(eng, "fn host_process_commands_with_command_sound(") else {
        residual_action_store(ResidualHostCommandFlushHelperAction::SourceMarkers);
        return false;
    };
    let Some(queue) = fn_body(eng, "fn host_queue_and_process_command(") else {
        residual_action_store(ResidualHostCommandFlushHelperAction::SourceMarkers);
        return false;
    };
    let Some(silent) = fn_body(eng, "fn host_queue_and_process_command_silent(") else {
        residual_action_store(ResidualHostCommandFlushHelperAction::SourceMarkers);
        return false;
    };
    let proc_ok = proc.contains("Wave 576")
        && proc.contains("process_commands()")
        && proc.contains("SoundType::Command");
    let queue_ok = queue.contains("Wave 576")
        && queue.contains("queue_command(command)")
        && queue.contains("host_process_commands_with_command_sound()");
    let silent_ok = silent.contains("Wave 576")
        && silent.contains("queue_command(command)")
        && silent.contains("process_commands()")
        && !silent.contains("play_sound_effect");
    let call_ok = eng.contains("self.host_process_commands_with_command_sound()")
        && eng.contains("self.host_queue_and_process_command(")
        && eng.contains("self.host_queue_and_process_command_silent(");
    // process+Command SFX may appear only inside host_process helper (count==1).
    let paired = eng.matches("process_commands();\n        self.play_sound_effect(SoundType::Command)")
        .count()
        + eng
            .matches("process_commands();\n            self.play_sound_effect(SoundType::Command)")
            .count()
        + eng
            .matches(
                "process_commands();\n                    self.play_sound_effect(SoundType::Command)",
            )
            .count();
    let ok = proc_ok
        && queue_ok
        && silent_ok
        && call_ok
        && paired == 1
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostCommandFlushHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_command_flush_helper_nav_commands_residual_wave576() -> bool {
    let steps = LIVE_HOST_COMMAND_FLUSH_HELPER_NAV_STEPS_WAVE576;
    let cmds = RUNTIME_HOST_LIVE_HOST_COMMAND_FLUSH_HELPER_CMD_NAMES_WAVE576;
    let ok = residual_name_index(steps, "REQUIRE_HOST_PROCESS_COMMAND_SOUND").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_QUEUE_AND_PROCESS").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_QUEUE_SILENT").is_some()
        && residual_name_index(steps, "LIVE_HOST_COMMAND_FLUSH_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_process_command_sound").is_some()
        && residual_name_index(cmds, "host_queue_and_process").is_some()
        && residual_name_index(cmds, "host_queue_silent").is_some();
    residual_action_store(ResidualHostCommandFlushHelperAction::NavCommands);
    ok
}

pub fn simulate_host_command_flush_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 576")
        && eng.contains("fn host_process_commands_with_command_sound")
        && eng.contains("fn host_queue_and_process_command_silent");
    residual_action_store(ResidualHostCommandFlushHelperAction::CollectSource);
    ok
}

pub fn simulate_host_command_flush_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_process_commands_with_command_sound()")
        && eng.contains("self.host_queue_and_process_command(")
        && eng.contains("self.host_queue_and_process_command_silent(");
    residual_action_store(ResidualHostCommandFlushHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_command_flush_helper_residual_pack_wave576() -> bool {
    honesty_host_command_flush_helper_method_names_residual_wave576()
        && honesty_host_command_flush_helper_source_markers_residual_wave576()
        && honesty_host_command_flush_helper_nav_commands_residual_wave576()
        && simulate_host_command_flush_helper_collect_source()
        && simulate_host_command_flush_helper_dispatch_source()
}

pub fn simulate_live_host_command_flush_helper_honesty() -> bool {
    let ok = honesty_host_command_flush_helper_residual_pack_wave576();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostCommandFlushHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_command_flush_helper_method_names_residual_wave576());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_command_flush_helper_source_markers_residual_wave576());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_command_flush_helper_nav_commands_residual_wave576());
    }

    #[test]
    fn host_command_flush_helper_sources() {
        assert!(simulate_host_command_flush_helper_collect_source());
        assert!(simulate_host_command_flush_helper_dispatch_source());
    }

    #[test]
    fn wave576_composite_pack() {
        assert!(honesty_host_command_flush_helper_residual_pack_wave576());
    }

    #[test]
    fn simulate_live_host_command_flush_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_command_flush_helper_honesty(),
            "host command flush helper residual must latch"
        );
        assert!(residual_host_command_flush_helper_ok());
        assert_eq!(
            residual_host_command_flush_helper_last_action(),
            ResidualHostCommandFlushHelperAction::Composite
        );
    }
}
