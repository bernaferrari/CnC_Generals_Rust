//! Control Bar Commands - handles command processing
//! C++ parity: ControlBarCommandProcessing.cpp, ControlBarCommand.cpp

use super::{CommandAvailability, CommandButton, CommandOption, CommandSourceType};
use crate::helpers::TheInGameUI;
use crate::message_stream::{get_message_stream, Coord3D as MsgCoord3D, GameMessageType};
use gamelogic::commands::command::CommandType;
use gamelogic::commands::selection::get_selection_manager;
use gamelogic::control_bar::get_control_bar_bridge;
use gamelogic::helpers::TheThingFactory;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{player_list as logic_player_list, PlayerIndex, PLAYER_INDEX_INVALID};

const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
const CMD_ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;

/// Command processor for control bar
pub struct ControlBarCommandProcessor;

impl ControlBarCommandProcessor {
    pub fn new() -> Self {
        Self
    }

    /// Process a command button click from the control bar UI.
    ///
    /// C++ parity: mirrors `ControlBar::processCommandUI()` from
    /// `ControlBarCommandProcessing.cpp`.
    pub fn process_command(
        &self,
        button: &CommandButton,
        source: CommandSourceType,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        // C++ parity: selecting a control-bar command clears any pending
        // structure placement preview before evaluating command behavior.
        TheInGameUI::place_build_available(None, None);

        if command_needs_target(button.options) {
            if (button.options & CommandOption::UsesMineClearingWeaponSet as u32) != 0 {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::SetMineClearingDetail(0));
                }
            }

            let pending_payload = if button.command_type == CommandType::FireWeapon {
                get_control_bar_bridge()
                    .and_then(|bridge| {
                        bridge
                            .find_command_button_by_name(&button.command_name)
                            .map(|logic_button| logic_button.get_weapon_slot() as u32)
                    })
                    .unwrap_or(0)
            } else {
                0
            };

            TheInGameUI::clear_pending_special_power();
            TheInGameUI::set_pending_command_with_visual(
                button.command_type,
                button.options,
                pending_payload,
                button.cursor_name.clone(),
                button.invalid_cursor_name.clone(),
                button.radius_cursor_type.clone(),
            );
            TheInGameUI::set_force_attack_mode(false);
            TheInGameUI::set_force_move_to_mode(false);
            TheInGameUI::set_prefer_selection_mode(false);

            if (button.options & CommandOption::NeedTargetEnemyObject as u32) != 0
                || (button.options & CommandOption::AttackObjectsPosition as u32) != 0
            {
                TheInGameUI::set_force_attack_mode(true);
            }
            if (button.options & CommandOption::NeedTargetPos as u32) != 0 {
                TheInGameUI::set_force_move_to_mode(true);
            }
            if (button.options
                & (CommandOption::NeedTargetAllyObject as u32
                    | CommandOption::NeedTargetNeutralObject as u32))
                != 0
            {
                TheInGameUI::set_prefer_selection_mode(true);
            }

