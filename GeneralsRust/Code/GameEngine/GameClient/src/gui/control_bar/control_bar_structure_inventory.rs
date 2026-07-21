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
    append_structure_inventory_commands_with_presentation(context, 0, 0)
}

/// Host/presentation residual: when OBJECT_REGISTRY is empty, use frozen
/// max_garrison / garrisoned_count from PresentationFrame.
pub(super) fn append_structure_inventory_commands_with_presentation(
    context: &mut ControlBarContext,
    presentation_max_garrison: usize,
    presentation_garrisoned_count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if context.selected_objects.len() != 1 {
        return Ok(());
    }

    let mut max_capacity = 0usize;
    let mut contained_count = 0usize;
    let mut used_registry = false;

    if let Some(object_arc) = OBJECT_REGISTRY.get_object(context.selected_objects[0]) {
        if let Ok(object) = object_arc.read() {
            if let Some(contain) = object.get_contain() {
                if let Ok(contain_guard) = contain.lock() {
                    if contain_guard.is_displayed_on_control_bar()
                        && contain_guard.get_max_capacity() > 0
                    {
                        max_capacity = contain_guard.get_max_capacity();
                        contained_count = contain_guard.get_contained_count();
                        used_registry = true;
                    } else {
                        // Dual-world object says not shown — do not fall back.
                        return Ok(());
                    }
                }
            }
        }
    }

    if !used_registry {
        // Host presentation residual — no dual-world contain modules.
        if presentation_max_garrison == 0 {
            return Ok(());
        }
        max_capacity = presentation_max_garrison;
        contained_count = presentation_garrisoned_count;
    }

    if max_capacity == 0 {
        return Ok(());
    }

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

    // Keep inventory count residual coherent for UI consumers.
    context.last_recorded_inventory_count = contained_count as u32;

    Ok(())
}
