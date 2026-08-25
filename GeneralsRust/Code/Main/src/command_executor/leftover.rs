//! Shared path helper plus remaining utility commands.
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
        self.path_to_goal_with_state_ignoring(unit_id, goal, state, None)
    }

    /// C++ `ignoreObstacle(goalObject)` then `aiMoveToPosition`.
    pub(super) fn path_to_goal_with_state_ignoring(
        &mut self,
        unit_id: ObjectId,
        goal: Vec3,
        state: AIState,
        ignore_obstacle: Option<ObjectId>,
    ) -> bool {
        self.game_logic
            .unit_command_path_with_state_ignoring(unit_id, goal, state, ignore_obstacle)
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
                        let _ = self.path_to_goal_with_state_ignoring(
                            unit_id,
                            bpos,
                            AIState::Entering,
                            Some(building_id),
                        );
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
        use crate::game_logic::host_mines::{DOZER_MINE_CLEAR_SCAN_RANGE, is_mine_clearer};
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
            // C++ drops boxes only while already isClearingMines (aiDoCommand tail).
            // Initial order is not attacking yet — apply_dozer_ai_do_command handles the tail.
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
            // C++ CommandXlat.cpp:336-339 MSG_DO_REPAIR → PerUnitSound VoiceRepair.
            self.game_logic.queue_picked_unit_voice(
                units,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Repair,
            );
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
                if is_aircraft {
                    self.game_logic
                        .try_aircraft_land_for_repair(unit_id, target_id);
                }
                if self.begin_support_order(unit_id, target_id, target_pos, AIState::SeekingRepair)
                {
                    any = true;
                }
            }
        }
        if any {
            // C++ CommandXlat.cpp:384-443 MSG_GET_REPAIRED VoiceMove / VoiceMoveUpgraded.
            self.play_context_move_voice(units);
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
            // C++ CommandXlat.cpp:384-443 MSG_GET_HEALED VoiceMove / VoiceMoveUpgraded.
            self.play_context_move_voice(units);
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
        if self.game_logic.templates.get(&template_name).is_none() {
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
            live_beacon_set_caption(self.game_logic, beacon_id, text);
        }
        live_beacon_note_pulse_frame(beacon_id, self.game_logic.frame);

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
            let placer_name = self
                .game_logic
                .get_player(player_id)
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Player".to_string());
            let alert = localization::localize_with_args(
                "GUI:BeaconPlaced",
                "{player} placed a beacon",
                &[("player", placer_name.as_str())],
            )
            .replace("%s", &placer_name);
            self.game_logic
                .queue_radar_message_at(alert, pos, RadarKind::Generic);
            self.game_logic
                .queue_audio_event(AudioEventRequest::new(translate_audio_event(
                    "BeaconPlaced",
                )));
            crate::game_logic::host_radar::host_create_radar_event(
                pos,
                game_engine::common::system::radar::RadarEventType::Information,
            );
            self.game_logic.try_eva_beacon_detected(player_id);
            if let Ok(mut manager) = get_beacon_manager().lock() {
                let coord = LogicCoord3D::new(pos.x, pos.y, pos.z);
                manager.place_beacon(player_id as i32, coord, current_frame());
                if !text.is_empty() {
                    manager.set_beacon_text(player_id as i32, &coord, AsciiString::from(text));
                }
            }
            self.game_logic.note_beacon_placed(pos);
        } else {
            live_beacon_hide(self.game_logic, beacon_id);
        }
        CommandResult::Success
    }

    pub(super) fn execute_remove_beacon(
        &mut self,
        player_id: u32,
        selected: &[ObjectId],
    ) -> CommandResult {
        // C++ GameLogicDispatch.cpp:1675-1727 MSG_REMOVE_BEACON:
        // owner destroys selected beacon objects; local non-owner hides them.
        // Retail is strictly selection-driven: empty / non-beacon selection is a no-op.
        let local_id = self
            .game_logic
            .get_players()
            .values()
            .find(|p| p.is_local)
            .map(|p| p.id);
        let mut any = false;
        for &id in selected {
            if !host_object_is_beacon(self.game_logic, id) {
                continue;
            }
            let owner = self
                .game_logic
                .host_object(id)
                .and_then(|o| o.owner_player_id);
            let pos = self.game_logic.host_object(id).map(|o| o.get_position());
            if owner == Some(player_id) {
                if let Some(pos) = pos {
                    self.game_logic.note_beacon_removed_at(pos);
                }
                self.game_logic.destroy_object(id);
                live_beacon_clear(id);
                any = true;
            } else if local_id == Some(player_id) {
                live_beacon_hide(self.game_logic, id);
                any = true;
            }
        }

        if !any {
            return CommandResult::InvalidCommand;
        }
        if let Ok(mut manager) = get_beacon_manager().lock() {
            let _ = manager.remove_latest_beacon(player_id as i32);
        }
        // C++ MSG_REMOVE_BEACON is silent: no audio, no InGameUI/radar message.
        // Wave 211: note_beacon_removed_latest replaced by per-id note_beacon_removed_at.
        CommandResult::Success
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
                live_beacon_clear_caption(self.game_logic, id);
            } else {
                live_beacon_set_caption(self.game_logic, id, text);
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
    // C++ Player::transferAssetsFromThat — skip beacon templates, then cash.
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
    let amount = logic
        .get_player(source_player)
        .map(|p| p.effective_supplies())
        .unwrap_or(0);
    if amount > 0 {
        if let Some(src) = logic.get_player_mut(source_player) {
            crate::game_logic::host_economy_log::record_money_audio(
                source_player,
                crate::game_logic::host_economy_log::HostMoneyAudio::Withdraw,
            );
            src.apply_supply_spend_unchecked(amount);
        }
        if let Some(dst) = logic.get_player_mut(dest_player) {
            crate::game_logic::host_economy_log::record_money_audio(
                dest_player,
                crate::game_logic::host_economy_log::HostMoneyAudio::Deposit,
            );
            dst.apply_supply_gain(amount);
        }
    }
}

