//! AIUpdateInterface::execute_command body.

#![allow(unused_imports)]

use super::ai_command_owner::CommandOwner;
use super::ai_core::UnitAIUpdate;
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{
    dual_world_registry_unavailable, get_unit_arc, with_unit_mut, with_unit_ref,
};
use super::types::*;
use crate::ai::states::AIStateMachineDriver;

impl UnitAIUpdate {
    fn finish_attack_object_command(
        &mut self,
        target_id: crate::common::ObjectID,
        command: &crate::ai::AiCommandParams,
        guard: CommandOwner<'_>,
    ) {
        if let Ok(mut obj_guard) = guard.base_arc().write() {
            obj_guard.set_current_weapon_max_shot_count(command.int_value);
        }
        if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_ref() {
            if command.cmd == crate::ai::AiCommandType::ForceAttackObject {
                chinook_ai.private_force_attack_object(
                    target_id,
                    command.int_value,
                    command.cmd_source,
                );
            } else {
                chinook_ai.private_attack_object(target_id, command.int_value, command.cmd_source);
            }
        }
        if let Some(transport_ai) = self.runtime.components.transport_ai.as_ref() {
            if command.cmd == crate::ai::AiCommandType::ForceAttackObject {
                transport_ai.private_force_attack_object(
                    target_id,
                    command.int_value,
                    command.cmd_source,
                );
            } else {
                transport_ai.private_attack_object(
                    target_id,
                    command.int_value,
                    command.cmd_source,
                );
            }
        }
        drop(guard);
        let clearing_mines = self.is_clearing_mines();
        if let Some(worker_ai) = self.runtime.components.worker_ai.as_mut() {
            if clearing_mines {
                worker_ai.drop_all_boxes_if_carrying();
            }
        }
    }

    pub(super) fn execute_command(
        &mut self,
        command: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(machine) = self.ai_state_machine.as_mut() {
            let mut runtime = super::UnitAiStateRuntime::new(&mut self.runtime, true);
            return runtime.execute_command_native(command, &mut machine.driver());
        }
        self.execute_standalone_command(command)
    }

    pub(super) fn execute_command_with_world(
        &mut self,
        command: &crate::ai::AiCommandParams,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.execute_owned_command_with_ignore(command, world, None)
    }
    pub(super) fn execute_owned_command_with_ignore(
        &mut self,
        command: &crate::ai::AiCommandParams,
        world: &mut dyn crate::modules::ai_world::AiWorld,
        ignored: Option<ObjectID>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.validate_owned_command(command, world)?;
        if world.pathfinder().is_none() {
            return Err("admitted pathfinder disappeared".into());
        }
        if let Some(ignored) = ignored {
            self.runtime.ignore_obstacle_id(ignored)?;
        }
        let machine = self
            .ai_state_machine
            .as_mut()
            .ok_or("installed AI has no state machine")?;
        let mut runtime = super::UnitAiStateRuntime {
            runtime: &mut self.runtime,
            parent_exists: true,
            parent_state_id: None,
            world: Some(world),
        };
        runtime.execute_command_native(command, &mut machine.driver())
    }

    pub(super) fn validate_owned_command(
        &self,
        command: &crate::ai::AiCommandParams,
        world: &dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.validate_world_owner(world)?;
        let idle = command.cmd == crate::ai::AiCommandType::Idle;
        let waypoint = matches!(
            command.cmd,
            crate::ai::AiCommandType::FollowWaypointPath
                | crate::ai::AiCommandType::FollowWaypointPathExact
                | crate::ai::AiCommandType::FollowWaypointPathAsTeam
                | crate::ai::AiCommandType::FollowWaypointPathAsTeamExact
        );
        if waypoint {
            let id = command
                .waypoint
                .ok_or("owned waypoint command is missing its initial waypoint")?;
            let first = world
                .waypoint_by_id(id)
                .ok_or("owned waypoint command references no waypoint in this world")?;
            if !first.position.x.is_finite()
                || !first.position.y.is_finite()
                || !first.position.z.is_finite()
            {
                return Err("owned waypoint command has non-finite coordinates".into());
            }
            world.waypoint_pathfind_extent()?;
            world.waypoint_is_on_wall(&first.position)?;
            world.waypoint_wall_height()?;
            if (self.runtime.get_mood_matrix_action_adjustment_with_world(
                true,
                crate::ai::MoodMatrixAction::Move,
                world,
            ) & crate::ai::mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE)
                != 0
            {
                return Err(
                    "owned waypoint command does not yet admit attack-move mood escalation".into(),
                );
            }
        }
        let exits = matches!(
            command.cmd,
            crate::ai::AiCommandType::Exit | crate::ai::AiCommandType::ExitInstantly
        );
        if exits && !world.drives_owned_exit() {
            return Err("owned exit requires a synchronous handle driver".into());
        }
        if command.cmd == crate::ai::AiCommandType::MoveToPosition
            && command.cmd_source != crate::ai::CommandSourceType::FromAi
        {
            return Err("supplied movement currently admits AI-origin removal motion only".into());
        }
        self.validate_owned_components(
            command.cmd == crate::ai::AiCommandType::MoveToPosition || idle || exits || waypoint,
            idle,
        )?;
        // The supplied endpoint owns Enter and its immediate terminal entries.
        // An arbitrary old state could execute an ambient specialized onExit.
        let current = self
            .ai_state_machine
            .as_ref()
            .ok_or("installed AI has no state machine")?;
        let initializes_idle = current.get_current_state_id().is_none()
            && command.cmd == crate::ai::AiCommandType::Idle;
        if current.get_temporary_state().is_some()
            || (!initializes_idle
                && !matches!(current.get_current_state_id(),
                Some(id) if id == AIStateType::Idle as u32
                    || id == AIStateType::Enter as u32
                    || id == AIStateType::Busy as u32
                    || (idle && id == AIStateType::MoveTo as u32)
                    || id == AIStateType::Exit as u32 || id == AIStateType::ExitInstantly as u32
                    || id == AIStateType::FollowWaypointPathAsIndividuals as u32
                    || id == AIStateType::FollowWaypointPathAsIndividualsExact as u32
                    || id == AIStateType::FollowWaypointPathAsTeam as u32
                    || id == AIStateType::FollowWaypointPathAsTeamExact as u32))
        {
            return Err(
                "supplied AIEnter does not admit this previous state or temporary state".into(),
            );
        }
        if !matches!(
            command.cmd,
            crate::ai::AiCommandType::Enter
                | crate::ai::AiCommandType::Idle
                | crate::ai::AiCommandType::AttackObject
                | crate::ai::AiCommandType::Busy
                | crate::ai::AiCommandType::MoveToPosition
                | crate::ai::AiCommandType::FollowWaypointPath
                | crate::ai::AiCommandType::FollowWaypointPathExact
                | crate::ai::AiCommandType::FollowWaypointPathAsTeam
                | crate::ai::AiCommandType::FollowWaypointPathAsTeamExact
                | crate::ai::AiCommandType::Exit
                | crate::ai::AiCommandType::ExitInstantly
        ) {
            return Err(
                "supplied AI operation supports the standard AIEnter transition family only".into(),
            );
        }
        if !world.has_pathfinder() {
            return Err("supplied AIEnter requires its owner's pathfinder".into());
        }
        let owner = self
            .runtime
            .native_owner()
            .ok_or("installed owner missing")?;
        let owner = owner
            .try_read()
            .map_err(|_| "installed owner unavailable for native command")?;
        world.validate_enter_operation(&owner, command.obj, command.cmd, command.cmd_source)?;
        drop(owner);
        Ok(())
    }

