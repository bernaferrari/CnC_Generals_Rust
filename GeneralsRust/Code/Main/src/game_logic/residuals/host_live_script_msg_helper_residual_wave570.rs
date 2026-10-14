//! Wave 570 residual peels: new script messages residual is centralized through
//! `take_presentation_or_boot_new_script_messages` (pipeline/last freeze prefer +
//! drain, else boot take). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 569 defeat/alliance helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` take_presentation_or_boot_new_script_messages
//! - `presentation_frame.rs` new_script_messages
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_SCRIPT_MSG_HELPER_METHOD_NAMES_WAVE570: &[&str] = &[
    "take_presentation_or_boot_new_script_messages",
    "new_script_messages",
    "take_new_script_messages",
    "Wave 570",
    "playable_claim = false",
];

pub const LIVE_SCRIPT_MSG_HELPER_NAV_STEPS_WAVE570: &[&str] = &[
    "REQUIRE_SCRIPT_MSG_HELPER",
    "LIVE_SCRIPT_MSG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_SCRIPT_MSG_HELPER_CMD_NAMES_WAVE570: &[&str] =
    &["script_msg_helper", "new_script_messages_residual"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResidualScriptMsgHelperAction {
    MethodNames,
    SourceMarkers,
    NavCommands,
    CollectSource,
    DispatchSource,
    Composite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InspectionOutcome {
    ok: bool,
    action: ResidualScriptMsgHelperAction,
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
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
pub fn honesty_script_msg_helper_method_names_residual_wave570() -> bool {
    let names = LIVE_SCRIPT_MSG_HELPER_METHOD_NAMES_WAVE570;
    let ok = residual_name_index(names, "take_presentation_or_boot_new_script_messages").is_some()
        && residual_name_index(names, "new_script_messages").is_some()
        && residual_name_index(names, "take_new_script_messages").is_some()
        && residual_name_index(names, "Wave 570").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();

    ok
}

pub fn honesty_script_msg_helper_source_markers_residual_wave570() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("pub new_script_messages: Vec<String>");
    let Some(body) = fn_body(
        eng,
        "fn host_take_presentation_or_boot_new_script_messages(",
    )
    .or_else(|| fn_body(eng, "fn take_presentation_or_boot_new_script_messages(")) else {
        return false;
    };
    // 2026-08-15: Wave 900 fail-closed — host helper clones freeze messages,
    // no live take_new_script_messages drain (camera_drain.rs:945-956).
    let body_ok =
        (body.contains("Wave 570") || body.contains("Wave 607") || body.contains("Wave 900"))
            && body.contains("new_script_messages")
            && body.contains("presentation_frame()")
            && !body.contains("self.game_logic.take_new_script_messages()");
    let call_ok = eng.contains("self.take_presentation_or_boot_new_script_messages()");
    let ok = field_ok
        && body_ok
        && call_ok
        && !eng.contains("self.game_logic.take_new_script_messages()")
        && !eng.contains("playable_claim = true");

    ok
}

pub fn honesty_script_msg_helper_nav_commands_residual_wave570() -> bool {
    let steps = LIVE_SCRIPT_MSG_HELPER_NAV_STEPS_WAVE570;
    let cmds = RUNTIME_HOST_LIVE_SCRIPT_MSG_HELPER_CMD_NAMES_WAVE570;
    let ok = residual_name_index(steps, "REQUIRE_SCRIPT_MSG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_SCRIPT_MSG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "script_msg_helper").is_some()
        && residual_name_index(cmds, "new_script_messages_residual").is_some();

    ok
}

pub fn simulate_script_msg_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 570")
        && eng.contains("fn take_presentation_or_boot_new_script_messages");

    ok
}

pub fn simulate_script_msg_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.take_presentation_or_boot_new_script_messages()")
        && eng.contains("push_script_message");

    ok
}

fn inspect_script_msg_helper() -> InspectionOutcome {
    use ResidualScriptMsgHelperAction::*;
    for (action, predicate) in [
        (
            MethodNames,
            honesty_script_msg_helper_method_names_residual_wave570 as fn() -> bool,
        ),
        (
            SourceMarkers,
            honesty_script_msg_helper_source_markers_residual_wave570,
        ),
        (
            NavCommands,
            honesty_script_msg_helper_nav_commands_residual_wave570,
        ),
        (CollectSource, simulate_script_msg_helper_collect_source),
        (DispatchSource, simulate_script_msg_helper_dispatch_source),
    ] {
        if !predicate() {
            return InspectionOutcome { ok: false, action };
        }
    }
    InspectionOutcome {
        ok: true,
        action: Composite,
    }
}

pub fn honesty_script_msg_helper_residual_pack_wave570() -> bool {
    inspect_script_msg_helper().ok
}

pub fn simulate_live_script_msg_helper_honesty() -> bool {
    inspect_script_msg_helper().ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_script_msg_helper_method_names_residual_wave570());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_script_msg_helper_source_markers_residual_wave570());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_script_msg_helper_nav_commands_residual_wave570());
    }

    #[test]
    fn script_msg_helper_sources() {
        assert!(simulate_script_msg_helper_collect_source());
        assert!(simulate_script_msg_helper_dispatch_source());
    }

    #[test]
    fn wave570_composite_pack() {
        assert!(honesty_script_msg_helper_residual_pack_wave570());
    }

    #[test]
    fn simulate_live_script_msg_helper_honesty_residual_live() {
        let result = inspect_script_msg_helper();
        assert!(result.ok, "script msg helper inspection must succeed");
        assert_eq!(result.action, ResidualScriptMsgHelperAction::Composite);
    }

    #[test]
    fn interleaved_inspections_preserve_each_owned_result() {
        let first = inspect_script_msg_helper();
        assert!(honesty_script_msg_helper_source_markers_residual_wave570());
        assert!(honesty_script_msg_helper_method_names_residual_wave570());
        let second = inspect_script_msg_helper();
        assert!(simulate_script_msg_helper_dispatch_source());
        assert!(first.ok && second.ok);
        assert_eq!(first.action, ResidualScriptMsgHelperAction::Composite);
        assert_eq!(second, first);
    }
}
