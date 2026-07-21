//! Control-bar beacon helpers.
//!
//! Ported from `ControlBarBeacon.cpp`.

use super::control_bar::ControlBar;
use super::ControlBarContext;
use game_engine::common::ini::ini_command_button::get_control_bar as get_ini_control_bar;
use gamelogic::object::registry::OBJECT_REGISTRY;

/// Append beacon-only commands when the current selection is a local beacon object.
///
/// Host/presentation residual: when OBJECT_REGISTRY is empty, `presentation_command_set`
/// containing "BEACON" still enables Command_BeaconDelete (Main snapshot path).
pub(super) fn append_beacon_commands(
    context: &mut ControlBarContext,
) -> Result<(), Box<dyn std::error::Error>> {
    append_beacon_commands_with_presentation(context, "")
}

/// Same as [`append_beacon_commands`], with optional presentation command-set residual.
pub(super) fn append_beacon_commands_with_presentation(
    context: &mut ControlBarContext,
    presentation_command_set: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if context.selected_objects.len() != 1 {
        return Ok(());
    }

    let is_beacon =
        if let Some(object_arc) = OBJECT_REGISTRY.get_object(context.selected_objects[0]) {
            let Ok(object) = object_arc.read() else {
                return Ok(());
            };
            if !object.is_locally_controlled() {
                return Ok(());
            }
            let command_set_name = object.get_command_set_string();
            command_set_name.to_ascii_uppercase().contains("BEACON")
        } else {
            // Host presentation residual — no dual-world registry modules.
            presentation_command_set
                .to_ascii_uppercase()
                .contains("BEACON")
        };

    if !is_beacon {
        return Ok(());
    }

    let Some(common_bar) = get_ini_control_bar() else {
        return Ok(());
    };
    if let Some(button) = common_bar.find_command_button_resolved("Command_BeaconDelete") {
        ControlBar::push_command_if_missing(context, ControlBar::command_from_definition(button));
    }

    Ok(())
}
