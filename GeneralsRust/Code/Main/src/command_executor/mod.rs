//! Wave 955: CommandExecutor host_object seal.
//! Wave 958: host_object dual-read seal (tests + residual).
#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unreachable_patterns
)]
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

/// Command executor that processes game commands.
///
/// Wave 955: host_object/host_objects authority dual-read seal (no presentation dual-read).
pub struct CommandExecutor<'a> {
    /// Reference to game logic for object manipulation
    game_logic: &'a mut GameLogic,

    /// Current player executing commands
    current_player_id: u32,

    /// Track command execution metrics
    commands_executed: usize,
    commands_failed: usize,
    /// Exact subset accepted by the most recent Gather command. This is kept
    /// separate from generic command success so Main can bind physical mouse
    /// provenance only to carriers whose path assignment actually succeeded.
    accepted_gather_carrier_ids: Vec<ObjectId>,
}

mod abilities;
mod attack;
mod cheat_debug;
mod command_button;
mod construct;
mod group;
mod leftover;
mod movement;
mod produce;
mod selection;
mod sell_upgrade;
mod special_power;
mod transport;
mod validate;

pub use leftover::{
    host_beacon_caption, host_beacon_is_hidden, host_beacon_position_is_hidden,
    host_local_player_can_place_beacon, take_leftover_dispatch_commands_from_common_stream,
    tick_live_beacon_client_updates,
};

#[cfg(test)]
mod tests;

impl<'a> CommandExecutor<'a> {
    /// Create a new command executor with game logic reference
    pub fn new(game_logic: &'a mut GameLogic, player_id: u32) -> Self {
        Self {
            game_logic,
            current_player_id: player_id,
            commands_executed: 0,
            commands_failed: 0,
            accepted_gather_carrier_ids: Vec::new(),
        }
    }

    /// Consume only the carrier IDs accepted by the current Gather execution.
    /// Non-Gather commands and rejected Gather commands return an empty vector.
    pub(crate) fn take_accepted_gather_carrier_ids(&mut self) -> Vec<ObjectId> {
        std::mem::take(&mut self.accepted_gather_carrier_ids)
    }

    fn player_team(&self, player_id: u32) -> Team {
        self.game_logic
            .get_player(player_id)
            .map(|player| player.team)
            .unwrap_or_else(|| Team::from_player_id(player_id))
    }

