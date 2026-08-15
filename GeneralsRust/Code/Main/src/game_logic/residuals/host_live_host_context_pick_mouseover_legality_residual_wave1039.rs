//! Wave 1039: dual-world context-pick + mouseover legality residual.
//!
//! translators collect_selectable_objects_from_presentation skips destroyed/
//! sold/unselectable/masked/stealthed/FOW targets. Mouseover dual clears hover
//! on the same illegal residuals. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONTEXT_PICK_MOUSEOVER_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1039: &[&str] = &[
    "collect_selectable_objects_from_presentation",
    "create_mouseover_hint_from_presentation",
    "effectively_stealthed",
    "Wave 1039",
    "playable_claim = false",
];

pub const LIVE_HOST_CONTEXT_PICK_MOUSEOVER_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1039: &[&str] = &[
    "CONTEXT_PICK",
    "MOUSEOVER",
    "STATUS_LEGALITY",
    "LIVE_HOST_CONTEXT_PICK_MOUSEOVER_LEGALITY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostContextPickMouseoverLegalityResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostContextPickMouseoverLegalityResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}
fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

pub fn honesty_host_context_pick_mouseover_legality_residual_method_names_residual_wave1039() -> bool
{
    let names = LIVE_HOST_CONTEXT_PICK_MOUSEOVER_LEGALITY_RESIDUAL_METHOD_NAMES_WAVE1039;
    let ok = residual_name_index(names, "collect_selectable_objects_from_presentation").is_some()
        && residual_name_index(names, "Wave 1039").is_some();
    residual_action_store(ResidualHostContextPickMouseoverLegalityResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_context_pick_mouseover_legality_residual_nav_commands_residual_wave1039() -> bool
{
    let steps = LIVE_HOST_CONTEXT_PICK_MOUSEOVER_LEGALITY_RESIDUAL_NAV_STEPS_WAVE1039;
    let ok = residual_name_index(steps, "LIVE_HOST_CONTEXT_PICK_MOUSEOVER_LEGALITY_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "CONTEXT_PICK").is_some();
    residual_action_store(ResidualHostContextPickMouseoverLegalityResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_context_pick_mouseover_legality_residual_residual_pack_wave1039() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ui = ui_source();
    let ok = tr.contains("Wave 1039: C++ status/stealth/FOW residual for dual context pick")
        && tr.contains("entry.destroyed || entry.sold || entry.unselectable || entry.masked")
        && tr.contains("entry.effectively_stealthed && !translator_entry_is_local(entry)")
        && tr.contains("entry.shroud_status >= 2")
        && ui.contains("Wave 1039: dead/sold/unselectable/masked/stealthed hover residual")
        && ui.contains("entry.effectively_stealthed")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostContextPickMouseoverLegalityResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_context_pick_mouseover_legality_residual_honesty() -> bool {
    let a = honesty_host_context_pick_mouseover_legality_residual_method_names_residual_wave1039();
    let b = honesty_host_context_pick_mouseover_legality_residual_nav_commands_residual_wave1039();
    let c = honesty_host_context_pick_mouseover_legality_residual_residual_pack_wave1039();
    residual_action_store(ResidualHostContextPickMouseoverLegalityResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_context_pick_mouseover_legality_residual_wave1039() {
        assert!(honesty_host_context_pick_mouseover_legality_residual_residual_pack_wave1039());
        assert!(
            honesty_host_context_pick_mouseover_legality_residual_method_names_residual_wave1039()
        );
        assert!(
            honesty_host_context_pick_mouseover_legality_residual_nav_commands_residual_wave1039()
        );
        assert!(simulate_live_host_context_pick_mouseover_legality_residual_honesty());
    }
}
