//! Host tick `impl GameLogic` — `production`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// C++ DozerActionDoActionState BUILD (DozerAIUpdate.cpp:482-526):
    /// progress only after idle at ACTION dock (`DOZER_DO_BUILD_AT_DOCK`).
    /// C++ DozerAIUpdate.cpp:305 — one exclusive builder per structure.

    pub(in super::super) fn update_construction(&mut self, object_ids: &[ObjectId], dt: f32) {
        const BUILDER_RANGE: f32 = crate::game_logic::host_repair::DOZER_MIN_ACTION_TOLERANCE;

        // C++ parity: calcTimeToBuild applies the same power penalty to dozer
        // construction as to production queue speed.
        let player_power_factor = self.compute_player_power_factors();
        // Resolve legacy ownerless objects before taking mutable object borrows.
        // A concrete PlayerId is authoritative; the helper only supplies a
        // compatibility owner when this team has exactly one living player.
        let object_owner_player_ids: std::collections::HashMap<ObjectId, Option<u32>> = self
            .objects
            .values()
            .map(|obj| (obj.id, self.player_owner_for_host_object(obj)))
            .collect();

        // C++ `ThingTemplate::calcTimeToBuild(player)` converts build time to
        // integer logic frames, multiplies Handicap::BUILDTIME (BUILDINGS vs
        // GENERIC) then PlayerTemplate `ProductionTimeChange`, and only then
        // applies low-power timing.
        // Snapshot the pre-power frame counts before mutable Object borrows;
        // they remain player-owned start state and do not depend on tick
        // ordering.
        let authored_time_frames: std::collections::HashMap<ObjectId, u32> = object_ids
            .iter()
            .filter_map(|id| {
                let obj = self.objects.get(id)?;
                let player_id = object_owner_player_ids.get(id).copied().flatten()?;
                let is_structure = obj.is_kind_of(KindOf::Structure);
                let handicap = self
                    .players
                    .get(&player_id)
                    .map(|p| p.handicap_build_time_multiplier(is_structure))
                    .unwrap_or(1.0);
                let factor = self
                    .player_template_production_time_factor(player_id, &obj.template_name)
                    * handicap;
                Some((
                    *id,
                    Self::cpp_build_time_frames_from_factor(obj.thing.template.build_time, factor),
                ))
            })
            .collect();

        // Pre-scan dozers: exclusive builder + idle at ACTION dock
        // (C++ DozerActionDoActionState BUILD, DozerAIUpdate.cpp:482-507).
        let dozer_info: Vec<(
            ObjectId,
            Vec3,
            Option<u32>,
            Option<ObjectId>,
            bool,
            Option<Vec3>,
        )> = self
            .objects
            .values()
            .filter(|obj| obj.is_alive() && obj.can_construct())
            .map(|obj| {
                let arrived = !obj.status.moving
                    && obj.movement.current_path_index >= obj.movement.path.len();
                (
                    obj.id,
                    obj.get_position(),
                    object_owner_player_ids.get(&obj.id).copied().flatten(),
                    obj.target,
                    arrived,
                    obj.dozer_dock_action,
                )
            })
            .collect();

        let mut completed_superweapon_detects: Vec<ObjectId> = Vec::new();
        let mut completed_structures: Vec<ObjectId> = Vec::new();
        let mut ready_superweapons: Vec<ObjectId> = Vec::new();
        let mut radar_extend_done: Vec<ObjectId> = Vec::new();
        let mut start_build_loops: Vec<(ObjectId, String, Vec3)> = Vec::new();
        let mut stop_build_loops: Vec<ObjectId> = Vec::new();
        // Wave 617: under sole-tick, GameWorld writeback records ready structures;
        // host applies completion after writeback same frame (Wave 715; not mid-update drain).
        let construction_sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
        // Empty mid-update ready set: sole completes only via post-writeback helper (Wave 715).
        // Non-sole completes via projected percent (may_complete=true).
        // Unmapped sole-tick also uses projected (no writeback entity).
        let ready_structures: std::collections::HashSet<ObjectId> =
            std::collections::HashSet::new();
        for &id in object_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if obj.status.under_construction {
                    let build_pos = obj.get_position();
                    let build_radius = obj.selection_radius;
                    let site_template = obj.template_name.clone();
                    let build_owner_player_id = object_owner_player_ids.get(&id).copied().flatten();
                    // Exclusive dock: only the structure's builder_id (C++ getBuilderID)
                    // or, if unset, a single targeting dozer may contribute.
                    let exclusive_builder = obj.builder_id;
                    let nearby_dozers = dozer_info
                        .iter()
                        .filter(|(did, pos, owner_player_id, target, arrived, stored_dock)| {
                            *owner_player_id == build_owner_player_id
                                && *target == Some(id)
                                && *arrived
                                && exclusive_builder.map(|bid| bid == *did).unwrap_or(true)
                                && {
                                    // C++ DOZER_DO_BUILD_AT_DOCK: idle at ACTION dock,
                                    // not within 70wu of the structure centre (cpp:485-507).
                                    let dock = crate::game_logic::host_repair::resolve_dozer_action_dock(
                                        *stored_dock,
                                        *pos,
                                        build_pos,
                                        build_radius,
                                    );
                                    pos.distance(dock) <= BUILDER_RANGE
                                }
                        })
                        .count()
                        .min(1);

                    // C++ DozerAIUpdate: no docked dozer ⇒ no progress. Do not invent a ghost builder.
                    let dozer_count = nearby_dozers;
                    let actively_built = nearby_dozers > 0;
                    obj.set_under_construction_model_conditions(actively_built);
                    self.construction_model_condition_updates =
                        self.construction_model_condition_updates.saturating_add(1);

                    let power_factor = build_owner_player_id
                        .and_then(|player_id| player_power_factor.get(&player_id).copied())
                        .unwrap_or(1.0);
                    let authored_frames =
                        authored_time_frames.get(&id).copied().unwrap_or_else(|| {
                            Self::cpp_build_time_frames_from_factor(
                                obj.thing.template.build_time,
                                1.0,
                            )
                        });
                    // Keep the existing zero-duration one-tick safeguard, but
                    // otherwise advance from C++'s already-truncated authored
                    // frame count rather than multiplying seconds first.
                    let base_rate = if authored_frames == 0 {
                        100.0
                    } else {
                        30.0 / authored_frames as f32
                    };
                    let effective_rate = base_rate * dozer_count as f32 * power_factor;
                    // Under CONSTRUCTION_AUTHORITY + shadow, GameWorld sole-ticks percent
                    // using effective_rate; host only completes when writeback hits 1.0
                    // (Wave 617: readiness gated by host_construction_ready_log).
                    // Prior freeze without rate residual stalled builds — rate is logged.
                    let sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
                    let gw_mapped = crate::gameworld_shadow::coupled_host_mapped(id);
                    // Sole-tick only when this object is actually in GameWorld.
                    // Unmapped barracks (eager spawn miss) must keep host-advancing
                    // *and storing* percent or they stay at 0 forever
                    // (train_fail_no_ready_barracks).
                    let projected = if sole && gw_mapped {
                        obj.construction_percent
                    } else {
                        (obj.construction_percent + effective_rate * dt).min(1.0)
                    };
                    if !sole || !gw_mapped {
                        // Host-owned percent: no coupled entity, or sole-tick off.
                        obj.construction_percent = projected;
                        crate::game_logic::host_construction_progress_log::record(
                            id,
                            projected,
                            obj.status.under_construction,
                            effective_rate,
                        );
                    } else {
                        // Wave 478: publish dozer/power rate only — no percent stomp.
                        crate::game_logic::host_construction_progress_log::record_rate_only(
                            id,
                            obj.status.under_construction,
                            effective_rate,
                        );
                    }

                    // Wave 617/713: mapped sole-tick completes only via ready-log.
                    // Unmapped objects never appear in writeback, so host must
                    // complete them when projected hits 1.0.
                    let may_complete = if construction_sole && gw_mapped {
                        ready_structures.contains(&id)
                    } else {
                        true
                    };
                    // C++ DozerAIUpdate.cpp:526 then :536 — increment health first,
                    // then complete. Completion itself never writes max health, so
                    // scaffold damage taken during build persists.
                    if !(construction_sole && gw_mapped) {
                        let frames = authored_frames.max(1) as f32;
                        let per_frame = obj.health.maximum / frames;
                        let logic_frames = (dt * 30.0).max(0.0);
                        let build_hp = if actively_built {
                            (obj.health.current + per_frame * logic_frames)
                                .min(obj.health.maximum)
                                .max(1.0)
                        } else {
                            obj.health.current.max(1.0).min(obj.health.maximum)
                        };
                        if crate::gameworld_shadow::gameworld_damage_authority_live() {
                            crate::game_logic::host_heal_log::record(id, build_hp);
                        } else {
                            obj.health.current = build_hp;
                            crate::game_logic::host_heal_log::record(id, obj.health.current);
                        }
                    }
                    if may_complete && projected >= 1.0 {
                        obj.construction_percent = 1.0;
                        obj.set_status_under_construction(false);
                        obj.clear_under_construction_model_conditions();
                        crate::game_logic::host_construction_progress_log::record(
                            id, 1.0, false, 0.0,
                        );
                        crate::game_logic::host_construction_log::record(
                            id,
                            obj.template_name.clone(),
                        );
                        // C++ onStructureConstructionComplete SuperweaponDetected residual.
                        completed_superweapon_detects.push(id);
                        completed_structures.push(id);
                        stop_build_loops.push(id);
                    }

                    if !(may_complete && projected >= 1.0) && actively_built {
                        start_build_loops.push((id, site_template, build_pos));
                    }
                }
                if obj.tick_timers(dt) {
                    // Defer EVA until after borrow ends.
                    ready_superweapons.push(id);
                }
                // Wave 744: under coupled GameWorld shadow, radar-extend complete
                // is owned by writeback + host_apply_radar_extend_ready_completions.
                // Host must not dual-complete via tick_radar_extend mid-frame.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if obj.tick_radar_extend(self.frame) {
                        radar_extend_done.push(id);
                    }
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
                if !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
                    if obj.tick_construction_complete_clear(self.frame) {
                        self.construction_complete_clears =
                            self.construction_complete_clears.saturating_add(1);
                    }
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

        for id in completed_superweapon_detects {
            self.try_eva_superweapon_detected_for_source(id);
        }

        // C++ parity: when a structure finishes construction, release any dozers
        // that were constructing it — set them to Idle.
        for &completed_id in &completed_structures {
            // C++ SupplyCenterCreate::onBuildComplete residual.
            self.on_supply_center_build_complete(completed_id);
            let building_pos = self.objects.get(&completed_id).map(|o| o.get_position());
            let mut end_moves: Vec<(ObjectId, glam::Vec3)> = Vec::new();
            for obj in self.objects.values_mut() {
                if obj.ai_state == AIState::Constructing
                    && obj.target == Some(completed_id)
                    && obj.is_alive()
                {
                    let oid = obj.id;
                    let dozer_pos = obj.get_position();
                    let stored_action = obj.dozer_dock_action;
                    let is_worker = obj.is_resource_collector()
                        || obj.template_name.to_ascii_lowercase().contains("worker");
                    obj.set_target(None);
                    if is_worker {
                        obj.stop_moving();
                    } else if let Some(bpos) = building_pos {
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
                    }
                    obj.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(oid, 0);
                    }
                }
            }
            for (oid, mut end) in end_moves {
                // C++ DozerAIUpdate.cpp:627-635 adjustToPossibleDestination then aiMoveToPosition(END).
                self.adjust_to_possible_destination(oid, &mut end);
                self.path_approach_with_state(oid, end, AIState::Moving);
            }
            if self.objects.contains_key(&completed_id) {
                self.record_structure_completion(completed_id);
            }
            // C++ onStructureConstructionComplete feedback residual.
            self.notify_structure_construction_complete(completed_id);
            // C++ RadarUpdate::extendRadar is only called from
            // RadarUpgrade::upgradeImplementation, not on every CC complete.
            // Constructed footprint is a static path/LOS obstacle.
            self.block_structure_object_path(completed_id);
        }
        // C++ ACTIVELY_CONSTRUCTING residual for dozers/factories.
        // Wave 815: under coupled shadow, model bit owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_actively_constructing_model_conditions();
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
        if crate::game_logic::audio_dispatch_impl::building_loop_event(site_id.0).as_deref()
            == Some(event.as_str())
        {
            return;
        }
        if let Some(old) = crate::game_logic::audio_dispatch_impl::take_building_loop(site_id.0) {
            self.queue_audio_event(
                crate::game_logic::AudioEventRequest::new(&old)
                    .with_object(site_id)
                    .with_position(pos)
                    .stopping(),
            );
        }
        crate::game_logic::audio_dispatch_impl::note_building_loop(site_id.0, &event);
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
        let Some(event) = crate::game_logic::audio_dispatch_impl::take_building_loop(site_id.0)
        else {
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

    /// Wave 715: after GW construction writeback records ready structures, host
    /// applies completion side effects in the same coupled tick (not next frame).

    /// Wave 717: after GW special-power writeback records ready flips, host
    /// applies EVA superweapon-ready residual in the same coupled tick.
    pub(crate) fn host_apply_special_power_ready_after_writeback(&mut self) {
        if !crate::gameworld_shadow::gameworld_special_power_sole_tick_enabled() {
            return;
        }
        for ev in crate::game_logic::host_special_power_ready_log::drain() {
            if self
                .objects
                .get(&ev.object)
                .is_some_and(|obj| obj.is_alive())
            {
                self.try_eva_superweapon_ready_for_source(ev.object);
            }
        }
    }

    pub(crate) fn host_apply_construction_completions_after_ready_writeback(&mut self) {
        if !crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            return;
        }
        let ready: Vec<ObjectId> = crate::game_logic::host_construction_ready_log::drain()
            .into_iter()
            .map(|ev| ev.structure)
            .collect();
        if ready.is_empty() {
            return;
        }
        let mut completed_superweapon_detects: Vec<ObjectId> = Vec::new();
        let mut completed_structures: Vec<ObjectId> = Vec::new();
        for id in ready {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            // Writeback may already have percent=1.0 while under_construction remains set.
            if !(obj.status.under_construction || obj.construction_percent + 1e-6 >= 1.0) {
                continue;
            }
            obj.construction_percent = 1.0;
            obj.set_status_under_construction(false);
            obj.clear_under_construction_model_conditions();
            // C++ DozerAIUpdate.cpp:538-561: complete does not set health.
            // Persist scaffold HP (damage authority writeback already owns it).
            crate::game_logic::host_heal_log::record(id, obj.health.current);
            crate::game_logic::host_construction_progress_log::record(id, 1.0, false, 0.0);
            crate::game_logic::host_construction_log::record(id, obj.template_name.clone());
            completed_superweapon_detects.push(id);
            completed_structures.push(id);
        }
        for id in completed_superweapon_detects {
            self.try_eva_superweapon_detected_for_source(id);
        }
        for &completed_id in &completed_structures {
            self.on_supply_center_build_complete(completed_id);
            let building_pos = self.objects.get(&completed_id).map(|o| o.get_position());
            let mut end_moves: Vec<(ObjectId, glam::Vec3)> = Vec::new();
            for obj in self.objects.values_mut() {
                if obj.ai_state == AIState::Constructing
                    && obj.target == Some(completed_id)
                    && obj.is_alive()
                {
                    let oid = obj.id;
                    let dozer_pos = obj.get_position();
                    let stored_action = obj.dozer_dock_action;
                    let is_worker = obj.is_resource_collector()
                        || obj.template_name.to_ascii_lowercase().contains("worker");
                    obj.set_target(None);
                    if is_worker {
                        obj.stop_moving();
                    } else if let Some(bpos) = building_pos {
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
                    }
                    obj.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(oid, 0);
                    }
                }
            }
            for (oid, mut end) in end_moves {
                self.adjust_to_possible_destination(oid, &mut end);
                self.path_approach_with_state(oid, end, AIState::Moving);
            }
            if self.objects.contains_key(&completed_id) {
                self.record_structure_completion(completed_id);
            }
            self.notify_structure_construction_complete(completed_id);
            self.block_structure_object_path(completed_id);
        }
        if !completed_structures.is_empty() {
            // Wave 828: under coupled shadow, ACTIVELY_CONSTRUCTING bit owned by GW expire.
            if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                && crate::gameworld_shadow::shadow_coupled_tick_active())
            {
                self.update_actively_constructing_model_conditions();
            }
        }
    }

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
            crate::game_logic::host_production_log::record_cancel(producer_id, item.template_name);
        }
    }

    pub(in super::super) fn update_production(&mut self, dt: f32) {
        // C++ ProductionUpdate.cpp:671 — re-check allowedToBuild on queue head.
        self.cancel_script_disallowed_production_queue_heads();
        // Wave 613: production complete collect + apply via host helpers.
        // Under PRODUCTION_AUTHORITY sole-tick, GameWorld advances queue progress
        // and writeback finishes heads; host try_complete + spawn runs after
        // shadow writeback same frame (Wave 714) so ready-log is not a frame late.
        // Wave 875: sole-tick early-return honesty — no host dual-advance.
        if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
            self.publish_production_power_factors();
            return;
        }

        let (upgrade_completions, unit_completions) = self.host_collect_production_completions(dt);
        // Wave 595/608: host production complete/spawn apply residual via host helpers.
        self.apply_upgrade_production_completions(upgrade_completions);
        self.apply_unit_production_completions(unit_completions);
    }

    /// Wave 714: after GW production writeback records ready producers, host
    /// try_completes + spawns in the same coupled tick (not next frame).
    pub(crate) fn host_apply_production_completions_after_ready_writeback(&mut self, dt: f32) {
        if !crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
            return;
        }
        self.cancel_script_disallowed_production_queue_heads();
        let (upgrade_completions, unit_completions) = self.host_collect_production_completions(dt);
        self.apply_upgrade_production_completions(upgrade_completions);
        self.apply_unit_production_completions(unit_completions);
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

        // Wave 614: under sole-tick, GameWorld writeback records ready producers;
        // host only try_completes those IDs (GW decides readiness).
        let sole = crate::gameworld_shadow::gameworld_production_sole_tick_enabled();
        // Wave 735: keep every ready event (template + GW spawn pose/rally),
        // not just a producer ID.  A C++ QuantityModifier batch can produce
        // multiple units from the same completed head in this update.
        let ready_by_producer: std::collections::HashMap<
            ObjectId,
            Vec<crate::game_logic::host_production_ready_log::HostProductionReadyEvent>,
        > = if sole {
            let mut events_by_producer = std::collections::HashMap::new();
            for event in crate::game_logic::host_production_ready_log::drain() {
                events_by_producer
                    .entry(event.producer)
                    .or_insert_with(Vec::new)
                    .push(event);
            }
            events_by_producer
        } else {
            std::collections::HashMap::new()
        };
        let ready_producers: std::collections::HashSet<ObjectId> =
            ready_by_producer.keys().copied().collect();

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
            let exit_metadata = obj.thing.template.production_exit_metadata;
            let producer_template = obj.template_name.clone();
            let parking_place = obj.thing.template.parking_place.clone();
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
                let completed_prod = if sole {
                    // Wave 464/614: GameWorld sole-ticks progress + exit delay and
                    // records ready producers on writeback; host try_completes only
                    // ready IDs (Wave 713: empty ready log ⇒ no host scan).
                    // Limit a QuantityModifier release to matching entity-first
                    // ready events so stale/missing events cannot make the host
                    // create an unbound unit.
                    if ready_producers.contains(&id) {
                        if airfield_holds_parking && parking_free == 0 {
                            if let Some(events) = ready_by_producer.get(&id) {
                                for event in events {
                                    crate::game_logic::host_production_ready_log::record_with_pose(
                                        event.producer,
                                        event.template_name.clone(),
                                        event.is_upgrade,
                                        event.spawn_pos,
                                        event.rally,
                                        event.gw_entity_raw,
                                    );
                                }
                            }
                            None
                        } else {
                            let ready_count = ready_by_producer
                                .get(&id)
                                .and_then(|events| {
                                    building.production_queue.first().map(|head| {
                                        events
                                            .iter()
                                            .filter(|event| {
                                                event.is_upgrade == head.is_upgrade()
                                                    && event
                                                        .template_name
                                                        .eq_ignore_ascii_case(&head.template_name)
                                            })
                                            .count()
                                            .min(u32::MAX as usize)
                                            as u32
                                    })
                                })
                                .unwrap_or(0);
                            let ready_count = if airfield_holds_parking {
                                ready_count.min(parking_free as u32)
                            } else {
                                ready_count
                            };
                            (ready_count > 0)
                                .then(|| {
                                    building.try_complete_production_at_power_with_exit_metadata(
                                        pf,
                                        exit_metadata.as_ref(),
                                        Some(ready_count),
                                    )
                                })
                                .flatten()
                        }
                    } else {
                        None
                    }
                } else {
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
                if (!building.production_queue.is_empty() || completed_this_tick)
                    && !crate::gameworld_shadow::gameworld_production_sole_tick_enabled()
                {
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
                } else if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
                    // Wave 477: still publish power factor for GW sole-tick rate without
                    // stomping queue progress via full progress-log apply.
                    crate::game_logic::host_production_progress_log::record_power_factor_only(
                        id, pf,
                    );
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
                            for completion_index in 0..completed.quantity {
                                let mut rally = completion_rally;
                                // Spawn slightly offset from the building facing to reduce clumping.
                                let forward = obj.thing.get_direction_vector();
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
                                let mut completed_name = completed_template.clone();
                                if sole {
                                    let event = ready_by_producer.get(&id).and_then(|events| {
                                        events
                                            .iter()
                                            .filter(|event| {
                                                !event.is_upgrade
                                                    && event
                                                        .template_name
                                                        .eq_ignore_ascii_case(&completed_template)
                                            })
                                            .nth(completion_index as usize)
                                    });
                                    let Some(event) = event else {
                                        // The batch completion was capped by this exact
                                        // filtered set, but retain a fail-closed guard if
                                        // a future producer changes it mid-collection.
                                        continue;
                                    };
                                    if !event.template_name.is_empty() {
                                        completed_name = event.template_name.clone();
                                    }
                                    if let Some(p) = event.spawn_pos {
                                        spawn_pos = Vec3::new(p[0], p[1], p[2]);
                                    }
                                    if let Some(r) = event.rally {
                                        rally = Some(Vec3::new(r[0], r[1], r[2]));
                                    }
                                    if let Some(raw) = event.gw_entity_raw {
                                        crate::game_logic::host_production_ready_log::push_pending_bind(
                                            raw,
                                        );
                                    }
                                }
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
        // Wave 608: thin wrapper — production complete apply via host helper.
        self.host_apply_upgrade_production_completions(upgrade_completions)
    }

    /// Wave 595: host upgrade production completion residual (still host-side under
    /// PRODUCTION_AUTHORITY; GameWorld sole-ticks queue progress only).
    pub(in super::super) fn host_apply_upgrade_production_completions(
        &mut self,
        upgrade_completions: Vec<(Team, String, ObjectId)>,
    ) {
        // Wave 608: host production complete/spawn apply residual.
        // Wave 595: host upgrade production completion residual.
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
            crate::game_logic::host_production_log::record_complete(
                producer_id,
                upgrade_name.clone(),
                ObjectId(0),
            );
            // Unlock via player queue drain + host apply path.
            if let Some(pid) = producer_owner_player_id {
                let already = self
                    .players
                    .get(&pid)
                    .map(|p| p.has_unlocked_upgrade(&upgrade_name))
                    .unwrap_or(false);
                if let Some(player) = self.players.get_mut(&pid) {
                    player.complete_researched_upgrade(&upgrade_name);
                    // record_upgrade_production_complete: leftover AcademyStats::recordUpgrade
                    // (FALSE) + ScoreKeeper::addMoneySpent (ProductionUpdate.cpp:874-879 / 931).
                }
                if !already {
                    self.apply_host_upgrade_complete(team, pid, &upgrade_name);
                    // C++ ProductionUpdate.cpp:881-885 TheScriptEngine->notifyOfCompletedUpgrade
                    if let Ok(mut engine_guard) =
                        gamelogic::scripting::engine::get_script_engine().write()
                    {
                        if let Some(engine) = engine_guard.as_mut() {
                            engine.notify_of_completed_upgrade(
                                pid as usize,
                                upgrade_name.as_str(),
                                producer_id.0,
                            );
                        }
                    }
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
        // Wave 608: thin wrapper — production complete apply via host helper.
        self.host_apply_unit_production_completions(unit_completions)
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
    pub(in super::super) fn host_spawn_rebuild_bound_object(
        &mut self,
        template: &str,
        team: Team,
        owner_player_id: Option<u32>,
        spawn_pos: Vec3,
        gw_entity_raw: Option<u32>,
    ) -> Option<ObjectId> {
        if crate::gameworld_shadow::gameworld_construction_sole_tick_enabled() {
            if let Some(raw) = gw_entity_raw {
                crate::gameworld_shadow::set_next_host_spawn_bind_entity(raw);
                let preferred = ObjectId(raw);
                if raw != 0 && !self.objects.contains_key(&preferred) {
                    let saved_next = self.next_object_id;
                    self.next_object_id = preferred;
                    let spawned = self.create_object_for_owner_or_team(
                        template,
                        team,
                        owner_player_id,
                        spawn_pos,
                    );
                    let after = self.next_object_id.0;
                    self.next_object_id = ObjectId(saved_next.0.max(after));
                    if spawned.is_some() {
                        return spawned;
                    }
                    self.next_object_id = saved_next;
                }
                // Bind present: allocate host id and map to pre-spawned entity.
                return self.create_object_for_owner_or_team(
                    template,
                    team,
                    owner_player_id,
                    spawn_pos,
                );
            }
            let allow_without_bind =
                std::env::var_os("GENERALS_RUNTIME_HOST_REBUILD_SPAWN_WITHOUT_GW_BIND")
                    .is_some_and(|v| {
                        let s = v.to_string_lossy();
                        !(s.is_empty()
                            || s == "0"
                            || s.eq_ignore_ascii_case("false")
                            || s.eq_ignore_ascii_case("no"))
                    });
            if !allow_without_bind {
                log::debug!(
                    "Wave 741: construction sole-tick rebuild spawn denied without GW entity bind (template={template})"
                );
                return None;
            }
        }
        self.create_object_for_owner_or_team(template, team, owner_player_id, spawn_pos)
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
        if crate::gameworld_shadow::gameworld_production_sole_tick_enabled()
            || crate::gameworld_shadow::shadow_coupled_tick_active()
        {
            if let Some(raw) = crate::game_logic::host_production_ready_log::pop_pending_bind() {
                crate::gameworld_shadow::set_next_host_spawn_bind_entity(raw);
                let preferred = ObjectId(raw);
                if raw != 0 && !self.objects.contains_key(&preferred) {
                    let saved_next = self.next_object_id;
                    self.next_object_id = preferred;
                    let spawned = self.create_object_for_owner_or_team(
                        template,
                        team,
                        owner_player_id,
                        spawn_pos,
                    );
                    // Keep monotonic next_id at least past both saved and allocated.
                    let after = self.next_object_id.0;
                    self.next_object_id = ObjectId(saved_next.0.max(after));
                    if spawned.is_some() {
                        return spawned;
                    }
                    // create_object failed — restore and fall through with bind still set
                    // only if create_object did not consume it (template miss).
                    self.next_object_id = saved_next;
                }
                // Bind present (preferred collision or create miss): host allocate + map.
                return self.create_object_for_owner_or_team(
                    template,
                    team,
                    owner_player_id,
                    spawn_pos,
                );
            }
            let allow_without_bind =
                std::env::var_os("GENERALS_RUNTIME_HOST_PRODUCTION_SPAWN_WITHOUT_GW_BIND")
                    .is_some_and(|v| {
                        let s = v.to_string_lossy();
                        !(s.is_empty()
                            || s == "0"
                            || s.eq_ignore_ascii_case("false")
                            || s.eq_ignore_ascii_case("no"))
                    });
            if !allow_without_bind {
                log::debug!(
                    "Wave 738: sole-tick production spawn denied without GW entity bind (template={template})"
                );
                return None;
            }
        }
        self.create_object_for_owner_or_team(template, team, owner_player_id, spawn_pos)
    }

    /// Wave 595: host unit production completion residual — spawn, door, exit delay,
    /// rally path. GameWorld sole-ticks progress; host still completes/spawns.
    pub(in super::super) fn host_apply_unit_production_completions(
        &mut self,
        unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)>,
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
            let new_id =
                match self.apply_production_authority_op(ProductionAuthorityOp::SpawnUnit {
                    template: template.clone(),
                    team,
                    owner_player_id,
                    spawn_pos,
                }) {
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
                        let _ = self.place_produced_jet_at_parking_pose(producer_id, new_id);
                    } else {
                        log::warn!(
                            "Production aircraft {:?} from airfield {:?} held: no ParkingPlace stall",
                            new_id,
                            producer_id
                        );
                        self.destroy_object(new_id);
                        continue;
                    }
                }

                crate::game_logic::host_production_log::record_complete(
                    producer_id,
                    template.clone(),
                    new_id,
                );
                // Wave 679: production spawn ObjectId ready residual —
                // host door/notify/exit/path apply drains the ready log.
                crate::game_logic::host_production_spawn_ready_log::record(
                    new_id,
                    producer_id,
                    template,
                    [spawn_pos.x, spawn_pos.y, spawn_pos.z],
                    rally.map(|r| [r.x, r.y, r.z]),
                );
                let _ = self.apply_production_authority_op(
                    ProductionAuthorityOp::ApplySpawnReadyCompletions,
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
        let cell = self.pathfinding_system.grid.world_to_grid(dest);
        let adj = self.pathfinding_system.grid.adjust_destination_ex(
            cell,
            surfaces,
            is_crusher,
            400,
            seeker_player,
            crusher_level,
        )?;
        let (_, center_in_cell) = Self::factory_exit_radius_and_center(unit_radius);
        Some(self.factory_exit_coord_to_cell(adj, center_in_cell, dest.y))
    }

    /// C++ `AIUpdateInterface::aiFollowExitProductionPath`.
    /// Ignore the producer, walk the authored points, and path through units.
    fn follow_exit_production_path(
        &mut self,
        unit_id: ObjectId,
        path: &[Vec3],
        ignore_producer: ObjectId,
    ) {
        if path.is_empty() {
            return;
        }
        self.path_approach_with_state_ignoring(
            unit_id,
            path[0],
            AIState::Moving,
            Some(ignore_producer),
        );
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
                    unit.movement.target_position = Some(end);
                }
            } else {
                let _ = self.append_unit_waypoint(unit_id, wp);
            }
        }
        // After the path is installed — move_object_with_pathfinding
        // clears this on new orders (C++ FollowPath onExit).
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.can_path_through_units = true;
        }
        // C++ AIUpdate.cpp:1674-1681 computePath factory-exit:
        // computeQuickPath ok → leftover moveAlliesAwayFromDestination.
        self.move_allies_away_from_destination(unit_id, path[0]);
    }

    /// Wave 679: drain production-spawn ready log and apply host presentation residual
    /// (notify/door/exit/path) for the newly allocated host ObjectId.
    /// Still host ObjectId authority — not full GameWorld spawn-ID ownership.
    pub fn host_apply_production_spawn_ready_completions(&mut self) -> usize {
        // Wave 679: drain production-spawn ready log and apply host presentation residual.
        let events = crate::game_logic::host_production_spawn_ready_log::drain();
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
                .and_then(|producer| producer.thing.template.production_exit_metadata);
            let mut spawn_pos = Vec3::new(ev.spawn_pos[0], ev.spawn_pos[1], ev.spawn_pos[2]);
            let rally = ev.rally.map(|r| Vec3::new(r[0], r[1], r[2]));
            // Wave 739: under production sole-tick, GameWorld ready-log pose is
            // authoritative — do not reposition the unit here (host
            // create_object already placed at GW exit pose). Non-sole path
            // places at the exact transformed UnitCreatePoint (C++ Queue/Default
            // exitObjectViaDoor). Anti-stack is the doubled natural-rally
            // waypoint, not a spawn offset.
            let sole = crate::gameworld_shadow::gameworld_production_sole_tick_enabled();
            // C++ VoiceCreated + UnitReady residual.
            self.notify_unit_production_complete(new_id, producer_id, &template);
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
                    prod.record_host_production_door();
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
                    if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
                        crate::game_logic::host_production_progress_log::record_exit_runtime_only(
                            producer_id,
                            building.exit_delay_remaining,
                            building.production_exit_runtime_state(),
                        );
                    }
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
            if !sole && !parked_jet {
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
                    if crate::gameworld_shadow::gameworld_movement_authority_live() {
                        crate::game_logic::host_move_log::record(
                            new_id,
                            Some([spawn_pos.x, spawn_pos.y, spawn_pos.z]),
                        );
                        // Factory exit residual still needs host pose for same-frame doors.
                        unit.set_position(spawn_pos);
                        unit.record_host_movement();
                    } else {
                        unit.set_position(spawn_pos);
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
                        unit.record_host_shock_stun();
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
                    let f = prod.thing.get_direction_vector();
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
                self.follow_exit_production_path(new_id, &exit_path, producer_id);
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
            n = n.saturating_add(1);
        }
        n
    }

    /// C++ GameLogic starting-unit residual (PlayerTemplate StartingUnit0..N).
    /// Spawns each active skirmish/SP player's starting construction unit near their
    /// base if they do not already own a matching mobile builder.
    pub(crate) fn spawn_skirmish_starting_units(&mut self) {
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
                    if self.create_object_for_player(building, pid, pos).is_some() {
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

            let Some(base_pos0) = base.or_else(|| self.player_base_position(pid)) else {
                continue;
            };
            let mut base_pos = base_pos0;
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
                if let Some(id) = self.create_object_for_player(unit_name, pid, unit_pos) {
                    gamelogic::player::notify_skirmish_starting_object(pid, unit_name, false);
                    log::info!(
                        "Wave 832: starting unit player={} team={:?} spawned {} id={:?}",
                        pid,
                        player.team,
                        unit_name,
                        id
                    );
                } else if i == 0 && !exact_player_template {
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
                        if let Some(id) = self.create_object_for_player(fallback, pid, unit_pos) {
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
