//! Host scripts `impl GameLogic` — `unit_commands`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! pick / unit commands
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// C++ `UnicodeString::format(TheGameText->fetch("GUI:RallyPointSet"), displayName)`.
pub(crate) fn format_rally_point_set_message(template: &str, display_name: &str) -> String {
    if template.contains("%s") {
        template.replace("%s", display_name)
    } else if template == "GUI:RallyPointSet" || template.starts_with("MISSING:") {
        format!("Rally point set for {display_name}")
    } else {
        format!("{template} {display_name}")
    }
}

/// C++ `AIHuntState::onExit` (`AIStates.cpp:7099-7111`) releases
/// `LOCKED_TEMPORARILY` on any hunt exit. Guard dispatch has no
/// `releaseWeaponLockForGroup`; leftover `classic_on_exit` is that path.
fn release_hunt_temp_lock_when_entering_guard(unit: &mut Object) {
    if unit.is_hunting() || matches!(unit.ai_state(), AIState::Patrolling) {
        unit.release_weapon_lock(WeaponLockType::LockedTemporarily);
    }
}

/// C++ `AIHuntState::onExit` (`AIStates.cpp:7099-7109`) on ANY parent
/// replacement: halt the hunt machine and `releaseWeaponLock(LOCKED_TEMPORARILY)`.
/// Player `aiAttackObject` / `aiMoveTo` replace `AI_HUNT` so hunt never resumes.
fn end_hunt_on_player_parent_order(unit: &mut Object) {
    if unit.is_hunting() || matches!(unit.ai_state(), AIState::Patrolling) {
        unit.release_weapon_lock(WeaponLockType::LockedTemporarily);
    }
    unit.set_hunting(false);
}

/// C++ AIUpdateInterface last command source. Player/script orders are not
/// CMD_FROM_AI, so CommandButtonHuntUpdate quits on the next scan.
fn stamp_last_command_from_player(unit: &mut Object) {
    unit.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_PLAYER;
}

impl GameLogic {
    fn stamp_player_command_source(&mut self, id: ObjectId) {
        if let Some(unit) = self.objects.get_mut(&id) {
            stamp_last_command_from_player(unit);
            // StateMachine::setGoalObject also preserves a locked parent goal.
            if unit.transport_return_state.is_none() {
                unit.set_movement_goal_object(None);
            }
        }
    }

    /// Wave 246: world-position object pick without exposing object dual-walk to callers.
    ///
    /// Priority bands mirror command_integration residual acquire:
    /// - with selection: enemy attackable, then friendly selectable, then other
    /// - without selection: own selectable first, then any other selectable
    pub fn pick_object_id_at_world(
        &self,
        origin: glam::Vec3,
        player_team: Option<Team>,
        has_selected_units: bool,
        base_selection_radius: f32,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_residual_acquire::{
            PriorityAcquireCandidate, pick_best_priority_residual_target,
        };

        let cands: Vec<_> = self
            .objects
            .registered()
            .filter_map(|(&id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                let distance = (pos - origin).length();
                let radius = base_selection_radius.max(obj.selection_radius);
                if distance > radius {
                    return None;
                }
                let priority = if has_selected_units {
                    match player_team {
                        Some(team)
                            if obj.team != team
                                && obj.is_attackable()
                                && !obj.is_kind_of(KindOf::Unattackable)
                                && !obj.status.masked =>
                        {
                            Some(0)
                        }
                        Some(team) if obj.team == team && obj.is_selectable() => Some(1),
                        _ if obj.is_attackable()
                            && !obj.is_kind_of(KindOf::Unattackable)
                            && !obj.status.masked =>
                        {
                            Some(2)
                        }
                        _ if obj.is_selectable() => Some(3),
                        _ => None,
                    }
                } else {
                    // C++ SelectionXlat.cpp:181-189 / 679-693: a point click
                    // may select a lone enemy, civilian, or allied drawable.
                    match player_team {
                        Some(team) if obj.team == team && obj.is_selectable() => Some(0),
                        Some(_) if obj.is_selectable() => Some(1),
                        None if obj.is_selectable() => Some(0),
                        _ => None,
                    }
                };
                Some(PriorityAcquireCandidate {
                    id,
                    position: pos,
                    is_alive: true,
                    priority,
                })
            })
            .collect();

        pick_best_priority_residual_target(
            ObjectId(0),
            origin,
            (origin.x, origin.z),
            f32::MAX,
            cands,
        )
        .map(|(id, _, _)| id)
    }

    /// C++ `AIUpdateInterface::isAllowedToRespondToAiCommands` Sleep + dead gate.
    /// AI-controlled Sleep units ignore Hunt/Guard/AttackMove until attitude rises.
    fn unit_sleep_mood_blocks_ai_command(&self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get(&id) else {
            return true;
        };
        if unit.status.effectively_dead || !unit.is_alive() {
            return true;
        }
        if unit.ai_attitude() != crate::game_logic::host_strategy_center::HostAiAttitude::Sleep {
            return false;
        }
        match unit.owner_player_id.and_then(|pid| self.players.get(&pid)) {
            Some(player) => !player.is_local,
            None => true,
        }
    }

    #[inline]
    pub fn unit_is_dead_or_missing(&self, id: ObjectId) -> bool {
        match self.objects.get(&id) {
            Some(o) => !o.is_alive(),
            None => true,
        }
    }

    /// C++ `HackInternetAIUpdate::aiDoCommand`: PACKING on HACK_INTERNET/PACKING.
    pub(super) fn note_hacker_ai_command(
        &mut self,
        id: ObjectId,
        pending: crate::game_logic::host_hacker_income::PendingHackerCommand,
    ) -> bool {
        let pack_frames = self
            .objects
            .get(&id)
            .and_then(|obj| obj.thing().template.hack_internet_ai_update)
            .map(|meta| meta.pack_time_frames)
            .unwrap_or(0);
        if self
            .hacker_income
            .request_pack(id, self.frame, pack_frames, pending)
        {
            self.leftover_sa_set_pack_model(id, false, true, false);
            self.queue_resolved_per_unit_sound(
                id,
                crate::game_logic::host_hacker_income::HACKER_UNIT_PACK_AUDIO,
                true,
                false,
                None,
                150,
            );
            return true;
        }
        self.stop_hacker_internet_hack(id);
        false
    }

    pub(super) fn stop_attack_clearing_jet_targeter(&mut self, id: ObjectId) {
        self.drop_jet_targeters_on_attack_exit(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
        }
    }

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

