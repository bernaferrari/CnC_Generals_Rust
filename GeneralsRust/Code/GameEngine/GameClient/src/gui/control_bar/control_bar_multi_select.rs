//! Control-bar multi-select helpers.
//!
//! Ported from `ControlBarMultiSelect.cpp`.

use super::control_bar::ControlBar;
use super::{CommandOption, ControlBarContext};
use game_engine::common::ini::ini_command_button::get_control_bar as get_ini_control_bar;
use gamelogic::command_button::MAX_COMMANDS_PER_SET;
use gamelogic::commands::CommandType;
use gamelogic::common::types::{KindOf, OBJECT_STATUS_SOLD};
use gamelogic::control_bar::get_control_bar_bridge;
use gamelogic::object::registry::OBJECT_REGISTRY;

/// Populate multi-select buttons by intersecting command-set names
/// (host/presentation path — no OBJECT_REGISTRY).
pub(super) fn populate_multi_select_commands_from_sets(
    context: &mut ControlBarContext,
    command_set_names: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    if command_set_names.len() < 2 {
        return Ok(());
    }
    let Some(control_bar) = get_control_bar_bridge() else {
        return Ok(());
    };
    let Some(common_bar) = get_ini_control_bar() else {
        return Ok(());
    };

    let mut common_slots: Vec<Option<gamelogic::command_button::CommandButton>> =
        vec![None; MAX_COMMANDS_PER_SET];
    let mut saw_first = false;

    for name in command_set_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let command_set = control_bar
            .find_command_set_by_name(name)
            .or_else(|| control_bar.find_command_set_by_name(&name.to_ascii_uppercase()));
        let Some(command_set) = command_set else {
            // C++ clears the shared set when a selected object has no command set.
            common_slots.fill(None);
            saw_first = true;
            break;
        };
        intersect_command_set_into_slots(&mut common_slots, &mut saw_first, &command_set);
    }

    if !saw_first {
        return Ok(());
    }
    push_common_slots(context, &common_bar, common_slots);
    Ok(())
}

/// Populate command buttons shared across all selected objects via OBJECT_REGISTRY.
///
/// Matches the original C++ behaviour:
/// - starts with commands from the first valid selected object
/// - removes slots that diverge on subsequent objects
/// - keeps `ATTACK_MOVE` if any selected unit contributes it in that slot
pub(super) fn populate_multi_select_commands(
    context: &mut ControlBarContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(control_bar) = get_control_bar_bridge() else {
        return Ok(());
    };
    let Some(common_bar) = get_ini_control_bar() else {
        return Ok(());
    };

    let mut common_slots: Vec<Option<gamelogic::command_button::CommandButton>> =
        vec![None; MAX_COMMANDS_PER_SET];
    let mut saw_first_drawable = false;

    for object_id in &context.selected_objects {
        let Some(object_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
            continue;
        };

        let Ok(object) = object_arc.read() else {
            continue;
        };

        if object.is_kind_of(KindOf::IgnoredInGui) || object.test_status(OBJECT_STATUS_SOLD) {
            continue;
        }

        let command_set_name = object.get_command_set_string().to_string();
        let command_set = control_bar
            .find_command_set_by_name(&command_set_name)
            .or_else(|| {
                control_bar.find_command_set_by_name(&command_set_name.to_ascii_uppercase())
            });

        let Some(command_set) = command_set else {
            // C++ clears the shared set when a selected object has no command set.
            common_slots.fill(None);
            saw_first_drawable = true;
            break;
        };

        intersect_command_set_into_slots(&mut common_slots, &mut saw_first_drawable, &command_set);
    }

    if !saw_first_drawable {
        return Ok(());
    }
    push_common_slots(context, &common_bar, common_slots);
    Ok(())
}

fn intersect_command_set_into_slots(
    common_slots: &mut [Option<gamelogic::command_button::CommandButton>],
    saw_first: &mut bool,
    command_set: &gamelogic::command_button::CommandSet,
) {
    if !*saw_first {
        for slot in 0..MAX_COMMANDS_PER_SET {
            let Some(button) = command_set
                .buttons
                .get(slot)
                .and_then(|button| button.as_ref())
            else {
                continue;
            };

            if (button.get_options_bits() & CommandOption::OkForMultiSelect as u32) != 0 {
                common_slots[slot] = Some(button.clone());
            }
        }
        *saw_first = true;
        return;
    }

    for slot in 0..MAX_COMMANDS_PER_SET {
        let command = command_set
            .buttons
            .get(slot)
            .and_then(|button| button.as_ref());
        let common = common_slots[slot].as_ref();

        let attack_move = command
            .map(|button| button.get_command_type() == CommandType::DoAttackMoveTo)
            .unwrap_or(false)
            || common
                .map(|button| button.get_command_type() == CommandType::DoAttackMoveTo)
                .unwrap_or(false);

        if attack_move && common_slots[slot].is_none() {
            common_slots[slot] = command.cloned();
            continue;
        }

        if attack_move {
            continue;
        }

        let matches = match (command, common) {
            (Some(a), Some(b)) => a.get_id() == b.get_id(),
            (None, None) => true,
            _ => false,
        };

        if !matches {
            common_slots[slot] = None;
        }
    }
}

fn push_common_slots(
    context: &mut ControlBarContext,
    common_bar: &game_engine::common::ini::ini_command_button::ControlBar,
    common_slots: Vec<Option<gamelogic::command_button::CommandButton>>,
) {
    for button in common_slots.into_iter().flatten() {
        if let Some(common_button) = common_bar.find_command_button_resolved(button.get_name()) {
            context
                .available_commands
                .push(ControlBar::command_from_definition(common_button));
        } else {
            context
                .available_commands
                .push(ControlBar::command_from_logic_button(&button));
        }
    }
}
