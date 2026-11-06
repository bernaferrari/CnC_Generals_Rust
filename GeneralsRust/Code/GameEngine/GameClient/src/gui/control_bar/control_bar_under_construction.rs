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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ResidualUnderConstructionAction {
    #[default]
    None = 0,
    Populate = 1,
    UpdatePercent = 2,
    Complete = 3,
    CancelCommand = 4,
}

/// Per-ControlBar under-construction presentation latch (was process atomics).
///
/// C++ ControlBarUnderConstruction keeps percent/cancel UI on the bar; Rust
/// owns it per client so multiple instances never share process statics.
#[derive(Debug, Clone)]
pub struct UnderConstructionLatch {
    pub last_action: ResidualUnderConstructionAction,
    pub percent: i32,
    pub completed: bool,
    pub cancel_visible: bool,
}

impl Default for UnderConstructionLatch {
    fn default() -> Self {
        Self {
            last_action: ResidualUnderConstructionAction::None,
            percent: -1,
            completed: false,
            cancel_visible: false,
        }
    }
}

/// Residual: last under-construction residual action.
pub fn residual_under_construction_last_action(
    latch: &UnderConstructionLatch,
) -> ResidualUnderConstructionAction {
    latch.last_action
}

/// Residual: displayed construction percent latch (-1 = unset).
pub fn residual_under_construction_percent(latch: &UnderConstructionLatch) -> i32 {
    latch.percent
}

/// Residual: completed latch.
pub fn residual_under_construction_is_completed(latch: &UnderConstructionLatch) -> bool {
    latch.completed
}

/// Residual: cancel command visible latch.
pub fn residual_under_construction_cancel_visible(latch: &UnderConstructionLatch) -> bool {
    latch.cancel_visible
}

/// Retail cancel construction command name residual.
pub const UNDER_CONSTRUCTION_CANCEL_COMMAND_NAME: &str = "Command_CancelConstruction";

/// Residual: format construction percent text (C++ descriptive UI residual).
pub fn format_under_construction_percent_text(object_name: &str, percent: i32) -> String {
    let p = percent.clamp(0, 100);
    format!("Under construction...\n{}\n{:3}%", object_name, p)
}

/// Residual: populate under-construction cancel command residual (no INI).
pub fn simulate_under_construction_populate(
    latch: &mut UnderConstructionLatch,
    object_name: &str,
    percent: i32,
) -> bool {
    if object_name.is_empty() {
        return false;
    }
    let p = percent.clamp(0, 100);
    latch.percent = p;
    latch.completed = false;
    latch.cancel_visible = true;
    latch.last_action = ResidualUnderConstructionAction::Populate;
    residual_under_construction_cancel_visible(latch)
        && residual_under_construction_percent(latch) == p
}

/// Residual: update construction percent residual (skip redraw when unchanged).
pub fn simulate_under_construction_update_percent(
    latch: &mut UnderConstructionLatch,
    current_percent: i32,
) -> bool {
    if residual_under_construction_is_completed(latch) {
        return false;
    }
    let p = current_percent.clamp(0, 100);
    let prev = residual_under_construction_percent(latch);
    latch.last_action = ResidualUnderConstructionAction::UpdatePercent;
    if prev == p {
        return false; // no text change — C++ shouldUpdate residual
    }
    latch.percent = p;
    true
}

/// Residual: mark construction complete residual (hide cancel).
pub fn simulate_under_construction_complete(latch: &mut UnderConstructionLatch) -> bool {
    latch.completed = true;
    latch.cancel_visible = false;
    latch.percent = 100;
    latch.last_action = ResidualUnderConstructionAction::Complete;
    residual_under_construction_is_completed(latch)
        && !residual_under_construction_cancel_visible(latch)
}

/// Residual: cancel command name residual honesty.
pub fn simulate_under_construction_cancel_command_name(
    latch: &mut UnderConstructionLatch,
) -> &'static str {
    latch.last_action = ResidualUnderConstructionAction::CancelCommand;
    UNDER_CONSTRUCTION_CANCEL_COMMAND_NAME
}

/// Residual: populate + mid-progress update composite.
pub fn simulate_under_construction_prepare_cycle(
    latch: &mut UnderConstructionLatch,
    object_name: &str,
    start_percent: i32,
    next_percent: i32,
) -> bool {
    if !simulate_under_construction_populate(latch, object_name, start_percent) {
        return false;
    }
    let _changed = simulate_under_construction_update_percent(latch, next_percent);
    residual_under_construction_cancel_visible(latch)
        && !residual_under_construction_is_completed(latch)
        && residual_under_construction_percent(latch) == next_percent.clamp(0, 100)
}
