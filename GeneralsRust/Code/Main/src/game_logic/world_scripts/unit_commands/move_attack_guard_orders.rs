//! Move, attack, stop, guard and attack-move orders.

use super::*;

impl GameLogic {
    /// Stop the current attack, then queue a path. A refused path stays refused.
    pub fn unit_command_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        self.stamp_player_command_source(id);
        self.unit_command_move_owned(id, destination, true)
    }

    /// ScriptActions::doNamedMoveTo uses an unclipped CMD_FROM_SCRIPT goal.
    pub(crate) fn unit_command_script_move_to(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
    ) -> bool {
        if let Some(object) = self.objects.get_mut(&id) {
            object.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        }
        self.unit_command_move_owned(id, destination, false)
    }

    /// AIPlayer::recruitSpecificAITeam issues an unclipped CMD_FROM_AI move
    /// only when the recruited object exposes an AIUpdate interface.
    pub(crate) fn unit_command_ai_move_to(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
    ) -> bool {
        let Some(object) = self
            .objects
            .get_mut(&id)
            .filter(|object| object.has_ai_update_interface())
        else {
            return false;
        };
        object.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::MoveTo(destination),
        ) {
            return true;
        }
        self.private_move_to_position(id, destination)
    }

    pub(super) fn unit_command_move_owned(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        from_player: bool,
    ) -> bool {
        if !self.unit_can_move(id) {
            return false;
        }
        if self.apply_locked_transport_command(id) {
            return true;
        }
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::MoveTo(destination),
        ) {
            return true;
        }
        self.stop_attack_clearing_jet_targeter(id);
        let _ = self.note_move_to_request_path(id);
        let destination = if from_player {
            self.clip_player_goal(id, destination)
        } else {
            destination
        };
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.unit_ai_runtime.set_machine_goal_position(destination);
            unit.unit_ai_runtime.enter_normal_move_body(destination);
        }
        let ok = self.assign_unit_path(id, destination, &[]);
        if ok {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.unit_ai_runtime.cancel_tighten_path_check();
                unit.ignored_obstacle_id = None;
                end_hunt_on_player_parent_order(unit);
                unit.set_ai_state(
                    AIState::Moving,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            self.clear_unit_hunt_scan(id);
        }
        ok
    }

    /// C++ `setGoalPositionClipped` for `CMD_FROM_PLAYER` only.
    pub(crate) fn clip_player_goal(&self, id: ObjectId, mut pos: glam::Vec3) -> glam::Vec3 {
        use crate::game_logic::partition_manager::PARTITION_CELL_SIZE_RESIDUAL;
        let mut fudge = PARTITION_CELL_SIZE_RESIDUAL * 0.5;
        if let Some(unit) = self.objects.get(&id) {
            if unit.is_kind_of(crate::game_logic::KindOf::Aircraft)
                && unit.is_significantly_above_terrain()
            {
                fudge = fudge.max(unit.loco_preferred_height);
            }
        }
        let (min, max) = self.world_bounds();
        if max.x <= min.x + fudge * 2.0 || max.z <= min.z + fudge * 2.0 {
            return pos;
        }
        pos.x = pos.x.clamp(min.x + fudge, max.x - fudge);
        pos.z = pos.z.clamp(min.z + fudge, max.z - fudge);
        pos
    }

    /// Wave 232: path with waypoints after stop_attack (executor move_to residual).
    pub fn unit_command_move_to_waypoints(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        waypoints: &[glam::Vec3],
    ) -> bool {
        self.stamp_player_command_source(id);
        if self.objects.get(&id).is_none() {
            return false;
        }
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::MoveTo(destination),
        ) {
            return true;
        }
        self.stop_attack_clearing_jet_targeter(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            end_hunt_on_player_parent_order(unit);
        }
        self.clear_unit_hunt_scan(id);
        // C++ GameLogicDispatch.cpp:104-106 (doMoveTo): a player move order
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.release_weapon_lock(crate::game_logic::WeaponLockType::LockedTemporarily);
        }
        let _ = self.note_move_to_request_path(id);
        let destination = self.clip_player_goal(id, destination);
        let ok = self.assign_unit_path(id, destination, waypoints);
        if ok {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.unit_ai_runtime.cancel_tighten_path_check();
                unit.ignored_obstacle_id = None;
            }
        }
        ok
    }

    /// Wave 232: force-move — stop attack, path, force Moving state.
    pub fn unit_command_force_move_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        self.stamp_player_command_source(id);
        if self.objects.get(&id).is_none() {
            return false;
        }
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::MoveTo(destination),
        ) {
            return true;
        }
        self.stop_attack_clearing_jet_targeter(id);
        let _ = self.note_move_to_request_path(id);
        let destination = self.clip_player_goal(id, destination);
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.unit_ai_runtime.cancel_tighten_path_check();
            unit.ignored_obstacle_id = None;
            end_hunt_on_player_parent_order(unit);
            unit.set_ai_state(
                AIState::Moving,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
        }
        self.clear_unit_hunt_scan(id);
        true
    }

    /// Wave 230/232: attack target (records host attack log).
    pub fn unit_command_attack(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        self.stamp_player_command_source(id);
        self.unit_command_attack_with_source(id, target_id, true, false, &mut Default::default())
    }

    /// SCRIPT uses the same owned attack algorithm without player shroud gates.
    pub(crate) fn unit_command_script_attack(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        if let Some(unit) = self.host_object_mut(id) {
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        }
        self.unit_command_attack_with_source(id, target_id, false, false, &mut Default::default())
    }

    pub(crate) fn unit_command_attack_with_effects(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.stamp_player_command_source(id);
        self.unit_command_attack_with_source(id, target_id, true, false, effects)
    }
    pub(crate) fn unit_command_script_attack_with_effects(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if let Some(unit) = self.host_object_mut(id) {
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        }
        self.unit_command_attack_with_source(id, target_id, false, false, effects)
    }

    pub(super) fn unit_command_attack_with_source(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
        from_player: bool,
        prevalidated: bool,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Attack(target_id),
        ) {
            return Ok(true);
        }
        // This is the authority boundary for a player AttackObject order.
        // Do not merely stamp `target`: C++ WeaponSet validates the concrete
        // target relationship/status and every available Weapon.ini Anti*
        // mask before an attack state is allowed to begin.
        if !prevalidated
            && !matches!(
                self.get_able_to_attack_specific_object(
                    id,
                    target_id,
                    AbleToAttackType::NewTarget,
                    from_player,
                ),
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
            )
        {
            return Ok(false);
        }
        let (can_attack, switch_away) = {
            let Some(unit) = self.objects.get(&id) else {
                return Ok(false);
            };
            (
                unit.can_attack(),
                unit.target.is_some_and(|prev| prev != target_id),
            )
        };
        if !can_attack {
            return Ok(false);
        }
        if switch_away {
            self.drop_jet_targeters_on_attack_exit(id);
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        unit.mark_jet_command_for_reload_interrupt(false);
        unit.clear_guard_chase();
        end_hunt_on_player_parent_order(unit);
        unit.set_force_attack(false);
        drop(unit);
        // C++ privateAttackObject enters the attack machine and requests a
        // chase path. A weaponless order that already passed the command
        // legality check still stamps Attacking so existing callers keep it.
        let Some((source_birth, target_birth)) = self
            .objects
            .get(&id)
            .zip(self.objects.get(&target_id))
            .map(|(source, target)| {
                (
                    std::sync::Arc::clone(&source.visual_draw_identity_token),
                    std::sync::Arc::clone(&target.visual_draw_identity_token),
                )
            })
        else {
            return Ok(false);
        };
        let engaged = self.private_attack_object_with_effects(id, target_id, -1, effects)?;
        if !self.objects.get(&id).is_some_and(|source| {
            std::sync::Arc::ptr_eq(&source.visual_draw_identity_token, &source_birth)
        }) || !self.objects.get(&target_id).is_some_and(|target| {
            std::sync::Arc::ptr_eq(&target.visual_draw_identity_token, &target_birth)
        }) {
            return Ok(false);
        }
        if !engaged {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_target_with_operation(
                    Some(target_id),
                    &mut self.attack_events,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
            } else {
                return Ok(false);
            }
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if unit.ignored_obstacle_id.is_some_and(|old| old != target_id) {
                unit.ignored_obstacle_id = None;
            }
        }
        self.clear_unit_hunt_scan(id);
        if !engaged {
            self.admit_jet_targeter_with_effects(target_id, id, true, effects)?;
        }
        self.assault_transport_on_player_attack(id);
        Ok(true)
    }

    /// Wave 230/232: force-attack target (records host attack log).
    pub fn unit_command_force_attack(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_force_attack_with_effects(id, target_id, &mut Default::default())
    }
    /// AIGroup's SpawnBehavior dispatch calls the child's AI directly, without
    /// the passenger/command-translator weapon eligibility preflight.
    pub(crate) fn unit_command_spawn_force_attack_with_effects(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if !self.objects.contains_key(&target_id) {
            return Ok(false);
        }
        self.stamp_player_command_source(id);
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Attack(target_id),
        ) {
            return Ok(true);
        }
        let Some(switch_away) = self.objects.get(&id)
            .map(|unit| unit.target.is_some_and(|previous| previous != target_id))
        else {
            return Ok(false);
        };
        if switch_away {
            self.drop_jet_targeters_on_attack_exit(id);
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        unit.mark_jet_command_for_reload_interrupt(false);
        unit.clear_guard_chase();
        end_hunt_on_player_parent_order(unit);
        unit.set_target_with_operation(
            Some(target_id),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut effects.reborrow_for_object(&mut self.partition_manager),
        );
        unit.set_force_attack(true);
        if unit.weapon.is_some() {
            unit.set_max_shots_to_fire(-1);
        }
        if unit.ignored_obstacle_id.is_some_and(|old| old != target_id) {
            unit.ignored_obstacle_id = None;
        }
        self.clear_unit_hunt_scan(id);
        self.admit_jet_targeter_with_effects(target_id, id, true, effects)?;
        Ok(true)
    }
    pub(crate) fn unit_command_force_attack_with_effects(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Attack(target_id),
        ) {
            return Ok(true);
        }
        // C++ force attack still uses WeaponSet target legality: it changes
        // relationship/force handling, not UNATTACKABLE, MASKED, contained,
        // stealth, or exact Weapon.ini anti-mask rules.
        if !matches!(
            self.get_able_to_attack_specific_object(
                id,
                target_id,
                AbleToAttackType::NewTargetForced,
                true,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return Ok(false);
        }
        let (can_attack, switch_away) = {
            let Some(unit) = self.objects.get(&id) else {
                return Ok(false);
            };
            (
                unit.can_attack(),
                unit.target.is_some_and(|prev| prev != target_id),
            )
        };
        if !can_attack {
            return Ok(false);
        }
        if switch_away {
            self.drop_jet_targeters_on_attack_exit(id);
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        unit.mark_jet_command_for_reload_interrupt(false);
        unit.clear_guard_chase();
        end_hunt_on_player_parent_order(unit);

        unit.set_target_with_operation(
            Some(target_id),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut effects.reborrow_for_object(&mut self.partition_manager),
        );
        unit.set_force_attack(true);
        drop(unit);
        if let Some(unit) = self.objects.get_mut(&id) {
            if unit.ignored_obstacle_id.is_some_and(|old| old != target_id) {
                unit.ignored_obstacle_id = None;
            }
        }
        self.clear_unit_hunt_scan(id);
        self.admit_jet_targeter_with_effects(target_id, id, true, effects)?;
        Ok(true)
    }

    /// Wave 230/232: full player stop (idle + clear guard/target/force + logs).
    /// C++ `AIUpdateInterface::privateIdle` then `aiIdle` every contained
    /// rider (`AIUpdate.cpp:3067-3090`) so Humvee/Chinook Stop parks
    /// PassengersAllowedToFire infantry.
    pub fn unit_command_stop(&mut self, id: ObjectId) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_stop_with_effects(id, &mut Default::default())
    }

    pub(crate) fn unit_command_stop_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_stop_with_source_with_effects(
            id,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER,
            effects,
        )
    }

    pub(crate) fn unit_command_stop_with_source(
        &mut self,
        id: ObjectId,
        source: u32,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_stop_with_source_with_effects(id, source, &mut Default::default())
    }

    pub(crate) fn unit_command_stop_with_source_with_effects(
        &mut self,
        id: ObjectId,
        source: u32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        // JetAIUpdate::doCommand handles airborne Idle before the base
        // AIUpdateInterface::privateIdle path. It clears the retained machine,
        // enters RETURNING_FOR_LANDING, and does not recursively idle riders.
        // Resolve the selected provider's typed module data, not KindOf/name.
        let jet_idle = self.objects.get(&id).and_then(|unit| {
            unit.thing()
                .template
                .receiving_client_template
                .as_ref()
                .filter(|template| template.receiving_ai_is_jet())
                .map(|_| {
                    (
                        unit.jet_ai.takeoff_in_progress
                            || unit.jet_ai.landing_in_progress
                            || unit.airfield_rearm_ready_frame.is_some(),
                        unit.status.airborne_target && !Self::object_is_produced_at_helipad(unit),
                    )
                })
        });
        if let Some((pending, airborne)) = jet_idle {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.jet_ai.pending_resume =
                    crate::game_logic::object::HostJetPendingResume::AiIdle;
                if pending {
                    unit.jet_ai.has_pending_command = true;
                    return Ok(true);
                }
                unit.jet_ai.has_pending_command = false;
                unit.jet_ai.allow_interrupt_for_reload = false;
            }
            if airborne {
                return self.clear_base_ai_for_jet_return_owned(id, effects);
            }
        }

        // C++ AIGroup::groupIdle / AICommandInterface::idle: stop clears the
        // latched target and guard anchors up front, then runs the idle state.
        self.drop_jet_targeters_on_attack_exit(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_guard_position(
                None,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.end_guard_retaliate_state();
            unit.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_movement_goal_object(None);
        }
        let occupants = self
            .objects
            .get(&id)
            .filter(|unit| !unit.is_kind_of(KindOf::Projectile))
            .map(|unit| unit.contained_units())
            .unwrap_or_default();
        let ok = self.unit_command_stop_self_with_effects(id, effects)?;
        if ok {
            let jet_airborne_return = self.objects.get(&id).is_some_and(|unit| {
                unit.thing()
                    .template
                    .receiving_client_template
                    .as_ref()
                    .is_some_and(|template| template.receiving_ai_is_jet())
                    && unit.retained_ai_root_state() == AIState::JetReturningForLanding
            });
            if !jet_airborne_return {
                if let Some(unit) = self.objects.get_mut(&id) {
                    unit.last_command_source = source;
                }
                for occ in occupants {
                    if occ != id {
                        self.unit_command_stop_with_source_with_effects(occ, source, effects)?;
                    }
                }
            }
        }
        Ok(ok)
    }
    pub(super) fn unit_command_stop_self_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        // C++ AssaultTransportAIUpdate AICMD_IDLE: retrieveMembers then reset.
        self.assault_transport_on_player_idle(id);
        self.drop_jet_targeters_on_attack_exit(id);
        if self.flight_deck_ai_do_command_with_effects(
            id,
            crate::game_logic::host_flight_deck::HostFlightDeckCommand::Idle,
            None,
            None,
            effects,
        ) {
            return Ok(true);
        }

        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Stop,
        ) {
            return Ok(true);
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        let needs_idle_entry = unit.ai_state() != AIState::Idle;
        unit.pending_move = None;
        unit.stop_moving_for_owned_idle_entry(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.ignored_obstacle_id = None;
        unit.clear_target(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_force_attack(false);
        unit.set_guard_position(
            None,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_guard_target(
            None,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );

        unit.end_guard_retaliate_state();
        unit.set_hunting(false);
        // C++ SupplyTruckAIUpdate::privateIdle(CMD_FROM_PLAYER) setForceBusyState.
        // Workers omit the latch (WorkerAIUpdate.cpp:516-526).
        if unit.thing().template.supply_truck_metadata.is_some() && !unit.is_kind_of(KindOf::Worker)
        {
            unit.supply_truck_state = crate::game_logic::SupplyTruckState::Idle;
            unit.supply_truck_force_pending = false;
        }
        drop(unit);
        if needs_idle_entry && !self.enter_base_ai_idle_owned(id, effects)? {
            return Ok(false);
        }
        Ok(true)
    }

    pub fn unit_command_guard_position(&mut self, id: ObjectId, pos: glam::Vec3) -> bool {
        self.unit_command_guard_position_with_effects(id, pos, &mut Default::default())
    }

    pub(crate) fn unit_command_guard_position_with_effects(
        &mut self,
        id: ObjectId,
        pos: glam::Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let prior = self
            .objects
            .get(&id)
            .map(|unit| (unit.last_command_source, unit.movement_goal_object()));
        self.stamp_player_command_source(id);
        if self.flight_deck_ai_do_command_with_effects(
            id,
            crate::game_logic::host_flight_deck::HostFlightDeckCommand::GuardPosition,
            None,
            Some(pos),
            effects,
        ) {
            return true;
        }

        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Stop,
        ) {
            return true;
        }
        // Ordinary guard publishes its command source only after exit.
        // The special AI overrides above retain their existing admission path.
        if let (Some(unit), Some((source, goal))) = (self.objects.get_mut(&id), prior) {
            unit.last_command_source = source;
            unit.set_movement_goal_object(goal);
        }
        let was_retaliating = self
            .objects
            .get(&id)
            .is_some_and(|unit| unit.ai_state() == AIState::GuardRetaliating);
        if was_retaliating {
            if !matches!(self.clear_base_ai_machine_owned(id, effects), Ok(true)) {
                return false;
            }
        }
        if !self.objects.get(&id).is_some_and(|unit| {
            unit.is_mobile_for_ai_command() && !unit.is_kind_of(KindOf::Projectile)
        }) {
            return false;
        }
        // privateGuardPosition stages both its type and destination before
        // clearing the outgoing machine.
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.unit_ai_runtime
                .stage_guard_target_type(crate::game_logic::object::GuardTargetType::Location);
            unit.unit_ai_runtime.set_raw_guard_position(Some(pos));
        }
        if !matches!(self.clear_base_ai_machine_owned(id, effects), Ok(true)) {
            return false;
        }
        self.stamp_player_command_source(id);
        {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            release_hunt_temp_lock_when_entering_guard(unit);
            unit.unit_ai_runtime.clear_guard();
            unit.set_guard_position(
                Some(pos),
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.set_hunting(false);
            unit.mark_jet_command_for_reload_interrupt(true);
        }
        self.return_guard_to_post(id);
        true
    }

    /// C++ `Pathfinder::adjustDestination` (AIPathfind.cpp:5331), not the weaker
    /// `adjustToPossibleDestination` spiral.
    pub(crate) fn adjust_guard_goal(&self, unit_id: ObjectId, dest: &mut glam::Vec3) {
        let Some(obj) = self.objects.get(&unit_id) else {
            return;
        };
        let surfaces = if obj.locomotor_surfaces != 0 {
            obj.locomotor_surfaces
        } else {
            crate::game_logic::object::LOCO_SURFACE_GROUND
        };
        let grid = &self.pathfinding_system.grid;
        let cell = grid.world_to_grid(*dest);
        let raw_layer = obj.pathfind_layer as u32;
        let layer = if raw_layer <= 1 {
            gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground
        } else {
            let parsed = gamelogic::ai::pathfind_astar::PathfindLayerEnum::from_u32(raw_layer);
            if parsed == gamelogic::ai::pathfind_astar::PathfindLayerEnum::Invalid {
                gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground
            } else {
                parsed
            }
        };
        let Some(adjusted) = ({
            let ground_owner = self.world_services.terrain();
            let ground_loan = ground_owner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                &ground_loan,
                self.map_loaded,
                self.terrain.as_ref(),
                self.pathfinding_height_samples.as_ref(),
                &*grid,
            );
            grid.adjust_destination_on_layer(
                cell,
                surfaces,
                false,
                400,
                None,
                0,
                layer,
                &ground_source,
            )
        }) else {
            return;
        };
        let world = ({
            let ground_owner = self.world_services.terrain();
            let ground_loan = ground_owner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                &ground_loan,
                self.map_loaded,
                self.terrain.as_ref(),
                self.pathfinding_height_samples.as_ref(),
                &*grid,
            );
            grid.grid_to_world(adjusted, &ground_source)
        });
        dest.x = world.x;
        dest.z = world.z;
    }

    pub fn unit_command_guard_object(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        self.unit_command_guard_object_owned(id, target_id, true)
    }

    pub(crate) fn unit_command_script_guard_object(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        self.unit_command_guard_object_owned(id, target_id, false)
    }

    pub(super) fn unit_command_guard_object_owned(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
        player: bool,
    ) -> bool {
        let prior = self
            .objects
            .get(&id)
            .map(|unit| (unit.last_command_source, unit.movement_goal_object()));
        if player {
            self.stamp_player_command_source(id);
        } else if let Some(unit) = self.objects.get_mut(&id) {
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        }
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Stop,
        ) {
            return true;
        }
        if let (Some(unit), Some((source, goal))) = (self.objects.get_mut(&id), prior) {
            unit.last_command_source = source;
            unit.set_movement_goal_object(goal);
        }
        if self
            .objects
            .get(&id)
            .is_some_and(|unit| unit.ai_state() == AIState::GuardRetaliating)
            && !matches!(
                self.clear_base_ai_machine_owned(id, &mut Default::default()),
                Ok(true)
            )
        {
            return false;
        }
        if !self.objects.get(&id).is_some_and(|unit| {
            unit.is_mobile_for_ai_command() && !unit.is_kind_of(KindOf::Projectile)
        }) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.unit_ai_runtime
                .stage_guard_target_type(crate::game_logic::object::GuardTargetType::Object);
            unit.unit_ai_runtime.set_raw_guard_target(Some(target_id));
        }
        if !matches!(
            self.clear_base_ai_machine_owned(id, &mut Default::default()),
            Ok(true)
        ) {
            return false;
        }
        if player {
            self.stamp_player_command_source(id);
        } else if let Some(unit) = self.objects.get_mut(&id) {
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        }
        {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            release_hunt_temp_lock_when_entering_guard(unit);
            unit.unit_ai_runtime.clear_guard();
            unit.set_guard_target(
                Some(target_id),
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.set_hunting(false);
            unit.mark_jet_command_for_reload_interrupt(true);
        }
        self.return_guard_to_post(id);
        true
    }

    /// Wave 231/232: attack-move via path + AttackMoving state.
    pub fn unit_command_attack_move_to(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_attack_move_to_with_effects(id, destination, &mut Default::default())
    }

    pub(crate) fn unit_command_attack_move_to_with_effects(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_attack_move_to_ex_with_effects(id, destination, -1, effects)
    }

    /// Wave 232: attack-move with max-shots + attack-path flags (executor residual).
    pub fn unit_command_attack_move_to_ex(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        max_shots: i32,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_attack_move_to_ex_with_effects(
            id,
            destination,
            max_shots,
            &mut Default::default(),
        )
    }

    pub(crate) fn unit_command_attack_move_to_ex_with_effects(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        max_shots: i32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.stamp_player_command_source(id);
        self.unit_command_attack_move_owned_with_effects(id, destination, max_shots, effects)
    }

    /// Same movement algorithm for SCRIPT, without converting it to PLAYER.
    pub(crate) fn unit_command_script_attack_move_to(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_script_attack_move_to_with_effects(
            id,
            destination,
            &mut Default::default(),
        )
    }

    pub(crate) fn unit_command_script_attack_move_to_with_effects(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if let Some(unit) = self.host_object_mut(id) {
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
            unit.set_movement_goal_object(None);
        }
        self.unit_command_attack_move_owned_with_effects(id, destination, -1, effects)
    }

    pub(super) fn unit_command_attack_move_owned(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        max_shots: i32,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_attack_move_owned_with_effects(
            id,
            destination,
            max_shots,
            &mut Default::default(),
        )
    }

    pub(super) fn unit_command_attack_move_owned_with_effects(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        max_shots: i32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if self.unit_sleep_mood_blocks_ai_command(id) {
            return Ok(false);
        }

        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::MoveTo(destination),
        ) {
            return Ok(true);
        }
        if self.flight_deck_ai_do_command_with_effects(
            id,
            crate::game_logic::host_flight_deck::HostFlightDeckCommand::AttackMoveToPosition,
            None,
            Some(destination),
            effects,
        ) {
            return Ok(true);
        }

        // C++ privateAttackMoveToPosition returns before setState when
        // isMobile() is false (KINDOF_IMMOBILE or disabled). A deploy pack
        // block is still mobile.
        // It then enters AI_ATTACK_MOVE_TO whatever the weapons
        // (AIUpdate.cpp:3558-3572); a weaponless mover simply never acquires.
        let (alive, mobile) = match self.objects.get(&id) {
            Some(unit) => (
                unit.is_alive(),
                !unit.is_kind_of(crate::game_logic::KindOf::Immobile) && !unit.is_disabled(),
            ),
            None => return Ok(false),
        };
        if !alive || !mobile {
            return Ok(false);
        }
        self.drop_jet_targeters_on_attack_exit(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            unit.set_force_attack(false);
            unit.set_max_shots_to_fire(max_shots);
            unit.attack_move_retry_count = 5;
            unit.attack_move_sleep_until = 0;
            let landed_chinook = unit.chinook_ai.as_ref().is_some_and(|ai| {
                ai.flight_status
                    == crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
            });
            unit.is_final_goal = !unit.is_parachuting()
                && !landed_chinook
                && !(unit.chinook_ai.is_some() && unit.allow_invalid_position)
                && unit.adjust_destinations();
        }
        let destination = self.clip_player_goal(id, destination);
        let path_ok = self.assign_unit_path(id, destination, &[]);
        // C++ does not walk a raw dest line when pathing fails (hq-65aus).
        // assign false is not always onEnter failure: a queue, a deploy
        // block, !can_move, and an already-there route stay out of AI_IDLE.
        // Only computePath's closest-path miss matches STATE_FAILURE.
        if self.objects.get(&id).is_none() {
            return Ok(false);
        }
        let hard_miss = !path_ok
            && self
                .objects
                .get(&id)
                .is_some_and(|unit| unit.path_timestamp == self.frame && unit.retry_path);
        let deploy_blocked = !path_ok
            && self.objects.get(&id).is_some_and(|unit| {
                unit.deploy_style
                    .as_ref()
                    .is_some_and(|style| !style.is_ready_to_move())
            });
        let start = self
            .objects
            .get(&id)
            .map(|unit| unit.get_position())
            .unwrap_or(destination);
        let already_there = !path_ok && Self::route_is_already_there(start, destination, &[]);
        let mut entered_idle = false;
        if let Some(unit) = self.objects.get_mut(&id) {
            end_hunt_on_player_parent_order(unit);
            if hard_miss {
                unit.requested_destination = None;
                unit.retry_path = false;
                if unit.ai_state() != AIState::Idle {
                    entered_idle = true;
                }
            } else if path_ok || deploy_blocked || already_there {
                if path_ok {
                    unit.ignored_obstacle_id = None;
                }
                unit.set_auto_acquire_when_idle(true);
                unit.requested_destination = Some(destination);
                if unit.ai_state() != AIState::AttackMoving {
                    unit.set_ai_state(
                        AIState::AttackMoving,
                        self.frame,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                }
                unit.unit_ai_runtime.set_attack_move_goal(destination);
            }
            drop(unit);
            if entered_idle && !self.enter_base_ai_idle_owned(id, effects)? {
                return Err(gamelogic::GameLogicError::Configuration(
                    "AttackMove failure lost its receiving actor during Idle entry".into(),
                ));
            }
            if self.objects.get(&id).is_some_and(|unit| {
                unit.unit_ai_runtime.root_state() == AIState::AttackMoving
                    && unit.unit_ai_runtime.parent_state().is_none()
            }) && !self.enter_attack_move_idle_child_owned(id)
            {
                return Err(gamelogic::GameLogicError::Configuration(
                    "AttackMove could not enter its child Idle body".into(),
                ));
            }
            if !hard_miss {
                self.clear_unit_hunt_scan(id);
                self.assault_transport_on_player_attack_move(id, destination);
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Wave 232: promote unit onto attack-move path after waypoint follow.
    pub fn unit_command_promote_attack_path(&mut self, id: ObjectId) -> bool {
        let can_attack = self
            .objects
            .get(&id)
            .map(|u| u.is_alive() && (u.can_attack() || u.weapon.is_some()))
            .unwrap_or(false);
        if !can_attack {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if let Some(slot) = unit.find_waypoint_following_capable_weapon_slot() {
                unit.set_active_weapon_slot(slot);
            }
            unit.attack_move_retry_count = 5;
            unit.attack_move_sleep_until = 0;
            if unit.requested_destination.is_none() {
                unit.requested_destination = unit
                    .movement
                    .path
                    .last()
                    .copied()
                    .or(unit.movement.target_position);
            }
            if unit.ai_state() != AIState::AttackMoving {
                unit.set_ai_state(
                    AIState::AttackMoving,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            if let Some(goal) = unit.requested_destination {
                unit.unit_ai_runtime.set_attack_move_goal(goal);
            }
            drop(unit);
            if self.objects.get(&id).is_some_and(|unit| {
                unit.unit_ai_runtime.root_state() == AIState::AttackMoving
                    && unit.unit_ai_runtime.parent_state().is_none()
            }) && !self.enter_attack_move_idle_child_owned(id)
            {
                return false;
            }
            return true;
        }
        false
    }
}