    /// Execute a game command and return result
    pub fn execute_command(&mut self, command: GameCommand) -> Result<CommandResult, String> {
        self.accepted_gather_carrier_ids.clear();
        debug!(
            "Executing command {:?} for player {}",
            command.command_type, command.player_id
        );
        self.current_player_id = command.player_id;

        // C++ Player.cpp:708-724 posts MSG_ENABLE_RETALIATION_MODE once/sec
        // when TheGlobalData->m_clientRetaliationModeEnabled differs from the
        // logical flag. Apply the same setLogicalRetaliationModeEnabled here
        // so Auto-Retaliate reaches the live host without GameNetwork.
        if !matches!(
            command.command_type,
            CommandType::EnableRetaliationMode { .. }
        ) {
            self.sync_logical_retaliation_from_client();
        }

        // Validate player ownership
        if !self.validate_player_ownership(&command) {
            self.commands_failed += 1;
            return Ok(CommandResult::InvalidCommand);
        }
        // C++ HintSpy.cpp:91-96 — MSG_DO_MOVETO / ATTACKMOVETO / FORCEMOVETO /
        // ADD_WAYPOINT create the destination marker as the command is issued.
        self.create_live_move_hint(&command);
        // C++ Dozer/Worker aiDoCommand (DozerAIUpdate.cpp:2326, 2386-2387):
        // any command clears ACTIVELY_CONSTRUCTING; CMD_FROM_PLAYER default
        // arm (not Repair/ResumeConstruction) cancels getCurrentTask().
        self.apply_dozer_ai_do_command(&command);

        let result = match &command.command_type {
            // Movement commands
            CommandType::Move { destination } => {
                self.execute_move(&command.selected_units, *destination)
            }
            CommandType::MoveTo {
                destination,
                waypoints,
            } => self.execute_move_to(&command.selected_units, *destination, waypoints),
            CommandType::DoSalvage { destination } => {
                // C++ CommandXlat.cpp:1921-1937 / 423-431 — move, then VoiceSalvage.
                self.execute_salvage(&command.selected_units, *destination)
            }
            CommandType::AttackMoveTo {
                destination,
                max_shots,
            } => self.execute_attack_move(&command.selected_units, *destination, *max_shots),
            CommandType::ForceMoveTo { destination } => {
                self.execute_force_move(&command.selected_units, *destination)
            }
            CommandType::AddWaypoint { destination } => {
                self.execute_add_waypoint(&command.selected_units, *destination)
            }

            // Combat commands
            CommandType::Attack { target_id } => {
                self.execute_attack(&command.selected_units, *target_id)
            }
            CommandType::AttackObject { target_id } => {
                self.execute_attack_object(&command.selected_units, *target_id)
            }
            CommandType::ForceAttackObject { target_id } => {
                self.execute_force_attack(&command.selected_units, *target_id)
            }
            CommandType::ForceAttackGround { location } => {
                self.execute_attack_ground(&command.selected_units, Some(*location), -1)
            }
            CommandType::AttackPosition {
                location,
                max_shots,
            } => self.execute_attack_ground(&command.selected_units, *location, *max_shots),
            CommandType::Stop => self.execute_stop(&command.selected_units),
            CommandType::Guard { target, mode } => {
                self.execute_guard(&command.selected_units, target, *mode)
            }
            CommandType::Patrol => self.execute_patrol(&command.selected_units),
            CommandType::AttitudeSleep => self.execute_set_attitude(
                &command.selected_units,
                crate::game_logic::host_strategy_center::HostAiAttitude::Sleep,
            ),
            CommandType::AttitudePassive => self.execute_set_attitude(
                &command.selected_units,
                crate::game_logic::host_strategy_center::HostAiAttitude::Passive,
            ),
            CommandType::AttitudeNormal => self.execute_set_attitude(
                &command.selected_units,
                crate::game_logic::host_strategy_center::HostAiAttitude::Normal,
            ),
            CommandType::AttitudeAggressive => self.execute_set_attitude(
                &command.selected_units,
                crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive,
            ),
            CommandType::Scatter => self.execute_scatter(&command.selected_units),
            CommandType::TightenToPosition { destination } => {
                self.execute_tighten_to_position(&command.selected_units, *destination)
            }
            CommandType::AttackTeam { team, max_shots } => {
                self.execute_attack_team(&command.selected_units, *team, *max_shots)
            }
            CommandType::OverrideSpecialPowerDestination { location } => {
                self.execute_override_special_power_destination(&command.selected_units, *location)
            }
            CommandType::SetWeaponSetFlag { flag, enabled } => {
                self.execute_set_weapon_set_flag(&command.selected_units, *flag, *enabled)
            }
            CommandType::FollowWaypointPath {
                waypoints,
                exact,
                as_team,
            } => self.execute_follow_waypoint_path(
                &command.selected_units,
                waypoints,
                *exact,
                *as_team,
            ),
            CommandType::AttackFollowWaypointPath {
                waypoints,
                exact,
                as_team,
            } => self.execute_attack_follow_waypoint_path(
                &command.selected_units,
                waypoints,
                *exact,
                *as_team,
            ),
            CommandType::DoCommandButtonUsingWaypoints { button, waypoints } => self
                .execute_do_command_button_using_waypoints(
                    &command.selected_units,
                    button,
                    waypoints,
                ),
            CommandType::Surrender { surrendered } => {
                self.execute_surrender(&command.selected_units, *surrendered)
            }
            CommandType::DoCommandButton { button } => {
                self.execute_do_command_button(&command.selected_units, button, None, None)
            }
            CommandType::DoCommandButtonAtPosition { button, location } => self
                .execute_do_command_button(&command.selected_units, button, Some(*location), None),
            CommandType::DoCommandButtonAtObject { button, target } => {
                self.execute_do_command_button(&command.selected_units, button, None, Some(*target))
            }
            CommandType::ExecuteRailedTransport => {
                self.execute_railed_transport(&command.selected_units)
            }
            CommandType::Deploy => self.execute_deploy(&command.selected_units),
            CommandType::Gather { target_id } => {
                self.execute_gather(&command.selected_units, *target_id)
            }

            // Building and construction
            CommandType::Build {
                template_name,
                location,
            } => self.execute_build(&command.selected_units, template_name, *location, 0.0),
            CommandType::DozerConstruct {
                template_name,
                location,
                orientation,
            } => self.execute_dozer_construct(
                &command.selected_units,
                template_name,
                *location,
                *orientation,
            ),
            CommandType::DozerConstructLine {
                template_name,
                start,
                end,
            } => self.execute_dozer_line(&command.selected_units, template_name, *start, *end),
            CommandType::DozerCancelConstruct { object_id } => {
                self.execute_cancel_construction(*object_id, command.player_id)
            }
            CommandType::ResumeConstruction { target_id } => {
                self.execute_resume_construction(&command.selected_units, *target_id)
            }
            CommandType::Sell { object_id } => {
                // C++ MSG_SELL → currentlySelectedGroup->groupSell.
                if command.selected_units.is_empty() {
                    self.execute_sell(*object_id, command.player_id)
                } else {
                    self.execute_sell_selected(&command.selected_units, command.player_id)
                }
            }

            // Unit production
            CommandType::QueueUnitCreate {
                template_name,
                quantity,
            } => self.execute_queue_unit(&command.selected_units, template_name, *quantity),
            CommandType::CancelUnitCreate { template_name } => {
                self.execute_cancel_unit(&command.selected_units, template_name)
            }

            // Special abilities
            CommandType::DoSpecialPower { power_type, target } => {
                self.execute_special_power(&command.selected_units, power_type, target)
            }
            CommandType::DoWeapon {
                weapon_slot,
                max_shots_to_fire,
                target,
            } => self.execute_weapon(
                &command.selected_units,
                weapon_slot,
                *max_shots_to_fire,
                target,
            ),

            // Transport and container
            CommandType::Enter { target_id } => {
                self.execute_enter(&command.selected_units, *target_id)
            }
            CommandType::Exit => self.execute_exit(&command.selected_units),
            CommandType::Evacuate => self.execute_evacuate(&command.selected_units),
            CommandType::MoveToAndEvacuate {
                destination,
                and_exit,
            } => {
                self.execute_move_to_and_evacuate(&command.selected_units, *destination, *and_exit)
            }
            CommandType::HackInternet => self.execute_hack_internet(&command.selected_units),
            CommandType::ReturnToBase => self.execute_return_to_base(&command.selected_units),
            CommandType::ReturnSupplies => self.execute_return_supplies(&command.selected_units),
            CommandType::ClearMines => self.execute_clear_mines(&command.selected_units),
            CommandType::SetMineClearingDetail { enabled } => {
                self.execute_set_mine_clearing_detail(&command.selected_units, *enabled)
            }
            CommandType::GoProne => self.execute_go_prone(&command.selected_units),
            CommandType::SetWeaponLock { slot, lock_type } => {
                self.execute_set_weapon_lock(&command.selected_units, *slot, *lock_type)
            }
            CommandType::ReleaseWeaponLock { lock_type } => {
                self.execute_release_weapon_lock(&command.selected_units, *lock_type)
            }
            CommandType::SetEmoticon {
                name,
                duration_frames,
            } => self.execute_set_emoticon(&command.selected_units, name, *duration_frames),
            CommandType::AttackArea {
                center,
                radius,
                polygon_name,
            } => self.execute_attack_area(
                &command.selected_units,
                *center,
                *radius,
                polygon_name.as_deref(),
            ),
            CommandType::Dock { target_id } => {
                self.execute_dock(&command.selected_units, *target_id)
            }
            CommandType::CombatDrop { target } => {
                self.execute_combat_drop(&command.selected_units, target)
            }

            // Utility commands
            CommandType::Repair { target_id } => {
                self.execute_repair(&command.selected_units, *target_id)
            }
            CommandType::GetRepaired { target_id } => {
                self.execute_get_repaired(&command.selected_units, *target_id)
            }
            CommandType::GetHealed { target_id } => {
                self.execute_get_healed(&command.selected_units, *target_id)
            }
            CommandType::SetRallyPoint { location } => {
                self.execute_set_rally_point(&command.selected_units, *location)
            }

            // Economy and upgrades
            CommandType::PurchaseScience { science_name } => {
                self.execute_purchase_science(command.player_id, science_name)
            }
            CommandType::QueueUpgrade { upgrade_name } => {
                self.execute_queue_upgrade(&command.selected_units, upgrade_name)
            }
            CommandType::CancelUpgrade { upgrade_name } => {
                self.execute_cancel_upgrade(&command.selected_units, upgrade_name)
            }

            // Special unit abilities
            CommandType::Hijack { target_id } => {
                self.execute_hijack(&command.selected_units, *target_id)
            }
            CommandType::Sabotage { target_id } => {
                self.execute_sabotage(&command.selected_units, *target_id)
            }
            CommandType::ConvertToCarbomb { target_id } => {
                self.execute_convert_carbomb(&command.selected_units, *target_id)
            }
            CommandType::CaptureBuilding { target_id } => {
                self.execute_capture_building(&command.selected_units, *target_id)
            }
            CommandType::SnipeVehicle { target_id } => {
                self.execute_snipe_vehicle(&command.selected_units, *target_id)
            }
            CommandType::PlantTimedDemoCharge { target_id } => {
                self.execute_plant_timed_demo_charge(&command.selected_units, *target_id)
            }
            CommandType::PlantRemoteDemoCharge { target_id } => {
                self.execute_plant_remote_demo_charge(&command.selected_units, *target_id)
            }
            CommandType::DetonateRemoteDemoCharges => {
                self.execute_detonate_remote_demo_charges(&command.selected_units)
            }
            CommandType::DemoTertiarySuicide => {
                self.execute_demo_tertiary_suicide(&command.selected_units)
            }
            CommandType::StealCashHack { target_id } => {
                self.execute_steal_cash_hack(&command.selected_units, *target_id)
            }
            CommandType::DisableVehicleHack { target_id } => {
                self.execute_disable_vehicle_hack(&command.selected_units, *target_id)
            }
            CommandType::HackerDisableBuilding { target_id } => {
                self.execute_hacker_disable_building(&command.selected_units, *target_id)
            }
            CommandType::DisguiseAsVehicle { target_id } => {
                self.execute_disguise_as_vehicle(&command.selected_units, *target_id)
            }
            CommandType::PlantBoobyTrap { target_id } => {
                self.execute_plant_booby_trap(&command.selected_units, *target_id)
            }
            CommandType::SwitchWeapons { slot } => {
                self.execute_switch_weapons(&command.selected_units, *slot)
            }
            CommandType::ToggleOvercharge => {
                self.execute_toggle_overcharge(&command.selected_units)
            }

            // Formation commands
            CommandType::CreateFormation => self.execute_create_formation(&command.selected_units),
            CommandType::Cheer => self.execute_cheer(&command.selected_units),

            // Other commands
            CommandType::PlaceBeacon { location, text } => {
                self.execute_place_beacon(command.player_id, *location, text)
            }
            CommandType::RemoveBeacon => {
                self.execute_remove_beacon(command.player_id, &command.selected_units)
            }
            CommandType::SetBeaconText { text } => {
                self.execute_set_beacon_text(command.player_id, &command.selected_units, text)
            }
            CommandType::EnableRetaliationMode {
                player_index,
                enabled,
            } => self.execute_enable_retaliation(*player_index, *enabled),
            CommandType::SelfDestruct { transfer_to_ally } => {
                self.execute_self_destruct(command.player_id, *transfer_to_ally)
            }
            CommandType::ViewRadarAt { position } => self.execute_view_radar_at(*position),

            // Selection commands
            CommandType::CreateSelectedGroup { create_new, units } => {
                self.execute_selection(command.player_id, *create_new, units)
            }
            CommandType::DestroySelectedGroup { team_id } => {
                self.execute_destroy_group(command.player_id, *team_id)
            }
            CommandType::RemoveFromSelectedGroup { units } => {
                self.execute_remove_from_selection(command.player_id, units)
            }
            CommandType::ViewLastRadarEvent => self.execute_view_last_radar_event(),
            CommandType::ViewCommandCenter => {
                // Center camera on the current player's command center, matching C++ quick-jump.
                self.execute_view_command_center()
            }

            CommandType::Invalid => self.execute_invalid_command(),
        };

        if result == CommandResult::Success {
            self.commands_executed += 1;
        } else {
            self.commands_failed += 1;
        }

        Ok(result)
    }