            return Ok(true);
        }

        if button.command_type == CommandType::PurchaseScience {
            return Ok(self.process_purchase_science(button));
        }

        if button.command_type == CommandType::MetaSelectMatchingUnits {
            return Ok(self.process_select_matching_units(button));
        }

        // C++ parity: many command types (GUARD, STOP, FIRE_WEAPON, SPECIAL_POWER)
        // are handled via per-object doCommandButton.
        if self.dispatch_to_selected_objects(button, source) {
            return Ok(true);
        }

        let handled = match button.command_type {
            CommandType::MetaStop => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::MetaStop);
                }
                true
            }
            CommandType::MetaSelectNextWorker => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::MetaSelectNextWorker);
                }
                true
            }
            CommandType::MetaSelectPrevWorker => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::MetaSelectPrevWorker);
                }
                true
            }

            // C++ parity: GUI_COMMAND_DOZER_CONSTRUCT -> placeBuildAvailable()
            CommandType::DozerConstruct => self.process_dozer_construct(button),

            // C++ parity: GUI_COMMAND_DOZER_CONSTRUCT_CANCEL -> MSG_DOZER_CANCEL_CONSTRUCT
            CommandType::DozerCancelConstruct => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::DozerCancelConstruct(0));
                }
                true
            }

            // C++ parity: GUI_COMMAND_UNIT_BUILD -> MSG_QUEUE_UNIT_CREATE
            CommandType::QueueUnitCreate => self.process_unit_build(button),

            // C++ parity: GUI_COMMAND_CANCEL_UNIT_BUILD -> MSG_CANCEL_UNIT_CREATE
            CommandType::CancelUnitCreate => {
                let production_id = button
                    .purchase_cost
                    .get("production_id")
                    .copied()
                    .unwrap_or(0) as u32;
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::CancelUnitCreate(production_id));
                }
                true
            }

            // C++ parity: GUI_COMMAND_PLAYER_UPGRADE / GUI_COMMAND_OBJECT_UPGRADE -> MSG_QUEUE_UPGRADE
            CommandType::QueueUpgrade => self.process_queue_upgrade(button),

            // C++ parity: GUI_COMMAND_CANCEL_UPGRADE -> MSG_CANCEL_UPGRADE
            CommandType::CancelUpgrade => {
                let upgrade_key = button
                    .purchase_cost
                    .get("upgrade_key")
                    .copied()
                    .unwrap_or(0) as u32;
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::CancelUpgrade(upgrade_key));
                }
                true
            }

            // C++ parity: GUI_COMMAND_SELL -> MSG_SELL
            CommandType::Sell => {
                if let Ok(mut stream) = get_message_stream().write() {
                    let obj_id = selected_objects_for_local_player()
                        .first()
                        .copied()
                        .unwrap_or(0);
                    stream.append_message(GameMessageType::Sell(obj_id));
                }
                true
            }

            // C++ parity: GUI_COMMAND_EXIT_CONTAINER -> MSG_EXIT
            CommandType::Exit => {
                let obj_id = selected_objects_for_local_player()
                    .first()
                    .copied()
                    .unwrap_or(0);
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::Exit(obj_id));
                }
                true
            }

            // C++ parity: GUI_COMMAND_EVACUATE -> MSG_EVACUATE
            CommandType::Evacuate => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::Evacuate);
                }
                true
            }

            // C++ parity: GUI_COMMAND_EXECUTE_RAILED_TRANSPORT -> MSG_EXECUTE_RAILED_TRANSPORT
            CommandType::ExecuteRailedTransport => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::ExecuteRailedTransport);
                }
                true
            }

            // C++ parity: GUI_COMMAND_HACK_INTERNET -> MSG_INTERNET_HACK
            CommandType::InternetHack => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::InternetHack);
                }
                true
            }

            // C++ parity: GUI_COMMAND_TOGGLE_OVERCHARGE -> MSG_TOGGLE_OVERCHARGE
            CommandType::ToggleOvercharge => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::ToggleOvercharge);
                }
                true
            }

            // C++ parity: GUI_COMMAND_SWITCH_WEAPON -> MSG_SWITCH_WEAPONS
            CommandType::SwitchWeapons => {
                let slot = get_control_bar_bridge()
                    .and_then(|bridge| {
                        bridge
                            .find_command_button_by_name(&button.command_name)
                            .map(|logic_button| logic_button.get_weapon_slot() as u32)
                    })
                    .unwrap_or(0);
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::SwitchWeapons(slot));
                }
                true
            }

            // C++ parity: GUI_COMMAND_FIRE_WEAPON -> MSG_DO_WEAPON
            CommandType::FireWeapon => {
                let (slot, max_shots) = get_control_bar_bridge()
                    .and_then(|bridge| {
                        bridge
                            .find_command_button_by_name(&button.command_name)
                            .map(|logic_button| {
                                (
                                    logic_button.get_weapon_slot() as u32,
                                    logic_button.get_max_shots_to_fire() as u32,
                                )
                            })
                    })
                    .unwrap_or((0, i32::MAX as u32));
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::DoWeapon(slot));
                }
                let _ = max_shots;
                true
            }

            // C++ parity: GUI_COMMAND_SPECIAL_POWER -> MSG_DO_SPECIAL_POWER
            CommandType::SpecialPower => self.process_special_power(button),

            // Attack move toggle
            CommandType::DoAttackMoveTo => {
                TheInGameUI::set_pending_command(
                    CommandType::DoAttackMoveTo,
                    CMD_NEED_TARGET_POS,
                    0,
                );
                true
            }

            CommandType::DoGuardPosition => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::DoGuardPosition(
                        MsgCoord3D {
                            x: 0.0,
                            y: 0.0,
                            z: 0.0,
                        },
                        0,
                    ));
                }
                true
            }

            // C++ parity: GUI_COMMAND_STOP -> MSG_DO_STOP
            CommandType::DoStop => {
                if let Ok(mut stream) = get_message_stream().write() {
                    stream.append_message(GameMessageType::DoStop);
                }
                true
            }

            CommandType::PlaceBeacon => {
                TheInGameUI::set_pending_command(CommandType::PlaceBeacon, CMD_NEED_TARGET_POS, 0);
                true
            }
            CommandType::RemoveBeacon => {
                TheInGameUI::set_pending_command(CommandType::RemoveBeacon, CMD_NEED_TARGET_POS, 0);
                true
            }

            CommandType::SetRallyPoint => {
                TheInGameUI::set_pending_command(
                    CommandType::SetRallyPoint,
                    CMD_NEED_TARGET_POS,
                    0,
                );
                true
            }

            CommandType::AddWaypoint => {
                TheInGameUI::set_waypoint_mode(true);
                true
            }

            CommandType::CombatDropAtLocation => {
                TheInGameUI::set_pending_command(
                    CommandType::CombatDropAtLocation,
                    CMD_NEED_TARGET_POS,
                    0,
                );
                true
            }

            _ => false,
        };

        Ok(handled)
    }
}