fn host_kill_player(logic: &mut GameLogic, player_id: u32) {
    // C++ Player::killPlayer — same army/beacon/tech/SP-AI path as victory.
    let ids: Vec<ObjectId> = logic
        .host_objects()
        .iter()
        .filter_map(|(id, obj)| (obj.owner_player_id == Some(player_id)).then_some(*id))
        .collect();
    logic.kill_player_for_victory(player_id);
    for id in ids {
        live_beacon_clear(id);
    }
}

#[derive(Default)]
struct LiveBeaconClientState {
    hidden: HashSet<u32>,
    captions: HashMap<u32, String>,
    last_pulse: HashMap<u32, u32>,
    smoke: HashMap<u32, u32>,
}

fn live_beacon_client_state() -> &'static std::sync::Mutex<LiveBeaconClientState> {
    static STATE: std::sync::LazyLock<std::sync::Mutex<LiveBeaconClientState>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(LiveBeaconClientState::default()));
    &STATE
}

/// C++ BeaconClientUpdate defaults live on leftover `BeaconClientUpdateModuleData`.

fn live_beacon_note_pulse_frame(id: ObjectId, frame: u32) {
    if let Ok(mut state) = live_beacon_client_state().lock() {
        state.last_pulse.insert(id.0, frame);
    }
}

fn live_beacon_hide(logic: &mut GameLogic, id: ObjectId) {
    let pos = logic.host_object(id).map(|o| o.get_position());
    if let Some(obj) = logic.host_object_mut(id) {
        // C++ BeaconClientUpdate::hideBeacon: setDrawableHidden + no shadows.
        obj.set_drawable_hidden(true);
    }
    logic.deselect_drawable(id);
    if let Some(pos) = pos {
        logic.note_beacon_removed_at(pos);
    }
    let smoke_id = live_beacon_client_state().lock().ok().and_then(|mut s| {
        s.hidden.insert(id.0);
        s.smoke.remove(&id.0)
    });
    if let Some(smoke_id) = smoke_id {
        logic.combat_particles_mut().deactivate(smoke_id);
    }
}

fn live_beacon_set_caption(logic: &mut GameLogic, id: ObjectId, text: &str) {
    if let Ok(mut state) = live_beacon_client_state().lock() {
        state.captions.insert(id.0, text.to_string());
    }
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = text.to_string();
    }
}

fn live_beacon_clear_caption(logic: &mut GameLogic, id: ObjectId) {
    if let Ok(mut state) = live_beacon_client_state().lock() {
        state.captions.remove(&id.0);
    }
    if let Some(obj) = logic.host_object_mut(id) {
        obj.name = obj.template_name.clone();
    }
}

fn live_beacon_clear(id: ObjectId) {
    if let Ok(mut state) = live_beacon_client_state().lock() {
        state.hidden.remove(&id.0);
        state.captions.remove(&id.0);
        state.last_pulse.remove(&id.0);
        state.smoke.remove(&id.0);
    }
}

/// C++ `BeaconClientUpdate::hideBeacon` residual — enemy beacons stay hidden.
pub fn host_beacon_is_hidden(id: ObjectId) -> bool {
    live_beacon_client_state()
        .lock()
        .map(|s| s.hidden.contains(&id.0))
        .unwrap_or(false)
}

/// C++ hidden drawable has no world or minimap presence.
/// `host_beacons` is position-only, so presentation matches hide by pose.
pub fn host_beacon_position_is_hidden(logic: &GameLogic, pos: Vec3) -> bool {
    const MATCH: f32 = 3.0; // beacon_manager BEACON_MATCH_THRESHOLD
    logic.host_objects().values().any(|obj| {
        obj.template_name.to_ascii_lowercase().contains("beacon")
            && (host_beacon_is_hidden(obj.id) || obj.drawable_hidden)
            && (obj.get_position() - pos).length() <= MATCH
    })
}

/// C++ Drawable::getCaptionText residual for live beacons.
pub fn host_beacon_caption(id: ObjectId) -> Option<String> {
    live_beacon_client_state()
        .lock()
        .ok()
        .and_then(|s| s.captions.get(&id.0).cloned())
}

/// C++ ControlBar.cpp update — count < MaxBeaconsPerPlayer.
pub fn host_local_player_can_place_beacon(logic: &GameLogic, player_id: u32) -> bool {
    let Some(player) = logic.get_player(player_id) else {
        return false;
    };
    if !player.is_alive {
        return false;
    }
    let template_name = host_beacon_template_name(logic, player_id, player.team);
    host_count_player_beacons(logic, player_id, &template_name) < host_max_beacons_per_player()
}

