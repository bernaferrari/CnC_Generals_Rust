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
            CommandType::AttackMoveTo { destination } => {
                self.execute_attack_move(&command.selected_units, *destination)
            }
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
                self.execute_attack_ground(&command.selected_units, *location)
            }
            CommandType::Stop => self.execute_stop(&command.selected_units),
            CommandType::Guard { target } => self.execute_guard(&command.selected_units, target),
            CommandType::Patrol => self.execute_patrol(&command.selected_units),
            CommandType::Scatter => self.execute_scatter(&command.selected_units),
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

    fn execute_move(&mut self, units: &[ObjectId], destination: Vec3) -> CommandResult {
        let goals = self.group_move_destinations(units, destination);
        for (unit_id, goal) in goals {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            } else {
                return CommandResult::InvalidTarget;
            }
            if !self.game_logic.assign_unit_path(unit_id, goal, &[]) {
                return CommandResult::InvalidCommand;
            }
            debug!("Unit {} moving to {:?}", unit_id.0, goal);
        }
        CommandResult::Success
    }

    fn execute_move_to(
        &mut self,
        units: &[ObjectId],
        destination: Vec3,
        waypoints: &[Vec3],
    ) -> CommandResult {
        let goals = self.group_move_destinations(units, destination);
        for (unit_id, goal) in goals {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            } else {
                return CommandResult::InvalidTarget;
            }
            if !self.game_logic.assign_unit_path(unit_id, goal, waypoints) {
                return CommandResult::InvalidCommand;
            }
            debug!("Unit {} moving via waypoints to {:?}", unit_id.0, goal);
        }
        CommandResult::Success
    }

    /// C++-style group move goal spread: one unit keeps the click point; others
    /// ring around it by selection radius so paths/goals don't collapse to one cell.
    fn group_move_destinations(
        &self,
        units: &[ObjectId],
        destination: Vec3,
    ) -> Vec<(ObjectId, Vec3)> {
        if units.len() <= 1 {
            return units.iter().map(|&id| (id, destination)).collect();
        }
        let n = units.len() as f32;
        let mut out = Vec::with_capacity(units.len());
        for (i, &unit_id) in units.iter().enumerate() {
            let spread = self
                .game_logic
                .get_object(unit_id)
                .map(|u| u.selection_radius.max(6.0))
                .unwrap_or(8.0);
            // Keep first unit on the exact click; ring the rest (index 1..).
            let goal = if i == 0 {
                destination
            } else {
                let angle = (i as f32) * std::f32::consts::TAU / (n - 1.0).max(1.0);
                // Slight radial growth for larger groups so outer ring clears.
                let ring = spread * (1.0 + ((i as f32) / n) * 0.35);
                destination + Vec3::new(angle.cos() * ring, 0.0, angle.sin() * ring)
            };
            out.push((unit_id, goal));
        }
        out
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

    fn execute_attack_move(&mut self, units: &[ObjectId], destination: Vec3) -> CommandResult {
        let goals = self.group_move_destinations(units, destination);
        for (unit_id, goal) in goals {
            // Capability check first (borrow ends before assign_unit_path).
            let ok = match self.game_logic.get_object(unit_id) {
                Some(unit) => unit.can_move() && unit.can_attack(),
                None => return CommandResult::InvalidTarget,
            };
            if !ok {
                continue;
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop_attack();
            }
            // Path through host A* like Move — straight-line set_destination skipped
            // obstacles and left AttackMoving units stuck behind buildings.
            if !self.game_logic.assign_unit_path(unit_id, goal, &[]) {
                return CommandResult::InvalidCommand;
            }
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                // assign_unit_path leaves AIState::Moving; restore attack-move mode.
                unit.set_ai_state(AIState::AttackMoving);
            }
        }
        CommandResult::Success
    }

    fn execute_force_move(&mut self, units: &[ObjectId], destination: Vec3) -> CommandResult {
        let goals = self.group_move_destinations(units, destination);
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
        }
        CommandResult::Success
    }

    fn execute_add_waypoint(&mut self, units: &[ObjectId], destination: Vec3) -> CommandResult {
        for &unit_id in units {
            if self.game_logic.get_object(unit_id).is_none() {
                return CommandResult::InvalidTarget;
            }
            if !self.game_logic.append_unit_waypoint(unit_id, destination) {
                return CommandResult::InvalidCommand;
            }
            debug!("Added waypoint for unit {} at {:?}", unit_id.0, destination);
        }
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

        let mut any_attacker = false;

        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                if unit.can_attack() && unit.team != target_team {
                    unit.set_force_attack(false);
                    unit.set_target(Some(target_id));
                    unit.set_ai_state(AIState::Attacking);
                    any_attacker = true;
                }
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

        let mut any_attacker = false;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                if unit.can_attack() {
                    unit.set_target(Some(target_id));
                    unit.set_force_attack(true);
                    unit.set_ai_state(AIState::Attacking);
                    any_attacker = true;
                }
            }
        }
        if any_attacker {
            CommandResult::Success
        } else {
            CommandResult::CannotAttackTarget
        }
    }

    fn execute_attack_ground(&mut self, units: &[ObjectId], location: Vec3) -> CommandResult {
        let mut any_attacker = false;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                if unit.can_attack() {
                    unit.set_force_attack(true);
                    unit.set_target_location(Some(location));
                    unit.set_ai_state(AIState::AttackingGround);
                    any_attacker = true;
                }
            }
        }
        if any_attacker {
            CommandResult::Success
        } else {
            CommandResult::CannotAttackTarget
        }
    }

    fn execute_stop(&mut self, units: &[ObjectId]) -> CommandResult {
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.stop();
                unit.set_target(None);
                unit.set_force_attack(false);
                // C++ stop clears guard/waypoint residual anchors.
                unit.set_guard_position(None);
                unit.set_guard_target(None);
                unit.end_guard_retaliate();
                unit.set_ai_state(AIState::Idle);
            }
        }
        CommandResult::Success
    }

    fn execute_guard(&mut self, units: &[ObjectId], target: &GuardTarget) -> CommandResult {
        for &unit_id in units {
            let target_pos = match target {
                GuardTarget::Position(pos) => Some(*pos),
                GuardTarget::Object(target_id) => {
                    self.game_logic.get_object(*target_id).map(|o| o.position)
                }
            };

            // Set guard anchors first (short borrow).
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.guard_radius = unit.selection_radius * 2.0;
                match target {
                    GuardTarget::Position(pos) => {
                        unit.set_guard_position(Some(*pos));
                    }
                    GuardTarget::Object(target_id) => {
                        unit.set_guard_target(Some(*target_id));
                    }
                }
            } else {
                continue;
            }
            // Path to guard anchor / object, then restore guard AI mode.
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
        }
        CommandResult::Success
    }

    fn execute_patrol(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                if !unit.is_alive() || !unit.can_move() {
                    continue;
                }
                unit.set_target(None);
                unit.set_force_attack(false);
                unit.set_guard_position(None);
                unit.set_guard_target(None);
                unit.end_guard_retaliate();
                // C++ AI_HUNT / patrol residual: wander + auto-engage.
                unit.set_ai_state(AIState::Patrolling);
                unit.status.moving = false;
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    fn execute_scatter(&mut self, units: &[ObjectId]) -> CommandResult {
        // Scatter units in deterministic radial offsets to avoid clumping.
        let mut any = false;
        let count = units.len() as f32;
        for (i, &unit_id) in units.iter().enumerate() {
            if let Some(unit) = self.game_logic.get_object(unit_id) {
                let angle = (i as f32) * std::f32::consts::TAU / count.max(1.0);
                let base_radius = match unit.object_type {
                    ObjectType::Infantry => 8.0,
                    ObjectType::Vehicle => 14.0,
                    ObjectType::Aircraft => 18.0,
                    _ => 10.0,
                };
                let radius = base_radius + (unit.selection_radius * 0.5);
                let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
                let dest = unit.position + offset;
                if self.path_to_goal_with_state(unit_id, dest, AIState::Moving) {
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
        let mut best_id: Option<ObjectId> = None;
        let mut best_dist = f32::MAX;

        for (&obj_id, obj) in self.game_logic.get_objects() {
            if obj.team != unit_team || !obj.is_alive() || !obj.can_contain() {
                continue;
            }
            if !obj.has_capacity_for(1) {
                continue;
            }
            let dist = obj.get_position().distance(unit_pos);
            if dist < best_dist {
                best_dist = dist;
                best_id = Some(obj_id);
            }
        }
        best_id
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

        let mut placed = false;
        let last_index = units.len().saturating_sub(1).max(1) as f32;

        for (i, &unit_id) in units.iter().enumerate() {
            let t = i as f32 / last_index;
            let pos = start + (end - start) * t;
            if self.execute_dozer_construct(&[unit_id], template_name, pos, 0.0)
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
            PowerTarget::None => units.first().and_then(|id| {
                self.game_logic
                    .get_object(*id)
                    .map(|obj| obj.get_position())
            }),
        };

        debug!(
            "Executing special power {:?} with target {:?}",
            power_type, target
        );
        let mut any = false;
        for &unit_id in units {
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
                    let placed = self
                        .game_logic
                        .place_cluster_mines(team, pos, Some(unit_id));
                    if placed.is_empty() {
                        continue;
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
                        (false, false, false, false, false, false, false, true)
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
                        (false, false, false, false, false, false, false, true)
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

    fn execute_evacuate(&mut self, units: &[ObjectId]) -> CommandResult {
        // Emergency exit all units
        self.execute_exit(units)
    }

    fn execute_dock(&mut self, units: &[ObjectId], target_id: ObjectId) -> CommandResult {
        let target_pos = if let Some(target) = self.game_logic.get_object(target_id) {
            if target.is_alive() && !target.status.under_construction && target.can_contain() {
                target.position
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
        let mut any = false;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.active_weapon_slot ^= 1;
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

    fn execute_create_formation(&mut self, units: &[ObjectId]) -> CommandResult {
        if units.is_empty() {
            return CommandResult::InvalidCommand;
        }

        // Use the centroid as a formation anchor and add a spread based on selection radius.
        let mut count = 0.0;
        let mut sum = Vec3::ZERO;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object(unit_id) {
                sum += unit.position;
                count += 1.0;
            }
        }
        if count == 0.0 {
            return CommandResult::InvalidCommand;
        }
        let anchor = sum / count;

        // Offset units slightly to reduce stacking, proportional to their selection radius.
        for (i, &unit_id) in units.iter().enumerate() {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                let angle = (i as f32) * std::f32::consts::TAU / (count.max(1.0));
                let spread = unit.selection_radius.max(6.0);
                let offset = Vec3::new(angle.cos() * spread, 0.0, angle.sin() * spread);
                let pos = anchor + offset;
                unit.guard_position = Some(pos);
                unit.guard_radius = spread * 1.5;
                unit.set_ai_state(AIState::GuardingArea);
            }
        }

        CommandResult::Success
    }

    fn execute_cheer(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if let Some(unit) = self.game_logic.get_object_mut(unit_id) {
                unit.set_ai_state(AIState::SpecialAbility);
                unit.cheer_timer = 2.0;
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
            let w = &prod[i..prod.len().min(i + 2500)];
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
        let body = &src[start..start + 700];
        assert!(
            body.contains("set_guard_position(None)")
                && body.contains("end_guard_retaliate")
                && body.contains("set_target(None)"),
            "Stop must clear guard anchors and targets residual"
        );
    }
}
