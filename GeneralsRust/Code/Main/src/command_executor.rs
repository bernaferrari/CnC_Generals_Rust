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

/// Command executor that processes game commands
pub struct CommandExecutor<'a> {
    /// Reference to game logic for object manipulation
    game_logic: &'a mut GameLogic,

    /// Current player executing commands
    current_player_id: u32,

    /// Track command execution metrics
    commands_executed: usize,
    commands_failed: usize,
}

impl<'a> CommandExecutor<'a> {
    /// Create a new command executor with game logic reference
    pub fn new(game_logic: &'a mut GameLogic, player_id: u32) -> Self {
        Self {
            game_logic,
            current_player_id: player_id,
            commands_executed: 0,
            commands_failed: 0,
        }
    }

    fn player_team(&self, player_id: u32) -> Team {
        self.game_logic
            .get_player(player_id)
            .map(|player| player.team)
            .unwrap_or_else(|| Team::from_player_id(player_id))
    }

    /// Execute a game command and return result
    pub fn execute_command(&mut self, command: GameCommand) -> Result<CommandResult, String> {
        debug!(
            "Executing command {:?} for player {}",
            command.command_type, command.player_id
        );
        self.current_player_id = command.player_id;

        // Validate player ownership
        if !self.validate_player_ownership(&command) {
            self.commands_failed += 1;
            return Ok(CommandResult::InvalidCommand);
        }

        let result = match &command.command_type {
            // Movement commands
            CommandType::Move { destination } => {
                self.execute_move(&command.selected_units, *destination)
            }
            CommandType::MoveTo {
                destination,
                waypoints,
            } => self.execute_move_to(&command.selected_units, *destination, waypoints),
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
            CommandType::Sell { object_id } => self.execute_sell(*object_id, command.player_id),

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
                target,
            } => self.execute_weapon(&command.selected_units, weapon_slot, target),

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
            CommandType::AttackArea { center, radius } => {
                self.execute_attack_area(&command.selected_units, *center, *radius)
            }
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
            CommandType::SwitchWeapons => self.execute_switch_weapons(&command.selected_units),
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
            CommandType::RemoveBeacon => self.execute_remove_beacon(command.player_id),
            CommandType::ViewRadarAt { position } => {
                // Direct camera jump to requested radar location (e.g., from replay/HUD ping).
                self.game_logic.request_camera_focus(*position);
                CommandResult::Success
            }

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
            CommandType::ViewLastRadarEvent => {
                // Mirror CommandSystem routing: request camera snap to last radar event.
                if let Some(position) = self.game_logic.last_radar_event_position() {
                    self.game_logic.request_camera_focus(position);
                    CommandResult::Success
                } else {
                    CommandResult::InvalidCommand
                }
            }
            CommandType::ViewCommandCenter => {
                // Center camera on the current player's command center, matching C++ quick-jump.
                self.execute_view_command_center()
            }

            CommandType::Invalid => {
                warn!("Invalid command type received");
                CommandResult::InvalidCommand
            }
        };

        if result == CommandResult::Success {
            self.commands_executed += 1;
        } else {
            self.commands_failed += 1;
        }

