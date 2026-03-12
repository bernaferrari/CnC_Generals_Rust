//! Control-bar beacon helpers.
//!
//! Ported from `ControlBarBeacon.cpp`.

use super::control_bar::ControlBar;
use super::ControlBarContext;
use game_engine::common::ini::ini_command_button::get_control_bar as get_ini_control_bar;
use gamelogic::object::registry::OBJECT_REGISTRY;

/// Append beacon-only commands when the current selection is a local beacon object.
pub(super) fn append_beacon_commands(
    context: &mut ControlBarContext,
) -> Result<(), Box<dyn std::error::Error>> {
    if context.selected_objects.len() != 1 {
        return Ok(());
    }

    let Some(object_arc) = OBJECT_REGISTRY.get_object(context.selected_objects[0]) else {
        return Ok(());
    };
    let Ok(object) = object_arc.read() else {
        return Ok(());
    };

    if !object.is_locally_controlled() {
        return Ok(());
    }

    let command_set_name = object.get_command_set_string();
    if !command_set_name.to_ascii_uppercase().contains("BEACON") {
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