    // === Validation Helpers ===

    fn validate_player_ownership(&self, command: &GameCommand) -> bool {
        // Check if player owns all selected units
        for &unit_id in &command.selected_units {
            if let Some(unit) = self.game_logic.host_object(unit_id) {
                if unit.owner_player_id != Some(command.player_id) {
                    warn!(
                        "Player {} doesn't own unit {}",
                        command.player_id, unit_id.0
                    );
                    return false;
                }
            }
        }
        true
    }

    /// C++ HintSpy.cpp:91-96 / InGameUI::createMoveHint (InGameUI.cpp:2141).
    fn create_live_move_hint(&self, command: &GameCommand) {
        let destination = match &command.command_type {
            CommandType::Move { destination }
            | CommandType::MoveTo { destination, .. }
            | CommandType::AttackMoveTo { destination, .. }
            | CommandType::ForceMoveTo { destination }
            | CommandType::AddWaypoint { destination } => *destination,
            _ => return,
        };
        // C++ InGameUI.cpp:2152-2160 — single IMMOBILE selection suppresses the hint.
        if command.selected_units.len() == 1 {
            if let Some(obj) = self.game_logic.host_object(command.selected_units[0]) {
                if obj.is_kind_of(KindOf::Immobile) {
                    return;
                }
            }
        }
        #[cfg(feature = "game_client")]
        {
            let source_id = command.selected_units.first().map(|id| id.0).unwrap_or(0);
            let pos = game_client::message_stream::Coord3D::new(
                destination.x,
                destination.y,
                destination.z,
            );
            game_client::helpers::TheInGameUI::create_move_hint(pos.clone(), pos, source_id);
        }
        let _ = destination;
    }

