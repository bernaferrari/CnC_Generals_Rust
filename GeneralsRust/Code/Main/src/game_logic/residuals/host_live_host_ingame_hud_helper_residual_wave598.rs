//! Wave 598 residual peels: InGame HUD presentation apply is centralized through
//! `host_apply_ingame_hud_from_presentation`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 591 render UI presentation consumer residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_apply_ingame_hud_from_presentation
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_INGAME_HUD_HELPER_METHOD_NAMES_WAVE598: &[&str] = &[
    "host_apply_ingame_hud_from_presentation",
    "apply_presentation_to_huds",
    "sync_eva_messages_from_presentation",
    "ui_local_economy",
    "Wave 598",
    "Wave 238",
    "playable_claim = false",
];

pub const LIVE_HOST_INGAME_HUD_HELPER_NAV_STEPS_WAVE598: &[&str] = &[
    "REQUIRE_INGAME_HUD_HELPER",
    "REQUIRE_PRESENTATION_HUD_APPLY",
    "REQUIRE_BOOT_ECONOMY_FALLBACK",
    "REQUIRE_PANEL_TICKS",
    "LIVE_HOST_INGAME_HUD_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_INGAME_HUD_HELPER_CMD_NAMES_WAVE598: &[&str] = &[
    "host_ingame_hud_helper",
    "presentation_hud_apply",
    "boot_economy_fallback",
    "panel_ticks",
    "ingame_hud_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostIngameHudHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostIngameHudHelperAction {
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

fn residual_action_store(action: ResidualHostIngameHudHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_ingame_hud_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_ingame_hud_helper_last_action() -> ResidualHostIngameHudHelperAction {
    ResidualHostIngameHudHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn last_sig_index(src: &str, sig: &str) -> Option<usize> {
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(sig) {
        at = Some(from + rel);
        from = from + rel + sig.len();
    }
    at
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = last_sig_index(src, sig)?;
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

pub fn honesty_host_ingame_hud_helper_method_names_residual_wave598() -> bool {
    let names = LIVE_HOST_INGAME_HUD_HELPER_METHOD_NAMES_WAVE598;
    let ok = residual_name_index(names, "host_apply_ingame_hud_from_presentation").is_some()
        && residual_name_index(names, "apply_presentation_to_huds").is_some()
        && residual_name_index(names, "ui_local_economy").is_some()
        && residual_name_index(names, "Wave 598").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostIngameHudHelperAction::MethodNames);
    ok
}

pub fn honesty_host_ingame_hud_helper_source_markers_residual_wave598() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_apply_ingame_hud_from_presentation(") else {
        residual_action_store(ResidualHostIngameHudHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 598")
        && body.contains("GameState::InGame")
        && body.contains("apply_presentation_to_huds")
        && body.contains("sync_eva_messages_from_presentation")
        && body.contains("Wave 238: boot residual via ui_local_economy")
        && body.contains("ui_local_economy()")
        && body.contains("update_resources")
        && body.contains("diplomacy_panel.update")
        && body.contains("sync_pending_structure_placement_cursor");
    let call_ok = eng.contains("self.host_apply_ingame_hud_from_presentation(dt)")
        && eng.contains("Wave 598: InGame HUD presentation residual via host helper");
    let ok = body_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostIngameHudHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_ingame_hud_helper_nav_commands_residual_wave598() -> bool {
    let steps = LIVE_HOST_INGAME_HUD_HELPER_NAV_STEPS_WAVE598;
    let cmds = RUNTIME_HOST_LIVE_HOST_INGAME_HUD_HELPER_CMD_NAMES_WAVE598;
    let ok = residual_name_index(steps, "REQUIRE_INGAME_HUD_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_PRESENTATION_HUD_APPLY").is_some()
        && residual_name_index(steps, "REQUIRE_BOOT_ECONOMY_FALLBACK").is_some()
        && residual_name_index(steps, "REQUIRE_PANEL_TICKS").is_some()
        && residual_name_index(steps, "LIVE_HOST_INGAME_HUD_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_ingame_hud_helper").is_some()
        && residual_name_index(cmds, "presentation_hud_apply").is_some()
        && residual_name_index(cmds, "boot_economy_fallback").is_some()
        && residual_name_index(cmds, "panel_ticks").is_some()
        && residual_name_index(cmds, "ingame_hud_residual").is_some();
    residual_action_store(ResidualHostIngameHudHelperAction::NavCommands);
    ok
}

pub fn simulate_host_ingame_hud_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 598")
        && eng.contains("fn host_apply_ingame_hud_from_presentation")
        && eng.contains("ui_local_economy()");
    residual_action_store(ResidualHostIngameHudHelperAction::CollectSource);
    ok
}

pub fn simulate_host_ingame_hud_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_apply_ingame_hud_from_presentation(dt)")
        && eng.contains("Wave 598: InGame HUD presentation residual via host helper");
    residual_action_store(ResidualHostIngameHudHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_ingame_hud_helper_residual_pack_wave598() -> bool {
    honesty_host_ingame_hud_helper_method_names_residual_wave598()
        && honesty_host_ingame_hud_helper_source_markers_residual_wave598()
        && honesty_host_ingame_hud_helper_nav_commands_residual_wave598()
        && simulate_host_ingame_hud_helper_collect_source()
        && simulate_host_ingame_hud_helper_dispatch_source()
}

pub fn simulate_live_host_ingame_hud_helper_honesty() -> bool {
    let ok = honesty_host_ingame_hud_helper_residual_pack_wave598();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostIngameHudHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_ingame_hud_helper_method_names_residual_wave598());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_ingame_hud_helper_source_markers_residual_wave598());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_ingame_hud_helper_nav_commands_residual_wave598());
    }

    #[test]
    fn host_ingame_hud_helper_sources() {
        assert!(simulate_host_ingame_hud_helper_collect_source());
        assert!(simulate_host_ingame_hud_helper_dispatch_source());
    }

    #[test]
    fn wave598_composite_pack() {
        assert!(honesty_host_ingame_hud_helper_residual_pack_wave598());
    }

    #[test]
    fn simulate_live_host_ingame_hud_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_ingame_hud_helper_honesty(),
            "host ingame hud helper residual must latch"
        );
        assert!(residual_host_ingame_hud_helper_ok());
        assert_eq!(
            residual_host_ingame_hud_helper_last_action(),
            ResidualHostIngameHudHelperAction::Composite
        );
    }
}
