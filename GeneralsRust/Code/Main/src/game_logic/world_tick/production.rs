//! Host tick `impl GameLogic` — `production`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;

/// The two original callback points cannot be interchanged by exit helpers.
enum SpawnReadyCallbacks {
    AlreadyNotified,
    NotifyAfterExit,
}

impl GameLogic {
    /// C++ DozerActionDoActionState BUILD (DozerAIUpdate.cpp:482-526):
    /// progress only after idle at ACTION dock (`DOZER_DO_BUILD_AT_DOCK`).
    /// C++ DozerAIUpdate.cpp:305 — one exclusive builder per structure.

    /// C++ `DozerAIUpdate::internalTaskComplete(DOZER_TASK_BUILD)`.
    /// Clears the task slot and the actively-constructing bit. The end dock
    /// must already have been copied; this also drops `dozer_dock_action`.
    fn release_finished_builders(&mut self, ids: &[ObjectId]) {
        for &oid in ids {
            self.dozer_internal_task_complete(oid, false);
            if let Some(obj) = self.objects.get_mut(&oid) {
                obj.set_actively_constructing(false);
            }
        }
    }

    pub(crate) fn update_construction(&mut self, object_ids: &[ObjectId], dt: f32) {
        self.update_construction_with_effects(object_ids, dt, &mut Default::default());
    }

