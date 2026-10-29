//! Named ScriptActions operate synchronously on the driving Main world.
use super::*;
use gamelogic::scripting::engine::ScriptNamedCommand;

impl GameLogic {
    pub(crate) fn owned_named_script_object(
        &self,
        name: &str,
        this_object: Option<ObjectId>,
    ) -> Option<ObjectId> {
        // CPP ScriptEngine::getUnitNamed: exact authored name, or the calling
        // object (before condition object) supplied by the borrowed engine.
        if name == gamelogic::scripting::core::THIS_OBJECT {
            return this_object.filter(|id| self.host_object(*id).is_some());
        }
        self.host_object_id_by_script_name(name)
    }

    /// CPP ScriptActions.cpp1966 chooses NORMAL even if AI rejects the order;
    /// privateHunt.cpp3611 clears and reenters Hunt without leaving the group.
    pub(super) fn apply_owned_named_hunt(&mut self, name: &str, this_object: Option<ObjectId>) {
        let Some(id) = self.owned_named_script_object(name, this_object) else {
            return;
        };
        if !self
            .host_object(id)
            .is_some_and(|o| o.has_ai_update_interface())
        {
            return;
        }
        self.apply_unit_locomotor_set(id, "normal");
        if !self.owned_named_ai_command_admitted(id) {
            return;
        }

        self.drop_jet_targeters_on_attack_exit(id);
        self.clear_unit_movement_path(id);
        // The existing Rust stream is owned by this session. Initialize at
        // onEnter, rather than adopting an ambient stream on the next tick.
        let deadline = self.frame.wrapping_add(self.logic_random.next_u32() % 31);
        let (unit, audio_events) = self
            .host_object_and_audio_events_mut(id)
            .expect("admitted Hunt unit");
        Self::clear_owned_named_ai_goal(
            unit,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.last_command_source =
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        unit.set_auto_acquire_when_idle(true);
        unit.set_hunting(true);
        unit.set_ai_state(
            AIState::Patrolling,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.mark_jet_command_for_reload_interrupt(true);
        unit.set_status_moving(false);
        unit.unit_ai_runtime.set_hunt_scan_deadline(Some(deadline));
    }

    /// CPP AIUpdate.cpp2572/4014: SCRIPT admission uses the simulation
    /// controller and Object::isMobile, independently of display locality.
    fn owned_named_ai_command_admitted(&self, id: ObjectId) -> bool {
        let Some(unit) = self.host_object(id) else {
            return false;
        };
        let sleeping_ai = unit.ai_attitude()
            == crate::game_logic::host_strategy_center::HostAiAttitude::Sleep
            && unit
                .owner_player_id
                .and_then(|pid| self.players.get(&pid))
                .is_none_or(|p| !p.is_human);
        unit.is_alive()
            && !unit.status.effectively_dead
            && !sleeping_ai
            && unit.is_mobile_for_ai_command()
            && !unit.is_kind_of(KindOf::Projectile)
    }

    /// The bounded onExit phase shared by commands that clear the current
    /// AI machine. It borrows only the canonical object, with no registry lookup.
    pub(super) fn clear_owned_named_ai_goal(
        unit: &mut Object,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        if unit.is_hunting() || matches!(unit.ai_state(), AIState::Patrolling) {
            unit.release_weapon_lock(WeaponLockType::LockedTemporarily);
        }
        // Deleting the parent also exits its active attack submachine
        // (CPP AIAttackState::onExit5668), including Hunt -> Hunt reentry.
        if matches!(
            unit.ai_state(),
            AIState::Attacking
                | AIState::AttackingGround
                | AIState::AttackMoving
                | AIState::GuardRetaliating
        ) {
            unit.set_status_attacking(false);
            unit.set_status_firing_weapon(false);
            unit.set_status_aiming_weapon(false);
            unit.set_status_ignoring_stealth(false);
            unit.model_condition_bits &=
                !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_ATTACKING);
            unit.clear_leech_range_mode_for_all_weapons();
            unit.set_turret_target_object(None, false);
        }
        unit.clear_pending_waypoint_labels();
        unit.requested_destination = None;
        unit.waiting_for_path = false;
        unit.pending_move = None;
        unit.clear_target(audio);
        unit.set_target_location(None, audio);
        unit.set_force_attack(false);
        unit.set_guard_position(None, audio);
        unit.set_guard_target(None, audio);
        unit.end_guard_retaliate(audio);
    }

    /// SCRIPT groupIdle keeps group/locomotor preparation intact. Ordinary
    /// player Stop has additional effects and cannot stand in for this command.
    pub(super) fn apply_owned_script_idle(&mut self, id: ObjectId) {
        self.apply_owned_idle(
            id,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT,
        );
    }

    /// The same privateIdle transition with an explicit original command source.
    pub(in crate::game_logic) fn apply_owned_idle(&mut self, id: ObjectId, source: u32) {
        self.apply_owned_idle_member(id, source, &mut std::collections::HashSet::new());
    }

    fn apply_owned_idle_member(
        &mut self,
        id: ObjectId,
        source: u32,
        visited: &mut std::collections::HashSet<ObjectId>,
    ) {
        // Malformed/stale containment links cannot reenter an already visited
        // object. Effects remain depth first at the original synchronous point.
        if !visited.insert(id) {
            return;
        }
        let Some(unit) = self.host_object(id) else {
            return;
        };
        if !unit.has_ai_update_interface() {
            return;
        }
        let occupants = if unit.thing().template.contain_module.kind != ContainModuleKind::None {
            unit.contained_units()
        } else {
            Vec::new()
        };
        // CPP2572 general command admission and3067 privateIdle: Idle
        // accepts immobile AI. The mobile check belongs to Guard/Hunt.
        let sleeping_ai = unit.ai_attitude()
            == crate::game_logic::host_strategy_center::HostAiAttitude::Sleep
            && unit
                .owner_player_id
                .and_then(|pid| self.players.get(&pid))
                .is_none_or(|player| !player.is_human);
        if !unit.is_alive()
            || unit.status.effectively_dead
            || sleeping_ai
            || unit.is_kind_of(KindOf::Projectile)
            || unit.is_surrendered
        {
            return;
        }
        self.drop_jet_targeters_on_attack_exit(id);
        self.clear_unit_movement_path(id);
        let (unit, audio_events) = self
            .host_object_and_audio_events_mut(id)
            .expect("admitted Idle unit");
        Self::clear_owned_named_ai_goal(
            unit,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.stop(&mut crate::game_logic::ObjectAudio::queue(audio_events));
        unit.clear_guard_chase();
        unit.unit_ai_runtime.clear_guard();
        unit.unit_ai_runtime.clear_hunt();
        unit.set_hunting(false);
        unit.set_ai_state(
            AIState::Idle,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.last_command_source = source;
        unit.mark_jet_command_for_reload_interrupt(true);
        // CPP privateIdle3094 and AIGroup2069 also idle garrisoned AI.
        for occupant in occupants {
            if occupant != id {
                self.apply_owned_idle_member(occupant, source, visited);
            }
        }
    }

    /// CPP ScriptActions.cpp1861: leaveGroup, capture position, NORMAL,
    /// then SCRIPT GuardPosition. Even a rejected order keeps the first effects.
    pub(super) fn apply_owned_named_guard(&mut self, name: &str, this_object: Option<ObjectId>) {
        let Some(id) = self.owned_named_script_object(name, this_object) else {
            return;
        };
        let unit = self.host_object_mut(id).expect("resolved Guard unit");
        if !unit.has_ai_update_interface() {
            return;
        }
        self.host_object_leave_group(id);
        let position = self.host_object(id).expect("resolved Guard unit").get_position();
        self.apply_unit_locomotor_set(id, "normal");
        self.apply_owned_script_guard_position(id, position);
    }

    /// Shared private Guard entry, after each action's distinct preparation.
    /// TeamGuard does not leave its group or select a new locomotor set.
    pub(super) fn apply_owned_script_guard_position(&mut self, id: ObjectId, position: Vec3) {
        self.apply_owned_script_guard(id, position, None);
    }

    pub(super) fn apply_owned_script_guard_area(
        &mut self,
        id: ObjectId,
        position: Vec3,
        area: &str,
    ) {
        self.apply_owned_script_guard(id, position, Some(area));
    }

    fn apply_owned_script_guard(&mut self, id: ObjectId, position: Vec3, area: Option<&str>) {
        if !self
            .host_object(id)
            .is_some_and(|unit| unit.has_ai_update_interface())
            || !self.owned_named_ai_command_admitted(id)
        {
            return;
        }
        if let Some(area) = area {
            // privateGuardArea installs these fields before clearing the old
            // machine, so outgoing-state effects observe the new guard data.
            let unit = self.host_object_mut(id).expect("admitted Guard unit");
            unit.unit_ai_runtime.set_raw_guard_position(Some(position));
            unit.unit_ai_runtime.set_raw_guard_target(None);
            unit.unit_ai_runtime.set_raw_guard_area(Some(area.to_owned()));
            unit.set_guard_mode(GuardMode::Normal);
        }
        let clear_team_target = self
            .host_object(id)
            .is_some_and(|unit| matches!(unit.guard_chase_phase, 1 | 3));
        self.drop_jet_targeters_on_attack_exit(id);
        if clear_team_target {
            self.set_host_team_common_target(id, None);
        }
        self.clear_unit_movement_path(id);
        let (unit, audio_events) = self
            .host_object_and_audio_events_mut(id)
            .expect("admitted Guard unit");
        Self::clear_owned_named_ai_goal(
            unit,
            &mut crate::game_logic::ObjectAudio::queue(audio_events),
        );
        unit.clear_guard_chase();
        unit.unit_ai_runtime.clear_guard();
        unit.unit_ai_runtime.clear_hunt();
        unit.set_hunting(false);
        unit.set_guard_mode(GuardMode::Normal);
        if let Some(area) = area {
            // privateGuardArea installs both its polygon and center before
            // entering AI_GUARD; state entry must observe the area target.
            unit.set_guard_area_trigger(Some(area.to_owned()));
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
            unit.set_ai_state(
                AIState::GuardingArea,
                &mut crate::game_logic::ObjectAudio::queue(audio_events),
            );
        } else {
            unit.set_guard_position(
                Some(position),
                &mut crate::game_logic::ObjectAudio::queue(audio_events),
            );
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        }
        unit.mark_jet_command_for_reload_interrupt(true);
        self.return_guard_to_post(id);
    }

    /// CPP privateEvacuate(FALSE) orders AI_EXIT in contained-list order;
    /// onEnter notifies the container synchronously, while its update waits
    /// for landing and doors before removing the passenger.
    pub(super) fn order_owned_script_passengers_to_exit(
        &mut self,
        container_id: ObjectId,
        reject_subdued: bool,
    ) {
        let Some(container) = self.host_object(container_id) else {
            return;
        };
        if reject_subdued && container.is_subdued_disabled() {
            return;
        }
        let passengers = container.contained_units();
        for passenger in passengers {
            if passenger == container_id
                || !self.host_object(passenger).is_some_and(|p| {
                    p.has_ai_update_interface()
                        && p.is_alive()
                        && !p.status.effectively_dead
                        && p.contained_by == Some(container_id)
                })
            {
                continue;
            }
            self.drop_jet_targeters_on_attack_exit(passenger);
            self.clear_unit_movement_path(passenger);
            let operation_frame = self.frame;
            let (object, attack_events, audio_events, partition) = self
                .host_object_attack_audio_partition_mut(passenger)
                .expect("resolved passenger");
            Self::clear_owned_named_ai_goal(
                object,
                &mut crate::game_logic::ObjectAudio::queue(audio_events),
            );
            object.set_target_with_operation(
                Some(container_id),
                attack_events,
                &mut crate::game_logic::ObjectAudio::queue(audio_events),
                operation_frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(partition),
            );
            object.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
            object.set_ai_state(
                AIState::Exiting,
                &mut crate::game_logic::ObjectAudio::queue(audio_events),
            );
            // ChinookAIUpdate::onObjectWantsToEnterOrExit makes idle flight
            // seek a landing place. No passenger is removed by this command.
            if let Some(ai) = self
                .host_object_mut(container_id)
                .and_then(|c| c.chinook_ai.as_mut())
            {
                ai.wanting_enter_or_exit = true;
            }
        }
    }

    /// CPP AIExitState::update. The goal and passenger are resolved from this
    /// instance each time, so deletion/replacement cannot select another world.
    pub(crate) fn update_owned_passenger_exit(&mut self, passenger: ObjectId) {
        self.update_owned_passenger_exit_with_effects(passenger, &mut Default::default());
    }
    pub(crate) fn update_owned_passenger_exit_with_effects(
        &mut self,
        passenger: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some(object) = self.host_object(passenger) else {
            return;
        };
        let Some(container_id) = object.target.filter(|id| *id != passenger) else {
            self.apply_owned_script_idle(passenger);
            return;
        };
        let can_rappel = object.is_kind_of(KindOf::Infantry);
        let Some(container) = self.host_object(container_id) else {
            self.apply_owned_script_idle(passenger);
            return;
        };
        // A malformed or stale restored relationship must not eject a rider
        // from a different container or manufacture a passenger at this one.
        if object.contained_by != Some(container_id)
            || !container.contained_units().contains(&passenger)
            || !container.can_contain()
        {
            self.apply_owned_script_idle(passenger);
            return;
        }
        if container.is_subdued_disabled()
            || container.is_transport_exit_busy(self.frame)
            || container.chinook_ai.as_ref().is_some_and(|ai| {
                ai.ai_free_to_exit(can_rappel)
                    == crate::game_logic::host_combat_chinook::HostChinookFreeToExit::WaitToExit
            })
        {
            return;
        }
        if container.is_garrison_contain() {
            {
                let (object, ownership_events, partition) = self
                    .host_object_ownership_partition_mut(container_id)
                    .unwrap();
                object.remove_occupant_with_partition_owner(passenger, ownership_events, partition)
            };
            self.garrison_exit_occupant_via_door_with_effects(passenger, container_id, effects);
        } else {
            self.unit_command_exit_via_open_contain_with_effects(passenger, container_id, effects);
        }
        if self
            .host_object(passenger)
            .is_some_and(|p| p.ai_state() == AIState::Exiting)
        {
            self.apply_owned_script_idle(passenger);
        }
    }

    pub(super) fn apply_owned_named_script_command(
        &mut self,
        request: ScriptNamedCommand<'_>,
        this_object: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        match request {
            ScriptNamedCommand::Stop { unit } => {
                if let Some(id) = self.owned_named_script_object(unit, this_object) {
                    // CPP4519 Idle alone: retain group and current locomotor.
                    self.apply_owned_script_idle(id);
                }
                return Ok(());
            }
            ScriptNamedCommand::Hunt { unit } => {
                self.apply_owned_named_hunt(unit, this_object);
                return Ok(());
            }
            ScriptNamedCommand::Guard { unit } => {
                self.apply_owned_named_guard(unit, this_object);
                return Ok(());
            }
            ScriptNamedCommand::Enter { unit, target } => {
                let Some(id) = self.owned_named_script_object(unit, this_object) else {
                    return Ok(());
                };
                let Some(target) = self.owned_named_script_object(target, this_object) else {
                    return Ok(());
                };
                let Some(position) = self.host_object(target).map(|object| object.get_position())
                else {
                    return Ok(());
                };
                if !self
                    .host_object(id)
                    .is_some_and(|object| object.has_ai_update_interface())
                {
                    return Ok(());
                }
                // CPP ScriptActions1499: leaveGroup and NORMAL precede aiEnter.
                self.host_object_leave_group(id);
                self.apply_unit_locomotor_set(id, "normal");
                if !self.owned_named_ai_command_admitted(id)
                    || !self.can_unit_enter_script_target(id, target)
                {
                    return Ok(());
                }
                self.host_object_mut(id).unwrap().last_command_source =
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
                self.unit_command_stop_moving_order_target(id, Some(target))?;
                if !self.unit_command_path_with_state_ignoring(
                    id,
                    position,
                    AIState::Entering,
                    Some(target),
                )? {
                    // Preserve the existing Enter state's obstacle continuation
                    // when no ordinary path reaches the contained destination.
                    if let Some((object, audio_events)) = self.host_object_and_audio_events_mut(id)
                    {
                        object.ignored_obstacle_id = Some(target);
                        object.set_ai_state(
                            AIState::Entering,
                            &mut crate::game_logic::ObjectAudio::queue(audio_events),
                        );
                    }
                }
                return Ok(());
            }
            ScriptNamedCommand::Evacuate { unit } => {
                let Some(id) = self.owned_named_script_object(unit, this_object) else {
                    return Ok(());
                };
                if !self
                    .host_object(id)
                    .is_some_and(|object| object.has_ai_update_interface())
                {
                    return Ok(());
                }
                // CPP ScriptActions1544: leaveGroup, NORMAL, aiEvacuate(FALSE).
                self.host_object_leave_group(id);
                self.apply_unit_locomotor_set(id, "normal");
                self.order_owned_script_passengers_to_exit(id, true);
                return Ok(());
            }
            _ => {}
        }
        let (unit, target) = match request {
            ScriptNamedCommand::ForceAttack { unit, target }
            | ScriptNamedCommand::FaceObject { unit, target } => (unit, target),
            ScriptNamedCommand::Hunt { .. }
            | ScriptNamedCommand::Stop { .. }
            | ScriptNamedCommand::Guard { .. }
            | ScriptNamedCommand::Enter { .. }
            | ScriptNamedCommand::Evacuate { .. } => {
                unreachable!("single-unit command handled above")
            }
        };
        let Some(id) = self.owned_named_script_object(unit, this_object) else {
            return Ok(());
        };
        let Some(target_id) = self.owned_named_script_object(target, this_object) else {
            return Ok(());
        };
        if !self
            .host_object(id)
            .is_some_and(|o| o.has_ai_update_interface())
        {
            return Ok(());
        }
        if matches!(request, ScriptNamedCommand::FaceObject { .. }) {
            // Main's AddWaypoint appends to this object's movement route. Clear
            // that route and its pending destination without issuing player
            // Stop (which also idles passengers and invokes other behaviors).
            self.clear_unit_movement_path(id);
            let unit = self.host_object_mut(id).expect("resolved script unit");
            unit.clear_pending_waypoint_labels();
            unit.requested_destination = None;
            unit.waiting_for_path = false;
            unit.pending_move = None;
        }
        // CPP ScriptActions.cpp1042/6081: leaveGroup precedes NORMAL selection.
        self.host_object_leave_group(id);
        self.apply_unit_locomotor_set(id, "normal");
        match request {
            ScriptNamedCommand::Hunt { .. }
            | ScriptNamedCommand::Stop { .. }
            | ScriptNamedCommand::Guard { .. }
            | ScriptNamedCommand::Enter { .. }
            | ScriptNamedCommand::Evacuate { .. } => {
                unreachable!("single-unit command handled above")
            }
            ScriptNamedCommand::ForceAttack { .. } => {
                self.host_object_mut(id)
                    .expect("resolved script unit")
                    .last_command_source =
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
                if self.unit_command_force_attack_with_effects(id, target_id, effects)? {
                    // CPP privateForceAttackObject applies the limit after
                    // entering the state because onEnter resets the weapon.
                    let unit = self.host_object_mut(id).expect("resolved script unit");
                    unit.pending_move = None;
                    unit.set_max_shots_to_fire(-1);
                }
            }
            ScriptNamedCommand::FaceObject { .. } => {
                let unit = self.host_object_mut(id).expect("resolved script unit");
                if !unit.can_move() {
                    return Ok(());
                }
                unit.last_command_source =
                    crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
                unit.num_frames_blocked = 0;
                self.private_face_object(id, target_id);
            }
        }
        Ok(())
    }
}
