//! AIUpdateInterface::execute_command body.

#![allow(unused_imports)]

use super::ai_core::UnitAIUpdate;
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{
    dual_world_registry_unavailable, get_unit_arc, with_unit_mut, with_unit_ref,
};
use super::types::*;

impl UnitAIUpdate {
    pub(super) fn execute_command(
        &mut self,
        command: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.forbid_player_commands
            && command.cmd_source == crate::ai::CommandSourceType::FromPlayer
        {
            return Ok(());
        }

        if let Some(deliver_ai) = self.deliver_payload_ai.as_ref() {
            if !deliver_ai.is_allowed_to_respond_to_ai_commands() {
                return Ok(());
            }
        }

        if self.railed_transport_ai.is_some()
            && command.cmd_source == crate::ai::CommandSourceType::FromPlayer
            && !matches!(
                command.cmd,
                crate::ai::AiCommandType::ExecuteRailedTransport
                    | crate::ai::AiCommandType::Evacuate
            )
        {
            return Ok(());
        }

        if let Some(mut assault_ai) = self.assault_transport_ai.take() {
            assault_ai.handle_command(command);
            self.assault_transport_ai = Some(assault_ai);
        }

        if let Some(mut hack_ai) = self.hack_internet_ai.take() {
            if hack_ai.handle_command(command, self) {
                self.hack_internet_ai = Some(hack_ai);
                return Ok(());
            }
            self.hack_internet_ai = Some(hack_ai);
        }

        if let Some(mut chinook_ai) = self.chinook_ai.take() {
            if chinook_ai.handle_command(command, self) {
                self.chinook_ai = Some(chinook_ai);
                return Ok(());
            }
            self.chinook_ai = Some(chinook_ai);
        }

        if let Some(mut jet_ai) = self.jet_ai.take() {
            if jet_ai.handle_command(command, self) {
                self.jet_ai = Some(jet_ai);
                return Ok(());
            }
            self.jet_ai = Some(jet_ai);
        }

        if let Some(jet_ai) = self.jet_ai.as_mut() {
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
            if let Some(pow_ai) = self.pow_truck_ai.as_mut() {
                pow_ai.on_player_command();
            }
        }

        self.last_command_source = command.cmd_source;
        self.current_command = Some(command.cmd);
        if self.jet_ai.is_some() {
            self.pending_command = Some(command.cmd);
        } else {
            self.pending_command = None;
        }
        if let Some(supply_ai) = self.supply_truck_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                supply_ai.private_idle(command.cmd_source);
            }
        }
        if let Some(chinook_ai) = self.chinook_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                chinook_ai.private_idle(command.cmd_source);
            }
        }
        if let Some(worker_ai) = self.worker_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                worker_ai.private_idle(command.cmd_source);
            }
        }
        if command.cmd != crate::ai::AiCommandType::Enter {
            self.enter_target = None;
        }
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;

        guard.forward_command_to_flight_deck(command);

        match command.cmd {
            crate::ai::AiCommandType::Repair => {
                if let Some(target_id) = command.obj {
                    if let Some(worker_ai) = self.worker_ai.as_mut() {
                        worker_ai.set_repair_target(target_id, command.cmd_source);
                    } else if let Some(dozer_ai) = self.dozer_ai.as_mut() {
                        dozer_ai.set_repair_target(target_id, command.cmd_source);
                    }
                }
            }
            crate::ai::AiCommandType::ResumeConstruction => {
                if let Some(target_id) = command.obj {
                    if let Some(worker_ai) = self.worker_ai.as_mut() {
                        worker_ai.set_resume_construction_target(target_id, command.cmd_source);
                    } else if let Some(dozer_ai) = self.dozer_ai.as_mut() {
                        dozer_ai.set_resume_construction_target(target_id, command.cmd_source);
                    }
                }
            }
            crate::ai::AiCommandType::MoveToPosition
            | crate::ai::AiCommandType::MoveToPositionEvenIfSleeping
            | crate::ai::AiCommandType::MoveToPositionAndEvacuate
            | crate::ai::AiCommandType::MoveToPositionAndEvacuateAndExit => {
                let clipped = self.clip_goal_position(&guard, command.pos, command.cmd_source);
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        if command.cmd_source == CommandSourceType::FromAi && !self.is_idle() {
                            machine.set_goal_position(clipped);
                            let _ = machine.set_temporary_state(
                                AIStateType::MoveTo as u32,
                                LOGICFRAMES_PER_SECOND * 20,
                            );
                        } else {
                            let mut params = command.clone();
                            params.pos = clipped;
                            machine.clear();
                            let _ = machine.ai_do_command(&params);
                        }
                        return Ok(());
                    }
                }

                guard.give_move_order(clipped, Vec::new(), false, false)?;
            }
            crate::ai::AiCommandType::TightenToPosition => {
                let is_mobile = guard.current_locomotor.is_some();
                if !is_mobile {
                    return Ok(());
                }
                let clipped = self.clip_goal_position(&guard, command.pos, command.cmd_source);
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let mut params = command.clone();
                        params.pos = clipped;
                        machine.clear();
                        let _ = machine.ai_do_command(&params);
                        return Ok(());
                    }
                }

                guard.give_move_order(clipped, Vec::new(), false, false)?;
            }
            crate::ai::AiCommandType::RappelInto => {
                let _ = self.start_rappel_state(command.obj);
            }
            crate::ai::AiCommandType::MoveToObject => {
                if let Some(target_id) = command.obj {
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            guard.give_move_order(
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
                let is_mobile = guard.current_locomotor.is_some();
                if !is_mobile {
                    return Ok(());
                }
                if let Some(target_id) = command.obj {
                    if (target_id == self.move_out_of_way_1 || target_id == self.move_out_of_way_2)
                        && self.is_blocked_and_stuck()
                    {
                        self.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
                        return Ok(());
                    }
                    self.move_out_of_way_2 = self.move_out_of_way_1;
                    self.move_out_of_way_1 = target_id;
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            let my_pos = guard.get_position();
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
                            let clipped =
                                self.clip_goal_position(&guard, desired, command.cmd_source);

                            if let Some(state_machine) = self.ai_state_machine.as_ref() {
                                if let Ok(mut machine) = state_machine.lock() {
                                    machine.set_goal_position(clipped);
                                    let _ = machine.set_temporary_state(
                                        AIStateType::MoveOutOfTheWay as u32,
                                        LOGICFRAMES_PER_SECOND * 10,
                                    );
                                    return Ok(());
                                }
                            }

                            guard.give_move_order(clipped, Vec::new(), false, false)?;
                        }
                    }
                }
            }
            crate::ai::AiCommandType::FollowPath
            | crate::ai::AiCommandType::FollowExitProductionPath
            | crate::ai::AiCommandType::FollowUserPath => {
                let is_mobile = guard.current_locomotor.is_some();
                if !is_mobile {
                    return Ok(());
                }
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
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
                guard.give_move_order(first, waypoints, false, false)?;
            }
            crate::ai::AiCommandType::FollowPathAppend => {
                let is_mobile = guard.current_locomotor.is_some();
                if !is_mobile {
                    return Ok(());
                }
                let effectively_moving = !self.is_idle() || self.is_waiting_for_path();
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_follow_path = matches!(
                            machine.get_current_state_id(),
                            Some(id) if id == AIStateType::FollowPath as u32
                        );
                        if is_follow_path && machine.get_goal_path_size() > 0 && effectively_moving
                        {
                            let _ = machine.ai_do_command(command);
                            return Ok(());
                        }
                        if effectively_moving {
                            if let Some(goal) = machine.get_goal_position() {
                                let mut params = command.clone();
                                params.cmd = crate::ai::AiCommandType::FollowPath;
                                params.coords = vec![goal, command.pos];
                                machine.clear();
                                let _ = machine.ai_do_command(&params);
                            }
                            return Ok(());
                        }
                        let mut params = command.clone();
                        params.cmd = crate::ai::AiCommandType::FollowPath;
                        params.coords = vec![command.pos];
                        machine.clear();
                        let _ = machine.ai_do_command(&params);
                        return Ok(());
                    }
                }

                if effectively_moving {
                    let mut coords = Vec::new();
                    if let Some(goal) = guard
                        .target_position
                        .or_else(|| guard.path_following_state.as_ref().map(|s| s.goal_position))
                    {
                        coords.push(goal);
                    }
                    coords.push(command.pos);
                    let first = coords.remove(0);
                    let waypoints = coords
                        .iter()
                        .map(|pos| Waypoint::new(INVALID_ID, *pos, String::new()))
                        .collect::<Vec<_>>();
                    guard.give_move_order(first, waypoints, false, false)?;
                } else {
                    guard.give_move_order(command.pos, Vec::new(), false, false)?;
                }
            }
            crate::ai::AiCommandType::AttackMoveToPosition => {
                let clipped = self.clip_goal_position(&guard, command.pos, command.cmd_source);
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let mut params = command.clone();
                        params.pos = clipped;
                        machine.clear();
                        let _ = machine.ai_do_command(&params);
                        if let Ok(mut obj_guard) = guard.base_arc().write() {
                            obj_guard.set_current_weapon_max_shot_count(command.int_value);
                        }
                        return Ok(());
                    }
                }

                guard.process_attack_move_order(clipped, true)?;
                if let Ok(mut obj_guard) = guard.base_arc().write() {
                    obj_guard.set_current_weapon_max_shot_count(command.int_value);
                }
            }
            crate::ai::AiCommandType::AttackPosition => {
                let base_object = guard.base_arc().clone();
                let mut local_pos =
                    self.clip_goal_position(&guard, command.pos, command.cmd_source);
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
                        if let Some(state_machine) = self.ai_state_machine.as_ref() {
                            if let Ok(mut machine) = state_machine.lock() {
                                let mut attack_params = crate::ai::AiCommandParams::new(
                                    crate::ai::AiCommandType::AttackObject,
                                    command.cmd_source,
                                );
                                attack_params.obj = Some(target_id);
                                attack_params.int_value = max_shots;
                                machine.clear();
                                let _ = machine.ai_do_command(&attack_params);
                                if let Ok(mut obj_guard) = guard.base_arc().write() {
                                    obj_guard.set_current_weapon_max_shot_count(max_shots);
                                }
                                if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                                    chinook_ai.private_attack_object(
                                        target_id,
                                        max_shots,
                                        command.cmd_source,
                                    );
                                }
                                if let Some(transport_ai) = self.transport_ai.as_ref() {
                                    transport_ai.private_attack_object(
                                        target_id,
                                        max_shots,
                                        command.cmd_source,
                                    );
                                }
                                return Ok(());
                            }
                        }

                        guard.give_attack_order(target_id, true, false)?;
                        if let Ok(mut obj_guard) = guard.base_arc().write() {
                            obj_guard.set_current_weapon_max_shot_count(max_shots);
                        }
                        if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                            chinook_ai.private_attack_object(
                                target_id,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        if let Some(transport_ai) = self.transport_ai.as_ref() {
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
                    if let Some(locomotor) = guard.current_locomotor.as_ref().cloned() {
                        if let Ok(loco_guard) = locomotor.lock() {
                            if let Ok(ai_guard) = THE_AI.read() {
                                if let Some(system) = ai_guard.pathfinding_system() {
                                    if let Ok(mut system_guard) = system.write() {
                                        let capabilities = loco_guard.to_movement_capabilities();
                                        let unit_radius = base_object
                                            .read()
                                            .ok()
                                            .map(|obj_guard| {
                                                obj_guard.get_geometry_info().get_major_radius()
                                            })
                                            .unwrap_or(0.0);
                                        let request = crate::ai::pathfinding_system::PathRequest {
                                            requester: guard.get_id(),
                                            start: guard.get_position(),
                                            goal: local_pos,
                                            capabilities,
                                            unit_size: unit_radius,
                                            priority: 0,
                                            allow_partial: false,
                                            frame_requested: TheGameLogic::get_frame(),
                                            move_allies: self.can_path_through_units,
                                            ignore_obstacle_id: if self.ignore_obstacle_id
                                                == INVALID_ID
                                            {
                                                None
                                            } else {
                                                Some(self.ignore_obstacle_id)
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
                    }
                    if !path_available {
                        if let Some(partition) = ThePartitionManager::get() {
                            let mut options = FindPositionOptions::default();
                            options.min_radius = 0.0;
                            options.max_radius = 100.0;
                            options.source_to_path_to_dest_id = Some(guard.get_id());
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

                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let mut params = command.clone();
                        params.pos = local_pos;
                        params.int_value = max_shots;
                        machine.clear();
                        let _ = machine.ai_do_command(&params);
                        if let Ok(mut obj_guard) = guard.base_arc().write() {
                            obj_guard.set_current_weapon_max_shot_count(max_shots);
                        }
                        if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                            chinook_ai.private_attack_position(
                                &local_pos,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        if let Some(transport_ai) = self.transport_ai.as_ref() {
                            transport_ai.private_attack_position(
                                &local_pos,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        return Ok(());
                    }
                }

                guard.process_attack_move_order(local_pos, true)?;
                if let Ok(mut obj_guard) = guard.base_arc().write() {
                    obj_guard.set_current_weapon_max_shot_count(max_shots);
                }
                if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                    chinook_ai.private_attack_position(&local_pos, max_shots, command.cmd_source);
                }
                if let Some(transport_ai) = self.transport_ai.as_ref() {
                    transport_ai.private_attack_position(&local_pos, max_shots, command.cmd_source);
                }
            }
            crate::ai::AiCommandType::AttackObject
            | crate::ai::AiCommandType::ForceAttackObject => {
                if let Some(target_id) = command.obj {
                    if let Some(state_machine) = self.ai_state_machine.as_ref() {
                        if let Ok(mut machine) = state_machine.lock() {
                            machine.clear();
                            let _ = machine.ai_do_command(command);
                            if let Ok(mut obj_guard) = guard.base_arc().write() {
                                obj_guard.set_current_weapon_max_shot_count(command.int_value);
                            }
                            if let Some(chinook_ai) = self.chinook_ai.as_ref() {
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
                            if let Some(transport_ai) = self.transport_ai.as_ref() {
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
                            if let Some(worker_ai) = self.worker_ai.as_mut() {
                                if clearing_mines {
                                    worker_ai.drop_all_boxes_if_carrying();
                                }
                            }
                            return Ok(());
                        }
                    }

                    guard.give_attack_order(target_id, true, false)?;
                    if let Ok(mut obj_guard) = guard.base_arc().write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                    if let Some(chinook_ai) = self.chinook_ai.as_ref() {
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
                    if let Some(transport_ai) = self.transport_ai.as_ref() {
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
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        if let Ok(mut obj_guard) = guard.base_arc().write() {
                            obj_guard.set_current_weapon_max_shot_count(command.int_value);
                        }
                        return Ok(());
                    }
                }

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
                                    guard.give_attack_order(target_id, true, false)?;
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
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                guard.current_order = Some(UnitOrder::Guard {
                    position: command.pos,
                    area_radius: guard.engagement_range,
                });
                guard.order_queue.clear();
            }
            crate::ai::AiCommandType::GuardObject => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                if let Some(target_id) = command.obj {
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            guard.current_order = Some(UnitOrder::Guard {
                                position: *target_guard.get_position(),
                                area_radius: guard.engagement_range,
                            });
                            guard.order_queue.clear();
                        }
                    }
                }
            }
            crate::ai::AiCommandType::GuardArea => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
                guard.current_order = Some(UnitOrder::Guard {
                    position: command.pos,
                    area_radius: guard.engagement_range,
                });
                guard.order_queue.clear();
            }
            crate::ai::AiCommandType::GuardTunnelNetwork => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::GuardRetaliate => {
                if let Some(target_id) = command.obj {
                    if let Some(state_machine) = self.ai_state_machine.as_ref() {
                        if let Ok(mut machine) = state_machine.lock() {
                            machine.clear();
                            let _ = machine.ai_do_command(command);
                            if let Ok(mut obj_guard) = guard.base_arc().write() {
                                obj_guard.set_current_weapon_max_shot_count(command.int_value);
                            }
                            return Ok(());
                        }
                    }

                    guard.current_order = Some(UnitOrder::Guard {
                        position: command.pos,
                        area_radius: guard.engagement_range,
                    });
                    guard.order_queue.clear();
                    guard.give_attack_order(target_id, true, false)?;
                    if let Ok(mut obj_guard) = guard.base_arc().write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                }
            }
            crate::ai::AiCommandType::Enter => {
                self.enter_target = command.obj;
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        if command.obj.is_some() {
                            machine.clear();
                            let _ = machine.ai_do_command(command);
                            return Ok(());
                        }
                    }
                }
                if let Some(container_id) = command.obj {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    if let Some(unit) = get_unit_arc(self.unit_id) {
                                        if let Ok(unit_guard) = unit.read() {
                                            let base_arc = unit_guard.base_arc();
                                            drop(unit_guard);
                                            let base_lock = base_arc.read();
                                            if let Ok(base_guard) = base_lock {
                                                let _ = contain_guard
                                                    .on_object_wants_to_enter_or_exit(
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
            }
            crate::ai::AiCommandType::Exit => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                let container_id = command.obj.or_else(|| {
                    get_unit_arc(self.unit_id).and_then(|unit| {
                        let unit_guard = unit.read().ok()?;
                        let base_arc = unit_guard.base_arc();
                        drop(unit_guard);
                        let base_guard = base_arc.read().ok()?;
                        base_guard.get_contained_by()
                    })
                });
                if let Some(container_id) = container_id {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    if let Some(unit) = get_unit_arc(self.unit_id) {
                                        if let Ok(unit_guard) = unit.read() {
                                            let base_arc = unit_guard.base_arc();
                                            drop(unit_guard);
                                            let base_lock = base_arc.read();
                                            if let Ok(base_guard) = base_lock {
                                                let _ = contain_guard
                                                    .on_object_wants_to_enter_or_exit(
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
                }
            }
            crate::ai::AiCommandType::ExitInstantly => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
                let container_id = command.obj.or_else(|| {
                    get_unit_arc(self.unit_id).and_then(|unit| {
                        let unit_guard = unit.read().ok()?;
                        let base_arc = unit_guard.base_arc();
                        drop(unit_guard);
                        let base_guard = base_arc.read().ok()?;
                        base_guard.get_contained_by()
                    })
                });
                if let Some(container_id) = container_id {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    if let Some(unit) = get_unit_arc(self.unit_id) {
                                        if let Ok(unit_guard) = unit.read() {
                                            let base_arc = unit_guard.base_arc();
                                            drop(unit_guard);
                                            let base_lock = base_arc.read();
                                            if let Ok(base_guard) = base_lock {
                                                let _ = contain_guard
                                                    .on_object_wants_to_enter_or_exit(
                                                        &base_guard,
                                                        crate::modules::ContainWant::WantsToExit,
                                                    );
                                                let _ = contain_guard
                                                    .release_object(base_guard.get_id());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::Dock => {
                if let Some(supply_ai) = self.supply_truck_ai.as_mut() {
                    supply_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(chinook_ai) = self.chinook_ai.as_mut() {
                    chinook_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(worker_ai) = self.worker_ai.as_mut() {
                    worker_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(mut existing) = self.dock_machine.take() {
                    let _ = existing.halt();
                }
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        if command.obj.is_some() {
                            machine.clear();
                            let _ = machine.ai_do_command(command);
                            return Ok(());
                        }
                    }
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

                    if let Some(mut existing) = self.dock_machine.take() {
                        let _ = existing.halt();
                    }

                    let owner_object = guard.base_arc();
                    let dock_machine =
                        AIDockMachine::new(owner_object.clone()).map_err(|err| err.to_string())?;
                    if let Ok(mut machine) = dock_machine.state_machine.lock() {
                        machine.set_goal_object_by_id(target_arc.read().ok().map(|g| g.get_id()));
                        let _ = machine.init_default_state();
                    }
                    let _ = self.set_can_path_through_units(true);
                    self.dock_machine = Some(dock_machine);
                }
            }
            crate::ai::AiCommandType::ExecuteRailedTransport => {
                if let Some(mut railed_ai) = self.railed_transport_ai.take() {
                    let _ = railed_ai.handle_execute_railed_transport(command.cmd_source, self);
                    self.railed_transport_ai = Some(railed_ai);
                }
            }
            crate::ai::AiCommandType::HackInternet => {
                if let Some(mut hack_ai) = self.hack_internet_ai.take() {
                    hack_ai.hack_internet();
                    self.hack_internet_ai = Some(hack_ai);
                }
            }
            crate::ai::AiCommandType::Evacuate | crate::ai::AiCommandType::EvacuateInstantly => {
                let instantly = command.cmd == crate::ai::AiCommandType::EvacuateInstantly;
                if let Ok(obj_guard) = guard.base_arc().write() {
                    if let Some(contain) = obj_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            let _ = contain_guard
                                .order_all_passengers_to_exit(command.cmd_source, instantly);
                        }
                    }
                }
                if let Some(mut railed_ai) = self.railed_transport_ai.take() {
                    let _ = railed_ai.handle_evacuate(command.int_value, command.cmd_source, self);
                    self.railed_transport_ai = Some(railed_ai);
                }
            }
            crate::ai::AiCommandType::CombatDrop => {
                if let Some(mut chinook_ai) = self.chinook_ai.take() {
                    chinook_ai.private_combat_drop(
                        command.obj,
                        command.pos,
                        command.cmd_source,
                        self,
                    );
                    self.chinook_ai = Some(chinook_ai);
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
                    self.enter_target = enter_params.obj;
                    if let Some(state_machine) = self.ai_state_machine.as_ref() {
                        if let Ok(mut machine) = state_machine.lock() {
                            let is_mobile = guard.current_locomotor.is_some();
                            if !is_mobile {
                                return Ok(());
                            }
                            if enter_params.obj.is_some() {
                                machine.clear();
                                let _ = machine.ai_do_command(&enter_params);
                                return Ok(());
                            }
                        }
                    }
                    if let Some(container_id) = enter_params.obj {
                        if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                            if let Ok(container_guard) = container.write() {
                                if let Some(contain) = container_guard.get_contain() {
                                    if let Ok(mut contain_guard) = contain.lock() {
                                        if let Some(unit) = get_unit_arc(self.unit_id) {
                                            if let Ok(unit_guard) = unit.read() {
                                                let base_arc = unit_guard.base_arc();
                                                drop(unit_guard);
                                                let base_lock = base_arc.read();
                                                if let Ok(base_guard) = base_lock {
                                                    let _ = contain_guard
                                                        .on_object_wants_to_enter_or_exit(
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
                }
            }
            crate::ai::AiCommandType::GetRepaired => {
                if let Some(target_id) = command.obj {
                    if let Some(mut chinook_ai) = self.chinook_ai.take() {
                        chinook_ai.private_get_repaired(target_id, command.cmd_source, self);
                        self.chinook_ai = Some(chinook_ai);
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
                    if let Some(supply_ai) = self.supply_truck_ai.as_mut() {
                        supply_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(chinook_ai) = self.chinook_ai.as_mut() {
                        chinook_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(worker_ai) = self.worker_ai.as_mut() {
                        worker_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(mut existing) = self.dock_machine.take() {
                        let _ = existing.halt();
                    }
                    if let Some(state_machine) = self.ai_state_machine.as_ref() {
                        if let Ok(mut machine) = state_machine.lock() {
                            let is_mobile = guard.current_locomotor.is_some();
                            if !is_mobile {
                                return Ok(());
                            }
                            if dock_params.obj.is_some() {
                                machine.clear();
                                let _ = machine.ai_do_command(&dock_params);
                                return Ok(());
                            }
                        }
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

                        if let Some(mut existing) = self.dock_machine.take() {
                            let _ = existing.halt();
                        }

                        let owner_object = guard.base_arc();
                        let dock_machine = AIDockMachine::new(owner_object.clone())
                            .map_err(|err| err.to_string())?;
                        if let Ok(mut machine) = dock_machine.state_machine.lock() {
                            machine
                                .set_goal_object_by_id(target_arc.read().ok().map(|g| g.get_id()));
                            let _ = machine.init_default_state();
                        }
                        let _ = self.set_can_path_through_units(true);
                        self.dock_machine = Some(dock_machine);
                    }
                }
            }
            #[cfg(feature = "allow_surrender")]
            crate::ai::AiCommandType::PickUpPrisoner => {
                if let (Some(prisoner_id), Some(mut pow_ai)) =
                    (command.obj, self.pow_truck_ai.take())
                {
                    let owner_id = guard.get_id();
                    let _ = pow_ai.handle_pick_up_prisoner(
                        owner_id,
                        prisoner_id,
                        command.cmd_source,
                        self,
                    );
                    self.pow_truck_ai = Some(pow_ai);
                }
            }
            #[cfg(feature = "allow_surrender")]
            crate::ai::AiCommandType::ReturnPrisoners => {
                if let Some(mut pow_ai) = self.pow_truck_ai.take() {
                    let owner_id = guard.get_id();
                    let _ = pow_ai.handle_return_prisoners(
                        owner_id,
                        command.obj,
                        command.cmd_source,
                        self,
                    );
                    self.pow_truck_ai = Some(pow_ai);
                }
            }
            crate::ai::AiCommandType::Idle => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
                guard.stop_movement();
            }
            crate::ai::AiCommandType::Busy => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::Wander
            | crate::ai::AiCommandType::WanderInPlace
            | crate::ai::AiCommandType::Panic => {
                if guard.current_locomotor.is_none() {
                    return Ok(());
                }
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::Hunt => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                guard.attack_target = None;
                guard.auto_acquire_enemies = true;
                guard.combat_mode = CombatMode::Aggressive;
                guard.attack_move_active = true;
            }
            crate::ai::AiCommandType::AttackArea => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_arc()
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::FollowWaypointPath
            | crate::ai::AiCommandType::FollowWaypointPathExact
            | crate::ai::AiCommandType::FollowWaypointPathAsTeam
            | crate::ai::AiCommandType::FollowWaypointPathAsTeamExact
            | crate::ai::AiCommandType::AttackFollowWaypointPath
            | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        if matches!(
                            command.cmd,
                            crate::ai::AiCommandType::AttackFollowWaypointPath
                                | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam
                        ) {
                            if let Ok(mut obj_guard) = guard.base_arc().write() {
                                obj_guard.set_current_weapon_max_shot_count(command.int_value);
                            }
                        }
                        return Ok(());
                    }
                }

                if matches!(
                    command.cmd,
                    crate::ai::AiCommandType::AttackFollowWaypointPath
                        | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam
                ) {
                    guard.combat_mode = CombatMode::Aggressive;
                    guard.attack_move_active = true;
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
                        guard.give_move_order(first.position, remaining, false, false)?;
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
        // C++ WorkerAIUpdate::aiDoCommand (WorkerAIUpdate.cpp:1043-1050).
        let clearing_mines = self.is_clearing_mines();
        if let Some(worker_ai) = self.worker_ai.as_mut() {
            if clearing_mines {
                worker_ai.drop_all_boxes_if_carrying();
            }
        }

        Ok(())
    }
}
