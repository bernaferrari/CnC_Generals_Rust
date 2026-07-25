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

/// Residual: last structure-inventory action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualStructureInventoryAction {
    None = 0,
    Populate = 1,
    Exit = 2,
    Evacuate = 3,
    Stop = 4,
    Clear = 5,
}

static RESIDUAL_SI_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_SI_MAX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static RESIDUAL_SI_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static RESIDUAL_SI_EXIT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static RESIDUAL_SI_EVACUATE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static RESIDUAL_SI_STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn residual_si_action_store(action: ResidualStructureInventoryAction) {
    RESIDUAL_SI_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last structure-inventory residual action.
pub fn residual_structure_inventory_last_action() -> ResidualStructureInventoryAction {
    match RESIDUAL_SI_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualStructureInventoryAction::Populate,
        2 => ResidualStructureInventoryAction::Exit,
        3 => ResidualStructureInventoryAction::Evacuate,
        4 => ResidualStructureInventoryAction::Stop,
        5 => ResidualStructureInventoryAction::Clear,
        _ => ResidualStructureInventoryAction::None,
    }
}

/// Residual: presentation max garrison latch.
pub fn residual_structure_inventory_max_garrison() -> usize {
    RESIDUAL_SI_MAX.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: presentation garrisoned count latch.
pub fn residual_structure_inventory_garrisoned_count() -> usize {
    RESIDUAL_SI_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: exit command visible latch.
pub fn residual_structure_inventory_exit_visible() -> bool {
    RESIDUAL_SI_EXIT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: evacuate command visible latch.
pub fn residual_structure_inventory_evacuate_visible() -> bool {
    RESIDUAL_SI_EVACUATE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: stop command visible latch.
pub fn residual_structure_inventory_stop_visible() -> bool {
    RESIDUAL_SI_STOP.load(std::sync::atomic::Ordering::Relaxed)
}

/// Retail structure inventory command names residual.
pub const STRUCTURE_INVENTORY_EXIT_COMMAND_NAME: &str = "Command_StructureExit";
pub const STRUCTURE_INVENTORY_EVACUATE_COMMAND_NAME: &str = "Command_Evacuate";
pub const STRUCTURE_INVENTORY_STOP_COMMAND_NAME: &str = "Command_Stop";

/// Residual: populate inventory from presentation counts (no OBJECT_REGISTRY).
pub fn simulate_structure_inventory_populate(max_garrison: usize, garrisoned_count: usize) -> bool {
    if max_garrison == 0 {
        return false;
    }
    let count = garrisoned_count.min(max_garrison);
    RESIDUAL_SI_MAX.store(max_garrison, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_COUNT.store(count, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_EXIT.store(true, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_EVACUATE.store(count > 0, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_STOP.store(count > 0, std::sync::atomic::Ordering::Relaxed);
    residual_si_action_store(ResidualStructureInventoryAction::Populate);
    residual_structure_inventory_exit_visible()
}

/// Residual: clear inventory residual.
pub fn simulate_structure_inventory_clear() -> bool {
    RESIDUAL_SI_MAX.store(0, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_COUNT.store(0, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_EXIT.store(false, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_EVACUATE.store(false, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_STOP.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_si_action_store(ResidualStructureInventoryAction::Clear);
    !residual_structure_inventory_exit_visible()
}

/// Residual: exit command name residual.
pub fn simulate_structure_inventory_exit_command_name() -> &'static str {
    residual_si_action_store(ResidualStructureInventoryAction::Exit);
    STRUCTURE_INVENTORY_EXIT_COMMAND_NAME
}

/// Residual: evacuate command name residual.
pub fn simulate_structure_inventory_evacuate_command_name() -> &'static str {
    residual_si_action_store(ResidualStructureInventoryAction::Evacuate);
    STRUCTURE_INVENTORY_EVACUATE_COMMAND_NAME
}

/// Residual: stop command name residual.
pub fn simulate_structure_inventory_stop_command_name() -> &'static str {
    residual_si_action_store(ResidualStructureInventoryAction::Stop);
    STRUCTURE_INVENTORY_STOP_COMMAND_NAME
}

/// Residual: populate occupied garrison composite (exit+evacuate+stop).
pub fn simulate_structure_inventory_prepare_occupied(
    max_garrison: usize,
    garrisoned_count: usize,
) -> bool {
    if garrisoned_count == 0 || max_garrison == 0 {
        return false;
    }
    if !simulate_structure_inventory_populate(max_garrison, garrisoned_count) {
        return false;
    }
    residual_structure_inventory_exit_visible()
        && residual_structure_inventory_evacuate_visible()
        && residual_structure_inventory_stop_visible()
}
