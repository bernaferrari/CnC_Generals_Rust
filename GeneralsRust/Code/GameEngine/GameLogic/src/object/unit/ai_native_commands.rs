//! AIUpdateInterface::execute_command body.

#![allow(unused_imports)]

use super::UnitAiStateRuntime;
use super::ai_command_owner::CommandOwner;
use super::ai_helpers::*;
use super::identity::Unit;
use super::imports::*;
use super::registry::{
    dual_world_registry_unavailable, get_unit_arc, with_unit_mut, with_unit_ref,
};
use super::types::*;
use crate::ai::states::AIStateMachineDriver;
use crate::modules::ai_state_runtime::AiStateRuntime;

impl UnitAiStateRuntime<'_> {
    // Retained Jet boundary: these callers still use the standalone Unit order
    // until their C++ AIInternalMoveToState ownership is migrated (hq-n8qlq).
    pub(crate) fn move_registered_unit(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = get_unit_arc(self.runtime.unit_id)
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.give_move_order(*pos, Vec::new(), false, false)?;
        Ok(())
    }

    fn is_clearing_mines(&self) -> bool {
        let Some(owner) = self
            .runtime
            .owner
            .as_ref()
            .and_then(std::sync::Weak::upgrade)
        else {
            return false;
        };
        let Ok(owner) = owner.read() else {
            return false;
        };
        owner.test_status(ObjectStatusTypes::OBJECT_STATUS_IS_ATTACKING)
            && owner
                .get_current_weapon()
                .is_some_and(|(weapon, _)| (weapon.get_anti_mask() & WeaponAntiMask::MINE) != 0)
    }

    fn clip_goal_position(
        &self,
        owner: &Arc<RwLock<crate::object::Object>>,
        mut pos: Coord3D,
        cmd_source: CommandSourceType,
    ) -> Coord3D {
        if cmd_source != CommandSourceType::FromPlayer {
            return pos;
        }
        let mut fudge = PATHFIND_CELL_SIZE_F * 0.5;
        if let Ok(object) = owner.read() {
            if object.is_kind_of(KindOf::Aircraft) && object.is_significantly_above_terrain() {
                let preferred = self
                    .runtime
                    .data
                    .locomotor_set
                    .get_active()
                    .map(|loc| loc.preferred_height)
                    .unwrap_or(0.0);
                if preferred > fudge {
                    fudge = preferred;
                }
            }
        }
        if let Ok(terrain_guard) = crate::terrain::get_terrain_logic().read() {
            let extent = terrain_guard.get_maximum_pathfind_extent();
            pos.x = pos.x.clamp(extent.lo.x + fudge, extent.hi.x - fudge);
            pos.y = pos.y.clamp(extent.lo.y + fudge, extent.hi.y - fudge);
        }
        pos
    }

    fn finish_command_worker_side_effects(&mut self) {
        let clearing_mines = self.is_clearing_mines();
        if let Some(worker_ai) = self.runtime.components.worker_ai.as_mut() {
            if clearing_mines {
                worker_ai.drop_all_boxes_if_carrying();
            }
        }
    }

    fn command_button_type_for_owner(
        &self,
        owner: &CommandOwner<'_>,
        button_id: Option<crate::command_button::CommandButtonId>,
    ) -> Option<crate::commands::command::CommandType> {
        let button_id = button_id?;
        let owner_arc = owner.base_arc();
        let owner_guard = owner_arc.read().ok()?;
        if owner_guard.is_any_kind_of(&[KindOf::Projectile]) {
            return None;
        }
        let command_set_name = owner_guard.get_command_set_string();
        let control_bar = crate::control_bar::get_control_bar_bridge()?;
        let command_set = control_bar.find_command_set_by_name(command_set_name)?;
        (0..crate::command_button::MAX_COMMANDS_PER_SET).find_map(|slot| {
            command_set
                .get_command_button(slot)
                .filter(|button| button.get_id() == button_id)
                .map(|button| button.get_command_type())
        })
    }

    fn is_idle_with_driver(&self, driver: &AIStateMachineDriver<'_>) -> bool {
        !self.runtime.idle_blocked_by_specialized_ai()
            && (driver.get_current_state_id() == Some(AIStateType::Idle as u32) || driver.is_idle())
    }

    fn is_moving_with_driver(&self, driver: &AIStateMachineDriver<'_>) -> bool {
        !self.is_idle_with_driver(driver)
            && (self.runtime.data.locomotor_goal_type != 0 || self.runtime.data.cpp_is_moving)
    }
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

    pub(crate) fn execute_command_native(
        &mut self,
        command: &crate::ai::AiCommandParams,
        native_driver: &mut AIStateMachineDriver<'_>,
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

        // C++ FaceObject/FacePosition reject immobile owners before clearing
        // the state machine or changing command source and movement bookkeeping.
        if matches!(
            command.cmd,
            crate::ai::AiCommandType::FaceObject | crate::ai::AiCommandType::FacePosition
        ) {
            let owner = self.owner().ok_or("native AI owner no longer available")?;
            if !owner
                .read()
                .map_err(|_| "native AI owner lock poisoned")?
                .is_mobile()
            {
                return Ok(());
            }
        }

        if let Some(mut assault_ai) = self.runtime.components.assault_transport_ai.take() {
            assault_ai.handle_command(command);
            self.runtime.components.assault_transport_ai = Some(assault_ai);
        }

        if let Some(mut hack_ai) = self.runtime.components.hack_internet_ai.take() {
            if hack_ai.handle_command_with_runtime(command, self, native_driver) {
                self.runtime.components.hack_internet_ai = Some(hack_ai);
                return Ok(());
            }
            self.runtime.components.hack_internet_ai = Some(hack_ai);
        }

        if let Some(mut chinook_ai) = self.runtime.components.chinook_ai.take() {
            if chinook_ai.handle_command_with_runtime(command, self, native_driver) {
                self.runtime.components.chinook_ai = Some(chinook_ai);
                return Ok(());
            }
            self.runtime.components.chinook_ai = Some(chinook_ai);
        }

        if let Some(mut jet_ai) = self.runtime.components.jet_ai.take() {
            if jet_ai.handle_command(command, self, native_driver) {
                self.runtime.components.jet_ai = Some(jet_ai);
                return Ok(());
            }
            self.runtime.components.jet_ai = Some(jet_ai);
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

        // C++ privateCommandButton changes AI order/source only when its
        // validated button dispatches an actual command. Null, unmatched,
        // unsupported and projectile buttons leave movement state intact.
        if matches!(
            command.cmd,
            crate::ai::AiCommandType::CommandButton
                | crate::ai::AiCommandType::CommandButtonObj
                | crate::ai::AiCommandType::CommandButtonPos
        ) {
            let owner = self.owner().ok_or("native AI owner no longer available")?;
            let guard = CommandOwner::Object(owner);
            guard.forward_command_to_flight_deck(command)?;
            match command.cmd {
                crate::ai::AiCommandType::CommandButton => {
                    if self.command_button_type_for_owner(&guard, command.command_button)
                        == Some(crate::commands::command::CommandType::DoStop)
                    {
                        let params = crate::ai::AiCommandParams::new(
                            crate::ai::AiCommandType::Idle,
                            command.cmd_source,
                        );
                        self.execute_command_native(&params, native_driver)?;
                    }
                }
                crate::ai::AiCommandType::CommandButtonObj => {
                    if self.command_button_type_for_owner(&guard, command.command_button)
                        == Some(crate::commands::command::CommandType::CombatDropAtLocation)
                    {
                        if let Some(target_id) = command.obj {
                            if let Some(target) = get_legacy_object(target_id) {
                                if let Ok(target_guard) = target.read() {
                                    let mut params = crate::ai::AiCommandParams::new(
                                        crate::ai::AiCommandType::CombatDrop,
                                        command.cmd_source,
                                    );
                                    params.obj = Some(target_id);
                                    params.pos = *target_guard.get_position();
                                    drop(target_guard);
                                    self.execute_command_native(&params, native_driver)?;
                                }
                            }
                        }
                    }
                }
                crate::ai::AiCommandType::CommandButtonPos => {
                    // C++ privateCommandButtonPosition currently has no implemented
                    // location command; a matching button reaches its assert-only
                    // default and otherwise produces no command-side effects.
                    let _ = self.command_button_type_for_owner(&guard, command.command_button);
                }
                _ => unreachable!(),
            }
            drop(guard);
            self.finish_command_worker_side_effects();
            return Ok(());
        }

        if !matches!(
            command.cmd,
            crate::ai::AiCommandType::FaceObject
                | crate::ai::AiCommandType::FacePosition
                | crate::ai::AiCommandType::GoProne
        ) {
            self.runtime.data.last_command_source = command.cmd_source;
        }
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
                .runtime
                .rappel_owner()
                .ok_or("rappel owner no longer available")?;
            {
                let owner = owner.read().map_err(|_| "rappel owner lock poisoned")?;
                owner.forward_command_to_flight_deck(command);
            }
            let _ = self.runtime.start_rappel_state(&owner, command.obj);
            self.finish_command_worker_side_effects();
            return Ok(());
        }
        // Native commands are bound to the factory-admitted object. This
        // kernel never falls back to a same-ID legacy Unit.
        let owner = self.owner().ok_or("native AI owner no longer available")?;
        let mut guard = CommandOwner::Object(owner);
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
                {
                    let mut machine = &mut *native_driver;

                    drop(guard);
                    let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                    if !is_mobile {
                        return Ok(());
                    }
                    if command.cmd_source == CommandSourceType::FromAi
                        && !self.is_idle_with_driver(machine)
                    {
                        machine.set_goal_position(clipped);
                        self.runtime.data.blocked_frames = 0;
                        self.runtime.data.is_blocked = false;
                        self.runtime.data.blocked_and_stuck = false;
                        let _ = machine.enter_temporary_with_ai(
                            AIStateType::MoveTo as u32,
                            LOGICFRAMES_PER_SECOND * 20,
                            self,
                        );
                    } else {
                        let mut params = command.clone();
                        params.pos = clipped;
                        machine.clear_with_ai(self);
                        self.runtime.data.blocked_frames = 0;
                        self.runtime.data.is_blocked = false;
                        self.runtime.data.blocked_and_stuck = false;
                        machine.ai_do_command_with_ai(&params, self)?;
                    }
                    return Ok(());
                }

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
                {
                    let mut machine = &mut *native_driver;

                    let mut params = command.clone();
                    params.pos = clipped;
                    machine.clear_with_ai(self);
                    machine.ai_do_command_with_ai(&params, self)?;
                    return Ok(());
                }

                guard
                    .legacy()?
                    .give_move_order(clipped, Vec::new(), false, false)?;
            }
            crate::ai::AiCommandType::MoveToObject => {
                let mut machine = &mut *native_driver;
                if self.runtime.data.ai_dead
                    || self.runtime.data.locomotor_set.get_active().is_none()
                {
                    return Ok(());
                }

                machine.clear_with_ai(self);
                self.runtime.data.blocked_frames = 0;
                self.runtime.data.is_blocked = false;
                self.runtime.data.blocked_and_stuck = false;
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::MoveAwayFromUnit => {
                if !self
                    .runtime
                    .components
                    .jet_ai
                    .as_ref()
                    .map(|jet| jet.is_allowed_to_move_away_from_unit())
                    .unwrap_or(true)
                {
                    return Ok(());
                }
                if self.runtime.data.is_ai_in_dead_state() {
                    return Ok(());
                }
                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                if !is_mobile {
                    return Ok(());
                }
                if let Some(target_id) = command.obj {
                    if (target_id == self.runtime.data.move_out_of_way_1
                        || target_id == self.runtime.data.move_out_of_way_2)
                        && self.runtime.is_blocked_and_stuck()
                    {
                        self.runtime
                            .set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
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

                            {
                                let mut machine = &mut *native_driver;

                                machine.set_goal_position(clipped);
                                let _ = machine.set_temporary_state(
                                    AIStateType::MoveOutOfTheWay as u32,
                                    LOGICFRAMES_PER_SECOND * 10,
                                );
                                return Ok(());
                            }

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
                {
                    let mut machine = &mut *native_driver;

                    machine.clear_with_ai(self);
                    machine.ai_do_command_with_ai(command, self)?;
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
                {
                    let mut machine = &mut *native_driver;

                    // C++ privateFollowPathAppend uses isMoving || waiting;
                    // borrow the live machine while this command owns Unit.write.
                    let effectively_moving =
                        self.is_moving_with_driver(machine) || self.runtime.is_waiting_for_path();
                    let is_follow_path = matches!(
                        machine.get_current_state_id(),
                        Some(id) if id == AIStateType::FollowPath as u32
                    );
                    if is_follow_path && machine.get_goal_path_size() > 0 && effectively_moving {
                        machine.ai_do_command_with_ai(command, self)?;
                        return Ok(());
                    }
                    if effectively_moving {
                        if let Some(goal) = machine.get_goal_position() {
                            let mut params = command.clone();
                            params.cmd = crate::ai::AiCommandType::FollowPath;
                            params.coords = vec![goal, command.pos];
                            machine.clear_with_ai(self);
                            machine.ai_do_command_with_ai(&params, self)?;
                        }
                        return Ok(());
                    }
                    let mut params = command.clone();
                    params.cmd = crate::ai::AiCommandType::FollowPath;
                    params.coords = vec![command.pos];
                    machine.clear_with_ai(self);
                    machine.ai_do_command_with_ai(&params, self)?;
                    return Ok(());
                }
            }
            crate::ai::AiCommandType::AttackMoveToPosition => {
                let clipped =
                    self.clip_goal_position(&guard.base_arc(), command.pos, command.cmd_source);
                {
                    let mut machine = &mut *native_driver;

                    let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                    if !is_mobile {
                        return Ok(());
                    }
                    let mut params = command.clone();
                    params.pos = clipped;
                    machine.clear_with_ai(self);
                    machine.ai_do_command_with_ai(&params, self)?;
                    if let Ok(mut obj_guard) = guard.base_arc().write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                    return Ok(());
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
                        {
                            let mut machine = &mut *native_driver;

                            let mut attack_params = crate::ai::AiCommandParams::new(
                                crate::ai::AiCommandType::AttackObject,
                                command.cmd_source,
                            );
                            attack_params.obj = Some(target_id);
                            attack_params.int_value = max_shots;
                            machine.clear_with_ai(self);
                            machine.ai_do_command_with_ai(&attack_params, self)?;
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
                            if let Some(transport_ai) =
                                self.runtime.components.transport_ai.as_ref()
                            {
                                transport_ai.private_attack_object(
                                    target_id,
                                    max_shots,
                                    command.cmd_source,
                                );
                            }
                            return Ok(());
                        }
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

                {
                    let mut machine = &mut *native_driver;

                    let mut params = command.clone();
                    params.pos = local_pos;
                    params.int_value = max_shots;
                    machine.clear_with_ai(self);
                    machine.ai_do_command_with_ai(&params, self)?;
                    if let Ok(mut obj_guard) = guard.base_arc().write() {
                        obj_guard.set_current_weapon_max_shot_count(max_shots);
                    }
                    if let Some(chinook_ai) = self.runtime.components.chinook_ai.as_ref() {
                        chinook_ai.private_attack_position(
                            &local_pos,
                            max_shots,
                            command.cmd_source,
                        );
                    }
                    if let Some(transport_ai) = self.runtime.components.transport_ai.as_ref() {
                        transport_ai.private_attack_position(
                            &local_pos,
                            max_shots,
                            command.cmd_source,
                        );
                    }
                    return Ok(());
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
                    {
                        let mut machine = &mut *native_driver;

                        machine.clear_with_ai(self);
                        machine.ai_do_command_with_ai(command, self)?;
                        self.finish_attack_object_command(target_id, command, guard);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::AttackTeam => {
                let mut machine = &mut *native_driver;

                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                if let Ok(mut obj_guard) = guard.base_arc().write() {
                    obj_guard.set_current_weapon_max_shot_count(command.int_value);
                }
                return Ok(());
            }
            crate::ai::AiCommandType::GuardPosition => {
                let mut machine = &mut *native_driver;

                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
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
                drop(guard);
                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::GuardObject => {
                let mut machine = &mut *native_driver;

                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
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
                drop(guard);
                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::GuardArea => {
                let mut machine = &mut *native_driver;

                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
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
                drop(guard);
                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::GuardTunnelNetwork => {
                let mut machine = &mut *native_driver;

                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
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
                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::GuardRetaliate => {
                if let Some(target_id) = command.obj {
                    {
                        let mut machine = &mut *native_driver;

                        machine.clear_with_ai(self);
                        machine.ai_do_command_with_ai(command, self)?;
                        if let Ok(mut obj_guard) = guard.base_arc().write() {
                            obj_guard.set_current_weapon_max_shot_count(command.int_value);
                        }
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::Enter => {
                self.runtime.data.enter_target = command.obj;
                {
                    let mut machine = &mut *native_driver;

                    let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                    if !is_mobile {
                        return Ok(());
                    }
                    if command.obj.is_some() {
                        machine.clear_with_ai(self);
                        machine.ai_do_command_with_ai(command, self)?;
                        return Ok(());
                    }
                }
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
                {
                    let mut machine = &mut *native_driver;

                    machine.clear_with_ai(self);
                    let mut exit_command = command.clone();
                    exit_command.obj = Some(exit_container);
                    machine.ai_do_command_with_ai(&exit_command, self)?;
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
                {
                    let mut machine = &mut *native_driver;

                    machine.clear_with_ai(self);
                    let mut exit_command = command.clone();
                    exit_command.obj = Some(exit_container);
                    machine.ai_do_command_with_ai(&exit_command, self)?;
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
                {
                    let mut machine = &mut *native_driver;

                    let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                    if !is_mobile {
                        return Ok(());
                    }
                    if command.obj.is_some() {
                        machine.clear_with_ai(self);
                        machine.ai_do_command_with_ai(command, self)?;
                        return Ok(());
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
                    let _ = self.runtime.set_can_path_through_units(true);
                    self.runtime.dock_machine = Some(dock_machine);
                    self.finish_command_worker_side_effects();
                    return Ok(());
                }
            }
            crate::ai::AiCommandType::ExecuteRailedTransport => {
                if let Some(mut railed_ai) = self.runtime.components.railed_transport_ai.take() {
                    if let Some(owner) = self.owner() {
                        let _ = railed_ai.handle_execute_railed_transport_with_runtime(
                            command.cmd_source,
                            &owner,
                            self,
                            native_driver,
                        );
                    }
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
                    if let Some(owner) = self.owner() {
                        let _ = railed_ai.handle_evacuate_with_runtime(
                            command.int_value,
                            command.cmd_source,
                            &owner,
                        );
                    }
                    self.runtime.components.railed_transport_ai = Some(railed_ai);
                }
            }
            crate::ai::AiCommandType::CombatDrop => {
                if let Some(mut chinook_ai) = self.runtime.components.chinook_ai.take() {
                    if let Some(owner) = self.owner() {
                        chinook_ai.private_combat_drop_for_owner(
                            &owner,
                            command.obj,
                            command.pos,
                            command.cmd_source,
                            self,
                        );
                    }
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
                    {
                        let mut machine = &mut *native_driver;

                        let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        if enter_params.obj.is_some() {
                            machine.clear_with_ai(self);
                            machine.ai_do_command_with_ai(&enter_params, self)?;
                            return Ok(());
                        }
                    }
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
                        if let Some(owner) = self.owner() {
                            chinook_ai.private_get_repaired_for_owner(
                                &owner,
                                target_id,
                                command.cmd_source,
                                self,
                            );
                        }
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
                    {
                        let mut machine = &mut *native_driver;

                        let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        if dock_params.obj.is_some() {
                            machine.clear_with_ai(self);
                            machine.ai_do_command_with_ai(&dock_params, self)?;
                            return Ok(());
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
                        let _ = self.runtime.set_can_path_through_units(true);
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
                        // Resolve every fallible value before taking the
                        // component so an early return cannot detach it.
                        let owner_id = guard.get_id()?;
                        if let Some(mut pow_ai) = self.runtime.components.pow_truck_ai.take() {
                            let _ = pow_ai.handle_pick_up_prisoner_with_runtime(
                                owner_id,
                                prisoner_id,
                                command.cmd_source,
                                self,
                                native_driver,
                            );
                            self.runtime.components.pow_truck_ai = Some(pow_ai);
                        }
                    }
                }
            }
            #[cfg(feature = "allow_surrender")]
            crate::ai::AiCommandType::ReturnPrisoners => {
                if self.runtime.components.pow_truck_ai.is_some() {
                    // See PickUpPrisoner: owner lookup must precede `take()`.
                    let owner_id = guard.get_id()?;
                    if let Some(mut pow_ai) = self.runtime.components.pow_truck_ai.take() {
                        let _ = pow_ai.handle_return_prisoners_with_runtime(
                            owner_id,
                            command.obj,
                            command.cmd_source,
                            self,
                            native_driver,
                        );
                        self.runtime.components.pow_truck_ai = Some(pow_ai);
                    }
                }
            }
            crate::ai::AiCommandType::FaceObject => {
                drop(guard);
                native_driver.clear_with_ai(self);
                self.runtime.data.blocked_frames = 0;
                self.runtime.data.is_blocked = false;
                self.runtime.data.blocked_and_stuck = false;
                self.runtime.data.last_command_source = command.cmd_source;
                native_driver.ai_do_command_with_ai(command, self)?;
                self.finish_command_worker_side_effects();
                return Ok(());
            }
            crate::ai::AiCommandType::FacePosition => {
                let owner_arc = guard.base_arc();
                let clipped = self.clip_goal_position(&owner_arc, command.pos, command.cmd_source);
                drop(guard);
                native_driver.clear_with_ai(self);
                native_driver.set_goal_position(clipped);
                self.runtime.data.blocked_frames = 0;
                self.runtime.data.is_blocked = false;
                self.runtime.data.blocked_and_stuck = false;
                self.runtime.data.last_command_source = command.cmd_source;
                let mut clipped_command = command.clone();
                clipped_command.pos = clipped;
                native_driver.ai_do_command_with_ai(&clipped_command, self)?;
                self.finish_command_worker_side_effects();
                return Ok(());
            }
            crate::ai::AiCommandType::GoProne => {
                let prone_module = guard
                    .base_arc()
                    .read()
                    .map_err(|_| "native AI owner lock poisoned")?
                    .find_update_module("ProneUpdate");
                drop(guard);
                if let Some(module) = prone_module {
                    let damage_dealt = command.damage.output.actual_damage_dealt as i32;
                    module.with_module(|module| {
                        if let Some(prone) = module.get_prone_control_interface() {
                            prone.go_prone(damage_dealt);
                        }
                    });
                }
                self.finish_command_worker_side_effects();
                return Ok(());
            }
            crate::ai::AiCommandType::Idle => {
                let mut machine = &mut *native_driver;

                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::Busy => {
                let mut machine = &mut *native_driver;

                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::Wander
            | crate::ai::AiCommandType::WanderInPlace
            | crate::ai::AiCommandType::Panic => {
                if self.runtime.data.locomotor_set.get_active().is_none() {
                    return Ok(());
                }
                {
                    let mut machine = &mut *native_driver;

                    machine.clear_with_ai(self);
                    machine.ai_do_command_with_ai(command, self)?;
                    return Ok(());
                }
            }
            crate::ai::AiCommandType::Hunt => {
                let mut machine = &mut *native_driver;

                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
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
                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::AttackArea => {
                let mut machine = &mut *native_driver;

                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
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
                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
                return Ok(());
            }
            crate::ai::AiCommandType::FollowWaypointPath
            | crate::ai::AiCommandType::FollowWaypointPathExact
            | crate::ai::AiCommandType::FollowWaypointPathAsTeam
            | crate::ai::AiCommandType::FollowWaypointPathAsTeamExact
            | crate::ai::AiCommandType::AttackFollowWaypointPath
            | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam => {
                let mut machine = &mut *native_driver;

                let is_mobile = self.runtime.data.locomotor_set.get_active().is_some();
                if !is_mobile {
                    return Ok(());
                }
                machine.clear_with_ai(self);
                machine.ai_do_command_with_ai(command, self)?;
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
            _ => {}
        }

        drop(guard);
        self.finish_command_worker_side_effects();
        Ok(())
    }
}
