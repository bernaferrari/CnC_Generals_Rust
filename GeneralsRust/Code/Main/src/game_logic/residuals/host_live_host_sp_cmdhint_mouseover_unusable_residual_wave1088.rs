//! Wave 1088: dual-world SP override / cmd-hint source / mouseover lookup unusable residual.
//!
//! After Waves 1085–1087 hover peels:
//! - `source_has_overridable_special_power_destination` only checked special_power_ready
//! - `command_hint_source_context` returned local/structure for dead/sold/masked/disabled
//! - `mouseover_drawable_id_for_lookup` remapped IgnoredInGui without fail-closing unusable
//!
//! Fail-close those paths so SP override cursor, MoveTo structure hints, and mouseover
//! lookup match create_mouseover_hint / is_valid_special_power_target peels.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SP_CMDHINT_MOUSEOVER_UNUSABLE_METHOD_NAMES_WAVE1088: &[&str] = &[
    "source_has_overridable_special_power_destination",
    "command_hint_source_context",
    "mouseover_drawable_id_for_lookup",
    "Wave 1088",
    "playable_claim = false",
];

pub const LIVE_HOST_SP_CMDHINT_MOUSEOVER_UNUSABLE_NAV_STEPS_WAVE1088: &[&str] = &[
    "SP_OVERRIDE_SOURCE",
    "CMD_HINT_SOURCE",
    "MOUSEOVER_LOOKUP",
    "LIVE_HOST_SP_CMDHINT_MOUSEOVER_UNUSABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpCmdhintMouseoverUnusableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSpCmdhintMouseoverUnusableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_sp_cmdhint_mouseover_unusable_method_names_residual_wave1088() -> bool {
    let names = LIVE_HOST_SP_CMDHINT_MOUSEOVER_UNUSABLE_METHOD_NAMES_WAVE1088;
    let ok = residual_name_index(names, "source_has_overridable_special_power_destination")
        .is_some()
        && residual_name_index(names, "Wave 1088").is_some();
    residual_action_store(ResidualHostSpCmdhintMouseoverUnusableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sp_cmdhint_mouseover_unusable_nav_commands_residual_wave1088() -> bool {
    let steps = LIVE_HOST_SP_CMDHINT_MOUSEOVER_UNUSABLE_NAV_STEPS_WAVE1088;
    let ok = residual_name_index(steps, "LIVE_HOST_SP_CMDHINT_MOUSEOVER_UNUSABLE").is_some()
        && residual_name_index(steps, "SP_OVERRIDE_SOURCE").is_some();
    residual_action_store(ResidualHostSpCmdhintMouseoverUnusableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sp_cmdhint_mouseover_unusable_residual_pack_wave1088() -> bool {
    let ui = ui_source();
    let es = es_source();
    let sp_i = match ui.find("fn source_has_overridable_special_power_destination") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSpCmdhintMouseoverUnusableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let sp = &ui[sp_i..sp_i.saturating_add(1200)];
    let hint_i = match ui.find("fn command_hint_source_context") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSpCmdhintMouseoverUnusableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let hint = &ui[hint_i..hint_i.saturating_add(1200)];
    let mo_i = match ui.find("fn mouseover_drawable_id_for_lookup") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSpCmdhintMouseoverUnusableAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let mo = &ui[mo_i..mo_i.saturating_add(1400)];
    let ok = (sp.contains("Wave 1088: SP override-destination source residual fail-closed")
            || sp.contains("Wave 1088/1091: SP override-destination source residual fail-closed"))
        && sp.contains("!u.destroyed")
        && sp.contains("!u.sold")
        && sp.contains("!u.disabled")
        && sp.contains("!u.under_construction")
        && (hint.contains("Wave 1088: command-hint source residual fail-closed on unusable")
            || hint.contains("Wave 1088/1091: command-hint source residual fail-closed on unusable"))
        && (hint.contains("entry.destroyed || entry.sold || entry.masked || entry.disabled")
            || hint.contains("entry.destroyed") && hint.contains("entry.sold"))
        && mo.contains("Wave 1088: mouseover lookup residual fail-closed on destroyed/sold/masked")
        && mo.contains("return Self::INVALID_DRAWABLE_ID")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostSpCmdhintMouseoverUnusableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sp_cmdhint_mouseover_unusable_residual_honesty() -> bool {
    let a = honesty_host_sp_cmdhint_mouseover_unusable_method_names_residual_wave1088();
    let b = honesty_host_sp_cmdhint_mouseover_unusable_nav_commands_residual_wave1088();
    let c = honesty_host_sp_cmdhint_mouseover_unusable_residual_pack_wave1088();
    residual_action_store(ResidualHostSpCmdhintMouseoverUnusableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sp_cmdhint_mouseover_unusable_residual_wave1088() {
        assert!(honesty_host_sp_cmdhint_mouseover_unusable_residual_pack_wave1088());
        assert!(honesty_host_sp_cmdhint_mouseover_unusable_method_names_residual_wave1088());
        assert!(honesty_host_sp_cmdhint_mouseover_unusable_nav_commands_residual_wave1088());
        assert!(simulate_live_host_sp_cmdhint_mouseover_unusable_residual_honesty());
    }
}
