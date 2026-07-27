//! Wave 574 residual peels: boot local player id residual is centralized through
//! `boot_local_player_id_from_host`. `local_player_id_for_ui` prefers presentation
//! freeze then that helper. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 573 boot player info helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` boot_local_player_id_from_host / local_player_id_for_ui
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_BOOT_LOCAL_PLAYER_HELPER_METHOD_NAMES_WAVE574: &[&str] = &[
    "boot_local_player_id_from_host",
    "local_player_id_for_ui",
    "presentation_or_boot_local_player_id",
    "player_exists",
    "Wave 574",
    "playable_claim = false",
];

pub const LIVE_BOOT_LOCAL_PLAYER_HELPER_NAV_STEPS_WAVE574: &[&str] = &[
    "REQUIRE_BOOT_LOCAL_PLAYER_HELPER",
    "REQUIRE_LOCAL_PLAYER_UI_USES_HELPER",
    "LIVE_BOOT_LOCAL_PLAYER_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_BOOT_LOCAL_PLAYER_HELPER_CMD_NAMES_WAVE574: &[&str] = &[
    "boot_local_player_helper",
    "local_player_id_for_ui_helper",
    "local_player_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualBootLocalPlayerHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualBootLocalPlayerHelperAction {
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

fn residual_action_store(action: ResidualBootLocalPlayerHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_boot_local_player_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_boot_local_player_helper_last_action() -> ResidualBootLocalPlayerHelperAction {
    ResidualBootLocalPlayerHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
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

pub fn honesty_boot_local_player_helper_method_names_residual_wave574() -> bool {
    let names = LIVE_BOOT_LOCAL_PLAYER_HELPER_METHOD_NAMES_WAVE574;
    let ok = residual_name_index(names, "boot_local_player_id_from_host").is_some()
        && residual_name_index(names, "local_player_id_for_ui").is_some()
        && residual_name_index(names, "presentation_or_boot_local_player_id").is_some()
        && residual_name_index(names, "player_exists").is_some()
        && residual_name_index(names, "Wave 574").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualBootLocalPlayerHelperAction::MethodNames);
    ok
}

pub fn honesty_boot_local_player_helper_source_markers_residual_wave574() -> bool {
    let eng = eng_source();
    let Some(boot) = fn_body(eng, "fn boot_local_player_id_from_host(") else {
        residual_action_store(ResidualBootLocalPlayerHelperAction::SourceMarkers);
        return false;
    };
    let Some(ui) = fn_body(eng, "fn local_player_id_for_ui(") else {
        residual_action_store(ResidualBootLocalPlayerHelperAction::SourceMarkers);
        return false;
    };
    let boot_ok = boot.contains("Wave 574")
        && boot.contains("player_exists(self.current_player_id)")
        && boot.contains("min_player_id()");
    let ui_ok = ui.contains("Wave 574")
        && ui.contains("presentation_or_boot_local_player_id()")
        && ui.contains("boot_local_player_id_from_host()")
        && !ui.contains("player_exists(self.current_player_id)");
    let raw_exists = eng.matches("self.game_logic.player_exists").count();
    // boot_player_info_from_host + boot_local_player_id_from_host
    let ok = boot_ok && ui_ok && raw_exists == 2 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualBootLocalPlayerHelperAction::SourceMarkers);
    ok
}

pub fn honesty_boot_local_player_helper_nav_commands_residual_wave574() -> bool {
    let steps = LIVE_BOOT_LOCAL_PLAYER_HELPER_NAV_STEPS_WAVE574;
    let cmds = RUNTIME_HOST_LIVE_BOOT_LOCAL_PLAYER_HELPER_CMD_NAMES_WAVE574;
    let ok = residual_name_index(steps, "REQUIRE_BOOT_LOCAL_PLAYER_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_LOCAL_PLAYER_UI_USES_HELPER").is_some()
        && residual_name_index(steps, "LIVE_BOOT_LOCAL_PLAYER_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "boot_local_player_helper").is_some()
        && residual_name_index(cmds, "local_player_id_for_ui_helper").is_some()
        && residual_name_index(cmds, "local_player_residual").is_some();
    residual_action_store(ResidualBootLocalPlayerHelperAction::NavCommands);
    ok
}

pub fn simulate_boot_local_player_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 574")
        && eng.contains("fn boot_local_player_id_from_host")
        && eng.contains("fn local_player_id_for_ui");
    residual_action_store(ResidualBootLocalPlayerHelperAction::CollectSource);
    ok
}

pub fn simulate_boot_local_player_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(ui) = fn_body(eng, "fn local_player_id_for_ui(") else {
        residual_action_store(ResidualBootLocalPlayerHelperAction::DispatchSource);
        return false;
    };
    let ok = ui.contains("self.boot_local_player_id_from_host()")
        && ui.contains("presentation_or_boot_local_player_id()");
    residual_action_store(ResidualBootLocalPlayerHelperAction::DispatchSource);
    ok
}

pub fn honesty_boot_local_player_helper_residual_pack_wave574() -> bool {
    honesty_boot_local_player_helper_method_names_residual_wave574()
        && honesty_boot_local_player_helper_source_markers_residual_wave574()
        && honesty_boot_local_player_helper_nav_commands_residual_wave574()
        && simulate_boot_local_player_helper_collect_source()
        && simulate_boot_local_player_helper_dispatch_source()
}

pub fn simulate_live_boot_local_player_helper_honesty() -> bool {
    let ok = honesty_boot_local_player_helper_residual_pack_wave574();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualBootLocalPlayerHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_boot_local_player_helper_method_names_residual_wave574());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_boot_local_player_helper_source_markers_residual_wave574());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_boot_local_player_helper_nav_commands_residual_wave574());
    }

    #[test]
    fn boot_local_player_helper_sources() {
        assert!(simulate_boot_local_player_helper_collect_source());
        assert!(simulate_boot_local_player_helper_dispatch_source());
    }

    #[test]
    fn wave574_composite_pack() {
        assert!(honesty_boot_local_player_helper_residual_pack_wave574());
    }

    #[test]
    fn simulate_live_boot_local_player_helper_honesty_residual_live() {
        assert!(
            simulate_live_boot_local_player_helper_honesty(),
            "boot local player helper residual must latch"
        );
        assert!(residual_boot_local_player_helper_ok());
        assert_eq!(
            residual_boot_local_player_helper_last_action(),
            ResidualBootLocalPlayerHelperAction::Composite
        );
    }
}
