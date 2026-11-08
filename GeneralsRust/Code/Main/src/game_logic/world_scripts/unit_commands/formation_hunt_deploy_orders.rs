//! Construct, formation, patrol, hunt and deploy orders.

use super::*;

impl GameLogic {
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
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_ai_state(
            AIState::AttackingGround,
            self.frame,
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
                unit.unit_ai_runtime.cancel_tighten_path_check();
                end_hunt_on_player_parent_order(unit);
                unit.ignored_obstacle_id = None;
                unit.pending_move = Some(clipped);
                unit.movement.target_position = None;
                unit.movement.path.clear();
                unit.set_ai_state(
                    AIState::Moving,
                    self.frame,
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
            unit.unit_ai_runtime.cancel_tighten_path_check();
            end_hunt_on_player_parent_order(unit);
            unit.ignored_obstacle_id = None;
            unit.set_ai_state(
                AIState::Moving,
                self.frame,
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
            unit.unit_ai_runtime.cancel_tighten_path_check();
            unit.ignored_obstacle_id = None;
            if clear_formation && unit.formation_id != 0 {
                unit.set_formation(0, glam::Vec2::ZERO);
            }
            unit.set_ai_state(
                AIState::Moving,
                self.frame,
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
        self.remove_registered_path_goal(id);
        let cannot_path_yet = self
            .objects
            .get(&id)
            .is_some_and(|unit| unit.is_alive() && !unit.can_move());
        if cannot_path_yet {
            if let Some(unit) = self.objects.get_mut(&id) {
                unit.set_adjust_destinations(false);
                unit.is_approach_path = true;
                unit.is_final_goal = true;
                Self::accept_tighten_order(
                    unit,
                    destination,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                unit.unit_ai_runtime.begin_tighten_path_check();
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
        if !self.assign_unit_approach_path(id, destination) {
            return false;
        }
        if let Some(unit) = self.objects.get_mut(&id) {
            Self::accept_tighten_order(
                unit,
                destination,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.unit_ai_runtime.begin_tighten_path_check();
            return true;
        }
        false
    }

    pub(super) fn accept_tighten_order(
        unit: &mut crate::game_logic::Object,
        destination: glam::Vec3,
        frame: u32,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        unit.stop_attack(audio);
        unit.set_formation(0, glam::Vec2::ZERO);
        unit.set_guard_position(None, frame, audio);
        unit.set_guard_target(None, frame, audio);
        unit.end_guard_retaliate(audio);
        if !unit.can_move() {
            unit.pending_move = Some(destination);
            unit.movement.target_position = None;
            unit.movement.path.clear();
        }
        unit.set_ai_state(AIState::Moving, frame, audio);
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
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            } else if let Some(pos) = position {
                unit.set_guard_target(
                    None,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                unit.set_guard_position(
                    Some(pos),
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            unit.set_ai_state(
                ai_state.clone(),
                self.frame,
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
        self.apply_owned_attack_position(id, location, max_shots, true)
    }

    /// C++ AIUpdateInterface::aiAttackPosition enters the non-forced attack
    /// state. The group checks passenger eligibility before this call; owners
    /// and spawned children receive the command directly.
    pub(crate) fn unit_command_attack_position(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
        max_shots: i32,
    ) -> gamelogic::GameLogicResult<bool> {
        self.stamp_player_command_source(id);
        let Some((range, contact, source_birth, source_off_map)) =
            self.host_object(id).map(|source| {
                let range = source.selected_weapon_slot().map_or(0.0, |slot| {
                    source.selected_native_weapon_rule(slot).map_or_else(
                        || {
                            source
                                .weapon_name_for_slot(slot)
                                .map_or(0.0, |name| self.receiving_continue_attack_range(name))
                        },
                        |weapon| weapon.continue_attack_range.max(0.0),
                    )
                });
                let contact = source.selected_weapon_slot().is_some_and(|slot| {
                    source.selected_native_weapon_rule(slot).map_or_else(
                        || {
                            source.weapon_slot(slot).is_some_and(|weapon| {
                                crate::game_logic::weapon_bootstrap::is_contact_weapon_range(
                                    weapon.range,
                                )
                            })
                        },
                        |weapon| weapon.is_contact_weapon(),
                    )
                });
                (
                    range,
                    contact,
                    std::sync::Arc::downgrade(&source.visual_draw_identity_token),
                    source.status.off_map,
                )
            })
        else {
            return Ok(false);
        };
        let mut shot_limit = max_shots;
        if range > 0.0 {
            if let Some(source) = self.host_object_mut(id) {
                source.set_status_ignoring_stealth(true);
            }
            let best = self
                .partition_manager
                .closest_in_radius(location.x, location.z, range, |raw| {
                    let candidate_id = ObjectId(raw);
                    let candidate = self.host_object(candidate_id)?;
                    let pos = candidate.get_position();
                    let dist = (pos.x - location.x).powi(2) + (pos.z - location.z).powi(2);
                    if source_off_map != candidate.status.off_map {
                        return None;
                    }
                    matches!(
                        self.get_able_to_attack_specific_object(
                            id,
                            candidate_id,
                            AbleToAttackType::NewTarget,
                            true
                        ),
                        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                    )
                    .then_some(dist)
                })
                .map(ObjectId);
            if let Some(source) = self.host_object_mut(id) {
                source.set_status_ignoring_stealth(false);
            }
            if let Some(target) = best {
                let accepted = self.unit_command_attack_with_source(
                    id,
                    target,
                    true,
                    true,
                    &mut Default::default(),
                )?;
                if accepted && self.object_birth_matches(id, &source_birth) {
                    if let Some(source) = self.host_object_mut(id) {
                        source.set_max_shots_to_fire(max_shots);
                    }
                }
                return Ok(accepted);
            }
            shot_limit = 1;
        }
        let goal = if contact {
            self.resolve_contact_attack_position(id, location)?
        } else {
            location
        };
        Ok(self.apply_owned_attack_position(id, goal, shot_limit, false))
    }

    pub(super) fn apply_owned_attack_position(
        &mut self,
        id: ObjectId,
        location: glam::Vec3,
        max_shots: i32,
        forced: bool,
    ) -> bool {
        self.drop_jet_targeters_on_attack_exit(id);
        let Some(unit) = self.objects.get_mut(&id) else {
            return false;
        };
        if !unit.is_alive() {
            return false;
        }
        end_hunt_on_player_parent_order(unit);
        unit.target = None;

        unit.set_force_attack(forced);
        unit.leftover_choose_best_reset_primary_for_ground();
        unit.set_target_location(
            Some(location),
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        unit.set_ai_state(
            AIState::AttackingGround,
            self.frame,
            &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
        );
        // AIUpdate.cpp sets the shot limit after the attack state's onEnter.
        unit.set_max_shots_to_fire(max_shots);
        drop(unit);
        self.set_turret_target_position(id, Some(location));
        true
    }

    /// Wave 232: hunt/patrol residual.
    pub fn unit_command_patrol(&mut self, id: ObjectId) -> bool {
        self.stamp_player_command_source(id);
        self.unit_command_hunt_transition(id)
    }

    /// C++ AIGroup::groupHunt forwards `CMD_FROM_SCRIPT` to every member AI
    /// without changing groups or choosing a locomotor set.
    pub(in crate::game_logic) fn unit_command_script_hunt(&mut self, id: ObjectId) -> bool {
        let Some(object) = self
            .objects
            .get_mut(&id)
            .filter(|object| object.has_ai_update_interface())
        else {
            return false;
        };
        object.last_command_source =
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT;
        self.unit_command_hunt_transition(id)
    }

    pub(in crate::game_logic) fn unit_command_ai_hunt(&mut self, id: ObjectId) -> bool {
        // AIUpdate.cpp3611: reject disabled/IMMOBILE and PROJECTILE before
        // touching the receiving machine or its command source.
        let Some(object) = self.objects.get_mut(&id).filter(|object| {
            object.has_ai_update_interface()
                && object.is_mobile_for_ai_command()
                && !object.is_kind_of(KindOf::Projectile)
        }) else {
            return false;
        };
        object.last_command_source = crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
        self.unit_command_hunt_transition(id)
    }

    pub(super) fn unit_command_hunt_transition(&mut self, id: ObjectId) -> bool {
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
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.set_guard_target(
                None,
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
            unit.end_guard_retaliate(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));

            unit.set_auto_acquire_when_idle(true);
            unit.set_hunting(true);
            unit.set_ai_state(
                AIState::Patrolling,
                self.frame,
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
            self.frame,
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
            let has_turret =
                unit.turret_ai.turret_enabled || unit.turret_ai.turret_turn_rate_rad > 0.0;
            let turret_natural =
                crate::game_logic::host_deploy_style::leftover_host_turret_is_in_natural_position(
                    unit.status.under_construction,
                    unit.turret_ai.turret_angle_deg,
                    unit.turret_ai.turret_pitch_deg,
                    unit.turret_ai.turret_natural_angle_deg,
                    unit.turret_ai.turret_natural_pitch_deg,
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
                    unit.turret_ai.turret_substate =
                        crate::game_logic::object::TurretSubState::Recenter;
                    unit.turret_ai.turret_idle_recentering = true;
                    unit.turret_ai.turret_target_id = None;
                    unit.turret_runtime.target_pos = None;
                    unit.turret_runtime.victim_team = None;
                    unit.turret_ai.turret_holding = false;
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
                self.objects
                    .read()
                    .with_players(&self.players)
                    .with_team_factory(&self.team_factory),
                &ground_source,
            )
        });
        self.register_path_goal_on_layer(
            id,
            adjusted,
            gamelogic::ai::pathfind_astar::PathfindLayerEnum::Ground as u8,
        );
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
            unit.unit_ai_runtime.cancel_tighten_path_check();
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
                self.frame,
                &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
            );
        }
        true
    }
}
