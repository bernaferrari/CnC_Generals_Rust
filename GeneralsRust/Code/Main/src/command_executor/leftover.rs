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

    /// Begin one C++ support order after its caller has validated the concrete
    /// repair/heal provider and relationship.  The support-state machine heals
    /// at `HOST_REPAIR_INTERACT_RANGE`, so an already-in-range unit must enter
    /// the requested state directly.  Requiring A* to the centre of a repair
    /// pad in that case can reject a legal order because the pad's own static
    /// footprint occupies the goal cell.
    ///
    /// Outside that physical range we deliberately retain the normal
    /// fail-closed path allocation; this is not a straight-line bypass for an
    /// unreachable support target.
    fn begin_support_order(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
        target_pos: Vec3,
        state: AIState,
    ) -> bool {
        let Some((source_pos, target_selection_radius)) = self
            .game_logic
            .host_object(unit_id)
            .zip(self.game_logic.host_object(target_id))
            .map(|(unit, target)| (unit.get_position(), target.selection_radius))
        else {
            return false;
        };
        let in_interaction_range = source_pos.distance(target_pos)
            <= crate::game_logic::host_repair::HOST_REPAIR_INTERACT_RANGE;

        if !self
            .game_logic
            .unit_command_set_order_target(unit_id, Some(target_id))
        {
            return false;
        }

        if in_interaction_range {
            self.game_logic.unit_command_set_ai_state(unit_id, state)
        } else {
            // C++ DozerAIUpdate does not hand A* the centre of a structure;
            // it seeds a position on the source-facing side and finds a
            // viable dock/repair point there.  This keeps the normal
            // fail-closed path allocation while avoiding the target's own
            // static footprint as an impossible A* endpoint.
            let approach = crate::game_logic::host_repair::support_approach_position(
                source_pos,
                target_pos,
                target_selection_radius,
            );
            self.path_to_goal_with_state(unit_id, approach, state)
        }
    }

    /// Deploy selected units at their current position.
    /// C&C Generals: garrisonable infantry deploy into structures,
    /// dozers unpack into construction yards, etc.
    pub(super) fn execute_deploy(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some((alive, name, is_infantry, has_deploy_style_metadata)) =
                self.game_logic.host_object(unit_id).map(|unit| {
                    (
                        unit.is_alive(),
                        unit.template_name.to_ascii_lowercase(),
                        unit.is_kind_of(KindOf::Infantry),
                        unit.get_template().deploy_style_metadata.is_some(),
                    )
                })
            else {
                continue;
            };
            if !alive {
                continue;
            }

            // C++ DeployStyleAIUpdate is a concrete Object INI behavior, not
            // a list of vehicle basenames.  Recheck it in GameLogic before
            // transitioning so a stale/injected UI command cannot bypass the
            // template's authored module data.
            if has_deploy_style_metadata && !is_infantry {
                if self.game_logic.unit_command_toggle_deploy_style(unit_id) {
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

    /// Send C++ `KINDOF_HARVESTER` units to gather from a resource target.
    pub(super) fn execute_gather(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
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
                        && unit.is_resource_collector()
                        && unit.can_move()
                        && unit.owner_player_id == Some(self.current_player_id)
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
            // Freeze the command player at the physical UI boundary.  The
            // GameLogic authority rechecks that exact owner and performs the
            // C++ producer-first ParkingPlace reservation before it mutates
            // landing state; no team/name airfield search or generic Dock is
            // allowed on this route.
            if self
                .game_logic
                .request_return_to_base(unit_id, self.current_player_id)
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
            // C++ WorkerAIUpdate.cpp:1043-1050: drop carried boxes when clearing mines.
            self.game_logic
                .drop_worker_supply_boxes_for_mine_clear(unit_id);
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
        // C++ ActionManager::canRepairObject + DozerAIUpdate::privateRepair.
        let (
            target_pos,
            target_alive,
            target_is_structure,
            target_is_damaged,
            target_under_construction,
            target_is_rebuild_hole,
            target_is_bridge,
            sole_benefactor,
            sole_expires,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.get_position(),
                target.is_alive() && !target.status.effectively_dead,
                target.is_kind_of(KindOf::Structure),
                target.health.current + 0.01 < target.health.maximum,
                target.status.under_construction,
                target.is_rebuild_hole,
                host_object_is_bridge_or_tower(target),
                target.sole_healing_benefactor,
                target.sole_healing_benefactor_expiration_frame,
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !target_alive
            || !target_is_structure
            || !target_is_damaged
            || target_under_construction
            || target_is_rebuild_hole
            || target_is_bridge
        {
            return CommandResult::InvalidTarget;
        }

        let now = self.game_logic.frame;
        let mut any = false;
        for &unit_id in units {
            let can = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    if !unit.can_repair()
                        || unit.contained_by.is_some()
                        || unit.status.under_construction
                    {
                        return false;
                    }
                    if !self
                        .game_logic
                        .repair_relationship_is_not_enemy(unit_id, target_id)
                    {
                        return false;
                    }
                    // C++ isObjectShroudedForAction — fogged/black targets reject.
                    // Fail-open when the shroud grid is uninitialized.
                    let player_id = unit.owner_player_id.unwrap_or(0);
                    if !self
                        .game_logic
                        .is_build_location_shroud_clear(player_id, target_pos)
                    {
                        return false;
                    }
                    true
                })
                .unwrap_or(false);
            if !can {
                continue;
            }
            // C++ privateRepair / InGameUI sole-benefactor gate.
            if let Some(ben) = sole_benefactor {
                if ben != unit_id && sole_expires > now {
                    continue;
                }
            }
            self.game_logic.dozer_new_task_repair(unit_id, target_id);
            self.game_logic.worker_exit_supply_for_dozer_task(unit_id);
            if self.begin_support_order(unit_id, target_id, target_pos, AIState::Repairing) {
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
        // C++ ActionManager::canGetRepairedAt: vehicle → authored
        // KINDOF_REPAIR_PAD, aircraft → authored KINDOF_FS_AIRFIELD.  The
        // host BuildingType is name-derived and cannot authorize this path.
        let (
            target_pos,
            target_alive,
            target_under_construction,
            target_sold,
            target_contained,
            target_is_repair_pad,
            target_is_fs_airfield,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.get_position(),
                target.is_alive(),
                target.status.under_construction,
                target.status.sold,
                target.contained_by.is_some(),
                target.is_kind_of(KindOf::RepairPad),
                target.is_kind_of(KindOf::FSAirfield),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !target_alive || target_under_construction || target_sold || target_contained {
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
                    let supports_unit = is_vehicle
                        && if is_aircraft {
                            target_is_fs_airfield
                        } else {
                            target_is_repair_pad
                        };
                    self.game_logic
                        .service_relationship_is_allies(unit_id, target_id)
                        && unit.is_alive()
                        && unit.can_move()
                        && !unit.status.under_construction
                        && unit.contained_by.is_none()
                        && is_damaged
                        && supports_unit
                        // C++ ActionManager::canGetRepairedAt accepts an
                        // aircraft only while it is above terrain.  A live
                        // airborne flag or sourced terrain height is required;
                        // an unknown ground sample cannot fabricate a landing
                        // request.
                        && (!is_aircraft
                            || unit.status.airborne_target
                            || (unit.ground_height_from_terrain
                                && unit.get_position().y > unit.ground_height + 0.01))
                })
                .unwrap_or(false);
            if can {
                let is_aircraft = self
                    .game_logic
                    .host_object(unit_id)
                    .is_some_and(|u| u.is_kind_of(KindOf::Aircraft));
                if is_aircraft
                    && self
                        .game_logic
                        .try_jet_enter_or_repair_airfield(unit_id, target_id)
                {
                    any = true;
                    continue;
                }
                if self.begin_support_order(unit_id, target_id, target_pos, AIState::SeekingRepair)
                {
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
            target_pos,
            target_alive,
            target_under_construction,
            target_sold,
            target_contained,
            target_is_heal_pad,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.get_position(),
                target.is_alive(),
                target.status.under_construction,
                target.status.sold,
                target.contained_by.is_some(),
                target.is_kind_of(KindOf::HealPad),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !target_alive
            || target_under_construction
            || target_sold
            || target_contained
            || !target_is_heal_pad
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
                    self.game_logic
                        .service_relationship_is_allies(unit_id, target_id)
                        && unit.is_alive()
                        && unit.can_move()
                        && !unit.status.under_construction
                        && unit.contained_by.is_none()
                        && is_injured
                        && unit.is_kind_of(KindOf::Infantry)
                })
                .unwrap_or(false);
            if can {
                if self.begin_support_order(unit_id, target_id, target_pos, AIState::SeekingHealing)
                {
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

    // === Beacon / retaliation / self-destruct (GameLogicDispatch.cpp) ===

    pub(super) fn execute_place_beacon(
        &mut self,
        player_id: u32,
        location: Vec3,
        text: &str,
    ) -> CommandResult {
        // C++ GameLogicDispatch.cpp:1582-1671 MSG_PLACE_BEACON.
        let Some(player) = self.game_logic.get_player(player_id) else {
            return CommandResult::InvalidCommand;
        };
        if !player.is_alive {
            self.notify_beacon_placement_failed(location);
            return CommandResult::InvalidCommand;
        }
        let team = player.team;
        let local_id = self
            .game_logic
            .get_players()
            .values()
            .find(|p| p.is_local)
            .map(|p| p.id);
        let template_name = host_beacon_template_name(self.game_logic, player_id, team);
        let max_beacons = host_max_beacons_per_player();
        let existing = host_count_player_beacons(self.game_logic, player_id, &template_name);
        if existing >= max_beacons {
            if local_id == Some(player_id) {
                let alert = localization::localize("GUI:TooManyBeacons", "Too many beacons");
                self.game_logic
                    .queue_radar_message_at(alert, location, RadarKind::Generic);
                self.game_logic
                    .queue_audio_event(AudioEventRequest::new(translate_audio_event(
                        "BeaconPlacementFailed",
                    )));
            }
            return CommandResult::InvalidCommand;
        }

        let pos = clamp_beacon_to_world(self.game_logic, location);
        if self
            .game_logic
            .templates
            .get(&template_name)
            .is_none()
        {
            let mut tpl = crate::game_logic::ThingTemplate::new(&template_name);
            tpl.set_health(1.0);
            self.game_logic.templates.insert(template_name.clone(), tpl);
        }
        let Some(beacon_id) =
            self.game_logic
                .create_object_for_player(&template_name, player_id, pos)
        else {
            self.notify_beacon_placement_failed(pos);
            return CommandResult::InvalidCommand;
        };

        if !text.is_empty() {
            live_beacon_set_caption(beacon_id, text);
        }

        let allied_or_observer = match local_id {
            Some(local) => {
                self.game_logic.player_relationship(player_id, local)
                    == gamelogic::common::Relationship::Allies
                    || self
                        .game_logic
                        .get_player(local)
                        .map(|p| {
                            p.name.eq_ignore_ascii_case("observer")
                                || p.team == crate::game_logic::Team::Neutral
                        })
                        .unwrap_or(false)
            }
            None => true,
        };

        if allied_or_observer {
            let alert = localization::localize("GUI:BeaconPlaced", "Beacon placed");
            self.game_logic
                .queue_radar_message_at(alert, pos, RadarKind::Generic);
            self.game_logic
                .queue_audio_event(AudioEventRequest::new(translate_audio_event(
                    "BeaconPlaced",
                )));
            self.game_logic.try_eva_beacon_detected(player_id);
        } else {
            live_beacon_hide(beacon_id);
        }

        // Keep the leftover manager + presentation note (Wave 210 residual).
        if let Ok(mut manager) = get_beacon_manager().lock() {
            let coord = LogicCoord3D::new(pos.x, pos.y, pos.z);
            manager.place_beacon(player_id as i32, coord, current_frame());
            if !text.is_empty() {
                manager.set_beacon_text(player_id as i32, &coord, AsciiString::from(text));
            }
        }
        self.game_logic.note_beacon_placed(pos);
        CommandResult::Success
    }

    pub(super) fn execute_remove_beacon(
        &mut self,
        player_id: u32,
        selected: &[ObjectId],
    ) -> CommandResult {
        // C++ GameLogicDispatch.cpp:1675-1727 MSG_REMOVE_BEACON:
        // owner destroys selected beacon objects; local non-owner hides them.
        let local_id = self
            .game_logic
            .get_players()
            .values()
            .find(|p| p.is_local)
            .map(|p| p.id);
        let mut any = false;
        let ids: Vec<ObjectId> = if selected.is_empty() {
            host_player_beacon_ids(self.game_logic, player_id)
        } else {
            selected.to_vec()
        };
        for id in ids {
            if !host_object_is_beacon(self.game_logic, id) {
                continue;
            }
            let owner = self
                .game_logic
                .host_object(id)
                .and_then(|o| o.owner_player_id);
            if owner == Some(player_id) {
                self.game_logic.destroy_object(id);
                live_beacon_clear(id);
                any = true;
            } else if local_id == Some(player_id) {
                live_beacon_hide(id);
                any = true;
            }
        }

        if let Ok(mut manager) = get_beacon_manager().lock() {
            if manager.remove_latest_beacon(player_id as i32) {
                any = true;
            }
        }
        if any {
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

    pub(super) fn execute_set_beacon_text(
        &mut self,
        player_id: u32,
        selected: &[ObjectId],
        text: &str,
    ) -> CommandResult {
        // C++ GameLogicDispatch.cpp:1731-1758 MSG_SET_BEACON_TEXT.
        let mut any = false;
        for &id in selected {
            if !host_object_is_beacon(self.game_logic, id) {
                continue;
            }
            let owner = self
                .game_logic
                .host_object(id)
                .and_then(|o| o.owner_player_id);
            if owner.is_some() && owner != Some(player_id) && !text.is_empty() {
                // Owner sets captions; empty still clears locally-selected beacons.
            }
            if text.is_empty() {
                live_beacon_clear_caption(id);
            } else {
                live_beacon_set_caption(id, text);
            }
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_enable_retaliation(
        &mut self,
        player_index: u32,
        enabled: bool,
    ) -> CommandResult {
        // C++ GameLogicDispatch.cpp:603-614 MSG_ENABLE_RETALIATION_MODE.
        if self.game_logic.get_player(player_index).is_none() {
            return CommandResult::InvalidCommand;
        }
        self.game_logic
            .set_logical_retaliation_mode(player_index, enabled);
        CommandResult::Success
    }

    pub(super) fn execute_self_destruct(
        &mut self,
        player_id: u32,
        transfer_to_ally: bool,
    ) -> CommandResult {
        // C++ GameLogicDispatch.cpp:1762-1797 MSG_SELF_DESTRUCT.
        if self.game_logic.get_player(player_id).is_none() {
            return CommandResult::InvalidCommand;
        }
        if transfer_to_ally {
            if let Some(ally_id) = host_living_mutual_ally(self.game_logic, player_id) {
                host_transfer_assets_from(self.game_logic, ally_id, player_id);
            }
        }
        host_kill_player(self.game_logic, player_id);
        CommandResult::Success
    }

    fn notify_beacon_placement_failed(&mut self, location: Vec3) {
        let alert = localization::localize("GUI:BeaconPlacementFailed", "Beacon placement failed");
        self.game_logic
            .queue_radar_message_at(alert, location, RadarKind::Generic);
        self.game_logic
            .queue_audio_event(AudioEventRequest::new(translate_audio_event(
                "BeaconPlacementFailed",
            )));
    }
}

fn clamp_beacon_to_world(logic: &GameLogic, location: Vec3) -> Vec3 {
    let (min, max) = logic.world_bounds();
    Vec3::new(
        location.x.clamp(min.x, max.x),
        location.y,
        location.z.clamp(min.z, max.z),
    )
}

fn host_beacon_template_name(logic: &GameLogic, player_id: u32, team: Team) -> String {
    if let Some(name) = logic
        .resolved_player_template(player_id)
        .map(|template| template.get_beacon_template().to_string())
        .filter(|name| !name.is_empty())
    {
        return name;
    }
    match team {
        Team::USA => "AmericaBeacon".to_string(),
        Team::China => "ChinaBeacon".to_string(),
        Team::GLA => "GLABeacon".to_string(),
        _ => "PlyrCivilianBeacon".to_string(),
    }
}

fn host_max_beacons_per_player() -> i32 {
    game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
        settings.max_beacons_per_player
    })
    .max(0)
}

fn host_object_is_beacon(logic: &GameLogic, id: ObjectId) -> bool {
    logic
        .host_object(id)
        .map(|obj| {
            let n = obj.template_name.to_ascii_lowercase();
            n.contains("beacon")
        })
        .unwrap_or(false)
}

fn host_count_player_beacons(logic: &GameLogic, player_id: u32, template_name: &str) -> i32 {
    logic
        .host_objects()
        .values()
        .filter(|obj| {
            obj.owner_player_id == Some(player_id)
                && obj.is_alive()
                && (obj.template_name.eq_ignore_ascii_case(template_name)
                    || obj.template_name.to_ascii_lowercase().contains("beacon"))
        })
        .count() as i32
}

fn host_player_beacon_ids(logic: &GameLogic, player_id: u32) -> Vec<ObjectId> {
    logic
        .host_objects()
        .iter()
        .filter_map(|(id, obj)| {
            (obj.owner_player_id == Some(player_id)
                && obj.is_alive()
                && obj.template_name.to_ascii_lowercase().contains("beacon"))
            .then_some(*id)
        })
        .collect()
}

/// C++ KINDOF_BRIDGE / KINDOF_BRIDGE_TOWER residual for repair reject.
fn host_object_is_bridge_or_tower(obj: &crate::game_logic::Object) -> bool {
    let name = obj.template_name.to_ascii_lowercase();
    name.contains("bridgetower")
        || name.contains("bridge_tower")
        || (name.contains("bridge") && !name.contains("bridger"))
}

fn host_living_mutual_ally(logic: &GameLogic, player_id: u32) -> Option<u32> {
    use gamelogic::common::Relationship;
    let ids: Vec<u32> = logic.get_players().keys().copied().collect();
    for other in ids {
        if other == player_id {
            continue;
        }
        let Some(other_player) = logic.get_player(other) else {
            continue;
        };
        if !other_player.is_alive {
            continue;
        }
        if logic.player_relationship(player_id, other) == Relationship::Allies
            && logic.player_relationship(other, player_id) == Relationship::Allies
        {
            return Some(other);
        }
    }
    None
}

fn host_transfer_assets_from(logic: &mut GameLogic, dest_player: u32, source_player: u32) {
    // C++ Player::transferAssetsFromThat — skip beacon templates.
    let ids: Vec<ObjectId> = logic
        .host_objects()
        .iter()
        .filter_map(|(id, obj)| {
            (obj.owner_player_id == Some(source_player)
                && obj.is_alive()
                && !obj.template_name.to_ascii_lowercase().contains("beacon"))
            .then_some(*id)
        })
        .collect();
    for id in ids {
        let _ = logic.transfer_object_to_player(id, dest_player);
    }
}

fn host_kill_player(logic: &mut GameLogic, player_id: u32) {
    // C++ Player::killPlayer — destroy remaining owned objects (incl. beacons).
    let ids: Vec<ObjectId> = logic
        .host_objects()
        .iter()
        .filter_map(|(id, obj)| (obj.owner_player_id == Some(player_id)).then_some(*id))
        .collect();
    for id in ids {
        logic.destroy_object(id);
        live_beacon_clear(id);
    }
    if let Some(player) = logic.get_player_mut(player_id) {
        player.is_alive = false;
        player.selected_objects.clear();
    }
}

#[derive(Default)]
struct LiveBeaconClientState {
    hidden: HashSet<u32>,
    captions: HashMap<u32, String>,
}

fn live_beacon_client_state() -> &'static std::sync::Mutex<LiveBeaconClientState> {
    static STATE: std::sync::LazyLock<std::sync::Mutex<LiveBeaconClientState>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(LiveBeaconClientState::default()));
    &STATE
}

fn live_beacon_hide(id: ObjectId) {
    if let Ok(mut state) = live_beacon_client_state().lock() {
        state.hidden.insert(id.0);
    }
}

fn live_beacon_set_caption(id: ObjectId, text: &str) {
    if let Ok(mut state) = live_beacon_client_state().lock() {
        state.captions.insert(id.0, text.to_string());
    }
}

fn live_beacon_clear_caption(id: ObjectId) {
    if let Ok(mut state) = live_beacon_client_state().lock() {
        state.captions.remove(&id.0);
    }
}

fn live_beacon_clear(id: ObjectId) {
    if let Ok(mut state) = live_beacon_client_state().lock() {
        state.hidden.remove(&id.0);
        state.captions.remove(&id.0);
    }
}

/// C++ `BeaconClientUpdate::hideBeacon` residual — enemy beacons stay hidden.
pub fn host_beacon_is_hidden(id: ObjectId) -> bool {
    live_beacon_client_state()
        .lock()
        .map(|s| s.hidden.contains(&id.0))
        .unwrap_or(false)
}

/// C++ Drawable::getCaptionText residual for live beacons.
pub fn host_beacon_caption(id: ObjectId) -> Option<String> {
    live_beacon_client_state()
        .lock()
        .ok()
        .and_then(|s| s.captions.get(&id.0).cloned())
}

impl GameLogic {
    /// C++ `DozerAIUpdate::newTask(DOZER_TASK_REPAIR)` (DozerAIUpdate.cpp:1948-2008).
    /// Parks an in-flight BUILD in its own slot; only the REPAIR slot is replaced.
    pub fn dozer_new_task_repair(&mut self, dozer_id: ObjectId, repair_target: ObjectId) {
        let frame = self.frame.max(1);
        if let Some(obj) = self.objects.get_mut(&dozer_id) {
            if matches!(obj.ai_state, AIState::Constructing) {
                if let Some(build_id) = obj.target {
                    if obj.dozer_task_build_target.is_none() {
                        obj.dozer_task_build_target = Some(build_id);
                        // Older than this REPAIR so getMostRecentCommand prefers REPAIR.
                        obj.dozer_task_build_order_frame = frame.saturating_sub(1).max(1);
                    }
                }
            }
            obj.dozer_task_repair_target = Some(repair_target);
            obj.dozer_task_repair_order_frame = frame;
        }
    }

    /// C++ `DozerAIUpdate::internalTaskComplete` for one task slot.
    pub fn dozer_internal_task_complete(&mut self, dozer_id: ObjectId, repair: bool) {
        if let Some(obj) = self.objects.get_mut(&dozer_id) {
            if repair {
                obj.dozer_task_repair_target = None;
                obj.dozer_task_repair_order_frame = 0;
            } else {
                obj.dozer_task_build_target = None;
                obj.dozer_task_build_order_frame = 0;
            }
        }
    }

    fn dozer_most_recent_pending_task(&self, dozer_id: ObjectId) -> Option<(bool, ObjectId)> {
        let obj = self.objects.get(&dozer_id)?;
        let mut best: Option<(u32, bool, ObjectId)> = None;
        if let Some(tid) = obj.dozer_task_build_target {
            let frame = obj.dozer_task_build_order_frame;
            if best.is_none_or(|(f, _, _)| frame >= f) {
                best = Some((frame, false, tid));
            }
        }
        if let Some(tid) = obj.dozer_task_repair_target {
            let frame = obj.dozer_task_repair_order_frame;
            if best.is_none_or(|(f, _, _)| frame >= f) {
                best = Some((frame, true, tid));
            }
        }
        best.map(|(_, is_repair, tid)| (is_repair, tid))
    }

    /// C++ idle `isBuildMostImportant` / `isRepairMostImportant` (DozerAIUpdate.cpp:1314).
    /// Call only while the dozer is Idle. Returns true if a pending task was resumed.
    pub fn dozer_idle_resume_pending_build(&mut self, dozer_id: ObjectId) -> bool {
        let Some(obj) = self.objects.get(&dozer_id) else {
            return false;
        };
        if !matches!(obj.ai_state, AIState::Idle) {
            return false;
        }
        if !obj.is_alive() || !obj.can_construct() {
            return false;
        }
        // Drop dead / finished slots so getMostRecentCommand matches C++.
        let build_id = obj.dozer_task_build_target;
        let repair_id = obj.dozer_task_repair_target;
        if let Some(tid) = build_id {
            let keep = self.objects.get(&tid).is_some_and(|t| {
                t.is_alive() && t.is_kind_of(KindOf::Structure) && t.status.under_construction
            });
            if !keep {
                self.dozer_internal_task_complete(dozer_id, false);
            }
        }
        if let Some(tid) = repair_id {
            let keep = self.objects.get(&tid).is_some_and(|t| {
                t.is_alive()
                    && t.is_kind_of(KindOf::Structure)
                    && !t.status.under_construction
                    && t.health.current + 0.01 < t.health.maximum
            });
            if !keep {
                self.dozer_internal_task_complete(dozer_id, true);
            }
        }
        let Some((is_repair, tid)) = self.dozer_most_recent_pending_task(dozer_id) else {
            return false;
        };
        if is_repair {
            return false;
        }
        // C++ idleConditions → DOZER_PRIMARY_BUILD. This is our parked slot,
        // not a player resume (ActionManager::canResumeConstructionOf).
        let (dozer_pos, st_pos, st_radius) = {
            let dpos = self
                .objects
                .get(&dozer_id)
                .map(|d| d.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            let (spos, srad) = self
                .objects
                .get(&tid)
                .map(|s| (s.get_position(), s.selection_radius))
                .unwrap_or((glam::Vec3::ZERO, 0.0));
            (dpos, spos, srad)
        };
        if let Some(dozer) = self.objects.get_mut(&dozer_id) {
            dozer.target = Some(tid);
            dozer.set_ai_state(AIState::Constructing);
            dozer.idle_since_frame = 0;
        }
        let approach = crate::game_logic::host_repair::dozer_repair_approach_position(
            dozer_pos, st_pos, st_radius,
        );
        self.path_approach_with_state(dozer_id, approach, AIState::Constructing);
        if let Some(st) = self.objects.get_mut(&tid) {
            st.set_under_construction_model_conditions(true);
            st.builder_id = Some(dozer_id);
        }
        true
    }

    /// C++ `Player::update` (`Player.cpp:708-724`): once per second, if the
    /// local client Auto-Retaliate flag differs from the logical flag, post
    /// `MSG_ENABLE_RETALIATION_MODE` so `CommandExecutor` applies it.
    pub fn leftover_dispatch_tick(&mut self) {
        if self.frame == 0 || self.frame % 30 != 0 {
            return;
        }
        let client_enabled =
            game_engine::common::global_data::read().client_retaliation_mode_enabled;
        let pending: Vec<(u32, bool)> = self
            .get_players()
            .values()
            .filter(|player| {
                player.is_local && player.logical_retaliation_mode_enabled != client_enabled
            })
            .map(|player| (player.id, client_enabled))
            .collect();
        for (player_id, enabled) in pending {
            self.queue_command(crate::command_system::GameCommand {
                command_type: CommandType::EnableRetaliationMode {
                    player_index: player_id,
                    enabled,
                },
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: Vec::new(),
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        }
    }
}

/// C++ `getRappellerCount` — CombatDrop Restricted at 0.
pub fn host_rappeller_count(logic: &GameLogic, transport_id: ObjectId) -> usize {
    let Some(obj) = logic.host_object(transport_id) else {
        return 0;
    };
    obj.occupants
        .iter()
        .copied()
        .filter(|&pid| {
            logic.host_object(pid).is_some_and(|pax| {
                pax.is_alive() && pax.is_kind_of(KindOf::Infantry)
            })
        })
        .count()
}

/// C++ `HackInternetAIInterface::isHackingPackingOrUnpacking`.
pub fn host_hack_internet_restricted(logic: &GameLogic, hacker_id: ObjectId) -> bool {
    logic.hacker_income.is_hacking(hacker_id)
}

/// Convert crate `GameMessageType` leftovers into live host commands.
/// C++ `GameLogicDispatch` is the only consumer of these MSG_* types.
pub fn leftover_command_from_common_message(
    message: &game_engine::common::message_stream::GameMessage,
    selected: &[ObjectId],
) -> Option<crate::command_system::GameCommand> {
    use game_engine::common::message_stream::{GameMessageArgumentType, GameMessageType};

    let command_type = match message.get_type() {
        GameMessageType::SwitchWeapons(slot) => CommandType::SwitchWeapons { slot: *slot as u8 },
        GameMessageType::PlaceBeacon(coord) => CommandType::PlaceBeacon {
            location: Vec3::new(coord.x, coord.y, coord.z),
            text: String::new(),
        },
        GameMessageType::RemoveBeacon(_) => CommandType::RemoveBeacon,
        GameMessageType::SetBeaconText(_, text) => CommandType::SetBeaconText { text: text.clone() },
        GameMessageType::SelfDestruct(player_id) => {
            let _ = player_id;
            let transfer_to_ally = match message.get_argument(0) {
                Some(GameMessageArgumentType::Boolean(flag)) => *flag,
                _ => true,
            };
            CommandType::SelfDestruct { transfer_to_ally }
        }
        GameMessageType::EnableRetaliationMode(player_index, enabled) => {
            CommandType::EnableRetaliationMode {
                player_index: *player_index,
                enabled: *enabled,
            }
        }
        _ => return None,
    };
    let player_id = match message.get_type() {
        GameMessageType::SelfDestruct(player_id) => *player_id,
        GameMessageType::EnableRetaliationMode(player_index, _) => *player_index,
        _ => message.get_player_index().max(0) as u32,
    };
    Some(crate::command_system::GameCommand {
        command_type,
        player_id,
        command_id: 0,
        timestamp: std::time::SystemTime::now(),
        selected_units: selected.to_vec(),
        modifier_keys: crate::command_system::ModifierKeys::default(),
    })
}

/// Drain leftover dispatch messages from TheMessageStream into host commands.
pub fn take_leftover_dispatch_commands_from_common_stream(
    selected: &[ObjectId],
) -> Vec<crate::command_system::GameCommand> {
    let stream = game_engine::common::message_stream::get_message_stream();
    let mut stream = stream.write().unwrap_or_else(|e| e.into_inner());
    let messages: Vec<_> = stream.get_messages().iter().cloned().collect();
    let mut kept = Vec::new();
    let mut commands = Vec::new();
    for message in messages {
        if let Some(command) = leftover_command_from_common_message(&message, selected) {
            commands.push(command);
        } else {
            kept.push(message);
        }
    }
    stream.clear_messages();
    for message in &kept {
        let forwarded = stream.append_message(message.get_type().clone());
        for arg in message.get_arguments() {
            match &arg.data {
                game_engine::common::message_stream::GameMessageArgumentType::Integer(v) => {
                    forwarded.append_integer_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::Real(v) => {
                    forwarded.append_real_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::Boolean(v) => {
                    forwarded.append_boolean_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::ObjectID(v) => {
                    forwarded.append_object_id_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::DrawableID(v) => {
                    forwarded.append_drawable_id_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::TeamID(v) => {
                    forwarded.append_team_id_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::SquadID(v) => {
                    forwarded.append_team_id_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::Location(v) => {
                    forwarded.append_location_argument(v.clone())
                }
                game_engine::common::message_stream::GameMessageArgumentType::Pixel(v) => {
                    forwarded.append_pixel_argument(v.clone())
                }
                game_engine::common::message_stream::GameMessageArgumentType::PixelRegion(v) => {
                    forwarded.append_pixel_region_argument(v.clone())
                }
                game_engine::common::message_stream::GameMessageArgumentType::Timestamp(v) => {
                    forwarded.append_timestamp_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::WideChar(v) => {
                    forwarded.append_wide_char_argument(*v)
                }
                game_engine::common::message_stream::GameMessageArgumentType::String(v) => {
                    forwarded.append_string_argument(v.clone())
                }
            }
        }
    }
    commands
}

#[cfg(test)]
mod leftover_dispatch_tests {
    use super::*;
    use crate::command_system::{CommandResult, GameCommand, ModifierKeys};
    use crate::game_logic::Player;
    use std::time::SystemTime;

    fn command(player_id: u32, command_type: CommandType, selected: Vec<ObjectId>) -> GameCommand {
        GameCommand {
            command_type,
            player_id,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: selected,
            modifier_keys: ModifierKeys::default(),
        }
    }

    #[test]
    fn switch_weapons_locks_button_slot() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        let mut tpl = crate::game_logic::ThingTemplate::new("HumveeSW");
        tpl.set_health(200.0);
        logic.templates.insert("HumveeSW".into(), tpl);
        let id = logic
            .create_object_for_player("HumveeSW", 0, Vec3::ZERO)
            .expect("unit");
        {
            let unit = logic.host_object_mut(id).expect("obj");
            unit.weapon = Some(crate::game_logic::Weapon {
                damage: 1.0,
                range: 10.0,
                ..crate::game_logic::Weapon::default()
            });
            unit.secondary_weapon = Some(crate::game_logic::Weapon {
                damage: 5.0,
                range: 20.0,
                ..crate::game_logic::Weapon::default()
            });
            unit.active_weapon_slot = 0;
        }
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(0, CommandType::SwitchWeapons { slot: 1 }, vec![id]))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        let unit = logic.host_object(id).expect("obj");
        assert_eq!(unit.weapon_lock_slot, 1);
        assert_eq!(unit.active_weapon_slot, 1);
    }

    #[test]
    fn enable_retaliation_reaches_host_player() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        assert!(!logic.get_player(0).unwrap().logical_retaliation_mode_enabled);
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::EnableRetaliationMode {
                    player_index: 0,
                    enabled: true,
                },
                vec![],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        assert!(logic.get_player(0).unwrap().logical_retaliation_mode_enabled);
    }

    #[test]
    fn leftover_dispatch_tick_posts_enable_retaliation() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        logic.frame = 30;
        game_engine::common::global_data::write().client_retaliation_mode_enabled = true;
        logic.leftover_dispatch_tick();
        assert!(logic.has_pending_commands());
        logic.process_commands();
        assert!(logic.get_player(0).unwrap().logical_retaliation_mode_enabled);
    }

    #[test]
    fn self_destruct_transfers_to_living_ally() {
        let mut logic = GameLogic::new();
        let mut p0 = Player::new(0, Team::USA, "P0", true);
        let mut p1 = Player::new(1, Team::USA, "P1", false);
        p0.alliance_team = 1;
        p1.alliance_team = 1;
        logic.get_players_mut().insert(0, p0);
        logic.get_players_mut().insert(1, p1);
        let mut tpl = crate::game_logic::ThingTemplate::new("RangerSD");
        tpl.set_health(100.0);
        logic.templates.insert("RangerSD".into(), tpl);
        let id = logic
            .create_object_for_player("RangerSD", 0, Vec3::ZERO)
            .expect("unit");
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::SelfDestruct {
                    transfer_to_ally: true,
                },
                vec![],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        assert!(!logic.get_player(0).unwrap().is_alive);
        assert_eq!(logic.host_object(id).unwrap().owner_player_id, Some(1));
    }

    #[test]
    fn view_command_center_uses_own_player_not_same_faction() {
        // C++ viewCommandCenter iterates localPlayer objects only.
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        logic
            .get_players_mut()
            .insert(1, Player::new(1, Team::USA, "P1", false));
        let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
        cc.add_kind_of(KindOf::CommandCenter)
            .add_kind_of(KindOf::Structure)
            .set_health(1000.0);
        logic.templates.insert("AmericaCommandCenter".into(), cc);
        let mine = logic
            .create_object_for_player("AmericaCommandCenter", 0, Vec3::new(10.0, 0.0, 10.0))
            .expect("own CC");
        let theirs = logic
            .create_object_for_player("AmericaCommandCenter", 1, Vec3::new(500.0, 0.0, 500.0))
            .expect("enemy CC");
        let _ = (mine, theirs);
        let mine_pos = logic.player_command_center_position(0).expect("own CC pose");
        assert!((mine_pos.x - 10.0).abs() < 0.1);
        assert!((mine_pos.z - 10.0).abs() < 0.1);
        let theirs_pos = logic.player_command_center_position(1).expect("p1 CC pose");
        assert!((theirs_pos.x - 500.0).abs() < 0.1);
    }

    #[test]
    fn place_beacon_spawns_world_object_and_respects_cap() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        let loc = Vec3::new(10.0, 0.0, 12.0);
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::PlaceBeacon {
                    location: loc,
                    text: "here".into(),
                },
                vec![],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        let beacons: Vec<_> = logic
            .host_objects()
            .iter()
            .filter(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(beacons.len(), 1);
        assert_eq!(host_beacon_caption(beacons[0]).as_deref(), Some("here"));

        let max = host_max_beacons_per_player().max(1);
        for i in 1..max {
            let r = CommandExecutor::new(&mut logic, 0)
                .execute_command(command(
                    0,
                    CommandType::PlaceBeacon {
                        location: Vec3::new(20.0 + i as f32, 0.0, 0.0),
                        text: String::new(),
                    },
                    vec![],
                ))
                .expect("exec");
            assert_eq!(r, CommandResult::Success);
        }
        let overflow = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::PlaceBeacon {
                    location: Vec3::new(99.0, 0.0, 99.0),
                    text: String::new(),
                },
                vec![],
            ))
            .expect("exec");
        assert_eq!(overflow, CommandResult::InvalidCommand);
    }

    #[test]
    fn set_beacon_text_updates_selected_caption() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::PlaceBeacon {
                    location: Vec3::new(4.0, 0.0, 5.0),
                    text: String::new(),
                },
                vec![],
            ))
            .unwrap();
        let id = logic
            .host_objects()
            .iter()
            .find(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
            .map(|(id, _)| *id)
            .expect("beacon");
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::SetBeaconText {
                    text: "go".into(),
                },
                vec![id],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        assert_eq!(host_beacon_caption(id).as_deref(), Some("go"));
    }

    #[test]
    fn repair_mid_build_keeps_pending_build_and_idle_resumes() {
        // C++ DozerAIUpdate.cpp:1948 newTask slots + 1314 isBuildMostImportant.
        use crate::game_logic::ThingTemplate;
        let mut logic = GameLogic::new();
        logic.frame = 10;
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        let mut dozer_tpl = ThingTemplate::new("DozerParkBuild");
        dozer_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .set_health(300.0);
        logic
            .templates
            .insert("DozerParkBuild".into(), dozer_tpl);
        let mut bld = ThingTemplate::new("ScaffoldParkBuild");
        bld.add_kind_of(KindOf::Structure).set_health(500.0);
        logic.templates.insert("ScaffoldParkBuild".into(), bld.clone());
        logic
            .templates
            .insert("DamagedParkBuild".into(), bld);
        let dozer = logic
            .create_object_for_player("DozerParkBuild", 0, Vec3::ZERO)
            .expect("dozer");
        let scaffold = logic
            .create_object_for_player("ScaffoldParkBuild", 0, Vec3::new(20.0, 0.0, 0.0))
            .expect("scaffold");
        let damaged = logic
            .create_object_for_player("DamagedParkBuild", 0, Vec3::new(40.0, 0.0, 0.0))
            .expect("damaged");
        {
            let sc = logic.host_object_mut(scaffold).expect("sc");
            sc.status.under_construction = true;
            sc.builder_id = Some(dozer);
        }
        {
            let dmg = logic.host_object_mut(damaged).expect("dmg");
            let _ = dmg.take_damage(200.0);
        }
        {
            let dz = logic.host_object_mut(dozer).expect("dz");
            dz.target = Some(scaffold);
            dz.set_ai_state(AIState::Constructing);
        }
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::Repair { target_id: damaged },
                vec![dozer],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        {
            let dz = logic.host_object(dozer).expect("dz");
            assert_eq!(dz.ai_state, AIState::Repairing);
            assert_eq!(dz.target, Some(damaged));
            assert_eq!(
                dz.dozer_task_build_target,
                Some(scaffold),
                "REPAIR must keep BUILD pending"
            );
            assert_eq!(dz.dozer_task_repair_target, Some(damaged));
        }
        logic.dozer_internal_task_complete(dozer, true);
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.set_target(None);
            dz.set_ai_state(AIState::Idle);
        }
        assert!(
            logic.dozer_idle_resume_pending_build(dozer),
            "idle isBuildMostImportant must resume parked BUILD"
        );
        let dz = logic.host_object(dozer).expect("dz");
        assert_eq!(dz.ai_state, AIState::Constructing);
        assert_eq!(dz.target, Some(scaffold));
        assert_eq!(dz.dozer_task_build_target, Some(scaffold));
    }
}
