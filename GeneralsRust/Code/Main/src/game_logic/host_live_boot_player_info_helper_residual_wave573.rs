//! Wave 573 residual peels: boot player roster probe is centralized through
//! `boot_player_info_from_host`. `ui_player_info` and
//! `presentation_or_boot_diplomacy_players` share it. Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 572 boot camera helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` boot_player_info_from_host / ui_player_info /
//!   presentation_or_boot_diplomacy_players
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_BOOT_PLAYER_INFO_HELPER_METHOD_NAMES_WAVE573: &[&str] = &[
    "boot_player_info_from_host",
    "ui_player_info",
    "presentation_or_boot_diplomacy_players",
    "player_exists",
    "Wave 573",
    "playable_claim = false",
];

pub const LIVE_BOOT_PLAYER_INFO_HELPER_NAV_STEPS_WAVE573: &[&str] = &[
    "REQUIRE_BOOT_PLAYER_INFO_HELPER",
    "REQUIRE_UI_PLAYER_USES_HELPER",
    "REQUIRE_DIPLOMACY_USES_HELPER",
    "LIVE_BOOT_PLAYER_INFO_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_BOOT_PLAYER_INFO_HELPER_CMD_NAMES_WAVE573: &[&str] = &[
    "boot_player_info_helper",
    "ui_player_info_helper",
    "diplomacy_boot_helper",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualBootPlayerInfoHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualBootPlayerInfoHelperAction {
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

fn residual_action_store(action: ResidualBootPlayerInfoHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_boot_player_info_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_boot_player_info_helper_last_action() -> ResidualBootPlayerInfoHelperAction {
    ResidualBootPlayerInfoHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_boot_player_info_helper_method_names_residual_wave573() -> bool {
    let names = LIVE_BOOT_PLAYER_INFO_HELPER_METHOD_NAMES_WAVE573;
    let ok = residual_name_index(names, "boot_player_info_from_host").is_some()
        && residual_name_index(names, "ui_player_info").is_some()
        && residual_name_index(names, "presentation_or_boot_diplomacy_players").is_some()
        && residual_name_index(names, "player_exists").is_some()
        && residual_name_index(names, "Wave 573").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualBootPlayerInfoHelperAction::MethodNames);
    ok
}

pub fn honesty_boot_player_info_helper_source_markers_residual_wave573() -> bool {
    let eng = eng_source();
    let Some(boot) = fn_body(eng, "fn boot_player_info_from_host(") else {
        residual_action_store(ResidualBootPlayerInfoHelperAction::SourceMarkers);
        return false;
    };
    let Some(ui) = fn_body(eng, "fn ui_player_info(") else {
        residual_action_store(ResidualBootPlayerInfoHelperAction::SourceMarkers);
        return false;
    };
    let Some(dip) = fn_body(eng, "fn presentation_or_boot_diplomacy_players(") else {
        residual_action_store(ResidualBootPlayerInfoHelperAction::SourceMarkers);
        return false;
    };
    let boot_ok = boot.contains("Wave 573")
        && boot.contains("player_exists(player_id)")
        && boot.contains("player_name(player_id)")
        && boot.contains("player_is_alive(player_id)")
        && boot.contains("ai_manager_contains_player(player_id)");
    let ui_ok = ui.contains("Wave 549")
        && ui.contains("frame.player_info(player_id)")
        && ui.contains("boot_player_info_from_host(player_id)")
        && !ui.contains("player_exists(player_id)");
    let dip_ok = dip.contains("Wave 573")
        && dip.contains("player_ids()")
        && dip.contains("boot_player_info_from_host(id)")
        && !dip.contains("player_name(id)");
    let raw_exists = eng.matches("self.game_logic.player_exists").count();
    // boot helper + local_player_id_for_ui residual
    let ok =
        boot_ok && ui_ok && dip_ok && raw_exists == 2 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualBootPlayerInfoHelperAction::SourceMarkers);
    ok
}

pub fn honesty_boot_player_info_helper_nav_commands_residual_wave573() -> bool {
    let steps = LIVE_BOOT_PLAYER_INFO_HELPER_NAV_STEPS_WAVE573;
    let cmds = RUNTIME_HOST_LIVE_BOOT_PLAYER_INFO_HELPER_CMD_NAMES_WAVE573;
    let ok = residual_name_index(steps, "REQUIRE_BOOT_PLAYER_INFO_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_UI_PLAYER_USES_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_DIPLOMACY_USES_HELPER").is_some()
        && residual_name_index(steps, "LIVE_BOOT_PLAYER_INFO_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "boot_player_info_helper").is_some()
        && residual_name_index(cmds, "ui_player_info_helper").is_some()
        && residual_name_index(cmds, "diplomacy_boot_helper").is_some();
    residual_action_store(ResidualBootPlayerInfoHelperAction::NavCommands);
    ok
}

pub fn simulate_boot_player_info_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 573")
        && eng.contains("fn boot_player_info_from_host")
        && eng.contains("fn ui_player_info");
    residual_action_store(ResidualBootPlayerInfoHelperAction::CollectSource);
    ok
}

pub fn simulate_boot_player_info_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.boot_player_info_from_host(player_id)")
        && eng.contains("self.boot_player_info_from_host(id)")
        && eng.contains("presentation_or_boot_diplomacy_players");
    residual_action_store(ResidualBootPlayerInfoHelperAction::DispatchSource);
    ok
}

pub fn honesty_boot_player_info_helper_residual_pack_wave573() -> bool {
    honesty_boot_player_info_helper_method_names_residual_wave573()
        && honesty_boot_player_info_helper_source_markers_residual_wave573()
        && honesty_boot_player_info_helper_nav_commands_residual_wave573()
        && simulate_boot_player_info_helper_collect_source()
        && simulate_boot_player_info_helper_dispatch_source()
}

pub fn simulate_live_boot_player_info_helper_honesty() -> bool {
    let ok = honesty_boot_player_info_helper_residual_pack_wave573();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualBootPlayerInfoHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_boot_player_info_helper_method_names_residual_wave573());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_boot_player_info_helper_source_markers_residual_wave573());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_boot_player_info_helper_nav_commands_residual_wave573());
    }

    #[test]
    fn boot_player_info_helper_sources() {
        assert!(simulate_boot_player_info_helper_collect_source());
        assert!(simulate_boot_player_info_helper_dispatch_source());
    }

    #[test]
    fn wave573_composite_pack() {
        assert!(honesty_boot_player_info_helper_residual_pack_wave573());
    }

    #[test]
    fn simulate_live_boot_player_info_helper_honesty_residual_live() {
        assert!(
            simulate_live_boot_player_info_helper_honesty(),
            "boot player info helper residual must latch"
        );
        assert!(residual_boot_player_info_helper_ok());
        assert_eq!(
            residual_boot_player_info_helper_last_action(),
            ResidualBootPlayerInfoHelperAction::Composite
        );
    }
}