impl Default for ControlBarCommandProcessor {
    fn default() -> Self {
        Self::new()
    }
}

// Private helper methods
impl ControlBarCommandProcessor {
    fn dispatch_to_selected_objects(
        &self,
        button: &CommandButton,
        source: CommandSourceType,
    ) -> bool {
        let Some(control_bar) = get_control_bar_bridge() else {
            return false;
        };
        let Some(logic_button) = control_bar.find_command_button_by_name(&button.command_name)
        else {
            return false;
        };
        let selected = selected_objects_for_local_player();
        if selected.is_empty() {
            return false;
        }

        let mapped_source = map_command_source(source);
        let mut sent_any = false;
        for object_id in selected {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let _ = obj.do_command_button(logic_button.get_id(), mapped_source);
            sent_any = true;
        }

        sent_any
    }

    fn process_purchase_science(&self, button: &CommandButton) -> bool {
        let Some(local_player_index) = local_player_index() else {
            return false;
        };
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(local_player_index).cloned());
        let Some(player_arc) = player_arc else {
            return false;
        };
        let Ok(player) = player_arc.read() else {
            return false;
        };
        let Some(store) = game_engine::common::rts::get_science_store() else {
            return false;
        };

        let selected_science = button.sciences_ids.iter().copied().find(|science| {
            *science != game_engine::common::rts::SCIENCE_INVALID
                && !player.has_science(*science)
                && store.player_has_prereqs_for_science(&*player, *science)
                && store.get_science_purchase_cost(*science) <= player.get_science_purchase_points()
        });

        let Some(science) = selected_science else {
            return false;
        };