    pub(super) fn validate_owned_enter_components(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.validate_owned_components(false, false)
    }
    pub(super) fn validate_owned_components(
        &self,
        allow_contained: bool,
        allow_dozer_idle: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let owner = self
            .runtime
            .native_owner()
            .ok_or("installed AI owner missing")?;
        let owner = owner
            .try_read()
            .map_err(|_| "installed AI owner unavailable")?;
        if owner.get_formation_id() != crate::common::FormationID::NONE
            && owner.get_group_id().is_some()
        {
            return Err("supplied AIEnter does not admit formation group movement".into());
        }
        if let Some(body) = owner.get_body_module() {
            drop(
                body.try_lock()
                    .map_err(|_| "installed AI body unavailable")?,
            );
        }
        if let Some(physics) = owner.get_physics() {
            drop(
                physics
                    .try_access()
                    .map_err(|_| "installed AI physics unavailable")?,
            );
        }
        // Eligibility recursively inspects firing passengers and enclosing
        // garrison fire points. Those native callbacks are not yet admitted by
        // this bounded infantry-enter endpoint, including their slave effects.
        if owner.get_contain().is_some() || (!allow_contained && owner.get_contained_by().is_some())
        {
            return Err("supplied AIEnter does not admit a containing or contained source".into());
        }
        if owner.has_flight_deck_module() {
            return Err("supplied native command does not admit flight deck forwarding".into());
        }
        if owner.get_spawn_behavior_interface_public().is_some() {
            return Err("supplied AIEnter does not admit slave forwarding callbacks".into());
        }
        drop(owner);
        if self.runtime.data.turret_primary_machine.is_some()
            || self.runtime.data.turret_secondary_machine.is_some()
        {
            return Err("supplied AIEnter does not admit autonomous turret updates".into());
        }
        let c = &self.runtime.components;
        if c.supply_truck_ai.is_some()
            || c.chinook_ai.is_some()
            || c.jet_ai.is_some()
            || c.worker_ai.is_some()
            || (c.dozer_ai.is_some() && !allow_dozer_idle)
            || c.railed_transport_ai.is_some()
            || c.hack_internet_ai.is_some()
            || c.assault_transport_ai.is_some()
            || c.deliver_payload_ai.is_some()
            || c.transport_ai.is_some()
            || c.deploy_style_ai.is_some()
            || c.wander_ai.is_some()
        {
            return Err("supplied AIEnter does not admit specialized AI components".into());
        }
        #[cfg(feature = "allow_surrender")]
        if c.pow_truck_ai.is_some() {
            return Err("supplied AIEnter does not admit POW AI".into());
        }
        Ok(())
    }

    pub(super) fn validate_world_owner(
        &self,
        world: &dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let owner = self
            .runtime
            .owner
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
            .ok_or("installed AI owner is missing")?;
        let admitted = world
            .object(self.runtime.unit_id)
            .ok_or("AI owner absent from supplied world")?;
        if !Arc::ptr_eq(&owner, &admitted) {
            return Err("supplied world does not own this installed AI".into());
        }
        Ok(())
    }