    /// C++ `DozerAIUpdate::aiDoCommand` / `WorkerAIUpdate::aiDoCommand`.
    fn apply_dozer_ai_do_command(&mut self, command: &GameCommand) {
        if Self::player_command_clears_dozer_constructing(&command.command_type) {
            for &unit_id in &command.selected_units {
                self.game_logic
                    .dozer_clear_actively_constructing_on_command(unit_id);
            }
        }
        if Self::player_command_cancels_current_dozer_task(&command.command_type) {
            for &unit_id in &command.selected_units {
                self.game_logic
                    .dozer_cancel_current_task_from_player(unit_id);
            }
        }
        // C++ WorkerAIUpdate::aiDoCommand tail: drop boxes only if already
        // isClearingMines. Runs before the new order mutates attack state.
        if Self::player_command_clears_dozer_constructing(&command.command_type) {
            for &unit_id in &command.selected_units {
                self.game_logic
                    .drop_worker_supply_boxes_for_mine_clear(unit_id);
            }
        }
        // C++ WorkerStateMachine readyToLeave: player move/attack is ST_BUSY
        // then AS_DOZER. Live Idle must not re-enter Wanting.
        if Self::player_command_parks_worker_ferry(&command.command_type) {
            for &unit_id in &command.selected_units {
                self.game_logic
                    .worker_exit_supply_for_player_command(unit_id);
            }
        }
    }

