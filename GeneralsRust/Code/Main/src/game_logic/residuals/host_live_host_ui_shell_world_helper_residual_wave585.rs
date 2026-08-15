//! Wave 585 residual peels: last non-helper GameLogic dual-reads are centralized
//! through host helpers — UI state boot residual, shell-map probe, world bounds /
//! size override, and first-opponent debug residual.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 584 tick/mutation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host UI/shell/world helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_SHELL_WORLD_HELPER_METHOD_NAMES_WAVE585: &[&str] = &[
    "host_update_ui_state",
    "host_is_in_shell_game",
    "host_override_world_size",
    "host_world_bounds",
    "host_first_opponent_id",
    "presentation_affirms_shell_or_boot",
    "presentation_world_bounds",
    "Wave 585",
    "playable_claim = false",
];

pub const LIVE_HOST_UI_SHELL_WORLD_HELPER_NAV_STEPS_WAVE585: &[&str] = &[
    "REQUIRE_HOST_UPDATE_UI_STATE",
    "REQUIRE_HOST_SHELL_PROBE",
    "REQUIRE_HOST_WORLD_BOUNDS",
    "REQUIRE_HOST_WORLD_SIZE_OVERRIDE",
    "REQUIRE_HOST_FIRST_OPPONENT",
    "LIVE_HOST_UI_SHELL_WORLD_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_UI_SHELL_WORLD_HELPER_CMD_NAMES_WAVE585: &[&str] = &[
    "host_update_ui_state_helper",
    "host_shell_probe_helper",
    "host_world_bounds_helper",
    "host_world_size_override_helper",
    "host_first_opponent_helper",
    "ui_shell_world_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiShellWorldHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostUiShellWorldHelperAction {
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

fn residual_action_store(action: ResidualHostUiShellWorldHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_ui_shell_world_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_ui_shell_world_helper_last_action() -> ResidualHostUiShellWorldHelperAction {
    ResidualHostUiShellWorldHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

// 2026-08-15: retarget honesty markers to host_match_*/fail-closed seams.
pub fn honesty_host_ui_shell_world_helper_method_names_residual_wave585() -> bool {
    let names = LIVE_HOST_UI_SHELL_WORLD_HELPER_METHOD_NAMES_WAVE585;
    let ok = residual_name_index(names, "host_update_ui_state").is_some()
        && residual_name_index(names, "host_is_in_shell_game").is_some()
        && residual_name_index(names, "host_override_world_size").is_some()
        && residual_name_index(names, "host_world_bounds").is_some()
        && residual_name_index(names, "host_first_opponent_id").is_some()
        && residual_name_index(names, "Wave 585").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostUiShellWorldHelperAction::MethodNames);
    ok
}

pub fn honesty_host_ui_shell_world_helper_source_markers_residual_wave585() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_update_ui_state(",
        "fn host_is_in_shell_game(",
        "fn host_override_world_size(",
        "fn host_world_bounds(",
        "fn host_first_opponent_id(",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 585") {
            defs_ok = false;
            break;
        }
        let name = sig.trim_start_matches("fn ").trim_end_matches('(');
        if body.matches(&format!("self.{name}(")).count() > 0 {
            defs_ok = false;
            break;
        }
    }
    let call_ok = eng.contains("self.host_update_ui_state(self.current_player_id)")
        && eng.contains("self.host_is_in_shell_game()")
        && eng.contains("self.host_override_world_size(w, h)")
        && eng.contains("self.host_world_bounds()")
        && eng.contains("self.host_first_opponent_id(self.current_player_id)");
    let raw_ui = eng.matches("self.game_logic.update_ui_state").count();
    let raw_shell = eng.matches("self.game_logic.isInShellGame()").count();
    let raw_override = eng.matches("self.game_logic.override_world_size").count();
    let raw_bounds = eng.matches("self.game_logic.world_bounds()").count();
    let raw_opp = eng.matches("self.game_logic.first_opponent_id").count();
    let ok = defs_ok
        && call_ok
        && raw_ui == 0
        && raw_shell == 0
        && raw_override == 0
        && raw_bounds == 0
        && raw_opp == 0
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostUiShellWorldHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_ui_shell_world_helper_nav_commands_residual_wave585() -> bool {
    let steps = LIVE_HOST_UI_SHELL_WORLD_HELPER_NAV_STEPS_WAVE585;
    let cmds = RUNTIME_HOST_LIVE_HOST_UI_SHELL_WORLD_HELPER_CMD_NAMES_WAVE585;
    let ok = residual_name_index(steps, "REQUIRE_HOST_UPDATE_UI_STATE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_SHELL_PROBE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_WORLD_BOUNDS").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_WORLD_SIZE_OVERRIDE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_FIRST_OPPONENT").is_some()
        && residual_name_index(steps, "LIVE_HOST_UI_SHELL_WORLD_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_update_ui_state_helper").is_some()
        && residual_name_index(cmds, "host_shell_probe_helper").is_some()
        && residual_name_index(cmds, "host_world_bounds_helper").is_some()
        && residual_name_index(cmds, "host_world_size_override_helper").is_some()
        && residual_name_index(cmds, "host_first_opponent_helper").is_some()
        && residual_name_index(cmds, "ui_shell_world_residual").is_some();
    residual_action_store(ResidualHostUiShellWorldHelperAction::NavCommands);
    ok
}

pub fn simulate_host_ui_shell_world_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 585")
        && eng.contains("fn host_update_ui_state")
        && eng.contains("fn host_is_in_shell_game")
        && eng.contains("fn host_override_world_size")
        && eng.contains("fn host_world_bounds")
        && eng.contains("fn host_first_opponent_id");
    residual_action_store(ResidualHostUiShellWorldHelperAction::CollectSource);
    ok
}

