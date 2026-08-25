//! Radar camera jumps and invalid-command helper.
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
    /// Direct camera jump to requested radar location (e.g., from replay/HUD ping).
    pub(super) fn execute_view_radar_at(&mut self, position: Vec3) -> CommandResult {
        self.game_logic.request_player_camera_look_at(position);
        CommandResult::Success
    }

    /// Mirror CommandSystem routing: request camera snap to last radar event.
    pub(super) fn execute_view_last_radar_event(&mut self) -> CommandResult {
        if let Some(position) = crate::game_logic::host_radar::last_the_radar_event_host_position()
        {
            self.game_logic.request_player_camera_look_at(position);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_invalid_command(&self) -> CommandResult {
        warn!("Invalid command type received");
        CommandResult::InvalidCommand
    }
}
