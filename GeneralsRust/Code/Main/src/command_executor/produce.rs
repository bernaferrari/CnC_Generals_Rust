//! Unit production queue and cancel.
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
    // === Production Commands ===

    pub(super) fn execute_queue_unit(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        quantity: u32,
    ) -> CommandResult {
        let mut queued = false;
        for &unit_id in units {
            for _ in 0..quantity {
                if self
                    .game_logic
                    .enqueue_production(unit_id, template_name.to_string())
                {
                    queued = true;
                }
            }
        }
        if queued {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_cancel_unit(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
    ) -> CommandResult {
        // Resolve empty name → unit production head residual (not PRODUCTION_UPGRADE).
        let resolved = if template_name.trim().is_empty() {
            units.iter().find_map(|&unit_id| {
                self.game_logic.host_object(unit_id).and_then(|obj| {
                    obj.building_data.as_ref().and_then(|b| {
                        b.production_queue
                            .iter()
                            .find(|i| !i.is_upgrade())
                            .map(|i| i.template_name.clone())
                    })
                })
            })
        } else {
            Some(template_name.to_string())
        };
        let Some(template_name) = resolved.filter(|s| !s.trim().is_empty()) else {
            return CommandResult::InvalidCommand;
        };
        let mut cancelled = false;
        for &unit_id in units {
            if self
                .game_logic
                .cancel_production(unit_id, template_name.clone())
            {
                cancelled = true;
            }
        }
        if cancelled {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
}
