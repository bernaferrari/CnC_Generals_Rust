//! Control-bar under-construction helpers.
//!
//! Ported from `ControlBarUnderConstruction.cpp`.

use super::ControlBarContext;
use super::control_bar::ControlBar;
use game_engine::common::ini::ini_command_button::get_control_bar as get_ini_control_bar;

/// Populate command buttons for an under-construction selection.
///
/// C++ shows the cancel construction command and updates descriptive UI text.
/// This Rust pass provides command parity; text/portrait updates remain in window code.
pub(super) fn populate_under_construction_commands(
    context: &mut ControlBarContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(common_bar) = get_ini_control_bar() else {
        return Ok(());
    };

    if let Some(button) = common_bar.find_command_button_resolved("Command_CancelConstruction") {
        ControlBar::push_command_if_missing(context, ControlBar::command_from_definition(button));
    }

    Ok(())
}

/// Residual: last under-construction action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualUnderConstructionAction {
    None = 0,
    Populate = 1,
    UpdatePercent = 2,
    Complete = 3,
    CancelCommand = 4,
}

static RESIDUAL_UC_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_UC_PERCENT: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);
static RESIDUAL_UC_COMPLETED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static RESIDUAL_UC_CANCEL_VISIBLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_uc_action_store(action: ResidualUnderConstructionAction) {
    RESIDUAL_UC_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last under-construction residual action.
pub fn residual_under_construction_last_action() -> ResidualUnderConstructionAction {
    match RESIDUAL_UC_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualUnderConstructionAction::Populate,
        2 => ResidualUnderConstructionAction::UpdatePercent,
        3 => ResidualUnderConstructionAction::Complete,
        4 => ResidualUnderConstructionAction::CancelCommand,
        _ => ResidualUnderConstructionAction::None,
    }
}

/// Residual: displayed construction percent latch (-1 = unset).
pub fn residual_under_construction_percent() -> i32 {
    RESIDUAL_UC_PERCENT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: completed latch.
pub fn residual_under_construction_is_completed() -> bool {
    RESIDUAL_UC_COMPLETED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: cancel command visible latch.
pub fn residual_under_construction_cancel_visible() -> bool {
    RESIDUAL_UC_CANCEL_VISIBLE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Retail cancel construction command name residual.
pub const UNDER_CONSTRUCTION_CANCEL_COMMAND_NAME: &str = "Command_CancelConstruction";

/// Residual: format construction percent text (C++ descriptive UI residual).
pub fn format_under_construction_percent_text(object_name: &str, percent: i32) -> String {
    let p = percent.clamp(0, 100);
    format!("Under construction...\n{}\n{:3}%", object_name, p)
}

/// Residual: populate under-construction cancel command residual (no INI).
pub fn simulate_under_construction_populate(object_name: &str, percent: i32) -> bool {
    if object_name.is_empty() {
        return false;
    }
    let p = percent.clamp(0, 100);
    RESIDUAL_UC_PERCENT.store(p, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_UC_COMPLETED.store(false, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_UC_CANCEL_VISIBLE.store(true, std::sync::atomic::Ordering::Relaxed);
    residual_uc_action_store(ResidualUnderConstructionAction::Populate);
    residual_under_construction_cancel_visible() && residual_under_construction_percent() == p
}

/// Residual: update construction percent residual (skip redraw when unchanged).
pub fn simulate_under_construction_update_percent(current_percent: i32) -> bool {
    if residual_under_construction_is_completed() {
        return false;
    }
    let p = current_percent.clamp(0, 100);
    let prev = residual_under_construction_percent();
    if prev == p {
        residual_uc_action_store(ResidualUnderConstructionAction::UpdatePercent);
        return false; // no text change — C++ shouldUpdate residual
    }
    RESIDUAL_UC_PERCENT.store(p, std::sync::atomic::Ordering::Relaxed);
    residual_uc_action_store(ResidualUnderConstructionAction::UpdatePercent);
    true
}

/// Residual: mark construction complete residual (hide cancel).
pub fn simulate_under_construction_complete() -> bool {
    RESIDUAL_UC_COMPLETED.store(true, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_UC_CANCEL_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_UC_PERCENT.store(100, std::sync::atomic::Ordering::Relaxed);
    residual_uc_action_store(ResidualUnderConstructionAction::Complete);
    residual_under_construction_is_completed() && !residual_under_construction_cancel_visible()
}

/// Residual: cancel command name residual honesty.
pub fn simulate_under_construction_cancel_command_name() -> &'static str {
    residual_uc_action_store(ResidualUnderConstructionAction::CancelCommand);
    UNDER_CONSTRUCTION_CANCEL_COMMAND_NAME
}

/// Residual: populate + mid-progress update composite.
pub fn simulate_under_construction_prepare_cycle(
    object_name: &str,
    start_percent: i32,
    next_percent: i32,
) -> bool {
    if !simulate_under_construction_populate(object_name, start_percent) {
        return false;
    }
    let _changed = simulate_under_construction_update_percent(next_percent);
    residual_under_construction_cancel_visible()
        && !residual_under_construction_is_completed()
        && residual_under_construction_percent() == next_percent.clamp(0, 100)
}