        if let Ok(mut stream) = get_message_stream().write() {
            stream.append_message(GameMessageType::PurchaseScience(science as u32));
            return true;
        }
        false
    }

    fn process_select_matching_units(&self, button: &CommandButton) -> bool {
        let Some(local_player_index) = local_player_index() else {
            return false;
        };
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(local_player_index).cloned());
        let Some(player_arc) = player_arc else {
            return false;
        };

        let template_id = if !button.object.is_empty() {
            TheThingFactory::find_template(button.object.as_str()).map(|t| t.get_id())
        } else {
            None
        };
        let Some(template_id) = template_id else {
            return false;
        };

        let Ok(player) = player_arc.read() else {
            return false;
        };
        let mut matches = Vec::new();
        let _ = player.iterate_objects(|obj| {
            let guard = obj
                .read()
                .map_err(|_| gamelogic::common::GameError::LockError)?;
            if guard.get_template().get_id() == template_id {
                matches.push(guard.get_id());
            }
            Ok(())
        });

        if matches.is_empty() {
            return false;
        }

        if let Ok(mut stream) = get_message_stream().write() {
            stream.append_message(GameMessageType::CreateSelectedGroup(true, matches));
            return true;
        }
        false
    }

    // C++ parity: GUI_COMMAND_DOZER_CONSTRUCT -> placeBuildAvailable()
    fn process_dozer_construct(&self, button: &CommandButton) -> bool {
        let Some(control_bar) = get_control_bar_bridge() else {
            return false;
        };
        let Some(logic_button) = control_bar.find_command_button_by_name(&button.command_name)
        else {
            return false;
        };

        let Some(thing_template) = logic_button.get_thing_template() else {
            return false;
        };

        let Some(local_player_index) = local_player_index() else {
            return false;
        };
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(local_player_index).cloned());
        let Some(player_arc) = player_arc else {
            return false;
        };

        let Ok(player) = player_arc.read() else {
            return false;
        };
        if !player.can_build_template(thing_template.as_ref()) {
            TheInGameUI::display_cant_build_message("GUI:NotEnoughMoneyToBuild");
            return false;
        }

        TheInGameUI::place_build_available(Some(thing_template.get_name().to_string()), None);

        true
    }

    // C++ parity: GUI_COMMAND_UNIT_BUILD -> MSG_QUEUE_UNIT_CREATE
    fn process_unit_build(&self, button: &CommandButton) -> bool {
        let Some(control_bar) = get_control_bar_bridge() else {
            return false;
        };
        let Some(logic_button) = control_bar.find_command_button_by_name(&button.command_name)
        else {
            return false;
        };

        let Some(thing_template) = logic_button.get_thing_template() else {
            return false;
        };

        let template_id = thing_template.get_id();
        let production_id = selected_objects_for_local_player()
            .first()
            .copied()
            .and_then(|producer_id| OBJECT_REGISTRY.get_object(producer_id))
            .and_then(|producer| {
                producer
                    .write()
                    .ok()
                    .and_then(|mut guard| guard.request_unique_unit_production_id())
            })
            .unwrap_or(0);

        if let Ok(mut stream) = get_message_stream().write() {
            stream.append_message(GameMessageType::QueueUnitCreate(template_id, production_id));
            return true;
        }
        false
    }

    // C++ parity: GUI_COMMAND_PLAYER_UPGRADE / GUI_COMMAND_OBJECT_UPGRADE -> MSG_QUEUE_UPGRADE
    fn process_queue_upgrade(&self, button: &CommandButton) -> bool {
        let Some(control_bar) = get_control_bar_bridge() else {
            return false;
        };
        let Some(logic_button) = control_bar.find_command_button_by_name(&button.command_name)
        else {
            return false;
        };

        let Some(upgrade_template) = logic_button.get_upgrade_template() else {
            return false;
        };

        let obj_id = selected_objects_for_local_player()
            .first()
            .copied()
            .unwrap_or(0);

        let upgrade_key = upgrade_template.get_name_key() as u32;
        if let Ok(mut stream) = get_message_stream().write() {
            stream.append_message(GameMessageType::QueueUpgrade(upgrade_key));
            let _ = obj_id;
            return true;
        }
        false
    }

    // C++ parity: GUI_COMMAND_SPECIAL_POWER -> MSG_DO_SPECIAL_POWER
    fn process_special_power(&self, button: &CommandButton) -> bool {
        let Some(control_bar) = get_control_bar_bridge() else {
            return false;
        };
        let Some(logic_button) = control_bar.find_command_button_by_name(&button.command_name)
        else {
            return false;
        };

        let Some(sp_template) = logic_button.get_special_power_template() else {
            return false;
        };

        let sp_id = sp_template.get_id() as u32;
        let options = logic_button.get_options_bits();
        let source_obj_id = selected_objects_for_local_player()
            .first()
            .copied()
            .unwrap_or(0);

        if let Ok(mut stream) = get_message_stream().write() {
            stream.append_message(GameMessageType::DoSpecialPower(
                sp_id,
                options,
                source_obj_id,
            ));
            return true;
        }
        false
    }
}

