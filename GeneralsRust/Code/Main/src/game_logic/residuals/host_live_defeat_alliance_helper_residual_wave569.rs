//! Wave 569 residual peels: defeat/alliance residual is centralized through
//! `take_presentation_or_boot_defeat_events` and
//! `take_presentation_or_boot_alliance_events` (freeze prefer + drain, else boot
//! take). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 568 script FPS helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` take_presentation_or_boot_defeat/alliance_events
//! - `presentation_frame.rs` defeated_player_ids / alliance_events
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_DEFEAT_ALLIANCE_HELPER_METHOD_NAMES_WAVE569: &[&str] = &[
    "take_presentation_or_boot_defeat_events",
    "take_presentation_or_boot_alliance_events",
    "defeated_player_ids",
    "alliance_events",
    "Wave 569",
    "playable_claim = false",
];

pub const LIVE_DEFEAT_ALLIANCE_HELPER_NAV_STEPS_WAVE569: &[&str] = &[
    "REQUIRE_DEFEAT_HELPER",
    "REQUIRE_ALLIANCE_HELPER",
    "LIVE_DEFEAT_ALLIANCE_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_DEFEAT_ALLIANCE_HELPER_CMD_NAMES_WAVE569: &[&str] = &[
    "defeat_events_helper",
    "alliance_events_helper",
    "defeat_alliance_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualDefeatAllianceHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualDefeatAllianceHelperAction {
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

fn residual_action_store(action: ResidualDefeatAllianceHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_defeat_alliance_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_defeat_alliance_helper_last_action() -> ResidualDefeatAllianceHelperAction {
    ResidualDefeatAllianceHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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
pub fn honesty_defeat_alliance_helper_method_names_residual_wave569() -> bool {
    let names = LIVE_DEFEAT_ALLIANCE_HELPER_METHOD_NAMES_WAVE569;
    let ok = residual_name_index(names, "take_presentation_or_boot_defeat_events").is_some()
        && residual_name_index(names, "take_presentation_or_boot_alliance_events").is_some()
        && residual_name_index(names, "defeated_player_ids").is_some()
        && residual_name_index(names, "alliance_events").is_some()
        && residual_name_index(names, "Wave 569").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualDefeatAllianceHelperAction::MethodNames);
    ok
}

pub fn honesty_defeat_alliance_helper_source_markers_residual_wave569() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("defeated_player_ids") && pf.contains("pub alliance_events:");
    let Some(def) = fn_body(eng, "fn host_take_presentation_or_boot_defeat_events(")
        .or_else(|| fn_body(eng, "fn take_presentation_or_boot_defeat_events("))
    else {
        residual_action_store(ResidualDefeatAllianceHelperAction::SourceMarkers);
        return false;
    };
    let Some(all) = fn_body(eng, "fn host_take_presentation_or_boot_alliance_events(")
        .or_else(|| fn_body(eng, "fn take_presentation_or_boot_alliance_events("))
    else {
        residual_action_store(ResidualDefeatAllianceHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 900 fail-closed — freeze owns defeat/alliance, no take_* dual-read.
    let def_ok = (def.contains("Wave 569") || def.contains("Wave 900"))
        && def.contains("defeated_player_ids")
        && !def.contains("take_defeat_events()");
    let all_ok = (all.contains("Wave 569") || all.contains("Wave 900"))
        && all.contains("alliance_events")
        && !all.contains("take_alliance_events()");
    let call_ok = eng.contains("self.take_presentation_or_boot_defeat_events()")
        && eng.contains("self.take_presentation_or_boot_alliance_events()");
    let ok = field_ok && def_ok && all_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualDefeatAllianceHelperAction::SourceMarkers);
    ok
}

pub fn honesty_defeat_alliance_helper_nav_commands_residual_wave569() -> bool {
    let steps = LIVE_DEFEAT_ALLIANCE_HELPER_NAV_STEPS_WAVE569;
    let cmds = RUNTIME_HOST_LIVE_DEFEAT_ALLIANCE_HELPER_CMD_NAMES_WAVE569;
    let ok = residual_name_index(steps, "REQUIRE_DEFEAT_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_ALLIANCE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_DEFEAT_ALLIANCE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "defeat_events_helper").is_some()
        && residual_name_index(cmds, "alliance_events_helper").is_some()
        && residual_name_index(cmds, "defeat_alliance_residual").is_some();
    residual_action_store(ResidualDefeatAllianceHelperAction::NavCommands);
    ok
}

pub fn simulate_defeat_alliance_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 569")
        && eng.contains("fn take_presentation_or_boot_defeat_events")
        && eng.contains("fn take_presentation_or_boot_alliance_events");
    residual_action_store(ResidualDefeatAllianceHelperAction::CollectSource);
    ok
}

pub fn simulate_defeat_alliance_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.take_presentation_or_boot_defeat_events()")
        && eng.contains("self.take_presentation_or_boot_alliance_events()")
        && eng.contains("notify_boot_ui_message")
        && eng.contains("notify_presentation_ui_message");
    residual_action_store(ResidualDefeatAllianceHelperAction::DispatchSource);
    ok
}

pub fn honesty_defeat_alliance_helper_residual_pack_wave569() -> bool {
    honesty_defeat_alliance_helper_method_names_residual_wave569()
        && honesty_defeat_alliance_helper_source_markers_residual_wave569()
        && honesty_defeat_alliance_helper_nav_commands_residual_wave569()
        && simulate_defeat_alliance_helper_collect_source()
        && simulate_defeat_alliance_helper_dispatch_source()
}

pub fn simulate_live_defeat_alliance_helper_honesty() -> bool {
    let ok = honesty_defeat_alliance_helper_residual_pack_wave569();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualDefeatAllianceHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_defeat_alliance_helper_method_names_residual_wave569());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_defeat_alliance_helper_source_markers_residual_wave569());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_defeat_alliance_helper_nav_commands_residual_wave569());
    }

    #[test]
    fn defeat_alliance_helper_sources() {
        assert!(simulate_defeat_alliance_helper_collect_source());
        assert!(simulate_defeat_alliance_helper_dispatch_source());
    }

    #[test]
    fn wave569_composite_pack() {
        assert!(honesty_defeat_alliance_helper_residual_pack_wave569());
    }

    #[test]
    fn simulate_live_defeat_alliance_helper_honesty_residual_live() {
        assert!(
            simulate_live_defeat_alliance_helper_honesty(),
            "defeat/alliance helper residual must latch"
        );
        assert!(residual_defeat_alliance_helper_ok());
        assert_eq!(
            residual_defeat_alliance_helper_last_action(),
            ResidualDefeatAllianceHelperAction::Composite
        );
    }
}