    pub(crate) fn update_construction_with_effects(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // C++ parity: calcTimeToBuild applies the same power penalty to dozer
        // construction as to production queue speed.
        let player_power_factor = self.compute_player_power_factors();
        // Resolve legacy ownerless objects before taking mutable object borrows.
        // A concrete PlayerId is authoritative; the helper only supplies a
        // compatibility owner when this team has exactly one living player.
        let owner_raw: Vec<(ObjectId, Option<u32>, crate::game_logic::Team)> = self
            .objects
            .values()
            .map(|obj| (obj.id, obj.owner_player_id, obj.team))
            .collect();
        let object_owner_player_ids: std::collections::HashMap<ObjectId, Option<u32>> = owner_raw
            .into_iter()
            .map(|(id, owner_id, team)| (id, self.player_owner_for_event(owner_id, team)))
            .collect();

        // C++ `ThingTemplate::calcTimeToBuild(player)` converts build time to
        // integer logic frames, multiplies Handicap::BUILDTIME (BUILDINGS vs
        // GENERIC) then PlayerTemplate `ProductionTimeChange`, and only then
        // applies low-power timing.
        // Snapshot the pre-power frame counts before mutable Object borrows;
        // they remain player-owned start state and do not depend on tick
        // ordering.
        // CPP DozerAIUpdate.cpp510: timing belongs to the active builder,
        // including an allied player resuming another player's scaffold.
        let authored_time_frames: std::collections::HashMap<ObjectId, u32> = self
            .objects
            .values()
            .filter_map(|builder| {
                let target_id = builder.dozer_task_build_target.or(builder.target)?;
                let target = self.objects.get(&target_id)?;
                let player_id = object_owner_player_ids
                    .get(&builder.id)
                    .copied()
                    .flatten()?;
                let handicap = self
                    .players
                    .get(&player_id)
                    .map(|player| {
                        player.handicap_build_time_multiplier(target.is_kind_of(KindOf::Structure))
                    })
                    .unwrap_or(1.0);
                let template_factor =
                    self.player_template_production_time_factor(player_id, &target.template_name);
                Some((
                    builder.id,
                    Self::cpp_build_time_frames_from_modifiers(
                        target.thing().template.build_time,
                        handicap,
                        template_factor,
                    ),
                ))
            })
            .collect();
        let allied_controllers: std::collections::HashSet<(u32, u32)> = self
            .players
            .keys()
            .flat_map(|&builder| self.players.keys().map(move |&owner| (builder, owner)))
            .filter(|&(builder, owner)| {
                self.player_relationship(builder, owner) == gamelogic::common::Relationship::Allies
            })
            .collect();

        // Pre-scan dozers: exclusive builder + at ACTION dock
        // (C++ DozerActionDoActionState BUILD, DozerAIUpdate.cpp:482-507).
        let dozer_info: Vec<(
            ObjectId,
            Vec3,
            f32,
            Option<u32>,
            Option<ObjectId>,
            Option<ObjectId>,
            Option<ObjectId>,
            Option<Vec3>,
            AIState,
            bool,
        )> = self
            .objects
            .values()
            .filter(|obj| obj.is_alive() && obj.can_construct())
            .map(|obj| {
                (
                    obj.id,
                    obj.get_position(),
                    obj.selection_radius,
                    object_owner_player_ids.get(&obj.id).copied().flatten(),
                    obj.dozer_task_build_target,
                    obj.target,
                    obj.dozer_task_build_target.or(obj.target),
                    obj.dozer_dock_action,
                    obj.ai_state.clone(),
                    obj.movement.path.is_empty() && !obj.waiting_for_path,
                )
            })
            .collect();

        let mut completed_structures: Vec<ObjectId> = Vec::new();
        let mut ready_superweapons: Vec<ObjectId> = Vec::new();
        let mut radar_extend_done: Vec<ObjectId> = Vec::new();
        let mut start_build_loops: Vec<(ObjectId, String, Vec3)> = Vec::new();
        let mut stop_build_loops: Vec<ObjectId> = Vec::new();
        let mut rejected_builders: Vec<ObjectId> = Vec::new();
        let mut arrived_moving: Vec<ObjectId> = Vec::new();
        for &id in object_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if obj.status.under_construction {
                    let build_pos = obj.get_position();
                    let build_radius = obj.selection_radius;
                    let site_template = obj.template_name.clone();
                    let build_owner_player_id = object_owner_player_ids.get(&id).copied().flatten();
                    let exclusive_builder = obj.builder_id.filter(|bid| {
                        dozer_info
                            .iter()
                            .any(|(did, _, _, _, slot, _, _, _, _, _)| {
                                did == bid && *slot == Some(id)
                            })
                    });
                    for (did, _, _, _, build_slot, live_target, _, _, ai_state, _) in &dozer_info {
                        if *build_slot == Some(id)
                            && *live_target == Some(id)
                            && matches!(ai_state, AIState::Constructing)
                            && exclusive_builder != Some(*did)
                        {
                            rejected_builders.push(*did);
                        }
                    }
                    let active_builder = dozer_info
                        .iter()
                        .filter(|(did, pos, radius, owner_player_id, build_slot, live_target, _, stored_dock, ai_state, arrived)| {
                            (match (*owner_player_id, build_owner_player_id) {
                                (Some(builder), Some(owner)) => allied_controllers.contains(&(builder, owner)),
                                (None, None) => true,
                                _ => false,
                            })
                                && (*build_slot == Some(id)
                                    || (*ai_state == AIState::Constructing && *live_target == Some(id)))
                                && match exclusive_builder {
                                    Some(bid) => bid == *did,
                                    None => true,
                                }
                                && (matches!(ai_state, AIState::Constructing)
                                    || (*ai_state == AIState::Moving && *arrived))
                                && {
                                    let dock = crate::game_logic::host_repair::resolve_dozer_action_dock(
                                        *stored_dock,
                                        *pos,
                                        build_pos,
                                        build_radius,
                                    );
                                    let at = crate::game_logic::host_repair::dozer_within_action_dock(
                                        *pos,
                                        *radius,
                                        dock,
                                    );
                                    if at && *ai_state == AIState::Moving {
                                        arrived_moving.push(*did);
                                    }
                                    at
                                }
                        })
                        .next()
                        .map(|builder| (builder.0, builder.3));

                    // C++ DozerAIUpdate: no docked dozer ⇒ no progress. Do not invent a ghost builder.
                    let dozer_count = usize::from(active_builder.is_some());
                    let actively_built = active_builder.is_some();
                    obj.set_under_construction_model_conditions(actively_built);
                    self.construction_model_condition_updates =
                        self.construction_model_condition_updates.saturating_add(1);

                    let power_factor = active_builder
                        .and_then(|(_, player)| player)
                        .and_then(|player_id| player_power_factor.get(&player_id).copied())
                        .unwrap_or(1.0);
                    let authored_frames = active_builder
                        .and_then(|(builder, _)| authored_time_frames.get(&builder).copied())
                        .unwrap_or_else(|| {
                            Self::cpp_build_time_frames_from_modifiers(
                                obj.thing().template.build_time,
                                1.0,
                                1.0,
                            )
                        });
                    let final_frames =
                        Self::cpp_build_time_frames_after_power(authored_frames, power_factor);
                    // Keep the existing zero-duration one-tick safeguard, but
                    // otherwise use C++ calcTimeToBuild's final integer frame
                    // count after the low-energy division.
                    let base_rate = if authored_frames == 0 {
                        100.0
                    } else {
                        30.0 / final_frames.max(1) as f32
                    };
                    let effective_rate = base_rate * dozer_count as f32;
                    let projected = (obj.construction_percent + effective_rate * dt).min(1.0);
                    obj.construction_percent = projected;

                    // C++ DozerAIUpdate.cpp:526 then :536 — increment health first,
                    // then complete. Completion itself never writes max health, so
                    // scaffold damage taken during build persists.
                    if let Some((builder, _)) = active_builder {
                        // CPP526-536: contribution refreshes provenance before
                        // completion callbacks, without changing the controller.
                        obj.producer_id = Some(builder);
                        obj.builder_id = Some(builder);
                        let frames = final_frames.max(1) as f32;
                        let per_frame = obj.health.maximum / frames;
                        let logic_frames = (dt * 30.0).max(0.0);
                        let build_hp = (obj.health.current + per_frame * logic_frames)
                            .clamp(0.0, obj.health.maximum);
                        {
                            obj.internal_change_health(build_hp - obj.health.current);
                            self.health_events
                                .record_applied_heal(id, obj.health.current);
                        }
                    }
                    if projected >= 1.0 {
                        obj.construction_percent = 1.0;
                        obj.set_status_under_construction(false);
                        obj.clear_under_construction_model_conditions();

                        self.construction_events
                            .record(id, obj.template_name.clone());
                        completed_structures.push(id);
                        stop_build_loops.push(id);
                    }

                    if projected < 1.0 && actively_built {
                        start_build_loops.push((id, site_template, build_pos));
                    }
                }
                if obj.tick_timers(dt) {
                    // Defer EVA until after borrow ends.
                    ready_superweapons.push(id);
                }
                // C++ RadarUpdate completes strictly after its own deadline.
                // The object's owner advances this timer; the shadow has no
                // corresponding completion ticker.
                if obj.tick_radar_extend(self.frame) {
                    radar_extend_done.push(id);
                }
                // C++ `ProductionUpdate::updateDoors` owns the visual door
                // state.  GameWorld sole-ticks queue progress, but has no door
                // phase timer; skipping this host tick freezes a factory before
                // WAITING_OPEN and causes speculative shadow spawns to leak.
                // The resulting host event is mirrored into GameWorld at the
                // coupled boundary, so there is no second door timer.
                let _ = obj.tick_production_door(self.frame);
                // Wave 626: under construction sole-tick, GW ready-log owns clear
                // residual; host tick still advances non-sole path.

                if obj.tick_construction_complete_clear(self.frame) {
                    self.construction_complete_clears =
                        self.construction_complete_clears.saturating_add(1);
                }
            }
        }
        for did in arrived_moving {
            if self
                .objects
                .get(&did)
                .is_some_and(|dozer| dozer.ai_state == AIState::Moving)
            {
                self.set_ai_state_decision_aware(did, AIState::Constructing);
            }
        }
        for did in rejected_builders {
            let live_construct = self.objects.get(&did).is_some_and(|dozer| {
                matches!(dozer.ai_state, AIState::Constructing)
                    && dozer.target.is_some()
                    && dozer.target == dozer.dozer_task_build_target
            });
            self.dozer_internal_task_complete(did, false);
            if live_construct {
                if let Some(dozer) = self.objects.get_mut(&did) {
                    dozer.target = None;
                    dozer.set_ai_state(AIState::Idle);
                }
            }
        }

        for (site_id, template_name, pos) in start_build_loops {
            self.start_building_sound(site_id, &template_name, pos);
        }
        for site_id in stop_build_loops {
            self.finish_building_sound(site_id);
        }
        // C++ Player sharedNSync timers advance with the logic frame.
        self.tick_shared_special_power_timers(dt);

        for id in ready_superweapons {
            self.try_eva_superweapon_ready_for_source(id);
        }
        // Wave 618: under sole-tick, GameWorld writeback records SP ready flips;
        // Wave 717: host EVA drain runs after writeback same frame (not mid-update).

        for _id in radar_extend_done {
            self.radar_extend_completes = self.radar_extend_completes.saturating_add(1);
        }

        // C++ parity: when a structure finishes construction, release any dozers
        // that were constructing it — set them to Idle.
        for &completed_id in &completed_structures {
            // CPP539-591: visual preparation, Player and CreateModules all
            // observe the builder's BUILD task before end-dock/task release.
            self.notify_structure_construction_complete_with_effects(completed_id, None, ui);
            let building_pos = self.objects.get(&completed_id).map(|o| o.get_position());
            let mut end_moves: Vec<(ObjectId, glam::Vec3)> = Vec::new();
            let mut finished_builders: Vec<ObjectId> = Vec::new();
            for obj in self.objects.values_mut() {
                let slot = obj.dozer_task_build_target;
                let constructing = obj.is_alive()
                    && obj.ai_state == AIState::Constructing
                    && (slot == Some(completed_id) || obj.target == Some(completed_id));
                let approaching = obj.is_alive()
                    && slot == Some(completed_id)
                    && matches!(obj.ai_state, AIState::Moving | AIState::Idle);
                if !(constructing || approaching) {
                    continue;
                }
                let oid = obj.id;
                if constructing {
                    let dozer_pos = obj.get_position();
                    let stored_action = obj.dozer_dock_action;
                    obj.target = None;
                    obj.target_location = None;

                    obj.set_status_force_attack(false);
                    obj.set_status_attacking(false);
                    if let Some(bpos) = building_pos {
                        end_moves.push((
                            oid,
                            crate::game_logic::host_repair::dozer_complete_end_dock(
                                stored_action,
                                dozer_pos,
                                bpos,
                            ),
                        ));
                    } else {
                        obj.stop_moving();
                        if obj.ai_state != AIState::Idle {
                            obj.set_ai_state(AIState::Idle);
                        }
                        {}
                    }
                } else {
                    let radius = obj.selection_radius;
                    let on_dock_walk =
                        obj.dozer_dock_action
                            .is_some_and(|dock| match obj.movement.path.last() {
                                Some(end) => {
                                    crate::game_logic::host_repair::dozer_within_action_dock(
                                        *end, radius, dock,
                                    )
                                }
                                None => crate::game_logic::host_repair::dozer_within_action_dock(
                                    obj.get_position(),
                                    radius,
                                    dock,
                                ),
                            });
                    if on_dock_walk {
                        if obj.target == Some(completed_id) {
                            obj.target = None;
                            obj.target_location = None;

                            obj.set_status_force_attack(false);
                            obj.set_status_attacking(false);
                        }
                        obj.stop_moving();
                    }
                }
                finished_builders.push(oid);
            }
            // C++ internalTaskComplete(DOZER_TASK_BUILD) after the end-dock
            // move is issued. The dock was already copied into end_moves.
            // A leftover task greys every dozer-construct cameo.
            // Seal first. A path computed before the footprint becomes
            // impassable cannot be followed.
            self.block_structure_object_path(completed_id);
            for (oid, mut end) in end_moves {
                self.adjust_to_possible_destination(oid, &mut end);
                self.path_approach_with_state_ignoring(
                    oid,
                    end,
                    AIState::Moving,
                    Some(completed_id),
                );
                if let Some(unit) = self.objects.get_mut(&oid) {
                    if !unit.movement.path.is_empty() {
                        unit.is_attack_path = false;
                    }
                }
            }
            self.release_finished_builders(&finished_builders);
        }
        // C++ ACTIVELY_CONSTRUCTING residual for dozers/factories.
        // Wave 815: under coupled shadow, model bit owned by GW expire + logs.
        {
            self.update_actively_constructing_model_conditions();
        }
    }
    // Explicit UI-only adapter for standalone callers; native roots lend both domains.
    pub(crate) fn update_construction_with_ui(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
        ui: &mut Option<&mut dyn crate::game_logic::ScriptUiEffects>,
    ) {
        let mut effects = crate::game_logic::ScriptPresentationEffects {
            terrain: None,
            ui: ui
                .as_mut()
                .map(|ui| &mut **ui as &mut dyn crate::game_logic::ScriptUiEffects),
            view: None,
        };
        self.update_construction_with_effects(object_ids, dt, &mut effects)
    }

    /// C++ DozerAIUpdate.cpp:318-348 — PICK_ACTION_POS re-issues the approach
    /// order until arrival. The host issues the walk once; an approach path
    /// queued the same frame its scaffold was created can install without ever
    /// integrating (velocity stays zero, percent frozen at 0). Called from
    /// `update()` AFTER the fixed step — the proven integration timing (paths
    /// queued between ticks install and walk). Throttled to once a second.
    pub(crate) fn reissue_dozer_approaches(&mut self) {
        if self.frame % 30 != 0 {
            return;
        }
        let raw_dozers: Vec<(
            ObjectId,
            Vec3,
            f32,
            Option<u32>,
            crate::game_logic::Team,
            Option<ObjectId>,
            Option<Vec3>,
        )> = self
            .objects
            .values()
            .filter(|obj| obj.is_alive() && obj.can_construct())
            .map(|obj| {
                (
                    obj.id,
                    obj.get_position(),
                    obj.selection_radius,
                    obj.owner_player_id,
                    obj.team,
                    obj.dozer_task_build_target.or(obj.target),
                    obj.dozer_dock_action,
                )
            })
            .collect();
        let dozer_info: Vec<(
            ObjectId,
            Vec3,
            f32,
            Option<u32>,
            Option<ObjectId>,
            Option<Vec3>,
        )> = raw_dozers
            .into_iter()
            .map(|(id, pos, radius, owner_id, team, target, dock)| {
                (
                    id,
                    pos,
                    radius,
                    self.player_owner_for_event(owner_id, team),
                    target,
                    dock,
                )
            })
            .collect();
        for (did, dpos, dradius, downer, dtarget, sdock) in dozer_info.iter() {
            let Some(sid) = dtarget else {
                continue;
            };
            let Some((under_construction, alive, bpos, bradius, site_owner_id, site_team)) =
                self.host_object(*sid).map(|site| {
                    (
                        site.status.under_construction,
                        site.is_alive(),
                        site.get_position(),
                        site.selection_radius,
                        site.owner_player_id,
                        site.team,
                    )
                })
            else {
                continue;
            };
            if !under_construction || !alive {
                continue;
            }
            if self.player_owner_for_event(site_owner_id, site_team) != *downer {
                continue;
            }
            let dock = crate::game_logic::host_repair::resolve_dozer_action_dock(
                *sdock, *dpos, bpos, bradius,
            );
            if crate::game_logic::host_repair::dozer_within_action_dock(*dpos, *dradius, dock) {
                continue;
            }
            let Some(d) = self.objects.get(did) else {
                continue;
            };
            // C++ DozerActionPickActionPosState (DozerAIUpdate.cpp:346-350):
            // the approach order is re-issued only when the dozer went IDLE
            // short of the dock (STATE_FAILURE back to PICK_ACTION_POS). A
            // pending path or an installed path being followed is a healthy
            // approach — the old speed threshold sat below dozer cruise speed
            // and re-pathed walking dozers every cycle.
            if d.waiting_for_path || d.status.moving || d.movement.velocity.length_squared() > 1e-3
            {
                continue;
            }
            let approach = self.find_good_build_or_repair_position(
                *dpos,
                bpos,
                bradius,
                false,
                None,
                Some(*did),
            );
            let _ = self.unit_command_path_with_state_ignoring(
                *did,
                approach,
                AIState::Moving,
                Some(*sid),
            );
        }
    }

    /// C++ `DozerAIUpdate::startBuildingSound` / Worker twin.
    /// Plays the building's PerUnitSound "UnderConstruction" on the site.
    pub fn start_building_sound(&mut self, site_id: ObjectId, template_name: &str, pos: Vec3) {
        let Some(event) = crate::game_logic::audio_dispatch_impl::resolve_per_unit_sound(
            template_name,
            "UnderConstruction",
        ) else {
            return;
        };
        if self
            .construction_audio_loops
            .get(&site_id)
            .map(String::as_str)
            == Some(event.as_str())
        {
            return;
        }
        if let Some(old) = self.construction_audio_loops.remove(&site_id) {
            self.queue_audio_event(
                crate::game_logic::AudioEventRequest::new(&old)
                    .with_object(site_id)
                    .with_position(pos)
                    .stopping(),
            );
        }
        self.construction_audio_loops.insert(site_id, event.clone());
        self.queue_audio_event(
            crate::game_logic::AudioEventRequest::new(&event)
                .with_object(site_id)
                .with_position(pos)
                .with_priority(80)
                .looping(),
        );
    }

    /// C++ `DozerAIUpdate::finishBuildingSound` / Worker twin.
    pub fn finish_building_sound(&mut self, site_id: ObjectId) {
        let Some(event) = self.construction_audio_loops.remove(&site_id) else {
            return;
        };
        let pos = self
            .objects
            .get(&site_id)
            .map(|o| o.get_position())
            .unwrap_or(Vec3::ZERO);
        self.queue_audio_event(
            crate::game_logic::AudioEventRequest::new(&event)
                .with_object(site_id)
                .with_position(pos)
                .stopping(),
        );
    }

    /// End transient playback before replacing this world's object roster.
    /// Stop requests retain the outgoing objects' positions; no playback state
    /// is serialized or inherited by recreated IDs.
    pub(in crate::game_logic) fn finish_all_building_sounds(&mut self) {
        let mut sites: Vec<_> = self.construction_audio_loops.keys().copied().collect();
        sites.sort();
        for site_id in sites {
            self.finish_building_sound(site_id);
        }
    }

    /// Wave 715: after GW construction writeback records ready structures, host
    /// applies completion side effects in the same coupled tick (not next frame).

    /// Wave 717: after GW special-power writeback records ready flips, host
    /// applies EVA superweapon-ready residual in the same coupled tick.

    fn publish_production_power_factors(&self) {
        let player_power_factor = self.compute_player_power_factors();
        for (&id, obj) in self.objects.iter() {
            if !obj.is_constructed() || !obj.is_alive() || obj.status.sold {
                continue;
            }
            if obj.building_data.is_none() {
                continue;
            }
            let pf = self
                .player_owner_for_host_object(obj)
                .and_then(|player_id| player_power_factor.get(&player_id).copied())
                .unwrap_or(1.0);
            crate::game_logic::host_production_progress_log::record_power_factor_only(id, pf);
        }
    }

    /// C++ ProductionUpdate::update (ProductionUpdate.cpp:671-682):
    /// scripts can disallow unit types mid-queue; cancel the unit head unless
    /// it is a dozer. Called every live production tick.
    pub(crate) fn cancel_script_disallowed_production_queue_heads(&mut self) {
        let mut cancelled: Vec<(
            ObjectId,
            Option<u32>,
            Team,
            crate::game_logic::buildings::ProductionItem,
        )> = Vec::new();
        let mut producers: Vec<ObjectId> = self.objects.keys().copied().collect();
        producers.sort_by_key(|id| id.0);
        for producer_id in producers {
            let Some(obj) = self.objects.get(&producer_id) else {
                continue;
            };
            if !obj.is_constructed() || !obj.is_alive() || obj.status.sold {
                continue;
            }
            let Some(building) = obj.building_data.as_ref() else {
                continue;
            };
            let Some(head) = building.production_queue.first() else {
                continue;
            };
            if head.is_upgrade() {
                continue;
            }
            let template_name = head.template_name.clone();
            let is_dozer = self
                .templates
                .get(&template_name)
                .is_some_and(|t| t.is_kind_of(KindOf::Dozer));
            if is_dozer {
                continue;
            }
            let is_structure = self
                .templates
                .get(&template_name)
                .is_some_and(|t| t.is_kind_of(KindOf::Structure));
            let owner_player_id = obj.owner_player_id;
            let owner_id = self.player_owner_for_host_object(obj);
            let team = obj.team;
            let allowed = match owner_id.and_then(|pid| self.get_player(pid)) {
                Some(player) => player.allowed_to_build(is_structure),
                None => true,
            };
            if allowed {
                continue;
            }
            if let Some(building) = self
                .objects
                .get_mut(&producer_id)
                .and_then(|o| o.building_data.as_mut())
            {
                if let Some(item) = building.cancel_production(0) {
                    if building.production_queue.is_empty() && building.exit_delay_remaining > 0.0 {
                        building.exit_delay_remaining = 0.0;
                        crate::game_logic::host_production_progress_log::record_exit_delay_only(
                            producer_id,
                            0.0,
                        );
                    }
                    cancelled.push((producer_id, owner_player_id, team, item));
                }
            }
        }
        for (producer_id, owner_player_id, team, item) in cancelled {
            self.refund_cancelled_production_item(owner_player_id, team, &item);
        }
    }

    pub(crate) fn update_production(&mut self, dt: f32) {
        self.update_production_with_effects(dt, &mut Default::default());
    }

    pub(crate) fn update_production_with_effects(
        &mut self,
        dt: f32,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // C++ ProductionUpdate.cpp:671 — re-check allowedToBuild on queue head.
        self.cancel_script_disallowed_production_queue_heads();
        // Wave 613: production complete collect + apply via host helpers.
        // Under PRODUCTION_AUTHORITY sole-tick, GameWorld advances queue progress
        // and writeback finishes heads; host try_complete + spawn runs after
        // shadow writeback same frame (Wave 714) so ready-log is not a frame late.
        // Wave 875: sole-tick early-return honesty — no host dual-advance.
        {}

        let (upgrade_completions, unit_completions) = self.host_collect_production_completions(dt);
        // Wave 595/608: host production complete/spawn apply residual via host helpers.
        self.apply_upgrade_production_completions_with_effects(upgrade_completions, ui);
        self.apply_unit_production_completions_with_effects(unit_completions, ui);
    }

    /// Wave 613: host production completion collection residual.
    ///
    /// Sole-tick path: GameWorld sole-ticks progress/exit delay; host
    /// `try_complete_production` only when writeback finished the head.
    /// Non-sole path: host still advances production via building.update_production.
    pub(crate) fn host_collect_production_completions(
        &mut self,
        dt: f32,
    ) -> (
        Vec<(Team, String, ObjectId)>,
        Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
    ) {
        // Wave 613: host production complete collect residual.

        // C++ parity: pre-compute per-player power factor so we don't borrow
        // self.players while self.objects is mutably borrowed.
        // Formula matches ThingTemplate::calcTimeToBuild():
        //   energy_ratio = produced / max(consumed, produced) clamped to [0,1]
        //   energy_short = (1.0 - ratio) * penalty_modifier
        //   rate = max(1.0 - energy_short, 0.5)
        //   if ratio < 1.0: rate = min(rate, 0.8)
        let player_power_factor = self.compute_player_power_factors();
        // Resolve the ownership scope before mutating producers.  Do not use
        // the historical first-player-for-team lookup: only a genuinely
        // ownerless object with one living team member gets a compatibility
        // owner through player_owner_for_host_object.
        let object_owner_player_ids: std::collections::HashMap<ObjectId, Option<u32>> = self
            .objects
            .values()
            .map(|obj| (obj.id, self.player_owner_for_host_object(obj)))
            .collect();

        use crate::game_logic::buildings::ProductionKind;
        // Unit completions: (team, template, spawn_pos, rally, producer_id)
        let mut unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)> = Vec::new();
        // Upgrade completions: (team, upgrade_name, producer_id)
        let mut upgrade_completions: Vec<(Team, String, ObjectId)> = Vec::new();

        for (&id, obj) in self.objects.iter_mut() {
            if !obj.is_constructed() || !obj.is_alive() {
                continue;
            }
            // C++ GameLogic.cpp:3677 / ProductionUpdate.cpp:86 — process mask is
            // DISABLED_HELD only. UNDERPOWERED does not intersect, so brownout
            // freezes ProductionUpdate instead of applying 50-80% power-factor.
            if obj.is_disabled() && !obj.status.disabled_held {
                continue;
            }
            if obj.status.sold {
                continue;
            }
            let mut start_door_cycle = false;
            let mut pop_closing_door = false;
            let door_spawn_phase = obj.production_door_spawn_phase();
            let exit_metadata = obj.thing().template.production_exit_metadata;
            let producer_template = obj.template_name.clone();
            let parking_place = obj.thing().template.parking_place.clone();
            let is_airfield = obj.is_kind_of(KindOf::FSAirfield);
            if let Some(building) = obj.building_data.as_mut() {
                let pf = object_owner_player_ids
                    .get(&id)
                    .copied()
                    .flatten()
                    .and_then(|player_id| player_power_factor.get(&player_id).copied())
                    .unwrap_or(1.0);
                // C++ ProductionUpdate checks an exit door only for completed
                // unit entries; upgrades complete independently of the door.
                let doors = crate::game_logic::host_production_buildable_command_residual::producer_num_door_animations(
                    &producer_template,
                );
                let airfield_holds_parking = is_airfield
                    && parking_place.is_some()
                    && building.production_queue.first().is_some_and(|head| {
                        !head.is_upgrade()
                            && !crate::game_logic::host_helicopter_slow_death::is_helicopter_slow_death_template(
                                &head.template_name,
                            )
                    });
                let parking_free = if airfield_holds_parking {
                    let cap = parking_place.and_then(|m| m.capacity()).unwrap_or(0);
                    Self::count_free_airfield_parking_slots(
                        self.airfield_parking_spaces.get(&id).map(|v| v.as_slice()),
                        cap,
                    )
                } else {
                    usize::MAX
                };

                // Under PRODUCTION_AUTHORITY, GameWorld ticks queue progress;
                // host only exits delay + completes when writeback already finished the head.
                let completed_prod = {
                    // `ProductionUpdate::update` increments the integer frame
                    // counter before it handles the completed unit's exit
                    // interface.  Splitting the residual here prevents a unit
                    // becoming ready on this terminal frame from bypassing a
                    // closed factory door.
                    building.tick_production_exit(exit_metadata.as_ref(), dt);
                    building.advance_production_progress(dt, pf);
                    let head_ready = building.production_head_complete_at_power(pf);
                    let head_is_unit = building
                        .production_queue
                        .first()
                        .is_some_and(|item| !item.is_upgrade());
                    let head_exit_available =
                        building.production_head_exit_available(exit_metadata.as_ref());
                    if head_ready
                        && head_is_unit
                        && head_exit_available
                        && airfield_holds_parking
                        && parking_free == 0
                    {
                        None
                    } else if head_ready
                        && head_is_unit
                        && head_exit_available
                        && !crate::game_logic::host_production_buildable_command_residual::production_door_allows_spawn(
                            doors,
                            door_spawn_phase,
                        )
                    {
                        if door_spawn_phase == 4 || door_spawn_phase == 3 {
                            // C++ ProductionUpdate.cpp:762-776: CLOSING pops to
                            // WAITING_OPEN so the next unit exits this frame.
                            pop_closing_door = true;
                            building.try_complete_production_at_power_with_exit_metadata(
                                pf,
                                exit_metadata.as_ref(),
                                if airfield_holds_parking {
                                    Some(parking_free.min(u32::MAX as usize) as u32)
                                } else {
                                    None
                                },
                            )
                        } else {
                            // C++ ProductionUpdate.cpp:746-754: start OPENING only
                            // when the reserved door is fully closed. Already-
                            // OPENING must not rewrite DoorOpeningTime.
                            start_door_cycle = door_spawn_phase == 0;
                            None
                        }
                    } else {
                        building.try_complete_production_at_power_with_exit_metadata(
                            pf,
                            exit_metadata.as_ref(),
                            if airfield_holds_parking {
                                Some(parking_free.min(u32::MAX as usize) as u32)
                            } else {
                                None
                            },
                        )
                    }
                };
                // GameWorld production residual: snapshot queue progress each tick
                // unless sole-tick owns progress (Wave 477) — then enqueue/complete logs
                // + writeback carry structure; GW advances progress/exit delay.
                let completed_this_tick = completed_prod.is_some();
                if (!building.production_queue.is_empty() || completed_this_tick) {
                    let items: Vec<crate::game_logic::host_production_progress_log::HostProductionQueueItem> =
                        building
                            .production_queue
                            .iter()
                            .take(16)
                            .map(|it| {
                                crate::game_logic::host_production_progress_log::HostProductionQueueItem {
                                    template_name: it.template_name.clone(),
                                    progress: it.progress,
                                    total_time: it.total_time,
                                    construction_frames: it.construction_frames,
                                    cost_supplies: it.cost.supplies,
                                    is_upgrade: it.is_upgrade(),
                                    quantity_total: it.quantity_total.max(1),
                                    quantity_produced: it.quantity_produced,
                                }
                            })
                            .collect();
                    crate::game_logic::host_production_progress_log::record_with_exit_state(
                        id,
                        items,
                        building.exit_delay_remaining,
                        building.production_exit_runtime_state(),
                        pf,
                    );
                } else {
                }
                // End the `BuildingData` field read before using producer
                // geometry below; the object owns both fields.
                let completion_rally = building.rally_point;
                if let Some(completed) = completed_prod {
                    let completed_template = completed.template_name;
                    match completed.kind {
                        ProductionKind::Upgrade => {
                            // C++ upgrades do not enter the unit exit batch loop.
                            upgrade_completions.push((obj.team, completed_template, id));
                        }
                        ProductionKind::Unit => {
                            // C++ ProductionUpdate loops every remaining unit in
                            // a completed QuantityModifier entry while its exit
                            // remains available.  The sole-tick batch is capped
                            // above by matching entity-first ready events.
                            for _ in 0..completed.quantity {
                                let rally = completion_rally;
                                // Spawn slightly offset from the building facing to reduce clumping.
                                let forward = obj.thing().get_direction_vector();
                                let base =
                                    obj.get_position() + forward * obj.selection_radius.max(10.0);
                                // Deterministic jitter based on template bytes (simple FNV-1a).
                                let mut hash: u32 = 0x811c9dc5;
                                for &b in completed_template.as_bytes() {
                                    hash ^= b as u32;
                                    hash = hash.wrapping_mul(0x01000193);
                                }
                                let angle = (hash as f32) * 0.001;
                                let radius = 3.0 + (hash as f32 % 5.0);
                                let jitter = Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
                                let mut spawn_pos = base + jitter;
                                if let Some(exit) = exit_metadata {
                                    spawn_pos = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                                        obj.get_position(),
                                        forward,
                                        (
                                            exit.unit_create_point[0],
                                            exit.unit_create_point[1],
                                            exit.unit_create_point[2],
                                        ),
                                    );
                                }
                                // Wave 735: under sole-tick, GameWorld ready-log pose/rally
                                // and template are authoritative for the completion spawn.
                                // Wave 736: queue each GW pre-spawned entity bind in
                                // the same order as its unit completion.
                                let completed_name = completed_template.clone();
                                {}
                                unit_completions.push((
                                    obj.team,
                                    completed_name,
                                    spawn_pos,
                                    rally,
                                    id,
                                ));
                            }
                        }
                    }
                }
            }
            if pop_closing_door {
                obj.pop_closing_production_door_to_waiting_open(self.frame);
            }
            if start_door_cycle {
                obj.start_production_door_cycle(self.frame);
            }
        }

        (upgrade_completions, unit_completions)
    }

    /// Wave 595: host upgrade production completion residual (still host-side under
    /// PRODUCTION_AUTHORITY; GameWorld sole-ticks queue progress only).
    /// Wave 608: via `host_apply_upgrade_production_completions`.
    pub(in super::super) fn apply_upgrade_production_completions(
        &mut self,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
    ) {
        self.apply_upgrade_production_completions_with_effects(
            upgrade_completions,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn apply_upgrade_production_completions_with_effects(
        &mut self,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Wave 608: thin wrapper — production complete apply via host helper.
        self.host_apply_upgrade_production_completions_with_effects(upgrade_completions, effects)
    }

    /// Wave 595: host upgrade production completion residual (still host-side under
    /// PRODUCTION_AUTHORITY; GameWorld sole-ticks queue progress only).
    pub(in super::super) fn host_apply_upgrade_production_completions(
        &mut self,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
    ) {
        self.host_apply_upgrade_production_completions_with_effects(
            upgrade_completions,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn host_apply_upgrade_production_completions_with_effects(
        &mut self,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.apply_upgrade_completions_on_owner_with_effects(None, upgrade_completions, effects);
    }

    /// The borrowed interpreter comes from the driving operation, never an
    /// active/global slot. Ordinary updates keep the owner's engine installed
    /// so upgrade effects that create objects can run their attached callbacks.
    pub(crate) fn host_apply_upgrade_production_completions_with_interpreter(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
    ) {
        self.host_apply_upgrade_production_completions_with_interpreter_with_effects(
            engine,
            upgrade_completions,
            &mut Default::default(),
        )
    }

    pub(crate) fn host_apply_upgrade_production_completions_with_interpreter_with_effects(
        &mut self,
        engine: &gamelogic::scripting::engine::ScriptEngine,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.apply_upgrade_completions_on_owner_with_effects(
            Some(engine),
            upgrade_completions,
            effects,
        );
    }

    fn apply_upgrade_completions_on_owner(
        &mut self,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
    ) {
        self.apply_upgrade_completions_on_owner_with_effects(
            interpreter,
            upgrade_completions,
            &mut Default::default(),
        )
    }

    fn apply_upgrade_completions_on_owner_with_effects(
        &mut self,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        for (team, upgrade_name, producer_id) in upgrade_completions {
            // Production completion carries its producer ObjectId, which in
            // turn carries the authoritative PlayerId.  Resolve it before the
            // mutable producer borrow; never credit the first same-faction
            // player just because this older event also contains a Team.
            let producer_owner_player_id = self
                .objects
                .get(&producer_id)
                .and_then(|producer| self.player_owner_for_host_object(producer))
                .filter(|player_id| {
                    self.players
                        .get(player_id)
                        .is_some_and(|player| player.team == team)
                });
            // `ProductionUpdate::update` handles PRODUCTION_UPGRADE directly:
            // unlike a unit it does not reserve an exit, open a factory door,
            // or set MODELCONDITION_CONSTRUCTION_COMPLETE.  Keep the producer
            // ID below solely for the authoritative research completion event.
            // Wave 483: refresh GW producer queue after host pop (sole-tick skips
            // per-frame progress log; Complete path snapshots host queue).
            self.production_events
                .record_complete(producer_id, upgrade_name.clone(), ObjectId(0));
            // Unlock via player queue drain + host apply path.
            if let Some(pid) = producer_owner_player_id {
                let already = self
                    .players
                    .get(&pid)
                    .map(|p| p.has_unlocked_upgrade(&upgrade_name))
                    .unwrap_or(false);
                // CPP ProductionUpdate.cpp879-925: edge precedes addUpgrade /
                // giveUpgrade, including an already-recorded completion.
                interpreter
                    .or(self.owned_script_engine.as_ref())
                    .expect("research completion requires its driving interpreter")
                    .notify_of_completed_upgrade(pid as usize, &upgrade_name, producer_id.0);
                let upgrade_type = self.upgrade_type(&upgrade_name);
                let definition = self.upgrade_template(&upgrade_name);
                if let Some(player) = self.players.get_mut(&pid) {
                    player.complete_researched_upgrade_with_definition(
                        &upgrade_name,
                        upgrade_type,
                        definition.as_deref(),
                    );
                    // record_upgrade_production_complete: leftover AcademyStats::recordUpgrade
                    // (FALSE) + ScoreKeeper::addMoneySpent (ProductionUpdate.cpp:874-879 / 931).
                }
                if !already {
                    self.apply_host_upgrade_complete_with_effects(
                        team,
                        pid,
                        &upgrade_name,
                        effects,
                    );
                }
            }
        }
    }

    /// Wave 595: host unit production completion residual — spawn, door, exit delay,
    /// rally path. GameWorld sole-ticks progress; host still completes/spawns.
    /// Wave 608: via `host_apply_unit_production_completions`.
    pub(in super::super) fn apply_unit_production_completions(
        &mut self,
        unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
    ) {
        self.apply_unit_production_completions_with_effects(
            unit_completions,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn apply_unit_production_completions_with_effects(
        &mut self,
        unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.host_apply_unit_production_completions_with_effects(unit_completions, ui)
    }

    /// Wave 615: host production unit spawn residual.
    ///
    /// Still host ObjectId authority (`create_object` + spawn log). GameWorld
    /// receives the unit via host_spawn_log / production Complete channel after
    /// sole-tick readiness (Waves 614/608). Wave 679: successful IDs enter
    /// `host_production_spawn_ready_log` before door/notify/exit residual.
    /// Not full GW spawn-ID authority.

    /// Wave 740: rebuild-hole worker/structure spawn with optional GW entity bind.
    /// Under construction sole-tick, prefers free GW entity raw as ObjectId and
    /// binds without a second Spawn.
    /// Wave 741: missing GW entity raw under construction sole-tick is fail-closed
    /// (default). Incomplete harnesses may set
    /// GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND=1.
    /// playable_claim stays false.
    pub(in super::super) fn host_spawn_rebuild_object(
        &mut self,
        template: &str,
        team: Team,
        owner_player_id: Option<u32>,
        spawn_pos: Vec3,
    ) -> Option<ObjectId> {
        self.host_spawn_rebuild_object_with_effects(
            template,
            team,
            owner_player_id,
            spawn_pos,
            &mut Default::default(),
        )
    }

    pub(in super::super) fn host_spawn_rebuild_object_with_effects(
        &mut self,
        template: &str,
        team: Team,
        owner_player_id: Option<u32>,
        spawn_pos: Vec3,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.create_object_for_owner_or_team_with_effects(
            template,
            team,
            owner_player_id,
            spawn_pos,
            None,
            effects,
        )
    }

    pub(in super::super) fn host_spawn_production_unit(
        &mut self,
        template: &str,
        team: Team,
        spawn_pos: Vec3,
    ) -> Option<ObjectId> {
        self.host_spawn_production_unit_with_owner(template, team, None, spawn_pos)
    }

    /// Complete a production queue for the exact player that owns its
    /// producer.  `team` remains in the lower-level spawning path for
    /// template/faction behavior, but must not choose between same-faction
    /// skirmish slots.
    pub(in super::super) fn host_spawn_production_unit_for_player(
        &mut self,
        template: &str,
        owner_player_id: u32,
        spawn_pos: Vec3,
    ) -> Option<ObjectId> {
        let team = self.players.get(&owner_player_id)?.team;
        self.host_spawn_production_unit_with_owner(template, team, Some(owner_player_id), spawn_pos)
    }

    fn host_spawn_production_unit_with_owner(
        &mut self,
        template: &str,
        team: Team,
        owner_player_id: Option<u32>,
        spawn_pos: Vec3,
    ) -> Option<ObjectId> {
        self.host_spawn_production_unit_with_effects(
            template,
            team,
            owner_player_id,
            spawn_pos,
            &mut Default::default(),
        )
    }

    pub(crate) fn host_spawn_production_unit_with_effects(
        &mut self,
        template: &str,
        team: Team,
        owner_player_id: Option<u32>,
        spawn_pos: Vec3,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        // C++ ThingFactory::newObject (ThingFactory.cpp:286-292) picks a
        // BuildVariations chassis before create.
        let template_owned = resolve_production_build_variation(template);
        let template = template_owned.as_str();
        // Wave 615: host production spawn residual.
        // Wave 736: under sole-tick, bind host ObjectId to GW pre-spawned entity
        // (entity-first).
        // Wave 737: when the GW entity raw id is free on the host, prefer it as the
        // production ObjectId so host ID space tracks GW entity-first spawns.
        // Wave 738: under sole-tick, spawn without a GW entity bind is fail-closed
        // (default). Incomplete harnesses may set
        // GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND=1.
        // Collision on preferred id still falls back to allocate_object_id *with* bind.
        // playable_claim stays false.
        // Wave 761: entity-first ObjectId bind under production sole-tick OR
        // coupled shadow (dual path still prefers GW pre-spawned entity raw id).
        {}
        self.create_production_object_for_owner_or_team_with_effects(
            template,
            team,
            owner_player_id,
            spawn_pos,
            ui,
        )
    }

    /// Wave 595: host unit production completion residual — spawn, door, exit delay,
    /// rally path. GameWorld sole-ticks progress; host still completes/spawns.
    pub(in super::super) fn host_apply_unit_production_completions(
        &mut self,
        unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
    ) {
        self.host_apply_unit_production_completions_with_effects(
            unit_completions,
            &mut Default::default(),
        );
    }

    pub(crate) fn host_apply_unit_production_completions_with_effects(
        &mut self,
        unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        // Wave 608: host production complete/spawn apply residual.
        // Wave 595: host unit production completion residual.
        for (team, template, spawn_pos, rally, producer_id) in unit_completions {
            // ProductionUpdate.cpp creates the unit, then immediately links it
            // to the factory before handing it to QueueProductionExitUpdate.
            // Preserve that relationship and the factory's exit-facing rather
            // than leaving a completed unit indistinguishable from a generic
            // script spawn.  The link is used by real airfield parking and by
            // presentation consumers that retain the producer identity.
            let Some(producer) = self.objects.get(&producer_id) else {
                // A completion is only valid while its producing building is
                // live.  Do not create a unit with a dangling producer id.
                log::warn!(
                    "Ignoring production completion for missing producer {:?} ({template})",
                    producer_id
                );
                continue;
            };
            let producer_orientation = producer.get_orientation();
            // Preserve a concrete player owner, and only use the compatibility
            // unique-team owner for genuinely ownerless legacy producers.
            let owner_player_id = self.player_owner_for_host_object(producer);
            // Wave 615: production unit spawn via host helper (still host ID authority).
            let new_id = match self.apply_production_authority_op_with_effects(
                ProductionAuthorityOp::SpawnUnit {
                    template: template.clone(),
                    team,
                    owner_player_id,
                    spawn_pos,
                },
                ui,
            ) {
                ProductionAuthorityResult::Spawned(id) => id,
                _ => None,
            };
            if let Some(new_id) = new_id {
                if let Some(unit) = self.host_object_mut(new_id) {
                    unit.producer_id = Some(producer_id);
                    unit.set_orientation(producer_orientation);
                }

                // C++ `ParkingPlaceBehavior::exitObjectViaDoor` reserves an
                // authored m_spaces slot before a non-helipad aircraft leaves
                // its airfield.  Do not model that relation through a generic
                // building garrison: producer_id is retained only if the
                // exact producer controller could reserve real ParkingPlace
                // metadata for this completed aircraft.
                let airfield_output = self
                    .host_object(producer_id)
                    .is_some_and(|producer| producer.is_kind_of(KindOf::FSAirfield))
                    && self.host_object(new_id).is_some_and(|unit| {
                        unit.is_kind_of(KindOf::Aircraft)
                            || unit.object_type == ObjectType::Aircraft
                    });
                if airfield_output {
                    if self.reserve_produced_aircraft_parking_space(producer_id, new_id) {
                        let _ = self.place_produced_jet_at_parking_pose_with_effects(
                            producer_id,
                            new_id,
                            ui,
                        );
                    } else {
                        log::warn!(
                            "Production aircraft {:?} from airfield {:?} held: no ParkingPlace stall",
                            new_id,
                            producer_id
                        );
                        self.destroy_object_with_effects(new_id, ui);
                        continue;
                    }
                }

                self.production_events
                    .record_complete(producer_id, template.clone(), new_id);
                // Wave 679: production spawn ObjectId ready residual —
                // host door/notify/exit/path apply drains the ready log.
                self.record_production_spawn_completion(
                    new_id,
                    producer_id,
                    template,
                    [spawn_pos.x, spawn_pos.y, spawn_pos.z],
                    rally.map(|r| [r.x, r.y, r.z]),
                );
                let _ = self.apply_production_authority_op_with_effects(
                    ProductionAuthorityOp::ApplySpawnReadyCompletions,
                    ui,
                );
            }
        }
    }

    /// C++ `Pathfinder::getRadiusAndCenter` (AIPathfind.cpp:5026-5054).
    fn factory_exit_radius_and_center(unit_radius: f32) -> (i32, bool) {
        let cell = crate::game_logic::host_ai_path_combat_residual_wave105::PATHFIND_CELL_SIZE_F;
        let mut diameter = 2.0 * unit_radius;
        if diameter > cell && diameter < 2.0 * cell {
            diameter = 2.0 * cell;
        }
        let mut radius = (diameter / cell + 0.3).floor() as i32;
        let mut center_in_cell = false;
        if radius == 0 {
            radius = 1;
        }
        if (radius & 1) != 0 {
            center_in_cell = true;
        }
        radius /= 2;
        if radius > 2 {
            radius = 2;
            center_in_cell = true;
        }
        (radius, center_in_cell)
    }

    /// C++ `Pathfinder::adjustCoordToCell` on the host XZ ground plane.
    fn factory_exit_coord_to_cell(
        &self,
        cell: crate::game_logic::pathfinding::GridPos,
        center_in_cell: bool,
        y: f32,
    ) -> Vec3 {
        let grid = &self.pathfinding_system.grid;
        let size = grid.grid_size();
        let origin = grid.origin();
        let bias = if center_in_cell { 0.5 } else { 0.05 };
        let mut pos = Vec3::new(
            origin.x + (cell.x as f32 + bias) * size,
            y,
            origin.z + (cell.y as f32 + bias) * size,
        );
        if let Some(h) = self.terrain_height_at(pos) {
            pos.y = h;
        }
        pos
    }

    /// C++ `Pathfinder::snapPosition` (AIPathfind.cpp:5082-5095).
    fn snap_factory_exit_position(&self, pos: Vec3, unit_radius: f32) -> Vec3 {
        let cell_size = self.pathfinding_system.grid.grid_size();
        let (_, center_in_cell) = Self::factory_exit_radius_and_center(unit_radius);
        let mut adjust = pos;
        if !center_in_cell {
            adjust.x += cell_size * 0.5;
            adjust.z += cell_size * 0.5;
        }
        let cell = self.pathfinding_system.grid.world_to_grid(adjust);
        self.factory_exit_coord_to_cell(cell, center_in_cell, pos.y)
    }

    /// C++ `Pathfinder::adjustDestination` spiral, then `adjustCoordToCell`.
    fn adjust_factory_exit_destination(
        &self,
        dest: Vec3,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unit_radius: f32,
    ) -> Option<Vec3> {
        let cell_size = self.pathfinding_system.grid.grid_size();
        let (_, center_in_cell) = Self::factory_exit_radius_and_center(unit_radius);
        let mut shifted = dest;
        if !center_in_cell {
            shifted.x += cell_size * 0.5;
            shifted.z += cell_size * 0.5;
        }
        let cell = self.pathfinding_system.grid.world_to_grid(shifted);
        let layer = ({
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
            self.pathfinding_system
                .grid
                .layer_for_destination(dest, &ground_source)
        });
        let adj = ({
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
            self.pathfinding_system.grid.adjust_destination_on_layer(
                cell,
                surfaces,
                is_crusher,
                400,
                seeker_player,
                crusher_level,
                layer,
                &ground_source,
            )
        })?;
        Some(self.factory_exit_coord_to_cell(adj, center_in_cell, dest.y))
    }

    /// AIPlayer::onUnitProduced follows its current exit goal to team home
    /// with no ignored factory (C++ passes NULL for ignoreObstacle).
    pub(crate) fn follow_ai_production_home_path(&mut self, unit: ObjectId, path: &[Vec3]) {
        self.follow_exit_production_path(unit, path, None);
    }

    /// C++ `AIUpdateInterface::aiFollowExitProductionPath`.
    /// Ignore the producer, walk the authored points, and path through units.
    fn follow_exit_production_path(
        &mut self,
        unit_id: ObjectId,
        path: &[Vec3],
        ignore_producer: Option<ObjectId>,
    ) {
        if path.is_empty() {
            return;
        }
        // C++ AI_FOLLOW_EXITPRODUCTION_PATH clears adjust before the nested
        // move, so the first exit search does not spiral.
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.adjust_destinations = false;
            unit.can_path_through_units = true;
        }
        self.path_approach_with_state_ignoring(unit_id, path[0], AIState::Moving, ignore_producer);
        for &wp in path.iter().skip(1) {
            // The A*-installed end and the authored snapped point can differ
            // only in ground Y; C++ path cells live on the 2D ground grid
            // (AIUpdateInterface::aiFollowExitProductionPath), so compare X/Z.
            let already_at = self.objects.get(&unit_id).is_some_and(|unit| {
                let same_cell = |a: Vec3, b: Vec3| {
                    let dx = a.x - b.x;
                    let dz = a.z - b.z;
                    dx * dx + dz * dz < 0.1 * 0.1
                };
                unit.movement
                    .path
                    .last()
                    .is_some_and(|last| same_cell(*last, wp))
                    || unit
                        .movement
                        .target_position
                        .is_some_and(|dest| same_cell(dest, wp))
            });
            if already_at {
                // C++ Queue pushes the snapped natural twice so Red Guards
                // do not stack. A* append would collapse the duplicate; keep
                // the installed end's exact pose so both entries repeat it.
                if let Some(unit) = self.objects.get_mut(&unit_id) {
                    let end = unit
                        .movement
                        .path
                        .last()
                        .copied()
                        .or(unit.movement.target_position)
                        .unwrap_or(wp);
                    unit.movement.path.push(end);
                    if unit.movement.target_position.is_some_and(|dest| {
                        let dx = dest.x - wp.x;
                        let dz = dest.z - wp.z;
                        dx * dx + dz * dz < 0.1 * 0.1
                    }) {
                        unit.movement.target_position = Some(end);
                    }
                }
            } else {
                let _ = self.append_unit_waypoint(unit_id, wp);
            }
        }
        // After the path is installed — move_object_with_pathfinding
        // clears this on new orders (C++ FollowPath onExit).
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            if let Some(node) = unit
                .movement
                .path
                .get(unit.movement.current_path_index)
                .copied()
            {
                unit.movement.target_position = Some(node);
                {}
            }
            unit.can_path_through_units = true;
            unit.ignored_obstacle_id = ignore_producer;
            unit.is_attack_path = false;
            unit.last_command_source =
                crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI;
            // C++ AIFollowPathState::onEnter writes m_adjustFinal after
            // requestPath has already copied the pre-search false.
            unit.adjust_destinations =
                unit.movement.current_path_index + 1 >= unit.movement.path.len();
            unit.refresh_follow_path_extra_distance();
        }
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            if !unit.movement.path.is_empty() {
                unit.set_status_moving(true);
            }
        }
        // computeQuickPath ok → leftover moveAlliesAwayFromDestination.
        self.move_allies_away_from_destination(unit_id, path[0]);
        // privateFollowPath setGoalPosition is the last node (AIUpdate.cpp:3373-3375).
        self.register_ground_path_goal(unit_id, path[path.len() - 1]);
    }

    /// Wave 679: drain production-spawn ready log and apply host presentation residual
    /// (notify/door/exit/path) for the newly allocated host ObjectId.
    /// Still host ObjectId authority — not full GameWorld spawn-ID ownership.
    /// Preserve accepted factory callback order on this receiving match.
    pub(crate) fn record_production_spawn_completion(
        &mut self,
        unit: ObjectId,
        producer: ObjectId,
        template: String,
        spawn_pos: [f32; 3],
        rally: Option<[f32; 3]>,
    ) {
        self.production_spawn_completions.push(
            crate::game_logic::host_production_spawn_ready_log::HostProductionSpawnReadyEvent {
                unit,
                producer,
                template,
                spawn_pos,
                rally,
            },
        );
    }

    pub fn host_apply_production_spawn_ready_completions(&mut self) -> usize {
        self.host_apply_production_spawn_ready_completions_with_effects(&mut Default::default())
    }

    pub(crate) fn host_apply_production_spawn_ready_completions_with_effects(
        &mut self,
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> usize {
        let events = std::mem::take(&mut self.production_spawn_completions);
        self.apply_spawn_ready_events(
            events,
            SpawnReadyCallbacks::NotifyAfterExit,
            |world, id, ui| world.apply_create_modules_on_build_complete_with_effects(id, None, ui),
            ui,
        )
    }

    pub(crate) fn host_apply_production_spawn_ready_completions_with_build_complete(
        &mut self,
        mut on_build_complete: impl FnMut(&mut Self, ObjectId),
    ) -> usize {
        // Wave 679: paid ProductionUpdate deliveries retain the drain boundary.
        let events = std::mem::take(&mut self.production_spawn_completions);
        self.apply_spawn_ready_events(
            events,
            SpawnReadyCallbacks::NotifyAfterExit,
            |world, id, _| on_build_complete(world, id),
            &mut Default::default(),
        )
    }

    /// SpawnBehavior already notified Player/AI at allocation. Admit only its
    /// owned exit event, without draining another paid factory's pending output.
    pub(crate) fn host_apply_supply_spawn_ready_completion(
        &mut self,
        event: crate::game_logic::host_production_spawn_ready_log::HostProductionSpawnReadyEvent,
    ) -> usize {
        self.host_apply_supply_spawn_ready_completion_with_effects(event, &mut Default::default())
    }
    pub(crate) fn host_apply_supply_spawn_ready_completion_with_effects(
        &mut self,
        event: crate::game_logic::host_production_spawn_ready_log::HostProductionSpawnReadyEvent,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> usize {
        self.apply_spawn_ready_events(
            std::iter::once(event),
            SpawnReadyCallbacks::AlreadyNotified,
            |_, _, _| {},
            effects,
        )
    }

    fn apply_spawn_ready_events(
        &mut self,
        events: impl IntoIterator<Item = crate::game_logic::host_production_spawn_ready_log::HostProductionSpawnReadyEvent>,
        callbacks: SpawnReadyCallbacks,
        mut on_build_complete: impl FnMut(
            &mut Self,
            ObjectId,
            &mut crate::game_logic::ScriptPresentationEffects<'_>,
        ),
        ui: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> usize {
        let pose_frame = self.frame;
        let mut n = 0usize;
        let mut voice_create_played: std::collections::HashSet<ObjectId> =
            std::collections::HashSet::new();
        for ev in events {
            let new_id = ev.unit;
            let producer_id = ev.producer;
            let template = ev.template;
            let producer_exit_metadata = self
                .objects
                .get(&producer_id)
                .and_then(|producer| producer.thing().template.production_exit_metadata);
            let mut spawn_pos = Vec3::new(ev.spawn_pos[0], ev.spawn_pos[1], ev.spawn_pos[2]);
            let rally = ev.rally.map(|r| Vec3::new(r[0], r[1], r[2]));
            // Wave 739: under production sole-tick, GameWorld ready-log pose is
            // authoritative — do not reposition the unit here (host
            // create_object already placed at GW exit pose). Non-sole path
            // places at the exact transformed UnitCreatePoint (C++ Queue/Default
            // exitObjectViaDoor). Anti-stack is the doubled natural-rally
            // waypoint, not a spawn offset.

            // C++ ProductionUpdate door + CONSTRUCTION_COMPLETE residual on producer.
            // Completing a unit opens only the reserved ExitDoorType (DOOR_1..4).
            let reserved_door = self
                .objects
                .get(&new_id)
                .and_then(|unit| unit.airfield_parking_space_index)
                .and_then(|slot| usize::try_from(slot).ok())
                .map(|slot| slot % 4);
            if let Some(prod) = self.objects.get_mut(&producer_id) {
                let now = self.frame.max(1);
                prod.set_construction_complete_condition_at(now);
                let door_count = crate::game_logic::host_production_buildable_command_residual::producer_num_door_animations(
                    &prod.template_name,
                );
                let door = reserved_door
                    .unwrap_or(prod.production_door_active_index as usize)
                    .min(3);
                if door_count > 0 && prod.production_door_phases[door] == 2 {
                    // C++ ProductionUpdate's `m_doorWaitOpenFrame = now` keeps
                    // an already-open reserved exit available for every member
                    // of the terminal QuantityModifier batch.  Do not restart
                    // the door-opening animation after a successful exit.
                    let wait = crate::game_logic::host_production_buildable_command_residual::producer_door_phase_duration(
                        &prod.template_name,
                        2,
                    );
                    prod.production_door_phase_end_frames[door] = now.saturating_add(wait);
                    if door == 0 {
                        prod.production_door_phase_end_frame = now.saturating_add(wait);
                    }
                } else if door_count > 0 && prod.production_door_phases[door] == 0 {
                    prod.start_production_door_cycle_at(self.frame, door);
                    self.production_door_cycles = self.production_door_cycles.saturating_add(1);
                }
                // C++ QueueProductionExitUpdate mutates its own per-Object
                // delay/burst state after each successful exit.  This is not
                // a China/Barracks-name delay table.
                if let Some(building) = prod.building_data.as_mut() {
                    building.record_successful_production_exit(producer_exit_metadata.as_ref());
                    // Under sole-tick, progress is shadow-owned.  Publish the
                    // exact state transition so the next GameWorld update
                    // decrements the same integer C++ counter.
                    {}
                }
            }
            // SCIENCE_StealthFighter residual: record gated production spawn.
            if crate::game_logic::host_stealth_fighter::requires_stealth_fighter_science(&template)
            {
                self.stealth_fighter_science.record_production_spawn();
            }
            let parked_jet = self
                .objects
                .get(&new_id)
                .is_some_and(|unit| unit.is_parked_at_airfield());

            // Wave 739: sole-tick keeps create_object/GW exit pose; non-sole
            // applies factory exit pose residual at UnitCreatePoint (no spawn jitter).
            if !parked_jet {
                // C++ Default/SupplyCenter snap spawn Z to terrain. Queue keeps
                // transformed Z only when AllowAirborneCreation is set.
                // creationInAir is pre-snap Y != ground (QueueProductionExitUpdate.cpp:69-73).
                let queue_exit = producer_exit_metadata.is_some_and(|exit| exit.is_queue());
                let ground_y = self.terrain_height_at(spawn_pos);
                let creation_in_air = queue_exit && ground_y.is_some_and(|h| spawn_pos.y != h);
                let snap_spawn_z = !producer_exit_metadata
                    .is_some_and(|exit| exit.is_queue() && exit.allow_airborne_creation);
                if snap_spawn_z {
                    if let Some(h) = ground_y {
                        spawn_pos.y = h;
                    }
                }
                if let Some(unit) = self.objects.get_mut(&new_id) {
                    {
                        unit.set_position_with_effects(spawn_pos, pose_frame, ui);
                    }
                }
                // C++ QueueProductionExitUpdate.cpp:91-105: kick after snap when
                // creationInAir (not AllowAirborneCreation). startingForce =
                // producer velocity * newborn mass, then pitchRate = COM * 0.04.
                if creation_in_air {
                    let producer_vel = self
                        .objects
                        .get(&producer_id)
                        .map(|producer| producer.movement.velocity);
                    if let (Some(vel), Some(unit)) = (producer_vel, self.objects.get_mut(&new_id)) {
                        let mass = unit.physics_get_mass();
                        unit.apply_motive_force(vel * mass);
                        const STARTING_PITCH_COEFF: f32 = 0.04;
                        unit.shock_pitch_rate = unit.center_of_mass_offset * STARTING_PITCH_COEFF;
                    }
                }
            }
            // C++ ExitInterface::exitObjectViaDoor → aiFollowExitProductionPath.
            // Queue snaps the natural rally and doubles it when no custom rally
            // is set so Red Guards do not stack. Custom rallies go through
            // adjustDestination. The producer is ignoreObstacle.
            if !parked_jet {
                let unit_radius = self
                    .objects
                    .get(&new_id)
                    .map(|unit| unit.selection_radius)
                    .unwrap_or(8.0);
                let (surfaces, is_crusher, seeker_player, crusher_level, ground_move) = self
                    .objects
                    .get(&new_id)
                    .map(|unit| {
                        let surfaces = if unit.locomotor_surfaces != 0 {
                            unit.locomotor_surfaces
                        } else {
                            crate::game_logic::LOCO_SURFACE_GROUND
                        };
                        (
                            surfaces,
                            unit.crusher_level > 0,
                            unit.owner_player_id,
                            unit.crusher_level,
                            (surfaces & crate::game_logic::LOCO_SURFACE_GROUND) != 0,
                        )
                    })
                    .unwrap_or((crate::game_logic::LOCO_SURFACE_GROUND, false, None, 0, true));
                let mut natural = if let Some(prod) = self.objects.get(&producer_id) {
                    let f = prod.thing().get_direction_vector();
                    if let Some(exit) = producer_exit_metadata {
                        let point = if exit.is_supply_center() {
                            exit.natural_rally_point
                        } else {
                            exit.natural_rally_point_with_path_offset(
                                crate::game_logic::host_ai_path_combat_residual_wave105::PATHFIND_CELL_SIZE_F,
                            )
                        };
                        crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                            prod.get_position(),
                            f,
                            (point[0], point[1], point[2]),
                        )
                    } else {
                        prod.get_position() + f * prod.selection_radius.max(10.0)
                    }
                } else {
                    spawn_pos
                };
                let is_queue = producer_exit_metadata.is_some_and(|exit| exit.is_queue());
                if is_queue {
                    natural = self.snap_factory_exit_position(natural, unit_radius);
                }
                let mut exit_path = vec![natural];
                if let Some(rally_point) = rally {
                    if producer_exit_metadata.is_some_and(|exit| exit.is_supply_center()) {
                        exit_path.push(rally_point);
                    } else if ground_move {
                        if let Some(adj) = self.adjust_factory_exit_destination(
                            rally_point,
                            surfaces,
                            is_crusher,
                            seeker_player,
                            crusher_level,
                            unit_radius,
                        ) {
                            exit_path.push(adj);
                        }
                    }
                } else if is_queue {
                    exit_path.push(natural);
                }
                self.follow_exit_production_path(new_id, &exit_path, Some(producer_id));
            }

            // SupplyCenterProductionExitUpdate performs the ordinary exit
            // route first, then forces only SupplyTruckAI-capable outputs into
            // their Wanting state, then GrantTemporaryStealth (cpp:114-126).
            // This shared completion path covers both paid ProductionUpdate
            // output and the one-shot SpawnBehavior.
            if producer_exit_metadata.is_some_and(|exit| exit.is_supply_center()) {
                let _ = self.force_supply_center_collector_wanting(new_id, producer_id);
                self.grant_supply_center_exit_temporary_stealth(producer_id, new_id);
            }
            // ProductionUpdate.cpp:798-825 exits before notifying Player/AI
            // and Create modules. AI home/dock orders must not be overwritten
            // by a factory exit installed after onBuildComplete.
            // C++ VoiceCreated + UnitReady residual.
            if matches!(callbacks, SpawnReadyCallbacks::NotifyAfterExit) {
                self.notify_unit_production_complete_with_build_complete(
                    new_id,
                    producer_id,
                    &template,
                    |world, id| on_build_complete(world, id, ui),
                );
            } else {
                // SpawnBehavior notified AI before exit, and its ordinary
                // AtCreation path already completed the host Create callbacks.
                self.notify_unit_production_presentation(new_id, &template);
                self.unit_ready_events = self.unit_ready_events.saturating_add(1);
            }
            // C++ ProductionUpdate.cpp:827-832: first unit of a QuantityModifier
            // batch also plays per-unit VoiceCreate (before oneProductionSuccessful).
            if voice_create_played.insert(producer_id) {
                if let Some(unit) = self.objects.get(&new_id) {
                    if let Some(event) =
                        crate::game_logic::audio_dispatch_impl::resolve_unit_voice_event(
                            &unit.template_name,
                            crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Create,
                        )
                    {
                        let pos = unit.get_position();
                        self.queue_audio_event(
                            crate::game_logic::game_logic::AudioEventRequest::new(&event)
                                .with_object(new_id)
                                .with_position(pos)
                                .with_priority(141),
                        );
                    }
                }
            }
            n = n.saturating_add(1);
        }
        n
    }

    /// Spawns each active skirmish/SP player's starting construction unit near their
    /// base if they do not already own a matching mobile builder.
    pub(crate) fn spawn_skirmish_starting_units(&mut self) {
        self.spawn_skirmish_starting_units_with_effects(&mut Default::default())
    }

    pub(crate) fn spawn_skirmish_starting_units_with_effects(
        &mut self,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_faction_skirmish_residual::{
            find_player_template_by_side, find_player_template_residual,
        };

        let mut player_ids: Vec<u32> = self.players.keys().copied().collect();
        player_ids.sort_unstable();

        for pid in player_ids {
            let Some(player) = self.players.get(&pid).cloned() else {
                continue;
            };
            if !player.is_alive || player.team == Team::Neutral {
                continue;
            }

            // C++ GameLogic uses the Player's exact PlayerTemplate for both
            // StartingBuilding and StartingUnit0..9.  A selected General must
            // never degrade to the base-side residual table merely because a
            // late Common-store lookup failed.
            let selected_template = self.player_template_identity(pid).cloned();
            let (starting_building, starting_units, exact_player_template) = if selected_template
                .is_some()
            {
                let Some(template) = self.resolved_player_template(pid) else {
                    log::error!(
                        "Rejecting selected PlayerTemplate starter spawn for player {}: identity no longer resolves",
                        pid
                    );
                    continue;
                };
                (
                    template.get_starting_building().to_string(),
                    (0..game_engine::common::rts::player_template::MAX_MP_STARTING_UNITS)
                        .map(|index| template.get_starting_unit(index as i32).to_string())
                        .collect::<Vec<_>>(),
                    true,
                )
            } else {
                let side = match player.team {
                    Team::USA => "America",
                    Team::China => "China",
                    Team::GLA => "GLA",
                    Team::Neutral => continue,
                };
                let residual = find_player_template_by_side(side)
                    .or_else(|| find_player_template_residual("FactionAmerica"));
                let Some(residual) = residual else {
                    log::warn!(
                        "Skirmish starting unit residual: no player template for side={} player={}",
                        side,
                        pid
                    );
                    continue;
                };
                (
                    residual.starting_building.to_string(),
                    residual
                        .starting_units
                        .iter()
                        .map(|unit| (*unit).to_string())
                        .collect::<Vec<_>>(),
                    false,
                )
            };

            // --- Starting building (C++ placeStartingStructures) ---
            let mut base = self.player_base_position(pid);
            if base.is_none() {
                // Wave 831/832: place at Player_N_Start when map has no faction army.
                let building = starting_building.as_str();
                let mut pos_opt: Option<Vec3> = None;
                if !building.is_empty() {
                    // Prefer already-parsed map settings so load_map does not
                    // RefPack-decompress the chunky map again per player.
                    // C++ TerrainLogic::loadMap (TerrainLogic.cpp:1248-1262)
                    // opens the .map once via CachedFileInputStream.
                    let starts = self.cached_player_start_waypoints().or_else(|| {
                        super::super::script_loader::parse_player_start_waypoints(&self.map_name)
                            .ok()
                    });
                    if let Some(starts) = starts {
                        let want_idx = if player.start_position >= 0 {
                            player.start_position as u32
                        } else {
                            pid
                        };
                        if let Some((_, wp, _rally)) =
                            starts.iter().find(|(idx, _, _)| *idx == want_idx)
                        {
                            let mut pos = Vec3::new(wp.x, wp.z, wp.y);
                            if let Some(h) = self.terrain_height_at(Vec3::new(pos.x, 0.0, pos.z)) {
                                pos.y = h;
                            }
                            pos_opt = Some(pos);
                        } else if let Some((_, wp, _)) = starts.first() {
                            let mut pos = Vec3::new(wp.x, wp.z, wp.y);
                            if let Some(h) = self.terrain_height_at(Vec3::new(pos.x, 0.0, pos.z)) {
                                pos.y = h;
                            }
                            pos_opt = Some(pos);
                        }
                    }
                }
                let allow_seed_building = pos_opt.is_some()
                    || std::env::var_os("GENERALS_RUNTIME_HOST_SEED_STARTING_BUILDING")
                        .is_some_and(|v| {
                            let s = v.to_string_lossy();
                            !(s.is_empty()
                                || s == "0"
                                || s.eq_ignore_ascii_case("false")
                                || s.eq_ignore_ascii_case("no"))
                        });
                if allow_seed_building && !building.is_empty() {
                    let mut pos = pos_opt.unwrap_or_else(|| {
                        let (bmin, bmax) = self.world_bounds();
                        let t = (pid as f32 + 1.0) / (self.players.len().max(1) as f32 + 1.0);
                        Vec3::new(
                            bmin.x + (bmax.x - bmin.x) * t,
                            0.0,
                            bmin.z + (bmax.z - bmin.z) * 0.2,
                        )
                    });
                    if let Some(h) = self.terrain_height_at(Vec3::new(pos.x, 0.0, pos.z)) {
                        pos.y = h;
                    }
                    self.ensure_ai_faction_templates(player.team);
                    if self
                        .create_object_for_player_with_effects(building, pid, pos, None, effects)
                        .is_some()
                    {
                        gamelogic::player::notify_skirmish_starting_object(pid, building, true);
                        base = Some(pos);
                        log::info!(
                            "Wave 831/832: seeded starting building {} for player {} at {:?}",
                            building,
                            pid,
                            pos
                        );
                    }
                }
            }

            let mut base_pos = base
                .or_else(|| self.player_base_position(pid))
                .unwrap_or_else(|| {
                    let (bmin, bmax) = self.world_bounds();
                    let t = (pid as f32 + 1.0) / (self.players.len().max(1) as f32 + 1.0);
                    Vec3::new(
                        bmin.x + (bmax.x - bmin.x) * t,
                        0.0,
                        bmin.z + (bmax.z - bmin.z) * 0.2,
                    )
                });
            if let Some(h) = self.terrain_height_at(Vec3::new(base_pos.x, 0.0, base_pos.z)) {
                base_pos.y = h;
            }

            // C++ placeNetworkBuildingsForPlayer: units at Player_%d_Rally when present.
            let want_idx = if player.start_position >= 0 {
                player.start_position as u32
            } else {
                pid
            };
            let rally_pos = self.player_rally_spawn_pos(want_idx);
            // --- Starting units 0..9 (C++ placeStartingUnits / MAX_MP_STARTING_UNITS) ---
            // Wave 832: walk PlayerTemplate StartingUnit0..9; retail usually
            // only unit0 (dozer).
            let unit_names: Vec<&str> = starting_units
                .iter()
                .map(String::as_str)
                .filter(|n| !n.is_empty())
                .collect();
            if unit_names.is_empty() {
                continue;
            }
            self.ensure_ai_faction_templates(player.team);
            for (i, unit_name) in unit_names.iter().enumerate() {
                // Skip if this exact starting unit template already exists for the team.
                let already = self.objects.values().any(|o| {
                    o.owner_player_id == Some(pid)
                        && o.is_alive()
                        && o.template_name.eq_ignore_ascii_case(unit_name)
                });
                // For builders/workers: also treat any mobile constructor as present.
                let is_builder = unit_name.to_ascii_lowercase().contains("dozer")
                    || unit_name.to_ascii_lowercase().contains("worker");
                let has_builder = is_builder
                    && self.objects.values().any(|o| {
                        o.owner_player_id == Some(pid)
                            && o.is_alive()
                            && o.is_mobile()
                            && (o.can_construct()
                                || o.template_name.to_ascii_lowercase().contains("dozer")
                                || o.template_name.to_ascii_lowercase().contains("worker"))
                    });
                if already || has_builder {
                    continue;
                }

                // C++ uses rally as findPositionAround center; invented yard offsets
                // only when the map has no Player_N_Rally.
                let mut unit_pos = if let Some(rally) = rally_pos {
                    rally
                } else {
                    base_pos + Vec3::new(40.0 + (i as f32) * 12.0, 0.0, -40.0 - (i as f32) * 6.0)
                };
                if let Some(h) = self.terrain_height_at(Vec3::new(unit_pos.x, 0.0, unit_pos.z)) {
                    unit_pos.y = h;
                }
                if let Some(id) = self
                    .create_object_for_player_with_effects(unit_name, pid, unit_pos, None, effects)
                {
                    if let Some(obj) = self.host_object_mut(id) {
                        if obj.is_mobile()
                            && !obj.is_kind_of(crate::game_logic::KindOf::Selectable)
                            && !obj.is_kind_of(crate::game_logic::KindOf::AlwaysSelectable)
                        {
                            obj.template_mut()
                                .add_kind_of(crate::game_logic::KindOf::Selectable);
                        }
                    }
                    gamelogic::player::notify_skirmish_starting_object(pid, unit_name, false);
                    log::info!(
                        "Wave 832: starting unit player={} team={:?} spawned {} id={:?}",
                        pid,
                        player.team,
                        unit_name,
                        id
                    );
                } else if i == 0 {
                    // Fallback retail short names for unit0 only.
                    // USA: ThingFactory AmericaVehicleDozer, then host USA_Dozer.
                    let fallbacks: &[&str] = match player.team {
                        Team::USA => &["AmericaVehicleDozer", "USA_Dozer"],
                        Team::China => &["ChinaVehicleDozer", "China_Dozer"],
                        Team::GLA => &["GLAInfantryWorker", "GLA_Worker"],
                        Team::Neutral => &[],
                    };
                    for fallback in fallbacks {
                        if fallback.eq_ignore_ascii_case(unit_name) {
                            continue;
                        }
                        if let Some(id) = self.create_object_for_player_with_effects(
                            fallback, pid, unit_pos, None, effects,
                        ) {
                            if let Some(obj) = self.host_object_mut(id) {
                                if obj.is_mobile()
                                    && !obj.is_kind_of(crate::game_logic::KindOf::Selectable)
                                    && !obj.is_kind_of(crate::game_logic::KindOf::AlwaysSelectable)
                                {
                                    obj.template_mut()
                                        .add_kind_of(crate::game_logic::KindOf::Selectable);
                                }
                            }
                            gamelogic::player::notify_skirmish_starting_object(
                                pid, fallback, false,
                            );
                            log::info!(
                                "Wave 832: starting unit fallback player={} {} id={:?}",
                                pid,
                                fallback,
                                id
                            );
                            break;
                        }
                    }
                }
            }
        }
    }
}

/// C++ ThingFactory.cpp:286-292 GameLogicRandomValue over BuildVariations.
fn resolve_production_build_variation(template: &str) -> String {
    let Some(guard) = game_engine::common::thing::thing_factory::try_get_thing_factory() else {
        return template.to_string();
    };
    let Some(factory) = guard.as_ref() else {
        return template.to_string();
    };
    let Some(tmpl) = factory.find_template(template, false) else {
        return template.to_string();
    };
    let variations = tmpl.get_build_variations();
    if variations.is_empty() {
        return template.to_string();
    }
    let max = variations.len().saturating_sub(1) as i32;
    let index = gamelogic::helpers::get_game_logic_random_value(0, max) as usize;
    let Some(name) = variations.get(index) else {
        return template.to_string();
    };
    if factory.find_template(name.as_str(), false).is_some() {
        name.as_str().to_string()
    } else {
        template.to_string()
    }
}

#[cfg(test)]
#[path = "allied_construction_tests.rs"]
mod allied_construction_tests;
