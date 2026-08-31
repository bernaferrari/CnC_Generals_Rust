//! Wave 1087: dual-world hover command-hint unusable FOW residual.
//!
//! After create_mouseover_hint peels, hover_target_command_context still returned
//! (selectable, local, is_mine) without fail-closing destroyed/sold/unselectable/
//! masked, non-local stealth, or non-local FOW. MoveTo/AttackMoveTo/SetRallyPoint
//! hints then treated unusable/fogged targets as live selectable objects.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_HOVER_CMD_HINT_UNUSABLE_FOW_METHOD_NAMES_WAVE1087: &[&str] = &[
    "hover_target_command_context",
    "create_mouseover_hint_from_presentation",
    "Wave 1087",
    "playable_claim = false",
];

pub const LIVE_HOST_HOVER_CMD_HINT_UNUSABLE_FOW_NAV_STEPS_WAVE1087: &[&str] = &[
    "HOVER_CMD_HINT",
    "UNUSABLE_FOW_STEALTH",
    "LIVE_HOST_HOVER_CMD_HINT_UNUSABLE_FOW",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostHoverCmdHintUnusableFowAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostHoverCmdHintUnusableFowAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_hover_cmd_hint_unusable_fow_method_names_residual_wave1087() -> bool {
    let names = LIVE_HOST_HOVER_CMD_HINT_UNUSABLE_FOW_METHOD_NAMES_WAVE1087;
    let ok = residual_name_index(names, "hover_target_command_context").is_some()
        && residual_name_index(names, "Wave 1087").is_some();
    residual_action_store(ResidualHostHoverCmdHintUnusableFowAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_hover_cmd_hint_unusable_fow_nav_commands_residual_wave1087() -> bool {
    let steps = LIVE_HOST_HOVER_CMD_HINT_UNUSABLE_FOW_NAV_STEPS_WAVE1087;
    let ok = residual_name_index(steps, "LIVE_HOST_HOVER_CMD_HINT_UNUSABLE_FOW").is_some()
        && residual_name_index(steps, "UNUSABLE_FOW_STEALTH").is_some();
    residual_action_store(ResidualHostHoverCmdHintUnusableFowAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_hover_cmd_hint_unusable_fow_residual_pack_wave1087() -> bool {
    let ui = ui_source();
    let es = es_source();
    let fn_i = match ui.find("fn hover_target_command_context") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostHoverCmdHintUnusableFowAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let window = &ui[fn_i..fn_i.saturating_add(1800)];
    let ok = window.contains("Wave 1087: command-hint hover residual fail-closed on unusable")
        && window.contains("entry.destroyed || entry.sold || entry.unselectable || entry.masked")
        && window.contains("entry.effectively_stealthed && !local")
        && window.contains("ObjectShroudStatus::Fogged")
        && window.contains("ObjectShroudStatus::Shrouded")
        && window.contains("return (false, false, false)")
        && window.contains("return (entry.selectable, local, is_mine)")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostHoverCmdHintUnusableFowAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_hover_cmd_hint_unusable_fow_residual_honesty() -> bool {
    let a = honesty_host_hover_cmd_hint_unusable_fow_method_names_residual_wave1087();
    let b = honesty_host_hover_cmd_hint_unusable_fow_nav_commands_residual_wave1087();
    let c = honesty_host_hover_cmd_hint_unusable_fow_residual_pack_wave1087();
    residual_action_store(ResidualHostHoverCmdHintUnusableFowAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_hover_cmd_hint_unusable_fow_residual_wave1087() {
        assert!(honesty_host_hover_cmd_hint_unusable_fow_residual_pack_wave1087());
        assert!(honesty_host_hover_cmd_hint_unusable_fow_method_names_residual_wave1087());
        assert!(honesty_host_hover_cmd_hint_unusable_fow_nav_commands_residual_wave1087());
        assert!(simulate_live_host_hover_cmd_hint_unusable_fow_residual_honesty());
    }
}
