//! Capture shortcut inputs from the receiving player, content and live objects.
use super::*;
use game_client::gui::control_bar::{
    CommandAvailability, CommandOption, ReceivingShortcutEntry, ReceivingShortcutSnapshot,
};
use game_engine::common::rts::SCIENCE_INVALID;
use gamelogic::commands::command::CommandType;

pub(super) fn freeze(logic: &GameLogic, player_id: u32) -> ReceivingShortcutSnapshot {
    let Some(player) = logic.get_player(player_id) else {
        return ReceivingShortcutSnapshot::default();
    };
    let Some(template) = logic
        .player_template_definitions
        .as_ref()
        .and_then(|store| logic.player_template_identity(player_id)?.resolve_in(store))
    else {
        return ReceivingShortcutSnapshot::default();
    };
    let mut snapshot = ReceivingShortcutSnapshot {
        layout: template.get_special_power_shortcut_win_name().to_owned(),
        button_count: template.get_special_power_shortcut_button_count().max(0) as usize,
        player_active: player.is_alive && !player.is_observer,
        ..Default::default()
    };
    let Some(content) = logic.receiving_module_content.as_ref() else {
        return snapshot;
    };
    let actions = content.action_definitions();
    let objects: Vec<_> = logic
        .host_objects()
        .registered_values()
        .filter(|object| object.owner_player_id == Some(player_id))
        .filter(|object| {
            !object.status.under_construction
                && !object.status.sold
                && !object.status.effectively_dead
        })
        .collect();
    snapshot.has_any_power = objects.iter().any(|object| {
        object
            .thing()
            .template
            .special_power_modules
            .iter()
            .any(|module| module.shortcut_power && !module.scripted_special_power_only)
    });
    let set_name = template.get_special_power_shortcut_command_set();
    let Some(set) = actions.sets.find_command_set_resolved(set_name) else {
        return snapshot;
    };
    let science_facts = content.science_facts_ref();
    let has_science = |science| {
        science_facts
            .name(science)
            .is_some_and(|name| player.has_unlocked_science(name))
    };
    for slot in 0..snapshot.button_count.min(set.buttons.len()) {
        let name = logic
            .find_control_bar_override(&set.name, slot as i32)
            .unwrap_or_else(|| set.get_button_at_position(slot).map(String::as_str));
        let Some(button) = name.and_then(|name| actions.buttons.find_command_button_resolved(name))
        else {
            continue;
        };
        if button.options_bits & CommandOption::NeedUpgrade as u32 != 0
            && !button.upgrade.is_empty()
            && logic
                .upgrade_definitions
                .find_upgrade(&button.upgrade)
                .is_some()
            && !player
                .completed_upgrades
                .iter()
                .any(|upgrade| upgrade.eq_ignore_ascii_case(&button.upgrade))
        {
            continue;
        }
        let command_type =
            gamelogic::command_button::map_gui_command_to_command_type(&button.command);
        let mut entry = ReceivingShortcutEntry {
            command_name: button.name.clone(),
            availability: CommandAvailability::Available,
            ready_count: 1,
            ranked_purchase_command: None,
            selects_objects: command_type == CommandType::MetaSelectMatchingUnits,
            source_object_id: None,
        };
        if button.options_bits & CommandOption::NeedSpecialPowerScience as u32 != 0 {
            let Some(power) = button
                .special_power_template
                .as_deref()
                .and_then(|name| actions.powers.find_template(name))
            else {
                continue;
            };
            if power.required_science != SCIENCE_INVALID && !has_science(power.required_science) {
                continue;
            }
            let Some(source) = logic.shortcut_power_source(player_id, power.power_type) else {
                continue;
            };
            entry.source_object_id = Some(source.id.0);
            entry.ready_count = source.ready_count;
            entry.availability = if source.disabled || source.remaining.is_none() {
                CommandAvailability::Restricted
            } else if source.paused || source.remaining.is_some_and(|seconds| seconds > 0.0) {
                CommandAvailability::NotReady
            } else {
                CommandAvailability::Available
            };
            if power.required_science != SCIENCE_INVALID {
                if let Some(science) = button
                    .parsed_science_required
                    .iter()
                    .copied()
                    .take_while(|science| has_science(*science))
                    .last()
                {
                    let rank_names = [
                        template.get_purchase_science_command_set_rank1(),
                        template.get_purchase_science_command_set_rank3(),
                        template.get_purchase_science_command_set_rank8(),
                    ];
                    if rank_names.iter().any(|name| name.is_empty()) {
                        continue;
                    }
                    let sets: Option<Vec<_>> = rank_names
                        .iter()
                        .map(|name| actions.sets.find_command_set_resolved(name))
                        .collect();
                    let Some(sets) = sets else { continue };
                    for (rank_set, limit) in sets.into_iter().zip([
                        game_client::gui::control_bar::control_bar::MAX_PURCHASE_SCIENCE_RANK_1,
                        game_client::gui::control_bar::control_bar::MAX_PURCHASE_SCIENCE_RANK_3,
                        game_client::gui::control_bar::control_bar::MAX_PURCHASE_SCIENCE_RANK_8,
                    ]) {
                        for rank_slot in 0..limit {
                            let name = logic
                                .find_control_bar_override(&rank_set.name, rank_slot as i32)
                                .unwrap_or_else(|| {
                                    rank_set
                                        .get_button_at_position(rank_slot)
                                        .map(String::as_str)
                                });
                            let Some(purchase) = name.and_then(|name| {
                                actions.buttons.find_command_button_resolved(name)
                            }) else {
                                continue;
                            };
                            if gamelogic::command_button::map_gui_command_to_command_type(
                                &purchase.command,
                            ) == CommandType::PurchaseScience
                                && purchase.parsed_science_required.first().copied()
                                    == Some(science)
                            {
                                entry.ranked_purchase_command = Some(purchase.name.clone());
                                break;
                            }
                        }
                        if entry.ranked_purchase_command.is_some() {
                            break;
                        }
                    }
                }
            }
        } else if entry.selects_objects {
            let Some(wanted) = logic.get_templates().get(&button.object) else {
                continue;
            };
            if !objects
                .iter()
                .any(|object| object.get_template().is_equivalent_to(wanted))
            {
                continue;
            }
        }
        snapshot.entries.push(entry);
    }
    snapshot
}
