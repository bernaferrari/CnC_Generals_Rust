//! Control Bar Commands - handles command processing

use super::{CommandButton, CommandOption, CommandSourceType};
use crate::helpers::TheInGameUI;
use crate::message_stream::{get_message_stream, GameMessageType};
use gamelogic::commands::selection::get_selection_manager;
use gamelogic::commands::command::CommandType;
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

            TheInGameUI::clear_pending_special_power();
            TheInGameUI::set_pending_command(button.command_type, button.options, 0);
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
            CommandType::DoAttackMoveTo => {
                TheInGameUI::set_pending_command(
                    CommandType::DoAttackMoveTo,
                    CMD_NEED_TARGET_POS,
                    0,
                );
                true
            }
            CommandType::PlaceBeacon => {
                TheInGameUI::set_pending_command(CommandType::PlaceBeacon, CMD_NEED_TARGET_POS, 0);
                true
            }
            CommandType::RemoveBeacon => {
                TheInGameUI::set_pending_command(
                    CommandType::RemoveBeacon,
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

impl ControlBarCommandProcessor {
    fn dispatch_to_selected_objects(
        &self,
        button: &CommandButton,
        source: CommandSourceType,
    ) -> bool {
        let Some(control_bar) = get_control_bar_bridge() else {
            return false;
        };
        let Some(logic_button) = control_bar.find_command_button_by_name(&button.command_name) else {
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