/// C++ BeaconClientUpdate::clientUpdate — house-color smoke + yellow pulse.
pub fn tick_live_beacon_client_updates(logic: &mut GameLogic) {
    use gamelogic::helpers::TheParticleSystemManager;
    use gamelogic::object::update::{BeaconClientUpdateModule, BeaconClientUpdateModuleData};

    let frame = logic.frame;
    let pulse_data = BeaconClientUpdateModuleData::default();
    let pulse_seconds = pulse_data.radar_pulse_duration as f32 / 30.0;
    let beacons: Vec<(ObjectId, glam::Vec3, bool, u32)> = logic
        .host_objects()
        .iter()
        .filter_map(|(id, obj)| {
            if !obj.is_alive() || !obj.template_name.to_ascii_lowercase().contains("beacon") {
                return None;
            }
            let hidden = obj.drawable_hidden || host_beacon_is_hidden(*id);
            let owner = obj.owner_player_id.unwrap_or(0);
            Some((*id, obj.get_position(), hidden, owner))
        })
        .collect();
    for (id, pos, hidden, owner) in beacons {
        let smoke_missing = live_beacon_client_state()
            .lock()
            .map(|s| !s.smoke.contains_key(&id.0))
            .unwrap_or(true);
        if smoke_missing {
            let rgb = logic
                .get_player(owner)
                .map(|p| p.color_rgb)
                .unwrap_or((255, 255, 255));
            let color = gamelogic::common::Color::rgb(rgb.0, rgb.1, rgb.2);
            let (template, tint) =
                BeaconClientUpdateModule::resolve_smoke_template_with_lookup(color, |name| {
                    TheParticleSystemManager::get()
                        .and_then(|m| m.find_template(name))
                        .is_some()
                })
                .unwrap_or_else(|| {
                    (
                        format!("BeaconSmoke{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2),
                        Some(color),
                    )
                });
            if let Some(smoke_id) = logic
                .combat_particles_mut()
                .attach_named_to_object(id, pos, frame, &template)
            {
                if let Some(tint) = tint {
                    if let Some(client_id) = logic
                        .combat_particles()
                        .get(smoke_id)
                        .and_then(|e| e.client_system_id)
                    {
                        if let Some(mgr) = TheParticleSystemManager::get() {
                            mgr.tint_particle_system_all_colors(client_id, tint);
                        }
                    }
                }
                if let Ok(mut state) = live_beacon_client_state().lock() {
                    state.smoke.insert(id.0, smoke_id);
                }
                if hidden {
                    logic.combat_particles_mut().deactivate(smoke_id);
                }
            }
        }
        if hidden {
            continue;
        }
        let should_pulse = {
            let Ok(mut state) = live_beacon_client_state().lock() else {
                continue;
            };
            let last = state.last_pulse.entry(id.0).or_insert(frame);
            if frame > *last + pulse_data.frames_between_radar_pulses {
                *last = frame;
                true
            } else {
                false
            }
        };
        if should_pulse {
            crate::game_logic::host_radar::host_create_radar_event_for(
                pos,
                game_engine::common::system::radar::RadarEventType::BeaconPulse,
                pulse_seconds,
            );
        }
    }
}

impl GameLogic {
    /// C++ `DozerAIUpdate::newTask(DOZER_TASK_BUILD)` (DozerAIUpdate.cpp:1717, 1948-2008).
    /// Records the BUILD slot so idle `isBuildMostImportant` can resume after a move-away.
    pub fn dozer_new_task_build(&mut self, dozer_id: ObjectId, build_target: ObjectId) {
        let frame = self.frame.max(1);
        let old = self
            .objects
            .get(&dozer_id)
            .and_then(|obj| obj.dozer_task_build_target);
        if let Some(old_id) = old {
            if old_id != build_target {
                if let Some(prev) = self.objects.get_mut(&old_id) {
                    if prev.builder_id == Some(dozer_id) {
                        prev.builder_id = None;
                    }
                }
            }
        }
        // C++ newTask findGoodBuildOrRepairPosition → ACTION dock (cpp:1973-1991).
        let action_dock = {
            let dozer_info = self.objects.get(&dozer_id).map(|d| {
                (
                    d.get_position(),
                    d.is_kind_of(KindOf::Aircraft) || d.status.airborne_target,
                )
            });
            let site = self
                .objects
                .get(&build_target)
                .map(|s| (s.get_position(), s.selection_radius));
            match (dozer_info, site) {
                (Some((dozer_pos, airborne)), Some((site_pos, site_radius))) => {
                    Some(self.find_good_build_or_repair_position(
                        dozer_pos,
                        site_pos,
                        site_radius,
                        airborne,
                        airborne.then_some(build_target),
                        Some(dozer_id),
                    ))
                }
                _ => None,
            }
        };
        if let Some(obj) = self.objects.get_mut(&dozer_id) {
            obj.dozer_task_build_target = Some(build_target);
            obj.dozer_task_build_order_frame = frame;
            obj.dozer_dock_action = action_dock;
        }
        if let Some(st) = self.objects.get_mut(&build_target) {
            // C++ newTask setBuilder (DozerAIUpdate.cpp:1986).
            st.builder_id = Some(dozer_id);
        }
    }

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
        let repair_target = if repair {
            self.objects
                .get(&dozer_id)
                .and_then(|obj| obj.dozer_task_repair_target.or(obj.target))
        } else {
            None
        };
        if let Some(obj) = self.objects.get_mut(&dozer_id) {
            if repair {
                obj.dozer_task_repair_target = None;
                obj.dozer_task_repair_order_frame = 0;
            } else {
                obj.dozer_task_build_target = None;
                obj.dozer_task_build_order_frame = 0;
                obj.dozer_dock_action = None;
            }
        }
        // C++ WorkerAIUpdate.cpp:830 removeBridgeScaffolding on repair complete/cancel.
        if let Some(tid) = repair_target {
            let is_bridge = self.objects.get(&tid).is_some_and(|t| {
                t.is_kind_of(KindOf::Bridge)
                    || t.is_kind_of(KindOf::BridgeTower)
                    || crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                        &t.template_name,
                    )
            });
            if is_bridge {
                if let Some(sid) = self.resolve_bridge_span_for_repair(tid) {
                    self.remove_bridge_scaffolding(sid);
                }
            }
        }
    }

    /// C++ `DozerAIUpdate` / `WorkerAIUpdate::aiDoCommand` head
    /// (DozerAIUpdate.cpp:2326, WorkerAIUpdate.cpp:946):
    /// every command clears `MODELCONDITION_ACTIVELY_CONSTRUCTING`.
    pub fn dozer_clear_actively_constructing_on_command(&mut self, dozer_id: ObjectId) {
        if let Some(obj) = self.objects.get_mut(&dozer_id) {
            if obj.is_kind_of(KindOf::Dozer) || obj.is_kind_of(KindOf::Worker) {
                obj.set_actively_constructing(false);
            }
        }
    }

    /// C++ `aiDoCommand` default arm (DozerAIUpdate.cpp:2386-2387,
    /// WorkerAIUpdate.cpp:990-991): `CMD_FROM_PLAYER` cancels
    /// `getCurrentTask()` so idle `isBuildMostImportant` does not
    /// auto-resume an interrupted scaffold. Repair/ResumeConstruction
    /// do not call this. Parked pending slots stay when current is
    /// invalid (AI move-away). Does not clear `builder_id`.
    pub fn dozer_cancel_current_task_from_player(&mut self, dozer_id: ObjectId) {
        let Some(obj) = self.objects.get(&dozer_id) else {
            return;
        };
        if !obj.is_alive() || !(obj.is_kind_of(KindOf::Dozer) || obj.is_kind_of(KindOf::Worker)) {
            return;
        }
        let current_is_repair = matches!(obj.ai_state, AIState::Repairing);
        let current_is_build = matches!(obj.ai_state, AIState::Constructing);
        if !current_is_repair && !current_is_build {
            return;
        }
        self.dozer_internal_task_complete(dozer_id, current_is_repair);
        if let Some(obj) = self.objects.get_mut(&dozer_id) {
            obj.set_actively_constructing(false);
        }
    }

    fn dozer_most_recent_pending_task(&self, dozer_id: ObjectId) -> Option<(bool, ObjectId)> {
        let obj = self.objects.get(&dozer_id)?;
        let mut best: Option<(u32, bool, ObjectId)> = None;
        if let Some(tid) = obj.dozer_task_build_target {
            let frame = obj.dozer_task_build_order_frame;
            if best.is_none_or(|(f, _, _)| frame > f) {
                best = Some((frame, false, tid));
            }
        }
        if let Some(tid) = obj.dozer_task_repair_target {
            let frame = obj.dozer_task_repair_order_frame;
            if best.is_none_or(|(f, _, _)| frame > f) {
                best = Some((frame, true, tid));
            }
        }
        best.map(|(_, is_repair, tid)| (is_repair, tid))
    }

    /// C++ idle `isBuildMostImportant` / `isRepairMostImportant` (DozerAIUpdate.cpp:1314).
    /// Call only while the dozer is Idle. Returns true if a pending task was resumed.
    pub fn dozer_idle_resume_pending_build(&mut self, dozer_id: ObjectId) -> bool {
        if self.worker_is_acting_as_supply_truck(dozer_id) {
            return false;
        }
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
        // C++ idleConditions: isBuildMostImportant → BUILD, isRepairMostImportant → REPAIR.
        let (dozer_pos, st_pos, st_radius, airborne, stored_dock) = {
            let d = self.objects.get(&dozer_id);
            let dpos = d.map(|d| d.get_position()).unwrap_or(glam::Vec3::ZERO);
            let airborne =
                d.is_some_and(|d| d.is_kind_of(KindOf::Aircraft) || d.status.airborne_target);
            let stored = d.and_then(|d| d.dozer_dock_action);
            let (spos, srad) = self
                .objects
                .get(&tid)
                .map(|s| (s.get_position(), s.selection_radius))
                .unwrap_or((glam::Vec3::ZERO, 0.0));
            (dpos, spos, srad, airborne, stored)
        };
        if is_repair {
            let approach = self.find_good_build_or_repair_position(
                dozer_pos,
                st_pos,
                st_radius,
                airborne,
                airborne.then_some(tid),
                Some(dozer_id),
            );
            if let Some(dozer) = self.objects.get_mut(&dozer_id) {
                dozer.target = Some(tid);
                dozer.set_ai_state(AIState::Repairing);
                dozer.idle_since_frame = 0;
            }
            self.path_approach_with_state_ignoring(
                dozer_id,
                approach,
                AIState::Repairing,
                Some(tid),
            );
            return true;
        }
        // C++ idleConditions → DOZER_PRIMARY_BUILD. This is our parked slot,
        // not a player resume (ActionManager::canResumeConstructionOf).
        let snapped = stored_dock.unwrap_or_else(|| {
            self.find_good_build_or_repair_position(
                dozer_pos,
                st_pos,
                st_radius,
                airborne,
                airborne.then_some(tid),
                Some(dozer_id),
            )
        });
        if let Some(dozer) = self.objects.get_mut(&dozer_id) {
            dozer.target = Some(tid);
            dozer.set_ai_state(AIState::Constructing);
            dozer.idle_since_frame = 0;
            if dozer.dozer_dock_action.is_none() {
                dozer.dozer_dock_action = Some(snapped);
            }
        }

        let approach = snapped;
        self.path_approach_with_state_ignoring(
            dozer_id,
            approach,
            AIState::Constructing,
            Some(tid),
        );
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
            logic
                .host_object(pid)
                .is_some_and(|pax| pax.is_alive() && pax.is_kind_of(KindOf::Infantry))
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
        GameMessageType::SetBeaconText(_, text) => {
            CommandType::SetBeaconText { text: text.clone() }
        }
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
        assert!(
            !logic
                .get_player(0)
                .unwrap()
                .logical_retaliation_mode_enabled
        );
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
        assert!(
            logic
                .get_player(0)
                .unwrap()
                .logical_retaliation_mode_enabled
        );
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
        assert!(
            logic
                .get_player(0)
                .unwrap()
                .logical_retaliation_mode_enabled
        );
    }

    #[test]
    fn self_destruct_transfers_to_living_ally() {
        let mut logic = GameLogic::new();
        let mut p0 = Player::new(0, Team::USA, "P0", true);
        let mut p1 = Player::new(1, Team::USA, "P1", false);
        p0.alliance_team = 1;
        p1.alliance_team = 1;
        p0.resources.supplies = 4_000;
        p1.resources.supplies = 1_000;
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
        assert_eq!(logic.get_player(0).unwrap().effective_supplies(), 0);
        assert_eq!(logic.get_player(1).unwrap().effective_supplies(), 5_000);
    }

    #[test]
    fn self_destruct_without_ally_wipes_cash() {
        let mut logic = GameLogic::new();
        let mut p0 = Player::new(0, Team::USA, "P0", true);
        p0.resources.supplies = 2_500;
        logic.get_players_mut().insert(0, p0);
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::SelfDestruct {
                    transfer_to_ally: false,
                },
                vec![],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        assert!(!logic.get_player(0).unwrap().is_alive);
        assert_eq!(logic.get_player(0).unwrap().effective_supplies(), 0);
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
        let mine_pos = logic
            .player_command_center_position(0)
            .expect("own CC pose");
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
                CommandType::SetBeaconText { text: "go".into() },
                vec![id],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        assert_eq!(host_beacon_caption(id).as_deref(), Some("go"));
    }

    #[test]
    fn enemy_place_hides_drawable_and_skips_host_beacons() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "Local", true));
        let mut enemy = Player::new(1, Team::GLA, "Enemy", false);
        enemy.alliance_team = 2;
        logic.get_players_mut().insert(1, enemy);
        logic.get_players_mut().get_mut(&0).unwrap().alliance_team = 1;
        let result = CommandExecutor::new(&mut logic, 1)
            .execute_command(command(
                1,
                CommandType::PlaceBeacon {
                    location: Vec3::new(30.0, 0.0, 40.0),
                    text: String::new(),
                },
                vec![],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        let id = logic
            .host_objects()
            .iter()
            .find(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
            .map(|(id, _)| *id)
            .expect("beacon");
        assert!(host_beacon_is_hidden(id));
        assert!(logic.host_object(id).unwrap().drawable_hidden);
        assert!(
            logic.host_beacons().is_empty(),
            "enemy hide must not freeze onto host_beacons"
        );
    }

    #[test]
    fn remove_beacon_empty_selection_is_noop_and_silent() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::PlaceBeacon {
                    location: Vec3::new(8.0, 0.0, 9.0),
                    text: String::new(),
                },
                vec![],
            ))
            .unwrap();
        let before = logic
            .host_objects()
            .values()
            .filter(|o| o.template_name.to_ascii_lowercase().contains("beacon"))
            .count();
        let audio_before = logic.queued_audio_events.len();
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(0, CommandType::RemoveBeacon, vec![]))
            .expect("exec");
        assert_eq!(result, CommandResult::InvalidCommand);
        let after = logic
            .host_objects()
            .values()
            .filter(|o| o.template_name.to_ascii_lowercase().contains("beacon"))
            .count();
        assert_eq!(before, after);
        assert_eq!(logic.queued_audio_events.len(), audio_before);
    }

    #[test]
    fn remove_selected_beacon_is_silent() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::PlaceBeacon {
                    location: Vec3::new(8.0, 0.0, 9.0),
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
        let audio_before = logic.queued_audio_events.len();
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(0, CommandType::RemoveBeacon, vec![id]))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        assert_eq!(logic.queued_audio_events.len(), audio_before);
        assert!(logic.host_object(id).is_none() || !logic.host_object(id).unwrap().is_alive());
    }

    #[test]
    fn beacon_client_update_pulses_after_frequency() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::PlaceBeacon {
                    location: Vec3::new(11.0, 0.0, 12.0),
                    text: String::new(),
                },
                vec![],
            ))
            .unwrap();
        tick_live_beacon_client_updates(&mut logic);
        logic.frame = logic.frame.saturating_add(31);
        tick_live_beacon_client_updates(&mut logic);
        let radar_system = game_engine::common::system::radar::get_radar_system();
        let radar = radar_system.read().expect("radar");
        assert!(
            radar.get_active_events().iter().any(|e| e.event_type
                == game_engine::common::system::radar::RadarEventType::BeaconPulse
                || e.event_type == game_engine::common::system::radar::RadarEventType::Information),
            "visible beacon must pulse or have place INFORMATION"
        );
    }

    #[test]
    fn place_beacon_button_greys_at_max() {
        let mut logic = GameLogic::new();
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        assert!(host_local_player_can_place_beacon(&logic, 0));
        let max = host_max_beacons_per_player().max(1);
        for i in 0..max {
            CommandExecutor::new(&mut logic, 0)
                .execute_command(command(
                    0,
                    CommandType::PlaceBeacon {
                        location: Vec3::new(i as f32 * 5.0, 0.0, 0.0),
                        text: String::new(),
                    },
                    vec![],
                ))
                .unwrap();
        }
        assert!(!host_local_player_can_place_beacon(&logic, 0));
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
        logic.templates.insert("DozerParkBuild".into(), dozer_tpl);
        let mut bld = ThingTemplate::new("ScaffoldParkBuild");
        bld.add_kind_of(KindOf::Structure).set_health(500.0);
        logic
            .templates
            .insert("ScaffoldParkBuild".into(), bld.clone());
        logic.templates.insert("DamagedParkBuild".into(), bld);
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

    #[test]
    fn same_frame_build_repair_tie_resumes_build() {
        // hq-ylhfz: C++ getMostRecentCommand uses `order_frame > mostRecentFrame`,
        // so BUILD (walked first) wins a same-frame BUILD+REPAIR tie.
        use crate::game_logic::ThingTemplate;
        let mut logic = GameLogic::new();
        logic.frame = 10;
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        let mut dozer_tpl = ThingTemplate::new("DozerSameFrameTie");
        dozer_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .set_health(300.0);
        logic
            .templates
            .insert("DozerSameFrameTie".into(), dozer_tpl);
        let mut bld = ThingTemplate::new("ScaffoldSameFrameTie");
        bld.add_kind_of(KindOf::Structure).set_health(500.0);
        logic
            .templates
            .insert("ScaffoldSameFrameTie".into(), bld.clone());
        logic.templates.insert("DamagedSameFrameTie".into(), bld);
        let dozer = logic
            .create_object_for_player("DozerSameFrameTie", 0, Vec3::ZERO)
            .expect("dozer");
        let scaffold = logic
            .create_object_for_player("ScaffoldSameFrameTie", 0, Vec3::new(20.0, 0.0, 0.0))
            .expect("scaffold");
        let damaged = logic
            .create_object_for_player("DamagedSameFrameTie", 0, Vec3::new(40.0, 0.0, 0.0))
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
        logic.dozer_new_task_build(dozer, scaffold);
        logic.dozer_new_task_repair(dozer, damaged);
        {
            let dz = logic.host_object(dozer).expect("dz");
            assert_eq!(
                dz.dozer_task_build_order_frame,
                dz.dozer_task_repair_order_frame
            );
            assert_eq!(dz.dozer_task_build_target, Some(scaffold));
            assert_eq!(dz.dozer_task_repair_target, Some(damaged));
        }
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.set_target(None);
            dz.set_ai_state(AIState::Idle);
        }
        assert!(
            logic.dozer_idle_resume_pending_build(dozer),
            "hq-ylhfz: same-frame BUILD+REPAIR must resume BUILD"
        );
        let dz = logic.host_object(dozer).expect("dz");
        assert_eq!(dz.ai_state, AIState::Constructing);
        assert_eq!(dz.target, Some(scaffold));
    }

    #[test]
    fn idle_resumes_parked_repair_when_most_recent() {
        // hq-ja2nm: C++ isRepairMostImportant resumes the parked REPAIR slot.
        use crate::game_logic::ThingTemplate;
        let mut logic = GameLogic::new();
        logic.frame = 10;
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        let mut dozer_tpl = ThingTemplate::new("DozerIdleRepair");
        dozer_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .set_health(300.0);
        logic.templates.insert("DozerIdleRepair".into(), dozer_tpl);
        let mut bld = ThingTemplate::new("DamagedIdleRepair");
        bld.add_kind_of(KindOf::Structure).set_health(500.0);
        logic.templates.insert("DamagedIdleRepair".into(), bld);
        let dozer = logic
            .create_object_for_player("DozerIdleRepair", 0, Vec3::ZERO)
            .expect("dozer");
        let damaged = logic
            .create_object_for_player("DamagedIdleRepair", 0, Vec3::new(40.0, 0.0, 0.0))
            .expect("damaged");
        {
            let dmg = logic.host_object_mut(damaged).expect("dmg");
            let _ = dmg.take_damage(200.0);
        }
        logic.dozer_new_task_repair(dozer, damaged);
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.set_target(None);
            dz.set_ai_state(AIState::Idle);
        }
        assert!(
            logic.dozer_idle_resume_pending_build(dozer),
            "hq-ja2nm: idle isRepairMostImportant must resume parked REPAIR"
        );
        let dz = logic.host_object(dozer).expect("dz");
        assert_eq!(dz.ai_state, AIState::Repairing);
        assert_eq!(dz.target, Some(damaged));
        assert_eq!(dz.dozer_task_repair_target, Some(damaged));
    }

    #[test]
    fn idle_resumes_repair_parked_by_newer_build() {
        // hq-ja2nm: BUILD parks REPAIR; after BUILD is dropped, idle resumes REPAIR.
        use crate::game_logic::ThingTemplate;
        let mut logic = GameLogic::new();
        logic.frame = 8;
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        let mut dozer_tpl = ThingTemplate::new("DozerParkedRepair");
        dozer_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .set_health(300.0);
        logic
            .templates
            .insert("DozerParkedRepair".into(), dozer_tpl);
        let mut bld = ThingTemplate::new("ScaffoldParkedRepair");
        bld.add_kind_of(KindOf::Structure).set_health(500.0);
        logic
            .templates
            .insert("ScaffoldParkedRepair".into(), bld.clone());
        logic.templates.insert("DamagedParkedRepair".into(), bld);
        let dozer = logic
            .create_object_for_player("DozerParkedRepair", 0, Vec3::ZERO)
            .expect("dozer");
        let scaffold = logic
            .create_object_for_player("ScaffoldParkedRepair", 0, Vec3::new(20.0, 0.0, 0.0))
            .expect("scaffold");
        let damaged = logic
            .create_object_for_player("DamagedParkedRepair", 0, Vec3::new(40.0, 0.0, 0.0))
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
        logic.dozer_new_task_repair(dozer, damaged);
        logic.frame = 9;
        logic.dozer_new_task_build(dozer, scaffold);
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.set_target(None);
            dz.set_ai_state(AIState::Idle);
        }
        assert!(
            logic.dozer_idle_resume_pending_build(dozer),
            "newer BUILD still wins getMostRecentCommand"
        );
        assert_eq!(
            logic.host_object(dozer).unwrap().ai_state,
            AIState::Constructing
        );
        logic.dozer_internal_task_complete(dozer, false);
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.set_target(None);
            dz.set_ai_state(AIState::Idle);
        }
        assert!(
            logic.dozer_idle_resume_pending_build(dozer),
            "hq-ja2nm: after BUILD clears, idle must resume parked REPAIR"
        );
        let dz = logic.host_object(dozer).expect("dz");
        assert_eq!(dz.ai_state, AIState::Repairing);
        assert_eq!(dz.target, Some(damaged));
    }

    #[test]
    fn player_stop_cancels_current_build_so_idle_does_not_resume() {
        // hq-msoee: C++ Worker/Dozer aiDoCommand default arm cancels
        // getCurrentTask() on CMD_FROM_PLAYER Stop/Move/Attack/Dock.
        use crate::game_logic::ThingTemplate;
        let mut logic = GameLogic::new();
        logic.frame = 10;
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        let mut dozer_tpl = ThingTemplate::new("DozerCancelBuild");
        dozer_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .set_health(300.0);
        logic.templates.insert("DozerCancelBuild".into(), dozer_tpl);
        let mut bld = ThingTemplate::new("ScaffoldCancelBuild");
        bld.add_kind_of(KindOf::Structure).set_health(500.0);
        logic.templates.insert("ScaffoldCancelBuild".into(), bld);
        let dozer = logic
            .create_object_for_player("DozerCancelBuild", 0, Vec3::ZERO)
            .expect("dozer");
        let scaffold = logic
            .create_object_for_player("ScaffoldCancelBuild", 0, Vec3::new(20.0, 0.0, 0.0))
            .expect("scaffold");
        {
            let sc = logic.host_object_mut(scaffold).expect("sc");
            sc.status.under_construction = true;
            sc.builder_id = Some(dozer);
        }
        logic.dozer_new_task_build(dozer, scaffold);
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.target = Some(scaffold);
            dz.set_ai_state(AIState::Constructing);
            dz.set_actively_constructing(true);
        }
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(0, CommandType::Stop, vec![dozer]))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        {
            let dz = logic.host_object(dozer).expect("dz");
            assert_eq!(
                dz.dozer_task_build_target, None,
                "hq-msoee: player Stop must cancel current BUILD slot"
            );
        }
        {
            let sc = logic.host_object(scaffold).expect("sc");
            assert_eq!(
                sc.builder_id,
                Some(dozer),
                "C++ cancelTask does not clear builder_id"
            );
            assert!(sc.status.under_construction);
        }
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.set_ai_state(AIState::Idle);
        }
        assert!(
            !logic.dozer_idle_resume_pending_build(dozer),
            "hq-msoee: cancelled BUILD must not auto-resume"
        );
    }

    #[test]
    fn execute_build_records_build_slot_and_docks_off_center() {
        // hq-gkpuk: C++ construct:1717 newTask BUILD.
        // hq-6gy32: C++ findGoodBuildOrRepairPosition half majorRadius + ignoreObstacle.
        use crate::game_logic::ThingTemplate;
        let mut logic = GameLogic::new();
        logic.frame = 12;
        let mut player = Player::new(0, Team::USA, "P0", true);
        player.resources.supplies = 100_000;
        logic.get_players_mut().insert(0, player);
        let mut dozer_tpl = ThingTemplate::new("AmericaVehicleDozerSlot");
        dozer_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .set_health(300.0);
        logic
            .templates
            .insert("AmericaVehicleDozerSlot".into(), dozer_tpl);
        let mut bld = ThingTemplate::new("AmericaBarracksSlot");
        bld.add_kind_of(KindOf::Structure)
            .set_cost(50, 0)
            .set_health(500.0);
        logic.templates.insert("AmericaBarracksSlot".into(), bld);
        let dozer = logic
            .create_object_for_player("AmericaVehicleDozerSlot", 0, Vec3::new(200.0, 0.0, 0.0))
            .expect("dozer");
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.selection_radius = 8.0;
        }
        let site = Vec3::new(0.0, 0.0, 0.0);
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::Build {
                    template_name: "AmericaBarracksSlot".into(),
                    location: site,
                },
                vec![dozer],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        let scaffold = logic
            .host_objects()
            .iter()
            .find(|(_, o)| o.template_name == "AmericaBarracksSlot")
            .map(|(id, _)| *id)
            .expect("scaffold");
        {
            let dz = logic.host_object(dozer).expect("dz");
            assert_eq!(
                dz.dozer_task_build_target,
                Some(scaffold),
                "hq-gkpuk: new construct must write BUILD slot"
            );
            assert!(dz.dozer_task_build_order_frame >= 12);
            assert_eq!(dz.target, Some(scaffold));
            assert_eq!(dz.ai_state, AIState::Constructing);
            let dest = dz
                .movement
                .target_position
                .or_else(|| dz.movement.path.last().copied())
                .unwrap_or(site);
            let pad = logic.host_object(scaffold).unwrap().get_position();
            let dx = dest.x - pad.x;
            let dz_ = dest.z - pad.z;
            assert!(
                dx * dx + dz_ * dz_ > 1.0,
                "hq-6gy32: dozer must dock off pad center, dest={dest:?} pad={pad:?}"
            );
        }
        {
            let sc = logic.host_object(scaffold).expect("sc");
            assert_eq!(sc.builder_id, Some(dozer));
            assert!(sc.status.under_construction);
        }
        if let Some(dz) = logic.host_object_mut(dozer) {
            dz.set_target(None);
            dz.set_ai_state(AIState::Idle);
            dz.set_position(Vec3::new(400.0, 0.0, 0.0));
        }
        assert!(
            logic.dozer_idle_resume_pending_build(dozer),
            "hq-gkpuk: idle isBuildMostImportant must resume a brand-new pad"
        );
    }

    #[test]
    fn gather_cancels_build_and_blocks_idle_resume_while_supply() {
        // hq-5nio2: gather enters exclusive AS_SUPPLY_TRUCK.
        use crate::game_logic::{SupplyTruckState, ThingTemplate};
        let mut logic = GameLogic::new();
        logic.frame = 22;
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::GLA, "P0", true));
        let mut worker_tpl = ThingTemplate::new("GLAWorkerGatherCancel");
        worker_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .add_kind_of(KindOf::Harvester)
            .set_health(100.0);
        logic
            .templates
            .insert("GLAWorkerGatherCancel".into(), worker_tpl);
        let mut pile = ThingTemplate::new("SupplyWarehouseGather");
        pile.add_kind_of(KindOf::Harvestable)
            .add_kind_of(KindOf::Structure)
            .set_health(200.0);
        logic.templates.insert("SupplyWarehouseGather".into(), pile);
        let mut bld = ThingTemplate::new("ScaffoldGatherCancel");
        bld.add_kind_of(KindOf::Structure).set_health(500.0);
        logic.templates.insert("ScaffoldGatherCancel".into(), bld);
        let worker = logic
            .create_object_for_player("GLAWorkerGatherCancel", 0, Vec3::ZERO)
            .expect("worker");
        let warehouse = logic
            .create_object_for_player("SupplyWarehouseGather", 0, Vec3::new(40.0, 0.0, 0.0))
            .expect("wh");
        let scaffold = logic
            .create_object_for_player("ScaffoldGatherCancel", 0, Vec3::new(80.0, 0.0, 0.0))
            .expect("scaffold");
        {
            let sc = logic.host_object_mut(scaffold).expect("sc");
            sc.status.under_construction = true;
            sc.builder_id = Some(worker);
        }
        logic.dozer_new_task_build(worker, scaffold);
        if let Some(w) = logic.host_object_mut(worker) {
            w.target = Some(scaffold);
            w.set_ai_state(AIState::Constructing);
        }
        let result = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::Gather {
                    target_id: warehouse,
                },
                vec![worker],
            ))
            .expect("exec");
        assert_eq!(result, CommandResult::Success);
        {
            let w = logic.host_object(worker).expect("w");
            assert!(
                w.dozer_task_build_target.is_none(),
                "hq-5nio2: gather must cancel current BUILD"
            );
            assert_eq!(w.ai_state, AIState::Gathering);
        }
        if let Some(w) = logic.host_object_mut(worker) {
            w.set_ai_state(AIState::Idle);
            w.supply_truck_state = SupplyTruckState::Regrouping;
        }
        assert!(
            !logic.dozer_idle_resume_pending_build(worker),
            "hq-5nio2: AS_SUPPLY_TRUCK must not run idle dozer resume"
        );
    }

    #[test]
    fn new_task_snaps_action_dock_off_pad_via_find_position_around() {
        // hq-z2plo: C++ findGoodBuildOrRepairPosition runs findPositionAround
        // so the stored ACTION dock is not the in-pad half-radius seed.
        use crate::game_logic::ThingTemplate;
        let mut logic = GameLogic::new();
        logic.frame = 4;
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::USA, "P0", true));
        let mut dozer_tpl = ThingTemplate::new("AmericaVehicleDozerSnap");
        dozer_tpl
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Dozer)
            .set_health(250.0);
        logic
            .templates
            .insert("AmericaVehicleDozerSnap".into(), dozer_tpl);
        let mut pad_tpl = ThingTemplate::new("AmericaWarFactorySnap");
        pad_tpl.add_kind_of(KindOf::Structure).set_health(1500.0);
        logic
            .templates
            .insert("AmericaWarFactorySnap".into(), pad_tpl);
        let dozer = logic
            .create_object_for_player(
                "AmericaVehicleDozerSnap",
                0,
                glam::Vec3::new(200.0, 0.0, 0.0),
            )
            .expect("dozer");
        let pad = logic
            .create_object_for_player("AmericaWarFactorySnap", 0, glam::Vec3::ZERO)
            .expect("pad");
        if let Some(p) = logic.host_object_mut(pad) {
            p.selection_radius = 80.0;
            p.status.under_construction = true;
        }
        if let Some(d) = logic.host_object_mut(dozer) {
            d.selection_radius = 8.0;
        }
        logic.dozer_new_task_build(dozer, pad);
        let dock = logic
            .host_object(dozer)
            .and_then(|d| d.dozer_dock_action)
            .expect("ACTION dock");
        let seed = crate::game_logic::host_repair::dozer_repair_approach_position(
            glam::Vec3::new(200.0, 0.0, 0.0),
            glam::Vec3::ZERO,
            80.0,
        );
        let dist = (dock.x * dock.x + dock.z * dock.z).sqrt();
        assert!(
            dist > 80.0,
            "findPositionAround must leave the pad, dock={dock:?} seed={seed:?}"
        );
        assert!(
            (dock - seed).length() > 1.0,
            "stored dock must not stay on the raw half-radius seed"
        );
    }

    #[test]
    fn mine_clear_order_keeps_boxes_until_already_attacking() {
        // hq-6je29: initial clear-mines does not drop; mid-attack command does.
        use crate::game_logic::ThingTemplate;
        let mut logic = GameLogic::new();
        logic.frame = 8;
        logic
            .get_players_mut()
            .insert(0, Player::new(0, Team::GLA, "P0", true));
        let mut worker_tpl = ThingTemplate::new("GLAWorkerMineBox");
        worker_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .add_kind_of(KindOf::Harvester)
            .set_health(100.0);
        logic
            .templates
            .insert("GLAWorkerMineBox".into(), worker_tpl);
        let worker = logic
            .create_object_for_player("GLAWorkerMineBox", 0, Vec3::ZERO)
            .expect("worker");
        if let Some(w) = logic.host_object_mut(worker) {
            w.set_stored_supplies(75);
            w.set_weapon_set_mine_clearing_detail(true);
        }
        logic.drop_worker_supply_boxes_for_mine_clear(worker);
        assert_eq!(
            logic.host_object(worker).unwrap().stored_resources.supplies,
            75,
            "hq-6je29: initial order is not isClearingMines"
        );
        if let Some(w) = logic.host_object_mut(worker) {
            w.set_ai_state(AIState::Attacking);
            w.status.attacking = true;
        }
        logic.drop_worker_supply_boxes_for_mine_clear(worker);
        assert_eq!(
            logic.host_object(worker).unwrap().stored_resources.supplies,
            0,
            "hq-6je29: mid-attack command drops boxes"
        );
    }
}