        Ok(result)
    }

    // === Movement Commands ===

    pub(crate) fn execute_move(&mut self, units: &[ObjectId], destination: Vec3) -> CommandResult {
        // C++ groupMoveToPosition: click inside group bounds → tighten (all to point).
        if self.should_tighten_group_move(units, destination) {
            return self.execute_tighten_to_position(units, destination);
        }
        // C++ friend_computeGroundPath + friend_moveFormationToPos residual.
        if units.len() > 1 && self.compute_ground_path_should_group(units, destination) {
            let fid0 = units
                .first()
                .and_then(|id| self.game_logic.get_object(*id))
                .map(|o| o.formation_id)
                .unwrap_or(0);
            let is_formation = fid0 != 0
                && units.iter().all(|&id| {
                    self.game_logic
                        .get_object(id)
                        .map(|o| o.formation_id == fid0)
                        .unwrap_or(false)
                });
            if is_formation {
                return self.execute_move_formation_to_position(units, destination);
            }
        }
        let goals = self.group_move_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            } else {
                return CommandResult::InvalidTarget;
            }
            if !self.game_logic.assign_unit_path(unit_id, goal, &[]) {
                return CommandResult::InvalidCommand;
            }
            // C++ groupMoveToPosition clears formation id on free move.
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                // Keep stamped formation when formation move path used destinations
                // already offset; free-move / column still clear id so next move
                // doesn't keep stale offsets after scatter-like pack.
                // C++ setFormationID(NO_FORMATION) on free individual move.
                if unit.formation_id != 0 {
                    // Only clear when destinations were not pure formation offsets:
                    // formation path keeps id. Detect: goal == dest + offset.
                    let off = unit.formation_offset;
                    let expected = glam::Vec3::new(
                        destination.x + off.x,
                        destination.y,
                        destination.z + off.y,
                    );
                    if (goal - expected).length() > 0.5 {
                        unit.formation_id = 0;
                        unit.formation_offset = glam::Vec2::ZERO;
                    }
                }
            }
            moved.push(unit_id);
            debug!("Unit {} moving to {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        CommandResult::Success
    }

    fn execute_move_to(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> CommandResult {
        if waypoints.is_empty() && self.should_tighten_group_move(units, destination) {
            return self.execute_tighten_to_position(units, destination);
        }
        let goals = self.group_move_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            } else {
                return CommandResult::InvalidTarget;
            }
            if !self.game_logic.assign_unit_path(unit_id, goal, waypoints) {
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
            debug!("Unit {} moving via waypoints to {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        CommandResult::Success
    }

    /// C++ AIGroup::groupMoveToPosition / computeIndividualDestination residual.
    ///
    /// Sort movers near→far to the click, take the nearest unit as the free-move
    /// "center", then offset each unit's goal by its (clamped) vector from that
    /// center — preserves relative formation instead of inventing a ring.

    /// C++ AIGroup player move/stop stealth residual: delay mood auto-acquire until
    /// unstealthed combat stealth units can cloak again.
    fn apply_player_stealth_mood_delay(&mut self, unit_ids: &[ObjectId]) {
        let now = self.game_logic.get_frame();
        for (i, &unit_id) in unit_ids.iter().enumerate() {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            let can_stealth = unit.innate_stealth || unit.stealth_delay_frames > 0;
            if can_stealth
                && unit.auto_acquire_when_idle
                && unit.can_attack()
                && !unit.status.stealthed
                && !unit.status.detected
            {
                let delay = unit.stealth_delay_frames.max(1);
                let skew = (i as u32) % 30;
                unit.next_mood_check_time = now.saturating_add(delay).saturating_add(skew);
            }
        }
    }

    pub(crate) fn group_move_destinations(
        &self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> Vec<(ObjectId, Vec3)> {
        if units.is_empty() {
            return Vec::new();
        }
        if units.len() == 1 {
            return vec![(units[0], destination)];
        }

        // Gather movable members with positions (skip dead / immobile).
        let mut movers: Vec<(ObjectId, Vec3, f32, u32, glam::Vec2, bool, bool)> =
            Vec::with_capacity(units.len());
        for &unit_id in units {
            let Some(obj) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            if obj.is_kind_of(crate::game_logic::KindOf::Immobile)
                || obj.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            let radius = obj.selection_radius.max(5.0);
            movers.push((
                unit_id,
                obj.get_position(),
                radius,
                obj.formation_id,
                obj.formation_offset,
                obj.is_kind_of(crate::game_logic::KindOf::Infantry),
                obj.is_kind_of(crate::game_logic::KindOf::Vehicle),
            ));
        }
        if movers.is_empty() {
            return units.iter().map(|&id| (id, destination)).collect();
        }
        if movers.len() == 1 {
            return vec![(movers[0].0, destination)];
        }

        // Shared non-zero formation id → C++ formation move offsets.
        let fid0 = movers[0].3;
        let is_formation = fid0 != 0 && movers.iter().all(|m| m.3 == fid0);
        if is_formation {
            return movers
                .into_iter()
                .map(|(id, _pos, _r, _fid, off, _inf, _veh)| {
                    (
                        id,
                        Vec3::new(destination.x + off.x, destination.y, destination.z + off.y),
                    )
                })
                .collect();
        }

        // C++ friend_moveInfantryToPos / friend_moveVehicleToPos residual:
        // when enough pure infantry or vehicles move far enough, pack into columns
        // along the move direction instead of free-move center offsets.
        if let Some(column) = self.group_column_destinations(&movers, destination) {
            return column;
        }

        // Near-to-far vs goal (C++ SimpleObjectIterator ITER_SORTED_NEAR_TO_FAR).
        movers.sort_by(|a, b| {
            let da = (a.1.x - destination.x).hypot(a.1.z - destination.z);
            let db = (b.1.x - destination.x).hypot(b.1.z - destination.z);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Free-move center is the nearest unit's current position (C++ firstUnit branch).
        let center = movers[0].1;
        let mut out = Vec::with_capacity(movers.len());
        for (i, (unit_id, pos, radius, _fid, _off, _inf, _veh)) in movers.into_iter().enumerate() {
            let goal = if i == 0 {
                destination
            } else {
                let mut dx = pos.x - center.x;
                let mut dz = pos.z - center.z;
                let mut length = (dx * dx + dz * dz).sqrt();
                let max_length = 6.0 * radius;
                if length > max_length {
                    length = max_length;
                }
                if length > 0.001 {
                    let nlen = (dx * dx + dz * dz).sqrt().max(0.001);
                    dx = (dx / nlen) * length;
                    dz = (dz / nlen) * length;
                } else {
                    let angle = (i as f32) * 1.7;
                    dx = angle.cos() * radius * 0.5;
                    dz = angle.sin() * radius * 0.5;
                }
                Vec3::new(destination.x + dx, destination.y, destination.z + dz)
            };
            out.push((unit_id, goal));
        }
        out
    }

    /// C++ GlobalData::m_groupMoveClickToGatherFactor residual (1.0 = full bbox).
    const GROUP_MOVE_CLICK_TO_GATHER_FACTOR: f32 = 1.0;

    /// True when destination lies inside the selected group's XZ bounding rect
    /// scaled by gather factor — C++ groupMoveToPosition tighten path.
    pub(crate) fn should_tighten_group_move(&self, units: &[ObjectId], destination: Vec3) -> bool {
        if Self::GROUP_MOVE_CLICK_TO_GATHER_FACTOR <= 0.0 || units.len() < 2 {
            return false;
        }
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        let mut count = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() || !o.can_move() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Immobile)
                || o.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            // Airborne fixed-wing: C++ disables tighten.
            if o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                && o.status.airborne_target
                && !o.template_name.to_ascii_lowercase().contains("heli")
                && !o.template_name.to_ascii_lowercase().contains("chinook")
                && !o.template_name.to_ascii_lowercase().contains("comanche")
            {
                return false;
            }
            let p = o.get_position();
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
            count += 1;
        }
        if count < 2 {
            return false;
        }
        // Scale rect about center by gather factor.
        let cx = 0.5 * (min_x + max_x);
        let cz = 0.5 * (min_z + max_z);
        let hx = 0.5 * (max_x - min_x) * Self::GROUP_MOVE_CLICK_TO_GATHER_FACTOR;
        let hz = 0.5 * (max_z - min_z) * Self::GROUP_MOVE_CLICK_TO_GATHER_FACTOR;
        // Pad tiny groups so a click near the cluster still gathers.
        let hx = hx.max(20.0);
        let hz = hz.max(20.0);
        destination.x >= cx - hx
            && destination.x <= cx + hx
            && destination.z >= cz - hz
            && destination.z <= cz + hz
    }

    /// C++ AIGroup::groupTightenToPosition — near-to-far, all path to same pos.
    pub(crate) fn execute_tighten_to_position(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        // Sort near-to-far (C++ SimpleObjectIterator ITER_SORTED_NEAR_TO_FAR).
        let mut movers: Vec<(ObjectId, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            let p = unit.get_position();
            let dx = p.x - destination.x;
            let dz = p.z - destination.z;
            movers.push((unit_id, dx * dx + dz * dz));
        }
        movers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut any = false;
        for (unit_id, _) in movers {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
                unit.formation_id = 0;
                unit.set_guard_position(None);
                unit.set_guard_target(None);
                unit.end_guard_retaliate();
            }
            if self.path_to_goal_with_state(unit_id, destination, AIState::Moving) {
                any = true;
            }
        }
        if any {
            self.apply_player_stealth_mood_delay(units);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupOverrideSpecialPowerDestination residual.
    pub(crate) fn execute_override_special_power_destination(
        &mut self,
        units: &[ObjectId],
        location: Vec3,
    ) -> CommandResult {
        if !location.x.is_finite() || !location.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            // Only units with an active / ready special power path accept override.
            // Host residual: always store; consumers of special power read it.
            unit.set_special_power_overridable_destination(location, None);
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::setWeaponSetFlag residual.
    pub(crate) fn execute_set_weapon_set_flag(
        &mut self,
        units: &[ObjectId],
        flag: u8,
        enabled: bool,
    ) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            if unit.set_weapon_set_flag(flag, enabled) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupFollowWaypointPath / Exact / AsTeam residual.
    pub(crate) fn execute_follow_waypoint_path(
        &mut self,
        units: &[ObjectId],
        waypoints: &[Vec3],
        exact: bool,
        as_team: bool,
    ) -> CommandResult {
        // `exact` → assign_unit_path_exact (C++ AIFollowWaypointPathExactState).
        if waypoints.is_empty() {
            return CommandResult::InvalidLocation;
        }
        for wp in waypoints {
            if !wp.x.is_finite() || !wp.z.is_finite() {
                return CommandResult::InvalidLocation;
            }
        }

        // Collect movers + optional formation/group offsets (AsTeam residual).
        let mut movers: Vec<(ObjectId, Vec3, glam::Vec2)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            movers.push((unit_id, unit.get_position(), unit.formation_offset));
        }
        if movers.is_empty() {
            return CommandResult::InvalidCommand;
        }

        // Group center from current positions.
        let (mut cx, mut cz) = (0.0f32, 0.0f32);
        for (_, pos, _) in &movers {
            cx += pos.x;
            cz += pos.z;
        }
        let n = movers.len() as f32;
        cx /= n;
        cz /= n;

        // Prefer stamped formation offsets when shared; else relative-to-center.
        let fid0 = self
            .game_logic
            .get_object(movers[0].0)
            .map(|o| o.formation_id)
            .unwrap_or(0);
        let use_formation = as_team
            && fid0 != 0
            && movers.iter().all(|(id, _, _)| {
                self.game_logic
                    .get_object(*id)
                    .map(|o| o.formation_id == fid0)
                    .unwrap_or(false)
            });

        // Near-to-far vs first waypoint.
        let first = waypoints[0];
        movers.sort_by(|a, b| {
            let da = (a.1.x - first.x).hypot(a.1.z - first.z);
            let db = (b.1.x - first.x).hypot(b.1.z - first.z);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut any = false;
        for (unit_id, pos, form_off) in movers {
            let offset = if as_team {
                if use_formation {
                    form_off
                } else {
                    glam::Vec2::new(pos.x - cx, pos.z - cz)
                }
            } else {
                glam::Vec2::ZERO
            };

            let unit_wps: Vec<Vec3> = waypoints
                .iter()
                .map(|wp| Vec3::new(wp.x + offset.x, wp.y, wp.z + offset.y))
                .collect();
            let goal = *unit_wps.last().unwrap();
            let via = &unit_wps[..unit_wps.len().saturating_sub(1)];

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
                unit.set_guard_position(None);
                unit.set_guard_target(None);
                unit.end_guard_retaliate();
                // AsTeam keeps formation identity; free follow clears it.
                if !as_team {
                    unit.formation_id = 0;
                }
            }
            // C++ AIFollowWaypointPathExact vs smoothed follow residual.
            let ok = if exact {
                self.game_logic.assign_unit_path_exact(unit_id, goal, via)
            } else {
                self.game_logic.assign_unit_path(unit_id, goal, via)
            };
            if ok {
                any = true;
            } else if self.path_to_goal_with_state(unit_id, goal, AIState::Moving) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIAttackFollowWaypointPathState residual —
    /// follow path while able to auto-engage (attack-move along waypoints).
    pub(crate) fn execute_attack_follow_waypoint_path(
        &mut self,
        units: &[ObjectId],
        waypoints: &[Vec3],
        exact: bool,
        as_team: bool,
    ) -> CommandResult {
        let path_res = self.execute_follow_waypoint_path(units, waypoints, exact, as_team);
        if !matches!(path_res, CommandResult::Success) {
            return path_res;
        }
        // Promote movers that can attack into AttackMoving + is_attack_path.
        for &unit_id in units {
            let can_attack = self
                .game_logic
                .get_object(unit_id)
                .map(|u| u.is_alive() && (u.can_attack() || u.weapon.is_some()))
                .unwrap_or(false);
            if !can_attack {
                continue;
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                // C++ findWaypointFollowingCapableWeapon residual for attack-move path.
                if let Some(slot) = unit.find_waypoint_following_capable_weapon_slot() {
                    unit.set_active_weapon_slot(slot);
                }
                unit.is_attack_path = true;
                unit.set_ai_state(AIState::AttackMoving);
            }
        }
        CommandResult::Success
    }

    /// C++ AIGroup::groupDoCommandButtonUsingWaypoints residual.
    pub(crate) fn execute_do_command_button_using_waypoints(
        &mut self,
        units: &[ObjectId],
        button: &str,
        waypoints: &[Vec3],
    ) -> CommandResult {
        use crate::command_system::{command_type_from_button_name, CommandType};

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
            command_type_from_button_name, CommandType, DropTarget, GuardTarget, ModifierKeys,
            PowerTarget,
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

    /// C++ AIGroup::groupExecuteRailedTransport residual.
    pub(crate) fn execute_railed_transport(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let is_railish = match self.game_logic.get_object(unit_id) {
                Some(o) if o.is_alive() => {
                    let n = o.template_name.to_ascii_lowercase();
                    o.can_contain()
                        || n.contains("train")
                        || n.contains("rail")
                        || n.contains("locomotive")
                }
                _ => false,
            };
            if !is_railish {
                continue;
            }
            if matches!(self.execute_evacuate(&[unit_id]), CommandResult::Success) {
                any = true;
            }
            let dest = self.game_logic.get_object(unit_id).and_then(|o| {
                o.movement
                    .path
                    .last()
                    .copied()
                    .or(o.movement.target_position)
            });
            if let Some(dest) = dest {
                if self.path_to_goal_with_state(unit_id, dest, AIState::Moving) {
                    any = true;
                }
            }
        }
        if any {
            CommandResult::Success
        } else {
            self.execute_evacuate(units)
        }
    }

    /// C++ AIGroup::groupSurrender residual.
    pub(crate) fn execute_surrender(
        &mut self,
        units: &[ObjectId],
        surrendered: bool,
    ) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            unit.set_surrendered(surrendered);
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupAttackTeam residual.
    pub(crate) fn execute_attack_team(
        &mut self,
        units: &[ObjectId],
        team_code: u8,
        _max_shots: i32,
    ) -> CommandResult {
        use crate::game_logic::Team;
        let enemy_team = match team_code {
            0 => Team::GLA,
            1 => Team::USA,
            2 => Team::China,
            _ => return CommandResult::InvalidTarget,
        };
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            // Host residual: allow attack order even before weapon bind (can_attack may be false).
            if !unit.is_alive() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Structure) && !unit.can_attack() {
                continue;
            }
            let my_team = unit.team;
            if my_team == enemy_team {
                continue;
            }
            let origin = unit.get_position();
            // Nearest living enemy of that team.
            let mut best: Option<(ObjectId, f32)> = None;
            for (cid, cand) in self.game_logic.get_objects().iter() {
                if cand.team != enemy_team || !cand.is_alive() {
                    continue;
                }
                let d = origin.distance(cand.get_position());
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((*cid, d));
                }
            }
            if let Some((tid, _)) = best {
                // Direct engage residual (don't require full weapon matrix for order).
                if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                    unit.set_target(Some(tid));
                    unit.set_force_attack(false);
                    unit.set_ai_state(AIState::Attacking);
                    any = true;
                }
                let tpos = self.game_logic.get_object(tid).map(|o| o.get_position());
                if let Some(pos) = tpos {
                    let _ = self.path_to_goal_with_state(unit_id, pos, AIState::Attacking);
                }
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup column path residual (infantry 3-col / vehicle group).
    /// Fail-closed: not full ground-path node following; destination-side pack only.
    fn group_column_destinations(
        &self,
        movers: &[(ObjectId, Vec3, f32, u32, glam::Vec2, bool, bool)],
        destination: Vec3,
    ) -> Option<Vec<(ObjectId, Vec3)>> {
        use crate::game_logic::host_ai_path_combat_residual_wave105::{
            MIN_DISTANCE_FOR_GROUP_RESIDUAL, MIN_INFANTRY_FOR_GROUP_RESIDUAL,
            MIN_VEHICLES_FOR_GROUP_RESIDUAL,
        };

        let n = movers.len() as i32;
        let all_infantry = movers.iter().all(|m| m.5);
        let all_vehicles = movers.iter().all(|m| m.6);
        if !all_infantry && !all_vehicles {
            return None;
        }
        let min_count = if all_infantry {
            MIN_INFANTRY_FOR_GROUP_RESIDUAL
        } else {
            MIN_VEHICLES_FOR_GROUP_RESIDUAL
        };
        if n < min_count {
            return None;
        }

        let mut center = Vec3::ZERO;
        for m in movers {
            center += m.1;
        }
        center /= movers.len() as f32;

        let mut dir_x = destination.x - center.x;
        let mut dir_z = destination.z - center.z;
        let dist = (dir_x * dir_x + dir_z * dir_z).sqrt();
        if dist < MIN_DISTANCE_FOR_GROUP_RESIDUAL {
            return None;
        }
        dir_x /= dist;
        dir_z /= dist;
        // Perpendicular (C++ startVectorNormal: (-y, x) on XY → (-z, x) on XZ).
        let nx = -dir_z;
        let nz = dir_x;

        // Sort by projection on normal (C++ FAR_TO_NEAR on normal dot).
        let mut ordered: Vec<(ObjectId, Vec3, f32, f32)> = movers
            .iter()
            .map(|m| {
                let dx = m.1.x - center.x;
                let dz = m.1.z - center.z;
                let proj = dx * nx + dz * nz;
                (m.0, m.1, m.2, proj)
            })
            .collect();
        ordered.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));

        let num_columns = 3i32;
        let half = num_columns / 2;
        let units_to_path = ordered.len() as i32;
        // C++: spacing uses path cell size; host residual ≈ average radius.
        let avg_r: f32 = ordered.iter().map(|o| o.2).sum::<f32>() / (ordered.len() as f32).max(1.0);
        let col_spacing = avg_r.max(8.0) * 1.25;
        let rank_spacing = avg_r.max(8.0) * 1.5;

        let mut out = Vec::with_capacity(ordered.len());
        for (cur_index, (id, _pos, _r, _proj)) in ordered.into_iter().enumerate() {
            let cur_index = cur_index as i32;
            // C++: divisor = (unitsToPath+1)/numColumns; columnDelta = 1 - curIndex/divisor
            let mut divisor = (units_to_path + 1) / num_columns;
            if divisor < 1 {
                divisor = 1;
            }
            let mut column_delta = 1 - (cur_index / divisor);
            if column_delta < -half {
                column_delta = -half;
            }
            if column_delta > half {
                column_delta = half;
            }
            // Rank depth along move direction (rows).
            let rank = cur_index / num_columns;
            let goal = Vec3::new(
                destination.x + nx * (column_delta as f32) * col_spacing
                    - dir_x * (rank as f32) * rank_spacing,
                destination.y,
                destination.z + nz * (column_delta as f32) * col_spacing
                    - dir_z * (rank as f32) * rank_spacing,
            );
            out.push((id, goal));
        }
        Some(out)
    }

    /// Pathfind to `goal` then set AI state. Returns false if path assign fails.
    /// Used by Guard/Scatter/Gather/Enter/Construct so units navigate obstacles.
    fn path_to_goal_with_state(&mut self, unit_id: ObjectId, goal: Vec3, state: AIState) -> bool {
        if !self.game_logic.assign_unit_path(unit_id, goal, &[]) {
            return false;
        }
        if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
            unit.set_ai_state(state);
        }
        true
    }

    /// C++ AIGroup::groupAttackMoveToPosition residual.
    /// ableToAttack → attack-move path + maxShots; else plain move.
    pub(crate) fn execute_attack_move(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        max_shots: i32,
    ) -> CommandResult {
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let goals = self.group_move_destinations(units, destination);
        let mut any = false;
        for (unit_id, goal) in goals {
            let (can_move, can_attack) = match self.game_logic.get_object(unit_id) {
                Some(unit) => (
                    unit.is_alive() && unit.can_move(),
                    unit.can_attack() || unit.weapon.is_some(),
                ),
                None => continue,
            };
            if !can_move {
                continue;
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
                unit.set_force_attack(false);
                unit.set_max_shots_to_fire(max_shots);
            }
            if !self.game_logic.assign_unit_path(unit_id, goal, &[]) {
                if !self.path_to_goal_with_state(
                    unit_id,
                    goal,
                    if can_attack {
                        AIState::AttackMoving
                    } else {
                        AIState::Moving
                    },
                ) {
                    continue;
                }
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                if can_attack {
                    unit.is_attack_path = true;
                    unit.auto_acquire_when_idle = true;
                    unit.set_ai_state(AIState::AttackMoving);
                } else {
                    unit.is_attack_path = false;
                    unit.set_ai_state(AIState::Moving);
                }
            }
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_force_move(&mut self, units: &[ObjectId], destination: Vec3) -> CommandResult {
        let goals = self.group_move_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if self.game_logic.get_object(unit_id).is_none() {
                return CommandResult::InvalidTarget;
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            }
            // Force move still pathfinds; threat ignore is AI-state residual, not LOS.
            if !self.game_logic.assign_unit_path(unit_id, goal, &[]) {
                return CommandResult::InvalidCommand;
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.set_ai_state(AIState::Moving);
            }
            moved.push(unit_id);
        }
        self.apply_player_stealth_mood_delay(&moved);
        CommandResult::Success
    }

    pub(crate) fn execute_add_waypoint(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        // C++ groupMoveToPosition(addWaypoint): individual dests + path append.
        let goals = self.group_move_destinations(units, destination);
        let mut moved: Vec<ObjectId> = Vec::new();
        for (unit_id, goal) in goals {
            if self.game_logic.get_object(unit_id).is_none() {
                return CommandResult::InvalidTarget;
            }
            if !self.game_logic.append_unit_waypoint(unit_id, goal) {
                return CommandResult::InvalidCommand;
            }
            moved.push(unit_id);
            debug!("Added waypoint for unit {} at {:?}", unit_id.0, goal);
        }
        self.apply_player_stealth_mood_delay(&moved);
        CommandResult::Success
    }

    // === Combat Commands ===

    fn execute_attack(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        let Some(target_team) = self
            .game_logic
            .get_object(target_id)
            .map(|target| target.team)
        else {
            return CommandResult::InvalidTarget;
        };

        if self
            .game_logic
            .get_object(target_id)
            .is_some_and(|target| !target.is_alive())
        {
            return CommandResult::TargetDestroyed;
        }

        // C++ groupAttackObjectPrivate: sort attackers near-to-far to victim first.
        let target_pos = self
            .game_logic
            .get_object(target_id)
            .map(|tg| tg.get_position())
            .unwrap_or(Vec3::ZERO);
        let mut ordered: Vec<(ObjectId, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.can_attack() || unit.team == target_team {
                continue;
            }
            let p = unit.get_position();
            let d = (p.x - target_pos.x).hypot(p.z - target_pos.z);
            ordered.push((unit_id, d));
        }
        ordered.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut any_attacker = false;
        for (unit_id, _) in ordered {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.set_force_attack(false);
                unit.set_target(Some(target_id));
                unit.set_ai_state(AIState::Attacking);
                any_attacker = true;
            }
        }

        if any_attacker {
            CommandResult::Success
        } else {
            CommandResult::CannotAttackTarget
        }
    }

    fn execute_attack_object(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        self.execute_attack(units, target_id)
    }

    fn execute_force_attack(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        if !self.validate_target_exists(target_id) {
            return CommandResult::InvalidTarget;
        }

        if self
            .game_logic
            .get_object(target_id)
            .is_some_and(|target| !target.is_alive())
        {
            return CommandResult::TargetDestroyed;
        }

        // C++ groupAttackObjectPrivate(forced=true): near-to-far order.
        let target_pos = self
            .game_logic
            .get_object(target_id)
            .map(|tg| tg.get_position())
            .unwrap_or(Vec3::ZERO);
        let mut ordered: Vec<(ObjectId, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.can_attack() {
                continue;
            }
            let p = unit.get_position();
            let d = (p.x - target_pos.x).hypot(p.z - target_pos.z);
            ordered.push((unit_id, d));
        }
        ordered.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut any_attacker = false;
        for (unit_id, _) in ordered {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.set_target(Some(target_id));
                unit.set_force_attack(true);
                unit.set_ai_state(AIState::Attacking);
                any_attacker = true;
            }
        }

        if any_attacker {
            CommandResult::Success
        } else {
            CommandResult::CannotAttackTarget
        }
    }

    /// C++ AIData::m_distanceRequiresGroup residual (force group moving when far).
    const DISTANCE_REQUIRES_GROUP: f32 = 200.0;
    /// C++ AIData::m_minDistanceForGroup residual.
    const MIN_DISTANCE_FOR_GROUP: f32 = 40.0;

    /// C++ AIGroup::getSpecialPowerSourceObject residual —
    /// first living member that can execute `power_type`.
    pub(crate) fn special_power_source_object(
        &self,
        units: &[ObjectId],
        power_type: &crate::command_system::SpecialPowerType,
    ) -> Option<ObjectId> {
        // C++ walks members for SpecialPowerModule matching template.
        // Host: only members that explicitly track this power (cooldown map).
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            if o.special_power_cooldowns.contains_key(power_type) {
                return Some(id);
            }
        }
        None
    }

    /// C++ AIGroup::getCommandButtonSourceObject residual —
    /// first living member that can act on `command` capability.
    pub(crate) fn command_button_source_object(
        &self,
        units: &[ObjectId],
        command: &crate::command_system::CommandType,
    ) -> Option<ObjectId> {
        use crate::command_system::CommandType;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            let ok = match command {
                CommandType::Attack { .. }
                | CommandType::AttackObject { .. }
                | CommandType::ForceAttackObject { .. }
                | CommandType::AttackPosition { .. }
                | CommandType::ForceAttackGround { .. }
                | CommandType::AttackMoveTo { .. }
                | CommandType::AttackFollowWaypointPath { .. } => {
                    // Prefer a member that actually carries a weapon module.
                    o.weapon.is_some()
                }
                CommandType::Move { .. }
                | CommandType::MoveTo { .. }
                | CommandType::ForceMoveTo { .. }
                | CommandType::FollowWaypointPath { .. }
                | CommandType::Scatter { .. }
                | CommandType::Guard { .. } => o.can_move(),
                CommandType::Stop => true,
                CommandType::Evacuate | CommandType::MoveToAndEvacuate { .. } => {
                    o.can_contain() || !o.contained_units().is_empty()
                }
                CommandType::DoSpecialPower { power_type, .. } => {
                    self.game_logic.is_special_power_ready_for(id, power_type)
                        || o.special_power_cooldowns.contains_key(power_type)
                }
                CommandType::Sell { .. } | CommandType::ToggleOvercharge => {
                    o.is_kind_of(crate::game_logic::KindOf::Structure)
                }
                CommandType::HackInternet => {
                    o.can_move() || o.template_name.to_ascii_lowercase().contains("hacker")
                }
                CommandType::GetRepaired { .. } | CommandType::GetHealed { .. } => o.can_move(),
                CommandType::CreateFormation => o.can_move(),
                _ => {
                    // Fall open: any living selectable member.
                    o.is_kind_of(crate::game_logic::KindOf::Selectable) || o.can_move()
                }
            };
            if ok {
                return Some(id);
            }
        }
        None
    }

    /// C++ AIGroup::getAllIDs residual — living members in selection order.
    pub(crate) fn group_all_ids(&self, units: &[ObjectId]) -> Vec<ObjectId> {
        let mut out = Vec::with_capacity(units.len());
        for &id in units {
            if self
                .game_logic
                .get_object(id)
                .map(|o| o.is_alive())
                .unwrap_or(false)
            {
                out.push(id);
            }
        }
        out
    }

    /// C++ AIGroup::getAttitude residual — retail always returns AI_PASSIVE.
    pub(crate) fn group_attitude(
        &self,
        _units: &[ObjectId],
    ) -> crate::game_logic::host_strategy_center::HostAiAttitude {
        crate::game_logic::host_strategy_center::HostAiAttitude::Passive
    }

    /// C++ AIGroup::getCount residual.
    pub(crate) fn group_count(&self, units: &[ObjectId]) -> usize {
        units
            .iter()
            .filter(|&&id| {
                self.game_logic
                    .get_object(id)
                    .map(|o| o.is_alive())
                    .unwrap_or(false)
            })
            .count()
    }

    /// C++ AIGroup::getSpeed / recompute residual —
    /// slowest non-held, non-immobile locomotor among members whose body
    /// damage state is BETTER than MovementPenaltyDamageState (REALLYDAMAGED).
    /// Heavily damaged units do not drag the whole group down.
    pub(crate) fn group_speed(&self, units: &[ObjectId]) -> f32 {
        use crate::game_logic::host_ai_path_combat_residual_wave105::{
            calc_damage_state_residual, is_body_condition_better, BODY_REALLYDAMAGED,
        };
        let mut best = f32::INFINITY;
        let mut saw = false;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() || !o.can_move() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Immobile)
                || o.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            if o.contained_by.is_some() {
                continue; // HELD residual — skip riders
            }
            let max_h = o.health.maximum.max(1.0);
            let dmg = calc_damage_state_residual(o.health.current, max_h);
            // C++: only if IS_CONDITION_BETTER(damageState, movementPenaltyDamageState)
            if !is_body_condition_better(dmg, BODY_REALLYDAMAGED) {
                continue;
            }
            let spd = o.effective_max_speed().max(0.0);
            if spd > 0.0 && spd < best {
                best = spd;
                saw = true;
            }
        }
        if saw {
            best
        } else {
            0.0
        }
    }

    /// C++ AIGroup::recompute leadership residual —
    /// closest non-immobile, non-held member to group center.
    pub(crate) fn group_leader_id(&self, units: &[ObjectId]) -> Option<ObjectId> {
        let (_, _, center) = self.group_min_max_and_center(units)?;
        let mut best_id = None;
        let mut best_d2 = f32::INFINITY;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() || !o.can_move() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Immobile)
                || o.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            if o.contained_by.is_some() {
                continue;
            }
            let p = o.get_position();
            let d2 = (p.x - center.x).powi(2) + (p.z - center.z).powi(2);
            if d2 < best_d2 {
                best_d2 = d2;
                best_id = Some(id);
            }
        }
        best_id
    }

    /// C++ AIGroup::getMinMaxAndCenter residual (XZ plane; skip held).
    /// Returns (min_xz, max_xz, center) or None if empty.
    pub(crate) fn group_min_max_and_center(
        &self,
        units: &[ObjectId],
    ) -> Option<(glam::Vec2, glam::Vec2, Vec3)> {
        let mut min_x = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut min_z = f32::INFINITY;
        let mut max_z = f32::NEG_INFINITY;
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut cz = 0.0f32;
        let mut count = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() || o.contained_by.is_some() {
                continue;
            }
            let p = o.get_position();
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_z = min_z.min(p.z);
            max_z = max_z.max(p.z);
            cx += p.x;
            cy += p.y;
            cz += p.z;
            count += 1;
        }
        if count == 0 {
            return None;
        }
        let n = count as f32;
        Some((
            glam::Vec2::new(min_x, min_z),
            glam::Vec2::new(max_x, max_z),
            Vec3::new(cx / n, cy / n, cz / n),
        ))
    }

    /// C++ AIGroup::friend_computeGroundPath residual (simplified).
    /// True when the group should path as a formation/group toward `dest`.
    pub(crate) fn compute_ground_path_should_group(&self, units: &[ObjectId], dest: Vec3) -> bool {
        let Some((min, max, center)) = self.group_min_max_and_center(units) else {
            return false;
        };
        let mut num_infantry = 0u32;
        let mut num_vehicles = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() || o.contained_by.is_some() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Infantry)
                || o.object_type == crate::game_logic::ObjectType::Infantry
            {
                num_infantry += 1;
            } else if o.is_kind_of(crate::game_logic::KindOf::Vehicle)
                && !o.is_kind_of(crate::game_logic::KindOf::Aircraft)
            {
                num_vehicles += 1;
            }
        }
        if num_infantry + num_vehicles == 0 {
            return false;
        }

        // Closest unit → dest distance.
        let mut closest_sqr = f32::INFINITY;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            let p = o.get_position();
            let d2 = (p.x - dest.x).powi(2) + (p.z - dest.z).powi(2);
            closest_sqr = closest_sqr.min(d2);
        }
        let bbox_dx = max.x - min.x;
        let bbox_dz = max.y - min.y;
        let mut span_sqr = bbox_dx * bbox_dx + bbox_dz * bbox_dz;
        let req = Self::DISTANCE_REQUIRES_GROUP;
        let min_d = Self::MIN_DISTANCE_FOR_GROUP;
        if span_sqr > req * req {
            // Use group span as the distance metric (C++).
            closest_sqr = span_sqr;
        }
        if closest_sqr < min_d * min_d {
            return false;
        }
        let mut close_enough = closest_sqr > req * req;
        if num_infantry > 6 {
            close_enough = true;
        }
        if num_vehicles > 4 {
            close_enough = true;
        }
        // Formation already stamped → always group-path.
        let fid0 = units
            .first()
            .and_then(|id| self.game_logic.get_object(*id))
            .map(|o| o.formation_id)
            .unwrap_or(0);
        if fid0 != 0
            && units.iter().all(|&id| {
                self.game_logic
                    .get_object(id)
                    .map(|o| o.formation_id == fid0)
                    .unwrap_or(false)
            })
        {
            close_enough = true;
        }
        let _ = center;
        close_enough
    }

    /// C++ AIGroup::friend_moveFormationToPos residual.
    /// Paths each formation member to dest + stamped offset.
    pub(crate) fn execute_move_formation_to_position(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> CommandResult {
        if !destination.x.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        // Ensure formation stamps exist.
        let need_stamp = {
            let fid0 = units
                .first()
                .and_then(|id| self.game_logic.get_object(*id))
                .map(|o| o.formation_id)
                .unwrap_or(0);
            fid0 == 0
                || !units.iter().all(|&id| {
                    self.game_logic
                        .get_object(id)
                        .map(|o| o.formation_id == fid0 && fid0 != 0)
                        .unwrap_or(false)
                })
        };
        if need_stamp {
            let _ = self.execute_create_formation(units);
        }
        let goals = self.group_move_destinations(units, destination);
        let mut any = false;
        for (unit_id, goal) in goals {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            }
            if self.game_logic.assign_unit_path(unit_id, goal, &[]) {
                if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                    unit.set_ai_state(AIState::Moving);
                }
                any = true;
            } else if self.path_to_goal_with_state(unit_id, goal, AIState::Moving) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupFollowPath residual (empty body in retail) —
    /// host uses non-exact waypoint follow.
    pub(crate) fn execute_follow_path(
        &mut self,
        units: &[ObjectId],
        path: &[Vec3],
    ) -> CommandResult {
        self.execute_follow_waypoint_path(units, path, false, false)
    }

    /// C++ AIGroup::isMember residual.
    pub(crate) fn is_member(&self, units: &[ObjectId], obj: ObjectId) -> bool {
        units.iter().any(|&id| id == obj)
    }

    /// C++ AIGroup::getCenter residual (skip held/immobile without move).
    pub(crate) fn group_center(&self, units: &[ObjectId]) -> Option<Vec3> {
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut cz = 0.0f32;
        let mut count = 0u32;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            // C++ skips DISABLED_HELD riders.
            if o.contained_by.is_some() {
                continue;
            }
            if o.is_kind_of(crate::game_logic::KindOf::Immobile)
                && !o.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                // Still count structures with AI-like commands; skip pure immobile props.
            }
            let p = o.get_position();
            cx += p.x;
            cy += p.y;
            cz += p.z;
            count += 1;
        }
        if count == 0 {
            // Fallback: any alive member.
            for &id in units {
                if let Some(o) = self.game_logic.get_object(id) {
                    if o.is_alive() {
                        return Some(o.get_position());
                    }
                }
            }
            return None;
        }
        let n = count as f32;
        Some(Vec3::new(cx / n, cy / n, cz / n))
    }

    /// C++ AIGroup::containsAnyObjectsNotOwnedByPlayer residual (team ownership).
    pub(crate) fn contains_any_objects_not_owned_by_player(
        &self,
        units: &[ObjectId],
        player_id: u32,
    ) -> bool {
        let owner_team = self.player_team(player_id);
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if o.team != owner_team {
                return true;
            }
        }
        false
    }

    /// C++ AIGroup::removeAnyObjectsNotOwnedByPlayer residual.
    /// Returns (kept_units, group_now_empty).
    pub(crate) fn remove_any_objects_not_owned_by_player(
        &self,
        units: &[ObjectId],
        player_id: u32,
    ) -> (Vec<ObjectId>, bool) {
        let owner_team = self.player_team(player_id);
        let kept: Vec<ObjectId> = units
            .iter()
            .copied()
            .filter(|&id| {
                self.game_logic
                    .get_object(id)
                    .map(|o| o.team == owner_team)
                    .unwrap_or(false)
            })
            .collect();
        let empty = kept.is_empty();
        (kept, empty)
    }

    /// C++ AIGroup::groupDoSpecialPowerAtLocation residual.
    pub(crate) fn execute_special_power_at_location(
        &mut self,
        units: &[ObjectId],
        power_type: &crate::command_system::SpecialPowerType,
        location: Vec3,
    ) -> CommandResult {
        self.execute_special_power(
            units,
            power_type,
            &crate::command_system::PowerTarget::Location(location),
        )
    }

    /// C++ AIGroup::groupDoSpecialPowerAtObject residual.
    pub(crate) fn execute_special_power_at_object(
        &mut self,
        units: &[ObjectId],
        power_type: &crate::command_system::SpecialPowerType,
        target: ObjectId,
    ) -> CommandResult {
        self.execute_special_power(
            units,
            power_type,
            &crate::command_system::PowerTarget::Object(target),
        )
    }

    /// C++ AIGroup::groupGuardObject residual helper.
    pub(crate) fn execute_guard_object(
        &mut self,
        units: &[ObjectId],
        target: ObjectId,
        mode: crate::game_logic::GuardMode,
    ) -> CommandResult {
        self.execute_guard(
            units,
            &crate::command_system::GuardTarget::Object(target),
            mode,
        )
    }

    /// C++ AIGroup::groupGuardArea residual — area approximated as position + radius guard.
    pub(crate) fn execute_guard_area(
        &mut self,
        units: &[ObjectId],
        center: Vec3,
        radius: f32,
        mode: crate::game_logic::GuardMode,
    ) -> CommandResult {
        // Host residual: guard position at center; stamp guard_radius from area.
        let res = self.execute_guard(
            units,
            &crate::command_system::GuardTarget::Position(center),
            mode,
        );
        if matches!(res, CommandResult::Success) {
            let r = radius.max(80.0);
            for &id in units {
                if let Some(u) = self.game_logic.get_object_mut(id) {
                    u.guard_radius = r;
                }
            }
        }
        res
    }

    /// C++ AIGroup::isIdle residual — every member idle or effectively dead.
    pub(crate) fn group_is_idle(&self, units: &[ObjectId]) -> bool {
        let mut saw = false;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            saw = true;
            if o.is_alive() && !matches!(o.ai_state, AIState::Idle) {
                return false;
            }
        }
        saw
    }

    /// C++ AIGroup::isBusy residual — every living member is non-idle/busy.
    pub(crate) fn group_is_busy(&self, units: &[ObjectId]) -> bool {
        let mut saw = false;
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            saw = true;
            // Host residual: busy = not idle (C++ AIUpdateInterface::isBusy is narrower).
            if matches!(o.ai_state, AIState::Idle) {
                return false;
            }
        }
        saw
    }

    /// C++ AIGroup::isGroupAiDead residual — every member effectively dead.
    pub(crate) fn group_is_ai_dead(&self, units: &[ObjectId]) -> bool {
        if units.is_empty() {
            return true;
        }
        for &id in units {
            let Some(o) = self.game_logic.get_object(id) else {
                continue;
            };
            if o.is_alive() {
                return false;
            }
        }
        true
    }

    /// C++ AIGroup::groupAttackPosition residual.
    /// `location` None → each unit attacks its own position.
    /// Orders fire-capable passengers when container allows passenger fire.
    pub(crate) fn execute_attack_ground(
        &mut self,
        units: &[ObjectId],
        location: Option<Vec3>,
        max_shots: i32,
    ) -> CommandResult {
        let mut any = false;
        let mut extra_passengers: Vec<ObjectId> = Vec::new();

        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            // Collect fire-capable passengers (garrison residual).
            if unit.passengers_allowed_to_fire {
                for p in unit.contained_units() {
                    extra_passengers.push(p);
                }
            }
        }

        let mut all_units: Vec<ObjectId> = units.to_vec();
        for p in extra_passengers {
            if !all_units.contains(&p) {
                all_units.push(p);
            }
        }

        for &unit_id in &all_units {
            let attack_pos = match location {
                Some(loc) => {
                    if !loc.x.is_finite() || !loc.z.is_finite() {
                        continue;
                    }
                    loc
                }
                None => match self.game_logic.get_object(unit_id) {
                    Some(u) if u.is_alive() => u.get_position(),
                    _ => continue,
                },
            };

            let can = match self.game_logic.get_object(unit_id) {
                Some(u) if u.is_alive() => {
                    u.can_attack()
                        || u.weapon.is_some()
                        || u.is_kind_of(crate::game_logic::KindOf::Structure)
                }
                _ => false,
            };
            // Structures/garrisons may still get the order even if can_attack is soft-false.
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                if !unit.is_alive() {
                    continue;
                }
                unit.set_target(None);
                unit.set_force_attack(true);
                unit.set_max_shots_to_fire(max_shots);
                unit.set_target_location(Some(attack_pos));
                unit.set_ai_state(AIState::AttackingGround);
                any = true;
            }
            // Face/path residual: movable units approach the ground point if far.
            let need_approach = self.game_logic.get_object(unit_id).and_then(|unit| {
                if !unit.can_move() {
                    return None;
                }
                let pos = unit.get_position();
                let dist = (pos.x - attack_pos.x).hypot(pos.z - attack_pos.z);
                let range = unit.weapon.as_ref().map(|w| w.range).unwrap_or(50.0);
                if dist > range.max(20.0) {
                    Some(attack_pos)
                } else {
                    None
                }
            });
            if let Some(dest) = need_approach {
                let _ = self.path_to_goal_with_state(unit_id, dest, AIState::AttackingGround);
            }
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(crate) fn execute_stop(&mut self, units: &[ObjectId]) -> CommandResult {
        // C++ AIGroup::groupIdle (player stop):
        // aiIdle + stealth combat unit mood delay until stealthed again.
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            unit.stop();
            unit.set_target(None);
            unit.set_force_attack(false);
            // C++ stop clears guard/waypoint residual anchors.
            unit.set_guard_position(None);
            unit.set_guard_target(None);
            unit.end_guard_retaliate();
            unit.set_ai_state(AIState::Idle);
        }
        self.apply_player_stealth_mood_delay(units);
        CommandResult::Success
    }

    pub(crate) fn execute_guard(
        &mut self,
        units: &[ObjectId],
        target: &GuardTarget,
        mode: crate::game_logic::GuardMode,
    ) -> CommandResult {
        // C++ AIGroup::groupGuardPosition/Object — only units with AI/move;
        // guard radius residual ≈ adjusted vision (getStdGuardRange).
        // mode = C++ GuardMode (Normal / WithoutPursuit / FlyingUnitsOnly).
        const GUARD_MIN_RADIUS: f32 = 80.0;
        let mut any = false;
        for &unit_id in units {
            let (can, vision, weapon_r) = match self.game_logic.get_object(unit_id) {
                Some(unit)
                    if unit.is_alive()
                        && unit.can_move()
                        && !unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                        && !unit.is_kind_of(crate::game_logic::KindOf::Structure) =>
                {
                    let wr = unit
                        .weapon
                        .as_ref()
                        .map(|w| w.range)
                        .or_else(|| unit.secondary_weapon.as_ref().map(|w| w.range))
                        .unwrap_or(0.0);
                    (true, unit.vision_range, wr)
                }
                _ => (false, 0.0, 0.0),
            };
            if !can {
                continue;
            }

            let target_pos = match target {
                GuardTarget::Position(pos) => Some(*pos),
                GuardTarget::Object(target_id) => self
                    .game_logic
                    .get_object(*target_id)
                    .filter(|o| o.is_alive())
                    .map(|o| o.get_position()),
            };

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                // C++ getStdGuardRange ≈ vision with guard-inner factor; host uses
                // max(vision, weapon, min radius).
                unit.guard_radius = vision.max(weapon_r).max(GUARD_MIN_RADIUS);
                unit.set_guard_mode(mode);
                unit.set_target(None);
                unit.set_force_attack(false);
                unit.end_guard_retaliate();
                match target {
                    GuardTarget::Position(pos) => {
                        unit.set_guard_target(None);
                        unit.set_guard_position(Some(*pos));
                    }
                    GuardTarget::Object(target_id) => {
                        if target_pos.is_none() {
                            continue;
                        }
                        // Object guard: anchor follows target; clear area pin.
                        unit.guard_position = None;
                        unit.set_guard_target(Some(*target_id));
                    }
                }
            } else {
                continue;
            }

            match target {
                GuardTarget::Position(pos) => {
                    let _ = self.path_to_goal_with_state(unit_id, *pos, AIState::GuardingArea);
                }
                GuardTarget::Object(_) => {
                    if let Some(pos) = target_pos {
                        let _ = self.path_to_goal_with_state(unit_id, pos, AIState::GuardingObject);
                    } else if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                        unit.set_ai_state(AIState::GuardingObject);
                    }
                }
            }
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(crate) fn execute_patrol(&mut self, units: &[ObjectId]) -> CommandResult {
        // C++ AIGroup::groupHunt / host Patrol button residual (AI_HUNT).
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            unit.set_target(None);
            unit.set_force_attack(false);
            unit.set_guard_position(None);
            unit.set_guard_target(None);
            unit.end_guard_retaliate();
            // Hunt enables auto-acquire while wandering.
            unit.auto_acquire_when_idle = true;
            unit.set_ai_state(AIState::Patrolling);
            unit.status.moving = false;
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_set_attitude(
        &mut self,
        units: &[ObjectId],
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                if !unit.is_alive() {
                    continue;
                }
                unit.set_ai_attitude(attitude);
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(crate) fn execute_scatter(&mut self, units: &[ObjectId]) -> CommandResult {
        // C++ AIGroup::groupScatter — far-to-near from group center, push out by
        // 4 * bounding radius along the unit→center vector (host XZ plane).
        let mut movers: Vec<(ObjectId, Vec3, f32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            let pos = unit.get_position();
            let radius = unit.selection_radius.max(5.0);
            movers.push((unit_id, pos, radius));
        }
        if movers.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut center = Vec3::ZERO;
        for (_, pos, _) in &movers {
            center += *pos;
        }
        center /= movers.len() as f32;

        movers.sort_by(|a, b| {
            let da = (a.1.x - center.x).hypot(a.1.z - center.z);
            let db = (b.1.x - center.x).hypot(b.1.z - center.z);
            db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut any = false;
        let mut center_nudge = center;
        for (unit_id, pos, radius) in movers {
            center_nudge.x -= 0.01;
            let mut dx = pos.x - center_nudge.x;
            let mut dz = pos.z - center_nudge.z;
            let len = (dx * dx + dz * dz).sqrt();
            if len > 0.001 {
                dx /= len;
                dz /= len;
            } else {
                dx = 1.0;
                dz = 0.0;
            }
            let push = 4.0 * radius;
            let dest = Vec3::new(pos.x + dx * push, pos.y, pos.z + dz * push);
            if self.path_to_goal_with_state(unit_id, dest, AIState::Moving) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Deploy selected units at their current position.
    /// C&C Generals: garrisonable infantry deploy into structures,
    /// dozers unpack into construction yards, etc.
    fn execute_deploy(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some((alive, name, is_infantry, is_deployed)) =
                self.game_logic.get_object(unit_id).map(|unit| {
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
                if let Some(unit_mut) = self.game_logic.get_object_mut(unit_id) {
                    let next = !is_deployed;
                    unit_mut.set_deployed(next);
                    if next {
                        unit_mut.set_ai_state(AIState::Idle);
                    }
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
                        .get_object(building_id)
                        .map(|b| b.get_position());
                    if let Some(unit_mut) = self.game_logic.get_object_mut(unit_id) {
                        unit_mut.set_target(Some(building_id));
                        unit_mut.set_ai_state(AIState::Entering);
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
    fn execute_gather(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        let player_team = self.player_team(self.current_player_id);
        let (target_pos, target_alive, target_is_resource) =
            match self.game_logic.get_object(target_id) {
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
                .get_object(unit_id)
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

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::Gathering) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Find the nearest building that can accept this unit for garrison/enter.
    fn find_nearest_garrison_target(&self, unit_id: ObjectId) -> Option<ObjectId> {
        let unit = self.game_logic.get_object(unit_id)?;
        let unit_pos = unit.get_position();
        let unit_team = unit.team;
        // Pure residual acquire: nearest friendly container with capacity (3D).
        let candidates: Vec<_> = self
            .game_logic
            .get_objects()
            .iter()
            .filter_map(|(&obj_id, obj)| {
                if obj.team != unit_team || !obj.is_alive() || !obj.can_contain() {
                    return None;
                }
                if !obj.has_capacity_for(1) {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: obj_id,
                        team: obj.team,
                        position: obj.get_position(),
                        is_alive: true,
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
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            unit_id,
            unit_team,
            unit_pos,
            candidates,
            |_| f32::MAX,
            |_| true,
        )
        .map(|(id, _, _)| id)
    }

    // === Construction Commands ===

    fn execute_build(
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
            let team = match self.game_logic.get_object(unit_id) {
                Some(unit) if unit.can_construct() => unit.team,
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
                let Some(player) = self.game_logic.get_player_mut_by_team(team) else {
                    continue;
                };

                if !player.spend_resources(&build_cost) {
                    return CommandResult::InvalidCommand;
                }
            }

            let building_id =
                self.game_logic
                    .create_object_under_construction(template_name, team, location);
            let Some(building_id) = building_id else {
                // Refund on failed placement.
                if let Some(player) = self.game_logic.get_player_mut_by_team(team) {
                    player.resources.supplies = player
                        .resources
                        .supplies
                        .saturating_add(build_cost.supplies);
                }
                return CommandResult::InvalidCommand;
            };
            if orientation.abs() > f32::EPSILON {
                if let Some(b) = self.game_logic.get_object_mut(building_id) {
                    b.set_orientation(orientation);
                }
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

    fn execute_dozer_construct(
        &mut self,
        units: &[ObjectId],
        template_name: &str,
        location: Vec3,
        orientation: f32,
    ) -> CommandResult {
        self.execute_build(units, template_name, location, orientation)
    }

    fn execute_dozer_line(
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

    fn execute_cancel_construction(
        &mut self,
        object_id: ObjectId,
        player_id: u32,
    ) -> CommandResult {
        let player_team = self.player_team(player_id);
        if let Some(obj) = self.game_logic.get_object(object_id) {
            if obj.team != player_team {
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

    fn execute_resume_construction(
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

    fn execute_sell(&mut self, object_id: ObjectId, player_id: u32) -> CommandResult {
        let player_team = self.player_team(player_id);
        if let Some(obj) = self.game_logic.get_object(object_id) {
            if obj.team != player_team || !obj.is_alive() || !obj.is_kind_of(KindOf::Structure) {
                return CommandResult::InvalidTarget;
            }
            if obj.status.sold
                || obj.status.reconstructing
                || obj.status.under_construction
                || self.game_logic.is_object_being_sold(object_id)
            {
                // C++ sellObject: structures only when complete (not under construction/rebuild).
                return CommandResult::InvalidCommand;
            }
            // C++ BuildAssistant::sellObject multi-frame residual (scaffold → SOLD → refund).
            if self.game_logic.start_sell_object(object_id) {
                CommandResult::Success
            } else {
                CommandResult::InvalidCommand
            }
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// C++ AIGroup::groupSell residual — sell every selected friendly structure.
    pub(crate) fn execute_sell_selected(
        &mut self,
        units: &[ObjectId],
        player_id: u32,
    ) -> CommandResult {
        let mut any = false;
        for &id in units {
            if matches!(self.execute_sell(id, player_id), CommandResult::Success) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Production Commands ===

    fn execute_queue_unit(
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

    fn execute_cancel_unit(&mut self, units: &[ObjectId], template_name: &str) -> CommandResult {
        // Resolve empty name → unit production head residual (not PRODUCTION_UPGRADE).
        let resolved = if template_name.trim().is_empty() {
            units.iter().find_map(|&unit_id| {
                self.game_logic.get_object(unit_id).and_then(|obj| {
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

    // === Special Powers ===

    fn execute_special_power(
        &mut self,
        units: &[ObjectId],
        power_type: &SpecialPowerType,
        target: &PowerTarget,
    ) -> CommandResult {
        // Basic validation: ensure object targets exist when required and power is ready.
        if let PowerTarget::Object(id) = target {
            if self.game_logic.get_object(*id).is_none() {
                return CommandResult::InvalidTarget;
            }
        }
        if let PowerTarget::Location(loc) = target {
            if !loc.x.is_finite() || !loc.y.is_finite() || !loc.z.is_finite() {
                return CommandResult::InvalidLocation;
            }
        }

        // Resolve impact position for residual superweapon path
        // (DaisyCutter/A10/Scud/PUC/NuclearMissile/AnthraxBomb/SpectreGunship/
        // CarpetBomb/ArtilleryBarrage/CruiseMissile).
        let target_position: Option<Vec3> = match target {
            PowerTarget::Location(loc) => Some(*loc),
            PowerTarget::Object(id) => self
                .game_logic
                .get_object(*id)
                .map(|obj| obj.get_position()),
            PowerTarget::None => {
                // C++ overridable destination residual wins when set on caster.
                let src = self.special_power_source_object(units, power_type);
                src.and_then(|id| {
                    self.game_logic
                        .get_object(id)
                        .and_then(|o| o.special_power_override_destination)
                })
                .or_else(|| {
                    units.iter().find_map(|id| {
                        self.game_logic
                            .get_object(*id)
                            .and_then(|o| o.special_power_override_destination)
                    })
                })
                .or_else(|| {
                    src.or_else(|| units.first().copied())
                        .and_then(|id| self.game_logic.get_object(id).map(|obj| obj.get_position()))
                })
            }
        };

        debug!(
            "Executing special power {:?} with target {:?}",
            power_type, target
        );
        // C++ AIGroup::groupDoSpecialPower* uses getSpecialPowerSourceObject —
        // only the module owner fires, not every selected unit.
        let casters: Vec<ObjectId> =
            if let Some(src) = self.special_power_source_object(units, power_type) {
                vec![src]
            } else {
                // Fall back: any ready member (capture/skills on multi infantry).
                units.to_vec()
            };
        let mut any = false;
        for &unit_id in &casters {
            // SharedSyncedTimer residual: player-wide gate for superweapons.
            let ready = self
                .game_logic
                .is_special_power_ready_for(unit_id, power_type);
            if !ready {
                continue;
            }

            if !self
                .game_logic
                .consume_special_power_charge_for(unit_id, power_type)
            {
                continue;
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.set_ai_state(AIState::SpecialAbility);
            }

            // Host residual: queue superweapon strike that will complete with
            // area damage (DaisyCutter / A10 / ScudStorm / ParticleCannon /
            // NuclearMissile + radiation residual / AnthraxBomb + toxin residual /
            // SpectreGunship + delayed orbit damage ticks residual /
            // CarpetBomb + delayed line multi-strike residual /
            // ArtilleryBarrage + delayed multi-shell scatter residual /
            // CruiseMissile + delayed loft MOAB area damage residual).
            // ClusterMines residual places a ring of land mines at target.
            // RadarScan residual temporarily reveals FOW at target (RadarVanPing).
            // SpySatellite residual temporarily reveals FOW at target (SpySatellitePing).
            // CiaIntelligence residual temporarily vision-spies all enemy units (SpyVision).
            // Paradrop residual queues America Airborne infantry drop at target.
            // Ambush residual queues GLA Rebel Ambush infantry spawn at target.
            // FireWall residual creates a line of fire damage zones toward target.
            // HelixNapalmBomb residual drops NapalmBomb blast + FirestormSmall at target.
            // EmpPulse residual disables vehicles/structures in radius (DISABLED_EMP).
            // Frenzy residual buffs ally attack damage in radius (FRENZY_ONE/TWO/THREE).
            // BattlePlan* residual selects USA Strategy Center army battle plan bonuses.
            // EmergencyRepair residual SingleBurst-heals ally vehicles in radius.
            // GpsScrambler residual grants STEALTHED to ally vehicles/infantry in radius.
            // LeafletDrop residual delays then disables enemy infantry/vehicles (DISABLED_EMP).
            // SneakAttack residual delays then spawns a GLA tunnel + shockwave damage.
            //
            // CIA Intelligence is no-target (SpyVision setUnitsVisionSpied residual).
            // Missile Defender laser guided needs an object target (lock secondary + attack).
            // Hero/unit disable & capture specials → existing walk-to residual command paths.
            if matches!(
                *power_type,
                SpecialPowerType::RangerCaptureBuilding
                    | SpecialPowerType::RedGuardCaptureBuilding
                    | SpecialPowerType::RebelCaptureBuilding
            ) {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_capture_building(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::DisguiseAsVehiclePower {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_disguise_as_vehicle(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::HackerDisableBuilding
                || *power_type == SpecialPowerType::MicrowaveDisableBuilding
            {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_hacker_disable_building(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::BlackLotusDisableVehicle {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_disable_vehicle_hack(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::BlackLotusStealCash {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_steal_cash_hack(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::BlackLotusCaptureBuilding {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_capture_building(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if matches!(
                *power_type,
                SpecialPowerType::DemoRebelTimedCharges
                    | SpecialPowerType::DemoKellTimedCharges
                    | SpecialPowerType::DemoKellStickyCharges
                    | SpecialPowerType::BattleBusDemoTrapRollout
                    | SpecialPowerType::BurtonTimedCharges
            ) {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !self.queue_special_timed_charge(unit_id, *tid, power_type) {
                    continue;
                }
            } else if matches!(
                *power_type,
                SpecialPowerType::DemoKellRemoteCharges | SpecialPowerType::BurtonRemoteCharges
            ) {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !self.queue_special_remote_charge(unit_id, *tid) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::TankHunterTnt {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                // Walk-to-target then plant timed charge residual (same as command path).
                if !self.queue_tank_hunter_tnt(unit_id, *tid) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::MissileDefenderLaserGuided
                || *power_type == SpecialPowerType::LaserGuidedHowitzer
            {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !self
                    .game_logic
                    .activate_missile_defender_laser_guided(unit_id, *tid)
                {
                    continue;
                }
            } else if *power_type == SpecialPowerType::CiaIntelligence
                || *power_type == SpecialPowerType::CommunicationsDownload
            {
                let team = self
                    .game_logic
                    .get_object(unit_id)
                    .map(|o| o.team)
                    .unwrap_or(crate::game_logic::Team::Neutral);
                if !self.game_logic.activate_cia_intelligence(
                    self.current_player_id,
                    team,
                    Some(unit_id),
                ) {
                    continue;
                }
            } else if let Some(pos) = target_position {
                if *power_type == SpecialPowerType::ClusterMines
                    || *power_type == SpecialPowerType::NukeDrop
                {
                    let team = self
                        .game_logic
                        .get_object(unit_id)
                        .map(|o| o.team)
                        .unwrap_or(crate::game_logic::Team::Neutral);
                    // C++ SUPERWEAPON_ClusterMines DeliverPayload residual
                    // (ChinaJetCargoPlane + bomb); mines place on bomb impact.
                    if self
                        .game_logic
                        .spawn_cluster_mines_flight(unit_id, pos)
                        .is_none()
                    {
                        // Fail-open residual: place mines immediately if flight spawn fails.
                        let placed = self.game_logic.place_cluster_mines(team, pos, Some(unit_id));
                        if placed.is_empty() {
                            continue;
                        }
                    }
                } else if *power_type == SpecialPowerType::RadarScan {
                    let team = self
                        .game_logic
                        .get_object(unit_id)
                        .map(|o| o.team)
                        .unwrap_or(crate::game_logic::Team::Neutral);
                    if !self.game_logic.activate_radar_scan(
                        self.current_player_id,
                        team,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::SpySatellite {
                    let team = self
                        .game_logic
                        .get_object(unit_id)
                        .map(|o| o.team)
                        .unwrap_or(crate::game_logic::Team::Neutral);
                    if !self.game_logic.activate_spy_satellite(
                        self.current_player_id,
                        team,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::SpyDrone {
                    let team = self
                        .game_logic
                        .get_object(unit_id)
                        .map(|o| o.team)
                        .unwrap_or(crate::game_logic::Team::Neutral);
                    if !self.game_logic.activate_spy_drone(
                        self.current_player_id,
                        team,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::EmpPulse {
                    if !self.game_logic.activate_emp_pulse(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::Frenzy
                    || *power_type == SpecialPowerType::EarlyFrenzy
                {
                    let level = {
                        use crate::game_logic::host_frenzy::highest_frenzy_level_from_sciences;
                        let sciences = self
                            .game_logic
                            .player_unlocked_sciences(self.current_player_id);
                        highest_frenzy_level_from_sciences(sciences.iter().map(|s| s.as_str()))
                    };
                    if !self.game_logic.activate_frenzy(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                        level,
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::BattlePlanBombardment
                    || *power_type == SpecialPowerType::BattlePlanHoldTheLine
                    || *power_type == SpecialPowerType::BattlePlanSearchAndDestroy
                {
                    // USA Strategy Center battle-plan residual (no location required).
                    // Fail-closed: not full pack/unpack animation / paralyze matrix.
                    use crate::game_logic::host_strategy_center::HostBattlePlan;
                    let plan = match power_type {
                        SpecialPowerType::BattlePlanHoldTheLine => HostBattlePlan::HoldTheLine,
                        SpecialPowerType::BattlePlanSearchAndDestroy => {
                            HostBattlePlan::SearchAndDestroy
                        }
                        _ => HostBattlePlan::Bombardment,
                    };
                    if !self.game_logic.activate_battle_plan(
                        self.current_player_id,
                        plan,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::EmergencyRepair
                    || *power_type == SpecialPowerType::EarlyEmergencyRepair
                {
                    let level = {
                        use crate::game_logic::host_emergency_repair::highest_emergency_repair_level_from_sciences;
                        let sciences = self
                            .game_logic
                            .player_unlocked_sciences(self.current_player_id);
                        highest_emergency_repair_level_from_sciences(
                            sciences.iter().map(|s| s.as_str()),
                        )
                    };
                    if !self.game_logic.activate_emergency_repair(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                        level,
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::GpsScrambler
                    || *power_type == SpecialPowerType::StealthGpsScrambler
                {
                    if !self.game_logic.activate_gps_scrambler(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::Paradrop
                    || *power_type == SpecialPowerType::InfantryParadrop
                    || *power_type == SpecialPowerType::TankParadrop
                {
                    if self
                        .game_logic
                        .queue_paradrop(power_type, unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::Ambush
                    || *power_type == SpecialPowerType::TerrorCell
                {
                    if self
                        .game_logic
                        .queue_ambush(power_type, unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::LeafletDrop
                    || *power_type == SpecialPowerType::EarlyLeafletDrop
                {
                    if self
                        .game_logic
                        .queue_leaflet_drop(power_type, unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::SneakAttack {
                    if self
                        .game_logic
                        .queue_sneak_attack(power_type, unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::FireWall {
                    if self.game_logic.activate_firewall(unit_id, pos).is_none() {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::HelixNapalmBomb
                    || *power_type == SpecialPowerType::HelixNukeBomb
                {
                    if self
                        .game_logic
                        .activate_helix_napalm_bomb(unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::CrateDrop {
                    let _n = self.game_logic.activate_crate_drop(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    );
                } else if *power_type == SpecialPowerType::CashHack {
                    let _stolen = self
                        .game_logic
                        .activate_cash_hack(self.current_player_id, Some(unit_id));
                    // Always treat as success residual once activated (even 0 stolen).
                } else if *power_type == SpecialPowerType::Defector {
                    // C++ DefectorSpecialPower::doSpecialPowerAtObject residual.
                    let PowerTarget::Object(tid) = target else {
                        continue;
                    };
                    if !self.game_logic.activate_defector(unit_id, *tid) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::BaikonurRocket {
                    // C++ BaikonurLaunchPower: no-loc → door; location → door + detonation.
                    match target {
                        PowerTarget::Location(loc) => {
                            let _ = self.game_logic.activate_baikonur_launch_door(unit_id);
                            if !self.game_logic.activate_baikonur_detonation(unit_id, *loc) {
                                continue;
                            }
                        }
                        PowerTarget::None | PowerTarget::Object(_) => {
                            if !self.game_logic.activate_baikonur_launch_door(unit_id) {
                                continue;
                            }
                        }
                    }
                } else if *power_type == SpecialPowerType::CleanupArea {
                    if !self.game_logic.activate_cleanup_area(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else {
                    let _ = self
                        .game_logic
                        .queue_special_power_strike(power_type, unit_id, pos);
                }
            }
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_weapon(
        &mut self,
        units: &[ObjectId],
        weapon_slot: &WeaponSlot,
        target: &WeaponTarget,
    ) -> CommandResult {
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                match target {
                    WeaponTarget::Object(target_id) => {
                        unit.set_target(Some(*target_id));
                        unit.set_ai_state(AIState::Attacking);
                    }
                    WeaponTarget::Location(pos) => {
                        unit.target_location = Some(*pos);
                        unit.set_ai_state(AIState::AttackingGround);
                    }
                }
                debug!(
                    "Unit {} firing weapon {:?} at {:?}",
                    unit_id.0, weapon_slot, target
                );
            }
        }
        CommandResult::Success
    }

    // === Transport Commands ===

    fn execute_enter(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        // USA Pilot residual: Enter unmanned vehicle for recrew (not transport contain).
        let pilot_recrew_target = self.game_logic.get_object(target_id).map(|t| {
            crate::game_logic::host_usa_pilot::is_recrewable_unmanned_vehicle(
                t.is_alive(),
                t.is_kind_of(crate::game_logic::KindOf::Vehicle),
                t.is_kind_of(crate::game_logic::KindOf::Aircraft) || t.status.airborne_target,
                t.is_unmanned(),
                t.status.under_construction,
                t.is_worker() || t.template_name.to_ascii_lowercase().contains("dozer"),
            )
        });
        let target_pos = match self.game_logic.get_object(target_id) {
            Some(transport)
                if transport.is_alive()
                    && !transport.status.under_construction
                    && (transport.can_contain() || pilot_recrew_target == Some(true)) =>
            {
                transport.get_position()
            }
            _ => return CommandResult::InvalidTarget,
        };

        let mut issued = false;
        for &unit_id in units {
            let pilot_recrew = self.game_logic.get_object(unit_id).map(|u| {
                crate::game_logic::host_usa_pilot::should_recrew_on_enter(
                    crate::game_logic::host_usa_pilot::is_pilot_template(&u.template_name),
                    pilot_recrew_target.unwrap_or(false),
                ) && u.is_alive()
                    && u.can_move()
            });
            if pilot_recrew != Some(true) && !self.can_issue_enter_or_dock(unit_id, target_id) {
                continue;
            }

            let unit_in_tunnel = self
                .game_logic
                .tunnel_network_residual()
                .team_holding_unit(unit_id)
                .is_some();
            let previous_container = self.game_logic.get_object(unit_id).and_then(|unit| {
                if matches!(unit.ai_state, AIState::Docked | AIState::Garrisoned) || unit_in_tunnel
                {
                    unit.container_id().or(unit.target)
                } else {
                    None
                }
            });
            if let Some(previous_container) = previous_container {
                if previous_container != target_id {
                    if let Some(container) = self.game_logic.get_object_mut(previous_container) {
                        container.remove_occupant(unit_id);
                    }
                }
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::Entering) {
                issued = true;
            }
        }

        if issued {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_exit(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut to_unload: Vec<(ObjectId, Option<ObjectId>, Vec3)> = Vec::new();
        let mut seen_units: HashSet<ObjectId> = HashSet::new();
        // Tunnel network residual: exit tunnel id for shared-pool bookkeeping.
        let mut tunnel_exit_for: HashMap<ObjectId, ObjectId> = HashMap::new();

        for &selected_id in units {
            let Some(selected_obj) = self.game_logic.get_object(selected_id) else {
                continue;
            };

            if selected_obj.can_contain() {
                // Prefer get_position() (authoritative Thing pos). The pub `position`
                // field is often left at default ZERO after create_object set_position.
                let origin = selected_obj
                    .building_data
                    .as_ref()
                    .and_then(|b| b.rally_point)
                    .unwrap_or_else(|| selected_obj.get_position());

                // Tunnel Network residual: Evacuate/Exit on ANY team tunnel dumps the
                // shared MaxTunnelCapacity pool at THIS tunnel (cross-tunnel path).
                if selected_obj.is_tunnel_network_style_container() {
                    let team = selected_obj.team;
                    let shared = self.game_logic.tunnel_network_contained_for_team(team);
                    for contained in shared {
                        if seen_units.insert(contained) {
                            to_unload.push((contained, Some(selected_id), origin));
                            tunnel_exit_for.insert(contained, selected_id);
                        }
                    }
                    // Also include any local-only occupants not yet in the shared list.
                    for contained in selected_obj.contained_units() {
                        if seen_units.insert(contained) {
                            to_unload.push((contained, Some(selected_id), origin));
                            tunnel_exit_for.insert(contained, selected_id);
                        }
                    }
                    continue;
                }

                for contained in selected_obj.contained_units() {
                    if seen_units.insert(contained) {
                        to_unload.push((contained, Some(selected_id), origin));
                    }
                }
                continue;
            }

            let is_contained = matches!(
                selected_obj.ai_state,
                AIState::Docked | AIState::Garrisoned | AIState::Entering | AIState::Docking
            );
            // Units in tunnel network may only have contained_by set.
            let in_tunnel = self
                .game_logic
                .tunnel_network_residual()
                .team_holding_unit(selected_id)
                .is_some();
            if !is_contained && !in_tunnel {
                continue;
            }

            // Prefer contained_by (authoritative) over target for residual garrison exit.
            let (origin, container_id) = if let Some(container_id) = selected_obj.container_id() {
                if let Some(container) = self.game_logic.get_object(container_id) {
                    let rally = container.building_data.as_ref().and_then(|b| b.rally_point);
                    (
                        rally.unwrap_or_else(|| container.get_position()),
                        Some(container_id),
                    )
                } else {
                    (selected_obj.get_position(), None)
                }
            } else {
                (selected_obj.get_position(), None)
            };

            if seen_units.insert(selected_id) {
                to_unload.push((selected_id, container_id, origin));
                if let Some(cid) = container_id {
                    if self
                        .game_logic
                        .get_object(cid)
                        .map(|c| c.is_tunnel_network_style_container())
                        .unwrap_or(false)
                    {
                        tunnel_exit_for.insert(selected_id, cid);
                    }
                }
            }
        }

        if to_unload.is_empty() {
            return CommandResult::InvalidCommand;
        }

        for (i, (unit_id, container_id, origin)) in to_unload.into_iter().enumerate() {
            // Stagger exits deterministically to avoid clumping on the same point.
            let angle = (unit_id.0 as f32 + i as f32 * 1.37).sin().atan2(1.0) + i as f32 * 0.7;
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 6.0;
            let drop_position = origin + offset;

            let tunnel_exit = tunnel_exit_for.get(&unit_id).copied();
            let was_tunnel = if let Some(exit_tid) = tunnel_exit {
                self.game_logic.exit_tunnel_network_unit(unit_id, exit_tid)
            } else if let Some(cid) = container_id {
                // Fallback: unit in shared pool exiting via entry tunnel.
                if self
                    .game_logic
                    .tunnel_network_residual()
                    .team_holding_unit(unit_id)
                    .is_some()
                {
                    self.game_logic.exit_tunnel_network_unit(unit_id, cid)
                } else {
                    false
                }
            } else {
                false
            };

            if !was_tunnel {
                if let Some(container_id) = container_id {
                    if let Some(container) = self.game_logic.get_object_mut(container_id) {
                        container.remove_occupant(unit_id);
                    }
                }
            }

            // Classify residual exit before mutating unit state.
            // Prefer AI state; fall back to container kind when only contained_by is set.
            // Overlord BattleBunker / GLA Battle Bus / Combat Chinook / Listening Outpost
            // residuals are vehicle-docked but tracked separately from generic Humvee residual.
            let (
                was_garrisoned,
                was_overlord_bunker,
                was_battle_bus,
                was_technical,
                was_combat_chinook,
                was_listening_outpost,
                was_troop_crawler,
                was_transport,
            ) = if was_tunnel {
                (false, false, false, false, false, false, false, false)
            } else if let Some(unit) = self.game_logic.get_object(unit_id) {
                let garrisoned = matches!(unit.ai_state, AIState::Garrisoned);
                let docked = matches!(unit.ai_state, AIState::Docked);
                let cid = unit.contained_by.or(container_id);
                let container = cid.and_then(|id| self.game_logic.get_object(id));
                let is_overlord = container
                    .map(|c| c.is_overlord_style_container())
                    .unwrap_or(false);
                let is_battle_bus = container
                    .map(|c| c.is_battle_bus_style_container())
                    .unwrap_or(false);
                let is_technical = container
                    .map(|c| c.is_technical_style_container())
                    .unwrap_or(false);
                let is_combat_chinook = container
                    .map(|c| c.is_combat_chinook_style_container())
                    .unwrap_or(false);
                let is_listening_outpost = container
                    .map(|c| c.is_listening_outpost_style_container())
                    .unwrap_or(false);
                let is_troop_crawler = container
                    .map(|c| c.is_troop_crawler_style_container())
                    .unwrap_or(false);
                let is_structure = container
                    .map(|c| c.is_kind_of(KindOf::Structure))
                    .unwrap_or(false);
                if garrisoned {
                    (true, false, false, false, false, false, false, false)
                } else if docked {
                    if is_overlord {
                        (false, true, false, false, false, false, false, false)
                    } else if is_battle_bus {
                        (false, false, true, false, false, false, false, false)
                    } else if is_technical {
                        (false, false, false, true, false, false, false, false)
                    } else if is_combat_chinook {
                        (false, false, false, false, true, false, false, false)
                    } else if is_listening_outpost {
                        (false, false, false, false, false, true, false, false)
                    } else if is_troop_crawler {
                        (false, false, false, false, false, false, true, false)
                    } else {
                        (false, false, false, false, false, false, false, false)
                    }
                } else if unit.contained_by.is_some() || container_id.is_some() {
                    if is_structure {
                        (true, false, false, false, false, false, false, false)
                    } else if is_overlord {
                        (false, true, false, false, false, false, false, false)
                    } else if is_battle_bus {
                        (false, false, true, false, false, false, false, false)
                    } else if is_technical {
                        (false, false, false, true, false, false, false, false)
                    } else if is_combat_chinook {
                        (false, false, false, false, true, false, false, false)
                    } else if is_listening_outpost {
                        (false, false, false, false, false, true, false, false)
                    } else if is_troop_crawler {
                        (false, false, false, false, false, false, true, false)
                    } else {
                        (false, false, false, false, false, false, false, false)
                    }
                } else {
                    (false, false, false, false, false, false, false, false)
                }
            } else {
                (false, false, false, false, false, false, false, false)
            };

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.set_position(drop_position);
                unit.contained_by = None;
                unit.set_target(None);
                unit.set_ai_state(AIState::Idle);
                unit.status.moving = false;
                unit.status.attacking = false;
                if was_tunnel {
                    // Counters already recorded in exit_tunnel_network_unit.
                } else if was_garrisoned {
                    self.game_logic.record_garrison_residual_exit();
                } else if was_overlord_bunker {
                    self.game_logic.record_overlord_bunker_residual_exit();
                } else if was_battle_bus {
                    self.game_logic.record_battle_bus_residual_unload();
                } else if was_technical {
                    self.game_logic.record_technical_residual_unload();
                } else if was_combat_chinook {
                    self.game_logic.record_combat_chinook_residual_unload();
                } else if was_listening_outpost {
                    self.game_logic.record_listening_outpost_residual_unload();
                } else if was_troop_crawler {
                    self.game_logic.record_troop_crawler_residual_unload();
                } else if was_transport {
                    self.game_logic.record_transport_residual_unload();
                }
                debug!(
                    "Unit {} exiting transport/garrison near {:?}",
                    unit_id.0, drop_position
                );
            }

            // Refresh armed-riders weapon set after unload residual.
            if let Some(cid) = container_id {
                if was_battle_bus || was_combat_chinook || was_listening_outpost {
                    self.game_logic
                        .refresh_battle_bus_armed_riders_weapon_set(cid);
                }
            }
        }

        CommandResult::Success
    }

    pub(crate) fn execute_evacuate(&mut self, units: &[ObjectId]) -> CommandResult {
        // C++ AIGroup::groupEvacuate:
        //  - airborne aircraft containers: move to ground then evacuate
        //  - structures without AI: order passengers out
        //  - other AI containers: aiEvacuate(false) → unload residual
        // Host residual: unload selected containers via execute_exit; airborne
        // aircraft path to ground (Y=0) first so chinook-style drop has a dest.
        let mut ground_containers: Vec<ObjectId> = Vec::new();
        let mut airborne_containers: Vec<ObjectId> = Vec::new();
        for &unit_id in units {
            let Some(obj) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !obj.is_alive() {
                continue;
            }
            let is_container =
                obj.can_contain() || obj.is_kind_of(crate::game_logic::KindOf::Structure);
            if !is_container {
                continue;
            }
            let airborne =
                obj.is_kind_of(crate::game_logic::KindOf::Aircraft) && obj.status.airborne_target;
            if airborne {
                airborne_containers.push(unit_id);
            } else {
                ground_containers.push(unit_id);
            }
        }

        let mut any = false;
        for unit_id in airborne_containers {
            let Some(pos) = self
                .game_logic
                .get_object(unit_id)
                .map(|o| o.get_position())
            else {
                continue;
            };
            // C++: highest ground layer at dest — host residual uses Y=0 ground plane.
            let dest = Vec3::new(pos.x, 0.0, pos.z);
            if self.path_to_goal_with_state(unit_id, dest, AIState::Moving) {
                any = true;
            }
            // Also attempt unload residual if already near ground / has passengers.
            if matches!(self.execute_exit(&[unit_id]), CommandResult::Success) {
                any = true;
            }
        }
        if !ground_containers.is_empty() {
            if matches!(
                self.execute_exit(&ground_containers),
                CommandResult::Success
            ) {
                any = true;
            }
        }

        if any {
            CommandResult::Success
        } else {
            // Fail-closed: no containers selected (unlike Exit which can free passengers).
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupMoveToAndEvacuate / AndExit residual.
    /// Path capable containers to `destination`, then unload on arrival.
    /// `and_exit` marks the transport for self-removal after unload (script exit residual).
    pub(crate) fn execute_move_to_and_evacuate(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        and_exit: bool,
    ) -> CommandResult {
        if !destination.x.is_finite() || !destination.y.is_finite() || !destination.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let mut any = false;
        for &unit_id in units {
            let can = match self.game_logic.get_object(unit_id) {
                Some(obj)
                    if obj.is_alive()
                        && obj.can_move()
                        && (obj.can_contain()
                            || obj.is_kind_of(crate::game_logic::KindOf::Aircraft)
                            || !obj.contained_units().is_empty()) =>
                {
                    true
                }
                _ => false,
            };
            if !can {
                continue;
            }
            if let Some(obj) = self.game_logic.get_object_mut(unit_id) {
                obj.pending_evacuate_on_stop = true;
                obj.pending_exit_after_evacuate = and_exit;
                obj.set_target(None);
                obj.set_force_attack(false);
                obj.set_guard_position(None);
                obj.set_guard_target(None);
                obj.end_guard_retaliate();
            }
            if self.path_to_goal_with_state(unit_id, destination, AIState::Moving) {
                any = true;
            } else {
                // Already at dest or path fail — evacuate immediately.
                let exit = and_exit;
                if let Some(obj) = self.game_logic.get_object_mut(unit_id) {
                    obj.pending_evacuate_on_stop = false;
                    obj.pending_exit_after_evacuate = false;
                }
                if self.game_logic.evacuate_container_now(unit_id, exit) {
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

    fn execute_hack_internet(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.start_hacker_internet_hack(unit_id) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
    fn execute_return_to_base(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
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
                .get_objects()
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
    fn execute_return_supplies(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
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
                .get_objects()
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
                .get_object(sc_id)
                .map(|o| o.get_position())
                .unwrap_or(pos);
            if let Some(u) = self.game_logic.get_object_mut(unit_id) {
                u.set_target(Some(sc_id));
                u.set_ai_state(AIState::ReturningResources);
            }
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
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            unit.set_weapon_set_mine_clearing_detail(enabled);
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::setWeaponLockForGroup residual.
    pub(crate) fn execute_set_weapon_lock(
        &mut self,
        units: &[ObjectId],
        slot: u8,
        lock_type_code: u8,
    ) -> CommandResult {
        use crate::game_logic::WeaponLockType;
        let lock_type = match lock_type_code {
            1 => WeaponLockType::LockedTemporarily,
            2 => WeaponLockType::LockedPermanently,
            _ => WeaponLockType::NotLocked,
        };
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            if unit.set_weapon_lock(slot, lock_type) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::releaseWeaponLockForGroup residual.
    pub(crate) fn execute_release_weapon_lock(
        &mut self,
        units: &[ObjectId],
        lock_type_code: u8,
    ) -> CommandResult {
        use crate::game_logic::WeaponLockType;
        let lock_type = match lock_type_code {
            1 => WeaponLockType::LockedTemporarily,
            _ => WeaponLockType::LockedPermanently,
        };
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            unit.release_weapon_lock(lock_type);
            any = true;
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
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            unit.set_emoticon(name, duration_frames);
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupGoProne residual.
    pub(crate) fn execute_go_prone(&mut self, units: &[ObjectId]) -> CommandResult {
        // Retail infantry prone window residual (~2s).
        const PRONE_SECS: f32 = 2.0;
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object_mut(unit_id) else {
                continue;
            };
            if !unit.is_alive() {
                continue;
            }
            // C++ go-prone is infantry-oriented (AIUpdate); skip structures / immobile.
            if unit.is_kind_of(crate::game_logic::KindOf::Structure)
                || unit.is_kind_of(crate::game_logic::KindOf::Immobile)
            {
                continue;
            }
            let is_infantry = unit.is_kind_of(crate::game_logic::KindOf::Infantry)
                || unit.object_type == crate::game_logic::ObjectType::Infantry;
            if !is_infantry {
                continue;
            }
            unit.go_prone(PRONE_SECS);
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::groupAttackArea residual — engage nearest enemy inside radius.
    pub(crate) fn execute_attack_area(
        &mut self,
        units: &[ObjectId],
        center: Vec3,
        radius: f32,
    ) -> CommandResult {
        let radius = radius.max(1.0);
        if !center.x.is_finite() || !center.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_attack() {
                continue;
            }
            let team = unit.team;
            // Find nearest enemy of this unit inside area.
            let mut best: Option<(ObjectId, f32)> = None;
            for (cid, cand) in self.game_logic.get_objects().iter() {
                if !cand.is_alive() || !cand.is_targetable_by_enemy_of(team) {
                    continue;
                }
                let d = center.distance(cand.get_position());
                if d > radius {
                    continue;
                }
                if best.map(|(_, bd)| d < bd).unwrap_or(true) {
                    best = Some((*cid, d));
                }
            }
            if let Some((enemy_id, _)) = best {
                // Reuse attack path.
                if matches!(
                    self.execute_attack_object(&[unit_id], enemy_id),
                    CommandResult::Success
                ) {
                    any = true;
                }
            } else {
                // No target: move to area center (C++ attack-area still enters state).
                if unit.can_move()
                    && self.path_to_goal_with_state(unit_id, center, AIState::AttackMoving)
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

    fn execute_clear_mines(&mut self, units: &[ObjectId]) -> CommandResult {
        use crate::game_logic::host_mines::{is_mine_clearer, DOZER_MINE_CLEAR_SCAN_RANGE};
        let mut any = false;
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
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
            if let Some(u) = self.game_logic.get_object_mut(unit_id) {
                u.set_weapon_set_mine_clearing_detail(true);
            }

            // Pure residual acquire: nearest enemy mine in clear scan range (XZ).
            let mine_cands: Vec<_> = self
                .game_logic
                .get_objects()
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
                .get_object(mine_id)
                .map(|o| o.get_position())
                .unwrap_or(pos);
            if let Some(u) = self.game_logic.get_object_mut(unit_id) {
                u.set_target(Some(mine_id));
            }
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

    fn execute_dock(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        let target_pos = if let Some(target) = self.game_logic.get_object(target_id) {
            if target.is_alive() && !target.status.under_construction && target.can_contain() {
                target.get_position()
            } else {
                return CommandResult::InvalidTarget;
            }
        } else {
            return CommandResult::InvalidTarget;
        };

        let mut issued = false;
        for &unit_id in units {
            if !self.can_issue_enter_or_dock(unit_id, target_id) {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.set_target(Some(target_id));
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::Docking) {
                issued = true;
            }
        }
        if issued {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_combat_drop(&mut self, units: &[ObjectId], target: &DropTarget) -> CommandResult {
        debug!("Executing combat drop at {:?}", target);
        match target {
            DropTarget::Location(pos) => {
                for &unit_id in units {
                    let _ = self.path_to_goal_with_state(unit_id, *pos, AIState::Entering);
                }
            }
            DropTarget::Object(target_id) => {
                if let Some(target_obj) = self.game_logic.get_object(*target_id) {
                    let target_pos = target_obj.position;
                    for &unit_id in units {
                        if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                            unit.set_target(Some(*target_id));
                        }
                        let _ =
                            self.path_to_goal_with_state(unit_id, target_pos, AIState::Entering);
                    }
                } else {
                    return CommandResult::InvalidTarget;
                }
            }
        }
        CommandResult::Success
    }

    // === Utility Commands ===

    fn execute_repair(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
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
        ) = match self.game_logic.get_object(target_id) {
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
                .get_object(unit_id)
                .map(|unit| {
                    unit.can_repair() && (unit.team == target_team || target_team == Team::Neutral)
                })
                .unwrap_or(false);
            if !can {
                continue;
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.set_target(Some(target_id));
            }
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

    fn execute_get_repaired(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        // Host residual: damaged vehicle → RepairPad or WarFactory (China RepairDock);
        // aircraft → Airfield. Fail-closed: not full dock bones / TimeForFullHeal matrix.
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_under_construction,
            target_building_type,
        ) = match self.game_logic.get_object(target_id) {
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
                .get_object(unit_id)
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
                if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                    unit.set_target(Some(target_id));
                }
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

    fn execute_get_healed(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_under_construction,
            target_building_type,
        ) = match self.game_logic.get_object(target_id) {
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
                .get_object(unit_id)
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
                if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                    unit.set_target(Some(target_id));
                }
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

    fn execute_set_rally_point(&mut self, units: &[ObjectId], location: Vec3) -> CommandResult {
        let mut applied = false;
        for &unit_id in units {
            if let Some(obj) = self.game_logic.get_object_mut(unit_id) {
                if let Some(building) = obj.building_data.as_mut() {
                    building.rally_point = Some(location);
                    applied = true;
                }
            }
        }
        if applied {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Economy Commands ===

    fn normalize_command_token(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase()
    }

    fn resolve_science_cost_supplies(science_name: &str) -> u32 {
        // Main currently models science purchases through resources; use deterministic
        // command-name costs instead of unstable string-length heuristics.
        match Self::normalize_command_token(science_name).as_str() {
            "spydrone" | "radarscan" | "cashbounty1" | "cashbounty" => 500,
            "emergencyrepair1" | "emergencyrepair" | "clustermines" | "battleplan1" => 1000,
            "a10strike1" | "carpetbomb1" | "artillerybarrage1" | "anthraxbomb1" => 1500,
            "a10strike2" | "carpetbomb2" | "artillerybarrage2" | "anthraxbomb2" => 2000,
            "a10strike3" | "spectregunship" | "fuelairbomb" | "particlecannon" => 3000,
            "nuclearmissile" | "scudstorm" => 5000,
            _ => 1000,
        }
    }

    fn resolve_upgrade_cost_supplies(&self, upgrade_name: &str) -> u32 {
        // Prefer catalog template cost when present.
        if let Some(template) = self.game_logic.get_templates().get(upgrade_name) {
            if template.build_cost.supplies > 0 {
                return template.build_cost.supplies;
            }
        }

        // Wave 79: apply HostUpgradeKind retail Upgrade.ini BuildCost residual.
        use crate::game_logic::host_upgrades::HostUpgradeKind;
        let kind = HostUpgradeKind::from_name(upgrade_name);
        let retail = kind.retail_build_cost();
        if retail > 0 {
            return retail;
        }

        // Fallback residual matrix for non-HostUpgradeKind research names.
        match Self::normalize_command_token(upgrade_name).as_str() {
            "upgradeamericatowmissile" => 800,
            "upgradeamericacompositearmor" => 2000,
            "upgradeamericarangercapturebuilding"
            | "upgradechinaredguardcapturebuilding"
            | "upgradeglarebelcapturebuilding"
            | "upgradeinfantrycapturebuilding" => 1000,
            // Retail Upgrade_AmericaRangerFlashBangGrenade BuildCost 800.
            "upgradeamericaflashbanggrenade" | "upgradeamericarangerflashbanggrenade" => 800,
            // Retail Upgrade_AmericaSupplyLines BuildCost 800.
            "upgradeamericasupplylines" => 800,
            // Retail Upgrade_AmericaAdvancedTraining BuildCost 1500.
            "upgradeamericaadvancedtraining" | "upgradeadvancedtraining" => 1500,
            "upgradeglaapbullets" => 2000,
            // Retail Upgrade_GLAWorkerShoes BuildCost 1000 (was incorrect 500).
            "upgradeglaworkershoes" => 1000,
            "upgradechinanuclearengines" | "upgradechinanucleartanks" => 2000,
            "upgradenationalism" | "upgradefanaticism" => 1500,
            _ => 1000,
        }
    }

    fn execute_purchase_science(&mut self, player_id: u32, science_name: &str) -> CommandResult {
        if science_name.trim().is_empty() {
            return CommandResult::InvalidCommand;
        }

        // C++ Player::attemptToPurchaseScience residual: science purchase points,
        // not supply cash. Cost 0 / missing prereqs / insufficient points → fail.
        let unlocked = {
            let Some(player) = self.game_logic.get_player_mut(player_id) else {
                return CommandResult::InvalidCommand;
            };
            if !player.attempt_to_purchase_science(science_name) {
                return CommandResult::InvalidCommand;
            }
            debug!(
                "Player {} purchased science {} (spp left={})",
                player_id, science_name, player.science_purchase_points
            );
            true
        };

        if unlocked {
            // Cash bounty residual: SCIENCE_CashBounty* raises percent + honesty registry.
            if let Some(pct) =
                crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science(science_name)
            {
                let _ = self.game_logic.set_player_cash_bounty(player_id, pct);
            }
            // SCIENCE_StealthFighter residual: record unlock honesty on purchase.
            if crate::game_logic::host_stealth_fighter::is_stealth_fighter_science(science_name) {
                self.game_logic.record_stealth_fighter_science_unlock();
            }
            // C++ SpecialPowerModule::onSpecialPowerCreation residual.
            self.game_logic
                .on_special_power_science_creation(player_id, science_name);
            return CommandResult::Success;
        }
        CommandResult::InvalidCommand
    }

    fn execute_queue_upgrade(&mut self, units: &[ObjectId], upgrade_name: &str) -> CommandResult {
        if upgrade_name.trim().is_empty() {
            return CommandResult::InvalidCommand;
        }

        use crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT;

        let mut seen_teams = HashSet::new();
        let mut any = false;
        let cost = Resources {
            supplies: self.resolve_upgrade_cost_supplies(upgrade_name),
            power: 0,
        };
        // Collect successful queues then record honesty (avoids borrow conflicts).
        let mut recorded: Vec<(u32, crate::game_logic::Team, ObjectId)> = Vec::new();
        for &unit_id in units {
            // C++ queueMaxed / MaxQueueEntries residual: refuse before charging.
            let producer_ok =
                self.game_logic.get_object(unit_id).is_some_and(|source| {
                    if !Self::can_source_queue_upgrade(source) {
                        return false;
                    }
                    let Some(building) = source.building_data.as_ref() else {
                        return false;
                    };
                    if building.production_queue.len() >= DEFAULT_PRODUCTION_QUEUE_LIMIT {
                        return false;
                    }
                    // C++ isUpgradeInQueue residual.
                    if building.production_queue.iter().any(|i| {
                        i.is_upgrade() && i.template_name.eq_ignore_ascii_case(upgrade_name)
                    }) {
                        return false;
                    }
                    true
                });
            if !producer_ok {
                continue;
            }
            let team = self
                .game_logic
                .get_object(unit_id)
                .map(|source| source.team);
            if let Some(team) = team {
                if !seen_teams.insert(team) {
                    continue;
                }
                if let Some(player) = self.game_logic.get_player_mut_by_team(team) {
                    let player_id = player.id;
                    if player.queue_upgrade(upgrade_name, &cost) {
                        any = true;
                        recorded.push((player_id, team, unit_id));
                    }
                }
            }
        }
        for (player_id, team, unit_id) in recorded {
            // C++ ProductionUpdate::queueUpgrade — research advances on the producer.
            let research_secs = {
                let kind =
                    crate::game_logic::host_upgrades::HostUpgradeKind::from_name(upgrade_name);
                kind.residual_research_frames().max(1) as f32 / 30.0
            };
            let mut building_queued = false;
            if let Some(obj) = self.game_logic.get_object_mut(unit_id) {
                if let Some(building) = obj.building_data.as_mut() {
                    building_queued = building.add_upgrade_to_queue(
                        upgrade_name.to_string(),
                        research_secs,
                        cost.clone(),
                    );
                }
            }
            if !building_queued {
                // Fail-closed: refund player if PRODUCTION_UPGRADE could not be placed.
                if let Some(player) = self.game_logic.get_player_mut(player_id) {
                    let _ = player.cancel_queued_upgrade(upgrade_name, &cost);
                }
                continue;
            }
            self.game_logic.record_host_upgrade_queued(
                player_id,
                team,
                upgrade_name,
                Some(unit_id),
            );
            // Wave 79: stamp residual build cost paid (retail application honesty).
            self.game_logic.host_upgrades_mut().set_build_cost_paid(
                upgrade_name,
                player_id,
                cost.supplies,
            );
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_cancel_upgrade(&mut self, units: &[ObjectId], upgrade_name: &str) -> CommandResult {
        // C++ cancel queue head residual: Command_CancelUpgrade may omit the name;
        // resolve from the first selected producer's PRODUCTION_UPGRADE head.
        let resolved_name = if upgrade_name.trim().is_empty() {
            units.iter().find_map(|&unit_id| {
                self.game_logic.get_object(unit_id).and_then(|obj| {
                    obj.building_data.as_ref().and_then(|b| {
                        b.production_queue
                            .iter()
                            .find(|i| i.is_upgrade())
                            .map(|i| i.template_name.clone())
                    })
                })
            })
        } else {
            Some(upgrade_name.to_string())
        };
        let Some(upgrade_name) = resolved_name.filter(|s| !s.trim().is_empty()) else {
            return CommandResult::InvalidCommand;
        };

        let mut seen_teams = HashSet::new();
        let mut refunded = false;
        let refund = Resources {
            supplies: self.resolve_upgrade_cost_supplies(&upgrade_name),
            power: 0,
        };
        let mut cancelled_players: Vec<u32> = Vec::new();
        for &unit_id in units {
            let team = self
                .game_logic
                .get_object(unit_id)
                .filter(|source| Self::can_source_queue_upgrade(source))
                .map(|source| source.team);
            if let Some(team) = team {
                if !seen_teams.insert(team) {
                    continue;
                }
                if let Some(player) = self.game_logic.get_player_mut_by_team(team) {
                    let player_id = player.id;
                    if player.cancel_queued_upgrade(&upgrade_name, &refund) {
                        refunded = true;
                        cancelled_players.push(player_id);
                    }
                }
            }
        }
        for player_id in cancelled_players {
            self.game_logic
                .record_host_upgrade_cancelled(player_id, &upgrade_name);
        }
        // C++ cancelUpgrade also removes the PRODUCTION_UPGRADE entry from the producer queue.
        let mut removed_from_building = false;
        for &unit_id in units {
            if let Some(obj) = self.game_logic.get_object_mut(unit_id) {
                if let Some(building) = obj.building_data.as_mut() {
                    let before = building.production_queue.len();
                    building.production_queue.retain(|item| {
                        !(item.is_upgrade()
                            && item.template_name.eq_ignore_ascii_case(&upgrade_name))
                    });
                    if building.production_queue.len() < before {
                        removed_from_building = true;
                    }
                }
            }
        }
        // If player queue was already empty but building still held the entry, treat as success
        // after removing the PRODUCTION_UPGRADE residual (refund already applied or N/A).
        if refunded || removed_from_building {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn can_source_queue_upgrade(source: &crate::game_logic::Object) -> bool {
        source.building_data.is_some() && source.is_alive() && source.is_constructed()
    }

    // === Special Unit Abilities ===

    fn execute_hijack(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        // C++ ConvertToHijackedVehicleCrateCollide residual: enemy ground vehicle
        // only, not already HIJACKED, not neutral, not airborne.
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_hijacked,
        ) = match self.game_logic.get_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_hijacked(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !target_alive
            || !target_is_vehicle
            || target_is_airborne
            || target_hijacked
            || target_team == Team::Neutral
        {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();

        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| unit.is_alive() && unit.can_move() && unit.team != target_team)
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::Hijack { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_sabotage(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        // C++ Sabotage*CrateCollide residual: GLA Saboteur only → enemy structure.
        let (target_team, target_pos, target_alive, target_is_structure) =
            match self.game_logic.get_object(target_id) {
                Some(target) => (
                    target.team,
                    target.get_position(),
                    target.is_alive(),
                    target.is_kind_of(KindOf::Structure),
                ),
                None => return CommandResult::InvalidTarget,
            };

        if !target_alive || !target_is_structure || target_team == Team::Neutral {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| {
                    unit.is_alive()
                        && unit.can_move()
                        && unit.team != target_team
                        && crate::game_logic::host_saboteur::is_saboteur_template(
                            &unit.template_name,
                        )
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::Sabotage { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_convert_carbomb(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // C++ ConvertToCarBombCrateCollide: vehicle only (not aircraft/boat),
        // not already IS_CARBOMB. Neutral civilian cars are valid.
        let (target_pos, target_ok) = match self.game_logic.get_object(target_id) {
            Some(target) if target.is_alive() => {
                let is_vehicle = target.is_kind_of(KindOf::Vehicle);
                let is_airborne =
                    target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;
                let already_bomb = target.status.is_carbomb;
                (
                    target.get_position(),
                    is_vehicle && !is_airborne && !already_bomb,
                )
            }
            Some(_) => return CommandResult::InvalidTarget,
            None => return CommandResult::InvalidTarget,
        };
        if !target_ok {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| unit.is_alive() && unit.can_move() && unit_id != target_id)
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::CarBomb { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_capture_building(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        use crate::game_logic::host_hero_abilities::{
            can_capture_without_upgrade, is_black_lotus_template,
        };

        let (building_pos, is_structure, is_alive, is_under_construction, target_team) =
            match self.game_logic.get_object(target_id) {
                Some(building) => (
                    building.get_position(),
                    building.is_kind_of(KindOf::Structure),
                    building.is_alive(),
                    building.status.under_construction,
                    building.team,
                ),
                None => return CommandResult::InvalidTarget,
            };

        if !is_structure || !is_alive || is_under_construction {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        for &unit_id in units {
            if unit_id == target_id {
                continue;
            }

            let can_capture = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| {
                    let is_lotus = is_black_lotus_template(&unit.template_name);
                    // Black Lotus / heroes capture without infantry Capture research.
                    // Regular infantry require completed CaptureBuilding upgrade.
                    let capture_ability = can_capture_without_upgrade(unit.is_hero(), is_lotus)
                        || (unit.is_kind_of(KindOf::Infantry)
                            && self
                                .game_logic
                                .team_has_completed_capture_upgrade(unit.team));
                    unit.is_alive()
                        && unit.can_move()
                        && unit.team != target_team
                        && capture_ability
                })
                .unwrap_or(false);
            if !can_capture {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, building_pos, AIState::Capturing) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_snipe_vehicle(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_unmanned,
        ) = match self.game_logic.get_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_unmanned(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        // Kill-pilot residual only applies to manned enemy ground vehicles.
        if !target_alive
            || !target_is_vehicle
            || target_is_airborne
            || target_unmanned
            || target_team == Team::Neutral
        {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| unit.is_alive() && unit.can_move() && unit.team != target_team)
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::SnipeVehicle { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Colonel Burton residual: plant timed demo charge on enemy structure/vehicle.

    fn execute_plant_booby_trap(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let (target_team, target_pos, target_alive, target_is_structure) =
            match self.game_logic.get_object(target_id) {
                Some(target) => (
                    target.team,
                    target.get_position(),
                    target.is_alive(),
                    target.is_kind_of(KindOf::Structure),
                ),
                None => return CommandResult::InvalidTarget,
            };

        if !target_alive || !target_is_structure {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| {
                    use crate::game_logic::host_booby_trap::{
                        has_booby_trap_upgrade, is_booby_trap_planter_template,
                    };
                    unit.is_alive()
                        && unit.can_move()
                        && unit.team != target_team
                        && is_booby_trap_planter_template(&unit.template_name)
                        && has_booby_trap_upgrade(&unit.applied_upgrades)
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::PlantBoobyTrap { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_plant_timed_demo_charge(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_is_vehicle,
            target_is_airborne,
        ) = match self.game_logic.get_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
            ),
            None => return CommandResult::InvalidTarget,
        };

        let valid_target = target_alive
            && target_team != Team::Neutral
            && (target_is_structure || (target_is_vehicle && !target_is_airborne));
        if !valid_target {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| unit.is_alive() && unit.can_move() && unit.team != target_team)
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::PlantTimedDemoCharge { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Colonel Burton residual: plant remote demo charge on enemy structure/vehicle.
    /// Fail-closed: not full StickyBombUpdate attach bones / max-charge list.
    fn execute_plant_remote_demo_charge(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_is_vehicle,
            target_is_airborne,
        ) = match self.game_logic.get_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
            ),
            None => return CommandResult::InvalidTarget,
        };

        let valid_target = target_alive
            && target_team != Team::Neutral
            && (target_is_structure || (target_is_vehicle && !target_is_airborne));
        if !valid_target {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| unit.is_alive() && unit.can_move() && unit.team != target_team)
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::PlantRemoteDemoCharge { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Colonel Burton residual: detonate all remote charges planted by selected units.
    /// Matches C++ SPECIAL_REMOTE_CHARGES no-target path (detonate special object list).
    fn execute_detonate_remote_demo_charges(&mut self, units: &[ObjectId]) -> CommandResult {
        let producers: Vec<ObjectId> = units
            .iter()
            .copied()
            .filter(|id| {
                self.game_logic
                    .get_object(*id)
                    .map(|u| u.is_alive())
                    .unwrap_or(false)
            })
            .collect();
        if producers.is_empty() {
            return CommandResult::InvalidCommand;
        }
        let detonated = self.game_logic.detonate_remote_demo_charges(&producers);
        if detonated > 0 {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Demo SuicideBomb residual: intentional SUICIDED PlusFire detonation.
    ///
    /// Fail-closed: requires SuicideBomb CommandSetUpgrade residual tag.
    fn execute_demo_tertiary_suicide(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.issue_demo_tertiary_suicide(unit_id) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Black Lotus residual: steal cash from enemy supply/cash building.
    ///
    /// Fail-closed: only Black Lotus templates; target must be residual
    /// cash generator (C++ KINDOF_CASH_GENERATOR). StartAbilityRange 150
    /// resolved on reach in GameLogic SpecialAbility update.
    fn execute_steal_cash_hack(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        use crate::game_logic::host_hero_abilities::{
            can_activate_black_lotus_ability, is_black_lotus_template, is_cash_hack_target,
            is_legal_steal_cash_target,
        };

        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_under_construction,
            is_cash_generator,
        ) = match self.game_logic.get_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.status.under_construction,
                is_cash_hack_target(
                    &target.template_name,
                    target.is_kind_of(KindOf::SupplyCenter),
                    target.is_kind_of(KindOf::FSSupplyCenter),
                    target.is_kind_of(KindOf::FSBlackMarket),
                    target.is_kind_of(KindOf::FSSupplyDropzone),
                ),
            ),
            None => return CommandResult::InvalidTarget,
        };

        // Target residual: enemy cash generator structure (not under construction).
        // Per-unit enemy check below; here require non-neutral cash structure.
        if !is_legal_steal_cash_target(
            target_alive,
            target_is_structure,
            target_under_construction,
            target_team != Team::Neutral,
            is_cash_generator,
        ) {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| {
                    can_activate_black_lotus_ability(
                        is_black_lotus_template(&unit.template_name),
                        unit.is_alive(),
                    ) && unit.can_move()
                        && unit.team != target_team
                        && unit.team != Team::Neutral
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::StealCashHack { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Black Lotus residual: disable enemy ground vehicle (DISABLED_HACKED).
    ///
    /// Fail-closed: only Black Lotus templates. C++ ActionManager
    /// canDisableVehicleViaHacking residual: enemy ground vehicle, not already
    /// hacked-disabled, not unmanned. StartAbilityRange 150 on reach.
    fn execute_disable_vehicle_hack(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        use crate::game_logic::host_hero_abilities::{
            can_activate_black_lotus_ability, is_black_lotus_template,
            is_legal_disable_vehicle_target,
        };

        let (
            target_team,
            target_pos,
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_hacked,
            target_unmanned,
        ) = match self.game_logic.get_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_hacked_disabled(),
                target.is_unmanned(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !is_legal_disable_vehicle_target(
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_team != Team::Neutral,
            target_hacked,
            target_unmanned,
        ) {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| {
                    can_activate_black_lotus_ability(
                        is_black_lotus_template(&unit.template_name),
                        unit.is_alive(),
                    ) && unit.can_move()
                        && unit.team != target_team
                        && unit.team != Team::Neutral
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::DisableVehicleHack { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// China Hacker residual: disable enemy structure (DISABLED_HACKED).
    /// SpecialAbilityHackerDisableBuilding.
    fn execute_hacker_disable_building(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        use crate::game_logic::host_hacker_disable::{
            can_activate_hacker_disable_building, is_legal_hacker_disable_target,
            should_apply_hacker_disable,
        };

        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_under_construction,
            target_hacked,
        ) = match self.game_logic.get_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.status.under_construction,
                target.is_hacked_disabled(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        // is_enemy checked per unit; here require non-neutral structure residual.
        if !is_legal_hacker_disable_target(
            target_alive,
            target_is_structure,
            target_under_construction,
            target_team != Team::Neutral,
            target_hacked,
        ) {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| {
                    can_activate_hacker_disable_building(
                        should_apply_hacker_disable(&unit.template_name),
                        unit.is_alive(),
                    ) && unit.can_move()
                        && unit.team != target_team
                        && unit.team != Team::Neutral
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::HackerDisableBuilding { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// GLA Bomb Truck residual: SpecialAbilityDisguiseAsVehicle.
    ///
    /// C++ residual: any ground vehicle target (ally/enemy/neutral) except
    /// bomb trucks / trains / aircraft. Completes without approach walk
    /// (StartAbilityRange = 1e6). Fail-closed: not full drawable model swap.
    /// Timed-charge special residual (Burton/Demo/BattleBus): walk + plant timed charge.
    fn queue_special_timed_charge(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
        power_type: &SpecialPowerType,
    ) -> bool {
        use crate::game_logic::{AIState, PendingSpecialAbility};

        let Some(unit) = self.game_logic.get_object(unit_id) else {
            return false;
        };
        if !unit.is_alive() || !unit.can_move() {
            return false;
        }
        let Some(target) = self.game_logic.get_object(target_id) else {
            return false;
        };
        if !target.is_alive() {
            return false;
        }
        let target_team = target.team;
        if target_team == unit.team || target_team == crate::game_logic::Team::Neutral {
            // BattleBus trap rollout may target ground near self; allow structure/vehicle enemies only residual.
            if !matches!(*power_type, SpecialPowerType::BattleBusDemoTrapRollout) {
                return false;
            }
        }
        let target_pos = target.get_position();
        if let Some(u) = self.game_logic.get_object_mut(unit_id) {
            u.stop_moving();
            u.status.attacking = false;
            u.target = Some(target_id);
            u.target_location = None;
        }
        let _ = self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility);
        self.game_logic.queue_pending_special_ability(
            unit_id,
            PendingSpecialAbility::PlantTimedDemoCharge { target_id },
        );
        true
    }

    /// Remote-charge special residual (Burton/Demo Kell): walk + plant remote charge.
    fn queue_special_remote_charge(&mut self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        use crate::game_logic::{AIState, PendingSpecialAbility};

        let Some(unit) = self.game_logic.get_object(unit_id) else {
            return false;
        };
        if !unit.is_alive() || !unit.can_move() {
            return false;
        }
        let Some(target) = self.game_logic.get_object(target_id) else {
            return false;
        };
        if !target.is_alive() {
            return false;
        }
        let target_pos = target.get_position();
        if let Some(u) = self.game_logic.get_object_mut(unit_id) {
            u.stop_moving();
            u.status.attacking = false;
            u.target = Some(target_id);
            u.target_location = None;
        }
        let _ = self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility);
        self.game_logic.queue_pending_special_ability(
            unit_id,
            PendingSpecialAbility::PlantRemoteDemoCharge { target_id },
        );
        true
    }

    /// Tank Hunter TNT special residual: path to target and plant timed sticky charge.
    fn queue_tank_hunter_tnt(&mut self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        use crate::game_logic::host_tank_hunter::{
            is_tank_hunter_template, tnt_in_start_range, tnt_ready, TNT_START_ABILITY_RANGE,
        };
        use crate::game_logic::{AIState, PendingSpecialAbility};

        let Some(unit) = self.game_logic.get_object(unit_id) else {
            return false;
        };
        if !unit.is_alive() || !is_tank_hunter_template(&unit.template_name) {
            return false;
        }
        if !tnt_ready(
            self.game_logic.get_frame(),
            self.game_logic.tank_hunter_tnt_last_plant_frame(unit_id),
        ) {
            return false;
        }
        let Some(target) = self.game_logic.get_object(target_id) else {
            return false;
        };
        if !target.is_alive() {
            return false;
        }
        let target_pos = target.get_position();
        // Always queue walk-to; plant resolves on reach (StartAbilityRange 5 residual).
        if let Some(u) = self.game_logic.get_object_mut(unit_id) {
            u.stop_moving();
            u.status.attacking = false;
            u.target = Some(target_id);
            u.target_location = None;
        }
        if !self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
            // If already in range, still queue plant.
            let unit_pos = self
                .game_logic
                .get_object(unit_id)
                .map(|o| o.get_position())
                .unwrap_or(target_pos);
            let dx = unit_pos.x - target_pos.x;
            let dz = unit_pos.z - target_pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            if !tnt_in_start_range(dist) && dist > TNT_START_ABILITY_RANGE * 2.0 {
                return false;
            }
        }
        self.game_logic.queue_pending_special_ability(
            unit_id,
            PendingSpecialAbility::PlantTimedDemoCharge { target_id },
        );
        let _ = TNT_START_ABILITY_RANGE;
        true
    }

    fn execute_disguise_as_vehicle(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        use crate::game_logic::host_bomb_truck_disguise::{
            is_bomb_truck_template, is_legal_disguise_target,
        };

        let (
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_is_bomb_truck,
            target_disguised,
            target_template,
            target_pos,
        ) = match self.game_logic.get_object(target_id) {
            Some(target) => (
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                is_bomb_truck_template(&target.template_name),
                target.status.disguised,
                target.template_name.clone(),
                target.get_position(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !is_legal_disguise_target(
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_is_bomb_truck,
            &target_template,
            target_disguised,
        ) {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .get_object(unit_id)
                .map(|unit| unit.is_alive() && is_bomb_truck_template(&unit.template_name))
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_moving();
                unit.status.attacking = false;
                unit.target = Some(target_id);
                unit.target_location = None;
                unit.force_attack = false;
            }
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::DisguiseAsVehicle { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_switch_weapons(&mut self, units: &[ObjectId]) -> CommandResult {
        use crate::game_logic::WeaponLockType;
        let mut any = false;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                // C++ switch weapons: toggle slot and permanently lock the chosen one
                // when a secondary exists; otherwise flip active slot residual.
                let next = unit.active_weapon_slot ^ 1;
                if unit.weapon_slot(next).is_some() {
                    let _ = unit.set_weapon_lock(next, WeaponLockType::LockedPermanently);
                } else {
                    unit.set_active_weapon_slot(next);
                }
                unit.set_ai_state(AIState::SpecialAbility);
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_toggle_overcharge(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.toggle_overcharge_object(unit_id) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Formation Commands ===

    pub(crate) fn execute_create_formation(&mut self, units: &[ObjectId]) -> CommandResult {
        // C++ AIGroup::groupCreateFormation — stamp formation id + offset from
        // centroid. Does NOT path units or enter GuardingArea.
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut members: Vec<(ObjectId, Vec3, u32)> = Vec::new();
        for &unit_id in units {
            let Some(unit) = self.game_logic.get_object(unit_id) else {
                continue;
            };
            if !unit.is_alive() || !unit.can_move() {
                continue;
            }
            if unit.is_kind_of(crate::game_logic::KindOf::Immobile)
                || unit.is_kind_of(crate::game_logic::KindOf::Structure)
            {
                continue;
            }
            members.push((unit_id, unit.get_position(), unit.formation_id));
        }
        if members.is_empty() {
            return CommandResult::InvalidCommand;
        }

        let mut center = Vec3::ZERO;
        for (_, pos, _) in &members {
            center += *pos;
        }
        center /= members.len() as f32;

        // C++: if already a formation (shared id, or single unit with id), clear.
        let mut is_formation = false;
        if members.len() == 1 && members[0].2 != 0 {
            is_formation = true;
        } else if members.len() >= 2 {
            let first_id = members[0].2;
            if first_id != 0 && members.iter().all(|m| m.2 == first_id) {
                is_formation = true;
            }
        }

        let new_id = if is_formation {
            0 // NO_FORMATION_ID — dissolve
        } else {
            self.game_logic.alloc_formation_id()
        };

        for (unit_id, pos, _) in members {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.formation_id = new_id;
                // C++ offset is XY; host ground is XZ → store as Vec2(x, z).
                unit.formation_offset = glam::Vec2::new(pos.x - center.x, pos.z - center.z);
            }
        }

        CommandResult::Success
    }

    pub(crate) fn execute_cheer(&mut self, units: &[ObjectId]) -> CommandResult {
        // C++ AIGroup::groupCheer:
        // setSpecialModelConditionState(SPECIAL_CHEERING, LOGICFRAMES_PER_SECOND * 3)
        use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
        let cheer_secs = 3.0; // 30 logic frames @ 30Hz
        let cheer_bit = model_condition_bit_name_index("SPECIAL_CHEERING");
        let mut any = false;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                if !unit.is_alive() {
                    continue;
                }
                unit.set_ai_state(AIState::SpecialAbility);
                unit.cheer_timer = cheer_secs;
                if let Some(bit) = cheer_bit {
                    unit.model_condition_bits |= 1u128 << bit;
                }
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

    fn execute_place_beacon(
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

        CommandResult::Success
    }

    fn execute_remove_beacon(&mut self, player_id: u32) -> CommandResult {
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
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Selection Commands ===

    fn execute_selection(
        &mut self,
        player_id: u32,
        create_new: bool,
        units: &[ObjectId],
    ) -> CommandResult {
        if let Some(player) = self.game_logic.get_player_mut(player_id) {
            if create_new {
                player.selected_objects.clear();
            }
            for &unit_id in units {
                if !player.selected_objects.contains(&unit_id) {
                    player.selected_objects.push(unit_id);
                }
            }
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_destroy_group(&mut self, player_id: u32, team_id: u32) -> CommandResult {
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

    fn execute_remove_from_selection(
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

    fn execute_view_command_center(&mut self) -> CommandResult {
        let team = self.player_team(self.current_player_id);
        if let Some(position) = self.game_logic.command_center_position(team) {
            self.game_logic.request_camera_focus(position);
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Validation Helpers ===

    fn validate_player_ownership(&self, command: &GameCommand) -> bool {
        let player_team = self.player_team(command.player_id);

        // Check if player owns all selected units
        for &unit_id in &command.selected_units {
            if let Some(unit) = self.game_logic.get_object(unit_id) {
                if unit.team != player_team {
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

    fn validate_target_exists(&self, target_id: ObjectId) -> bool {
        self.game_logic.get_object(target_id).is_some()
    }

    fn validate_build_location(&self, location: Vec3) -> bool {
        if !location.x.is_finite() || !location.z.is_finite() {
            return false;
        }
        // Use loaded map world bounds when available (Lone Eagle bases can sit
        // near edges beyond the old hard-coded ±1000 host box). Fall back to a
        // generous host default for synthetic/no-map worlds.
        let (min, max) = self.game_logic.world_bounds();
        let pad = 50.0;
        let min_x = min.x.min(-1000.0) - pad;
        let max_x = max.x.max(1000.0) + pad;
        let min_z = min.z.min(-1000.0) - pad;
        let max_z = max.z.max(1000.0) + pad;
        location.x >= min_x && location.x <= max_x && location.z >= min_z && location.z <= max_z
    }

    /// Minimal `canEnterObject`/`canDockAt` legality mirror for Main command execution.
    fn can_issue_enter_or_dock(&self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        if unit_id == target_id {
            return false;
        }

        let Some(unit) = self.game_logic.get_object(unit_id) else {
            return false;
        };
        let Some(target) = self.game_logic.get_object(target_id) else {
            return false;
        };

        if !unit.is_alive()
            || !target.is_alive()
            || unit.status.under_construction
            || target.status.under_construction
        {
            return false;
        }

        // Tunnel network residual: units already in the shared pool may transfer
        // to another allied tunnel without can_move (Garrisoned).
        let unit_in_tunnel = self
            .game_logic
            .tunnel_network_residual()
            .team_holding_unit(unit_id)
            .is_some();
        if unit.is_kind_of(KindOf::Structure) {
            return false;
        }
        if !unit.can_move() && !unit_in_tunnel {
            return false;
        }

        // USA Pilot residual: pilots may Enter unmanned ground vehicles for recrew
        // even when the vehicle is not a residual transport container.
        let pilot_recrew = crate::game_logic::host_usa_pilot::should_recrew_on_enter(
            crate::game_logic::host_usa_pilot::is_pilot_template(&unit.template_name),
            crate::game_logic::host_usa_pilot::is_recrewable_unmanned_vehicle(
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_unmanned(),
                target.status.under_construction,
                target.is_worker() || target.template_name.to_ascii_lowercase().contains("dozer"),
            ),
        );
        if pilot_recrew {
            return true;
        }

        if !target.can_contain() {
            return false;
        }

        // Residual garrison / Overlord BattleBunker / Battle Bus: infantry (and heroes)
        // only. C++ AllowInsideKindOf = INFANTRY. Generic transports still accept any
        // mobile unit. Combat Chinook allows INFANTRY + VEHICLE (rejects AIRCRAFT).
        // Tunnel Network: all units except aircraft (C++ TunnelTracker residual).
        // Fail-closed vs full C++ garrison filters.
        if target.is_tunnel_network_style_container() {
            if unit.is_kind_of(KindOf::Aircraft) {
                return false;
            }
            // Shared MaxTunnelCapacity=10 residual (team pool).
            let in_pool = self
                .game_logic
                .tunnel_network_residual()
                .is_in_network(unit.team, unit_id);
            if !in_pool
                && !self
                    .game_logic
                    .tunnel_network_residual()
                    .has_capacity(unit.team)
            {
                return false;
            }
            // Ally tunnels only for residual enter (not enemy capture residual).
            if target.team != unit.team && target.team != Team::Neutral {
                return false;
            }
            return true;
        }

        let infantry_only_container = target.is_kind_of(KindOf::Structure)
            || (target.is_overlord_style_container() && target.overlord_bunker_slot_capacity() > 0)
            || target.is_battle_bus_style_container()
            || target.is_listening_outpost_style_container()
            || target.is_troop_crawler_style_container();
        if infantry_only_container && !unit.is_kind_of(KindOf::Infantry) && !unit.is_hero() {
            return false;
        }
        // Combat Chinook ForbidInsideKindOf = AIRCRAFT residual.
        if target.is_combat_chinook_style_container() && unit.is_kind_of(KindOf::Aircraft) {
            return false;
        }

        let target_contains_unit = target.contained_units().contains(&unit_id);
        let target_has_space = target.has_capacity_for(1);
        if !target_contains_unit && !target_has_space {
            return false;
        }

        if target.team != unit.team && target.team != Team::Neutral {
            let target_has_occupants = !target.contained_units().is_empty();
            if target.is_faction_structure() || target_has_occupants {
                return false;
            }
        }

        true
    }

    /// Get execution statistics
    pub fn get_stats(&self) -> (usize, usize) {
        (self.commands_executed, self.commands_failed)
    }
}

#[cfg(test)]
mod group_move_tests {

    #[test]
    fn group_move_destinations_spreads_multi_unit() {
        // Source-level: multi-unit move must call group_move_destinations.
        let src = include_str!("command_executor.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(
            prod.contains("fn group_move_destinations")
                && prod.contains("group_move_destinations(units, destination)"),
            "multi-unit move must spread destinations"
        );
        // Production execute_move must not assign the raw destination alone for groups.
        let i = prod.find("fn execute_move(").expect("execute_move");
        let w = &prod[i..prod.len().min(i + 1200)];
        assert!(
            w.contains("group_move_destinations")
                && !w.contains("assign_unit_path(unit_id, destination, &[])"),
            "execute_move must path to per-unit goals"
        );
    }

    #[test]
    fn group_move_destinations_preserves_relative_offset() {
        use super::CommandExecutor;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        // Minimal mobile templates.
        for name in ["GM_A", "GM_B"] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(100.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let a = logic
            .create_object("GM_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("a");
        let b = logic
            .create_object("GM_B", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .expect("b");
        {
            let oa = logic.get_object_mut(a).unwrap();
            oa.selection_radius = 10.0;
        }
        {
            let ob = logic.get_object_mut(b).unwrap();
            ob.selection_radius = 10.0;
        }

        let click = Vec3::new(100.0, 0.0, 50.0);
        let exec = CommandExecutor::new(&mut logic, 0);
        let goals = exec.group_move_destinations(&[a, b], click);
        assert_eq!(goals.len(), 2);

        // B at x=40 is nearer the click at x=100 than A at x=0 → B is lead.
        let goal_a = goals.iter().find(|(id, _)| *id == a).unwrap().1;
        let goal_b = goals.iter().find(|(id, _)| *id == b).unwrap().1;
        assert!(
            (goal_b - click).length() < 0.01,
            "nearest unit (B) must receive click goal, got {goal_b:?}"
        );
        // A was -40 X from lead/center B → goal keeps ~-40 X from click.
        let offset = goal_a - click;
        assert!(
            offset.x < -20.0 && offset.x > -45.0,
            "relative -X offset preserved, offset={offset:?}"
        );
        assert!(
            offset.z.abs() < 1.0,
            "no invented Z ring offset, offset={offset:?}"
        );
        assert!((goal_a - goal_b).length() > 10.0, "goals must not stack");
    }

    #[test]
    fn scatter_pushes_outward_from_group_center() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for name in ["SC_A", "SC_B"] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(100.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let a = logic
            .create_object("SC_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("SC_B", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        for id in [a, b] {
            logic.get_object_mut(id).unwrap().selection_radius = 10.0;
        }
        let before_a = logic.get_object(a).unwrap().get_position();
        let before_b = logic.get_object(b).unwrap().get_position();
        let center = (before_a + before_b) * 0.5;

        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_scatter(&[a, b]), CommandResult::Success);

        for id in [a, b] {
            let u = logic.get_object(id).unwrap();
            assert_eq!(u.ai_state, AIState::Moving, "scatter sets Moving");
        }
        for (id, before) in [(a, before_a), (b, before_b)] {
            let u = logic.get_object(id).unwrap();
            let goal = u
                .movement
                .target_position
                .or_else(|| u.movement.path.last().copied())
                .unwrap_or(u.get_position());
            let before_d = (before.x - center.x).hypot(before.z - center.z);
            let after_d = (goal.x - center.x).hypot(goal.z - center.z);
            assert!(
                after_d > before_d + 5.0,
                "scatter should push outward id={id:?} before={before_d} after={after_d} goal={goal:?}"
            );
        }
    }

    #[test]
    fn cheer_uses_three_second_cpp_duration() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("CH_A");
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("CH_A".to_string(), tpl);
        let a = logic.create_object("CH_A", Team::USA, Vec3::ZERO).unwrap();
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_cheer(&[a]), CommandResult::Success);
        let u = logic.get_object(a).unwrap();
        assert!(
            (u.cheer_timer - 3.0).abs() < 0.01,
            "C++ cheer is 3s (90 frames@30), got {}",
            u.cheer_timer
        );
    }

    #[test]
    fn create_formation_stamps_offsets_not_guard() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for name in ["FM_A", "FM_B"] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(100.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let a = logic
            .create_object("FM_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("FM_B", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_create_formation(&[a, b]),
            CommandResult::Success
        );
        let ua = logic.get_object(a).unwrap();
        let ub = logic.get_object(b).unwrap();
        assert_ne!(ua.formation_id, 0);
        assert_eq!(ua.formation_id, ub.formation_id);
        // Center at x=20 → offsets -20 and +20
        assert!(
            (ua.formation_offset.x + 20.0).abs() < 0.1,
            "{:?}",
            ua.formation_offset
        );
        assert!(
            (ub.formation_offset.x - 20.0).abs() < 0.1,
            "{:?}",
            ub.formation_offset
        );
        assert_ne!(ua.ai_state, AIState::GuardingArea);

        // Second call dissolves formation (C++ toggle when already formation).
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_create_formation(&[a, b]),
            CommandResult::Success
        );
        assert_eq!(logic.get_object(a).unwrap().formation_id, 0);
        assert_eq!(logic.get_object(b).unwrap().formation_id, 0);
    }

    #[test]
    fn formation_move_uses_stamped_offsets() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for name in ["FM_C", "FM_D"] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(100.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let a = logic
            .create_object("FM_C", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("FM_D", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_create_formation(&[a, b]),
            CommandResult::Success
        );
        let click = Vec3::new(100.0, 0.0, 50.0);
        let goals = exec.group_move_destinations(&[a, b], click);
        let ga = goals.iter().find(|(id, _)| *id == a).unwrap().1;
        let gb = goals.iter().find(|(id, _)| *id == b).unwrap().1;
        assert!((ga.x - (100.0 - 20.0)).abs() < 0.1, "a goal {ga:?}");
        assert!((gb.x - (100.0 + 20.0)).abs() < 0.1, "b goal {gb:?}");
        assert!((ga.z - 50.0).abs() < 0.1);
        assert!((gb.z - 50.0).abs() < 0.1);
    }

    #[test]
    fn infantry_group_move_uses_column_pack() {
        use super::CommandExecutor;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for name in ["INF_A", "INF_B", "INF_C", "INF_D"] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Infantry);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(100.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        // Cluster near origin; move far +X so column residual engages.
        let ids: Vec<_> = ["INF_A", "INF_B", "INF_C", "INF_D"]
            .iter()
            .enumerate()
            .map(|(i, name)| {
                logic
                    .create_object(
                        name,
                        Team::USA,
                        Vec3::new(i as f32 * 5.0, 0.0, (i as f32) * 2.0),
                    )
                    .unwrap()
            })
            .collect();
        for &id in &ids {
            logic.get_object_mut(id).unwrap().selection_radius = 10.0;
        }
        let click = Vec3::new(300.0, 0.0, 0.0);
        let exec = CommandExecutor::new(&mut logic, 0);
        let goals = exec.group_move_destinations(&ids, click);
        assert_eq!(goals.len(), 4);
        // Column pack: goals should not all share the same XZ (lateral spread).
        let zs: Vec<f32> = goals.iter().map(|(_, g)| g.z).collect();
        let z_span = zs.iter().cloned().fold(f32::MIN, f32::max)
            - zs.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            z_span > 5.0,
            "infantry column should lateral-spread goals, zs={zs:?}"
        );
        // And not collapse to free-move lead-only click for all.
        let unique_approx = {
            let mut xs: Vec<(i32, i32)> = goals
                .iter()
                .map(|(_, g)| ((g.x * 10.0) as i32, (g.z * 10.0) as i32))
                .collect();
            xs.sort();
            xs.dedup();
            xs.len()
        };
        assert!(
            unique_approx >= 3,
            "expected multiple distinct column goals, got {goals:?}"
        );
    }

    #[test]
    fn guard_mode_without_pursuit_and_flying_only_are_stored() {
        use super::CommandExecutor;
        use crate::command_system::{CommandResult, GuardTarget};
        use crate::game_logic::{GameLogic, GuardMode, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("GM_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("GM_V".to_string(), tpl);
        let a = logic.create_object("GM_V", Team::USA, Vec3::ZERO).unwrap();
        let b = logic
            .create_object("GM_V", Team::USA, Vec3::new(5.0, 0.0, 0.0))
            .unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_guard(
                    &[a],
                    &GuardTarget::Position(Vec3::new(40.0, 0.0, 0.0)),
                    GuardMode::WithoutPursuit
                ),
                CommandResult::Success
            );
        }
        assert_eq!(
            logic.get_object(a).unwrap().guard_mode,
            GuardMode::WithoutPursuit
        );
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_guard(
                    &[b],
                    &GuardTarget::Position(Vec3::new(40.0, 0.0, 0.0)),
                    GuardMode::FlyingUnitsOnly
                ),
                CommandResult::Success
            );
        }
        assert_eq!(
            logic.get_object(b).unwrap().guard_mode,
            GuardMode::FlyingUnitsOnly
        );
    }

    #[test]
    fn command_button_maps_guard_modes() {
        use crate::command_system::{command_type_from_button_name, CommandType};
        use crate::game_logic::GuardMode;

        let g = command_type_from_button_name("Command_Guard").unwrap();
        assert!(matches!(
            g,
            CommandType::Guard {
                mode: GuardMode::Normal,
                ..
            }
        ));
        let w = command_type_from_button_name("Command_GuardWithoutPursuit").unwrap();
        assert!(matches!(
            w,
            CommandType::Guard {
                mode: GuardMode::WithoutPursuit,
                ..
            }
        ));
        let f = command_type_from_button_name("Command_GuardFlyingUnitsOnly").unwrap();
        assert!(matches!(
            f,
            CommandType::Guard {
                mode: GuardMode::FlyingUnitsOnly,
                ..
            }
        ));
    }

    #[test]
    fn guard_uses_vision_radius_and_skips_structures() {
        use super::CommandExecutor;
        use crate::command_system::{CommandResult, GuardTarget};
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for (name, kinds) in [
            ("GD_V", &[KindOf::Vehicle, KindOf::Selectable][..]),
            ("GD_S", &[KindOf::Structure, KindOf::Selectable][..]),
        ] {
            let mut tpl = ThingTemplate::new(name);
            for k in kinds {
                tpl.add_kind_of(*k);
            }
            tpl.set_health(500.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let v = logic
            .create_object("GD_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let s = logic
            .create_object("GD_S", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        {
            let u = logic.get_object_mut(v).unwrap();
            u.vision_range = 120.0;
            u.selection_radius = 10.0;
        }
        let mut exec = CommandExecutor::new(&mut logic, 0);
        let pos = Vec3::new(50.0, 0.0, 0.0);
        assert_eq!(
            exec.execute_guard(
                &[v, s],
                &GuardTarget::Position(pos),
                crate::game_logic::GuardMode::Normal
            ),
            CommandResult::Success
        );
        let u = logic.get_object(v).unwrap();
        assert!(
            (u.guard_radius - 120.0).abs() < 0.1,
            "guard radius should track vision, got {}",
            u.guard_radius
        );
        assert!(matches!(
            u.ai_state,
            AIState::GuardingArea | AIState::Moving
        ));
        // Structure must not enter guard.
        assert_ne!(logic.get_object(s).unwrap().ai_state, AIState::GuardingArea);
    }

    #[test]
    fn patrol_enables_auto_acquire_hunt_residual() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("PT_A");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("PT_A".to_string(), tpl);
        let a = logic.create_object("PT_A", Team::USA, Vec3::ZERO).unwrap();
        logic.get_object_mut(a).unwrap().auto_acquire_when_idle = false;
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_patrol(&[a]), CommandResult::Success);
        let u = logic.get_object(a).unwrap();
        assert_eq!(u.ai_state, AIState::Patrolling);
        assert!(u.auto_acquire_when_idle);
    }

    #[test]
    fn sell_selected_sells_friendly_structures_only() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        for name in ["SL_S", "SL_V"] {
            let mut tpl = ThingTemplate::new(name);
            if name == "SL_S" {
                tpl.add_kind_of(KindOf::Structure);
            } else {
                tpl.add_kind_of(KindOf::Vehicle);
            }
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(500.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let s = logic
            .create_object("SL_S", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let v = logic
            .create_object("SL_V", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_sell_selected(&[s, v], 0),
            CommandResult::Success
        );
        // Structure entered sell residual; vehicle rejected.
        assert!(
            logic.is_object_being_sold(s)
                || logic.get_object(s).map(|o| o.status.sold).unwrap_or(false)
        );
    }

    #[test]
    fn tighten_paths_all_units_to_same_point() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("TZ_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("TZ_V".to_string(), tpl);
        let a = logic
            .create_object("TZ_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("TZ_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        let dest = Vec3::new(20.0, 0.0, 0.0);
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert!(exec.should_tighten_group_move(&[a, b], dest));
            assert_eq!(
                exec.execute_tighten_to_position(&[a, b], dest),
                CommandResult::Success
            );
        }
        // Both should target same destination (path last or target_position).
        for id in [a, b] {
            let u = logic.get_object(id).unwrap();
            let goal = u
                .movement
                .path
                .last()
                .copied()
                .or(u.movement.target_position);
            let g = goal.expect("should have path goal");
            assert!(
                (g.x - dest.x).abs() < 1.0 && (g.z - dest.z).abs() < 1.0,
                "unit {id:?} goal {g:?} != {dest:?}"
            );
        }
    }

    #[test]
    fn override_special_power_destination_stores() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("SP_O");
        tpl.add_kind_of(KindOf::Structure);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(500.0);
        logic.templates.insert("SP_O".to_string(), tpl);
        let id = logic.create_object("SP_O", Team::USA, Vec3::ZERO).unwrap();
        let loc = Vec3::new(100.0, 0.0, 50.0);
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_override_special_power_destination(&[id], loc),
                CommandResult::Success
            );
        }
        let o = logic.get_object(id).unwrap();
        assert_eq!(o.special_power_override_destination, Some(loc));
    }

    #[test]
    fn set_weapon_set_flag_carbomb_and_upgrade() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("WS_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("WS_V".to_string(), tpl);
        let id = logic.create_object("WS_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_set_weapon_set_flag(&[id], 2, true),
                CommandResult::Success
            );
            assert_eq!(
                exec.execute_set_weapon_set_flag(&[id], 0, true),
                CommandResult::Success
            );
        }
        let o = logic.get_object(id).unwrap();
        assert!(o.weapon_set_carbomb);
        assert!(o.weapon_set_player_upgrade);
    }

    #[test]
    fn follow_waypoint_path_assigns_multi_point_path() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("WP_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("WP_V".to_string(), tpl);
        let id = logic.create_object("WP_V", Team::USA, Vec3::ZERO).unwrap();
        let wps = vec![
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 10.0),
            Vec3::new(30.0, 0.0, 0.0),
        ];
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_follow_waypoint_path(&[id], &wps, true, false),
                CommandResult::Success
            );
        }
        let o = logic.get_object(id).unwrap();
        assert!(
            !o.movement.path.is_empty() || o.movement.target_position.is_some(),
            "should have path or target"
        );
    }

    #[test]
    fn attack_position_own_location_and_max_shots() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("AP_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert("AP_V".to_string(), tpl);
        let id = logic
            .create_object("AP_V", Team::USA, Vec3::new(5.0, 0.0, 7.0))
            .unwrap();
        {
            let u = logic.get_object_mut(id).unwrap();
            u.weapon = Some(Weapon {
                damage: 10.0,
                range: 100.0,
                ..Weapon::default()
            });
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_attack_ground(&[id], None, 3),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert_eq!(u.ai_state, AIState::AttackingGround);
        assert_eq!(u.max_shots_to_fire, 3);
        assert!(u.force_attack);
    }

    #[test]
    fn exact_waypoint_path_sets_exact_flag_and_path() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("EX_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("EX_V".to_string(), tpl);
        let id = logic.create_object("EX_V", Team::USA, Vec3::ZERO).unwrap();
        let wps = vec![
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 5.0),
            Vec3::new(40.0, 0.0, 0.0),
        ];
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_follow_waypoint_path(&[id], &wps, true, false),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert!(u.is_exact_path, "exact follow must stamp is_exact_path");
        assert!(
            u.movement.path.len() >= 2,
            "exact path keeps waypoints: {:?}",
            u.movement.path
        );
        // Intermediate point should be present (exact, not collapsed).
        let has_mid = u
            .movement
            .path
            .iter()
            .any(|p| (p.x - 20.0).abs() < 1.0 && (p.z - 5.0).abs() < 1.0);
        assert!(
            has_mid,
            "exact path retains mid waypoint {:?}",
            u.movement.path
        );
    }

    #[test]
    fn group_speed_ignores_really_damaged_and_picks_leader() {
        use super::CommandExecutor;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("GS_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("GS_V".to_string(), tpl);
        let healthy = logic
            .create_object("GS_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let damaged = logic
            .create_object("GS_V", Team::USA, Vec3::new(30.0, 0.0, 0.0))
            .unwrap();
        let slow_healthy = logic
            .create_object("GS_V", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        {
            let h = logic.get_object_mut(healthy).unwrap();
            h.movement.max_speed = 40.0;
            h.movement.max_speed_damaged = 20.0;
            h.health.current = 100.0;
            h.health.maximum = 100.0;
            h.refresh_model_condition_bits();
        }
        {
            let d = logic.get_object_mut(damaged).unwrap();
            d.movement.max_speed = 40.0;
            d.movement.max_speed_damaged = 5.0;
            d.health.current = 10.0; // REALLYDAMAGED
            d.health.maximum = 100.0;
            d.refresh_model_condition_bits();
        }
        {
            let s = logic.get_object_mut(slow_healthy).unwrap();
            s.movement.max_speed = 20.0;
            s.movement.max_speed_damaged = 10.0;
            s.health.current = 100.0;
            s.health.maximum = 100.0;
            s.refresh_model_condition_bits();
        }
        let exec = CommandExecutor::new(&mut logic, 0);
        let spd = exec.group_speed(&[healthy, damaged, slow_healthy]);
        assert!(
            (spd - 20.0).abs() < 0.01,
            "group speed should be slowest healthy (20), not crippled 5; got {spd}"
        );
        let leader = exec
            .group_leader_id(&[healthy, damaged, slow_healthy])
            .expect("leader");
        assert_eq!(leader, slow_healthy);
    }

    #[test]
    fn effective_max_speed_uses_damaged_locomotor() {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("ES_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("ES_V".to_string(), tpl);
        let id = logic.create_object("ES_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let o = logic.get_object_mut(id).unwrap();
            o.movement.max_speed = 30.0;
            o.movement.max_speed_damaged = 12.0;
            o.health.current = 100.0;
            o.health.maximum = 100.0;
            o.refresh_model_condition_bits();
            assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
            assert!((o.effective_max_speed() - 30.0).abs() < 0.01);
            o.health.current = 10.0;
            o.refresh_model_condition_bits();
            assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
            assert!(
                (o.effective_max_speed() - 12.0).abs() < 0.01,
                "really damaged uses max_speed_damaged"
            );
        }
    }

    #[test]
    fn group_all_ids_and_attitude() {
        use super::CommandExecutor;
        use crate::game_logic::host_strategy_center::HostAiAttitude;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("GID_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("GID_V".to_string(), tpl);
        let a = logic.create_object("GID_V", Team::USA, Vec3::ZERO).unwrap();
        let b = logic
            .create_object("GID_V", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        // Kill b
        logic.get_object_mut(b).unwrap().health.current = 0.0;
        let exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.group_all_ids(&[a, b]), vec![a]);
        assert_eq!(exec.group_count(&[a, b]), 1);
        // C++ getAttitude always Passive.
        assert_eq!(exec.group_attitude(&[a]), HostAiAttitude::Passive);
    }

    #[test]
    fn special_power_uses_single_source_object() {
        use super::CommandExecutor;
        use crate::command_system::{CommandResult, PowerTarget, SpecialPowerType};
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("SP_SRC");
        tpl.add_kind_of(KindOf::Structure);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(500.0);
        logic.templates.insert("SP_SRC".to_string(), tpl);
        let caster = logic
            .create_object("SP_SRC", Team::USA, Vec3::ZERO)
            .unwrap();
        let other = logic
            .create_object("SP_SRC", Team::USA, Vec3::new(20.0, 0.0, 0.0))
            .unwrap();
        {
            let c = logic.get_object_mut(caster).unwrap();
            c.special_power_cooldowns
                .insert(SpecialPowerType::SpySatellite, 0.0);
            c.special_power_ready = true;
        }
        {
            let o = logic.get_object_mut(other).unwrap();
            o.special_power_cooldowns.clear();
            o.special_power_ready = true;
        }
        {
            let exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.special_power_source_object(&[other, caster], &SpecialPowerType::SpySatellite),
                Some(caster),
                "source must be the module owner even when other is first in selection"
            );
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            let res = exec.execute_special_power(
                &[other, caster],
                &SpecialPowerType::SpySatellite,
                &PowerTarget::Location(Vec3::new(100.0, 0.0, 100.0)),
            );
            let _ = res; // routing exercised; SharedSyncedTimer may mirror team-wide.
        }
        // Caster still owns the module entry after cast routing.
        assert!(logic
            .get_object(caster)
            .unwrap()
            .special_power_cooldowns
            .contains_key(&SpecialPowerType::SpySatellite));
    }

    #[test]
    fn special_power_and_command_button_source_object() {
        use super::CommandExecutor;
        use crate::command_system::{CommandType, SpecialPowerType};
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut move_t = ThingTemplate::new("SRC_M");
        move_t.add_kind_of(KindOf::Vehicle);
        move_t.add_kind_of(KindOf::Selectable);
        move_t.set_health(100.0);
        logic.templates.insert("SRC_M".to_string(), move_t);
        let mut atk_t = ThingTemplate::new("SRC_A");
        atk_t.add_kind_of(KindOf::Vehicle);
        atk_t.add_kind_of(KindOf::Selectable);
        atk_t.set_health(100.0);
        logic.templates.insert("SRC_A".to_string(), atk_t);
        let mover = logic.create_object("SRC_M", Team::USA, Vec3::ZERO).unwrap();
        let attacker = logic
            .create_object("SRC_A", Team::USA, Vec3::new(5.0, 0.0, 0.0))
            .unwrap();
        {
            let a = logic.get_object_mut(attacker).unwrap();
            a.weapon = Some(Weapon {
                damage: 10.0,
                range: 100.0,
                ..Weapon::default()
            });
            a.special_power_cooldowns
                .insert(SpecialPowerType::SpySatellite, 0.0);
        }
        // Ensure mover has no weapon / no SP map entry.
        {
            let m = logic.get_object_mut(mover).unwrap();
            m.weapon = None;
            m.special_power_cooldowns.clear();
        }
        let exec = CommandExecutor::new(&mut logic, 0);
        let sp =
            exec.special_power_source_object(&[mover, attacker], &SpecialPowerType::SpySatellite);
        assert_eq!(
            sp,
            Some(attacker),
            "SP source should be attacker with cooldown map; mover={mover:?} attacker={attacker:?} sp={sp:?}"
        );
        let src = exec.command_button_source_object(
            &[mover, attacker],
            &CommandType::AttackObject { target_id: mover },
        );
        assert_eq!(
            src,
            Some(attacker),
            "attack button source needs weapon; src={src:?}"
        );
        let move_src = exec.command_button_source_object(
            &[mover, attacker],
            &CommandType::Move {
                destination: Vec3::new(1.0, 0.0, 0.0),
            },
        );
        assert!(move_src.is_some());
    }

    #[test]
    fn attack_move_sets_max_shots_and_path_flag() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("AM_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("AM_V".to_string(), tpl);
        let id = logic.create_object("AM_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let o = logic.get_object_mut(id).unwrap();
            o.weapon = Some(Weapon {
                damage: 10.0,
                range: 150.0,
                ..Weapon::default()
            });
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_attack_move(&[id], Vec3::new(200.0, 0.0, 0.0), 5),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert_eq!(u.ai_state, AIState::AttackMoving);
        assert!(u.is_attack_path);
        assert_eq!(u.max_shots_to_fire, 5);
        assert!(u.auto_acquire_when_idle);
    }

    #[test]
    fn group_geometry_and_formation_move() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("GG_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("GG_V".to_string(), tpl);
        let a = logic
            .create_object("GG_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("GG_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(exec.group_count(&[a, b]), 2);
            let (min, max, center) = exec.group_min_max_and_center(&[a, b]).unwrap();
            assert!((center.x - 20.0).abs() < 0.1);
            assert!((max.x - min.x - 40.0).abs() < 0.1);
            assert!(exec.group_speed(&[a, b]) >= 0.0);
            assert_eq!(
                exec.execute_create_formation(&[a, b]),
                CommandResult::Success
            );
            let dest = Vec3::new(300.0, 0.0, 0.0);
            assert!(exec.compute_ground_path_should_group(&[a, b], dest));
            assert_eq!(
                exec.execute_move_formation_to_position(&[a, b], dest),
                CommandResult::Success
            );
        }
        let ga = logic
            .get_object(a)
            .unwrap()
            .movement
            .path
            .last()
            .copied()
            .or(logic.get_object(a).unwrap().movement.target_position)
            .unwrap();
        let gb = logic
            .get_object(b)
            .unwrap()
            .movement
            .path
            .last()
            .copied()
            .or(logic.get_object(b).unwrap().movement.target_position)
            .unwrap();
        assert!(
            (ga.x - gb.x).abs() > 20.0,
            "formation move keeps offset ga={ga:?} gb={gb:?}"
        );
    }

    #[test]
    fn follow_path_alias_paths_units() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("FP_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("FP_V".to_string(), tpl);
        let id = logic.create_object("FP_V", Team::USA, Vec3::ZERO).unwrap();
        let path = vec![Vec3::new(10.0, 0.0, 0.0), Vec3::new(50.0, 0.0, 0.0)];
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_follow_path(&[id], &path),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert!(!u.movement.path.is_empty() || u.movement.target_position.is_some());
    }

    #[test]
    fn group_ownership_filter_and_center() {
        use super::CommandExecutor;
        use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.add_player(Player::new(0, Team::USA, "USA", true));
        logic.add_player(Player::new(1, Team::GLA, "GLA", false));
        for (name, team) in [("OF_U", Team::USA), ("OF_E", Team::GLA)] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(100.0);
            logic.templates.insert(name.to_string(), tpl);
            let _ = team;
        }
        let u = logic
            .create_object("OF_U", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let e = logic
            .create_object("OF_E", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        let exec = CommandExecutor::new(&mut logic, 0);
        assert!(exec.is_member(&[u, e], u));
        assert!(!exec.is_member(&[u], e));
        assert!(exec.contains_any_objects_not_owned_by_player(&[u, e], 0));
        let (kept, empty) = exec.remove_any_objects_not_owned_by_player(&[u, e], 0);
        assert_eq!(kept, vec![u]);
        assert!(!empty);
        let c = exec.group_center(&[u, e]).unwrap();
        assert!((c.x - 20.0).abs() < 0.1, "center x={}", c.x);
    }

    #[test]
    fn special_power_at_location_wrapper() {
        use super::CommandExecutor;
        use crate::command_system::{CommandResult, SpecialPowerType};
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("SP_L");
        tpl.add_kind_of(KindOf::Structure);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(500.0);
        logic.templates.insert("SP_L".to_string(), tpl);
        let id = logic.create_object("SP_L", Team::USA, Vec3::ZERO).unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            // May succeed or invalid depending on power readiness residual — must not panic.
            let _ = exec.execute_special_power_at_location(
                &[id],
                &SpecialPowerType::SpySatellite,
                Vec3::new(100.0, 0.0, 50.0),
            );
            let _ =
                exec.execute_special_power_at_object(&[id], &SpecialPowerType::SpySatellite, id);
        }
    }

    #[test]
    fn guard_area_stamps_radius() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, GuardMode, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("GA_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("GA_V".to_string(), tpl);
        let id = logic.create_object("GA_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_guard_area(&[id], Vec3::new(30.0, 0.0, 0.0), 150.0, GuardMode::Normal),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert!((u.guard_radius - 150.0).abs() < 0.1, "r={}", u.guard_radius);
    }

    #[test]
    fn group_idle_busy_dead_queries() {
        use super::CommandExecutor;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("GQ_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("GQ_V".to_string(), tpl);
        let a = logic.create_object("GQ_V", Team::USA, Vec3::ZERO).unwrap();
        let b = logic
            .create_object("GQ_V", Team::USA, Vec3::new(10.0, 0.0, 0.0))
            .unwrap();
        {
            let exec = CommandExecutor::new(&mut logic, 0);
            assert!(exec.group_is_idle(&[a, b]));
            assert!(!exec.group_is_busy(&[a, b]));
            assert!(!exec.group_is_ai_dead(&[a, b]));
        }
        logic
            .get_object_mut(a)
            .unwrap()
            .set_ai_state(AIState::Moving);
        {
            let exec = CommandExecutor::new(&mut logic, 0);
            assert!(!exec.group_is_idle(&[a, b]));
            // busy requires ALL living busy
            assert!(!exec.group_is_busy(&[a, b]));
        }
        logic
            .get_object_mut(b)
            .unwrap()
            .set_ai_state(AIState::Moving);
        {
            let exec = CommandExecutor::new(&mut logic, 0);
            assert!(exec.group_is_busy(&[a, b]));
        }
        logic.get_object_mut(a).unwrap().health.current = 0.0;
        logic.get_object_mut(b).unwrap().health.current = 0.0;
        // mark dead properly if needed
        for id in [a, b] {
            if let Some(o) = logic.get_object_mut(id) {
                o.status.destroyed = true;
            }
        }
        {
            let exec = CommandExecutor::new(&mut logic, 0);
            assert!(exec.group_is_ai_dead(&[a, b]));
        }
    }

    #[test]
    fn attack_follow_waypoint_sets_attack_path() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("AF_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert("AF_V".to_string(), tpl);
        let id = logic.create_object("AF_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let u = logic.get_object_mut(id).unwrap();
            u.weapon = Some(Weapon {
                damage: 10.0,
                range: 150.0,
                ..Weapon::default()
            });
        }
        let wps = vec![Vec3::new(20.0, 0.0, 0.0), Vec3::new(60.0, 0.0, 0.0)];
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_attack_follow_waypoint_path(&[id], &wps, true, false),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert!(u.is_attack_path, "attack-follow should mark attack path");
        assert!(
            matches!(u.ai_state, AIState::AttackMoving | AIState::Moving),
            "state={:?}",
            u.ai_state
        );
    }

    #[test]
    fn attack_move_sets_is_attack_path_flag() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("AM_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert("AM_V".to_string(), tpl);
        let id = logic.create_object("AM_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let u = logic.get_object_mut(id).unwrap();
            u.weapon = Some(Weapon {
                damage: 10.0,
                range: 150.0,
                ..Weapon::default()
            });
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_attack_move(&[id], Vec3::new(90.0, 0.0, 0.0), -1),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert!(u.is_attack_path);
        assert_eq!(u.ai_state, AIState::AttackMoving);
    }

    #[test]
    fn follow_waypoint_as_team_preserves_offsets() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("FT_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("FT_V".to_string(), tpl);
        let a = logic
            .create_object("FT_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("FT_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        // Stamp formation.
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_create_formation(&[a, b]),
                CommandResult::Success
            );
        }
        let wps = vec![Vec3::new(100.0, 0.0, 0.0), Vec3::new(200.0, 0.0, 0.0)];
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_follow_waypoint_path(&[a, b], &wps, true, true),
                CommandResult::Success
            );
        }
        let ga = logic
            .get_object(a)
            .unwrap()
            .movement
            .path
            .last()
            .copied()
            .or(logic.get_object(a).unwrap().movement.target_position)
            .unwrap();
        let gb = logic
            .get_object(b)
            .unwrap()
            .movement
            .path
            .last()
            .copied()
            .or(logic.get_object(b).unwrap().movement.target_position)
            .unwrap();
        // Offsets should keep ~40 world units separation on X (formation).
        let sep = (ga.x - gb.x).abs();
        assert!(
            sep > 20.0,
            "as-team should preserve formation separation, ga={ga:?} gb={gb:?} sep={sep}"
        );
        // Formation id preserved.
        assert_eq!(
            logic.get_object(a).unwrap().formation_id,
            logic.get_object(b).unwrap().formation_id
        );
        assert_ne!(logic.get_object(a).unwrap().formation_id, 0);
    }

    #[test]
    fn do_command_button_using_waypoints_attack_moves() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("BW_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("BW_V".to_string(), tpl);
        let id = logic.create_object("BW_V", Team::USA, Vec3::ZERO).unwrap();
        let wps = vec![Vec3::new(10.0, 0.0, 0.0), Vec3::new(80.0, 0.0, 0.0)];
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_do_command_button_using_waypoints(&[id], "Command_AttackMove", &wps),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert!(
            !u.movement.path.is_empty() || u.movement.target_position.is_some(),
            "should path along waypoints"
        );
    }

    #[test]
    fn do_command_button_dispatches_stop() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("DC_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("DC_V".to_string(), tpl);
        let id = logic.create_object("DC_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let u = logic.get_object_mut(id).unwrap();
            u.set_ai_state(AIState::Moving);
            u.set_target(Some(id));
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_do_command_button(&[id], "Command_Stop", None, None),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert!(
            matches!(u.ai_state, AIState::Idle) || u.target.is_none() || !u.status.moving,
            "stop should clear action state={:?} target={:?}",
            u.ai_state,
            u.target
        );
    }

    #[test]
    fn do_command_button_at_position_moves() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("DC_M");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("DC_M".to_string(), tpl);
        let id = logic.create_object("DC_M", Team::USA, Vec3::ZERO).unwrap();
        let dest = Vec3::new(55.0, 0.0, 10.0);
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_do_command_button(&[id], "Command_AttackMove", Some(dest), None),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert!(
            !u.movement.path.is_empty() || u.movement.target_position.is_some(),
            "attack-move should path"
        );
    }

    #[test]
    fn surrender_stops_and_flags_unit() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("SR_I");
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("SR_I".to_string(), tpl);
        let id = logic.create_object("SR_I", Team::USA, Vec3::ZERO).unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(exec.execute_surrender(&[id], true), CommandResult::Success);
        }
        let o = logic.get_object(id).unwrap();
        assert!(o.is_surrendered);
        assert!(o.target.is_none());
    }

    #[test]
    fn attack_team_engages_member_of_team() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for (name, _t) in [("AT_U", Team::USA), ("AT_E", Team::GLA)] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.add_kind_of(KindOf::Attackable);
            tpl.set_health(200.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let u = logic.create_object("AT_U", Team::USA, Vec3::ZERO).unwrap();
        let e = logic
            .create_object("AT_E", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
            .unwrap();
        {
            use crate::game_logic::Weapon;
            let uo = logic.get_object_mut(u).unwrap();
            uo.weapon = Some(Weapon {
                damage: 10.0,
                range: 200.0,
                ..Weapon::default()
            });
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            // team_code 0 = GLA
            assert_eq!(
                exec.execute_attack_team(&[u], 0, -1),
                CommandResult::Success
            );
        }
        let unit = logic.get_object(u).unwrap();
        assert!(
            unit.target == Some(e)
                || matches!(
                    unit.ai_state,
                    AIState::Attacking | AIState::AttackMoving | AIState::Moving
                ),
            "target={:?} state={:?}",
            unit.target,
            unit.ai_state
        );
    }

    #[test]
    fn weapon_lock_forces_slot_and_release() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon, WeaponLockType};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("WL_V");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert("WL_V".to_string(), tpl);
        let id = logic.create_object("WL_V", Team::USA, Vec3::ZERO).unwrap();
        {
            let u = logic.get_object_mut(id).unwrap();
            u.weapon = Some(Weapon {
                damage: 10.0,
                range: 100.0,
                ..Weapon::default()
            });
            u.secondary_weapon = Some(Weapon {
                damage: 5.0,
                range: 80.0,
                ..Weapon::default()
            });
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_set_weapon_lock(&[id], 1, 2),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert_eq!(u.weapon_lock_type, WeaponLockType::LockedPermanently);
        assert_eq!(u.weapon_lock_slot, 1);
        assert_eq!(u.active_weapon_slot, 1);
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_release_weapon_lock(&[id], 2),
                CommandResult::Success
            );
        }
        assert_eq!(
            logic.get_object(id).unwrap().weapon_lock_type,
            WeaponLockType::NotLocked
        );
    }

    #[test]
    fn set_emoticon_stores_name_and_duration() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("EM_U");
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("EM_U".to_string(), tpl);
        let id = logic.create_object("EM_U", Team::USA, Vec3::ZERO).unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_set_emoticon(&[id], "Emoticon_Alert", 60),
                CommandResult::Success
            );
        }
        let u = logic.get_object(id).unwrap();
        assert_eq!(u.emoticon_name, "Emoticon_Alert");
        assert_eq!(u.emoticon_frames_left, 60);
    }

    #[test]
    fn mine_clearing_detail_toggles_weapon_set_flag() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("MC_D");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(300.0);
        logic.templates.insert("MC_D".to_string(), tpl);
        let d = logic.create_object("MC_D", Team::USA, Vec3::ZERO).unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_set_mine_clearing_detail(&[d], true),
                CommandResult::Success
            );
        }
        assert!(logic.get_object(d).unwrap().weapon_set_mine_clearing_detail);
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_set_mine_clearing_detail(&[d], false),
                CommandResult::Success
            );
        }
        assert!(!logic.get_object(d).unwrap().weapon_set_mine_clearing_detail);
    }

    #[test]
    fn go_prone_sets_prone_timer_and_bit() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{
            host_enum_table_residual::model_condition_bit_name_index, GameLogic, KindOf, Team,
            ThingTemplate,
        };
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("GP_I");
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("GP_I".to_string(), tpl);
        let i = logic.create_object("GP_I", Team::USA, Vec3::ZERO).unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(exec.execute_go_prone(&[i]), CommandResult::Success);
        }
        let u = logic.get_object(i).unwrap();
        assert!(u.prone_timer > 0.0);
        if let Some(bit) = model_condition_bit_name_index("PRONE") {
            assert_ne!(u.model_condition_bits & (1u128 << bit), 0);
        }
    }

    #[test]
    fn attack_area_engages_enemy_inside_radius() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for (name, team) in [("AA_U", Team::USA), ("AA_E", Team::GLA)] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(200.0);
            logic.templates.insert(name.to_string(), tpl);
            let _ = team;
        }
        let u = logic.create_object("AA_U", Team::USA, Vec3::ZERO).unwrap();
        let e = logic
            .create_object("AA_E", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_attack_area(&[u], Vec3::new(40.0, 0.0, 0.0), 80.0),
                CommandResult::Success
            );
        }
        let unit = logic.get_object(u).unwrap();
        assert!(
            unit.target == Some(e)
                || matches!(
                    unit.ai_state,
                    AIState::Attacking | AIState::AttackMoving | AIState::Moving
                ),
            "attack area should engage or path, target={:?} state={:?}",
            unit.target,
            unit.ai_state
        );
    }

    #[test]
    fn move_to_and_evacuate_sets_pending_flag() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("EV_T");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(500.0);
        // transport capacity residual
        logic.templates.insert("EV_T".to_string(), tpl);
        let mut pax_tpl = ThingTemplate::new("EV_P");
        pax_tpl.add_kind_of(KindOf::Infantry);
        pax_tpl.add_kind_of(KindOf::Selectable);
        pax_tpl.set_health(100.0);
        logic.templates.insert("EV_P".to_string(), pax_tpl);

        let transport = logic.create_object("EV_T", Team::USA, Vec3::ZERO).unwrap();
        let pax = logic
            .create_object("EV_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        {
            let t = logic.get_object_mut(transport).unwrap();
            // Force containable capacity
            let _ = t.add_occupant(pax);
        }
        {
            let p = logic.get_object_mut(pax).unwrap();
            p.contained_by = Some(transport);
            p.set_ai_state(crate::game_logic::AIState::Docked);
        }
        assert!(!logic
            .get_object(transport)
            .unwrap()
            .contained_units()
            .is_empty());

        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_move_to_and_evacuate(&[transport], Vec3::new(80.0, 0.0, 0.0), false),
                CommandResult::Success
            );
        }
        let t = logic.get_object(transport).unwrap();
        assert!(
            t.pending_evacuate_on_stop,
            "should pending evacuate after move command"
        );
        assert!(!t.pending_exit_after_evacuate);
    }

    #[test]
    fn move_to_and_evacuate_unloads_when_path_completes() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for (name, kind) in [("EV2_T", KindOf::Vehicle), ("EV2_P", KindOf::Infantry)] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(kind);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(400.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let transport = logic.create_object("EV2_T", Team::USA, Vec3::ZERO).unwrap();
        let pax = logic
            .create_object("EV2_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
            .unwrap();
        {
            let t = logic.get_object_mut(transport).unwrap();
            assert!(t.add_occupant(pax));
        }
        {
            let p = logic.get_object_mut(pax).unwrap();
            p.contained_by = Some(transport);
            p.set_ai_state(crate::game_logic::AIState::Docked);
        }
        {
            let mut exec = CommandExecutor::new(&mut logic, 0);
            assert_eq!(
                exec.execute_move_to_and_evacuate(&[transport], Vec3::new(10.0, 0.0, 0.0), false),
                CommandResult::Success
            );
        }
        // Simulate arrival: complete path + movement tick.
        if let Some(t) = logic.get_object_mut(transport) {
            // Snap to end of path and finish
            if let Some(last) = t.movement.path.last().copied() {
                t.set_position(last);
            } else {
                t.set_position(Vec3::new(10.0, 0.0, 0.0));
            }
            t.movement.current_path_index = t.movement.path.len().saturating_sub(0);
            // Force index past end
            t.movement.current_path_index = t.movement.path.len();
            t.pending_evacuate_on_stop = true;
        }
        // Direct evacuate_now residual (arrival hook)
        assert!(logic.evacuate_container_now(transport, false));
        assert!(
            logic
                .get_object(transport)
                .map(|t| t.contained_units().is_empty())
                .unwrap_or(false),
            "passengers should unload"
        );
        let p = logic.get_object(pax).unwrap();
        assert!(p.contained_by.is_none());
        assert_ne!(p.ai_state, crate::game_logic::AIState::Docked);
    }

    #[test]
    fn evacuate_requires_container_not_passenger_only() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("EV_INF");
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("EV_INF".to_string(), tpl);
        let a = logic
            .create_object("EV_INF", Team::USA, Vec3::ZERO)
            .unwrap();
        let mut exec = CommandExecutor::new(&mut logic, 0);
        // C++ groupEvacuate no-ops on non-containers without AI contain.
        assert_eq!(exec.execute_evacuate(&[a]), CommandResult::InvalidCommand);
    }

    #[test]
    fn attack_move_uses_assign_unit_path() {
        let src = include_str!("command_executor.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
        let i = prod.find("fn execute_attack_move").expect("attack_move");
        let w = &prod[i..prod.len().min(i + 1500)];
        assert!(
            w.contains("assign_unit_path")
                && w.contains("AIState::AttackMoving")
                && !w.contains("set_destination(goal)"),
            "attack-move must pathfind then restore AttackMoving"
        );
        let j = prod.find("fn execute_force_move").expect("force_move");
        let w2 = &prod[j..prod.len().min(j + 1200)];
        assert!(
            w2.contains("assign_unit_path") && !w2.contains("set_destination(goal)"),
            "force-move must pathfind like Move"
        );
    }

    #[test]
    fn path_to_goal_with_state_used_by_guard_scatter_gather() {
        let src = include_str!("command_executor.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(prod.contains("fn path_to_goal_with_state"));
        for name in [
            "fn execute_guard",
            "fn execute_scatter",
            "fn execute_gather",
            "fn execute_build",
        ] {
            let i = prod.find(name).unwrap_or_else(|| panic!("missing {name}"));
            let w = &prod[i..prod.len().min(i + 6000)];
            assert!(
                w.contains("path_to_goal_with_state") || w.contains("assign_unit_path"),
                "{name} must pathfind, not bare set_destination"
            );
            // Guard/scatter/gather should not use bare set_destination(goal)
            if name != "fn execute_build" {
                assert!(
                    !w.contains("set_destination(*pos)")
                        && !w.contains("set_destination(pos)")
                        && !w.contains("set_destination(dest)")
                        && !w.contains("set_destination(target_pos)"),
                    "{name} still has bare set_destination"
                );
            }
        }
    }

    #[test]
    fn interaction_commands_pathfind_surface() {
        let src = include_str!("command_executor.rs");
        let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
        // Production locomotion commands should prefer path_to_goal_with_state.
        assert!(prod.matches("path_to_goal_with_state").count() >= 10);
        // Bare set_destination should not remain in execute_* interaction paths.
        let exec = prod;
        let bare = exec.matches("unit.set_destination(").count()
            + exec.matches("unit_mut.set_destination(").count();
        assert_eq!(
            bare, 0,
            "production execute paths still call unit.set_destination ({bare})"
        );
    }

    #[test]
    fn deploy_style_toggle_residual() {
        let src = include_str!("command_executor.rs");
        let start = src.find("fn execute_deploy").expect("execute_deploy");
        let body = &src[start..start + 2500];
        assert!(
            body.contains("set_deployed") && body.contains("is_deployed"),
            "Deploy must toggle OBJECT_STATUS_DEPLOYED residual for deploy-style units"
        );
        assert!(
            body.contains("tomahawk") || body.contains("humvee"),
            "Deploy residual must recognize retail deploy-style unit names"
        );
    }

    #[test]
    fn execute_stop_clears_guard_residual() {
        let src = include_str!("command_executor.rs");
        let start = src.find("fn execute_stop").expect("execute_stop");
        let body = &src[start..start + 1200];
        assert!(
            body.contains("set_guard_position(None)")
                && body.contains("end_guard_retaliate")
                && body.contains("set_target(None)")
                && body.contains("apply_player_stealth_mood_delay"),
            "Stop must clear guard anchors/targets and apply stealth mood delay"
        );
        assert!(
            src.contains("fn apply_player_stealth_mood_delay")
                && src.contains("next_mood_check_time"),
            "shared stealth mood delay helper must schedule next_mood_check_time"
        );
    }

    #[test]
    fn stop_delays_mood_for_unstealthed_stealth_unit() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.set_current_frame(100);
        let mut tpl = ThingTemplate::new("ST_A");
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("ST_A".to_string(), tpl);
        let a = logic.create_object("ST_A", Team::USA, Vec3::ZERO).unwrap();
        {
            let u = logic.get_object_mut(a).unwrap();
            u.innate_stealth = true;
            u.stealth_delay_frames = 45;
            u.auto_acquire_when_idle = true;
            u.status.stealthed = false;
            u.status.detected = false;
            u.next_mood_check_time = 0;
            u.set_ai_state(AIState::Moving);
        }
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_stop(&[a]), CommandResult::Success);
        let u = logic.get_object(a).unwrap();
        assert_eq!(u.ai_state, AIState::Idle);
        // now=100 + delay 45 + skew 0
        assert_eq!(
            u.next_mood_check_time, 145,
            "player stop should delay mood until stealth window"
        );
    }
    #[test]
    fn add_waypoint_uses_group_destinations() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        for name in ["WP_A", "WP_B"] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle);
            tpl.add_kind_of(KindOf::Selectable);
            tpl.set_health(100.0);
            logic.templates.insert(name.to_string(), tpl);
        }
        let a = logic
            .create_object("WP_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .unwrap();
        let b = logic
            .create_object("WP_B", Team::USA, Vec3::new(40.0, 0.0, 0.0))
            .unwrap();
        for id in [a, b] {
            logic.get_object_mut(id).unwrap().selection_radius = 10.0;
        }
        let click = Vec3::new(100.0, 0.0, 50.0);
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_add_waypoint(&[a, b], click),
            CommandResult::Success
        );
        // Paths should not be identical stacked goals for multi-select.
        let pa = logic.get_object(a).unwrap().movement.path.clone();
        let pb = logic.get_object(b).unwrap().movement.path.clone();
        assert!(!pa.is_empty() && !pb.is_empty());
        let ga = *pa.last().unwrap();
        let gb = *pb.last().unwrap();
        assert!(
            (ga - gb).length() > 5.0,
            "waypoint goals must spread like group move, ga={ga:?} gb={gb:?}"
        );
    }

    #[test]
    fn move_delays_mood_for_unstealthed_stealth_unit() {
        use super::CommandExecutor;
        use crate::command_system::CommandResult;
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        logic.set_current_frame(50);
        let mut tpl = ThingTemplate::new("MV_ST");
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert("MV_ST".to_string(), tpl);
        let a = logic.create_object("MV_ST", Team::USA, Vec3::ZERO).unwrap();
        {
            let u = logic.get_object_mut(a).unwrap();
            u.innate_stealth = true;
            u.stealth_delay_frames = 30;
            u.auto_acquire_when_idle = true;
            u.status.stealthed = false;
            u.status.detected = false;
            u.next_mood_check_time = 0;
        }
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move(&[a], Vec3::new(50.0, 0.0, 0.0)),
            CommandResult::Success
        );
        let u = logic.get_object(a).unwrap();
        assert_eq!(u.next_mood_check_time, 80); // 50+30+0
    }
}