    fn execute_standalone_command(
        &mut self,
        command: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.runtime.data.forbid_player_commands
            && command.cmd_source == crate::ai::CommandSourceType::FromPlayer
        {
            return Ok(());
        }

        if let Some(deliver_ai) = self.runtime.components.deliver_payload_ai.as_ref() {
            if !deliver_ai.is_allowed_to_respond_to_ai_commands() {
                return Ok(());
            }
        }

        if self.runtime.components.railed_transport_ai.is_some()
            && command.cmd_source == crate::ai::CommandSourceType::FromPlayer
            && !matches!(
                command.cmd,
                crate::ai::AiCommandType::ExecuteRailedTransport
                    | crate::ai::AiCommandType::Evacuate
            )
        {
            return Ok(());
        }

        if let Some(mut assault_ai) = self.runtime.components.assault_transport_ai.take() {
            assault_ai.handle_command(command);
            self.runtime.components.assault_transport_ai = Some(assault_ai);
        }

        if let Some(mut hack_ai) = self.runtime.components.hack_internet_ai.take() {
            if hack_ai.handle_command(command, self) {
                self.runtime.components.hack_internet_ai = Some(hack_ai);
                return Ok(());
            }
            self.runtime.components.hack_internet_ai = Some(hack_ai);
        }

        if let Some(mut chinook_ai) = self.runtime.components.chinook_ai.take() {
            if chinook_ai.handle_command(command, self) {
                self.runtime.components.chinook_ai = Some(chinook_ai);
                return Ok(());
            }
            self.runtime.components.chinook_ai = Some(chinook_ai);
        }

        if let Some(jet_ai) = self.runtime.components.jet_ai.take() {
            // Jet command handling transitions the parent state machine. A
            // standalone UnitAIUpdate has no admitted parent machine/driver,
            // so fail explicitly instead of synthesizing one or skipping the
            // Jet pre-handler. Production object admission always installs it.
            self.runtime.components.jet_ai = Some(jet_ai);
            return Err(
                "standalone Jet command dispatch requires a parent AI state machine".into(),
            );
        }

        if let Some(jet_ai) = self.runtime.components.jet_ai.as_mut() {
            if jet_ai.suppress_command_store() {
                jet_ai.set_suppress_command_store(false);
            } else {
                jet_ai.store_most_recent_command(command);
            }
        }

        // C++ POWTruckAIUpdate::aiDoCommand: any CMD_FROM_PLAYER first
        // aiIdle(CMD_FROM_AI) + setTask(WAITING), then the new command.
        #[cfg(feature = "allow_surrender")]
        if command.cmd_source == crate::ai::CommandSourceType::FromPlayer {
            if let Some(pow_ai) = self.runtime.components.pow_truck_ai.as_mut() {
                pow_ai.on_player_command();
            }
        }

        self.runtime.data.last_command_source = command.cmd_source;
        self.runtime.data.current_command = Some(command.cmd);
        if self.runtime.components.jet_ai.is_some() {
            self.runtime.data.pending_command = Some(command.cmd);
        } else {
            self.runtime.data.pending_command = None;
        }
        if let Some(supply_ai) = self.runtime.components.supply_truck_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                supply_ai.private_idle(command.cmd_source);
            }
        }
        if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                chinook_ai.private_idle(command.cmd_source);
            }
        }
        if let Some(worker_ai) = self.runtime.components.worker_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                worker_ai.private_idle(command.cmd_source);
            }
        }
        if command.cmd != crate::ai::AiCommandType::Enter {
            self.runtime.data.enter_target = None;
        }
        if command.cmd == crate::ai::AiCommandType::RappelInto {
            // C++ AIUpdate.cpp:2981-2997 / AIStates.cpp:481-514: Rappel
            // operates on the admitted Object, not a second Unit registration.
            let owner = self
                .rappel_owner()
                .ok_or("rappel owner no longer available")?;
            {
                let owner = owner.read().map_err(|_| "rappel owner lock poisoned")?;
                owner.forward_command_to_flight_deck(command);
            }
            let _ = self.start_rappel_state(&owner, command.obj);
            self.finish_command_worker_side_effects();
            return Ok(());
        }
        // Native commands retain the exact factory-admitted owner. A stale
        // native owner is an error, never permission to select a same-ID Unit.
        let native_owner = match self.runtime.owner.as_ref() {
            Some(owner) => Some(
                owner
                    .upgrade()
                    .ok_or("native AI owner no longer available")?,
            ),
            None => None,
        };
        let legacy_unit = if native_owner.is_none() {
            Some(get_unit_arc(self.runtime.unit_id).ok_or("unit no longer available")?)
        } else {
            None
        };
        let mut guard = match native_owner {
            Some(owner) => CommandOwner::Object(owner),
            None => CommandOwner::LegacyUnit(
                legacy_unit
                    .as_ref()
                    .expect("legacy owner resolved")
                    .write()
                    .map_err(|_| "unit lock poisoned")?,
            ),
        };
        guard.forward_command_to_flight_deck(command)?;

        match command.cmd {
            crate::ai::AiCommandType::Repair => {
                if let Some(target_id) = command.obj {
                    if let Some(worker_ai) = self.runtime.components.worker_ai.as_mut() {
                        worker_ai.set_repair_target(target_id, command.cmd_source);
                    } else if let Some(dozer_ai) = self.runtime.components.dozer_ai.as_mut() {
                        dozer_ai.set_repair_target(target_id, command.cmd_source);
                    }
                }
            }
            crate::ai::AiCommandType::ResumeConstruction => {
                if let Some(target_id) = command.obj {
                    if let Some(worker_ai) = self.runtime.components.worker_ai.as_mut() {
                        worker_ai.set_resume_construction_target(target_id, command.cmd_source);
                    } else if let Some(dozer_ai) = self.runtime.components.dozer_ai.as_mut() {
                        dozer_ai.set_resume_construction_target(target_id, command.cmd_source);
                    }
                }
            }
            crate::ai::AiCommandType::MoveToPosition
            | crate::ai::AiCommandType::MoveToPositionEvenIfSleeping
            | crate::ai::AiCommandType::MoveToPositionAndEvacuate
            | crate::ai::AiCommandType::MoveToPositionAndEvacuateAndExit => {
                let clipped =
                    self.clip_goal_position(&guard.base_arc(), command.pos, command.cmd_source);

                guard
                    .legacy()?
                    .give_move_order(clipped, Vec::new(), false, false)?;
            }
            crate::ai::AiCommandType::TightenToPosition => {
                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                if !is_mobile {
                    return Ok(());
                }
                let clipped =
                    self.clip_goal_position(&guard.base_arc(), command.pos, command.cmd_source);

                guard
                    .legacy()?
                    .give_move_order(clipped, Vec::new(), false, false)?;
            }
            crate::ai::AiCommandType::MoveToObject => {
                if let Some(target_id) = command.obj {
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            guard.legacy()?.give_move_order(
                                *target_guard.get_position(),
                                Vec::new(),
                                false,
                                false,
                            )?;
                        }
                    }
                }
            }
            crate::ai::AiCommandType::MoveAwayFromUnit => {
                if !self.is_allowed_to_move_away_from_unit() {
                    return Ok(());
                }
                if self.is_ai_in_dead_state() {
                    return Ok(());
                }
                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                if !is_mobile {
                    return Ok(());
                }
                if let Some(target_id) = command.obj {
                    if (target_id == self.runtime.data.move_out_of_way_1
                        || target_id == self.runtime.data.move_out_of_way_2)
                        && self.is_blocked_and_stuck()
                    {
                        self.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
                        return Ok(());
                    }
                    self.runtime.data.move_out_of_way_2 = self.runtime.data.move_out_of_way_1;
                    self.runtime.data.move_out_of_way_1 = target_id;
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            let my_pos = guard.get_position()?;
                            let other_pos = target_guard.get_position();
                            let mut dir =
                                Coord3D::new(my_pos.x - other_pos.x, my_pos.y - other_pos.y, 0.0);
                            let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
                            if len > 0.001 {
                                dir.x /= len;
                                dir.y /= len;
                            } else {
                                dir.x = 1.0;
                                dir.y = 0.0;
                            }
                            let mut desired = my_pos;
                            desired.x += dir.x * (PATHFIND_CELL_SIZE_F * 2.0);
                            desired.y += dir.y * (PATHFIND_CELL_SIZE_F * 2.0);
                            let clipped = self.clip_goal_position(
                                &guard.base_arc(),
                                desired,
                                command.cmd_source,
                            );

                            guard
                                .legacy()?
                                .give_move_order(clipped, Vec::new(), false, false)?;
                        }
                    }
                }
            }
            crate::ai::AiCommandType::FollowPath
            | crate::ai::AiCommandType::FollowExitProductionPath
            | crate::ai::AiCommandType::FollowUserPath => {
                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                if !is_mobile {
                    return Ok(());
                }

                if command.coords.is_empty() {
                    return Ok(());
                }
                let mut coords = command.coords.clone();
                let first = coords.remove(0);
                let waypoints = coords
                    .iter()
                    .map(|pos| Waypoint::new(INVALID_ID, *pos, String::new()))
                    .collect::<Vec<_>>();
                guard
                    .legacy()?
                    .give_move_order(first, waypoints, false, false)?;
            }
            crate::ai::AiCommandType::FollowPathAppend => {
                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                if !is_mobile {
                    return Ok(());
                }

                let effectively_moving = (!self.is_idle_in_legacy_unit(guard.legacy()?)
                    && (self.runtime.data.locomotor_goal_type != 0
                        || self.runtime.data.cpp_is_moving))
                    || self.is_waiting_for_path();
                if effectively_moving {
                    let mut coords = Vec::new();
                    let legacy = guard.legacy()?;
                    if let Some(goal) = legacy.target_position.or_else(|| {
                        legacy
                            .path_following_state
                            .as_ref()
                            .map(|s| s.goal_position)
                    }) {
                        coords.push(goal);
                    }
                    coords.push(command.pos);
                    let first = coords.remove(0);
                    let waypoints = coords
                        .iter()
                        .map(|pos| Waypoint::new(INVALID_ID, *pos, String::new()))
                        .collect::<Vec<_>>();
                    guard
                        .legacy()?
                        .give_move_order(first, waypoints, false, false)?;
                } else {
                    guard
                        .legacy()?
                        .give_move_order(command.pos, Vec::new(), false, false)?;
                }
            }
            crate::ai::AiCommandType::AttackMoveToPosition => {
                let clipped =
                    self.clip_goal_position(&guard.base_arc(), command.pos, command.cmd_source);

                guard.legacy()?.process_attack_move_order(clipped, true)?;
                if let Ok(mut obj_guard) = guard.base_arc().write() {
                    obj_guard.set_current_weapon_max_shot_count(command.int_value);
                }
            }
            crate::ai::AiCommandType::AttackPosition => {
                let base_object = guard.base_arc().clone();
                let mut local_pos =
                    self.clip_goal_position(&guard.base_arc(), command.pos, command.cmd_source);
                let mut max_shots = command.int_value;
                let continue_range = base_object
                    .read()
                    .ok()
                    .and_then(|obj_guard| {
                        obj_guard
                            .get_current_weapon()
                            .map(|(weapon, _)| weapon.get_lock_on_range())
                    })
                    .unwrap_or(0.0);

                if continue_range > 0.0 {
                    if let Ok(mut obj_guard) = base_object.write() {
                        obj_guard.set_status(ObjectStatusMaskType::IGNORING_STEALTH, true);
                    }

                    let target_id =
                        crate::helpers::ThePartitionManager::get().and_then(|partition| {
                            let obj_guard = base_object.read().ok()?;
                            partition.get_closest_object(
                                &command.pos,
                                continue_range,
                                |candidate| {
                                    matches!(
                                        ActionManager::get_can_attack_object(
                                            &*obj_guard,
                                            candidate,
                                            command.cmd_source,
                                            crate::attack::AbleToAttackType::NewTarget
                                        ),
                                        CanAttackResult::Possible
                                            | CanAttackResult::PossibleAfterMoving
                                    )
                                },
                            )
                        });

                    if let Ok(mut obj_guard) = base_object.write() {
                        obj_guard.set_status(ObjectStatusMaskType::IGNORING_STEALTH, false);
                    }

                    if let Some(target_id) = target_id {
                        guard.legacy()?.give_attack_order(target_id, true, false)?;
                        if let Ok(mut obj_guard) = guard.base_arc().write() {
                            obj_guard.set_current_weapon_max_shot_count(max_shots);
                        }
                        if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_ref() {
                            chinook_ai.private_attack_object(
                                target_id,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        if let Some(transport_ai) = self.runtime.components.transport_ai.as_ref() {
                            transport_ai.private_attack_object(
                                target_id,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        return Ok(());
                    }
                    max_shots = 1;
                }

                let weapon_is_contact = base_object
                    .read()
                    .ok()
                    .map(|obj_guard| {
                        obj_guard
                            .get_current_weapon()
                            .map(|(weapon, _)| weapon.is_contact_weapon())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if weapon_is_contact {
                    let mut path_available = true;
                    if let Some(capabilities) = self
                        .runtime
                        .data
                        .locomotor_set
                        .get_active()
                        .map(|loco| loco.to_movement_capabilities())
                    {
                        let ai_store = the_ai();
                        if let Ok(ai_guard) = ai_store.read() {
                            if let Some(system) = ai_guard.pathfinding_system() {
                                if let Ok(mut system_guard) = system.write() {
                                    let unit_radius = base_object
                                        .read()
                                        .ok()
                                        .map(|obj_guard| {
                                            obj_guard.get_geometry_info().get_major_radius()
                                        })
                                        .unwrap_or(0.0);
                                    let request = crate::ai::pathfinding_system::PathRequest {
                                        requester: guard.get_id()?,
                                        start: guard.get_position()?,
                                        goal: local_pos,
                                        capabilities,
                                        unit_size: unit_radius,
                                        priority: 0,
                                        allow_partial: false,
                                        frame_requested: TheGameLogic::get_frame(),
                                        move_allies: self.runtime.data.can_path_through_units,
                                        ignore_obstacle_id: if self.runtime.data.ignore_obstacle_id
                                            == INVALID_ID
                                        {
                                            None
                                        } else {
                                            Some(self.runtime.data.ignore_obstacle_id)
                                        },
                                    };
                                    path_available = matches!(
                                        system_guard.find_path_immediate(&request),
                                        crate::ai::pathfinding_system::PathResult::Success(_)
                                    );
                                }
                            }
                        }
                    }
                    if !path_available {
                        if let Some(partition) = ThePartitionManager::get() {
                            let mut options = FindPositionOptions::default();
                            options.min_radius = 0.0;
                            options.max_radius = 100.0;
                            options.source_to_path_to_dest_id = Some(guard.get_id()?);
                            let mut adjusted = local_pos;
                            if partition.find_position_around_with_options(
                                &local_pos,
                                &options,
                                &mut adjusted,
                            ) {
                                local_pos = adjusted;
                            }
                        }
                    }
                }

                guard.legacy()?.process_attack_move_order(local_pos, true)?;
                if let Ok(mut obj_guard) = guard.base_arc().write() {
                    obj_guard.set_current_weapon_max_shot_count(max_shots);
                }
                if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_ref() {
                    chinook_ai.private_attack_position(&local_pos, max_shots, command.cmd_source);
                }
                if let Some(transport_ai) = self.runtime.components.transport_ai.as_ref() {
                    transport_ai.private_attack_position(&local_pos, max_shots, command.cmd_source);
                }
            }
            crate::ai::AiCommandType::AttackObject
            | crate::ai::AiCommandType::ForceAttackObject => {
                if let Some(target_id) = command.obj {
                    if self.runtime.components.chinook_ai.is_some() {
                        let can_attack = guard
                            .base_arc()
                            .read()
                            .ok()
                            .is_some_and(|obj| obj.is_kind_of(KindOf::CanAttack));
                        if !can_attack {
                            return Ok(());
                        }
                    }

                    guard.legacy()?.give_attack_order(target_id, true, false)?;
                    if let Ok(mut obj_guard) = guard.base_arc().write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                    if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_ref() {
                        if command.cmd == crate::ai::AiCommandType::ForceAttackObject {
                            chinook_ai.private_force_attack_object(
                                target_id,
                                command.int_value,
                                command.cmd_source,
                            );
                        } else {
                            chinook_ai.private_attack_object(
                                target_id,
                                command.int_value,
                                command.cmd_source,
                            );
                        }
                    }
                    if let Some(transport_ai) = self.runtime.components.transport_ai.as_ref() {
                        if command.cmd == crate::ai::AiCommandType::ForceAttackObject {
                            transport_ai.private_force_attack_object(
                                target_id,
                                command.int_value,
                                command.cmd_source,
                            );
                        } else {
                            transport_ai.private_attack_object(
                                target_id,
                                command.int_value,
                                command.cmd_source,
                            );
                        }
                    }
                }
            }
            crate::ai::AiCommandType::AttackTeam => {
                if let Some(team_name) = command.team.as_ref() {
                    if let Ok(mut factory) = crate::team::get_team_factory().lock() {
                        if let Some(team) = factory.find_team(team_name) {
                            if let Ok(team_guard) = team.read() {
                                let target_id = if team_guard.get_team_target_object() != INVALID_ID
                                {
                                    team_guard.get_team_target_object()
                                } else {
                                    team_guard
                                        .get_members()
                                        .first()
                                        .copied()
                                        .unwrap_or(INVALID_ID)
                                };
                                if target_id != INVALID_ID {
                                    guard.legacy()?.give_attack_order(target_id, true, false)?;
                                    if let Ok(mut obj_guard) = guard.base_arc().write() {
                                        obj_guard
                                            .set_current_weapon_max_shot_count(command.int_value);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::GuardPosition => {
                guard.legacy()?.current_order = Some(UnitOrder::Guard {
                    position: command.pos,
                    area_radius: guard.legacy()?.engagement_range,
                });
                guard.legacy()?.order_queue.clear();
            }
            crate::ai::AiCommandType::GuardObject => {
                if let Some(target_id) = command.obj {
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            guard.legacy()?.current_order = Some(UnitOrder::Guard {
                                position: *target_guard.get_position(),
                                area_radius: guard.legacy()?.engagement_range,
                            });
                            guard.legacy()?.order_queue.clear();
                        }
                    }
                }
            }
            crate::ai::AiCommandType::GuardArea => {
                guard.legacy()?.current_order = Some(UnitOrder::Guard {
                    position: command.pos,
                    area_radius: guard.legacy()?.engagement_range,
                });
                guard.legacy()?.order_queue.clear();
            }
            crate::ai::AiCommandType::GuardTunnelNetwork => {}
            crate::ai::AiCommandType::GuardRetaliate => {
                if let Some(target_id) = command.obj {
                    guard.legacy()?.current_order = Some(UnitOrder::Guard {
                        position: command.pos,
                        area_radius: guard.legacy()?.engagement_range,
                    });
                    guard.legacy()?.order_queue.clear();
                    guard.legacy()?.give_attack_order(target_id, true, false)?;
                    if let Ok(mut obj_guard) = guard.base_arc().write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                }
            }
            crate::ai::AiCommandType::Enter => {
                self.runtime.data.enter_target = command.obj;

                if let Some(container_id) = command.obj {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    let base_arc = guard.base_arc();
                                    let base_lock = base_arc.read();
                                    if let Ok(base_guard) = base_lock {
                                        let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                            &base_guard,
                                            crate::modules::ContainWant::WantsToEnter,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::Exit => {
                let exit_container = command
                    .obj
                    .or_else(|| guard.base_arc().try_read().ok()?.get_contained_by());
                let Some(exit_container) = exit_container else {
                    return Ok(());
                };
                let Some(container) = TheGameLogic::find_object_by_id(exit_container) else {
                    return Ok(());
                };
                if container.try_read().ok().is_some_and(|container| {
                    container
                        .is_disabled_by_type(crate::common::types::DisabledType::DisabledSubdued)
                }) {
                    return Ok(());
                }

                let container_id = command
                    .obj
                    .or_else(|| guard.base_arc().read().ok()?.get_contained_by());
                if let Some(container_id) = container_id {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    let base_arc = guard.base_arc();
                                    let base_lock = base_arc.read();
                                    if let Ok(base_guard) = base_lock {
                                        let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                            &base_guard,
                                            crate::modules::ContainWant::WantsToExit,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::ExitInstantly => {
                let exit_container = command
                    .obj
                    .or_else(|| guard.base_arc().try_read().ok()?.get_contained_by());
                let Some(exit_container) = exit_container else {
                    return Ok(());
                };
                let Some(container) = TheGameLogic::find_object_by_id(exit_container) else {
                    return Ok(());
                };
                if container.try_read().ok().is_some_and(|container| {
                    container
                        .is_disabled_by_type(crate::common::types::DisabledType::DisabledSubdued)
                }) {
                    return Ok(());
                }

                let container_id = command
                    .obj
                    .or_else(|| guard.base_arc().read().ok()?.get_contained_by());
                if let Some(container_id) = container_id {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    let base_arc = guard.base_arc();
                                    let base_lock = base_arc.read();
                                    if let Ok(base_guard) = base_lock {
                                        let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                            &base_guard,
                                            crate::modules::ContainWant::WantsToExit,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::Dock => {
                if let Some(supply_ai) = self.runtime.components.supply_truck_ai.as_mut() {
                    supply_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_mut() {
                    chinook_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(worker_ai) = self.runtime.components.worker_ai.as_mut() {
                    worker_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(mut existing) = self.runtime.dock_machine.take() {
                    let _ = existing.halt();
                }

                if let Some(target_id) = command.obj {
                    let target_arc = TheGameLogic::find_object_by_id(target_id);
                    let Some(target_arc) = target_arc else {
                        return Ok(());
                    };

                    let has_dock = target_arc
                        .read()
                        .ok()
                        .and_then(|guard| guard.with_dock_update_interface(|_| true))
                        .unwrap_or(false);
                    if !has_dock {
                        return Ok(());
                    }

                    if let Some(mut existing) = self.runtime.dock_machine.take() {
                        let _ = existing.halt();
                    }

                    let owner_object = guard.base_arc();
                    drop(guard);
                    let mut dock_machine =
                        AIDockMachine::new(owner_object).map_err(|err| err.to_string())?;
                    let goal_id = target_arc.read().ok().map(|goal| goal.get_id());
                    if let Some(goal_id) = goal_id {
                        let _ = dock_machine.start_with_ai(goal_id, self);
                    }
                    let _ = self.set_can_path_through_units(true);
                    self.runtime.dock_machine = Some(dock_machine);
                    self.finish_command_worker_side_effects();
                    return Ok(());
                }
            }
            crate::ai::AiCommandType::ExecuteRailedTransport => {
                if let Some(mut railed_ai) = self.runtime.components.railed_transport_ai.take() {
                    let _ = railed_ai.handle_execute_railed_transport(command.cmd_source, self);
                    self.runtime.components.railed_transport_ai = Some(railed_ai);
                }
            }
            crate::ai::AiCommandType::HackInternet => {
                if let Some(mut hack_ai) = self.runtime.components.hack_internet_ai.take() {
                    hack_ai.hack_internet();
                    self.runtime.components.hack_internet_ai = Some(hack_ai);
                }
            }
            crate::ai::AiCommandType::Evacuate | crate::ai::AiCommandType::EvacuateInstantly => {
                let instantly = command.cmd == crate::ai::AiCommandType::EvacuateInstantly;
                let subdued = guard.base_arc().try_read().ok().is_some_and(|obj| {
                    obj.is_disabled_by_type(crate::common::types::DisabledType::DisabledSubdued)
                });
                // C++ RailedTransportAIUpdate::privateEvacuate replaces the base
                // evacuate. It does not check subdued and does not order passengers.
                if !subdued && self.runtime.components.railed_transport_ai.is_none() {
                    if let Ok(obj_guard) = guard.base_arc().write() {
                        if let Some(contain) = obj_guard.get_contain() {
                            if let Ok(mut contain_guard) = contain.lock() {
                                if command.int_value != 0 {
                                    contain_guard.mark_all_passengers_detected();
                                }
                                let _ = contain_guard
                                    .order_all_passengers_to_exit(command.cmd_source, instantly);
                            }
                        }
                    }
                }
                if let Some(mut railed_ai) = self.runtime.components.railed_transport_ai.take() {
                    let _ = railed_ai.handle_evacuate(command.int_value, command.cmd_source, self);
                    self.runtime.components.railed_transport_ai = Some(railed_ai);
                }
            }
            crate::ai::AiCommandType::CombatDrop => {
                if let Some(mut chinook_ai) = self.runtime.components.chinook_ai.take() {
                    let mut ai_runtime =
                        crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(self);
                    chinook_ai.private_combat_drop(
                        command.obj,
                        command.pos,
                        command.cmd_source,
                        &mut ai_runtime,
                    );
                    drop(ai_runtime);
                    self.runtime.components.chinook_ai = Some(chinook_ai);
                }
            }
            crate::ai::AiCommandType::GetHealed => {
                if let Some(target_id) = command.obj {
                    let can_heal = guard
                        .base_arc()
                        .read()
                        .ok()
                        .and_then(|base_guard| {
                            let target = get_legacy_object(target_id)?;
                            let target_guard = target.read().ok()?;
                            Some(TheActionManager::can_get_healed_at(
                                &*base_guard,
                                &*target_guard,
                                command.cmd_source,
                            ))
                        })
                        .unwrap_or(false);
                    if !can_heal {
                        return Ok(());
                    }

                    let mut enter_params = command.clone();
                    enter_params.cmd = crate::ai::AiCommandType::Enter;
                    self.runtime.data.enter_target = enter_params.obj;

                    if let Some(container_id) = enter_params.obj {
                        if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                            if let Ok(container_guard) = container.write() {
                                if let Some(contain) = container_guard.get_contain() {
                                    if let Ok(mut contain_guard) = contain.lock() {
                                        let base_arc = guard.base_arc();
                                        let base_lock = base_arc.read();
                                        if let Ok(base_guard) = base_lock {
                                            let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                                &base_guard,
                                                crate::modules::ContainWant::WantsToEnter,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::GetRepaired => {
                if let Some(target_id) = command.obj {
                    if let Some(mut chinook_ai) = self.runtime.components.chinook_ai.take() {
                        let mut ai_runtime =
                            crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(self);
                        chinook_ai.private_get_repaired(
                            target_id,
                            command.cmd_source,
                            &mut ai_runtime,
                        );
                        drop(ai_runtime);
                        self.runtime.components.chinook_ai = Some(chinook_ai);
                        return Ok(());
                    }
                }

                if let Some(target_id) = command.obj {
                    let can_repair = guard
                        .base_arc()
                        .read()
                        .ok()
                        .and_then(|base_guard| {
                            let target = get_legacy_object(target_id)?;
                            let target_guard = target.read().ok()?;
                            Some(TheActionManager::can_get_repaired_at(
                                &*base_guard,
                                &*target_guard,
                                command.cmd_source,
                            ))
                        })
                        .unwrap_or(false);
                    if !can_repair {
                        return Ok(());
                    }

                    let mut dock_params = command.clone();
                    dock_params.cmd = crate::ai::AiCommandType::Dock;
                    if let Some(supply_ai) = self.runtime.components.supply_truck_ai.as_mut() {
                        supply_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_mut() {
                        chinook_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(worker_ai) = self.runtime.components.worker_ai.as_mut() {
                        worker_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(mut existing) = self.runtime.dock_machine.take() {
                        let _ = existing.halt();
                    }

                    if let Some(target_id) = dock_params.obj {
                        let target_arc = TheGameLogic::find_object_by_id(target_id);
                        let Some(target_arc) = target_arc else {
                            return Ok(());
                        };

                        let has_dock = target_arc
                            .read()
                            .ok()
                            .and_then(|guard| guard.with_dock_update_interface(|_| true))
                            .unwrap_or(false);
                        if !has_dock {
                            return Ok(());
                        }

                        if let Some(mut existing) = self.runtime.dock_machine.take() {
                            let _ = existing.halt();
                        }

                        let owner_object = guard.base_arc();
                        drop(guard);
                        let mut dock_machine =
                            AIDockMachine::new(owner_object).map_err(|err| err.to_string())?;
                        let goal_id = target_arc.read().ok().map(|goal| goal.get_id());
                        if let Some(goal_id) = goal_id {
                            let _ = dock_machine.start_with_ai(goal_id, self);
                        }
                        let _ = self.set_can_path_through_units(true);
                        self.runtime.dock_machine = Some(dock_machine);
                        self.finish_command_worker_side_effects();
                        return Ok(());
                    }
                }
            }
            #[cfg(feature = "allow_surrender")]
            crate::ai::AiCommandType::PickUpPrisoner => {
                if let Some(prisoner_id) = command.obj {
                    if self.runtime.components.pow_truck_ai.is_some() {
                        let owner_id = guard.get_id()?;
                        if let Some(mut pow_ai) = self.runtime.components.pow_truck_ai.take() {
                            let _ = pow_ai.handle_pick_up_prisoner(
                                owner_id,
                                prisoner_id,
                                command.cmd_source,
                                self,
                            );
                            self.runtime.components.pow_truck_ai = Some(pow_ai);
                        }
                    }
                }
            }
            #[cfg(feature = "allow_surrender")]
            crate::ai::AiCommandType::ReturnPrisoners => {
                if self.runtime.components.pow_truck_ai.is_some() {
                    let owner_id = guard.get_id()?;
                    if let Some(mut pow_ai) = self.runtime.components.pow_truck_ai.take() {
                        let _ = pow_ai.handle_return_prisoners(
                            owner_id,
                            command.obj,
                            command.cmd_source,
                            self,
                        );
                        self.runtime.components.pow_truck_ai = Some(pow_ai);
                    }
                }
            }
            crate::ai::AiCommandType::Idle => {
                guard.legacy()?.stop_movement();
            }
            crate::ai::AiCommandType::Busy => {}
            crate::ai::AiCommandType::Wander
            | crate::ai::AiCommandType::WanderInPlace
            | crate::ai::AiCommandType::Panic => {
                if self.runtime.data.locomotor_set.get_active().is_none() {
                    return Ok(());
                }
            }
            crate::ai::AiCommandType::Hunt => {
                guard.legacy()?.attack_target = None;
                guard.legacy()?.auto_acquire_enemies = true;
                guard.legacy()?.combat_mode = CombatMode::Aggressive;
                guard.legacy()?.attack_move_active = true;
            }
            crate::ai::AiCommandType::AttackArea => {}
            crate::ai::AiCommandType::FollowWaypointPath
            | crate::ai::AiCommandType::FollowWaypointPathExact
            | crate::ai::AiCommandType::FollowWaypointPathAsTeam
            | crate::ai::AiCommandType::FollowWaypointPathAsTeamExact
            | crate::ai::AiCommandType::AttackFollowWaypointPath
            | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam => {
                if matches!(
                    command.cmd,
                    crate::ai::AiCommandType::AttackFollowWaypointPath
                        | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam
                ) {
                    guard.legacy()?.combat_mode = CombatMode::Aggressive;
                    guard.legacy()?.attack_move_active = true;
                }

                if let Some(start_id) = command.waypoint {
                    let mut chain: Vec<Waypoint> = Vec::new();
                    if let Ok(terrain_guard) = crate::terrain::get_terrain_logic().read() {
                        if let Some(start) = terrain_guard.get_waypoint_by_id(start_id) {
                            // C++ setPathFromWaypoint: count > WAYPOINT_PATH_LIMIT.
                            // Also stop at a branch (num_links > 1) like the prior walk.
                            for node in terrain_guard.walk_link0_chain(start, WAYPOINT_PATH_LIMIT) {
                                chain.push(Waypoint::new(
                                    node.get_id(),
                                    *node.get_location(),
                                    String::new(),
                                ));
                                if node.get_num_links() > 1 {
                                    break;
                                }
                            }
                        }
                    }

                    if let Some(first) = chain.first().cloned() {
                        let mut remaining = chain;
                        remaining.remove(0);
                        guard
                            .legacy()?
                            .give_move_order(first.position, remaining, false, false)?;
                    }
                }

                if matches!(
                    command.cmd,
                    crate::ai::AiCommandType::AttackFollowWaypointPath
                        | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam
                ) {
                    if let Ok(mut obj_guard) = guard.base_arc().write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                }
            }
            _ => {}
        }

        drop(guard);
        self.finish_command_worker_side_effects();
        Ok(())
    }

    fn finish_command_worker_side_effects(&mut self) {
        // C++ WorkerAIUpdate::aiDoCommand (WorkerAIUpdate.cpp:1043-1050).
        let clearing_mines = self.is_clearing_mines();
        if let Some(worker_ai) = self.runtime.components.worker_ai.as_mut() {
            if clearing_mines {
                worker_ai.drop_all_boxes_if_carrying();
            }
        }
    }
}
