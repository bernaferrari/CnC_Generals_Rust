//! Control Bar Commands - handles command processing

use super::{CommandButton, CommandOption, CommandSourceType};
use crate::helpers::TheInGameUI;
use crate::message_stream::{get_message_stream, GameMessageType};
use gamelogic::commands::command::CommandType;

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
        _source: CommandSourceType,
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
