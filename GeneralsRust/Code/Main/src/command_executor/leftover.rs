//! Shared path helper plus remaining utility commands.
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
    /// C++ AIGroup::groupSurrender residual.
    pub(crate) fn execute_surrender(
        &mut self,
        units: &[ObjectId],
        surrendered: bool,
    ) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            // Wave 233: surrender via GameLogic authority API.
            if self
                .game_logic
                .unit_command_set_surrendered(unit_id, surrendered)
            {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Pathfind to `goal` then set AI state. Returns false if path assign fails.
    /// Used by Guard/Scatter/Gather/Enter/Construct so units navigate obstacles.
    pub(super) fn path_to_goal_with_state(
        &mut self,
        unit_id: ObjectId,
        goal: Vec3,
        state: AIState,
    ) -> bool {
        // Wave 233: path+AI state via GameLogic unit_command_path_with_state.
        self.game_logic
            .unit_command_path_with_state(unit_id, goal, state)
    }

    /// Deploy selected units at their current position.
    /// C&C Generals: garrisonable infantry deploy into structures,
    /// dozers unpack into construction yards, etc.
    pub(super) fn execute_deploy(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some((alive, name, is_infantry, is_deployed)) =
                self.game_logic.host_object(unit_id).map(|unit| {
                    (
                        unit.is_alive(),
                        unit.template_name.to_ascii_lowercase(),
                        unit.is_kind_of(KindOf::Infantry),
                        unit.is_deployed(),
                    )
                })
            else {
                continue;
            };
            if !alive {
                continue;
            }

            // C++ DeployStyleAIUpdate residual: toggle OBJECT_STATUS_DEPLOYED.
            let looks_deployable = [
                "tomahawk",
                "scud",
                "buggy",
                "humvee",
                "stinger",
                "crawler",
                "artillery",
                "nukecannon",
                "nuke cannon",
                "spectrum",
                "quadcannon",
                "infernocannon",
                "inferno cannon",
                "missile humvee",
                "tow",
            ]
            .iter()
            .any(|k| name.contains(k));

            if looks_deployable && !is_infantry {
                // Wave 232: deploy toggle via GameLogic unit_command_set_deployed.
                let next = !is_deployed;
                if self.game_logic.unit_command_set_deployed(unit_id, next) {
                    any = true;
                }
                continue;
            }

            // Troop crawler / transport assault deploy residual: unload occupants.
            if name.contains("transport")
                || name.contains("crawler")
                || name.contains("chinook")
                || name.contains("combatdrop")
            {
                let exit = self.execute_exit(&[unit_id]);
                if matches!(exit, CommandResult::Success) {
                    any = true;
                    continue;
                }
            }

            // Infantry residual: enter nearest garrison structure.
            if is_infantry {
                if let Some(building_id) = self.find_nearest_garrison_target(unit_id) {
                    let bpos = self
                        .game_logic
                        .host_object(building_id)
                        .map(|b| b.get_position());
                    // Wave 233: deploy-to-garrison via GameLogic authority API.
                    if self
                        .game_logic
                        .unit_command_order_enter(unit_id, building_id)
                    {
                        any = true;
                    }
                    if let Some(bpos) = bpos {
                        let _ = self.path_to_goal_with_state(unit_id, bpos, AIState::Entering);
                    }
                }
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Send worker/harvester units to gather from a resource target.
    pub(super) fn execute_gather(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let player_team = self.player_team(self.current_player_id);
        let (target_pos, target_alive, target_is_resource) =
            match self.game_logic.host_object(target_id) {
                Some(target) => (
                    target.get_position(),
                    target.is_alive(),
                    target.is_kind_of(KindOf::Harvestable)
                        || target.is_kind_of(KindOf::Resource)
                        || target.object_type == crate::game_logic::ObjectType::Supply,
                ),
                None => return CommandResult::InvalidTarget,
            };

        if !target_alive || !target_is_resource {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        for &unit_id in units {
            let can_gather = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    unit.is_alive()
                        && unit.is_worker()
                        && unit.can_move()
                        && unit.team == player_team
                })
                .unwrap_or(false);
            if !can_gather {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::Gathering) {
                any = true;
                self.accepted_gather_carrier_ids.push(unit_id);
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_return_to_base(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            let is_aircraft = unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
                || unit.object_type == crate::game_logic::ObjectType::Aircraft;
            if !is_aircraft {
                continue;
            }
            let team = unit.team;
            let pos = unit.get_position();
            // Nearest friendly airfield residual.
            // Pure residual acquire: nearest friendly airfield (3D).
            let af_cands: Vec<_> = self
                .game_logic
                .host_objects()
                .iter()
                .filter_map(|(&id, obj)| {
                    if !crate::game_logic::GameLogic::is_friendly_airfield(obj, team) {
                        return None;
                    }
                    Some(
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: obj.is_alive(),
                            is_neutral: false,
                            under_construction: obj.status.under_construction,
                            combat_kind: true,
                            effectively_stealthed: false,
                            is_air: false,
                            eject_invulnerable: false,
                        },
                    )
                })
                .collect();
            let Some((airfield_id, _, _)) =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
                    unit_id,
                    team,
                    pos,
                    af_cands,
                    |_| f32::MAX,
                    |_| true,
                )
            else {
                continue;
            };
            if self.execute_dock(&[unit_id], airfield_id) == CommandResult::Success {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_return_supplies(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            let team = unit.team;
            let pos = unit.get_position();
            let n = unit.template_name.to_ascii_lowercase();
            let is_collector = n.contains("supply")
                || n.contains("harvester")
                || n.contains("chinook")
                || (n.contains("worker") && !n.contains("dozer"))
                || matches!(
                    unit.ai_state,
                    AIState::Gathering | AIState::ReturningResources
                );
            if !is_collector {
                continue;
            }
            // Pure residual acquire: nearest friendly supply center (3D).
            let sc_cands: Vec<_> = self
                .game_logic
                .host_objects()
                .iter()
                .filter_map(|(&id, obj)| {
                    if obj.team != team || !obj.is_alive() || obj.status.under_construction {
                        return None;
                    }
                    let on = obj.template_name.to_ascii_lowercase();
                    let is_sc = obj.is_kind_of(crate::game_logic::KindOf::SupplyCenter)
                        || obj.is_kind_of(crate::game_logic::KindOf::FSSupplyCenter)
                        || on.contains("supplycenter")
                        || on.contains("supply_center")
                        || on.contains("dropzone");
                    if !is_sc {
                        return None;
                    }
                    Some(
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: true,
                            is_neutral: false,
                            under_construction: false,
                            combat_kind: true,
                            effectively_stealthed: false,
                            is_air: false,
                            eject_invulnerable: false,
                        },
                    )
                })
                .collect();
            let Some((sc_id, _, _)) =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
                    unit_id,
                    team,
                    pos,
                    sc_cands,
                    |_| f32::MAX,
                    |_| true,
                )
            else {
                continue;
            };
            let sc_pos = self
                .game_logic
                .host_object(sc_id)
                .map(|o| o.get_position())
                .unwrap_or(pos);
            // Wave 233: return-supplies via GameLogic authority API.
            let _ = self.game_logic.unit_command_return_supplies(unit_id, sc_id);
            if self.path_to_goal_with_state(unit_id, sc_pos, AIState::ReturningResources) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::setMineClearingDetail residual.
    pub(crate) fn execute_set_mine_clearing_detail(
        &mut self,
        units: &[ObjectId],
        enabled: bool,
    ) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            // Wave 233: mine-clearing detail via GameLogic authority API.
            if self
                .game_logic
                .host_object(unit_id)
                .is_some_and(|u| u.is_alive())
                && self
                    .game_logic
                    .unit_command_set_mine_clearing_detail(unit_id, enabled)
            {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupSetEmoticon residual.
    pub(crate) fn execute_set_emoticon(
        &mut self,
        units: &[ObjectId],
        name: &str,
        duration_frames: i32,
    ) -> CommandResult {
        // Wave 233: emoticon via GameLogic unit_command_set_emoticon.
        let mut any = false;
        for &unit_id in units {
            if self
                .game_logic
                .unit_command_set_emoticon(unit_id, name, duration_frames)
            {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupGoProne residual.
    pub(crate) fn execute_go_prone(&mut self, units: &[ObjectId]) -> CommandResult {
        // Wave 233: go-prone via GameLogic unit_command_go_prone.
        // Retail infantry prone window residual (~2s).
        const PRONE_SECS: f32 = 2.0;
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.unit_command_go_prone(unit_id, PRONE_SECS) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_clear_mines(&mut self, units: &[ObjectId]) -> CommandResult {
        use crate::game_logic::host_mines::{is_mine_clearer, DOZER_MINE_CLEAR_SCAN_RANGE};
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.host_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if !is_mine_clearer(
                unit.is_kind_of(crate::game_logic::KindOf::Worker),
                &unit.template_name,
            ) && !unit.is_dozer
                && !unit.template_name.to_ascii_lowercase().contains("dozer")
                && !unit.template_name.to_ascii_lowercase().contains("worker")
            {
                continue;
            }
            let team = unit.team;
            let pos = unit.get_position();
            // C++ DozerAIUpdate: setWeaponSetFlag(MINE_CLEARING_DETAIL) while clearing.
            let scan = DOZER_MINE_CLEAR_SCAN_RANGE.max(80.0);
            // Wave 233: mine-clear detail via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_set_mine_clearing_detail(unit_id, true);

            // Pure residual acquire: nearest enemy mine in clear scan range (XZ).
            let mine_cands: Vec<_> = self
                .game_logic
                .host_objects()
                .iter()
                .filter_map(|(&id, obj)| {
                    if !obj.is_alive() || obj.mine_data.is_none() || obj.team == team {
                        return None;
                    }
                    Some(
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team: obj.team,
                            position: obj.get_position(),
                            is_alive: true,
                            is_neutral: obj.team == crate::game_logic::Team::Neutral,
                            under_construction: false,
                            combat_kind: true,
                            effectively_stealthed: false,
                            is_air: false,
                            eject_invulnerable: false,
                        },
                    )
                })
                .collect();
            let Some((mine_id, _, _)) =
                crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    Some(unit_id),
                    (pos.x, pos.z),
                    mine_cands,
                    scan,
                    |_| true,
                )
            else {
                continue;
            };
            let mpos = self
                .game_logic
                .host_object(mine_id)
                .map(|o| o.get_position())
                .unwrap_or(pos);
            // Wave 233: mine order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_set_order_target(unit_id, Some(mine_id));
            if self.path_to_goal_with_state(unit_id, mpos, AIState::Moving) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Utility Commands ===

    pub(super) fn execute_repair(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // Host residual: dozer/worker repairs damaged structure over time
        // (C++ DozerAIUpdate::privateRepair → DOZER_TASK_REPAIR).
        // Fail-closed: not sole-benefactor reject / scaffolding / percent INI matrix.
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_is_damaged,
            target_under_construction,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.health.current + 0.01 < target.health.maximum,
                target.status.under_construction,
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !target_alive || !target_is_structure || !target_is_damaged || target_under_construction
        {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        for &unit_id in units {
            let can = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    unit.can_repair() && (unit.team == target_team || target_team == Team::Neutral)
                })
                .unwrap_or(false);
            if !can {
                continue;
            }
            // Wave 233: order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_set_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::Repairing) {
                any = true;
            }
        }
        if any {
            self.game_logic.record_structure_repair_residual_command();
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_get_repaired(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // Host residual: damaged vehicle → RepairPad or WarFactory (China RepairDock);
        // aircraft → Airfield. Fail-closed: not full dock bones / TimeForFullHeal matrix.
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_under_construction,
            target_building_type,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.status.under_construction,
                target
                    .building_data
                    .as_ref()
                    .map(|b| b.building_type)
                    .unwrap_or(crate::game_logic::BuildingType::CommandCenter),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !target_alive || !target_is_structure || target_under_construction {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        for &unit_id in units {
            let can = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    let is_damaged = unit.health.current + 0.01 < unit.health.maximum;
                    let is_aircraft = unit.is_kind_of(KindOf::Aircraft);
                    let is_vehicle = unit.is_kind_of(KindOf::Vehicle);
                    let supports_unit = if is_aircraft {
                        crate::game_logic::host_repair::building_provides_aircraft_repair(
                            target_building_type,
                        )
                    } else if is_vehicle {
                        crate::game_logic::host_repair::building_provides_vehicle_repair(
                            target_building_type,
                        )
                    } else {
                        false
                    };
                    unit.team == target_team
                        && unit.is_alive()
                        && unit.can_move()
                        && is_damaged
                        && supports_unit
                })
                .unwrap_or(false);
            if can {
                // Wave 233: order-target via GameLogic authority API.
                let _ = self
                    .game_logic
                    .unit_command_set_order_target(unit_id, Some(target_id));
                if self.path_to_goal_with_state(unit_id, target_pos, AIState::SeekingRepair) {
                    any = true;
                }
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_get_healed(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_under_construction,
            target_building_type,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.status.under_construction,
                target
                    .building_data
                    .as_ref()
                    .map(|b| b.building_type)
                    .unwrap_or(crate::game_logic::BuildingType::CommandCenter),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !target_alive
            || !target_is_structure
            || target_under_construction
            || target_building_type != crate::game_logic::BuildingType::HealPad
        {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        for &unit_id in units {
            let can = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    let is_injured = unit.health.current + 0.01 < unit.health.maximum;
                    unit.team == target_team
                        && unit.is_alive()
                        && unit.can_move()
                        && is_injured
                        && unit.is_kind_of(KindOf::Infantry)
                })
                .unwrap_or(false);
            if can {
                // Wave 233: order-target via GameLogic authority API.
                let _ = self
                    .game_logic
                    .unit_command_set_order_target(unit_id, Some(target_id));
                if self.path_to_goal_with_state(unit_id, target_pos, AIState::SeekingHealing) {
                    any = true;
                }
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_set_rally_point(
        &mut self,
        units: &[ObjectId],
        location: Vec3,
    ) -> CommandResult {
        let mut applied = false;
        for &unit_id in units {
            // Wave 233: rally point via GameLogic authority API.
            if self
                .game_logic
                .unit_command_set_rally_point(unit_id, location)
            {
                applied = true;
            }
        }
        if applied {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(crate) fn execute_cheer(&mut self, units: &[ObjectId]) -> CommandResult {
        // Wave 232: cheer last-writes via GameLogic unit_command_cheer.
        // C++ AIGroup::groupCheer:
        // setSpecialModelConditionState(SPECIAL_CHEERING, LOGICFRAMES_PER_SECOND * 3)
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        let cheer_secs = 3.0; // 30 logic frames @ 30Hz
        let cheer_bit = model_condition_bit_name_index("SPECIAL_CHEERING");
        let mut any = false;
        for &unit_id in units {
            if self
                .game_logic
                .unit_command_cheer(unit_id, cheer_secs, cheer_bit)
            {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Beacon Commands ===

    pub(super) fn execute_place_beacon(
        &mut self,
        player_id: u32,
        location: Vec3,
        text: &str,
    ) -> CommandResult {
        let mut manager = match get_beacon_manager().lock() {
            Ok(lock) => lock,
            Err(_) => {
                warn!("Failed to acquire beacon manager lock");
                return CommandResult::InvalidCommand;
            }
        };

        let coord = LogicCoord3D::new(location.x, location.y, location.z);
        manager.place_beacon(player_id as i32, coord, current_frame());
        if !text.is_empty() {
            manager.set_beacon_text(player_id as i32, &coord, AsciiString::from(text));
        }

        // Notify radar/UI immediately so the player sees feedback for the beacon.
        let alert = localization::localize("hud.beacon.placed", "Beacon placed");
        self.game_logic
            .queue_radar_message_at(alert, location, RadarKind::Generic);
        self.game_logic
            .queue_audio_event(AudioEventRequest::new(translate_audio_event(
                "Beacon_Placed",
            )));
        // C++ EVA_BeaconDetected when local is ALLIES with placer.
        self.game_logic.try_eva_beacon_detected(player_id);
        // Wave 210: host recent_beacons last-write for presentation freeze
        // (new_beacons / HUD bloom) without mid-frame manager dual-read.
        self.game_logic.note_beacon_placed(location);

        CommandResult::Success
    }

    pub(super) fn execute_remove_beacon(&mut self, player_id: u32) -> CommandResult {
        let mut manager = match get_beacon_manager().lock() {
            Ok(lock) => lock,
            Err(_) => {
                warn!("Failed to acquire beacon manager lock");
                return CommandResult::InvalidCommand;
            }
        };

        if manager.remove_latest_beacon(player_id as i32) {
            let alert = localization::localize("hud.beacon.removed", "Beacon removed");
            self.game_logic.queue_radar_message(alert);
            self.game_logic
                .queue_audio_event(AudioEventRequest::new(translate_audio_event(
                    "Beacon_Removed",
                )));
            // Wave 211: keep host beacon list in sync for presentation freeze.
            self.game_logic.note_beacon_removed_latest();
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
}