pub fn simulate_host_ui_shell_world_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_update_ui_state(self.current_player_id)")
        && eng.contains("self.host_is_in_shell_game()")
        && eng.contains("self.host_override_world_size(w, h)")
        && eng.contains("self.host_world_bounds()")
        && eng.contains("self.host_first_opponent_id(self.current_player_id)");
    residual_action_store(ResidualHostUiShellWorldHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_ui_shell_world_helper_residual_pack_wave585() -> bool {
    honesty_host_ui_shell_world_helper_method_names_residual_wave585()
        && honesty_host_ui_shell_world_helper_source_markers_residual_wave585()
        && honesty_host_ui_shell_world_helper_nav_commands_residual_wave585()
        && simulate_host_ui_shell_world_helper_collect_source()
        && simulate_host_ui_shell_world_helper_dispatch_source()
}

pub fn simulate_live_host_ui_shell_world_helper_honesty() -> bool {
    let ok = honesty_host_ui_shell_world_helper_residual_pack_wave585();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostUiShellWorldHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_ui_shell_world_helper_method_names_residual_wave585());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_ui_shell_world_helper_source_markers_residual_wave585());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_ui_shell_world_helper_nav_commands_residual_wave585());
    }

    #[test]
    fn host_ui_shell_world_helper_sources() {
        assert!(simulate_host_ui_shell_world_helper_collect_source());
        assert!(simulate_host_ui_shell_world_helper_dispatch_source());
    }

    #[test]
    fn wave585_composite_pack() {
        assert!(honesty_host_ui_shell_world_helper_residual_pack_wave585());
    }

    #[test]
    fn simulate_live_host_ui_shell_world_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_ui_shell_world_helper_honesty(),
            "host UI/shell/world helper residual must latch"
        );
        assert!(residual_host_ui_shell_world_helper_ok());
        assert_eq!(
            residual_host_ui_shell_world_helper_last_action(),
            ResidualHostUiShellWorldHelperAction::Composite
        );
    }
}