    /// C++ `ownerNotDockingOrIdle` → ST_BUSY → `supplyTruckSubMachineReadyToLeave`.
    /// Stop stays Idle (workers omit setForceBusyState). Gather/Dock/Return
    /// stay AS_SUPPLY_TRUCK. Repair/Resume use `newTask` instead.
    fn player_command_parks_worker_ferry(command_type: &CommandType) -> bool {
        matches!(
            command_type,
            CommandType::Move { .. }
                | CommandType::MoveTo { .. }
                | CommandType::DoSalvage { .. }
                | CommandType::AttackMoveTo { .. }
                | CommandType::ForceMoveTo { .. }
                | CommandType::AddWaypoint { .. }
                | CommandType::Attack { .. }
                | CommandType::AttackObject { .. }
                | CommandType::ForceAttackObject { .. }
                | CommandType::ForceAttackGround { .. }
                | CommandType::AttackPosition { .. }
                | CommandType::Guard { .. }
                | CommandType::Patrol
                | CommandType::Scatter
                | CommandType::TightenToPosition { .. }
                | CommandType::AttackTeam { .. }
                | CommandType::FollowWaypointPath { .. }
                | CommandType::AttackFollowWaypointPath { .. }
                | CommandType::Enter { .. }
                | CommandType::Exit
                | CommandType::Evacuate
                | CommandType::MoveToAndEvacuate { .. }
                | CommandType::ClearMines
                | CommandType::CombatDrop { .. }
                | CommandType::ReturnToBase
                | CommandType::HackInternet
                | CommandType::GetRepaired { .. }
                | CommandType::GetHealed { .. }
                | CommandType::Hijack { .. }
                | CommandType::Sabotage { .. }
                | CommandType::ConvertToCarbomb { .. }
                | CommandType::CaptureBuilding { .. }
                | CommandType::SnipeVehicle { .. }
                | CommandType::AttackArea { .. }
                | CommandType::Deploy
                | CommandType::GoProne
                | CommandType::Surrender { .. }
        )
    }
    /// C++ default arm: every player AI order except Repair / ResumeConstruction.
    fn player_command_cancels_current_dozer_task(command_type: &CommandType) -> bool {
        matches!(
            command_type,
            CommandType::Move { .. }
                | CommandType::MoveTo { .. }
                | CommandType::DoSalvage { .. }
                | CommandType::AttackMoveTo { .. }
                | CommandType::ForceMoveTo { .. }
                | CommandType::AddWaypoint { .. }
                | CommandType::Attack { .. }
                | CommandType::AttackObject { .. }
                | CommandType::ForceAttackObject { .. }
                | CommandType::ForceAttackGround { .. }
                | CommandType::AttackPosition { .. }
                | CommandType::Stop
                | CommandType::Guard { .. }
                | CommandType::Patrol
                | CommandType::AttitudeSleep
                | CommandType::AttitudePassive
                | CommandType::AttitudeNormal
                | CommandType::AttitudeAggressive
                | CommandType::Scatter
                | CommandType::TightenToPosition { .. }
                | CommandType::AttackTeam { .. }
                | CommandType::FollowWaypointPath { .. }
                | CommandType::AttackFollowWaypointPath { .. }
                | CommandType::Surrender { .. }
                | CommandType::Deploy
                | CommandType::Gather { .. }
                | CommandType::DoSpecialPower { .. }
                | CommandType::DoWeapon { .. }
                | CommandType::Enter { .. }
                | CommandType::Exit
                | CommandType::Evacuate
                | CommandType::MoveToAndEvacuate { .. }
                | CommandType::HackInternet
                | CommandType::ReturnToBase
                | CommandType::ReturnSupplies
                | CommandType::ClearMines
                | CommandType::SetMineClearingDetail { .. }
                | CommandType::GoProne
                | CommandType::AttackArea { .. }
                | CommandType::Dock { .. }
                | CommandType::CombatDrop { .. }
                | CommandType::GetRepaired { .. }
                | CommandType::GetHealed { .. }
                | CommandType::Hijack { .. }
                | CommandType::Sabotage { .. }
                | CommandType::ConvertToCarbomb { .. }
                | CommandType::CaptureBuilding { .. }
                | CommandType::SnipeVehicle { .. }
                | CommandType::PlantTimedDemoCharge { .. }
                | CommandType::PlantRemoteDemoCharge { .. }
                | CommandType::DetonateRemoteDemoCharges
                | CommandType::DemoTertiarySuicide
                | CommandType::StealCashHack { .. }
                | CommandType::DisableVehicleHack { .. }
                | CommandType::HackerDisableBuilding { .. }
                | CommandType::DisguiseAsVehicle { .. }
                | CommandType::PlantBoobyTrap { .. }
                | CommandType::SwitchWeapons { .. }
                | CommandType::CreateFormation
                | CommandType::Cheer
                | CommandType::ExecuteRailedTransport
                | CommandType::SetWeaponSetFlag { .. }
                | CommandType::SetWeaponLock { .. }
                | CommandType::ReleaseWeaponLock { .. }
                | CommandType::SetEmoticon { .. }
                | CommandType::OverrideSpecialPowerDestination { .. }
        )
    }

