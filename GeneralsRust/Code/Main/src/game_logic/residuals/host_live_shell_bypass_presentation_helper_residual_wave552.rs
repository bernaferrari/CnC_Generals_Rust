//! Wave 552 residual peels: shell-bypass residual is centralized through
//! `presentation_or_boot_shell_bypass` / `presentation_affirms_shell_or_boot` /
//! `shell_bypass_from_presentation` — presentation freeze owns `fow_shell_bypass`
//! when installed; boot residual without freeze uses host `isInShellGame`.
//! Menu residual still requires freeze to *affirm* shell (stale InGame frames
//! fall through). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 551 time_frozen presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` shell-bypass helpers / call sites
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_SHELL_BYPASS_PRESENTATION_HELPER_METHOD_NAMES_WAVE552: &[&str] = &[
    "presentation_or_boot_shell_bypass",
    "presentation_affirms_shell_or_boot",
    "shell_bypass_from_presentation",
    "fow_shell_bypass",
    "isInShellGame",
    "Wave 552",
    "playable_claim = false",
];

pub const LIVE_SHELL_BYPASS_PRESENTATION_HELPER_NAV_STEPS_WAVE552: &[&str] = &[
    "REQUIRE_SHELL_BYPASS_PRESENTATION_HELPER",
    "REQUIRE_MENU_AFFIRMS_SHELL_RESIDUAL",
    "LIVE_SHELL_BYPASS_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_SHELL_BYPASS_PRESENTATION_HELPER_CMD_NAMES_WAVE552: &[&str] = &[
    "shell_bypass_presentation_helper",
    "presentation_shell_bypass_owns",
    "boot_isInShellGame",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualShellBypassPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualShellBypassPresentationHelperAction {
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

fn residual_action_store(action: ResidualShellBypassPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_shell_bypass_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_shell_bypass_presentation_helper_last_action()
-> ResidualShellBypassPresentationHelperAction {
    ResidualShellBypassPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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
pub fn honesty_shell_bypass_presentation_helper_method_names_residual_wave552() -> bool {
    let names = LIVE_SHELL_BYPASS_PRESENTATION_HELPER_METHOD_NAMES_WAVE552;
    let ok = residual_name_index(names, "presentation_or_boot_shell_bypass").is_some()
        && residual_name_index(names, "presentation_affirms_shell_or_boot").is_some()
        && residual_name_index(names, "shell_bypass_from_presentation").is_some()
        && residual_name_index(names, "fow_shell_bypass").is_some()
        && residual_name_index(names, "isInShellGame").is_some()
        && residual_name_index(names, "Wave 552").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualShellBypassPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_shell_bypass_presentation_helper_source_markers_residual_wave552() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn presentation_or_boot_shell_bypass(") else {
        residual_action_store(ResidualShellBypassPresentationHelperAction::SourceMarkers);
        return false;
    };
    let helper_ok = body.contains("Wave 552")
        && body.contains("pres.fow_shell_bypass")
        && (body.contains("host_match_in_shell") || body.contains("self.host_is_in_shell_game()"));
    let affirms = eng.contains("fn presentation_affirms_shell_or_boot")
        && eng.contains("presentation_affirms_shell_or_boot()");
    let from_pres = eng.contains("fn shell_bypass_from_presentation")
        && eng.contains("shell_bypass_from_presentation(startup_camera_presentation)");
    // Wave 585: raw isInShellGame lives only in host_is_in_shell_game; callers use helper.
    // 2026-08-15: Wave 895 fail-closed — no isInShellGame dual-read.
    let boot_only = eng.matches("self.game_logic.isInShellGame()").count() == 0
        && (eng.contains("fn host_is_in_shell_game") || eng.contains("host_match_in_shell"));
    let ok = helper_ok
        && affirms
        && from_pres
        && boot_only
        && eng.contains("presentation_or_boot_shell_bypass()")
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualShellBypassPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_shell_bypass_presentation_helper_nav_commands_residual_wave552() -> bool {
    let steps = LIVE_SHELL_BYPASS_PRESENTATION_HELPER_NAV_STEPS_WAVE552;
    let cmds = RUNTIME_HOST_LIVE_SHELL_BYPASS_PRESENTATION_HELPER_CMD_NAMES_WAVE552;
    let ok = residual_name_index(steps, "REQUIRE_SHELL_BYPASS_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_MENU_AFFIRMS_SHELL_RESIDUAL").is_some()
        && residual_name_index(steps, "LIVE_SHELL_BYPASS_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "shell_bypass_presentation_helper").is_some()
        && residual_name_index(cmds, "presentation_shell_bypass_owns").is_some()
        && residual_name_index(cmds, "boot_isInShellGame").is_some();
    residual_action_store(ResidualShellBypassPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_shell_bypass_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 552")
        && eng.contains("fn presentation_or_boot_shell_bypass")
        && eng.contains("fn presentation_affirms_shell_or_boot")
        && eng.contains("fn shell_bypass_from_presentation");
    residual_action_store(ResidualShellBypassPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_shell_bypass_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("presentation_or_boot_shell_bypass()")
        && eng.contains("presentation_affirms_shell_or_boot()")
        && eng.contains("shell_bypass_from_presentation(startup_camera_presentation)")
        // 2026-08-15: Wave 895 fail-closed — last isInShellGame dual-read peeled.
        && eng.matches("self.game_logic.isInShellGame()").count() == 0
        && eng.contains("self.host_is_in_shell_game()");
    residual_action_store(ResidualShellBypassPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_shell_bypass_presentation_helper_residual_pack_wave552() -> bool {
    honesty_shell_bypass_presentation_helper_method_names_residual_wave552()
        && honesty_shell_bypass_presentation_helper_source_markers_residual_wave552()
        && honesty_shell_bypass_presentation_helper_nav_commands_residual_wave552()
        && simulate_shell_bypass_presentation_helper_collect_source()
        && simulate_shell_bypass_presentation_helper_dispatch_source()
}

pub fn simulate_live_shell_bypass_presentation_helper_honesty() -> bool {
    let ok = honesty_shell_bypass_presentation_helper_residual_pack_wave552();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualShellBypassPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_shell_bypass_presentation_helper_method_names_residual_wave552());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_shell_bypass_presentation_helper_source_markers_residual_wave552());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_shell_bypass_presentation_helper_nav_commands_residual_wave552());
    }

    #[test]
    fn shell_bypass_presentation_helper_sources() {
        assert!(simulate_shell_bypass_presentation_helper_collect_source());
        assert!(simulate_shell_bypass_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave552_composite_pack() {
        assert!(honesty_shell_bypass_presentation_helper_residual_pack_wave552());
    }

    #[test]
    fn simulate_live_shell_bypass_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_shell_bypass_presentation_helper_honesty(),
            "shell bypass presentation helper residual must latch"
        );
        assert!(residual_shell_bypass_presentation_helper_ok());
        assert_eq!(
            residual_shell_bypass_presentation_helper_last_action(),
            ResidualShellBypassPresentationHelperAction::Composite
        );
    }
}
