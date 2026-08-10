//! Selection groups and command-center camera jump.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    radar_notifications::RadarKind, AIState, GameLogic, KindOf, ObjectId, ObjectType,
    PendingSpecialAbility, Resources, Team,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::common::AsciiString;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};


impl<'a> CommandExecutor<'a> {
    // === Selection Commands ===

    pub(super) fn execute_selection(
        &mut self,
        player_id: u32,
        create_new: bool,
        units: &[ObjectId],
    ) -> CommandResult {
        // Wave 232: selection last-writes via select_objects / unit_select_if_team.
        if self.game_logic.get_player(player_id).is_none() {
            return CommandResult::InvalidCommand;
        }
        if create_new {
            // Full replace (includes empty clear): deselect previous + select new.
            self.game_logic.select_objects(player_id, units.to_vec());
            return CommandResult::Success;
        }
        // Additive selection residual (shift-click style).
        let player_team = self.game_logic.get_player(player_id).map(|p| p.team);
        let Some(player_team) = player_team else {
            return CommandResult::InvalidCommand;
        };
        let mut added = Vec::new();
        for &unit_id in units {
            if self.game_logic.unit_select_if_team(unit_id, player_team) {
                added.push(unit_id);
            }
        }
        if let Some(player) = self.game_logic.get_player_mut(player_id) {
            for id in added {
                if !player.selected_objects.contains(&id) {
                    player.selected_objects.push(id);
                }
            }
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_destroy_group(&mut self, player_id: u32, team_id: u32) -> CommandResult {
        let Some(player) = self.game_logic.get_player_mut(player_id) else {
            return CommandResult::InvalidCommand;
        };

        // `DestroySelectedGroup` is used by the command stream to clear a player's current selection
        // group. The C++ pipeline ties this into the selection manager; in this simplified Main model
        // we treat it as clearing the player's selected objects.
        let _ = team_id;
        player.selected_objects.clear();
        CommandResult::Success
    }

    pub(super) fn execute_remove_from_selection(
        &mut self,
        player_id: u32,
        units: &[ObjectId],
    ) -> CommandResult {
        if let Some(player) = self.game_logic.get_player_mut(player_id) {
            for &unit_id in units {
                player.selected_objects.retain(|&id| id != unit_id);
            }
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_view_command_center(&mut self) -> CommandResult {
        let team = self.player_team(self.current_player_id);
        if let Some(position) = self.game_logic.command_center_position(team) {
            self.game_logic.request_camera_focus(position);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

}