    /// C++ aiDoCommand head, including Repair / ResumeConstruction.
    fn player_command_clears_dozer_constructing(command_type: &CommandType) -> bool {
        Self::player_command_cancels_current_dozer_task(command_type)
            || matches!(
                command_type,
                CommandType::Repair { .. } | CommandType::ResumeConstruction { .. }
            )
    }

    fn sync_logical_retaliation_from_client(&mut self) {
        let enabled = game_engine::common::global_data::read_safe()
            .map(|data| data.client_retaliation_mode_enabled)
            .unwrap_or(true);
        let ids: Vec<u32> = self
            .game_logic
            .get_players()
            .values()
            .filter(|player| player.is_local && player.logical_retaliation_mode_enabled != enabled)
            .map(|player| player.id)
            .collect();
        for id in ids {
            self.game_logic.set_logical_retaliation_mode(id, enabled);
        }
    }

    /// Get execution statistics
    pub fn get_stats(&self) -> (usize, usize) {
        (self.commands_executed, self.commands_failed)
    }
}

/// Concatenated live command_executor sources for residual `include_str` scans.
pub const COMMAND_EXECUTOR_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("abilities.rs"),
    include_str!("attack.rs"),
    include_str!("cheat_debug.rs"),
    include_str!("command_button.rs"),
    include_str!("construct.rs"),
    include_str!("group.rs"),
    include_str!("leftover.rs"),
    include_str!("movement.rs"),
    include_str!("produce.rs"),
    include_str!("selection.rs"),
    include_str!("sell_upgrade.rs"),
    include_str!("special_power.rs"),
    include_str!("transport.rs"),
    include_str!("validate.rs"),
);
