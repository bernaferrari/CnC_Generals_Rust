//! Dozer/build/resume/cancel construction.
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
    // === Construction Commands ===

    pub(super) fn execute_build(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        orientation: f32,
    ) -> CommandResult {
        if !self.validate_build_location(location) {
            return CommandResult::InvalidLocation;
        }

        let (build_cost, is_structure) = match self.game_logic.get_templates().get(template_name) {
            Some(t) => (t.build_cost, t.is_kind_of(KindOf::Structure)),
            None => return CommandResult::InvalidCommand,
        };

        if !is_structure {
            return CommandResult::InvalidCommand;
        }

        for &unit_id in units {
            let team = match self.game_logic.host_object(unit_id) {
                Some(unit)
                    if unit.can_construct()
                        && unit.owner_player_id == Some(self.current_player_id) =>
                {
                    unit.team
                }
                Some(_) => continue,
                None => continue,
            };

            // C++ BuildAssistant CLEAR_PATH residual before charging resources.
            if !self.game_logic.is_location_legal_to_build_for_builder(
                team,
                location,
                template_name,
                Some(unit_id),
            ) {
                return CommandResult::InvalidLocation;
            }

            {
                let Some(player) = self.game_logic.get_player_mut(self.current_player_id) else {
                    continue;
                };

                if !player.spend_resources(&build_cost) {
                    return CommandResult::InvalidCommand;
                }
            }

            let building_id = self.game_logic.create_object_under_construction_for_player(
                template_name,
                self.current_player_id,
                location,
            );
            let Some(building_id) = building_id else {
                // Refund on failed placement.
                if let Some(player) = self.game_logic.get_player_mut(self.current_player_id) {
                    player.resources.supplies = player
                        .resources
                        .supplies
                        .saturating_add(build_cost.supplies);
                }
                return CommandResult::InvalidCommand;
            };
            if orientation.abs() > f32::EPSILON {
                // Wave 233: orientation stamp via GameLogic authority API.
                let _ = self
                    .game_logic
                    .unit_command_set_orientation(building_id, orientation);
            }

            let _ = self.path_to_goal_with_state(unit_id, location, AIState::Constructing);

            debug!(
                "Unit {} building {} at {:?}",
                unit_id.0, template_name, location
            );
            return CommandResult::Success;
        }
        CommandResult::InvalidCommand
    }

    pub(super) fn execute_dozer_construct(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        orientation: f32,
    ) -> CommandResult {
        self.execute_build(units, template_name, location, orientation)
    }

    pub(super) fn execute_dozer_line(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        start: Vec3,
        end: Vec3,
    ) -> CommandResult {
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let delta = end - start;
        let len = (delta.x * delta.x + delta.z * delta.z).sqrt();
        // Wall segment spacing residual (~structure footprint).
        let spacing = 20.0_f32;
        let count = if len < 1.0 {
            1usize
        } else {
            ((len / spacing).floor() as usize).saturating_add(1).min(32)
        };
        let builder = units[0];
        let mut placed = false;
        let orient = delta.z.atan2(delta.x);
        for i in 0..count {
            let t = if count <= 1 {
                0.0
            } else {
                i as f32 / (count - 1) as f32
            };
            let pos = start + delta * t;
            if self.execute_dozer_construct(&[builder], template_name, pos, orient)
                == CommandResult::Success
            {
                placed = true;
            }
        }
        if placed {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_cancel_construction(
        &mut self,
        object_id: ObjectId,
        player_id: u32,
    ) -> CommandResult {
        if let Some(obj) = self.game_logic.host_object(object_id) {
            if obj.owner_player_id != Some(player_id) {
                return CommandResult::InvalidTarget;
            }
            // C++ MSG_DOZER_CANCEL_CONSTRUCT: must be under construction, not sold.
            if !obj.status.under_construction || obj.status.sold {
                return CommandResult::InvalidCommand;
            }
            // C++: no refund when OBJECT_STATUS_RECONSTRUCTING (rebuild hole path).
            let refund = if obj.status.reconstructing {
                0
            } else {
                obj.thing.template.build_cost.supplies
            };
            if refund > 0 {
                if let Some(player) = self.game_logic.get_player_mut(player_id) {
                    player.resources.supplies = player.resources.supplies.saturating_add(refund);
                }
            }
            // C++ killing the building causes dozer cancelTask residual.
            self.game_logic.cancel_dozers_building(object_id);
            self.game_logic.destroy_object(object_id);
            debug!("Canceled construction of object {}", object_id.0);
            CommandResult::Success
        } else {
            CommandResult::InvalidTarget
        }
    }

    pub(super) fn execute_resume_construction(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // C++ MSG_RESUME_CONSTRUCTION / groupResumeConstruction residual.
        if self.game_logic.resume_construction(units, target_id) {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
}
