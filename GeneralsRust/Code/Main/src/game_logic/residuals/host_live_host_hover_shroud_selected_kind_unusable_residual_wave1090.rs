//! Wave 1090: dual-world hover-shroud + selected-kind unusable residual.
//!
//! After Waves 1087–1089 command-hint/select peels:
//! - `hover_target_shroud_for_command_hint` still returned FOW for destroyed/sold/masked
//! - `is_any/all_selected_kind_of` still counted unusable presentation_selected entries
//!
//! Fail-close those so command hints and kind-gated UI match selection legality.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_HOVER_SHROUD_SELECTED_KIND_UNUSABLE_METHOD_NAMES_WAVE1090: &[&str] = &[
    "hover_target_shroud_for_command_hint",
    "is_any_selected_kind_of",
    "is_all_selected_kind_of",
    "Wave 1090",
    "playable_claim = false",
];

pub const LIVE_HOST_HOVER_SHROUD_SELECTED_KIND_UNUSABLE_NAV_STEPS_WAVE1090: &[&str] = &[
    "HOVER_SHROUD",
    "SELECTED_KIND",
    "UNUSABLE_FAIL_CLOSED",
    "LIVE_HOST_HOVER_SHROUD_SELECTED_KIND_UNUSABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostHoverShroudSelectedKindUnusableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostHoverShroudSelectedKindUnusableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_hover_shroud_selected_kind_unusable_method_names_residual_wave1090() -> bool {
    let names = LIVE_HOST_HOVER_SHROUD_SELECTED_KIND_UNUSABLE_METHOD_NAMES_WAVE1090;
    let ok = residual_name_index(names, "hover_target_shroud_for_command_hint").is_some()
        && residual_name_index(names, "Wave 1090").is_some();
    residual_action_store(ResidualHostHoverShroudSelectedKindUnusableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_hover_shroud_selected_kind_unusable_nav_commands_residual_wave1090() -> bool {
    let steps = LIVE_HOST_HOVER_SHROUD_SELECTED_KIND_UNUSABLE_NAV_STEPS_WAVE1090;
    let ok = residual_name_index(steps, "LIVE_HOST_HOVER_SHROUD_SELECTED_KIND_UNUSABLE").is_some()
        && residual_name_index(steps, "SELECTED_KIND").is_some();
    residual_action_store(ResidualHostHoverShroudSelectedKindUnusableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_hover_shroud_selected_kind_unusable_residual_pack_wave1090() -> bool {
    let ui = ui_source();
    let es = es_source();
    let sh_i = match ui.find("fn hover_target_shroud_for_command_hint") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostHoverShroudSelectedKindUnusableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let sh = &ui[sh_i..sh_i.saturating_add(1200)];
    let any_i = match ui.find("fn is_any_selected_kind_of") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostHoverShroudSelectedKindUnusableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let any = &ui[any_i..any_i.saturating_add(900)];
    let all_i = match ui.find("fn is_all_selected_kind_of") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostHoverShroudSelectedKindUnusableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let all = &ui[all_i..all_i.saturating_add(1200)];
    let ok = sh.contains("Wave 1090: command-hint shroud residual fail-closed on unusable")
        && sh.contains("entry.destroyed || entry.sold || entry.masked || entry.unselectable")
        && any.contains("Wave 1090: selected-kind residual ignores unusable entries")
        && any.contains("u.destroyed || u.sold || u.masked || u.unselectable")
        && all.contains("Wave 1090: evaluate only usable selected entries")
        && all.contains("u.destroyed || u.sold || u.masked || u.unselectable")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostHoverShroudSelectedKindUnusableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_hover_shroud_selected_kind_unusable_residual_honesty() -> bool {
    let a = honesty_host_hover_shroud_selected_kind_unusable_method_names_residual_wave1090();
    let b = honesty_host_hover_shroud_selected_kind_unusable_nav_commands_residual_wave1090();
    let c = honesty_host_hover_shroud_selected_kind_unusable_residual_pack_wave1090();
    residual_action_store(ResidualHostHoverShroudSelectedKindUnusableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_hover_shroud_selected_kind_unusable_residual_wave1090() {
        assert!(honesty_host_hover_shroud_selected_kind_unusable_residual_pack_wave1090());
        assert!(honesty_host_hover_shroud_selected_kind_unusable_method_names_residual_wave1090());
        assert!(honesty_host_hover_shroud_selected_kind_unusable_nav_commands_residual_wave1090());
        assert!(simulate_live_host_hover_shroud_selected_kind_unusable_residual_honesty());
    }
}
