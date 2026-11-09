//! DoCommandButton and waypoint-button dispatch.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, ObjectType, PendingSpecialAbility, Resources, Team,
    radar_notifications::RadarKind,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::AsciiString;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

impl<'a> CommandExecutor<'a> {
    /// Dispatch a supplied effective definition without interpreting its name.
    /// C++ Object::doCommandButton dispatches the authored GUI command; the
    /// special-power module resolves the exact authored template on each object.
    pub(crate) fn execute_authored_command_button(
        &mut self,
        units: &[ObjectId],
        definition: &game_engine::common::ini::ini_command_button::CommandButton,
        location: Option<Vec3>,
        target: Option<ObjectId>,
    ) -> gamelogic::GameLogicResult<CommandResult> {
        let _bind = self.bind_services();
        if definition.command.eq_ignore_ascii_case("STOP") {
            return Ok(self.execute_stop(units));
        }
        // C++ Object::doCommandButton dispatches the authored GUI command,
        // independently of the special-power source used by the script query.
        if definition.command.eq_ignore_ascii_case("HIJACK_VEHICLE") {
            let Some(target) = target else {
                return Ok(CommandResult::InvalidTarget);
            };
            return self.execute_hijack(units, target);
        }
        if !definition.command.eq_ignore_ascii_case("SPECIAL_POWER") {
            return Ok(CommandResult::InvalidCommand);
        }
        let Some(template) = definition.special_power_template.as_deref() else {
            return Ok(CommandResult::InvalidCommand);
        };
        let target = target
            .map(PowerTarget::Object)
            .or_else(|| location.map(PowerTarget::Location))
            .unwrap_or(PowerTarget::None);
        let mut result = CommandResult::InvalidCommand;
        for &id in units {
            let power = self.game_logic.host_object(id).and_then(|object| {
                object
                    .thing()
                    .template
                    .special_power_modules
                    .iter()
                    .find(|module| module.special_power_template == template)
                    .and_then(|module| module.command_power.clone())
            });
            if let Some(power) = power {
                result = self.execute_special_power(&[id], &power, &target)?;
            }
        }
        Ok(result)
    }

    /// C++ AIGroup::groupDoCommandButtonUsingWaypoints residual.
    pub(crate) fn execute_do_command_button_using_waypoints(
        &mut self,
        units: &[ObjectId],
        button: &str,
        waypoints: &[Vec3],
    ) -> gamelogic::GameLogicResult<CommandResult> {
        let _bind = self.bind_services();
        use crate::command_system::{CommandType, command_type_from_button_name};

        if waypoints.is_empty() {
            return self.execute_do_command_button(units, button, None, None);
        }

        // C++ Object::doCommandButtonUsingWaypoints (Object.cpp:5737-5800):
        // require CAN_USE_WAYPOINTS; only GUI_COMMAND_SPECIAL_POWER with a
        // template acts. Unknown/AttackMove/Move/Guard must not invent a path.
        let definition = self.game_logic.command_definitions().find_command_button(button);
        let Some(definition) = definition else {
            return Ok(CommandResult::Success);
        };
        // C++ ControlBar.h: CAN_USE_WAYPOINTS = 0x00400000
        if !definition.options.can_use_waypoints
            && (definition.options_bits & 0x0040_0000) == 0
        {
            return Ok(CommandResult::Success);
        }
        let ct = command_type_from_button_name(button)
            .or_else(|| command_type_from_button_name(&definition.command));
        match ct {
            Some(CommandType::DoSpecialPower { .. })
            | Some(CommandType::DoSpecialPowerWithMetadata { .. }) => {
                // Authored special-power waypoint cast; reuse positional dispatch
                // at the final waypoint (script COMMAND_FIRED_BY_SCRIPT residual
                // is applied inside special-power admission).
                let last = *waypoints.last().unwrap();
                self.execute_do_command_button(units, button, Some(last), None)
            }
            _ => Ok(CommandResult::Success),
        }
    }

