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
    fn apply_pending_assaults(&mut self) {
        let Some(owner_id) = self.owner_object_id() else {
            return;
        };
        let Some(owner_arc) = crate::helpers::TheGameLogic::find_object_by_id(owner_id) else {
            return;
        };
        let Ok(mut owner) = owner_arc.try_write() else {
            return;
        };
        let pending = owner.weapon_set.take_pending_assaults();
        drop(owner);
        let Some(assault) = self.get_assault_transport_ai_update_interface() else {
            return;
        };
        for target in pending {
            assault.begin_assault(target);
        }
    }

    /// UnitAIUpdate's deferred idle-mood pass. The turret is loaned here by
    /// [`TurretStateMachine::take_turret`], so the borrow never aliases `self`.
    /// C++ `TurretAI::friend_checkForIdleMoodTarget` tail (TurretAI.cpp:855-876)
    /// runs as `set_current_target_from_idle_mood`, and the still-idle sleep
    /// clamp keeps the machine from out-sleeping the next mood check.
    fn apply_deferred_idle_mood_target(&mut self, turret: &mut crate::ai::turret::TurretAI) {
        let adjustment = self.get_mood_matrix_action_adjustment(crate::ai::MoodMatrixAction::Idle);
        if (adjustment & crate::ai::mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL) != 0 {
            return;
        }
        let Some(enemy) = self.get_next_mood_target(true, true) else {
            return;
        };
        if let Some(owner_id) = self.owner_object_id() {
            if let Some(owner_arc) = crate::helpers::TheGameLogic::find_object_by_id(owner_id) {
                if let (Ok(mut owner_write), Ok(target_guard)) =
                    (owner_arc.try_write(), enemy.read())
                {
                    let _ = owner_write.choose_best_weapon_for_target(
                        &target_guard,
                        crate::weapon::WeaponChoiceCriteria::PreferMostDamage,
                        crate::common::CommandSourceType::FromAi,
                    );
                }
            }
        }
        let enemy_id = enemy.read().ok().map(|guard| guard.get_id());
        turret.set_current_target_from_idle_mood(enemy_id);
        turret.set_next_mood_check_cached(self.get_next_mood_check_time());
        let idle_id = crate::ai::turret::TurretStateType::Idle.into();
        let still_idle = turret.get_current_state_id() == Some(idle_id);
        if still_idle {
            let next = turret
                .get_sleep_until()
                .min(self.get_next_mood_check_time());
            turret.set_sleep_until(next);
        } else {
            turret.set_sleep_until(0);
        }
    }

    /// One turret's tick (C++ `AIUpdate::update()` turret tail): stamp the
    /// per-tick UnitAI context, run the turret's owned machine, then drain the
    /// flags the step raised back into UnitAI. `turret` comes from
    /// [`TurretStateMachine::take_turret`], so the two borrows never alias.
    fn update_single_turret(unit_ai: &mut UnitAIUpdate, turret: &mut TurretAI) {
        turret.set_turrets_linked_cached(unit_ai.are_turrets_linked());
        let adjust = unit_ai.get_mood_matrix_action_adjustment(crate::ai::MoodMatrixAction::Attack);
        turret.set_attack_ok_cached((adjust & crate::ai::mood_matrix_adjustment::ACTION_OK) != 0);
        turret.set_goal_object_id_cached(unit_ai.get_goal_object_id());
        turret.set_last_command_source_cached(unit_ai.get_last_command_source());
        turret.set_next_mood_check_cached(unit_ai.get_next_mood_check_time());
        let _ = turret.update_turret_ai();
        if turret.take_reset_mood_check() {
            unit_ai.reset_next_mood_check_time();
        }
        if let Some(which) = turret.take_clear_turret_sync() {
            if unit_ai.friend_get_turret_sync() == which {
                unit_ai.friend_set_turret_sync(crate::common::TurretType::Invalid);
            }
        }
        if turret.take_idle_mood_check() {
            unit_ai.apply_deferred_idle_mood_target(turret);
        }
        unit_ai.apply_pending_assaults();
    }

    pub(super) fn xfer_ai_update_state(&mut self, xfer: &mut dyn Xfer) -> Result<bool, String> {
        const FACADE_WAYPOINT_ID: u32 = 0x00FA_CADE;

        let is_loading = xfer.is_reading();

        let mut prior_waypoint_id = self.data.prior_waypoint_id.unwrap_or(FACADE_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut prior_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.data.prior_waypoint_id =
                (prior_waypoint_id != FACADE_WAYPOINT_ID).then_some(prior_waypoint_id);
        }

        let mut current_waypoint_id = self.data.current_waypoint_id.unwrap_or(FACADE_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut current_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.data.current_waypoint_id =
                (current_waypoint_id != FACADE_WAYPOINT_ID).then_some(current_waypoint_id);
        }

        if let Some(state_machine) = self.ai_state_machine.as_ref() {
            let mut machine = state_machine
                .lock()
                .map_err(|_| "AIUpdate state machine lock poisoned during xfer".to_string())?;
            machine.xfer(xfer)?;
        }

        xfer.xfer_bool(&mut self.data.ai_dead)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.is_recruitable)
            .map_err(|e| e.to_string())?;

        xfer.xfer_unsigned_int(&mut self.data.next_enemy_scan_time)
            .map_err(|e| e.to_string())?;

        // AIUpdate.cpp:5012 transfers the owned field directly; loading must
        // not run targeter notifications from the public command setter.
        xfer.xfer_object_id(&mut self.current_victim_id)
            .map_err(|e| e.to_string())?;

        xfer.xfer_real(&mut self.data.desired_speed)
            .map_err(|e| e.to_string())?;

        let mut last_command_source = self.data.last_command_source as u32;
        xfer.xfer_unsigned_int(&mut last_command_source)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.data.last_command_source = match last_command_source {
                0 => CommandSourceType::FromPlayer,
                1 => CommandSourceType::FromScript,
                2 => CommandSourceType::FromAi,
                3 => CommandSourceType::FromDozer,
                4 => CommandSourceType::DefaultSwitchWeapon,
                _ => CommandSourceType::FromAi,
            };
        }

        xfer_guard_target_type(xfer, &mut self.data.guard_target_type[0])?;
        xfer_guard_target_type(xfer, &mut self.data.guard_target_type[1])?;
        xfer_unit_coord3d(xfer, &mut self.data.location_to_guard)?;
        xfer.xfer_object_id(&mut self.data.object_to_guard)
            .map_err(|e| e.to_string())?;

        // Area trigger and attack-info names still need their engine registries wired to UnitAIUpdate.
        let mut area_to_guard_name = String::new();
        xfer.xfer_ascii_string(&mut area_to_guard_name)
            .map_err(|e| e.to_string())?;
        let mut attack_info_name = String::new();
        xfer.xfer_ascii_string(&mut attack_info_name)
            .map_err(|e| e.to_string())?;

        xfer.xfer_int(&mut self.data.planning_waypoint_count)
            .map_err(|e| e.to_string())?;
        if self.data.planning_waypoint_count < 0
            || self.data.planning_waypoint_count as usize > AI_UPDATE_MAX_WAYPOINTS
        {
            return Err(format!(
                "Invalid AIUpdate waypoint count {}, max {}",
                self.data.planning_waypoint_count, AI_UPDATE_MAX_WAYPOINTS
            ));
        }
        for waypoint in self
            .data
            .planning_waypoint_queue
            .iter_mut()
            .take(self.data.planning_waypoint_count as usize)
        {
            xfer_unit_coord3d(xfer, waypoint)?;
        }
        xfer.xfer_int(&mut self.data.planning_waypoint_index)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.executing_waypoint_queue)
            .map_err(|e| e.to_string())?;

        let mut completed_waypoint_id = self
            .data
            .completed_waypoint_id
            .unwrap_or(crate::common::INVALID_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut completed_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.data.completed_waypoint_id = (completed_waypoint_id
                != crate::common::INVALID_WAYPOINT_ID)
                .then_some(completed_waypoint_id);
        }

        xfer.xfer_bool(&mut self.data.waiting_for_path)
            .map_err(|e| e.to_string())?;
        if is_loading && !self.data.waiting_for_path {
            self.data.queue_for_path_frame = 0;
        }

        let mut got_path = self.data.current_path_snapshot.is_some();
        xfer.xfer_bool(&mut got_path).map_err(|e| e.to_string())?;
        if is_loading {
            self.data.current_path_snapshot = got_path.then(AiPath::new);
        }
        if let Some(path) = self
            .data
            .current_path_snapshot
            .as_mut()
            .filter(|_| got_path)
        {
            path.xfer(xfer)?;
        }

        xfer.xfer_object_id(&mut self.data.requested_victim_id)
            .map_err(|e| e.to_string())?;
        xfer_unit_coord3d(xfer, &mut self.data.requested_destination)?;
        xfer_unit_coord3d(xfer, &mut self.data.requested_destination2)?;

        xfer.xfer_object_id(&mut self.data.ignore_obstacle_id)
            .map_err(|e| e.to_string())?;
        let mut path_extra_distance = self.current_path_extra_distance();
        xfer.xfer_real(&mut path_extra_distance)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.set_path_extra_distance(path_extra_distance)
                .map_err(|e| e.to_string())?;
        }
        xfer_unit_icoord2d(xfer, &mut self.data.pathfind_goal_cell)?;
        xfer_unit_icoord2d(xfer, &mut self.data.pathfind_cur_cell)?;

        xfer.xfer_unsigned_int(&mut self.data.ignore_collisions_until)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.data.queue_for_path_frame)
            .map_err(|e| e.to_string())?;

        xfer_unit_coord3d(xfer, &mut self.data.final_position)?;
        xfer.xfer_bool(&mut self.data.do_final_position)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.is_attack_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.is_final_goal)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.is_approach_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.is_safe_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.movement_complete)
            .map_err(|e| e.to_string())?;
        let mut is_safe_path_duplicate = self.data.is_safe_path;
        xfer.xfer_bool(&mut is_safe_path_duplicate)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.data.is_safe_path = is_safe_path_duplicate;
        }

        xfer.xfer_bool(&mut self.data.locomotor_upgraded)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.can_path_through_units)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.data.randomly_offset_mood_check)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.data.repulsor1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.data.repulsor2)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.data.move_out_of_way_1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.data.move_out_of_way_2)
            .map_err(|e| e.to_string())?;

        self.xfer_locomotor_set_state(xfer)?;

        xfer.xfer_unsigned_int(&mut self.data.locomotor_goal_type)
            .map_err(|e| e.to_string())?;
        xfer_unit_coord3d(xfer, &mut self.data.locomotor_goal_data)?;

        if let Some(machine) = self.data.turret_primary_machine.as_mut() {
            Self::xfer_turret_ai(machine, xfer)?;
        }
        if let Some(machine) = self.data.turret_secondary_machine.as_mut() {
            Self::xfer_turret_ai(machine, xfer)?;
        }

        let mut turret_sync_flag = match self.data.turret_sync_flag {
            TurretType::Primary => 0u32,
            TurretType::Secondary => 1u32,
            TurretType::Invalid => u32::MAX,
        };
        xfer.xfer_unsigned_int(&mut turret_sync_flag)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.data.turret_sync_flag = match turret_sync_flag {
                0 => TurretType::Primary,
                1 => TurretType::Secondary,
                _ => TurretType::Invalid,
            };
        }
        let mut attitude = self.data.attitude as u32;
        xfer.xfer_unsigned_int(&mut attitude)
            .map_err(|e| e.to_string())?;

        // Preserve the jitter flag already transferred above. The public
        // setter clears it; the original raw Xfer at AIUpdate.cpp:5160 does not.
        xfer.xfer_unsigned_int(&mut self.data.next_mood_check_time)
            .map_err(|e| e.to_string())?;

        let mut crate_created = self.crate_created;
        xfer.xfer_object_id(&mut crate_created)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.crate_created = crate_created;
        }

        if let Some(jet_ai) = self.jet_ai.as_mut() {
            Snapshotable::xfer(jet_ai, xfer)?;
        }

        Ok(true)
    }
    pub(super) fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.data.is_blocked {
            self.data.blocked_frames = self.data.blocked_frames.saturating_add(1);
        } else if self.data.blocked_frames > 1 {
            self.data.blocked_frames = 1;
        } else {
            self.data.blocked_frames = 0;
            self.data.blocked_and_stuck = false;
        }
        self.data.is_blocked = false;
        self.data.cur_max_blocked_speed = FAST_AS_POSSIBLE;

        if self.data.rappel_state.is_some() {
            self.update_rappel_state();
        }

        if self.data.demoralized_frames_left > 0 {
            let next = self.data.demoralized_frames_left.saturating_sub(1);
            self.set_demoralized(next);
        }

        if self.data.surrendered_frames_left > 0 {
            self.data.surrendered_frames_left = self.data.surrendered_frames_left.saturating_sub(1);
            if self.data.surrendered_frames_left == 0 {
                self.data.surrendered_player_index = None;
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
            supply_ai.update_with_ai(self, true);
            self.supply_truck_ai = Some(supply_ai);
        }
        if let Some(mut worker_ai) = self.worker_ai.take() {
            worker_ai.update_with_ai(self);
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

        let attack_adjust =
            self.get_mood_matrix_action_adjustment(crate::ai::MoodMatrixAction::Attack);
        let attack_ok = (attack_adjust & crate::ai::mood_matrix_adjustment::ACTION_OK) != 0;
        let has_primary = self.data.turret_primary_machine.is_some();
        let has_secondary = self.data.turret_secondary_machine.is_some();
        let linked = self.data.turrets_linked;
        let primary_enabled = self.data.turret_primary_enabled;
        let secondary_enabled = self.data.turret_secondary_enabled;
        let current_victim = self.get_current_victim();
        let original_victim_pos = self.data.original_victim_pos;
        let last_command_source = self.data.last_command_source;
        let which_turret = self.get_which_turret_for_cur_weapon();
        let primary_turn_rate = self.get_turret_turn_rate(crate::common::TurretType::Primary);
        let secondary_turn_rate = self.get_turret_turn_rate(crate::common::TurretType::Secondary);
        let state_id = self.get_current_state_id();
        let mood_target = self.get_next_mood_target_id(true, false);
        let ground_movement = self.is_doing_ground_movement();
        let mut can_turn_in_place = false;
        let mut ultra_accurate = false;
        let mut loco_appearance = None;
        self.with_cur_locomotor(&mut |loco| {
            can_turn_in_place = loco.template.min_speed == 0.0;
            ultra_accurate = loco.is_ultra_accurate();
            loco_appearance = Some(loco.get_appearance());
        });
        let next_mood_check = self.get_next_mood_check_time();
        let idle_mood_adjust =
            self.get_mood_matrix_action_adjustment(crate::ai::MoodMatrixAction::Idle);
        let crate_id = self.check_for_crate_to_pickup_id();
        let idle_attack = self.get_next_mood_target_id(true, true);
        let locomotor_speed = self.get_cur_locomotor_speed();
        let blocked_and_stuck = self.is_blocked_and_stuck();
        let has_path = self.get_path().is_some();
        let waiting_for_path = self.is_waiting_for_path();
        let has_path_destination = self.get_path_destination().is_some();
        let path_destination = self.get_path_destination();
        let is_moving = self.is_moving();
        let waypoint_queue_empty = self.is_waypoint_queue_empty();
        let hacking = self
            .get_hack_internet_ai_update_interface()
            .is_some_and(|hack| hack.is_hacking_packing_or_unpacking());
        let desired_speed = self.get_desired_speed();
        let is_idle = self.is_idle();
        // AIUpdate.cpp:4287-4344 reads the current owner/controller and
        // locomotor surfaces. Capture it before the owner's exclusive borrow;
        // publishing the fields below does not change any of those inputs.
        let mood_value = self.get_mood_matrix_value();
        if let Some(owner_id) = self.owner_object_id() {
            crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                owner.ai_fire_attack_ok = attack_ok;
                owner.ai_fire_turrets_linked = linked;
                owner.ai_fire_has_primary = has_primary;
                owner.ai_fire_has_secondary = has_secondary;
                owner.ai_fire_primary_enabled = primary_enabled;
                owner.ai_fire_secondary_enabled = secondary_enabled;
                owner.ai_fire_current_victim = current_victim;
                owner.ai_fire_original_victim_pos = original_victim_pos;
                owner.ai_fire_last_command_source = last_command_source;
                owner.ai_fire_pending_victim = None;
                owner.ai_fire_which_turret = which_turret;
                owner.ai_fire_state_id = state_id;
                owner.ai_fire_waiting_for_path = waiting_for_path;
                owner.ai_fire_has_path_destination = has_path_destination;
                owner.ai_fire_path_destination = path_destination;
                owner.ai_fire_is_moving = is_moving;
                owner.ai_fire_waypoint_queue_empty = waypoint_queue_empty;
                owner.ai_pending_completed_waypoint = None;
                owner.ai_pending_precise_z = None;
                owner.ai_pending_goal_path_index = None;
                owner.ai_fire_in_rappel = self.is_in_rappel_state();
                owner.ai_fire_combat_drop = self.is_doing_combat_drop();
                owner.ai_fire_hacking = hacking;
                owner.ai_fire_hack_known = true;
                owner.ai_fire_desired_speed = desired_speed;
                owner.ai_fire_primary_turn_rate = primary_turn_rate;
                owner.ai_fire_secondary_turn_rate = secondary_turn_rate;
                owner.ai_fire_ultra_accurate = ultra_accurate;
                owner.ai_fire_loco_appearance = loco_appearance;
                owner.ai_fire_mood_target =
                    (mood_target != crate::common::INVALID_ID).then_some(mood_target);
                owner.ai_fire_ground_movement = ground_movement;
                owner.ai_fire_is_idle = is_idle;
                owner.ai_fire_ultra_accurate = ultra_accurate;
                owner.ai_fire_next_mood_check = next_mood_check;
                owner.ai_fire_idle_mood_adjust = idle_mood_adjust;
                owner.ai_fire_crate_id = crate_id;
                owner.ai_fire_mood_value = mood_value;
                owner.ai_fire_idle_attack_target =
                    (idle_attack != crate::common::INVALID_ID).then_some(idle_attack);
                owner.ai_pending_move_crate = None;
                owner.ai_pending_attack_id = None;
                owner.ai_pending_attack_move = None;
                owner.ai_pending_attack_follow_waypoint = None;
                owner.ai_pending_attack_follow_as_team = false;
                owner.ai_pending_state_id = None;
                owner.ai_pending_clear_guard_target = false;
                owner.ai_fire_can_turn_in_place = can_turn_in_place;
                owner.ai_fire_locomotor_speed = locomotor_speed;
                owner.ai_fire_blocked_and_stuck = blocked_and_stuck;
                owner.ai_fire_has_path = has_path;
                owner.ai_fire_waiting_for_path = waiting_for_path;
                owner.ai_pending_path_goal = None;
                owner.ai_pending_ignore_id = None;
                owner.ai_pending_path_extra = None;
                owner.ai_pending_attack_path = None;
                owner.ai_pending_original_victim_pos = None;
                owner.ai_pending_clear_victim = false;
                owner.ai_pending_clear_goal = false;
                owner.ai_pending_set_victim = None;
                owner.ai_pending_path_through_units = None;
                owner.ai_pending_allow_invalid_position = None;
                owner.ai_pending_goal_id = None;
                owner.ai_pending_reset_mood = false;
                owner.ai_pending_victim_dead = false;
                owner.ai_pending_destroy_path = false;
                owner.ai_pending_clear_ignore = false;
                owner.ai_pending_goal_orientation = None;
                owner.ai_pending_goal_position = None;
                owner.ai_pending_goal_none = false;
                owner.ai_pending_turret_objects.clear();
                owner.ai_pending_turret_positions.clear();
            });
        }
        if let Some(state_machine) = self.ai_state_machine.clone() {
            if let Ok(mut machine) = state_machine.lock() {
                if self.data.ai_dead
                    && machine.get_current_state_id() != Some(AIStateType::Dead as u32)
                {
                    machine.clear();
                    let _ = machine.set_state(AIStateType::Dead as u32);
                    machine.lock();
                }
                let _ = machine.update_with_synchronous_commands(self, |driver, ai, terminal| {
                    let _ = ai.execute_terminal_attack_command(terminal, driver);
                });
            }
        }
        if let Some(owner_id) = self.owner_object_id() {
            let pending_state = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_pending_state_id.take());
            if let Some(Some(state_id)) = pending_state {
                self.enter_ai_state(state_id);
            }
        }
        if let Some(owner_id) = self.owner_object_id() {
            let pending = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_fire_pending_victim.take());
            if let Some(Some(victim)) = pending {
                self.notify_new_victim_chosen(victim);
            }
            let orders =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    (
                        std::mem::take(&mut owner.ai_pending_turret_objects),
                        std::mem::take(&mut owner.ai_pending_turret_positions),
                    )
                });
            if let Some((objects, positions)) = orders {
                for (turret, target, force) in objects {
                    self.set_turret_target_object(turret, target, force);
                }
                for (turret, pos) in positions {
                    self.set_turret_target_position(turret, &pos);
                }
            }
            let speed = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_pending_desired_speed.take());
            if let Some(Some(speed)) = speed {
                self.set_desired_speed(speed);
            }
            let clear_ignore =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let clear = owner.ai_pending_clear_ignore;
                    owner.ai_pending_clear_ignore = false;
                    clear
                });
            if clear_ignore == Some(true) {
                let _ = self.ignore_obstacle(None);
            }
            let victim_dead =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let dead = owner.ai_pending_victim_dead;
                    owner.ai_pending_victim_dead = false;
                    dead
                });
            if victim_dead == Some(true) {
                self.notify_victim_is_dead();
            }
            let destroy_path =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let destroy = owner.ai_pending_destroy_path;
                    owner.ai_pending_destroy_path = false;
                    destroy
                });
            if destroy_path == Some(true) {
                self.destroy_path();
            }
            let ending_move =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let ending = owner.ai_pending_ending_move;
                    owner.ai_pending_ending_move = false;
                    ending
                });
            if ending_move == Some(true) {
                self.friend_ending_move();
            }
            let completed = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_pending_completed_waypoint.take());
            if let Some(Some(id)) = completed {
                self.set_completed_waypoint_id(Some(id));
            }
            let precise_z = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_pending_precise_z.take());
            if let Some(Some(precise)) = precise_z {
                self.with_cur_locomotor_mut(&mut |loco| loco.set_precise_z_pos(precise));
            }
            let path_index = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_pending_goal_path_index.take());
            if let Some(Some(index)) = path_index {
                let _ = self.set_current_goal_path_index(index);
            }
            let drop =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let drop = owner.ai_pending_combat_drop;
                    let obj = owner.ai_pending_combat_drop_obj.take();
                    let pos = owner.ai_pending_combat_drop_pos.take();
                    owner.ai_pending_combat_drop = false;
                    (drop, obj, pos)
                });
            if let Some((true, obj, pos)) = drop {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::CombatDrop,
                    crate::common::CommandSourceType::FromAi,
                );
                params.obj = obj;
                if let Some(pos) = pos {
                    params.pos = pos;
                }
                let _ = self.execute_command(&params);
            }
            let hack =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let hack = owner.ai_pending_hack;
                    let source = owner.ai_pending_hack_source;
                    owner.ai_pending_hack = false;
                    (hack, source)
                });
            if let Some((true, source)) = hack {
                let params =
                    crate::ai::AiCommandParams::new(crate::ai::AiCommandType::HackInternet, source);
                let _ = self.execute_command(&params);
            }
            let idle =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let idle = owner.ai_pending_idle;
                    let source = owner.ai_pending_idle_source;
                    owner.ai_pending_idle = false;
                    (idle, source)
                });
            if let Some((true, source)) = idle {
                let params =
                    crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Idle, source);
                let _ = self.execute_command(&params);
            }
            let exit =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let exit = owner.ai_pending_exit.take();
                    let source = owner.ai_pending_exit_source;
                    let obj = owner.ai_pending_exit_obj.take();
                    (exit, source, obj)
                });
            if let Some((Some(instantly), source, obj)) = exit {
                let cmd = if instantly {
                    crate::ai::AiCommandType::ExitInstantly
                } else {
                    crate::ai::AiCommandType::Exit
                };
                let mut params = crate::ai::AiCommandParams::new(cmd, source);
                params.obj = obj;
                let _ = self.execute_command(&params);
            }
            let evacuate =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let evacuate = owner.ai_pending_evacuate;
                    owner.ai_pending_evacuate = false;
                    evacuate
                });
            if evacuate == Some(true) {
                let params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Evacuate,
                    crate::common::CommandSourceType::FromAi,
                );
                let _ = self.execute_command(&params);
            }
            let follow = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_pending_follow_pos.take());
            if let Some(Some(pos)) = follow {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::MoveToPosition,
                    crate::common::CommandSourceType::FromAi,
                );
                params.pos = pos;
                let _ = self.execute_command(&params);
            }
            let heal = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_pending_heal.take());
            if let Some(Some(target)) = heal {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::GetHealed,
                    crate::common::CommandSourceType::FromAi,
                );
                params.obj = Some(target);
                let _ = self.execute_command(&params);
            }
            let rappel =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let rappel = owner.ai_pending_rappel;
                    let obj = owner.ai_pending_rappel_obj.take();
                    let pos = owner.ai_pending_rappel_pos.take();
                    owner.ai_pending_rappel = false;
                    (rappel, obj, pos)
                });
            if let Some((true, obj, pos)) = rappel {
                let mut params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::RappelInto,
                    crate::common::CommandSourceType::FromAi,
                );
                params.obj = obj;
                if let Some(pos) = pos {
                    params.pos = pos;
                }
                let _ = self.execute_command(&params);
            }
            let path_goal = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| owner.ai_pending_path_goal.take());
            if let Some(Some(goal)) = path_goal {
                let _ = self.request_path(&goal, false);
            }
            let attack =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let ignore = owner.ai_pending_ignore_id.take();
                    let extra = owner.ai_pending_path_extra.take();
                    let path = owner.ai_pending_attack_path.take();
                    (ignore, extra, path)
                });
            if let Some((ignore, extra, path)) = attack {
                if let Some(id) = ignore {
                    let _ = self.ignore_obstacle(Some(id));
                }
                if let Some(extra) = extra {
                    let _ = self.set_path_extra_distance(extra);
                }
                if let Some((id, pos)) = path {
                    let _ = self.request_attack_path(id, &pos);
                }
            }
            let original_pos = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| {
                    owner.ai_pending_original_victim_pos.take()
                });
            if let Some(Some(stored)) = original_pos {
                self.set_original_victim_pos(stored);
            }
            let clears =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let set_victim = owner.ai_pending_set_victim.take();
                    let path_through = owner.ai_pending_path_through_units.take();
                    let victim = owner.ai_pending_clear_victim;
                    let goal = owner.ai_pending_clear_goal;
                    owner.ai_pending_clear_victim = false;
                    owner.ai_pending_clear_goal = false;
                    (set_victim, path_through, victim, goal)
                });
            if let Some((set_victim, path_through, victim, goal)) = clears {
                if let Some(id) = set_victim {
                    self.set_current_victim(Some(id));
                }
                if let Some(allow) = path_through {
                    let _ = self.set_can_path_through_units(allow);
                }
                let enter =
                    crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                        (
                            owner.ai_pending_allow_invalid_position.take(),
                            owner.ai_pending_goal_id.take(),
                        )
                    });
                if let Some((allow_invalid, goal_id)) = enter {
                    if let Some(allow) = allow_invalid {
                        let _ = self.set_allow_invalid_position(allow);
                    }
                    if let Some(id) = goal_id {
                        self.set_goal_object(Some(id));
                    }
                }
                if victim {
                    self.set_current_victim(None);
                }
                if goal {
                    self.set_goal_object(None);
                }
            }
            let reset_mood =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let reset = owner.ai_pending_reset_mood;
                    owner.ai_pending_reset_mood = false;
                    reset
                });
            if reset_mood == Some(true) {
                self.reset_next_mood_check_time();
            }
            let idle_cmd =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    (
                        owner.ai_pending_move_crate.take(),
                        owner.ai_pending_attack_id.take(),
                        owner.ai_pending_attack_move.take(),
                    )
                });
            if let Some((crate_id, attack_id, attack_move)) = idle_cmd {
                if let Some(id) = crate_id {
                    let mut params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::MoveToObject,
                        crate::common::CommandSourceType::FromAi,
                    );
                    params.obj = Some(id);
                    let _ = self.execute_command(&params);
                }
                if let Some(id) = attack_id {
                    let _ = self.ai_attack_object(id);
                }
                if let Some(pos) = attack_move {
                    let mut params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::AttackMoveToPosition,
                        crate::common::CommandSourceType::FromAi,
                    );
                    params.pos = pos;
                    params.int_value = crate::weapon::NO_MAX_SHOTS_LIMIT;
                    let _ = self.execute_command(&params);
                }
                if let Some((waypoint, as_team)) = crate::object::registry::OBJECT_REGISTRY
                    .with_object_mut(owner_id, |owner| {
                        let id = owner.ai_pending_attack_follow_waypoint.take();
                        let as_team = owner.ai_pending_attack_follow_as_team;
                        owner.ai_pending_attack_follow_as_team = false;
                        id.map(|waypoint| (waypoint, as_team))
                    })
                    .flatten()
                {
                    let cmd = if as_team {
                        crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam
                    } else {
                        crate::ai::AiCommandType::AttackFollowWaypointPath
                    };
                    let mut params = crate::ai::AiCommandParams::new(
                        cmd,
                        crate::common::CommandSourceType::FromAi,
                    );
                    params.waypoint = Some(waypoint);
                    params.int_value = crate::weapon::NO_MAX_SHOTS_LIMIT;
                    let _ = self.execute_command(&params);
                }
            }
            let clear_guard =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let clear = owner.ai_pending_clear_guard_target;
                    owner.ai_pending_clear_guard_target = false;
                    clear
                });
            if clear_guard == Some(true) {
                self.clear_guard_target_type();
            }
            let wake_path =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let wake = owner.ai_pending_wake_path;
                    owner.ai_pending_wake_path = false;
                    wake
                });
            if wake_path == Some(true) {
                self.set_queue_for_path_time(0);
            }
            let clear_move_out =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let clear = owner.ai_pending_clear_move_out;
                    owner.ai_pending_clear_move_out = false;
                    clear
                });
            if clear_move_out == Some(true) {
                self.clear_move_out_of_way();
            }
            let goals =
                crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner| {
                    let orientation = owner.ai_pending_goal_orientation.take();
                    let position = owner.ai_pending_goal_position.take();
                    let none = owner.ai_pending_goal_none;
                    owner.ai_pending_goal_none = false;
                    (orientation, position, none)
                });
            if let Some((orientation, position, none)) = goals {
                if let Some(angle) = orientation {
                    self.set_locomotor_goal_orientation(angle);
                }
                if let Some(pos) = position {
                    let _ = self.set_locomotor_goal_position_explicit(pos);
                }
                if none {
                    self.set_locomotor_goal_none();
                }
            }
        }
        if let Some(owner_id) = self.owner_object_id() {
            let produced = crate::object::registry::OBJECT_REGISTRY
                .with_object_mut(owner_id, |owner| {
                    std::mem::take(&mut owner.ai_pending_produced_exits)
                });
            if let Some(produced) = produced {
                for exit in produced {
                    match exit {
                        crate::object::PendingProducedExit::Quick(path) => {
                            self.do_quick_exit(&path);
                        }
                        crate::object::PendingProducedExit::Follow {
                            mut path,
                            ignore_id,
                            end,
                        } => {
                            let mut adjusted = end;
                            let _ = self.adjust_destination(&mut adjusted);
                            for point in &mut path {
                                if point.x == end.x && point.y == end.y && point.z == end.z {
                                    *point = adjusted;
                                }
                            }
                            let layer = crate::helpers::TheTerrainLogic::get()
                                .map(|terrain| terrain.get_layer_for_destination(&adjusted))
                                .unwrap_or(crate::common::PathfindLayerEnum::Ground);
                            let mut params = crate::ai::AiCommandParams::new(
                                crate::ai::AiCommandType::FollowPath,
                                crate::common::CommandSourceType::FromAi,
                            );
                            params.coords = path;
                            params.obj = Some(ignore_id);
                            let _ = self.execute_command(&params);
                            let _ = self.update_goal_position(&adjusted, layer);
                        }
                    }
                }
            }
        }

        self.apply_stored_locomotor_goal();
        self.finish_completed_movement_like_cpp();

        let now = TheGameLogic::get_frame();
        if self.data.queue_for_path_frame != 0 && now >= self.data.queue_for_path_frame {
            if let Ok(ai) = the_ai().read() {
                if let Some(pathfinder) = ai.pathfinder() {
                    if let Ok(mut pf) = pathfinder.write() {
                        let _ = pf.queue_for_path(self.unit_id);
                    }
                }
            }
            self.set_queue_for_path_time(0);
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
            if self.data.turret_primary_machine.is_some() {
                let mut turret = self
                    .data
                    .turret_primary_machine
                    .as_mut()
                    .and_then(TurretStateMachine::take_turret)
                    .expect("primary turret present");
                Self::update_single_turret(self, &mut turret);
                self.data
                    .turret_primary_machine
                    .as_mut()
                    .expect("primary turret slot")
                    .restore_turret(turret);
            }
            if self.data.turret_secondary_machine.is_some() {
                let mut turret = self
                    .data
                    .turret_secondary_machine
                    .as_mut()
                    .and_then(TurretStateMachine::take_turret)
                    .expect("secondary turret present");
                Self::update_single_turret(self, &mut turret);
                self.data
                    .turret_secondary_machine
                    .as_mut()
                    .expect("secondary turret slot")
                    .restore_turret(turret);
            }
        }

        if let Some(mut dock_machine) = self.dock_machine.take() {
            let update_result = dock_machine.update_with_ai(self);

            match update_result.convert_sleep_to_continue() {
                crate::state_machine::StateReturnType::Continue
                | crate::state_machine::StateReturnType::Blocked => {
                    self.dock_machine = Some(dock_machine);
                }
                _ => {
                    let _ = dock_machine.halt();
                    let _ = self.set_can_path_through_units(false);
                    if self.data.current_command == Some(crate::ai::AiCommandType::Dock) {
                        self.data.current_command = None;
                    }
                }
            }
        }
        let mut pending_params: Option<crate::ai::AiCommandParams> = None;
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if jet_ai.has_pending_command()
                && (self.data.current_command.is_none()
                    || self.data.current_command == Some(crate::ai::AiCommandType::Idle))
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
            && (self.data.current_command.is_none()
                || self.data.current_command == Some(crate::ai::AiCommandType::Idle))
            && !self
                .jet_ai
                .as_ref()
                .map(|jet| jet.has_pending_command())
                .unwrap_or(false)
        {
            self.data.pending_command = None;
        }

        let is_reloading = self.is_reloading();
        let mut queued_enter_command: Option<crate::ai::AiCommandParams> = None;
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            let takeoff = matches!(
                self.data.current_command,
                Some(crate::ai::AiCommandType::Exit)
                    | Some(crate::ai::AiCommandType::FollowExitProductionPath)
            );
            let landing = matches!(
                self.data.current_command,
                Some(crate::ai::AiCommandType::Enter) | Some(crate::ai::AiCommandType::Dock)
            );
            let taxiing = takeoff || landing;
            jet_ai.set_takeoff_in_progress(takeoff);
            jet_ai.set_landing_in_progress(landing);
            jet_ai.set_taxi_in_progress(taxiing);
            if taxiing {
                jet_ai.set_allow_air_loco(false);
            }
            jet_ai.set_has_pending_command(self.data.pending_command.is_some());
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
                    self.data.current_command,
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
                && self.data.current_locomotor_set == LocomotorSetType::Taxiing
            {
                let _ = self.choose_locomotor_set(LocomotorSetType::Normal);
            } else if !jet_ai.allow_air_loco()
                && self.data.current_locomotor_set != LocomotorSetType::Taxiing
            {
                let _ = self.choose_locomotor_set(LocomotorSetType::Taxiing);
            }
        }
        if let Some(params) = queued_enter_command {
            let _ = self.execute_command(&params);
        }
        self.apply_pending_assaults();
        Ok(())
    }
    pub(super) fn apply_bump_speed_limit(
        &mut self,
        mut desired_speed: Real,
        mut blocked: bool,
    ) -> Real {
        self.data.apply_bump_speed_limit(desired_speed, blocked)
    }
    pub(super) fn is_attacking(&self) -> bool {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(guard) = machine.lock() {
                let attacking = guard.is_in_attack_state();
                if self.owner.is_some() || attacking {
                    return attacking;
                }
            }
        }
        if self.owner.is_some() {
            return false;
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
        self.data.get_enter_target()
    }
    pub(super) fn set_demoralized(&mut self, duration_frames: UnsignedInt) {
        let prev = self.data.demoralized_frames_left;
        self.data.demoralized_frames_left = duration_frames;

        if (prev == 0 && self.data.demoralized_frames_left > 0)
            || (prev > 0 && self.data.demoralized_frames_left == 0)
        {
            self.evaluate_morale_bonus();
        }
    }
    pub(super) fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        self.data.get_which_turret_for_cur_weapon()
    }
    pub(super) fn get_turret_turn_rate(&self, turret: TurretType) -> f32 {
        self.data.get_turret_turn_rate(turret)
    }

    pub(super) fn get_which_turret_for_weapon_slot(&self, slot: WeaponSlotType) -> TurretType {
        self.data.get_which_turret_for_weapon_slot(slot)
    }
    pub(super) fn set_turret_enabled(&mut self, turret: TurretType, enabled: bool) {
        self.data.set_turret_enabled(turret, enabled)
    }
    pub(super) fn recenter_turret(&mut self, turret: TurretType) {
        self.data.recenter_turret(turret)
    }
    pub(super) fn is_turret_in_natural_position(&self, turret: TurretType) -> bool {
        self.data.is_turret_in_natural_position(turret)
    }
    pub(super) fn is_turret_enabled(&self, turret: TurretType) -> bool {
        self.data.is_turret_enabled(turret)
    }
    pub(super) fn get_turret_rot_and_pitch(&self, turret: TurretType) -> Option<(Real, Real)> {
        self.data.get_turret_rot_and_pitch(turret)
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
        self.data
            .is_weapon_slot_on_turret_and_aiming_at_target(slot, target)
    }
    pub(crate) fn load_post_process_path_cells(&mut self) {
        let Some((pos, layer, id, radius, bridge_end)) =
            get_unit_arc(self.unit_id).and_then(|unit| {
                let guard = unit.read().ok()?;
                let base = guard.base_arc();
                let object = base.read().ok()?;
                let layer = object.get_layer();
                let bridge_end = crate::terrain::get_terrain_logic()
                    .read()
                    .ok()
                    .map(|terrain| {
                        terrain.object_interacts_with_bridge_end(
                            &object,
                            crate::path::PathfindLayerEnum::from_u32(layer as u32),
                        )
                    })
                    .unwrap_or(false);
                Some((
                    *object.get_position(),
                    layer,
                    object.get_id(),
                    object.get_geometry_info().get_bounding_circle_radius(),
                    bridge_end,
                ))
            })
        else {
            return;
        };
        if !self.is_moving() {
            self.data.pathfind_goal_cell = ICoord2D::new(-1, -1);
            let _ = crate::ai::pathfind::update_goal_for_object(
                id,
                &pos,
                crate::ai::pathfind::PathfindLayerEnum::from_u32(layer as u32),
            );
            self.data.pathfind_cur_cell = ICoord2D::new(-1, -1);
            let immobile = get_unit_arc(self.unit_id)
                .and_then(|unit| {
                    let guard = unit.read().ok()?;
                    let base = guard.base_arc();
                    let object = base.read().ok()?;
                    Some(object.is_kind_of(crate::common::KindOf::Immobile))
                })
                .unwrap_or(false);
            if immobile || !self.is_doing_ground_movement() {
                return;
            }
            if let Ok(ai) = the_ai().read() {
                if let Some(pathfinder) = ai.pathfinder() {
                    if let Ok(mut pf) = pathfinder.write() {
                        let (cell_radius, center_in_cell) =
                            crate::ai::pathfind_complete::PathfindingSystem::compute_radius_and_center(
                                radius,
                            );
                        let cell_size = crate::ai::pathfind_astar::PATHFIND_CELL_SIZE_F;
                        let (nx, ny) = if center_in_cell {
                            (
                                (pos.x / cell_size).floor() as i32,
                                (pos.y / cell_size).floor() as i32,
                            )
                        } else {
                            (
                                (0.5 + pos.x / cell_size).floor() as i32,
                                (0.5 + pos.y / cell_size).floor() as i32,
                            )
                        };
                        let cell = crate::ai::pathfind_astar::GridCoord::new(nx, ny);
                        let pf_layer =
                            crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(layer as u32);
                        pf.update_pos_cells(
                            cell,
                            id,
                            pf_layer,
                            cell_radius,
                            center_in_cell,
                            bridge_end,
                        );
                        if pf.is_map_ready() {
                            self.data.pathfind_cur_cell = ICoord2D::new(nx, ny);
                        }
                    }
                }
            }
        } else if self.data.pathfind_goal_cell.x >= 0 && self.data.pathfind_goal_cell.y >= 0 {
            let cell = crate::ai::pathfind_astar::PATHFIND_CELL_SIZE_F;
            let goal = Coord3D::new(
                self.data.pathfind_goal_cell.x as f32 * cell + cell * 0.5,
                self.data.pathfind_goal_cell.y as f32 * cell + cell * 0.5,
                pos.z,
            );
            self.data.pathfind_goal_cell = ICoord2D::new(-1, -1);
            let _ = crate::ai::pathfind::update_goal_for_object(
                id,
                &goal,
                crate::ai::pathfind::PathfindLayerEnum::from_u32(layer as u32),
            );
        }
    }

    pub(super) fn is_moving(&self) -> bool {
        // C++ AIUpdate.cpp:3169-3180. Idle is false. A locomotor goal or
        // m_isMoving is true. The path existing is not enough.
        if self.is_idle() {
            return false;
        }
        if self.data.locomotor_goal_type != 0 || self.data.cpp_is_moving {
            return true;
        }
        false
    }
    fn idle_blocked_by_specialized_ai(&self) -> bool {
        self.jet_ai
            .as_ref()
            .is_some_and(|ai| ai.should_block_idle(self.data.pending_command))
            || self
                .hack_internet_ai
                .as_ref()
                .is_some_and(|ai| ai.has_pending_command())
    }

    pub(super) fn is_idle_in_machine(&self, machine: &AIStateMachine) -> bool {
        // C++ AIUpdate.cpp:3095-3103: classify the live state, including the
        // explicit Idle ID before its virtual idle classification.
        !self.idle_blocked_by_specialized_ai()
            && (machine.get_current_state_id() == Some(AIStateType::Idle as u32)
                || machine.is_idle())
    }

    fn legacy_unit_is_idle(unit: &Unit) -> bool {
        unit.movement_state == MovementState::Idle
            && !unit
                .path_following_state
                .as_ref()
                .is_some_and(|state| state.waiting_for_path)
            && unit.current_path.is_none()
            && unit.target_position.is_none()
    }

    pub(super) fn is_idle_in_legacy_unit(&self, unit: &Unit) -> bool {
        !self.idle_blocked_by_specialized_ai() && Self::legacy_unit_is_idle(unit)
    }

    pub(super) fn is_moving_in_machine(&self, machine: &AIStateMachine) -> bool {
        !self.is_idle_in_machine(machine)
            && (self.data.locomotor_goal_type != 0 || self.data.cpp_is_moving)
    }

    pub(super) fn is_idle(&self) -> bool {
        if self.idle_blocked_by_specialized_ai() {
            return false;
        }
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(machine) = machine.lock() {
                return self.is_idle_in_machine(&machine);
            }
        }
        // Residual compatibility when no authored machine exists. Ordinary
        // machine classification does not consult a second movement authority.
        get_unit_arc(self.unit_id)
            .and_then(|unit| {
                unit.read()
                    .ok()
                    .map(|unit| Self::legacy_unit_is_idle(&unit))
            })
            .unwrap_or(false)
    }
    pub(super) fn wake_up_and_attempt_to_target(&mut self) {
        if !self.is_idle() {
            return;
        }
        self.set_next_mood_check_time(crate::helpers::TheGameLogic::get_frame());
        self.data.randomly_offset_mood_check = true;
    }
    pub(super) fn take_random_mood_offset(&mut self) -> bool {
        self.data.take_random_mood_offset()
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
        self.data.set_attitude(attitude)
    }
    pub(super) fn get_attitude(&self) -> AIAttitudeType {
        self.data.get_attitude()
    }
    pub(super) fn is_idle_unrestricted(&self) -> bool {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(machine) = machine.lock() {
                return machine.get_current_state_id() == Some(AIStateType::Idle as u32)
                    || machine.is_idle();
            }
        }
        get_unit_arc(self.unit_id)
            .and_then(|unit| {
                unit.read()
                    .ok()
                    .map(|unit| Self::legacy_unit_is_idle(&unit))
            })
            .unwrap_or(false)
    }
    pub(super) fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        if let Some(path) = self.data.pending_safe_path.take() {
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
        self.data.set_current_goal_path_index(index)
    }
    pub(super) fn get_current_goal_path_index(&self) -> i32 {
        self.data.get_current_goal_path_index()
    }
    pub(super) fn set_can_path_through_units(
        &mut self,
        value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.data.set_can_path_through_units(value)
    }
    pub(super) fn get_can_path_through_units(&self) -> bool {
        self.data.get_can_path_through_units()
    }
    pub(super) fn is_blocked_and_stuck(&self) -> bool {
        const BLOCKED_RECOMPUTE_THRESHOLD: u32 = 60;
        if self.owner.is_some() {
            return self.data.blocked_and_stuck;
        }
        if self.data.blocked_and_stuck {
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
        self.data.set_is_blocked(blocked)
    }
    pub(super) fn set_blocked_and_stuck(&mut self, blocked: bool) {
        self.data.set_blocked_and_stuck(blocked)
    }
    pub(super) fn get_num_frames_blocked(&self) -> u32 {
        let mut frames = self.data.blocked_frames;
        if self.owner.is_some() {
            return frames;
        }
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
        self.data.current_path_snapshot = None;
        self.data.installed_path_layers.clear();
        self.data.waiting_for_path = false;
        self.data.is_attack_path = false;
        if let Some(unit) = get_unit_arc(self.unit_id) {
            if let Ok(mut guard) = unit.write() {
                guard.current_path = None;
                guard.path_following_state = None;
            }
        }
        self.set_locomotor_goal_none();
    }
    pub(super) fn clear_move_out_of_way(&mut self) {
        self.data.clear_move_out_of_way()
    }
}
