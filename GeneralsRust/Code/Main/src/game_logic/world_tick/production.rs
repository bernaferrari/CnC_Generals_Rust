//! Host tick `impl GameLogic` — `production`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// Update construction progress.
    /// C++ parity: buildings only progress when a worker/dozer is nearby.
    /// Multiple dozers stack their build rate (C++ BuildAssistant).
    pub(in super::super) fn update_construction(&mut self, object_ids: &[ObjectId], dt: f32) {
        const BUILDER_RANGE: f32 = 30.0; // Max distance for a dozer to contribute.

        // C++ parity: calcTimeToBuild applies the same power penalty to dozer
        // construction as to production queue speed.
        let team_power_factor = self.compute_team_power_factors();

        // Pre-scan all dozer positions/teams so we don't borrow-conflict.
        let dozer_info: Vec<(Vec3, Team)> = self
            .objects
            .values()
            .filter(|obj| obj.is_alive() && obj.can_construct())
            .map(|obj| (obj.get_position(), obj.team))
            .collect();

        let mut completed_superweapon_detects: Vec<(Team, String)> = Vec::new();
        let mut completed_structures: Vec<ObjectId> = Vec::new();
        let mut ready_superweapons: Vec<(ObjectId, Team, String)> = Vec::new();
        let mut radar_extend_done: Vec<ObjectId> = Vec::new();
        // Wave 617: under sole-tick, GameWorld writeback records ready structures;
        // host applies completion after writeback same frame (Wave 715; not mid-update drain).
        let construction_sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
        // Empty mid-update ready set: sole completes only via post-writeback helper (Wave 715).
        // Non-sole completes via projected percent (may_complete=true).
        let ready_structures: std::collections::HashSet<ObjectId> =
            std::collections::HashSet::new();
        for &id in object_ids {
            if let Some(obj) = self.objects.get_mut(&id) {
                if obj.status.under_construction {
                    let build_pos = obj.get_position();
                    let build_team = obj.team;
                    // True nearby dozer count (0 allowed) for model-condition residual.
                    let nearby_dozers = dozer_info
                        .iter()
                        .filter(|(pos, t)| {
                            *t == build_team && pos.distance(build_pos) <= BUILDER_RANGE
                        })
                        .count();
                    let dozer_count = nearby_dozers.max(1); // At least 1 so AI-built structures still progress.
                    let actively_built = nearby_dozers > 0;
                    obj.set_under_construction_model_conditions(actively_built);
                    self.construction_model_condition_updates =
                        self.construction_model_condition_updates.saturating_add(1);

                    let power_factor = team_power_factor.get(&build_team).copied().unwrap_or(1.0);
                    let base_rate = 1.0 / obj.thing.template.build_time.max(0.01);
                    let effective_rate = base_rate * dozer_count as f32 * power_factor;
                    // Under CONSTRUCTION_AUTHORITY + shadow, GameWorld sole-ticks percent
                    // using effective_rate; host only completes when writeback hits 1.0
                    // (Wave 617: readiness gated by host_construction_ready_log).
                    // Prior freeze without rate residual stalled builds — rate is logged.
                    let sole = crate::gameworld_shadow::gameworld_construction_sole_tick_enabled();
                    let projected = if sole {
                        // Last writeback percent (GW sole-ticks); host does not advance.
                        obj.construction_percent
                    } else {
                        (obj.construction_percent + effective_rate * dt).min(1.0)
                    };
                    if !sole {
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

                    // Wave 617/713: under sole-tick, only complete ready-log IDs.
                    // Empty ready log ⇒ no host percent-complete scan (GW readiness authority).
                    let may_complete = if construction_sole {
                        ready_structures.contains(&id)
                    } else {
                        true
                    };
                    if may_complete && projected >= 1.0 {
                        obj.construction_percent = 1.0;
                        obj.set_status_under_construction(false);
                        obj.clear_under_construction_model_conditions();
                        let full_hp = obj.health.maximum;
                        if crate::gameworld_shadow::gameworld_damage_authority_live() {
                            // HP last-writer via heal channel + writeback.
                            crate::game_logic::host_heal_log::record(id, full_hp);
                        } else {
                            obj.health.current = full_hp;
                            crate::game_logic::host_heal_log::record(id, obj.health.current);
                        }
                        crate::game_logic::host_construction_progress_log::record(
                            id, 1.0, false, 0.0,
                        );
                        crate::game_logic::host_construction_log::record(
                            id,
                            obj.template_name.clone(),
                        );
                        // C++ onStructureConstructionComplete SuperweaponDetected residual.
                        completed_superweapon_detects.push((obj.team, obj.template_name.clone()));
                        completed_structures.push(id);
                    } else {
                        let build_hp = obj.health.maximum * (0.1 + 0.9 * projected);
                        if crate::gameworld_shadow::gameworld_damage_authority_live() {
                            crate::game_logic::host_heal_log::record(id, build_hp);
                        } else {
                            obj.health.current = build_hp;
                            crate::game_logic::host_heal_log::record(id, obj.health.current);
                        }
                    }
                }
                if obj.tick_timers(dt) {
                    let team = obj.team;
                    let name = obj.template_name.clone();
                    // Defer EVA until after borrow ends.
                    ready_superweapons.push((id, team, name));
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
                // Wave 743: under production sole-tick, GameWorld owns door phase
                // advance + writeback; host must not dual-tick door residual.
                if !crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
                    let _ = obj.tick_production_door(self.frame);
                }
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
        // C++ Player sharedNSync timers advance with the logic frame.
        self.tick_shared_special_power_timers(dt);

        for (id, team, name) in ready_superweapons {
            self.try_eva_superweapon_ready(id, team, &name);
        }
        // Wave 618: under sole-tick, GameWorld writeback records SP ready flips;
        // Wave 717: host EVA drain runs after writeback same frame (not mid-update).

        for _id in radar_extend_done {
            self.radar_extend_completes = self.radar_extend_completes.saturating_add(1);
        }

        for (team, name) in completed_superweapon_detects {
            self.try_eva_superweapon_detected(team, &name);
        }

        // C++ parity: when a structure finishes construction, release any dozers
        // that were constructing it — set them to Idle.
        for &completed_id in &completed_structures {
            // C++ SupplyCenterCreate::onBuildComplete residual.
            self.on_supply_center_build_complete(completed_id);
            for obj in self.objects.values_mut() {
                if obj.ai_state == AIState::Constructing
                    && obj.target == Some(completed_id)
                    && obj.is_alive()
                {
                    let oid = obj.id;
                    obj.set_target(None);
                    obj.stop_moving();
                    // Collect for decision-aware Idle after borrow ends.
                    // (set below via second pass if needed — apply inline with free log)
                    obj.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(oid, 0);
                    }
                }
            }
            if let Some(team) = self.objects.get(&completed_id).map(|o| o.team) {
                self.record_structure_completion(team);
            }
            // C++ onStructureConstructionComplete feedback residual.
            self.notify_structure_construction_complete(completed_id);
            // C++ RadarUpgrade/RadarUpdate extendRadar residual on radar providers.
            self.maybe_start_radar_extend(completed_id);
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

    /// Wave 715: after GW construction writeback records ready structures, host
    /// applies completion side effects in the same coupled tick (not next frame).

    /// Wave 717: after GW special-power writeback records ready flips, host
    /// applies EVA superweapon-ready residual in the same coupled tick.
    pub(crate) fn host_apply_special_power_ready_after_writeback(&mut self) {
        if !crate::gameworld_shadow::gameworld_special_power_sole_tick_enabled() {
            return;
        }
        for ev in crate::game_logic::host_special_power_ready_log::drain() {
            if let Some(obj) = self.objects.get(&ev.object) {
                if obj.is_alive() {
                    let team = obj.team;
                    let name = obj.template_name.clone();
                    self.try_eva_superweapon_ready(ev.object, team, &name);
                }
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
        let mut completed_superweapon_detects: Vec<(Team, String)> = Vec::new();
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
            let full_hp = obj.health.maximum;
            if crate::gameworld_shadow::gameworld_damage_authority_live() {
                crate::game_logic::host_heal_log::record(id, full_hp);
            } else {
                obj.health.current = full_hp;
                crate::game_logic::host_heal_log::record(id, obj.health.current);
            }
            crate::game_logic::host_construction_progress_log::record(id, 1.0, false, 0.0);
            crate::game_logic::host_construction_log::record(id, obj.template_name.clone());
            completed_superweapon_detects.push((obj.team, obj.template_name.clone()));
            completed_structures.push(id);
        }
        for (team, name) in completed_superweapon_detects {
            self.try_eva_superweapon_detected(team, &name);
        }
        for &completed_id in &completed_structures {
            self.on_supply_center_build_complete(completed_id);
            for obj in self.objects.values_mut() {
                if obj.ai_state == AIState::Constructing
                    && obj.target == Some(completed_id)
                    && obj.is_alive()
                {
                    let oid = obj.id;
                    obj.set_target(None);
                    obj.stop_moving();
                    obj.set_ai_state(AIState::Idle);
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_set_state(oid, 0);
                    }
                }
            }
            if let Some(team) = self.objects.get(&completed_id).map(|o| o.team) {
                self.record_structure_completion(team);
            }
            self.notify_structure_construction_complete(completed_id);
            self.maybe_start_radar_extend(completed_id);
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

    pub(in super::super) fn update_production(&mut self, dt: f32) {
        // Wave 613: production complete collect + apply via host helpers.
        // Under PRODUCTION_AUTHORITY sole-tick, GameWorld advances queue progress
        // and writeback finishes heads; host try_complete + spawn runs after
        // shadow writeback same frame (Wave 714) so ready-log is not a frame late.
        // Wave 875: sole-tick early-return honesty — no host dual-advance.
        if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
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

        // C++ parity: pre-compute per-team power factor so we don't borrow
        // self.players while self.objects is mutably borrowed.
        // Formula matches ThingTemplate::calcTimeToBuild():
        //   energy_ratio = produced / max(consumed, produced) clamped to [0,1]
        //   energy_short = (1.0 - ratio) * penalty_modifier
        //   rate = max(1.0 - energy_short, 0.5)
        //   if ratio < 1.0: rate = min(rate, 0.8)
        let team_power_factor = self.compute_team_power_factors();

        use crate::game_logic::buildings::ProductionKind;
        // Unit completions: (team, template, spawn_pos, rally, producer_id)
        let mut unit_completions: Vec<(Team, String, Vec3, Option<Vec3>, ObjectId)> = Vec::new();
        // Upgrade completions: (team, upgrade_name, producer_id)
        let mut upgrade_completions: Vec<(Team, String, ObjectId)> = Vec::new();

        // Wave 614: under sole-tick, GameWorld writeback records ready producers;
        // host only try_completes those IDs (GW decides readiness).
        let sole = crate::gameworld_shadow::gameworld_production_sole_tick_enabled();
        // Wave 735: keep full ready events (template + GW spawn pose/rally), not
        // producer IDs alone — host sole-tick applies GW pose authority on spawn.
        let ready_by_producer: std::collections::HashMap<
            ObjectId,
            crate::game_logic::host_production_ready_log::HostProductionReadyEvent,
        > = if sole {
            crate::game_logic::host_production_ready_log::drain()
                .into_iter()
                .map(|ev| (ev.producer, ev))
                .collect()
        } else {
            std::collections::HashMap::new()
        };
        let ready_producers: std::collections::HashSet<ObjectId> =
            ready_by_producer.keys().copied().collect();

        for (&id, obj) in self.objects.iter_mut() {
            if !obj.is_constructed() || !obj.is_alive() {
                continue;
            }
            // C++ isDisabled residual: EMP / hacked / underpowered / unmanned
            // structures do not advance production while disabled.
            if obj.is_disabled() {
                continue;
            }
            if let Some(building) = obj.building_data.as_mut() {
                let pf = team_power_factor.get(&obj.team).copied().unwrap_or(1.0);
                // Under PRODUCTION_AUTHORITY, GameWorld ticks queue progress;
                // host only exits delay + completes when writeback already finished the head.
                let completed_prod = if sole {
                    // Wave 464/614: GameWorld sole-ticks progress + exit delay and
                    // records ready producers on writeback; host try_completes only
                    // ready IDs (Wave 713: empty ready log ⇒ no host scan).
                    if ready_producers.contains(&id) {
                        building.try_complete_production()
                    } else {
                        None
                    }
                } else {
                    building.update_production(dt, pf)
                };
                // GameWorld production residual: snapshot queue progress each tick
                // unless sole-tick owns progress (Wave 477) — then enqueue/complete logs
                // + writeback carry structure; GW advances progress/exit delay.
                if !building.production_queue.is_empty()
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
                                    cost_supplies: it.cost.supplies,
                                    is_upgrade: it.is_upgrade(),
                                    quantity_total: it.quantity_total.max(1),
                                    quantity_produced: it.quantity_produced,
                                }
                            })
                            .collect();
                    crate::game_logic::host_production_progress_log::record(
                        id,
                        items,
                        building.exit_delay_remaining,
                        pf,
                    );
                } else if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
                    // Wave 477: still publish power factor for GW sole-tick rate without
                    // stomping queue progress via full progress-log apply.
                    crate::game_logic::host_production_progress_log::record_power_factor_only(
                        id, pf,
                    );
                }
                if let Some((completed, kind)) = completed_prod {
                    match kind {
                        ProductionKind::Upgrade => {
                            upgrade_completions.push((obj.team, completed, id));
                        }
                        ProductionKind::Unit => {
                            let mut rally = building.rally_point;
                            // Spawn slightly offset from the building facing to reduce clumping.
                            let forward = obj.thing.get_direction_vector();
                            let base =
                                obj.get_position() + forward * obj.selection_radius.max(10.0);
                            // Deterministic jitter based on template bytes (simple FNV-1a).
                            let mut hash: u32 = 0x811c9dc5;
                            for &b in completed.as_bytes() {
                                hash ^= b as u32;
                                hash = hash.wrapping_mul(0x01000193);
                            }
                            let angle = (hash as f32) * 0.001;
                            let radius = 3.0 + (hash as f32 % 5.0);
                            let jitter = Vec3::new(angle.cos(), 0.0, angle.sin()) * radius;
                            let mut spawn_pos = base + jitter;
                            // C++ UnitCreatePoint residual sample for China barracks family.
                            let pname = obj.template_name.to_ascii_lowercase();
                            if pname.contains("chinabarracks")
                                || (pname.contains("barracks") && pname.contains("china"))
                            {
                                spawn_pos = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                                    obj.get_position(),
                                    forward,
                                    crate::game_logic::host_production_buildable_command_residual::CHINA_BARRACKS_UNIT_CREATE_MODEL,
                                );
                            }
                            // Wave 735: under sole-tick, GameWorld ready-log pose/rally
                            // and template are authoritative for the completion spawn.
                            // Wave 736: queue GW pre-spawned entity bind for host ObjectId.
                            let mut completed_name = completed;
                            if sole {
                                if let Some(ev) = ready_by_producer.get(&id) {
                                    if !ev.template_name.is_empty() {
                                        completed_name = ev.template_name.clone();
                                    }
                                    if let Some(p) = ev.spawn_pos {
                                        spawn_pos = Vec3::new(p[0], p[1], p[2]);
                                    }
                                    if let Some(r) = ev.rally {
                                        rally = Some(Vec3::new(r[0], r[1], r[2]));
                                    }
                                    if let Some(raw) = ev.gw_entity_raw {
                                        crate::game_logic::host_production_ready_log::push_pending_bind(
                                            raw,
                                        );
                                    }
                                }
                            }
                            unit_completions.push((obj.team, completed_name, spawn_pos, rally, id));
                        }
                    }
                }
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
            // Door + construction-complete flash residual on producer.
            if let Some(prod) = self.objects.get_mut(&producer_id) {
                let now = self.frame.max(1);
                prod.set_construction_complete_condition_at(now);
                prod.start_production_door_cycle(self.frame);
                self.production_door_cycles = self.production_door_cycles.saturating_add(1);
            }
            // Wave 483: refresh GW producer queue after host pop (sole-tick skips
            // per-frame progress log; Complete path snapshots host queue).
            crate::game_logic::host_production_log::record_complete(
                producer_id,
                upgrade_name.clone(),
                ObjectId(0),
            );
            // Unlock via player queue drain + host apply path.
            let player_id = self.players.values().find(|p| p.team == team).map(|p| p.id);
            if let Some(pid) = player_id {
                let already = self
                    .players
                    .get(&pid)
                    .map(|p| p.has_unlocked_upgrade(&upgrade_name))
                    .unwrap_or(false);
                if let Some(player) = self.players.get_mut(&pid) {
                    // Remove from queued set without refund (research finished).
                    if let Some(queued) = player.find_queued_upgrade_name(&upgrade_name) {
                        player.queued_upgrades.remove(&queued);
                    }
                    if !player.has_unlocked_upgrade(&upgrade_name) {
                        player.unlocked_sciences.insert(upgrade_name.clone());
                    }
                }
                if !already {
                    self.apply_host_upgrade_complete(team, pid, &upgrade_name);
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
                    let spawned = self.create_object(template, team, spawn_pos);
                    let after = self.next_object_id.0;
                    self.next_object_id = ObjectId(saved_next.0.max(after));
                    if spawned.is_some() {
                        return spawned;
                    }
                    self.next_object_id = saved_next;
                }
                // Bind present: allocate host id and map to pre-spawned entity.
                return self.create_object(template, team, spawn_pos);
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
        self.create_object(template, team, spawn_pos)
    }

    pub(in super::super) fn host_spawn_production_unit(
        &mut self,
        template: &str,
        team: Team,
        spawn_pos: Vec3,
    ) -> Option<ObjectId> {
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
                    let spawned = self.create_object(template, team, spawn_pos);
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
                return self.create_object(template, team, spawn_pos);
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
        self.create_object(template, team, spawn_pos)
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
            // Wave 615: production unit spawn via host helper (still host ID authority).
            let new_id =
                match self.apply_production_authority_op(ProductionAuthorityOp::SpawnUnit {
                    template: template.clone(),
                    team,
                    spawn_pos,
                }) {
                    ProductionAuthorityResult::Spawned(id) => id,
                    _ => None,
                };
            if let Some(new_id) = new_id {
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

    /// Wave 679: drain production-spawn ready log and apply host presentation residual
    /// (notify/door/exit/path) for the newly allocated host ObjectId.
    /// Still host ObjectId authority — not full GameWorld spawn-ID ownership.
    pub fn host_apply_production_spawn_ready_completions(&mut self) -> usize {
        // Wave 679: drain production-spawn ready log and apply host presentation residual.
        let events = crate::game_logic::host_production_spawn_ready_log::drain();
        let mut n = 0usize;
        for ev in events {
            let new_id = ev.unit;
            let producer_id = ev.producer;
            let template = ev.template;
            let mut spawn_pos = Vec3::new(ev.spawn_pos[0], ev.spawn_pos[1], ev.spawn_pos[2]);
            let rally = ev.rally.map(|r| Vec3::new(r[0], r[1], r[2]));
            // Wave 739: under production sole-tick, GameWorld ready-log pose is
            // authoritative — do not re-jitter/reposition the unit here (host
            // create_object already placed at GW exit pose). Non-sole path keeps
            // host stacking jitter residual.
            let sole = crate::gameworld_shadow::gameworld_production_sole_tick_enabled();
            let jitter_dir = if sole {
                Vec3::ZERO
            } else {
                Vec3::new(
                    (spawn_pos.x * 17.0 + spawn_pos.z).sin(),
                    0.0,
                    (spawn_pos.z * 31.0 + spawn_pos.x).cos(),
                )
                .normalize_or_zero()
            };
            // C++ VoiceCreated + UnitReady residual.
            self.notify_unit_production_complete(new_id, producer_id, &template);
            // C++ ProductionUpdate door + CONSTRUCTION_COMPLETE residual on producer.
            if let Some(prod) = self.objects.get_mut(&producer_id) {
                let now = self.frame.max(1);
                prod.set_construction_complete_condition_at(now);
                prod.start_production_door_cycle(self.frame);
                self.production_door_cycles = self.production_door_cycles.saturating_add(1);
                // C++ QueueProductionExitUpdate ExitDelay residual after release.
                if let Some(building) = prod.building_data.as_mut() {
                    let delay = crate::game_logic::host_dock_contain_exit_heal_residual::queue_exit_delay_seconds_for_template(
                        &prod.template_name,
                    );
                    building.arm_exit_delay(delay);
                    // Wave 480: under sole-tick, progress log is power-only —
                    // publish exit arm so GW QueueProductionExitUpdate advances.
                    if crate::gameworld_shadow::gameworld_production_sole_tick_enabled() {
                        crate::game_logic::host_production_progress_log::record_exit_delay_only(
                            producer_id,
                            delay,
                        );
                    }
                }
            }
            // SCIENCE_StealthFighter residual: record gated production spawn.
            if crate::game_logic::host_stealth_fighter::requires_stealth_fighter_science(&template)
            {
                self.stealth_fighter_science.record_production_spawn();
            }
            // Wave 739: sole-tick keeps create_object/GW exit pose; non-sole
            // applies host stacking jitter + factory exit pose residual.
            if !sole {
                if let Some(unit) = self.objects.get(&new_id) {
                    let selection_radius = unit.selection_radius.max(4.0);
                    spawn_pos += jitter_dir * selection_radius;
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
            }
            // C++ QueueProductionExitUpdate exit path residual:
            // natural rally first; custom rally appended; else double natural
            // so Red Guards do not stack on the door.
            let (natural, forward) = if let Some(prod) = self.objects.get(&producer_id) {
                let f = prod.thing.get_direction_vector();
                let natural = crate::game_logic::host_production_buildable_command_residual::transform_model_exit_offset(
                    prod.get_position(),
                    f,
                    crate::game_logic::host_production_buildable_command_residual::CHINA_BARRACKS_NATURAL_RALLY_MODEL,
                );
                // Generic residual: if producer is not China barracks family, fall back
                // to forward * selection_radius natural.
                let p_name = prod.template_name.to_ascii_lowercase();
                let natural = if p_name.contains("chinabarracks")
                    || (p_name.contains("barracks") && p_name.contains("china"))
                {
                    natural
                } else {
                    prod.get_position() + f * prod.selection_radius.max(10.0)
                };
                (natural, f)
            } else {
                (spawn_pos, glam::Vec3::new(0.0, 0.0, -1.0))
            };
            self.path_approach_with_state(new_id, natural, AIState::Moving);
            if let Some(rally_point) = rally {
                let _ = self.append_unit_waypoint(new_id, rally_point);
            } else {
                // Double natural residual (C++ exitPath.push_back(tmp) twice).
                let doubled = natural + forward.normalize_or_zero() * 5.0;
                let _ = self.append_unit_waypoint(new_id, doubled);
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

            // --- Starting building (C++ placeStartingStructures) ---
            let mut base = self.team_base_position(player.team);
            if base.is_none() {
                // Wave 831/832: place at Player_N_Start when map has no faction army.
                let building = residual.starting_building;
                let mut pos_opt: Option<Vec3> = None;
                if !building.is_empty() {
                    if let Ok(starts) =
                        super::super::script_loader::parse_player_start_waypoints(&self.map_name)
                    {
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
                    if self.create_object(building, player.team, pos).is_some() {
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

            let Some(base_pos0) = base.or_else(|| self.team_base_position(player.team)) else {
                continue;
            };
            let mut base_pos = base_pos0;
            if let Some(h) = self.terrain_height_at(Vec3::new(base_pos.x, 0.0, base_pos.z)) {
                base_pos.y = h;
            }

            // --- Starting units 0..9 (C++ placeStartingUnits / MAX_MP_STARTING_UNITS) ---
            // Wave 832: walk residual.starting_units; retail usually only unit0 (dozer).
            let unit_names: Vec<&str> = residual
                .starting_units
                .iter()
                .copied()
                .filter(|n| !n.is_empty())
                .collect();
            if unit_names.is_empty() {
                continue;
            }
            self.ensure_ai_faction_templates(player.team);
            for (i, unit_name) in unit_names.iter().enumerate() {
                // Skip if this exact starting unit template already exists for the team.
                let already = self.objects.values().any(|o| {
                    o.team == player.team
                        && o.is_alive()
                        && o.template_name.eq_ignore_ascii_case(unit_name)
                });
                // For builders/workers: also treat any mobile constructor as present.
                let is_builder = unit_name.to_ascii_lowercase().contains("dozer")
                    || unit_name.to_ascii_lowercase().contains("worker");
                let has_builder = is_builder
                    && self.objects.values().any(|o| {
                        o.team == player.team
                            && o.is_alive()
                            && o.is_mobile()
                            && (o.can_construct()
                                || o.template_name.to_ascii_lowercase().contains("dozer")
                                || o.template_name.to_ascii_lowercase().contains("worker"))
                    });
                if already || has_builder {
                    continue;
                }

                // Offset around yard like C++ minRadius/maxRadius residual.
                let mut unit_pos =
                    base_pos + Vec3::new(40.0 + (i as f32) * 12.0, 0.0, -40.0 - (i as f32) * 6.0);
                if let Some(h) = self.terrain_height_at(Vec3::new(unit_pos.x, 0.0, unit_pos.z)) {
                    unit_pos.y = h;
                }
                if let Some(id) = self.create_object(unit_name, player.team, unit_pos) {
                    log::info!(
                        "Wave 832: starting unit player={} team={:?} spawned {} id={:?}",
                        pid,
                        player.team,
                        unit_name,
                        id
                    );
                } else if i == 0 {
                    // Fallback retail short names for unit0 only.
                    let fallback = match player.team {
                        Team::USA => "AmericaVehicleDozer",
                        Team::China => "ChinaVehicleDozer",
                        Team::GLA => "GLAInfantryWorker",
                        Team::Neutral => "",
                    };
                    if !fallback.is_empty() {
                        if let Some(id) = self.create_object(fallback, player.team, unit_pos) {
                            log::info!(
                                "Wave 832: starting unit fallback player={} {} id={:?}",
                                pid,
                                fallback,
                                id
                            );
                        }
                    }
                }
            }
        }
    }
}
