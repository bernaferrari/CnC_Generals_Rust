//! AIUpdateInterface update, xfer, turret, and movement-status methods.

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
    pub(super) fn xfer_ai_update_state(&mut self, xfer: &mut dyn Xfer) -> Result<bool, String> {
        const FACADE_WAYPOINT_ID: u32 = 0x00FA_CADE;

        let is_loading = xfer.is_reading();

        let mut prior_waypoint_id = self.prior_waypoint_id.unwrap_or(FACADE_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut prior_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.prior_waypoint_id =
                (prior_waypoint_id != FACADE_WAYPOINT_ID).then_some(prior_waypoint_id);
        }

        let mut current_waypoint_id = self.current_waypoint_id.unwrap_or(FACADE_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut current_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.current_waypoint_id =
                (current_waypoint_id != FACADE_WAYPOINT_ID).then_some(current_waypoint_id);
        }

        if let Some(state_machine) = self.ai_state_machine.as_ref() {
            let mut machine = state_machine
                .lock()
                .map_err(|_| "AIUpdate state machine lock poisoned during xfer".to_string())?;
            machine.xfer(xfer)?;
        }

        xfer.xfer_bool(&mut self.ai_dead)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_recruitable)
            .map_err(|e| e.to_string())?;

        xfer.xfer_unsigned_int(&mut self.next_enemy_scan_time)
            .map_err(|e| e.to_string())?;

        let mut current_victim_id = self.get_current_victim().unwrap_or(INVALID_ID);
        xfer.xfer_object_id(&mut current_victim_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.set_current_victim((current_victim_id != INVALID_ID).then_some(current_victim_id));
        }

        xfer.xfer_real(&mut self.desired_speed)
            .map_err(|e| e.to_string())?;

        let mut last_command_source = self.last_command_source as u32;
        xfer.xfer_unsigned_int(&mut last_command_source)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.last_command_source = match last_command_source {
                0 => CommandSourceType::FromPlayer,
                1 => CommandSourceType::FromScript,
                2 => CommandSourceType::FromAi,
                3 => CommandSourceType::FromDozer,
                4 => CommandSourceType::DefaultSwitchWeapon,
                _ => CommandSourceType::FromAi,
            };
        }

        xfer_guard_target_type(xfer, &mut self.guard_target_type[0])?;
        xfer_guard_target_type(xfer, &mut self.guard_target_type[1])?;
        xfer_unit_coord3d(xfer, &mut self.location_to_guard)?;
        xfer.xfer_object_id(&mut self.object_to_guard)
            .map_err(|e| e.to_string())?;

        // Area trigger and attack-info names still need their engine registries wired to UnitAIUpdate.
        let mut area_to_guard_name = String::new();
        xfer.xfer_ascii_string(&mut area_to_guard_name)
            .map_err(|e| e.to_string())?;
        let mut attack_info_name = String::new();
        xfer.xfer_ascii_string(&mut attack_info_name)
            .map_err(|e| e.to_string())?;

        xfer.xfer_int(&mut self.planning_waypoint_count)
            .map_err(|e| e.to_string())?;
        if self.planning_waypoint_count < 0
            || self.planning_waypoint_count as usize > AI_UPDATE_MAX_WAYPOINTS
        {
            return Err(format!(
                "Invalid AIUpdate waypoint count {}, max {}",
                self.planning_waypoint_count, AI_UPDATE_MAX_WAYPOINTS
            ));
        }
        for waypoint in self
            .planning_waypoint_queue
            .iter_mut()
            .take(self.planning_waypoint_count as usize)
        {
            xfer_unit_coord3d(xfer, waypoint)?;
        }
        xfer.xfer_int(&mut self.planning_waypoint_index)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.executing_waypoint_queue)
            .map_err(|e| e.to_string())?;

        let mut completed_waypoint_id = self
            .completed_waypoint_id
            .unwrap_or(crate::common::INVALID_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut completed_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.completed_waypoint_id = (completed_waypoint_id
                != crate::common::INVALID_WAYPOINT_ID)
                .then_some(completed_waypoint_id);
        }

        xfer.xfer_bool(&mut self.waiting_for_path)
            .map_err(|e| e.to_string())?;
        if is_loading && !self.waiting_for_path {
            self.queue_for_path_frame = 0;
        }

        let mut got_path = self.current_path_snapshot.is_some();
        xfer.xfer_bool(&mut got_path).map_err(|e| e.to_string())?;
        if is_loading {
            self.current_path_snapshot = got_path.then(AiPath::new);
        }
        if let Some(path) = self.current_path_snapshot.as_mut().filter(|_| got_path) {
            path.xfer(xfer)?;
        }

        xfer.xfer_object_id(&mut self.requested_victim_id)
            .map_err(|e| e.to_string())?;
        xfer_unit_coord3d(xfer, &mut self.requested_destination)?;
        xfer_unit_coord3d(xfer, &mut self.requested_destination2)?;

        xfer.xfer_object_id(&mut self.ignore_obstacle_id)
            .map_err(|e| e.to_string())?;
        let mut path_extra_distance = self.current_path_extra_distance();
        xfer.xfer_real(&mut path_extra_distance)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.set_path_extra_distance(path_extra_distance)
                .map_err(|e| e.to_string())?;
        }
        xfer_unit_icoord2d(xfer, &mut self.pathfind_goal_cell)?;
        xfer_unit_icoord2d(xfer, &mut self.pathfind_cur_cell)?;

        xfer.xfer_unsigned_int(&mut self.ignore_collisions_until)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.queue_for_path_frame)
            .map_err(|e| e.to_string())?;

        xfer_unit_coord3d(xfer, &mut self.final_position)?;
        xfer.xfer_bool(&mut self.do_final_position)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_attack_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_final_goal)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_approach_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_safe_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.movement_complete)
            .map_err(|e| e.to_string())?;
        let mut is_safe_path_duplicate = self.is_safe_path;
        xfer.xfer_bool(&mut is_safe_path_duplicate)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.is_safe_path = is_safe_path_duplicate;
        }

        xfer.xfer_bool(&mut self.locomotor_upgraded)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.can_path_through_units)
            .map_err(|e| e.to_string())?;
        let mut randomly_offset_mood_check = false;
        xfer.xfer_bool(&mut randomly_offset_mood_check)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.repulsor1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.repulsor2)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.move_out_of_way_1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.move_out_of_way_2)
            .map_err(|e| e.to_string())?;

        self.xfer_locomotor_set_state(xfer)?;

        xfer.xfer_unsigned_int(&mut self.locomotor_goal_type)
            .map_err(|e| e.to_string())?;
        xfer_unit_coord3d(xfer, &mut self.locomotor_goal_data)?;

        if let Some(machine) = self.turret_primary_machine.as_ref() {
            Self::xfer_turret_ai(machine, xfer)?;
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            Self::xfer_turret_ai(machine, xfer)?;
        }

        let mut turret_sync_flag: u32 = 0;
        xfer.xfer_unsigned_int(&mut turret_sync_flag)
            .map_err(|e| e.to_string())?;
        let mut attitude = self.attitude as u32;
        xfer.xfer_unsigned_int(&mut attitude)
            .map_err(|e| e.to_string())?;

        let mut next_mood_check_time = self.get_next_mood_check_time();
        xfer.xfer_unsigned_int(&mut next_mood_check_time)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.set_next_mood_check_time(next_mood_check_time);
        }

        let mut crate_created = self
            .crate_created
            .lock()
            .map(|id| *id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_object_id(&mut crate_created)
            .map_err(|e| e.to_string())?;
        if is_loading {
            if let Ok(mut id) = self.crate_created.lock() {
                *id = crate_created;
            }
        }

        Ok(true)
    }
    pub(super) fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_blocked {
            self.blocked_frames = self.blocked_frames.saturating_add(1);
        } else if self.blocked_frames > 1 {
            self.blocked_frames = 1;
        } else {
            self.blocked_frames = 0;
            self.blocked_and_stuck = false;
        }
        self.is_blocked = false;
        self.cur_max_blocked_speed = FAST_AS_POSSIBLE;

        if self.rappel_state.is_some() {
            self.update_rappel_state();
        }

        if self.demoralized_frames_left > 0 {
            let next = self.demoralized_frames_left.saturating_sub(1);
            self.set_demoralized(next);
        }

        if self.surrendered_frames_left > 0 {
            self.surrendered_frames_left = self.surrendered_frames_left.saturating_sub(1);
            if self.surrendered_frames_left == 0 {
                self.surrendered_player_index = None;
            }
        }

        #[cfg(feature = "allow_surrender")]
        if let Some(mut pow_ai) = self.pow_truck_ai.take() {
            let owner_id = get_unit_arc(self.unit_id)
                .and_then(|unit| unit.read().ok().map(|guard| guard.get_id()))
                .unwrap_or(crate::common::INVALID_ID);
            let _ = pow_ai.update(owner_id, self);
            self.pow_truck_ai = Some(pow_ai);
        }

        if let Some(mut railed_ai) = self.railed_transport_ai.take() {
            let _ = railed_ai.update(self);
            self.railed_transport_ai = Some(railed_ai);
        }

        if let Some(mut hack_ai) = self.hack_internet_ai.take() {
            let _ = hack_ai.update(self);
            self.hack_internet_ai = Some(hack_ai);
        }

        if let Some(mut assault_ai) = self.assault_transport_ai.take() {
            let _ = assault_ai.update(self);
            self.assault_transport_ai = Some(assault_ai);
        }

        if let Some(mut deliver_ai) = self.deliver_payload_ai.take() {
            let _ = deliver_ai.update(self);
            self.deliver_payload_ai = Some(deliver_ai);
        }

        if let Some(mut deploy_ai) = self.deploy_style_ai.take() {
            let _ = deploy_ai.update(self);
            self.deploy_style_ai = Some(deploy_ai);
        }

        if let Some(mut chinook_ai) = self.chinook_ai.take() {
            let _ = chinook_ai.update(self);
            self.chinook_ai = Some(chinook_ai);
        }

        if let Some(mut supply_ai) = self.supply_truck_ai.take() {
            supply_ai.update();
            self.supply_truck_ai = Some(supply_ai);
        }
        if let Some(mut worker_ai) = self.worker_ai.take() {
            worker_ai.update();
            self.worker_ai = Some(worker_ai);
        }

        if let Some(mut wander_ai) = self.wander_ai.take() {
            let _ = wander_ai.update(self);
            self.wander_ai = Some(wander_ai);
        }
        if let Some(mut dozer_ai) = self.dozer_ai.take() {
            dozer_ai.update();
            self.dozer_ai = Some(dozer_ai);
        }

        if let Some(mut jet_ai) = self.jet_ai.take() {
            jet_ai.update_with_ai(self);
            self.jet_ai = Some(jet_ai);
        }

        if let Some(state_machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut machine) = state_machine.lock() {
                if self.ai_dead && machine.get_current_state_id() != Some(AIStateType::Dead as u32)
                {
                    machine.clear();
                    let _ = machine.set_state(AIStateType::Dead as u32);
                    machine.lock();
                }
                let _ = machine.update_state_machine();
            }
        }

        self.finish_completed_movement_like_cpp();

        let now = TheGameLogic::get_frame();
        if self.waiting_for_path
            && (self.queue_for_path_frame == 0 || now >= self.queue_for_path_frame)
        {
            let _ = self.do_queued_pathfind_now();
        } else if self.queue_for_path_frame != 0 && now >= self.queue_for_path_frame {
            self.queue_for_path_frame = 0;
            let _ = self.queue_path_request_now(self.requested_destination);
        }

        let update_turrets = get_unit_arc(self.unit_id)
            .and_then(|unit| unit.read().ok().map(|guard| guard.base_arc()))
            .and_then(|base| {
                base.read().ok().map(|obj| {
                    !obj.is_effectively_dead()
                        && !obj.is_disabled_by_type(DisabledType::Paralyzed)
                        && !obj.is_disabled_by_type(DisabledType::DisabledUnmanned)
                        && !obj.is_disabled_by_type(DisabledType::DisabledEmp)
                        && !obj.is_disabled_by_type(DisabledType::DisabledSubdued)
                        && !obj.is_disabled_by_type(DisabledType::DisabledHacked)
                })
            })
            .unwrap_or(false);

        if update_turrets {
            if let Some(machine) = self.turret_primary_machine.as_ref() {
                if let Some(turret) = machine.get_turret_ai() {
                    let _ = TurretAI::update_turret_ai_handle(&turret);
                }
            }
            if let Some(machine) = self.turret_secondary_machine.as_ref() {
                if let Some(turret) = machine.get_turret_ai() {
                    let _ = TurretAI::update_turret_ai_handle(&turret);
                }
            }
        }

        if let Some(mut dock_machine) = self.dock_machine.take() {
            let update_result = dock_machine
                .state_machine
                .lock()
                .map(|mut machine| machine.update())
                .unwrap_or(crate::state_machine::StateReturnType::Failure);

            match update_result.convert_sleep_to_continue() {
                crate::state_machine::StateReturnType::Continue
                | crate::state_machine::StateReturnType::Blocked => {
                    self.dock_machine = Some(dock_machine);
                }
                _ => {
                    let _ = dock_machine.halt();
                    let _ = self.set_can_path_through_units(false);
                    if self.current_command == Some(crate::ai::AiCommandType::Dock) {
                        self.current_command = None;
                    }
                }
            }
        }
        let mut pending_params: Option<crate::ai::AiCommandParams> = None;
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if jet_ai.has_pending_command()
                && (self.current_command.is_none()
                    || self.current_command == Some(crate::ai::AiCommandType::Idle))
                && !self.is_reloading()
            {
                pending_params = Some(jet_ai.reconstitute_command_params());
            }
        }
        if let Some(params) = pending_params {
            if let Some(jet_ai) = self.jet_ai.as_mut() {
                jet_ai.set_has_pending_command(false);
            }
            let _ = self.execute_command(&params);
        }
        if self.jet_ai.is_some()
            && (self.current_command.is_none()
                || self.current_command == Some(crate::ai::AiCommandType::Idle))
            && !self
                .jet_ai
                .as_ref()
                .map(|jet| jet.has_pending_command())
                .unwrap_or(false)
        {
            self.pending_command = None;
        }

        let is_reloading = self.is_reloading();
        let mut queued_enter_command: Option<crate::ai::AiCommandParams> = None;
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            let takeoff = matches!(
                self.current_command,
                Some(crate::ai::AiCommandType::Exit)
                    | Some(crate::ai::AiCommandType::FollowExitProductionPath)
            );
            let landing = matches!(
                self.current_command,
                Some(crate::ai::AiCommandType::Enter) | Some(crate::ai::AiCommandType::Dock)
            );
            let taxiing = takeoff || landing;
            jet_ai.set_takeoff_in_progress(takeoff);
            jet_ai.set_landing_in_progress(landing);
            jet_ai.set_taxi_in_progress(taxiing);
            if taxiing {
                jet_ai.set_allow_air_loco(false);
            }
            jet_ai.set_has_pending_command(self.pending_command.is_some());
            if jet_ai.allow_air_loco() && jet_ai.is_out_of_special_reload_ammo() {
                jet_ai.set_use_special_return_loco(true);
            } else if !jet_ai.allow_air_loco() {
                jet_ai.set_use_special_return_loco(false);
            }
            if !jet_ai.has_pending_command()
                && jet_ai.allow_air_loco()
                && jet_ai.is_out_of_special_reload_ammo()
                && !is_reloading
                && !matches!(
                    self.current_command,
                    Some(crate::ai::AiCommandType::Enter) | Some(crate::ai::AiCommandType::Dock)
                )
            {
                let producer_id = get_unit_arc(self.unit_id)
                    .and_then(|unit| unit.read().ok().map(|guard| guard.base_arc()))
                    .and_then(|obj| obj.read().ok().map(|guard| guard.get_producer_id()))
                    .unwrap_or(crate::common::INVALID_ID);
                if producer_id != crate::common::INVALID_ID {
                    jet_ai.set_has_pending_command(true);
                    jet_ai.set_suppress_command_store(true);
                    let mut params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::Enter,
                        crate::ai::CommandSourceType::FromAi,
                    );
                    params.obj = Some(producer_id);
                    queued_enter_command = Some(params);
                }
            }
            if let Some(desired) = jet_ai.desired_locomotor_set() {
                let _ = self.choose_locomotor_set(desired);
            } else if jet_ai.allow_air_loco()
                && self.current_locomotor_set == LocomotorSetType::Taxiing
            {
                let _ = self.choose_locomotor_set(LocomotorSetType::Normal);
            } else if !jet_ai.allow_air_loco()
                && self.current_locomotor_set != LocomotorSetType::Taxiing
            {
                let _ = self.choose_locomotor_set(LocomotorSetType::Taxiing);
            }
        }
        if let Some(params) = queued_enter_command {
            let _ = self.execute_command(&params);
        }
        Ok(())
    }
    pub(super) fn apply_bump_speed_limit(
        &mut self,
        mut desired_speed: Real,
        mut blocked: bool,
    ) -> Real {
        if blocked && desired_speed > self.cur_max_blocked_speed {
            desired_speed = self.cur_max_blocked_speed;
            if self.bump_speed_limit > desired_speed {
                self.bump_speed_limit = desired_speed;
            }
            self.bump_speed_limit *= 0.95;
            desired_speed = self.bump_speed_limit;
        } else {
            blocked = false;
            if self.bump_speed_limit < FAST_AS_POSSIBLE {
                let min_limit = desired_speed * 0.2;
                if self.bump_speed_limit < min_limit {
                    self.bump_speed_limit = min_limit;
                }
                self.bump_speed_limit *= 1.05;
            }
            if desired_speed > self.bump_speed_limit {
                desired_speed = self.bump_speed_limit;
            }
        }
        if !blocked && self.blocked_frames > 1 {
            self.blocked_frames = 1;
        }
        desired_speed
    }
    pub(super) fn is_attacking(&self) -> bool {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(guard) = machine.lock() {
                if guard.is_attack_state() {
                    return true;
                }
            }
        }
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        guard
            .base_arc()
            .read()
            .ok()
            .map(|obj| obj.test_status(ObjectStatusTypes::OBJECT_STATUS_IS_ATTACKING))
            .unwrap_or(false)
            || matches!(
                guard.current_order,
                Some(UnitOrder::Attack { .. }) | Some(UnitOrder::AttackMove { .. })
            )
            || guard.movement_state == MovementState::Attacking
    }
    pub(super) fn get_enter_target(&self) -> Option<ObjectID> {
        self.enter_target
    }
    pub(super) fn set_demoralized(&mut self, duration_frames: UnsignedInt) {
        let prev = self.demoralized_frames_left;
        self.demoralized_frames_left = duration_frames;

        if (prev == 0 && self.demoralized_frames_left > 0)
            || (prev > 0 && self.demoralized_frames_left == 0)
        {
            self.evaluate_morale_bonus();
        }
    }
    pub(super) fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_owners_cur_weapon_on_turret())
                    .unwrap_or(false)
                {
                    return TurretType::Primary;
                }
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_owners_cur_weapon_on_turret())
                    .unwrap_or(false)
                {
                    return TurretType::Secondary;
                }
            }
        }
        TurretType::Invalid
    }
    pub(super) fn get_which_turret_for_weapon_slot(&self, slot: WeaponSlotType) -> TurretType {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_weapon_slot_on_turret(slot))
                    .unwrap_or(false)
                {
                    return TurretType::Primary;
                }
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_weapon_slot_on_turret(slot))
                    .unwrap_or(false)
                {
                    return TurretType::Secondary;
                }
            }
        }
        TurretType::Invalid
    }
    pub(super) fn set_turret_enabled(&mut self, turret: TurretType, enabled: bool) {
        match turret {
            TurretType::Primary => {
                self.turret_primary_enabled = enabled;
                if let Some(machine) = self.turret_primary_machine.as_ref() {
                    if let Some(ai) = machine.get_turret_ai() {
                        if let Ok(mut guard) = ai.lock() {
                            guard.set_turret_enabled(enabled);
                        }
                    }
                }
                if self.turrets_linked {
                    self.turret_secondary_enabled = enabled;
                    if let Some(machine) = self.turret_secondary_machine.as_ref() {
                        if let Some(ai) = machine.get_turret_ai() {
                            if let Ok(mut guard) = ai.lock() {
                                guard.set_turret_enabled(enabled);
                            }
                        }
                    }
                }
            }
            TurretType::Secondary => {
                self.turret_secondary_enabled = enabled;
                if let Some(machine) = self.turret_secondary_machine.as_ref() {
                    if let Some(ai) = machine.get_turret_ai() {
                        if let Ok(mut guard) = ai.lock() {
                            guard.set_turret_enabled(enabled);
                        }
                    }
                }
                if self.turrets_linked {
                    self.turret_primary_enabled = enabled;
                    if let Some(machine) = self.turret_primary_machine.as_ref() {
                        if let Some(ai) = machine.get_turret_ai() {
                            if let Ok(mut guard) = ai.lock() {
                                guard.set_turret_enabled(enabled);
                            }
                        }
                    }
                }
            }
            TurretType::Invalid => {}
        }
    }
    pub(super) fn recenter_turret(&mut self, turret: TurretType) {
        match turret {
            TurretType::Primary => {
                self.turret_primary_natural = true;
                if let Some(machine) = self.turret_primary_machine.as_ref() {
                    if let Some(ai) = machine.get_turret_ai() {
                        if let Ok(mut guard) = ai.lock() {
                            guard.recenter_turret();
                        }
                    }
                }
                if self.turrets_linked {
                    self.turret_secondary_natural = true;
                    if let Some(machine) = self.turret_secondary_machine.as_ref() {
                        if let Some(ai) = machine.get_turret_ai() {
                            if let Ok(mut guard) = ai.lock() {
                                guard.recenter_turret();
                            }
                        }
                    }
                }
            }
            TurretType::Secondary => {
                self.turret_secondary_natural = true;
                if let Some(machine) = self.turret_secondary_machine.as_ref() {
                    if let Some(ai) = machine.get_turret_ai() {
                        if let Ok(mut guard) = ai.lock() {
                            guard.recenter_turret();
                        }
                    }
                }
                if self.turrets_linked {
                    self.turret_primary_natural = true;
                    if let Some(machine) = self.turret_primary_machine.as_ref() {
                        if let Some(ai) = machine.get_turret_ai() {
                            if let Ok(mut guard) = ai.lock() {
                                guard.recenter_turret();
                            }
                        }
                    }
                }
            }
            TurretType::Invalid => {}
        }
    }
    pub(super) fn is_turret_in_natural_position(&self, turret: TurretType) -> bool {
        match turret {
            TurretType::Primary => self
                .turret_primary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|guard| guard.is_turret_in_natural_position())
                })
                .unwrap_or(false),
            TurretType::Secondary => self
                .turret_secondary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|guard| guard.is_turret_in_natural_position())
                })
                .unwrap_or(false),
            TurretType::Invalid => false,
        }
    }
    pub(super) fn is_turret_enabled(&self, turret: TurretType) -> bool {
        match turret {
            TurretType::Primary => self
                .turret_primary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| ai.lock().ok().map(|guard| guard.is_turret_enabled()))
                .unwrap_or(false),
            TurretType::Secondary => self
                .turret_secondary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| ai.lock().ok().map(|guard| guard.is_turret_enabled()))
                .unwrap_or(false),
            TurretType::Invalid => false,
        }
    }
    pub(super) fn get_turret_rot_and_pitch(&self, turret: TurretType) -> Option<(Real, Real)> {
        match turret {
            TurretType::Primary => self
                .turret_primary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|guard| (guard.get_turret_angle(), guard.get_turret_pitch()))
                }),
            TurretType::Secondary => self
                .turret_secondary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|guard| (guard.get_turret_angle(), guard.get_turret_pitch()))
                }),
            TurretType::Invalid => None,
        }
    }
    pub(super) fn get_turret_angle(&self, turret: TurretType) -> Real {
        self.get_turret_rot_and_pitch(turret)
            .map(|(angle, _)| angle)
            .unwrap_or(0.0)
    }
    pub(super) fn get_turret_pitch(&self, turret: TurretType) -> Real {
        self.get_turret_rot_and_pitch(turret)
            .map(|(_, pitch)| pitch)
            .unwrap_or(0.0)
    }
    pub(super) fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        slot: WeaponSlotType,
        target: ObjectID,
    ) -> bool {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| {
                        guard.is_weapon_slot_on_turret(slot)
                            && guard.is_trying_to_aim_at_target(target)
                    })
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| {
                        guard.is_weapon_slot_on_turret(slot)
                            && guard.is_trying_to_aim_at_target(target)
                    })
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        false
    }
    pub(super) fn is_moving(&self) -> bool {
        if self.is_idle() {
            return false;
        }
        get_unit_arc(self.unit_id)
            .and_then(|unit| {
                unit.read().ok().map(|guard| {
                    guard.is_movement_active()
                        || guard
                            .path_following_state
                            .as_ref()
                            .map(|state| state.waiting_for_path)
                            .unwrap_or(false)
                        || guard.current_path.is_some()
                        || guard.target_position.is_some()
                })
            })
            .unwrap_or(false)
    }
    pub(super) fn is_idle(&self) -> bool {
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if jet_ai.should_block_idle(self.pending_command) {
                return false;
            }
        }
        if let Some(hack_ai) = self.hack_internet_ai.as_ref() {
            if hack_ai.has_pending_command() {
                return false;
            }
        }
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(guard) = machine.lock() {
                if !guard.is_idle() {
                    return false;
                }
            }
        }
        get_unit_arc(self.unit_id)
            .and_then(|unit| {
                unit.read().ok().map(|guard| {
                    guard.movement_state == MovementState::Idle
                        && !guard
                            .path_following_state
                            .as_ref()
                            .map(|state| state.waiting_for_path)
                            .unwrap_or(false)
                        && guard.current_path.is_none()
                        && guard.target_position.is_none()
                })
            })
            .unwrap_or(false)
    }
    pub(super) fn is_busy(&self) -> bool {
        self.ai_state_machine
            .as_ref()
            .and_then(|machine| machine.lock().ok())
            .map(|guard| guard.is_busy())
            .unwrap_or(false)
    }
    pub(super) fn set_attitude(
        &mut self,
        attitude: AIAttitudeType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.attitude = attitude;
        Ok(())
    }
    pub(super) fn get_attitude(&self) -> AIAttitudeType {
        self.attitude
    }
    pub(super) fn is_idle_unrestricted(&self) -> bool {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(guard) = machine.lock() {
                if !guard.is_idle() {
                    return false;
                }
            }
        }
        get_unit_arc(self.unit_id)
            .and_then(|unit| {
                unit.read().ok().map(|guard| {
                    guard.movement_state == MovementState::Idle
                        && !guard
                            .path_following_state
                            .as_ref()
                            .map(|state| state.waiting_for_path)
                            .unwrap_or(false)
                        && guard.current_path.is_none()
                        && guard.target_position.is_none()
                })
            })
            .unwrap_or(false)
    }
    pub(super) fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        if let Some(path) = self.pending_safe_path.take() {
            return self.set_path_from_coords(&path);
        }
        let unit =
            get_unit_arc(self.unit_id).ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard
            .give_move_order(*target, Vec::new(), false, false)
            .map_err(|err| err.to_string())
    }
    pub(super) fn set_current_goal_path_index(
        &mut self,
        index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.current_goal_path_index = index;
        Ok(())
    }
    pub(super) fn get_current_goal_path_index(&self) -> i32 {
        self.current_goal_path_index
    }
    pub(super) fn set_can_path_through_units(
        &mut self,
        value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.can_path_through_units = value;
        if value {
            self.blocked_and_stuck = false;
        }
        Ok(())
    }
    pub(super) fn get_can_path_through_units(&self) -> bool {
        self.can_path_through_units
    }
    pub(super) fn is_blocked_and_stuck(&self) -> bool {
        const BLOCKED_RECOMPUTE_THRESHOLD: u32 = 60;
        if self.blocked_and_stuck {
            return true;
        }
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        guard.path_following_state.as_ref().map_or(false, |state| {
            state.frames_blocked > BLOCKED_RECOMPUTE_THRESHOLD
        })
    }
    pub(super) fn set_is_blocked(&mut self, blocked: bool) {
        self.is_blocked = blocked;
    }
    pub(super) fn set_blocked_and_stuck(&mut self, blocked: bool) {
        self.blocked_and_stuck = blocked;
    }
    pub(super) fn get_num_frames_blocked(&self) -> u32 {
        let mut frames = self.blocked_frames;
        let Some(unit) = get_unit_arc(self.unit_id) else {
            return frames;
        };
        let Ok(guard) = unit.read() else {
            return frames;
        };
        if let Some(state) = guard.path_following_state.as_ref() {
            frames = frames.max(state.frames_blocked);
        }
        frames
    }
    pub(super) fn destroy_path(&mut self) {
        self.current_path_snapshot = None;
        self.waiting_for_path = false;
        self.is_attack_path = false;
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.current_path = None;
                guard.path_following_state = None;
            }
        }
        self.set_locomotor_goal_none();
    }
    pub(super) fn clear_move_out_of_way(&mut self) {
        self.move_out_of_way_1 = INVALID_ID;
        self.move_out_of_way_2 = INVALID_ID;
    }
}
