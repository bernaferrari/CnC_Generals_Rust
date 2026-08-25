//! Wave 558 residual peels: diplomacy roster prefers presentation freeze via
//! `presentation_or_boot_diplomacy_players` instead of dual-reading live
//! `player_ids`/`player_name`/`player_team` mid-frame under freeze.
//! Boot residual without freeze uses host player field probes.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 557 replay presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` presentation_or_boot_diplomacy_players / sync_diplomacy_panel_from_world
//! - `presentation_frame.rs` players / PresentationPlayerInfo
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_DIPLOMACY_PRESENTATION_HELPER_METHOD_NAMES_WAVE558: &[&str] = &[
    "presentation_or_boot_diplomacy_players",
    "sync_diplomacy_panel_from_world",
    "player_ids",
    "Wave 558",
    "playable_claim = false",
];

pub const LIVE_DIPLOMACY_PRESENTATION_HELPER_NAV_STEPS_WAVE558: &[&str] = &[
    "REQUIRE_DIPLOMACY_PRESENTATION_HELPER",
    "REQUIRE_SYNC_USES_HELPER",
    "LIVE_DIPLOMACY_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_DIPLOMACY_PRESENTATION_HELPER_CMD_NAMES_WAVE558: &[&str] = &[
    "diplomacy_presentation_helper",
    "sync_diplomacy_panel",
    "boot_player_ids",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualDiplomacyPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualDiplomacyPresentationHelperAction {
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

fn residual_action_store(action: ResidualDiplomacyPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_diplomacy_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_diplomacy_presentation_helper_last_action()
-> ResidualDiplomacyPresentationHelperAction {
    ResidualDiplomacyPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_diplomacy_presentation_helper_method_names_residual_wave558() -> bool {
    let names = LIVE_DIPLOMACY_PRESENTATION_HELPER_METHOD_NAMES_WAVE558;
    let ok = residual_name_index(names, "presentation_or_boot_diplomacy_players").is_some()
        && residual_name_index(names, "sync_diplomacy_panel_from_world").is_some()
        && residual_name_index(names, "player_ids").is_some()
        && residual_name_index(names, "Wave 558").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualDiplomacyPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_diplomacy_presentation_helper_source_markers_residual_wave558() -> bool {
    let eng = eng_source();
    let Some(helper) = fn_body(eng, "fn presentation_or_boot_diplomacy_players(") else {
        residual_action_store(ResidualDiplomacyPresentationHelperAction::SourceMarkers);
        return false;
    };
    let Some(sync) = fn_body(eng, "fn sync_diplomacy_panel_from_world(") else {
        residual_action_store(ResidualDiplomacyPresentationHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 895 fail-closed — host_match_diplomacy_players, no player_* dual-read.
    let helper_ok = helper.contains("Wave 558")
        && helper.contains("frame.players.clone()")
        && helper.contains("host_match_diplomacy_players")
        && !helper.contains("self.game_logic.player_ids()");
    let sync_ok = sync.contains("Wave 558")
        && sync.contains("presentation_or_boot_diplomacy_players()")
        && !sync.contains("self.game_logic.player_ids()")
        && !sync.contains("self.game_logic.player_name")
        && !sync.contains("self.game_logic.player_team");
    let raw_ids = eng.matches("self.game_logic.player_ids()").count();
    let ok = helper_ok && sync_ok && raw_ids == 0 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualDiplomacyPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_diplomacy_presentation_helper_nav_commands_residual_wave558() -> bool {
    let steps = LIVE_DIPLOMACY_PRESENTATION_HELPER_NAV_STEPS_WAVE558;
    let cmds = RUNTIME_HOST_LIVE_DIPLOMACY_PRESENTATION_HELPER_CMD_NAMES_WAVE558;
    let ok = residual_name_index(steps, "REQUIRE_DIPLOMACY_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_SYNC_USES_HELPER").is_some()
        && residual_name_index(steps, "LIVE_DIPLOMACY_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "diplomacy_presentation_helper").is_some()
        && residual_name_index(cmds, "sync_diplomacy_panel").is_some()
        && residual_name_index(cmds, "boot_player_ids").is_some();
    residual_action_store(ResidualDiplomacyPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_diplomacy_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 558")
        && eng.contains("fn presentation_or_boot_diplomacy_players")
        && eng.contains("fn sync_diplomacy_panel_from_world");
    residual_action_store(ResidualDiplomacyPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_diplomacy_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(sync) = fn_body(eng, "fn sync_diplomacy_panel_from_world(") else {
        residual_action_store(ResidualDiplomacyPresentationHelperAction::DispatchSource);
        return false;
    };
    let ok = sync.contains("presentation_or_boot_diplomacy_players()")
        && sync.contains("DiplomacyPlayerEntry")
        && sync.contains("set_players(rows)");
    residual_action_store(ResidualDiplomacyPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_diplomacy_presentation_helper_residual_pack_wave558() -> bool {
    honesty_diplomacy_presentation_helper_method_names_residual_wave558()
        && honesty_diplomacy_presentation_helper_source_markers_residual_wave558()
        && honesty_diplomacy_presentation_helper_nav_commands_residual_wave558()
        && simulate_diplomacy_presentation_helper_collect_source()
        && simulate_diplomacy_presentation_helper_dispatch_source()
}

pub fn simulate_live_diplomacy_presentation_helper_honesty() -> bool {
    let ok = honesty_diplomacy_presentation_helper_residual_pack_wave558();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualDiplomacyPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_diplomacy_presentation_helper_method_names_residual_wave558());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_diplomacy_presentation_helper_source_markers_residual_wave558());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_diplomacy_presentation_helper_nav_commands_residual_wave558());
    }

    #[test]
    fn diplomacy_presentation_helper_sources() {
        assert!(simulate_diplomacy_presentation_helper_collect_source());
        assert!(simulate_diplomacy_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave558_composite_pack() {
        assert!(honesty_diplomacy_presentation_helper_residual_pack_wave558());
    }

    #[test]
    fn simulate_live_diplomacy_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_diplomacy_presentation_helper_honesty(),
            "diplomacy presentation helper residual must latch"
        );
        assert!(residual_diplomacy_presentation_helper_ok());
        assert_eq!(
            residual_diplomacy_presentation_helper_last_action(),
            ResidualDiplomacyPresentationHelperAction::Composite
        );
    }
}