    fn unit_command_move_owned(
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
                unit.ignored_obstacle_id = None;
                end_hunt_on_player_parent_order(unit);
                unit.set_ai_state(
                    AIState::Moving,
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
            unit.ignored_obstacle_id = None;
            end_hunt_on_player_parent_order(unit);
            unit.set_ai_state(
                AIState::Moving,
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
        self.unit_command_attack_with_source(id, target_id, true)
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
        self.unit_command_attack_with_source(id, target_id, false)
    }

    fn unit_command_attack_with_source(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
        from_player: bool,
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
        if !matches!(
            self.get_able_to_attack_specific_object(
                id,
                target_id,
                AbleToAttackType::NewTarget,
                from_player,
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
        unit.set_force_attack(false);
        drop(unit);
        // C++ privateAttackObject enters the attack machine and requests a
        // chase path. A weaponless order that already passed the command
        // legality check still stamps Attacking so existing callers keep it.
        let engaged = self.private_attack_object(id, target_id, -1)?;
        if !engaged {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_target_with_operation(
                    Some(target_id),
                    &mut self.attack_events,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    self.frame,
                    &mut crate::game_logic::ScriptPresentationEffects::default()
                        .reborrow_for_object(&mut self.partition_manager),
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
        if let Some(tgt) = self.objects.get_mut(&target_id) {
            tgt.add_jet_targeter(id, true, self.frame);
        }
        self.assault_transport_on_player_attack(id);
        Ok(true)
    }

    /// Wave 230/232: force-attack target (records host attack log).
    pub fn unit_command_force_attack(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Attack(target_id),
        ) {
            return true;
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
            return false;
        }
        let (can_attack, switch_away) = {
            let Some(unit) = self.objects.get(&id) else {
                return false;
            };
            (
                unit.can_attack(),
                unit.target.is_some_and(|prev| prev != target_id),
            )
        };
        if !can_attack {
            return false;
        }
        if switch_away {
            self.drop_jet_targeters_on_attack_exit(id);
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.mark_jet_command_for_reload_interrupt(false);
        unit.clear_guard_chase();
        end_hunt_on_player_parent_order(unit);

        unit.set_target_with_operation(
            Some(target_id),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.set_force_attack(true);
        drop(unit);
        if let Some(unit) = self.objects.get_mut(&id) {
            if unit.ignored_obstacle_id.is_some_and(|old| old != target_id) {
                unit.ignored_obstacle_id = None;
            }
        }
        self.clear_unit_hunt_scan(id);
        if let Some(tgt) = self.objects.get_mut(&target_id) {
            tgt.add_jet_targeter(id, true, self.frame);
        }
        true
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
    fn unit_command_stop_self_with_effects(
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
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_guard_target(
            None,
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
        {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            release_hunt_temp_lock_when_entering_guard(unit);
            unit.unit_ai_runtime.clear_guard();
            unit.set_guard_position(
                Some(pos),
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
        self.stamp_player_command_source(id);
        self.unit_command_guard_object_owned(id, target_id)
    }

    pub(crate) fn unit_command_script_guard_object(
        &mut self,
        id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        if let Some(object) = self.objects.get_mut(&id) {
            object.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        }
        self.unit_command_guard_object_owned(id, target_id)
    }

    fn unit_command_guard_object_owned(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Stop,
        ) {
            return true;
        }
        {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            release_hunt_temp_lock_when_entering_guard(unit);
            unit.unit_ai_runtime.clear_guard();
            unit.set_guard_target(
                Some(target_id),
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

    fn unit_command_attack_move_owned(
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

    fn unit_command_attack_move_owned_with_effects(
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
        let (alive, can_attack, mobile) = match self.objects.get(&id) {
            Some(unit) => (
                unit.is_alive(),
                unit.can_attack() || unit.weapon.is_some(),
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
            } else if can_attack && (path_ok || deploy_blocked || already_there) {
                if path_ok {
                    unit.ignored_obstacle_id = None;
                }
                unit.set_auto_acquire_when_idle(true);
                unit.requested_destination = Some(destination);
                if unit.ai_state() != AIState::AttackMoving {
                    unit.set_ai_state(
                        AIState::AttackMoving,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                }
            } else if path_ok && unit.ai_state() != AIState::Moving {
                unit.ignored_obstacle_id = None;
                unit.is_attack_path = false;
                unit.set_ai_state(
                    AIState::Moving,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            } else if path_ok {
                unit.ignored_obstacle_id = None;
                unit.is_attack_path = false;
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
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
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

    /// Wave 231: force-attack ground location.
    pub fn unit_command_attack_ground(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        self.stamp_player_command_source(id);
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::MoveTo(location),
        ) {
            return true;
        }
        if !matches!(
            self.get_able_to_use_weapon_against_target(
                id,
                None,
                Some(location),
                AbleToAttackType::NewTargetForced,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.can_attack() {
            return false;
        }
        end_hunt_on_player_parent_order(unit);
        unit.set_force_attack(true);
        unit.target = None;

        unit.leftover_choose_best_reset_primary_for_ground();
        unit.set_target_location(
            Some(location),
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_ai_state(
            AIState::AttackingGround,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        drop(unit);
        self.set_turret_target_position(id, Some(location));
        self.clear_unit_hunt_scan(id);
        true
    }

    /// Scatter/formation move.
    ///
    /// A unit that cannot path yet keeps `pending_move` and retries
    /// `assign_unit_path` once it can move. That point is not a locomotor
    /// goal. A path the finder refuses does not walk the click.
    pub fn unit_command_move_to_moving(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        self.stamp_player_command_source(id);
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::MoveTo(destination),
        ) {
            return true;
        }
        let cannot_path_yet = self
            .objects
            .get(&id)
            .is_some_and(|unit| unit.is_alive() && !unit.can_move());
        if cannot_path_yet {
            let clipped = self.clip_player_goal(id, destination);
            if let Some(unit) = self.objects.get_mut(&id) {
                end_hunt_on_player_parent_order(unit);
                unit.ignored_obstacle_id = None;
                unit.pending_move = Some(clipped);
                unit.movement.target_position = None;
                unit.movement.path.clear();
                unit.set_ai_state(
                    AIState::Moving,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            self.clear_unit_hunt_scan(id);
            return true;
        }
        let _ = self.note_move_to_request_path(id);
        let destination = self.clip_player_goal(id, destination);
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            end_hunt_on_player_parent_order(unit);
            unit.ignored_obstacle_id = None;
            unit.set_ai_state(
                AIState::Moving,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            self.clear_unit_hunt_scan(id);
            return true;
        }
        false
    }

    /// Wave 232: dozer construct — path/destination + Constructing AI state.
    /// C++ `findGoodBuildOrRepairPosition` docks at half major radius and
    /// `ignoreObstacle(goalObject)` so the scaffold is not an A* wall.
    pub fn unit_command_begin_construct(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
    ) -> gamelogic::GameLogicResult<bool> {
        if self.objects.get(&id).is_none() {
            return Ok(false);
        }
        if self.note_hacker_ai_command(
            id,
            crate::game_logic::host_hacker_income::PendingHackerCommand::Stop,
        ) {
            return Ok(true);
        }
        let (dozer_pos, airborne, site) = {
            let Some(dozer) = self.objects.get(&id) else {
                return Ok(false);
            };
            let site = dozer.target.and_then(|tid| {
                self.objects.get(&tid).and_then(|st| {
                    if st.status.under_construction {
                        Some((st.get_position(), st.selection_radius, tid))
                    } else {
                        None
                    }
                })
            });
            (
                dozer.get_position(),
                dozer.is_kind_of(KindOf::Aircraft) || dozer.status.airborne_target,
                site,
            )
        };
        let (dock, ignore) = match site {
            Some((st_pos, st_radius, tid)) => {
                let dock = self.find_good_build_or_repair_position(
                    dozer_pos,
                    st_pos,
                    st_radius,
                    airborne,
                    airborne.then_some(tid),
                    Some(id),
                );
                (dock, Some(tid))
            }
            None => (location, None),
        };
        if ignore.is_some() {
            // C++ AIMoveToState::onEnter (AIStates.cpp:2013-2017): the goal
            // object is the ignored obstacle, so the destination must not snap.
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_adjust_destinations(false);
            }
        }
        if let Some(tid) = ignore {
            self.dozer_new_task_build(id, tid);
        }
        let dock = self
            .objects
            .get(&id)
            .and_then(|unit| unit.dozer_tasks.docks[0][1].point())
            .map(|dock| Vec3::new(dock.x, dock.z, dock.y))
            .unwrap_or(dock);
        let assigned = self.assign_unit_path_ignoring(id, dock, &[], ignore);
        if !assigned {
            if let Some(unit) = self.objects.get_mut(&id) {
                if unit.is_alive() && !unit.can_move() {
                    unit.pending_move = Some(dock);
                    unit.movement.target_position = None;
                    unit.movement.path.clear();
                }
            } else {
                return Ok(false);
            }
        }
        // C++ WorkerAIUpdate::newTask — drop preferred dock and leave supply mode.
        self.worker_exit_supply_for_dozer_task(id);
        self.set_ai_state_decision_aware_with_effects(
            id,
            AIState::Constructing,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )?;
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_ultra_accurate(true);
            if assigned && !unit.movement.path.is_empty() {
                unit.set_status_moving(true);
            }
            return Ok(true);
        }
        Ok(false)
    }

    /// Additive selection mark on a selectable unit controlled by this player.
    pub fn unit_select_if_player(&mut self, id: ObjectId, player_id: u32) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.owner_player_id == Some(player_id) && obj.is_selectable() {
            obj.select();
            obj.flash_as_selected();
        } else {
            return false;
        }
        self.apply_player_drawable_selection(player_id, &[id], false);
        self.client_visible_contained_flash_as_selected(id);
        true
    }

    /// Compatibility path for faction-only legacy callers. New player-command
    /// paths must use `unit_select_if_player` so duplicate factions stay apart.
    pub fn unit_select_if_team(&mut self, id: ObjectId, player_team: Team) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.team == player_team && obj.is_selectable() {
            obj.select();
            obj.flash_as_selected();
        } else {
            return false;
        }
        if let Some(player_id) = self
            .objects
            .get(&id)
            .and_then(|object| object.owner_player_id)
        {
            self.apply_player_drawable_selection(player_id, &[id], false);
        }
        self.client_visible_contained_flash_as_selected(id);
        true
    }

    /// Wave 232: path after stop_attack; optionally clear formation id (free move).
    pub fn unit_command_move_clear_formation(
        &mut self,
        id: ObjectId,
        destination: glam::Vec3,
        clear_formation: bool,
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.stop_attack_clearing_jet_targeter(id);
        let _ = self.note_move_to_request_path(id);
        let destination = self.clip_player_goal(id, destination);
        if !self.assign_unit_path(id, destination, &[]) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.ignored_obstacle_id = None;
            if clear_formation && unit.formation_id != 0 {
                unit.set_formation(0, glam::Vec2::ZERO);
            }
            unit.set_ai_state(
                AIState::Moving,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
        }
        true
    }

    /// Tighten-group prep. Formation and guard clear only when the order is
    /// kept: a queued path, or a destination stored because the unit cannot
    /// path yet. A refused path leaves the group alone.
    pub fn unit_command_tighten_to(&mut self, id: ObjectId, destination: glam::Vec3) -> bool {
        let can = self
            .objects
            .get(&id)
            .map(|u| {
                u.is_alive()
                    && !u.status.disabled_held
                    && !u.is_kind_of(KindOf::Immobile)
                    && !u.is_kind_of(KindOf::Structure)
            })
            .unwrap_or(false);
        if !can {
            return false;
        }
        let destination = self.clip_player_goal(id, destination);
        self.drop_jet_targeters_on_attack_exit(id);
        let cannot_path_yet = self
            .objects
            .get(&id)
            .is_some_and(|unit| unit.is_alive() && !unit.can_move());
        if cannot_path_yet {
            if let Some(unit) = self.objects.get_mut(&id) {
                Self::accept_tighten_order(
                    unit,
                    destination,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            return true;
        }
        let _ = self.note_move_to_request_path(id);
        // C++ AIMoveAndTightenState::onEnter clears adjust before the
        // nested move. update sets it true only once a path exists and
        // the unit is not waiting.
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.set_adjust_destinations(false);
        }
        self.pathfinding_system.tighten_restore_adjust = true;
        if !self.assign_unit_path(id, destination, &[]) {
            self.pathfinding_system.tighten_restore_adjust = false;
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if !unit.waiting_for_path && !unit.movement.path.is_empty() {
                unit.set_adjust_destinations(true);
            }
            Self::accept_tighten_order(
                unit,
                destination,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            return true;
        }
        false
    }

    fn accept_tighten_order(
        unit: &mut crate::game_logic::Object,
        destination: glam::Vec3,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        unit.stop_attack(audio);
        unit.set_formation(0, glam::Vec2::ZERO);
        unit.set_guard_position(None, audio);
        unit.set_guard_target(None, audio);
        unit.end_guard_retaliate(audio);
        if !unit.can_move() {
            unit.pending_move = Some(destination);
            unit.movement.target_position = None;
            unit.movement.path.clear();
        }
        unit.set_ai_state(AIState::Moving, audio);
    }

    /// Wave 232: stamp formation id + offset (create/dissolve).
    pub fn unit_command_set_formation(
        &mut self,
        id: ObjectId,
        formation_id: u32,
        offset: glam::Vec2,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_formation(formation_id, offset);
        true
    }

    /// Wave 232: full guard order (position or object) with radius/mode.
    /// Returns whether unit accepted the order; caller may still path.
    pub fn unit_command_guard_full(
        &mut self,
        id: ObjectId,
        position: Option<glam::Vec3>,
        target: Option<ObjectId>,
        guard_radius: f32,
        mode: GuardMode,
    ) -> bool {
        self.stamp_player_command_source(id);
        let movable = self.objects.get(&id).map(|u| u.can_move()).unwrap_or(false);
        if !self.host_unit_can_guard(id) {
            return false;
        }
        if self.unit_sleep_mood_blocks_ai_command(id) {
            return false;
        }

        // For object guard, require living target position when provided.
        let ai_state = if let Some(tid) = target {
            let tpos = self
                .objects
                .get(&tid)
                .filter(|o| o.is_alive())
                .map(|o| o.get_position());
            if tpos.is_none() {
                return false;
            }
            AIState::GuardingObject
        } else if position.is_some() {
            AIState::GuardingArea
        } else {
            return false;
        };
        self.drop_jet_targeters_on_attack_exit(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.guard_radius = guard_radius;
            unit.set_guard_mode(mode);
            unit.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            unit.set_force_attack(false);
            unit.clear_guard_chase();

            unit.end_guard_retaliate(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            release_hunt_temp_lock_when_entering_guard(unit);
            unit.set_hunting(false);
            if let Some(tid) = target {
                unit.set_guard_position_data(None);
                unit.set_guard_target(
                    Some(tid),
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            } else if let Some(pos) = position {
                unit.set_guard_target(
                    None,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                unit.set_guard_position(
                    Some(pos),
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            unit.set_ai_state(
                ai_state.clone(),
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
        } else {
            return false;
        }
        // C++ AIGuardState::onEnter walks to the post before Idle. Turrets stay.
        if movable {
            if let Some(pos) = position {
                if self.path_approach_with_state(id, pos, AIState::GuardingArea) {
                    if let Some(unit) = self.objects.get_mut(&id) {
                        unit.ignored_obstacle_id = None;
                    }
                }
            } else if let Some(tid) = target {
                if let Some(mut tpos) = self
                    .objects
                    .get(&tid)
                    .filter(|o| o.is_alive())
                    .map(|o| o.get_position())
                {
                    // C++ AIGuardReturnState::onEnter: pathfinder->adjustDestination.
                    self.adjust_guard_goal(id, &mut tpos);
                    if self.path_approach_with_state(id, tpos, AIState::GuardingObject) {
                        if let Some(unit) = self.objects.get_mut(&id) {
                            unit.ignored_obstacle_id = None;
                        }
                    }
                }
            }
        }
        true
    }

    /// Wave 232: set guard radius only (guard area residual).
    pub fn unit_command_set_guard_radius(&mut self, id: ObjectId, radius: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.guard_radius = radius;
        true
    }

    /// Wave 232: attack-ground with max shots + force flag.
    pub fn unit_command_attack_ground_ex(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
        max_shots: i32,
    ) -> bool {
        self.stamp_player_command_source(id);
        if !matches!(
            self.get_able_to_use_weapon_against_target(
                id,
                None,
                Some(location),
                AbleToAttackType::NewTargetForced,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        self.drop_jet_targeters_on_attack_exit(id);
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        end_hunt_on_player_parent_order(unit);
        unit.target = None;

        unit.set_force_attack(true);
        unit.set_max_shots_to_fire(max_shots);
        unit.leftover_choose_best_reset_primary_for_ground();
        unit.set_target_location(
            Some(location),
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_ai_state(
            AIState::AttackingGround,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        drop(unit);
        self.set_turret_target_position(id, Some(location));
        true
    }

    /// Wave 232: hunt/patrol residual.
    pub fn unit_command_patrol(&mut self, id: ObjectId) -> bool {
        self.stamp_player_command_source(id);
        // C++ AIGroup::groupHunt: AI present → aiHunt. No can_move / Immobile /
        // Structure gate (attack-move is the path that branches on isAbleToAttack).
        let can = self.objects.get(&id).map(|u| u.is_alive()).unwrap_or(false);
        if !can {
            return false;
        }
        if self.unit_sleep_mood_blocks_ai_command(id) {
            return false;
        }

        let entering_hunt = self.objects.get(&id).is_some_and(|u| !u.is_hunting());
        self.drop_jet_targeters_on_attack_exit(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            unit.set_force_attack(false);
            unit.set_guard_position(
                None,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.set_guard_target(
                None,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.end_guard_retaliate(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));

            unit.set_auto_acquire_when_idle(true);
            unit.set_hunting(true);
            unit.set_ai_state(
                AIState::Patrolling,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.mark_jet_command_for_reload_interrupt(true);

            unit.set_status_moving(false);
            drop(unit);
            // C++ AIHuntState::onEnter reseeds m_nextEnemyScanTime with jitter.
            if entering_hunt {
                self.clear_unit_hunt_scan(id);
            }
            return true;
        }
        false
    }

    /// Wave 232: cheer model-condition residual.
    pub fn unit_command_cheer(
        &mut self,
        id: ObjectId,
        cheer_secs: f32,
        cheer_bit: Option<usize>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.begin_cheer(
            cheer_secs,
            cheer_bit,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// Wave 232: toggle deployed status for deploy-style units.
    pub fn unit_command_set_deployed(
        &mut self,
        id: ObjectId,
        deployed: bool,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_set_deployed_with_effects(
            id,
            deployed,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn unit_command_set_deployed_with_effects(
        &mut self,
        id: ObjectId,
        deployed: bool,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let Some(unit) = self.objects.get(&id) else {
            return Ok(false);
        };
        if deployed
            && (unit.unit_ai_runtime.current_leaf_state() != AIState::Idle
                || unit.unit_ai_runtime.root_state() != AIState::Idle)
        {
            self.enter_base_ai_idle_owned(id, effects)?;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        unit.set_deployed(
            deployed,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        Ok(true)
    }

    /// Execute an explicit Deploy toggle for an object that owns an exact
    /// `DeployStyleAIUpdate` module.  The command is authoritative here as
    /// well as in the executor: stale UI input cannot turn an arbitrary
    /// vehicle into a DeployStyle unit merely by sharing a familiar name.
    ///
    /// C++ reverses an in-progress deploy/undeploy when the opposing intent
    /// arrives.  The compact host state's two directions preserve that timing
    /// behavior; OBJECT_STATUS_DEPLOYED only changes when unpack finishes and
    /// clears immediately when packing begins.
    pub fn unit_command_toggle_deploy_style(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_toggle_deploy_style_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn unit_command_toggle_deploy_style_with_effects(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let frame = self.frame;
        let mut deployed_direction = false;
        let mut aligning = false;
        let changed = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return Ok(false);
            };
            if !unit.is_alive() || unit.get_template().deploy_style_metadata.is_none() {
                return Ok(false);
            }
            if unit.deploy_style.is_none() {
                // A template with metadata must have installed state during
                // normal construction.  A malformed/legacy snapshot without
                // it is not safe to silently reconstruct mid-command.
                return Ok(false);
            }

            deployed_direction = matches!(
                unit.deploy_style.as_ref().map(|s| s.state),
                Some(
                    crate::game_logic::host_deploy_style::HostDeployStyleState::ReadyToMove
                        | crate::game_logic::host_deploy_style::HostDeployStyleState::Undeploying
                )
            );
            let has_turret = unit.turret_enabled || unit.turret_turn_rate_rad > 0.0;
            let turret_natural =
                crate::game_logic::host_deploy_style::leftover_host_turret_is_in_natural_position(
                    unit.status.under_construction,
                    unit.turret_angle_deg,
                    unit.turret_pitch_deg,
                    unit.turret_natural_angle_deg,
                    unit.turret_natural_pitch_deg,
                );
            let transitioned = {
                let Some(style) = unit.deploy_style.as_mut() else {
                    return Ok(false);
                };
                if deployed_direction {
                    style.begin_deploy(frame)
                } else {
                    style.begin_undeploy_with_weapon_turret(frame, has_turret, turret_natural)
                }
            };
            if transitioned {
                unit.stop_moving_for_owned_idle_entry(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                unit.set_status_moving(false);
            }
            transitioned
        };

        if !changed {
            return Ok(false);
        }
        // C++ stops movement before entering Idle and then applies deploy
        // conditions/turret state. The owner operation commits Idle onEnter
        // synchronously at that same point.
        if self.objects.get(&id).is_some_and(|unit| {
            unit.unit_ai_runtime.current_leaf_state() != AIState::Idle
                || unit.unit_ai_runtime.root_state() != AIState::Idle
        }) {
            self.enter_base_ai_idle_owned(id, effects)?;
        }
        {
            let Some(unit) = self.objects.get_mut(&id) else {
                return Ok(false);
            };
            if !deployed_direction {
                aligning = unit
                    .deploy_style
                    .as_ref()
                    .is_some_and(|ds| ds.is_aligning_turrets());
                if aligning {
                    unit.turret_substate = crate::game_logic::object::TurretSubState::Recenter;
                    unit.turret_idle_recentering = true;
                    unit.turret_target_id = None;
                    unit.turret_runtime.target_pos = None;
                    unit.turret_runtime.victim_team = None;
                    unit.turret_holding = false;
                } else {
                    // C++ setMyState(UNDEPLOY) clears OBJECT_STATUS_DEPLOYED
                    // before the pack timer elapses.
                    unit.set_deployed(
                        false,
                        &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    );
                }
            }
            if let Some(state) = unit.deploy_style.as_ref().map(|s| s.state) {
                crate::game_logic::host_deploy_style::leftover_stamp_deploy_style_conditions(
                    &mut unit.model_condition_bits,
                    state,
                );
            }
        }
        if deployed_direction {
            self.deploy_style_reg.record_deploy();
            self.queue_resolved_per_unit_sound(
                id,
                crate::game_logic::host_deploy_style::DEPLOY_STYLE_DEPLOY_AUDIO,
                true,
                false,
                None,
                150,
            );
        } else if aligning {
            // Recenter first; Undeploy audio plays when packing actually starts.
        } else {
            self.deploy_style_reg.record_undeploy();
            self.queue_resolved_per_unit_sound(
                id,
                crate::game_logic::host_deploy_style::DEPLOY_STYLE_UNDEPLOY_AUDIO,
                true,
                false,
                None,
                150,
            );
        }
        Ok(true)
    }

    /// Wave 232: attack nearest of team without force flag (attack-team residual).
    pub fn unit_command_attack_soft(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        // AI/script acquisition is subject to the same real WeaponSet masks,
        // with CMD_FROM_PLAYER false so C++ AI relationship behavior remains
        // distinct from an explicit right-click.
        if !matches!(
            self.get_able_to_attack_specific_object(
                id,
                target_id,
                AbleToAttackType::NewTarget,
                false,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        ) {
            return false;
        }
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_target_with_operation(
            Some(target_id),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.set_force_attack(false);
        true
    }

    /// C++ `computeIndividualDestination` → `adjustDestination(..., groupDest)`.
    pub fn adjust_group_member_goal(
        &mut self,
        id: ObjectId,
        dest: glam::Vec3,
        group_dest: glam::Vec3,
    ) -> glam::Vec3 {
        let Some(obj) = self.objects.get(&id) else {
            return dest;
        };
        if !PathfindingGrid::is_doing_ground_movement(obj) {
            return dest;
        }
        let from = obj.get_position();
        let surfaces = if obj.locomotor_surfaces != 0 {
            obj.locomotor_surfaces
        } else {
            gamelogic::ai::pathfind_complete::SURFACE_GROUND
        };

        let is_crusher = obj.crusher_level > 0;
        let seeker = obj.owner_player_id.or(Some(obj.team as u32));
        let crusher_level = obj.crusher_level;
        let adjusted = ({
            let ground_owner = self.world_services.terrain();
            let ground_loan = ground_owner
                .read()
                .unwrap_or_else(|error| error.into_inner());
            let ground_source = crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                &ground_loan,
                self.map_loaded,
                self.terrain.as_ref(),
                self.pathfinding_height_samples.as_ref(),
                &self.pathfinding_system.grid,
            );
            self.pathfinding_system.adjust_group_destination(
                from,
                dest,
                group_dest,
                surfaces,
                is_crusher,
                seeker,
                crusher_level,
                id.0,
                &ground_source,
            )
        });
        let (old, radius, player) = self
            .objects
            .get(&id)
            .map(|unit| {
                (
                    unit.pathfind_goal_cell,
                    unit.selection_radius,
                    unit.owner_player_id.unwrap_or(unit.team as u32),
                )
            })
            .unwrap_or(((-1, -1), 0.0, 0));
        let cell = self
            .pathfinding_system
            .grid
            .update_ground_goal_cell(id.0, player, radius, false, old, adjusted);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.pathfind_goal_cell = cell;
        }
        adjusted
    }

    /// Wave 232: free group move — stop attack, path, clear formation if goal not offset.
    pub fn unit_command_move_free(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        click_destination: glam::Vec3,
    ) -> bool {
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.stop_attack_clearing_jet_targeter(id);
        let goal = self.adjust_group_member_goal(id, goal, click_destination);
        let goal = self.clip_player_goal(id, goal);
        let _ = self.note_move_to_request_path(id);
        if !self.assign_unit_path(id, goal, &[]) {
            return false;
        }

        if let Some(unit) = self.objects.get_mut(&id) {
            unit.ignored_obstacle_id = None;
            // C++ setFormationID(NO_FORMATION) on free individual move when goal is not
            // the stamped formation offset destination.
            if unit.formation_id != 0 {
                let off = unit.formation_offset;
                let expected = glam::Vec3::new(
                    click_destination.x + off.x,
                    click_destination.y,
                    click_destination.z + off.y,
                );
                if (goal - expected).length() > 0.5 {
                    unit.set_formation(0, glam::Vec2::ZERO);
                }
            }
            unit.set_ai_state(
                AIState::Moving,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
        }
        true
    }

    /// Wave 233: set order target (records host attack/order log residual).
    pub fn unit_command_set_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target_with_operation(
            target,
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        let can_construct = unit.can_construct();
        if can_construct {
            if let Some(sid) = target {
                if let Some(st) = self.objects.get_mut(&sid) {
                    if st.status.under_construction {
                        // C++ Object::setBuilder (DozerAIUpdate.cpp:1677 / 1986).
                        st.builder_id = Some(id);
                    }
                }
            }
        }
        true
    }

    /// Wave 233: stop moving then set order target (enter/gather/dock residual).
    pub fn unit_command_stop_moving_order_target(
        &mut self,
        id: ObjectId,
        target: Option<ObjectId>,
    ) -> gamelogic::GameLogicResult<bool> {
        let should_enter_idle = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return Ok(false);
            };
            unit.stop_moving_for_owned_idle_entry(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ))
        };
        if should_enter_idle && !self.enter_base_ai_idle_owned(id, &mut Default::default())? {
            return Ok(false);
        }
        let frame = self.frame;
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        unit.set_order_target_with_operation(
            target,
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        Ok(true)
    }

    /// Begin the C++ `SpecialAbilityUpdate` capture intent.  This records an
    /// order and starts approach only; `markSpecialPowerTriggered` belongs to
    /// the later preparation phase once the source reaches StartAbilityRange.
    pub fn unit_command_begin_capture(&mut self, id: ObjectId, target: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() || !unit.can_move() {
            return false;
        }
        unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_order_target_with_operation(
            Some(target),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.capture_channel = None;
        unit.set_status_using_ability(false);
        unit.set_ai_state(
            AIState::Capturing,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// Begin a parsed Hacker Disable Building `SpecialAbilityUpdate` intent.
    ///
    /// Like capture, this records only the player-authorized target and
    /// resets an interrupted channel.  It deliberately does not spend the
    /// paired SpecialPower charge: C++ starts that reload in
    /// `startPreparation`, after the hacker reaches its authored range and
    /// finishes unpacking.
    pub fn unit_command_begin_hacker_disable_building(
        &mut self,
        id: ObjectId,
        target: ObjectId,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_order_target_with_operation(
            Some(target),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.hacker_disable_channel = Some(crate::game_logic::HackerDisableChannelState::new(
            target,
            crate::game_logic::HackerDisableChannelPhase::Approaching,
            0,
        ));
        unit.set_status_using_ability(false);
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// Wave 233: set AI attitude residual.
    pub fn unit_command_set_ai_attitude(
        &mut self,
        id: ObjectId,
        attitude: crate::game_logic::host_strategy_center::HostAiAttitude,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_ai_attitude(attitude);
        true
    }

    /// Wave 233: building orientation stamp after under-construction create.
    pub fn unit_command_set_orientation(&mut self, id: ObjectId, orientation: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_orientation_with_partition_owner(orientation, &mut self.partition_manager);
        true
    }

    /// Set the canonical building rally point at the receiving command phase.
    pub fn unit_command_set_rally_point(&mut self, id: ObjectId, location: glam::Vec3) -> bool {
        let Some(obj) = self.objects.get(&id) else {
            return false;
        };
        // C++ ACTIONTYPE_SET_RALLY_POINT: KINDOF_AUTO_RALLYPOINT only.
        if !obj.is_kind_of(crate::game_logic::KindOf::AutoRallypoint) {
            return false;
        }
        if obj.building_data.is_none() {
            return false;
        }
        // C++ GameLogicDispatch.cpp:156-161 format(GUI:RallyPointSet, displayName).
        let display_name = obj.get_display_name();
        let from = obj.get_position();
        if !self
            .pathfinding_system
            .grid
            .quick_path_exists(from, location)
        {
            #[cfg(feature = "game_client")]
            game_client::helpers::TheInGameUI::message("GUI:RallyPointNoPath");
            self.play_ui_sound("UnableToSetRallyPoint");
            return false;
        }
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.rally_point = Some(location);
        #[cfg(feature = "game_client")]
        {
            let formatted = format_rally_point_set_message(
                &game_client::game_text::GameText::fetch("GUI:RallyPointSet"),
                &display_name,
            );
            game_client::helpers::TheInGameUI::message(&formatted);
        }
        self.play_ui_sound("RallyPointSet");
        true
    }

    /// Wave 233: return-supplies order target + ReturningResources state.
    pub fn unit_command_return_supplies(&mut self, id: ObjectId, supply_center: ObjectId) -> bool {
        if !self
            .objects
            .get(&supply_center)
            .is_some_and(|dock| dock.is_alive())
        {
            return false;
        }
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.cancel_dock_reservation(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.preferred_dock_id = Some(supply_center);
            unit.supply_truck_state = crate::game_logic::SupplyTruckState::Wanting;
            unit.supply_truck_force_pending = true;
            unit.set_order_target_with_operation(
                Some(supply_center),
                &mut self.attack_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(&mut self.partition_manager),
            );
            unit.set_ai_state(
                AIState::ReturningResources,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.ignored_obstacle_id = None;
        } else {
            return false;
        }
        // C++ aiDock: DockUpdate approach. Do not path at the center or ignore it.
        let _ = self.try_claim_dock(supply_center, id);
        true
    }

    /// C++ `SupplyTruckAIUpdate::privateDock` / `WorkerAIUpdate::privateDock`
    /// against `SupplyWarehouseDockUpdate`: remember the chosen warehouse and
    /// enter the collection loop.  This is intentionally not `Entering` — a
    /// warehouse grants supply boxes and never becomes the unit's container.
    pub fn unit_command_dock_at_supply_warehouse(
        &mut self,
        id: ObjectId,
        warehouse: ObjectId,
    ) -> bool {
        self.drop_jet_targeters_on_attack_exit(id);
        if !self
            .objects
            .get(&warehouse)
            .is_some_and(|dock| dock.is_alive())
        {
            return false;
        }
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.cancel_dock_reservation(id);
        if let Some(unit) = self.objects.get_mut(&id) {
            unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            unit.preferred_dock_id = Some(warehouse);
            unit.supply_truck_state = crate::game_logic::SupplyTruckState::Wanting;
            unit.supply_truck_force_pending = true;
            unit.set_order_target_with_operation(
                Some(warehouse),
                &mut self.attack_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(&mut self.partition_manager),
            );
            unit.set_ai_state(
                AIState::Gathering,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.ignored_obstacle_id = None;
        } else {
            return false;
        }
        let _ = self.try_claim_dock(warehouse, id);
        true
    }

    /// C++ `RailedTransportDockUpdate` dispatches through `AI_DOCK`, not the
    /// generic Enter command.  The support-state loader may ultimately use
    /// the shared containment storage, but the command and legality stay
    /// distinct so normal transports do not accidentally qualify as docks.
    pub fn unit_command_dock_at_railed_transport(
        &mut self,
        id: ObjectId,
        transport: ObjectId,
    ) -> bool {
        self.drop_jet_targeters_on_attack_exit(id);
        if self.objects.get(&id).is_none() {
            return false;
        }
        self.cancel_dock_reservation(id);
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_order_target_with_operation(
            Some(transport),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.set_ai_state(
            AIState::Docking,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// C++ `RailedTransportAIUpdate::privateEvacuate` → dock `unloadAll`.
    /// Refuses in-transit / already loading-or-unloading. Does not walk ExitStart.
    pub fn railed_transport_unload_all(&mut self, ferry_id: ObjectId) -> bool {
        self.railed_transport_unload_all_with_effects(ferry_id, &mut Default::default())
    }

    pub(crate) fn railed_transport_unload_all_with_effects(
        &mut self,
        ferry_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let Some(ferry) = self.objects.get(&ferry_id) else {
            return false;
        };
        if !ferry.is_railed_transport() {
            return false;
        }
        if ferry.railed_in_transit {
            return false;
        }
        if ferry.dock_active_docker.is_some() {
            return false;
        }
        let passengers = ferry.contained_units();
        if passengers.is_empty() {
            return false;
        }
        let hull = ferry.get_position();
        let yaw = ferry.get_orientation();
        let (sin, cos) = yaw.sin_cos();
        let dest = glam::Vec3::new(hull.x + 20.0 * cos, hull.y, hull.z + 20.0 * sin);
        let first = passengers[0];
        for pid in passengers {
            if let Some(c) = self.objects.get_mut(&ferry_id) {
                let _ = c.remove_occupant_with_partition_owner(
                    pid,
                    &mut self.ownership_events,
                    &mut self.partition_manager,
                );
            }
            if let Some(p) = self.objects.get_mut(&pid) {
                let containment_actor_id = p.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    containment_actor_id,
                    None,
                    true,
                );
                let p = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                p.target = None;
                p.set_position_with_effects(
                    hull,
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
                p.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                p.set_destination_with_operation(
                    dest,
                    &mut self.movement_events.writer(self.frame),
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                    self.frame,
                    &mut effects.reborrow_for_object(&mut self.partition_manager),
                );
                p.set_status_disabled_held(true);
            }
            self.admit_host_radar_disabled_edges(pid);
        }
        if let Some(c) = self.objects.get_mut(&ferry_id) {
            c.dock_active_docker = Some(first);
        }
        true
    }

    /// Wave 233: waypoint-path prep — stop attack and clear guard anchors.
    pub fn unit_command_waypoint_path_prep(&mut self, id: ObjectId, as_team: bool) -> bool {
        self.drop_jet_targeters_on_attack_exit(id);
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.stop_attack(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_guard_position(
            None,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_guard_target(
            None,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.end_guard_retaliate(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        // AsTeam keeps formation identity; free follow clears it.
        if !as_team {
            unit.set_formation(0, glam::Vec2::ZERO);
        }
        true
    }

    /// Wave 233: remove occupant from container (enter/exit residual).
    pub fn unit_command_remove_occupant(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) -> bool {
        self.unit_command_remove_occupant_with_effects(
            container_id,
            occupant_id,
            &mut Default::default(),
        )
    }

    pub(crate) fn unit_command_remove_occupant_with_effects(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        // C++ TransportContain onRemoving: bike secondary → Kell secondary.
        self.transfer_kell_snipe_reload_from_bike(container_id, occupant_id);
        let is_rider_change = self.objects.get(&container_id).is_some_and(|container| {
            container.thing().template.contain_module.kind
                == crate::game_logic::ContainModuleKind::RiderChange
        });
        if is_rider_change {
            // RiderChangeContain removal owns selection/veterancy/scuttle
            // ordering.  It must not be reduced to a bare Vec removal or the
            // next normal Enter would leave the old rider/container corrupted.
            return self.rider_change_remove_occupant_with_effects(
                container_id,
                occupant_id,
                effects,
            );
        }
        let is_garrison = self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_garrison_contain());
        let Some(container) = self.objects.get_mut(&container_id) else {
            return false;
        };
        let removed = container.remove_occupant_with_partition_owner(
            occupant_id,
            &mut self.ownership_events,
            &mut self.partition_manager,
        );
        if removed && is_garrison {
            self.recalc_garrison_apparent_controller(container_id);
        }
        removed
    }

    /// C++ TransportContain patch 1.01: CLIFF_JUMPER + HERO/SALVAGER shot-stat copy.
    fn transfer_kell_snipe_reload_from_bike(
        &mut self,
        container_id: ObjectId,
        occupant_id: ObjectId,
    ) {
        use crate::game_logic::host_combat_cycle::{
            is_kell_snipe_transfer_rider, transfer_next_shot_last_fire_time,
        };
        let Some(bike_fire) = self.objects.get(&container_id).and_then(|c| {
            if !c.is_combat_cycle_style_container()
                && !crate::game_logic::host_combat_cycle::is_combat_cycle_template(&c.template_name)
            {
                return None;
            }
            c.secondary_weapon
                .as_ref()
                .or(c.weapon.as_ref())
                .map(|w| w.last_fire_time)
        }) else {
            return;
        };
        let Some(occupant) = self.objects.get_mut(&occupant_id) else {
            return;
        };
        if !is_kell_snipe_transfer_rider(
            occupant.is_kind_of(KindOf::Hero),
            occupant.is_kind_of(KindOf::Salvager),
            &occupant.template_name,
        ) {
            return;
        }
        if let Some(w) = occupant.secondary_weapon.as_mut() {
            transfer_next_shot_last_fire_time(bike_fire, w);
        } else if let Some(w) = occupant.weapon.as_mut() {
            transfer_next_shot_last_fire_time(bike_fire, w);
        }
    }

    /// Wave 233: exit-unit drop residual (position/contain/target/ai).
    /// C++ OpenContain::exitObjectViaDoor walks ExitStart/End; TransportContain
    /// onRemoving applies GoAggressiveOnExit. Garrison keeps drop_position.
    /// TunnelContain does not override exitObjectViaDoor — execute_exit must
    /// walk the *exit* entrance (not `contained_by` / entry) via
    /// `unit_command_exit_via_open_contain`.
    pub fn unit_command_exit_drop(
        &mut self,
        id: ObjectId,
        drop_position: glam::Vec3,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_exit_drop_with_effects(id, drop_position, &mut Default::default())
    }

    pub(crate) fn unit_command_exit_drop_with_effects(
        &mut self,
        id: ObjectId,
        drop_position: glam::Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        let container_id = self.objects.get(&id).and_then(|u| u.contained_by);
        let walk = container_id.is_some_and(|cid| {
            self.objects
                .get(&cid)
                .is_some_and(|c| !c.is_garrison_contain() && !c.is_tunnel_network_style_container())
        });
        if let (true, Some(cid)) = (walk, container_id) {
            return Ok(self.unit_command_exit_via_open_contain_with_effects(id, cid, effects));
        }
        // TransportContain::onRemoving wakes only previously idle AI, before
        // exitObjectViaDoor replaces the AI state with an exit/follow order.
        if let Some(cid) = container_id {
            self.reset_rider_mood_check_on_exit(cid, id);
        }
        let go_aggressive = container_id
            .and_then(|cid| self.objects.get(&cid))
            .is_some_and(|c| c.transport_go_aggressive_on_exit());
        let Some(unit) = self.objects.get_mut(&id) else {
            return Ok(false);
        };
        unit.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        unit.set_position_with_effects(
            drop_position,
            self.frame,
            &mut effects.reborrow_for_object(&mut self.partition_manager),
        );
        let containment_actor_id = unit.id;
        Self::set_owned_containment_in_parts(
            &mut self.objects,
            &mut self.partition_manager,
            containment_actor_id,
            None,
            true,
        );
        let unit = self
            .objects
            .get_mut(&containment_actor_id)
            .expect("containment operation preserves this birth");
        unit.clear_target(&mut crate::game_logic::ObjectAudio::queue(
            &mut self.queued_audio_events,
        ));
        let needs_idle_entry = unit.ai_state() != AIState::Idle;
        drop(unit);
        if needs_idle_entry && !self.enter_base_ai_idle_owned(id, effects)? {
            return Ok(false);
        }
        let unit = self
            .objects
            .get_mut(&id)
            .expect("owned Idle entry preserves this birth");
        if go_aggressive {
            unit.set_ai_attitude(
                crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive,
            );
        }
        unit.set_status_moving(false);
        unit.set_status_attacking(false);
        drop(unit);
        if let Some(cid) = container_id {
            self.play_container_removing_template_sounds(cid, id, effects);
        }
        Ok(true)
    }

    /// C++ OpenContain::exitObjectViaDoor — walk ExitStart/End even after
    /// `remove_occupant` cleared the container list.
    /// TunnelContain inherits this path (no override). Garrison stays burst/side.
    pub fn unit_command_exit_via_open_contain(
        &mut self,
        id: ObjectId,
        container_id: ObjectId,
    ) -> bool {
        self.unit_command_exit_via_open_contain_with_effects(
            id,
            container_id,
            &mut Default::default(),
        )
    }

    pub(crate) fn unit_command_exit_via_open_contain_with_effects(
        &mut self,
        id: ObjectId,
        container_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        if !self.objects.contains_key(&id) || !self.objects.contains_key(&container_id) {
            return false;
        }
        if self
            .objects
            .get(&container_id)
            .is_some_and(|c| c.is_garrison_contain())
        {
            return false;
        }
        self.walk_unit_via_open_contain_exit_with_effects(id, container_id, effects);
        true
    }

    /// Wave 233: mine-clearing weapon-set detail residual.
    pub fn unit_command_set_mine_clearing_detail(&mut self, id: ObjectId, enabled: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_weapon_set_mine_clearing_detail_at_frame(enabled, self.frame);
        true
    }

    /// Wave 233: evacuate-on-stop pending flags residual.
    pub fn unit_command_set_pending_evacuate(
        &mut self,
        id: ObjectId,
        pending_evacuate_on_stop: bool,
        pending_exit_after_evacuate: bool,
        prep_move: bool,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if prep_move && obj.transport_return_state.is_some() {
            // AIUpdate.cpp3042–3048: clear/goal/state setters are locked,
            // but command bookkeeping still executes in its original order.
            obj.num_frames_blocked = 0;
            obj.is_blocked = false;
            obj.is_blocked_and_stuck = false;
            return true;
        }
        if prep_move && pending_evacuate_on_stop && obj.chinook_ai.is_none() {
            if !obj.is_mobile_for_ai_command() {
                return false;
            }
            obj.transport_return_state = if pending_exit_after_evacuate {
                Some(crate::game_logic::host_transport_return::TransportReturnState::MoveAndEvacuate { origin: obj.get_position() })
            } else {
                None
            };
        } else if !pending_evacuate_on_stop {
            obj.transport_return_state = None;
        }
        obj.pending_evacuate_on_stop = pending_evacuate_on_stop;
        obj.pending_exit_after_evacuate = pending_exit_after_evacuate;
        if !pending_evacuate_on_stop || prep_move {
            // Arrival-gated evacuate must not inherit a prior ExitDelay stream.
            obj.pending_stream_exit = false;
        }
        if prep_move {
            obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            obj.set_force_attack(false);
            obj.set_guard_position(
                None,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            obj.set_guard_target(
                None,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            obj.end_guard_retaliate(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
        }
        true
    }

    /// Wave 233: order target + Entering state (deploy-to-garrison residual).
    pub fn unit_command_order_enter(&mut self, id: ObjectId, target_id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        unit.set_order_target_with_operation(
            Some(target_id),
            &mut self.attack_events,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            self.frame,
            &mut crate::game_logic::ScriptPresentationEffects::default()
                .reborrow_for_object(&mut self.partition_manager),
        );
        unit.set_ai_state(
            AIState::Entering,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        // C++ AIEnterState::onEnter ignoreObstacle(goalObject).
        unit.ignored_obstacle_id = Some(target_id);
        true
    }

    /// Wave 233: set weapon-set flag residual.
    pub fn unit_command_set_weapon_set_flag(
        &mut self,
        id: ObjectId,
        flag: u8,
        enabled: bool,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        if flag == 1 {
            unit.set_weapon_set_mine_clearing_detail_at_frame(enabled, self.frame);
            true
        } else {
            unit.set_weapon_set_flag(flag, enabled)
        }
    }

    /// Wave 233: surrender residual.
    pub fn unit_command_set_surrendered(&mut self, id: ObjectId, surrendered: bool) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_surrendered(
            surrendered,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// Wave 233: set AI state if alive.
    pub fn unit_command_set_ai_state(
        &mut self,
        id: ObjectId,
        state: AIState,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_set_ai_state_with_effects(
            id,
            state,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn unit_command_set_ai_state_with_effects(
        &mut self,
        id: ObjectId,
        state: AIState,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.cancel_dock_reservation(id);
        if self.objects.get(&id).is_none() {
            return Ok(false);
        }
        self.set_ai_state_decision_aware_with_effects(id, state, effects)
    }

    /// Select a concrete weapon slot for a player-issued `FIRE_WEAPON` command.
    ///
    /// Every accepted ordinal must map to a real concrete WeaponSet slot.
    /// Unknown slots fail closed; in particular, TERTIARY never falls through
    /// to PRIMARY.
    pub fn unit_command_select_weapon_slot(&mut self, id: ObjectId, slot: u8) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        let has_requested_slot = matches!(slot, 0..=2) && unit.weapon_slot(slot).is_some();
        if !has_requested_slot {
            return false;
        }

        // C++ Object::doCommandButton sets a temporary weapon lock before it
        // starts the attack, so the command cannot be re-routed by auto-choose.
        unit.set_weapon_lock(slot, WeaponLockType::LockedTemporarily)
    }

    /// Wave 233: weapon fire at object/location residual.
    pub fn unit_command_fire_weapon(
        &mut self,
        id: ObjectId,
        target_object: Option<ObjectId>,
        target_location: Option<glam::Vec3>,
        max_shots_to_fire: i32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }

        // A new player order replaces a nested AI attack machine in retail.
        // Without clearing these ownership bits, update_combat correctly
        // defers to the nested machine, which can leave a manual TERTIARY
        // command behind an old auto-acquired target state.
        unit.set_status_aiming_weapon(false);
        unit.set_status_firing_weapon(false);
        unit.set_attack_substate(AttackSubState::AimAtTarget);

        if let Some(tid) = target_object {
            unit.set_target_with_operation(
                Some(tid),
                &mut self.attack_events,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                self.frame,
                &mut crate::game_logic::ScriptPresentationEffects::default()
                    .reborrow_for_object(&mut self.partition_manager),
            );
            unit.set_max_shots_to_fire(max_shots_to_fire);
            true
        } else if let Some(pos) = target_location {
            unit.target = None;

            unit.set_target_location(
                Some(pos),
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.set_force_attack(true);
            unit.set_ai_state(
                AIState::AttackingGround,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.set_max_shots_to_fire(max_shots_to_fire);
            drop(unit);
            self.set_turret_target_position(id, Some(pos));
            true
        } else {
            false
        }
    }

    /// Wave 233: infantry go-prone residual.
    pub fn unit_command_go_prone(&mut self, id: ObjectId, prone_secs: f32) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        if unit.is_kind_of(KindOf::Structure) || unit.is_kind_of(KindOf::Immobile) {
            return false;
        }
        let is_infantry =
            unit.is_kind_of(KindOf::Infantry) || unit.object_type == ObjectType::Infantry;
        if !is_infantry {
            return false;
        }
        unit.go_prone(
            prone_secs,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// Wave 233: emoticon residual.
    pub fn unit_command_set_emoticon(
        &mut self,
        id: ObjectId,
        name: &str,
        duration_frames: i32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.set_emoticon(name, duration_frames);
        true
    }

    /// Wave 233: weapon lock residual.
    /// DemoTrapUpdate: PRIMARY=detonate, SECONDARY=proximity, TERTIARY=manual.
    pub fn unit_command_set_weapon_lock(
        &mut self,
        id: ObjectId,
        slot: u8,
        lock_type: WeaponLockType,
    ) -> bool {
        self.unit_command_set_weapon_lock_with_effects(id, slot, lock_type, &mut Default::default())
    }

    pub(crate) fn unit_command_set_weapon_lock_with_effects(
        &mut self,
        id: ObjectId,
        slot: u8,
        lock_type: WeaponLockType,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        let is_demo_trap = self
            .objects
            .get(&id)
            .and_then(|unit| unit.mine_data.as_ref())
            .is_some_and(|md| {
                matches!(
                    md.kind,
                    crate::game_logic::host_mines::HostMineKind::DemoTrap
                ) && !md.detonated
            });
        let locked = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            if !unit.is_alive() {
                return false;
            }
            unit.set_weapon_lock(slot, lock_type)
        };
        if locked && is_demo_trap {
            use crate::game_logic::host_mines::DemoTrapMode;
            let mode = match slot {
                0 => DemoTrapMode::Detonate,
                1 => DemoTrapMode::Proximity,
                2 => DemoTrapMode::Manual,
                _ => return locked,
            };
            let _ = self.set_demo_trap_mode_with_effects(id, mode, effects);
        }
        locked
    }

    /// Wave 233: release weapon lock residual.
    pub fn unit_command_release_weapon_lock(
        &mut self,
        id: ObjectId,
        lock_type: WeaponLockType,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        unit.release_weapon_lock(lock_type);
        true
    }

    /// Wave 233: switch weapons residual.
    pub fn unit_command_switch_weapons(&mut self, id: ObjectId) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let candidates = match unit.active_weapon_slot {
            0 => [1u8, 2u8, 0u8],
            1 => [2u8, 0u8, 1u8],
            2 => [0u8, 1u8, 2u8],
            _ => [0u8, 1u8, 2u8],
        };
        let next = candidates
            .into_iter()
            .find(|slot| unit.weapon_slot(*slot).is_some());
        let Some(next) = next else {
            return false;
        };
        let _ = unit.set_weapon_lock(next, WeaponLockType::LockedPermanently);
        unit.set_ai_state(
            AIState::SpecialAbility,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        true
    }

    /// C++ `setSpecialPowerOverridableDestination` on PUC / Spectre
    /// (SpectreGunshipUpdate.cpp:268-283).
    ///
    /// Gates: `!isDisabled()` (the whole click is dropped on a disabled ship)
    /// and gunship status `< DEPARTING`. Stores the dest on the live Object,
    /// then drives any matching `HostParticleBeamField` / the bound ship's
    /// `HostSpectreOrbitField` (and the gunship's `override_target`)
    /// immediately — not leftover-only. For the local controlling player the
    /// template's VoiceAttack plays on the ship (object id).
    pub fn unit_command_set_special_power_overridable_destination(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
    ) -> bool {
        let (producer, overridable) = {
            let Some(unit) = self.objects.get_mut(&id) else {
                return false;
            };
            // C++ :271 — `!me->isDisabled()` gates the whole click.
            if !unit.is_alive() || unit.is_disabled() {
                return false;
            }
            unit.set_special_power_overridable_destination(location, None);
            let mut overridable = true;
            if let Some(flight) = unit.spectre_gunship_update.as_mut() {
                // C++ :271-281 — the click lands only while status < DEPARTING;
                // it is stored UNCLAMPED. The AttackAreaRadius -
                // TargetingReticleRadius constraint applies in the update tick
                // (constrain_override against the FIXED initial target).
                overridable = flight.status.overridable_destination_active();
                if overridable {
                    flight.override_target = location;
                }
            }
            (unit.producer_id, overridable)
        };
        if overridable {
            let frame = self.frame;
            self.special_power_strikes
                .apply_source_override_destination(id, location, frame);
            if let Some(producer) = producer {
                if producer != id {
                    self.special_power_strikes
                        .apply_source_override_destination(producer, location, frame);
                }
            }
            // C++ :275-280 — VoiceAttack for the local controlling player.
            let local_owner = self
                .objects
                .get(&id)
                .and_then(|o| self.player_owner_for_host_object(o))
                .and_then(|pid| self.players.get(&pid))
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local_owner {
                self.queue_resolved_per_unit_sound(id, "VoiceAttack", true, false, None, 140);
            }
        }
        true
    }

    /// Wave 233: queue upgrade on producer building residual.
    pub fn unit_command_building_add_upgrade_to_queue(
        &mut self,
        id: ObjectId,
        upgrade_name: &str,
        research_secs: f32,
        cost: Resources,
    ) -> bool {
        let object_scoped = self.is_object_scoped_upgrade(upgrade_name);
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        // C++ ProductionUpdate::queueUpgrade OBJECT hasUpgrade / !affectedByUpgrade.
        if object_scoped && obj.refuses_object_upgrade(upgrade_name) {
            return false;
        }
        // C++ ProductionUpdate::queueUpgrade — STOP cheaters:
        // Object::canProduceUpgrade CommandSet walk.
        if !crate::game_logic::host_upgrades::object_can_produce_upgrade(
            &self.control_bar_overrides,
            obj,
            upgrade_name,
        ) {
            return false;
        }

        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        building.add_upgrade_to_queue(upgrade_name.to_string(), research_secs, cost)
    }

    /// C++ ProductionUpdate::canQueueUpgrade — QUEUE_FULL when the producer
    /// production queue is at capacity (ControlBarCommandProcessing.cpp:505-509
    /// fires GUI:ProductionQueueFull on the click).
    pub fn producer_upgrade_queue_is_full(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .map(|obj| {
                obj.building_data.as_ref().is_some_and(|building| {
                    building.production_queue.len()
                        >= crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT
                })
            })
            .unwrap_or(true)
    }

    /// Wave 233: remove upgrade entry from producer production queue.
    pub fn unit_command_building_remove_upgrade_from_queue(
        &mut self,
        id: ObjectId,
        upgrade_name: &str,
    ) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let Some(building) = obj.building_data.as_mut() else {
            return false;
        };
        let before = building.production_queue.len();
        building.production_queue.retain(|item| {
            !(item.is_upgrade() && item.template_name.eq_ignore_ascii_case(upgrade_name))
        });
        building.production_queue.len() < before
    }

    /// Wave 233: path then set AI state (executor path_to_goal_with_state residual).
    pub fn unit_command_path_with_state(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_path_with_state_ignoring(id, goal, state, None)
    }

    /// C++ `ignoreObstacle(goalObject)` then move (DozerAIUpdate.cpp:210-211).
    pub fn unit_command_path_with_state_ignoring(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
        ignore_obstacle: Option<ObjectId>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_path_with_state_ignoring_with_effects(
            id,
            goal,
            state,
            ignore_obstacle,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn unit_command_path_with_state_ignoring_with_effects(
        &mut self,
        id: ObjectId,
        goal: glam::Vec3,
        state: AIState,
        ignore_obstacle: Option<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        if !self.assign_unit_path_ignoring(id, goal, &[], ignore_obstacle) {
            return Ok(false);
        }
        self.cancel_dock_reservation(id);
        if self.objects.get(&id).is_none() {
            return Ok(false);
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            if ignore_obstacle.is_some() {
                unit.ignored_obstacle_id = ignore_obstacle;
            } else if !matches!(state, AIState::Entering) {
                unit.ignored_obstacle_id = None;
            }
        }
        self.set_ai_state_decision_aware_with_effects(id, state, effects)?;
        if let Some(unit) = self.objects.get_mut(&id) {
            if !unit.movement.path.is_empty() {
                unit.set_status_moving(true);
            }
        }
        Ok(true)
    }

    /// Wave 233: C++ groupIdle stealth mood delay residual for one unit.
    /// C++ AIGroup.cpp:2042-2061: CAN_STEALTH + canAutoAcquire + not STEALTHED/DETECTED
    /// **and** !canAutoAcquireWhileStealthed. Host residual for the while-stealthed
    /// gate is `stealth_breaks_on_attack` (units that fight while stealthed skip).
    pub fn unit_command_apply_stealth_mood_delay(
        &mut self,
        id: ObjectId,
        now_frame: u32,
        skew: u32,
    ) -> bool {
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        let can_stealth = unit.innate_stealth || unit.stealth_delay_frames > 0;
        if can_stealth
            && unit.auto_acquire_when_idle()
            && unit.can_attack()
            && !unit.status.stealthed
            && !unit.status.detected
            && unit.stealth_breaks_on_attack
        {
            let delay = unit.stealth_delay_frames.max(1);
            unit.next_mood_check_time = now_frame.saturating_add(delay).saturating_add(skew);
            return true;
        }
        false
    }

    #[inline]
    pub fn unit_position_if_movable(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects
            .get(&id)
            .filter(|o| o.can_move())
            .map(|o| o.get_position())
    }

    /// Wave 227: world position probe without exposing `&Object` to engine dual-read paths.
    #[inline]
    pub fn object_position(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects.get(&id).map(|o| o.get_position())
    }

    /// Wave 224: host residual — force-complete an under-construction structure
    /// (train/construct producer path). Authority mutation owned by GameLogic.
    /// C++ completion (DozerAIUpdate.cpp:536-561 / BuildAssistant.cpp:361-418)
    /// never writes max health; persist scaffold damage.
    pub fn force_complete_construction(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.construction_percent = 1.0;
        obj.status.under_construction = false;
        true
    }

    /// Wave 224: host residual — ensure barracks `building_data` for force-picked
    /// producers so production queue identity is honest without engine dual-scan.
    pub fn ensure_barracks_building_data(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        let need_bd = obj.building_data.is_none()
            || obj
                .building_data
                .as_ref()
                .map(|b| !matches!(b.building_type, BuildingType::Barracks))
                .unwrap_or(true);
        let name_ok = obj.template_name.to_ascii_lowercase().contains("barracks")
            || obj.is_kind_of(KindOf::FSBarracks);
        // Wave 834: also accept force-complete residual producers that already
        // look like infantry factories (Barracks building_type stamp only).
        if need_bd && name_ok {
            // Mirror engine residual: stamp Barracks building_data when missing/mismatched.
            obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
            return true;
        }
        false
    }

    /// Wave 834: force-stamp Barracks building_data for auto_target train residual
    /// when the producer is already known (host spawn / force-complete path).
    pub fn force_ensure_barracks_building_data(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        obj.building_data = Some(BuildingData::new(BuildingType::Barracks));
        obj.status.under_construction = false;
        obj.construction_percent = 1.0;
        true
    }

    /// Wave 225: clear movement path / target on a unit (host residual).
    pub fn clear_unit_movement_path(&mut self, id: ObjectId) -> bool {
        let Some(obj) = self.objects.get_mut(&id) else {
            return false;
        };
        if obj.movement.path.is_empty() && obj.movement.target_position.is_none() {
            return false;
        }
        obj.movement.path.clear();
        obj.movement.current_path_index = 0;
        obj.movement.target_position = None;
        obj.status.moving = false;
        true
    }

    /// Wave 225: adjust guard radius residual on a unit. Returns new radius when applied.
    pub fn adjust_unit_guard_radius(&mut self, id: ObjectId, delta: f32) -> Option<f32> {
        let obj = self.objects.get_mut(&id)?;
        let guarding = matches!(
            obj.ai_state(),
            AIState::GuardingArea | AIState::GuardingObject
        ) || obj.active_guard_position().is_some()
            || obj.active_guard_target().is_some();
        if !guarding && obj.guard_radius <= 0.0 {
            let base = obj.selection_radius.max(20.0) * 2.0;
            obj.guard_radius = (base + delta).clamp(30.0, 400.0);
        } else {
            let cur = if obj.guard_radius > 1.0 {
                obj.guard_radius
            } else {
                obj.selection_radius.max(20.0) * 2.0
            };
            obj.guard_radius = (cur + delta).clamp(30.0, 400.0);
        }
        Some(obj.guard_radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three_slot_logic() -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        logic.templates.insert(
            "ThreeSlotTest".to_string(),
            ThingTemplate::new("ThreeSlotTest"),
        );
        let id = logic
            .create_object("ThreeSlotTest", Team::USA, glam::Vec3::ZERO)
            .expect("test object");
        let object = logic.host_object_mut(id).expect("test object mut");
        object.weapon = Some(Weapon {
            damage: 1.0,
            range: 100.0,
            ..Weapon::default()
        });
        object.secondary_weapon = Some(Weapon {
            damage: 2.0,
            range: 100.0,
            ..Weapon::default()
        });
        object.tertiary_weapon = Some(Weapon {
            damage: 3.0,
            range: 100.0,
            ..Weapon::default()
        });
        (logic, id)
    }

    #[test]
    fn select_tertiary_slot_is_concrete_and_temporary_locked() {
        let (mut logic, id) = three_slot_logic();
        assert!(logic.unit_command_select_weapon_slot(id, 2));
        let object = logic.host_object(id).expect("selected object");
        assert_eq!(object.active_weapon_slot, 2);
        assert_eq!(object.weapon_lock_slot, 2);
        assert_eq!(object.weapon_lock_type, WeaponLockType::LockedTemporarily);
        assert_eq!(object.weapon_slot(2).map(|weapon| weapon.damage), Some(3.0));
        assert!(object.weapon_slot(3).is_none(), "unknown slots fail closed");
    }

    #[test]
    fn absent_or_unknown_tertiary_selection_cannot_fall_back_to_primary() {
        let (mut logic, id) = three_slot_logic();
        logic.host_object_mut(id).expect("object").tertiary_weapon = None;
        assert!(!logic.unit_command_select_weapon_slot(id, 2));
        assert!(!logic.unit_command_select_weapon_slot(id, 7));
        let object = logic.host_object(id).expect("object");
        assert_eq!(object.weapon_slot(0).map(|weapon| weapon.damage), Some(1.0));
        assert!(object.weapon_slot(2).is_none());
    }

    #[test]
    fn player_tertiary_fire_order_replaces_an_existing_nested_attack() {
        let (mut logic, id) = three_slot_logic();
        let target_id = logic
            .create_object("ThreeSlotTest", Team::GLA, glam::Vec3::new(20.0, 0.0, 0.0))
            .expect("target");
        {
            let object = logic.host_object_mut(id).expect("object");
            object.status.is_aiming_weapon = true;
            object.status.is_firing_weapon = true;
            object.set_attack_substate(AttackSubState::FireWeapon);
        }

        assert!(logic.unit_command_select_weapon_slot(id, 2));
        assert!(logic.unit_command_fire_weapon(id, Some(target_id), None, -1));

        let object = logic.host_object(id).expect("object");
        assert_eq!(object.target, Some(target_id));
        assert_eq!(object.weapon_lock_type, WeaponLockType::LockedTemporarily);
        assert_eq!(object.weapon_lock_slot, 2);
        assert!(!object.status.is_aiming_weapon);
        assert!(!object.status.is_firing_weapon);
        assert_eq!(object.attack_substate(), AttackSubState::AimAtTarget);
    }

    #[test]
    fn switch_weapons_cycles_to_tertiary_when_it_is_next_available_slot() {
        let (mut logic, id) = three_slot_logic();
        logic
            .host_object_mut(id)
            .expect("object")
            .active_weapon_slot = 1;
        assert!(logic.unit_command_switch_weapons(id));
        let object = logic.host_object(id).expect("object");
        assert_eq!(object.active_weapon_slot, 2);
        assert_eq!(object.weapon_lock_slot, 2);
        assert_eq!(object.weapon_lock_type, WeaponLockType::LockedPermanently);
    }

    /// hq-65aus: path fail must not set_destination (straight-line through obstacles).
    #[test]
    fn attack_move_path_fail_does_not_set_destination() {
        use crate::game_logic::host_deploy_style::{HostDeployStyleData, HostDeployStyleState};
        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("AtkMvBlock".to_string(), ThingTemplate::new("AtkMvBlock"));
        let id = logic
            .create_object("AtkMvBlock", Team::USA, glam::Vec3::ZERO)
            .expect("unit");
        {
            let unit = logic.host_object_mut(id).expect("unit");
            unit.weapon = Some(Weapon {
                range: 80.0,
                damage: 10.0,
                ..Weapon::default()
            });
            unit.deploy_style = Some(HostDeployStyleData {
                state: HostDeployStyleState::ReadyToAttack,
                ready_frame: 0,
                pack_frames: 30,
                unpack_frames: 30,
                ..Default::default()
            });
        }
        let dest = glam::Vec3::new(250.0, 0.0, 0.0);
        assert!(
            logic
                .unit_command_attack_move_to(id, dest)
                .expect("checked AttackMove command")
        );
        let (state, stored, retry, sleep, target) = {
            let unit = logic.host_object(id).expect("unit");
            (
                unit.ai_state().clone(),
                unit.requested_destination,
                unit.attack_move_retry_count,
                unit.attack_move_sleep_until,
                unit.movement.target_position,
            )
        };
        assert_eq!(state, AIState::AttackMoving);
        assert_eq!(stored, Some(logic.clip_player_goal(id, dest)));
        assert_eq!(retry, 5);
        assert_eq!(sleep, 0);
        assert!(
            target.is_none(),
            "path fail must not walk a raw dest line; got {target:?}"
        );
    }

    #[test]
    fn rally_point_set_message_substitutes_building_display_name() {
        assert_eq!(
            format_rally_point_set_message("Rally point set for %s.", "War Factory"),
            "Rally point set for War Factory."
        );
        assert_eq!(
            format_rally_point_set_message("GUI:RallyPointSet", "Barracks"),
            "Rally point set for Barracks"
        );
        assert_eq!(
            format_rally_point_set_message("MISSING: 'GUI:RallyPointSet'", "Command Center"),
            "Rally point set for Command Center"
        );
    }

    fn rally_factory_logic() -> (GameLogic, ObjectId) {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("RallyFactory");
        tmpl.display_name = "War Factory".to_string();
        tmpl.add_kind_of(KindOf::AutoRallypoint);
        logic.templates.insert("RallyFactory".to_string(), tmpl);
        let id = logic
            .create_object("RallyFactory", Team::USA, glam::Vec3::ZERO)
            .expect("rally factory");
        let obj = logic.host_object_mut(id).expect("factory mut");
        obj.building_data = Some(BuildingData::new(BuildingType::WarFactory));
        (logic, id)
    }

    #[test]
    fn set_rally_point_stores_location_on_auto_rallypoint_building() {
        let (mut logic, id) = rally_factory_logic();
        let dest = glam::Vec3::new(40.0, 0.0, 12.0);
        assert!(logic.unit_command_set_rally_point(id, dest));
        let obj = logic.host_object(id).expect("factory");
        assert_eq!(obj.get_display_name(), "War Factory");
        assert_eq!(
            obj.building_data.as_ref().and_then(|b| b.rally_point),
            Some(dest)
        );
    }

    #[test]
    fn set_rally_point_rejects_non_auto_rallypoint() {
        let mut logic = GameLogic::new();
        logic
            .templates
            .insert("PowerPlant".to_string(), ThingTemplate::new("PowerPlant"));
        let id = logic
            .create_object("PowerPlant", Team::USA, glam::Vec3::ZERO)
            .expect("plant");
        let obj = logic.host_object_mut(id).expect("plant mut");
        obj.building_data = Some(BuildingData::new(BuildingType::PowerPlant));
        assert!(!logic.unit_command_set_rally_point(id, glam::Vec3::new(10.0, 0.0, 10.0)));
    }

    /// hq-9fqep: player attack/move replaces AI_HUNT; hunt does not resume.
    fn hunting_infantry_pair() -> (GameLogic, ObjectId, ObjectId) {
        let mut logic = GameLogic::new();
        let hunter_id = ObjectId(3203);
        let mut hunter_template = ThingTemplate::new("HuntAttackSource");
        hunter_template.add_kind_of(KindOf::Infantry);
        logic.objects.insert(hunter_id, {
            let mut hunter = Object::new(hunter_template, hunter_id, Team::USA);
            hunter.set_position(glam::Vec3::ZERO);
            hunter.weapon = Some(Weapon {
                range: 100.0,
                damage: 10.0,
                can_target_ground: true,
                ..Weapon::default()
            });
            hunter.secondary_weapon = Some(Weapon {
                range: 80.0,
                damage: 5.0,
                can_target_ground: true,
                ..Weapon::default()
            });
            hunter.set_hunting(true);
            hunter.set_ai_state(
                AIState::Patrolling,
                &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
            );
            let _ = hunter.set_weapon_lock(1, WeaponLockType::LockedTemporarily);
            hunter
        });
        let enemy_id = ObjectId(3204);
        let mut enemy_template = ThingTemplate::new("HuntAttackTarget");
        enemy_template.add_kind_of(KindOf::Infantry);
        logic.objects.insert(enemy_id, {
            let mut enemy = Object::new(enemy_template, enemy_id, Team::GLA);
            enemy.set_position(glam::Vec3::new(10.0, 0.0, 0.0));
            enemy
        });
        (logic, hunter_id, enemy_id)
    }

    #[test]
    fn player_attack_ends_hunt_and_releases_temp_lock() {
        let (mut logic, hunter_id, enemy_id) = hunting_infantry_pair();
        assert!(
            logic
                .unit_command_attack(hunter_id, enemy_id)
                .expect("valid attack command fixture")
        );
        let hunter = logic.objects.get(&hunter_id).expect("hunter");
        assert!(!hunter.is_hunting(), "player attack must end Hunt");
        assert_eq!(hunter.ai_state(), AIState::Attacking);
        assert_eq!(hunter.target, Some(enemy_id));
        assert_eq!(
            hunter.weapon_lock_type,
            WeaponLockType::NotLocked,
            "AIHuntState::onExit releases LOCKED_TEMPORARILY"
        );
        if let Some(unit) = logic.objects.get_mut(&hunter_id) {
            unit.clear_target(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
            unit.set_ai_state(
                AIState::Idle,
                &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
            );
        }
        logic
            .update_ai(&[hunter_id, enemy_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        let hunter = logic.objects.get(&hunter_id).expect("hunter after tick");
        assert!(!hunter.is_hunting());
        assert!(
            !matches!(hunter.ai_state, AIState::Patrolling),
            "Hunt must not resume after the player attack finishes; got {:?}",
            hunter.ai_state()
        );
    }

    #[test]
    fn player_move_ends_hunt_and_does_not_resume() {
        let (mut logic, hunter_id, enemy_id) = hunting_infantry_pair();
        assert!(logic.unit_command_move_to(hunter_id, glam::Vec3::new(40.0, 0.0, 0.0)));
        let hunter = logic.objects.get(&hunter_id).expect("hunter");
        assert!(!hunter.is_hunting(), "player move must end Hunt");
        assert_eq!(hunter.ai_state(), AIState::Moving);
        assert_eq!(
            hunter.weapon_lock_type,
            WeaponLockType::NotLocked,
            "AIHuntState::onExit releases LOCKED_TEMPORARILY"
        );
        if let Some(unit) = logic.objects.get_mut(&hunter_id) {
            unit.set_ai_state(
                AIState::Idle,
                &mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()),
            );
            unit.clear_target(&mut crate::game_logic::ObjectAudio::queue(&mut Vec::new()));
        }
        logic
            .update_ai(&[hunter_id, enemy_id], 1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        let hunter = logic.objects.get(&hunter_id).expect("hunter after tick");
        assert!(!hunter.is_hunting());
        assert!(
            !matches!(hunter.ai_state, AIState::Patrolling),
            "Hunt must not resume after the player move finishes; got {:?}",
            hunter.ai_state()
        );
    }

    #[test]
    fn add_upgrade_to_queue_refuses_command_set_without_button() {
        let mut logic = GameLogic::new();
        let mut tmpl = ThingTemplate::new("AmericaBarracks");
        tmpl.add_kind_of(KindOf::Structure).set_health(1000.0);
        logic.templates.insert("AmericaBarracks".into(), tmpl);
        let id = logic
            .create_object("AmericaBarracks", Team::USA, glam::Vec3::ZERO)
            .expect("barracks");
        let cost = Resources {
            supplies: 800,
            power: 0,
        };
        assert!(
            !logic.unit_command_building_add_upgrade_to_queue(
                id,
                "Upgrade_AmericaSupplyLines",
                30.0,
                cost,
            ),
            "C++ canProduceUpgrade refuses SupplyLines at Barracks"
        );
    }
}

#[cfg(test)]
#[path = "unit_commands_exit_mood_tests.rs"]
mod exit_mood_tests;

impl GameLogic {
    pub(crate) fn unit_command_standard_ai_idle_owned(
        &mut self,
        id: ObjectId,
    ) -> gamelogic::GameLogicResult<bool> {
        self.unit_command_standard_ai_idle_tail_owned(id, &mut Default::default())
    }
    pub(super) fn unit_command_standard_ai_idle_tail_owned(
        &mut self,
        id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<bool> {
        self.enter_base_ai_idle_owned(id, effects)
    }
}