    /// C++ AIGroup::groupDoCommandButton / AtPosition / AtObject residual.
    pub(crate) fn execute_do_command_button(
        &mut self,
        units: &[ObjectId],
        button: &str,
        location: Option<Vec3>,
        target: Option<ObjectId>,
    ) -> gamelogic::GameLogicResult<CommandResult> {
        let _bind = self.bind_services();
        use crate::command_system::{
            CommandType, DropTarget, GuardTarget, ModifierKeys, PowerTarget,
            command_type_from_button_name,
        };
        use std::time::SystemTime;

        if button.trim().is_empty() {
            return Ok(CommandResult::InvalidCommand);
        }
        // CPP Object.cpp5413 and AIUpdate.cpp4826 dispatch the authored
        // Command type, independently of the button's definition name.
        // Keep the immutable catalog borrow bounded before executing commands.
        let authored_stop = self
            .game_logic
            .command_definitions()
            .find_command_button(button)
            .is_some_and(|definition| definition.command.eq_ignore_ascii_case("STOP"));
        let Some(mut ct) = (if authored_stop {
            Some(CommandType::Stop)
        } else {
            command_type_from_button_name(button)
        }) else {
            return Ok(CommandResult::InvalidCommand);
        };

        match &mut ct {
            CommandType::MoveTo { destination, .. }
            | CommandType::AttackMoveTo { destination, .. }
            | CommandType::ForceMoveTo { destination }
            | CommandType::TightenToPosition { destination }
            | CommandType::OverrideSpecialPowerDestination {
                location: destination,
            } => {
                if let Some(loc) = location {
                    *destination = loc;
                }
            }
            CommandType::SetRallyPoint { location: loc } => {
                if let Some(p) = location {
                    *loc = p;
                }
            }
            CommandType::Guard { target: gt, .. } => {
                if let Some(tid) = target {
                    *gt = GuardTarget::Object(tid);
                } else if let Some(loc) = location {
                    *gt = GuardTarget::Position(loc);
                }
            }
            CommandType::Attack { target_id }
            | CommandType::ForceAttackObject { target_id }
            | CommandType::Enter { target_id }
            | CommandType::CaptureBuilding { target_id }
            | CommandType::Hijack { target_id }
            | CommandType::Repair { target_id }
            | CommandType::GetRepaired { target_id }
            | CommandType::GetHealed { target_id }
            | CommandType::Gather { target_id }
            | CommandType::SnipeVehicle { target_id } => {
                if let Some(tid) = target {
                    *target_id = tid;
                }
            }
            CommandType::ForceAttackGround { location: loc } => {
                if let Some(p) = location {
                    *loc = p;
                }
            }
            CommandType::DoSpecialPower { target: pt, .. } => {
                if let Some(tid) = target {
                    *pt = PowerTarget::Object(tid);
                } else if let Some(loc) = location {
                    *pt = PowerTarget::Location(loc);
                }
            }
            CommandType::CombatDrop { target: dt } => {
                if let Some(tid) = target {
                    *dt = DropTarget::Object(tid);
                } else if let Some(loc) = location {
                    *dt = DropTarget::Location(loc);
                }
            }
            CommandType::FollowWaypointPath { waypoints, .. } => {
                if let Some(loc) = location {
                    if waypoints.is_empty() {
                        waypoints.push(loc);
                    }
                }
            }
            CommandType::AttackArea { center, .. } => {
                if let Some(loc) = location {
                    *center = loc;
                }
            }
            CommandType::DozerConstruct { location: loc, .. } => {
                if let Some(p) = location {
                    *loc = p;
                }
            }
            _ => {}
        }

        let cmd = crate::command_system::GameCommand {
            command_type: ct,
            player_id: self.current_player_id,
            command_id: 0,
            timestamp: SystemTime::now(),
            selected_units: units.to_vec(),
            modifier_keys: ModifierKeys::default(),
        };
        match self.execute_command(cmd) {
            Ok(r) => Ok(r),
            Err(error) => Err(gamelogic::GameLogicError::Configuration(error)),
        }
    }
}