// C++ parity: ControlBar::getCommandAvailability() from ControlBarCommand.cpp

pub fn get_command_availability(
    button: &CommandButton,
    obj: Option<&gamelogic::object::Object>,
) -> CommandAvailability {
    let Some(obj) = obj else {
        return CommandAvailability::Hidden;
    };

    if obj.is_disabled() {
        return CommandAvailability::Restricted;
    }

    if obj.has_single_use_command_been_used() {
        return CommandAvailability::Restricted;
    }

    if (button.options & CommandOption::MustBeStopped as u32) != 0 {
        if obj.is_moving() {
            return CommandAvailability::Restricted;
        }
    }

    if (button.options & CommandOption::NeedUpgrade as u32) != 0 {
        // Upgrade template check requires button -> template resolution
    }

    if (button.options & CommandOption::NotQueueable as u32) != 0 {
        if obj.has_production_in_queue() {
            return CommandAvailability::Restricted;
        }
    }

    match button.command_type {
        CommandType::DozerConstruct => {
            if !obj.is_kind_of(gamelogic::common::types::KindOf::Dozer) {
                return CommandAvailability::Restricted;
            }
            if obj.is_dozer_task_pending() {
                return CommandAvailability::Restricted;
            }
        }

        CommandType::Sell => {
            if obj.is_script_unsellable() {
                return CommandAvailability::Hidden;
            }
        }

        CommandType::DoGuardPosition => {
            return CommandAvailability::Available;
        }

        CommandType::Evacuate => {
            if !obj.has_contained_objects() {
                return CommandAvailability::Restricted;
            }
        }

        CommandType::DoStop => {
            return CommandAvailability::Available;
        }

        CommandType::MetaSelectMatchingUnits => {
            return CommandAvailability::Available;
        }

        _ => {}
    }

    CommandAvailability::Available
}

fn command_needs_target(options: u32) -> bool {
    let mut mask = CommandOption::NeedTargetEnemyObject as u32
        | CommandOption::NeedTargetNeutralObject as u32
        | CommandOption::NeedTargetAllyObject as u32
        | CommandOption::NeedTargetPos as u32
        | CommandOption::ContextmodeCommand as u32
        | CMD_ATTACK_OBJECTS_POSITION;
    #[cfg(feature = "allow_surrender")]
    {
        mask |= CommandOption::NeedTargetPrisoner as u32;
    }
    options & mask != 0
}

fn local_player_index() -> Option<PlayerIndex> {
    let local = logic_player_list()
        .read()
        .ok()
        .map(|list| list.get_local_player_index())?;
    if local == PLAYER_INDEX_INVALID {
        None
    } else {
        Some(local)
    }
}

fn selected_objects_for_local_player() -> Vec<u32> {
    let Some(player_index) = local_player_index() else {
        return Vec::new();
    };

    let selection_manager = get_selection_manager();
    let Ok(manager) = selection_manager.read() else {
        return Vec::new();
    };
    manager
        .get_player_selection_ref(player_index)
        .map(|selection| selection.get_selected_objects())
        .unwrap_or_default()
}

fn map_command_source(source: CommandSourceType) -> gamelogic::common::CommandSourceType {
    match source {
        CommandSourceType::FromUser => gamelogic::common::CommandSourceType::FromPlayer,
        CommandSourceType::FromScript => gamelogic::common::CommandSourceType::FromScript,
        CommandSourceType::FromAI => gamelogic::common::CommandSourceType::FromAi,
        CommandSourceType::None => gamelogic::common::CommandSourceType::FromPlayer,
    }
}
