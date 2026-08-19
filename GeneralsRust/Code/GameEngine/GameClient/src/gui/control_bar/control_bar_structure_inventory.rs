//! Control-bar structure inventory helpers.
//!
//! Ported from `ControlBarStructureInventory.cpp`.

use super::control_bar::ControlBar;
use super::{CommandButton, ControlBarContext};
use game_engine::common::ini::ini_command_button::get_control_bar as get_ini_control_bar;
use gamelogic::object::registry::OBJECT_REGISTRY;

/// C++ `MAX_STRUCTURE_INVENTORY_BUTTONS` (ControlBar.h:389).
pub const MAX_STRUCTURE_INVENTORY_BUTTONS: usize = 10;
/// C++ `STOP_ID` / `EVACUATE_ID` (ControlBarStructureInventory.cpp:29-30).
pub const STRUCTURE_INVENTORY_STOP_ID: usize = 10;
pub const STRUCTURE_INVENTORY_EVACUATE_ID: usize = 11;

/// Occupant portrait assigned to an inventory exit slot.
#[derive(Debug, Clone, Default)]
pub struct StructureInventoryOccupant {
    pub object_id: u32,
    pub button_image: String,
    pub overlay_image: Option<String>,
}

/// Append inventory commands for garrison/contain structures.
///
/// C++ `populateStructureInventory` / `populateButtonProc`: one `Command_StructureExit`
/// slot per contained unit with that unit's `getButtonImage()`, veterancy overlay,
/// Evacuate/Stop enabled when count > 0.
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
    let mut occupants: Vec<StructureInventoryOccupant> = Vec::new();
    let mut used_registry = false;

    if let Some(object_arc) = OBJECT_REGISTRY.get_object(context.selected_objects[0]) {
        if let Ok(object) = object_arc.read() {
            if let Some(contain) = object.get_contain() {
                if let Ok(contain_guard) = contain.lock() {
                    if contain_guard.is_displayed_on_control_bar()
                        && contain_guard.get_max_capacity() > 0
                    {
                        max_capacity = contain_guard.get_max_capacity();
                        for &occupant_id in contain_guard.get_contained_objects() {
                            occupants.push(occupant_from_registry(occupant_id));
                        }
                        used_registry = true;
                    } else {
                        return Ok(());
                    }
                }
            }
        }
    }

    if !used_registry {
        if presentation_max_garrison == 0 {
            return Ok(());
        }
        max_capacity = presentation_max_garrison;
        let count = presentation_garrisoned_count.min(max_capacity);
        for _ in 0..count {
            occupants.push(StructureInventoryOccupant::default());
        }
    }

    if max_capacity == 0 {
        return Ok(());
    }

    let Some(common_bar) = get_ini_control_bar() else {
        return Ok(());
    };

    let exit_def = common_bar.find_command_button_resolved("Command_StructureExit");
    let evacuate_def = common_bar.find_command_button_resolved("Command_Evacuate");
    let stop_def = common_bar.find_command_button_resolved("Command_Stop");
    let exit_image = exit_def
        .map(|button| button.button_image.clone())
        .unwrap_or_default();

    let contained_count = occupants.len();
    let slot_count = max_capacity.min(MAX_STRUCTURE_INVENTORY_BUTTONS);

    // C++ hides every command window first, then rebuilds inventory slots 0..9
    // plus Stop (10) / Evacuate (11).
    context.available_commands.clear();
    context.contain_data.clear();
    context.contain_data.resize(14, None);

    for i in 0..14 {
        if i < MAX_STRUCTURE_INVENTORY_BUTTONS {
            let mut button = exit_def
                .map(ControlBar::command_from_definition)
                .unwrap_or_else(|| CommandButton {
                    command_name: STRUCTURE_INVENTORY_EXIT_COMMAND_NAME.to_string(),
                    ..CommandButton::default()
                });
            if i < slot_count {
                if let Some(occupant) = occupants.get(i) {
                    if occupant.object_id != 0 {
                        button.exit_object_id = Some(occupant.object_id);
                        context.contain_data[i] = Some(occupant.object_id);
                    }
                    if !occupant.button_image.is_empty() {
                        button.button_image = occupant.button_image.clone();
                    } else if button.button_image.is_empty() {
                        button.button_image = exit_image.clone();
                    }
                    button.overlay_image = occupant.overlay_image.clone();
                    button.button_enabled = occupant.object_id != 0 || !used_registry;
                } else {
                    button.button_enabled = false;
                    if button.button_image.is_empty() {
                        button.button_image = exit_image.clone();
                    }
                }
                button.button_hidden = false;
            } else {
                button.button_hidden = true;
                button.button_enabled = false;
            }
            context.available_commands.push(button);
        } else if i == STRUCTURE_INVENTORY_STOP_ID {
            let mut button = stop_def
                .map(ControlBar::command_from_definition)
                .unwrap_or_else(|| CommandButton {
                    command_name: STRUCTURE_INVENTORY_STOP_COMMAND_NAME.to_string(),
                    ..CommandButton::default()
                });
            button.button_enabled = contained_count > 0;
            button.button_hidden = false;
            context.available_commands.push(button);
        } else if i == STRUCTURE_INVENTORY_EVACUATE_ID {
            let mut button = evacuate_def
                .map(ControlBar::command_from_definition)
                .unwrap_or_else(|| CommandButton {
                    command_name: STRUCTURE_INVENTORY_EVACUATE_COMMAND_NAME.to_string(),
                    ..CommandButton::default()
                });
            button.button_enabled = contained_count > 0;
            button.button_hidden = false;
            context.available_commands.push(button);
        } else {
            context.available_commands.push(CommandButton {
                button_hidden: true,
                button_enabled: false,
                ..CommandButton::default()
            });
        }
    }

    context.last_recorded_inventory_count = contained_count as u32;
    RESIDUAL_SI_MAX.store(max_capacity, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_COUNT.store(contained_count, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_EXIT.store(true, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_EVACUATE.store(contained_count > 0, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_SI_STOP.store(contained_count > 0, std::sync::atomic::Ordering::Relaxed);
    residual_si_action_store(ResidualStructureInventoryAction::Populate);

    Ok(())
}

fn occupant_from_registry(occupant_id: impl Into<u32>) -> StructureInventoryOccupant {
    let object_id = occupant_id.into();
    let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
        return StructureInventoryOccupant {
            object_id,
            ..StructureInventoryOccupant::default()
        };
    };
    let Ok(object) = object_arc.read() else {
        return StructureInventoryOccupant {
            object_id,
            ..StructureInventoryOccupant::default()
        };
    };
    let template_name = object.get_template_name().to_string();
    let overlay_image = match object.get_veterancy_level() {
        gamelogic::common::types::VeterancyLevel::Veteran => Some("SSChevron1L".to_string()),
        gamelogic::common::types::VeterancyLevel::Elite => Some("SSChevron2L".to_string()),
        gamelogic::common::types::VeterancyLevel::Heroic => Some("SSChevron3L".to_string()),
        _ => None,
    };
    StructureInventoryOccupant {
        object_id,
        button_image: template_name,
        overlay_image,
    }
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
