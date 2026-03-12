//! Control-bar structure inventory helpers.
//!
//! Ported from `ControlBarStructureInventory.cpp`.

use super::control_bar::ControlBar;
use super::ControlBarContext;
use game_engine::common::ini::ini_command_button::get_control_bar as get_ini_control_bar;
use gamelogic::object::registry::OBJECT_REGISTRY;

/// Append inventory commands for garrison/contain structures.
///
/// C++ renders one button per contained unit; this pass preserves command availability parity by
/// exposing exit/evacuate/stop controls through the command list model.
pub(super) fn append_structure_inventory_commands(
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
    let Some(contain) = object.get_contain() else {
        return Ok(());
    };

    let Ok(contain_guard) = contain.lock() else {
        return Ok(());
    };
    if !contain_guard.is_displayed_on_control_bar() || contain_guard.get_max_capacity() == 0 {
        return Ok(());
    }
    let contained_count = contain_guard.get_contained_count();
    drop(contain_guard);

    let Some(common_bar) = get_ini_control_bar() else {
        return Ok(());
    };

    if let Some(button) = common_bar.find_command_button_resolved("Command_StructureExit") {
        ControlBar::push_command_if_missing(context, ControlBar::command_from_definition(button));
    }

    if contained_count > 0 {
        if let Some(button) = common_bar.find_command_button_resolved("Command_Evacuate") {
            ControlBar::push_command_if_missing(
                context,
                ControlBar::command_from_definition(button),
            );
        }
        if let Some(button) = common_bar.find_command_button_resolved("Command_Stop") {
            ControlBar::push_command_if_missing(
                context,
                ControlBar::command_from_definition(button),
            );
        }
    }

    Ok(())
}
