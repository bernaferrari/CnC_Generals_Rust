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
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

impl<'a> CommandExecutor<'a> {
    /// C++ AIGroup::groupDoCommandButtonUsingWaypoints residual.
    pub(crate) fn execute_do_command_button_using_waypoints(
        &mut self,
        units: &[ObjectId],
        button: &str,
        waypoints: &[Vec3],
    ) -> CommandResult {
        use crate::command_system::{CommandType, command_type_from_button_name};

        if waypoints.is_empty() {
            return self.execute_do_command_button(units, button, None, None);
        }
        let Some(ct) = command_type_from_button_name(button) else {
            // Unknown button: still follow the waypoint path.
            return self.execute_follow_waypoint_path(units, waypoints, false, true);
        };
        match ct {
            CommandType::AttackMoveTo { .. } | CommandType::ForceMoveTo { .. } => {
                // Attack-move / force-move along waypoints as a team path.
                self.execute_follow_waypoint_path(units, waypoints, false, true)
            }
            CommandType::MoveTo { .. } | CommandType::FollowWaypointPath { .. } => {
                self.execute_follow_waypoint_path(units, waypoints, false, true)
            }
            CommandType::Guard { .. } => {
                // Guard at final waypoint.
                let last = *waypoints.last().unwrap();
                self.execute_do_command_button(units, button, Some(last), None)
            }
            _ => {
                // Default: path as team, then fire button at final point.
                let last = *waypoints.last().unwrap();
                let path_res = self.execute_follow_waypoint_path(units, waypoints, false, true);
                let btn_res = self.execute_do_command_button(units, button, Some(last), None);
                if matches!(path_res, CommandResult::Success)
                    || matches!(btn_res, CommandResult::Success)
                {
                    CommandResult::Success
                } else {
                    path_res
                }
            }
        }
    }

    /// C++ AIGroup::groupDoCommandButton / AtPosition / AtObject residual.
    pub(crate) fn execute_do_command_button(
        &mut self,
        units: &[ObjectId],
        button: &str,
        location: Option<Vec3>,
        target: Option<ObjectId>,
    ) -> CommandResult {
        use crate::command_system::{
            CommandType, DropTarget, GuardTarget, ModifierKeys, PowerTarget,
            command_type_from_button_name,
        };
        use std::time::SystemTime;

        if button.trim().is_empty() {
            return CommandResult::InvalidCommand;
        }
        let Some(mut ct) = command_type_from_button_name(button) else {
            return CommandResult::InvalidCommand;
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
            Ok(r) => r,
            Err(_) => CommandResult::InvalidCommand,
        }
    }
}
