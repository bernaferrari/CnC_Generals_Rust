//! Control-bar under-construction helpers.
//!
//! Ported from `ControlBarUnderConstruction.cpp`.

use super::control_bar::ControlBar;
use super::ControlBarContext;
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
