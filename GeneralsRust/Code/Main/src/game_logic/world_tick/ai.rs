//! Host tick `impl GameLogic` — `ai`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    pub(in super::super) fn update_ai(&mut self, object_ids: &[ObjectId], dt: f32) {
        use crate::ai_decisions::*;
        self.flush_countermeasure_flare_spawns();
        // Wave 806: under coupled shadow, lifetime owned by GW expire + logs.
        {
            self.update_countermeasure_flare_objects();
        }

        self.propagate_hacked_to_spawn_slaves();

        let mut ai_commands = Vec::new();
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP; // Convert frame to seconds
        let game_phase = GamePhase::from_time(current_time);
        // Campaign maps place thousands of decorative props. Skip AI for
        // non-combat, non-structure objects so frame cost stays reasonable.
        let dense_world = object_ids.len() > 400;

        // BattlePlan pack/unpack door residual (AnimationTime 7000ms → 210 frames).
        self.tick_battle_plan_door_residuals();
        // C++ ActiveBody.cpp:574-581 Player::setAttackedBy for PLAYER_ATTACKED_BY.

        // First pass: Dispatch object AI through the existing state machine.
        for &object_id in object_ids {
            // Advance local update timers. DISABLED_HACKED/EMP expire in
            // the separate end-of-frame disabled-status phase.
            let mut topple_kill = false;
            let mut final_death_destroy = false;
            let mut lifetime_kill = false;
            let mut poison_kill = false;
            let mut pending_stump: Option<(String, glam::Vec3, f32, bool)> = None;
            let mut defector_audio: Vec<String> = Vec::new();
            let mut disguise_halfpoint: Option<(glam::Vec3, bool, bool)> = None;
            let mut missile_idle_audio: Option<(Option<String>, bool, Option<String>, glam::Vec3)> =
                None;
            let height_die_terrain = {
                let (pos, ground, name) = match self.objects.get(&object_id) {
                    Some(o) => (o.get_position(), o.ground_height, o.template_name.clone()),
                    None => (glam::Vec3::ZERO, 0.0, String::new()),
                };
                // C++ HeightDieUpdate.cpp:132-195 getGroundHeight + structures.
                self.height_die_terrain_at(pos, &name, ground)
            };
            if let Some(obj) = self.objects.get_mut(&object_id) {
                // CPP Drawable.cpp:1174-1204 advances opacity once per frame.
                obj.tick_terrain_decal_fade();
                let stump_pos = obj.get_position();
                let stump_ori = obj.get_orientation();
                if let Some(td) = obj.topple_data.as_mut() {
                    if let Some(name) = td.take_pending_stump_name() {
                        pending_stump = Some((name, stump_pos, stump_ori, td.burned_at_topple));
                    }
                }
                // Wave 761: under coupled GameWorld shadow, status timer expire
                // (faerie/repulsor/frenzy/continuous-fire/selection flash)
                // is owned by `tick_status_timer_expirations` + writeback. Host must
                // not dual-expire mid-frame. Eject-invulnerable stays host-only
                // (no GW until_frame field yet).
                let peel_status_timers = false;
                if !peel_status_timers {
                    obj.tick_selection_flash();
                    obj.tick_weapon_bonus_frenzy(self.frame);
                    obj.tick_faerie_fire(self.frame);
                }
                // Wave 762: under coupled shadow, eject-invulnerable expire is
                // owned by GW tick_status_timer_expirations + writeback.
                {
                    obj.tick_eject_invulnerable(self.frame);
                }
                // Wave 766: under coupled shadow, ObjectDefectionHelper timer is
                // owned by GW tick_status_timer_expirations + writeback.
                {
                    // C++ ObjectDefectionHelper::update residual.
                    obj.tick_defection_helper(self.frame);
                }
                // Snapshot FireWeaponPower residual before further mut uses.
                let fwp_shot = obj.fire_weapon_power.as_ref().and_then(|req| {
                    if req.shots_remaining == 0 {
                        None
                    } else if req.has_location {
                        Some((req.target_x, req.target_z, true))
                    } else {
                        let p = obj.get_position();
                        Some((p.x, p.z, false))
                    }
                });
                if let Some((tx, tz, _has_loc)) = fwp_shot {
                    obj.target_location = Some(glam::Vec3::new(tx, 0.0, tz));
                    obj.set_ai_state(crate::game_logic::AIState::Attacking);
                    if let Some(req) = obj.fire_weapon_power.as_mut() {
                        req.shots_remaining = req.shots_remaining.saturating_sub(1);
                        if req.shots_remaining == 0 {
                            obj.fire_weapon_power = None;
                        }
                    }
                }
                // Drain defector audio residual (collect then queue outside obj borrow).
                defector_audio = obj
                    .defection_helper
                    .as_mut()
                    .map(|d| d.drain_audio())
                    .unwrap_or_default();
                // CPP PoisonedBehavior owns its timer on this Object. Movement
                // authority only changes pose ownership; its diagnostic shadow
                // does not schedule or apply another poison pulse.
                let poison_damage = obj.tick_poisoned_behavior(self.frame);
                let _ = obj;
                if let Some((dot, death_ty)) = poison_damage {
                    poison_kill = self.apply_residual_damage_owned(
                        object_id,
                        dot,
                        None,
                        crate::game_logic::combat::DamageType::Unresistable,
                        death_ty,
                        Some(crate::game_logic::host_poisoned_behavior::poison_dot_fx_override()),
                    );
                }
                let Some(obj) = self.objects.get_mut(&object_id) else {
                    continue;
                };
                // Advance the receiving object's continuous damaged-weapon state.
                {
                    if obj.temporary_weapon_runtime.damaged.is_empty() {
                        if let Some(w) = obj.tick_fire_weapon_when_damaged_continuous(self.frame) {
                            if obj.pending_fire_when_damaged_weapon.is_none() {
                                obj.pending_fire_when_damaged_weapon = Some(w);
                            }
                        }
                    }
                }
                // CPP LifetimeUpdate::update calls Object::kill at this point,
                // then sleeps forever. End the module/Object loan before the
                // receiving Body dispatch; its immediate callbacks can reenter.
                let lifetime_due = obj.tick_lifetime_update(self.frame);
                let _ = obj;
                self.tick_native_lifecycle(object_id);
                if lifetime_due {
                    lifetime_kill = self
                        .apply_owned_kill(
                            object_id,
                            crate::game_logic::combat::DamageType::Unresistable,
                            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
                        )
                        .is_some_and(|result| result.destroyed());
                }
                let Some(obj) = self.objects.get_mut(&object_id) else {
                    continue;
                };
                // C++ MissileLauncherBuildingUpdate::update (ready-frame door SM).
                let ml_pos = obj.get_position();
                let (play, stop) = obj.tick_missile_launcher_building(self.frame);
                if play.is_some() || stop {
                    let stop_name = obj
                        .missile_launcher_building
                        .as_ref()
                        .and_then(|d| d.ini.open_idle_audio.clone());
                    missile_idle_audio = Some((play, stop, stop_name, ml_pos));
                }
                // Wave 761: continuous-fire coast + repulsor expire peel under coupled.
                // Wave 761: CF coast + repulsor peel under coupled.
                // Wave 765: subdual heal owned by GW when coupled.
                // Wave 767: fire-sound loop stop owned by GW tick_status_timer_expirations
                // when coupled (non-coupled still via tick_continuous_fire_coast).
                {
                    obj.tick_continuous_fire_coast(
                        self.frame,
                        &mut self.combat_system.fire_sound_loop_events,
                        &mut self.health_events,
                    );
                    obj.tick_repulsor_status(self.frame);
                }
                // Wave 763: under coupled shadow, force-reload-when-idle is owned by
                // GW tick_status_timer_expirations + weapon_stats/continuous-fire writeback.
                {
                    obj.tick_force_reload_when_idle(self.frame);
                }
                // C++ ObjectWeaponStatusHelper every-frame getStatus:
                // refill auto-reload clips and flip READY when ClipReloadTime elapses.
                if [0u8, 1, 2]
                    .iter()
                    .any(|&slot| obj.weapon_slot(slot).is_some())
                {
                    obj.refresh_weapon_fire_status(current_time);
                }
                obj.tick_spy_vision_disabled(self.frame);
                if obj.tick_disguise_transition() {
                    self.bomb_truck_disguise.record_transition_halfpoint();
                    disguise_halfpoint = Some((
                        obj.get_position(),
                        obj.status.disguise_transitioning_to,
                        obj.target.is_some(),
                    ));
                }
                // Advance the receiving object's topple state before height death.
                {
                    topple_kill = obj.tick_topple();
                    let stump_pos = obj.get_position();
                    let stump_ori = obj.get_orientation();
                    if let Some(td) = obj.topple_data.as_mut() {
                        if let Some(name) = td.take_pending_stump_name() {
                            pending_stump = Some((name, stump_pos, stump_ori, td.burned_at_topple));
                        }
                    }
                }
                // C++ StructureToppleUpdate::update residual (buildings).
                // C++ HeightDieUpdate residual (bombs/missiles).
                // Apply the receiving object's height-death result.
                {
                    if !topple_kill && obj.tick_height_die(self.frame, height_die_terrain) {
                        topple_kill = true;
                    }
                }
                // Wave 772: under coupled shadow, JetSlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + its owned Jet completion drain.
                {
                    // C++ JetSlowDeathBehavior residual.
                    if !topple_kill
                        && obj
                            .jet_slow_death
                            .as_ref()
                            .map(|j| j.is_active())
                            .unwrap_or(false)
                    {
                        topple_kill = obj.tick_jet_slow_death(self.frame, 0.0);
                        final_death_destroy = topple_kill;
                    }
                }
                // Wave 773: under coupled shadow, HelicopterSlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + its owned Helicopter completion drain.
                {
                    // C++ HelicopterSlowDeathBehavior residual.
                    if !topple_kill
                        && obj
                            .helicopter_slow_death
                            .as_ref()
                            .map(|h| h.is_active())
                            .unwrap_or(false)
                    {
                        topple_kill = obj.tick_helicopter_slow_death(self.frame, 0.0);
                        final_death_destroy = topple_kill;
                    }
                }
                // Wave 774: under coupled shadow, SlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + its owned Slow completion drain.
                {
                    if !topple_kill
                        && obj
                            .slow_death
                            .as_ref()
                            .map(|s| s.is_active())
                            .unwrap_or(false)
                    {
                        topple_kill = obj.tick_slow_death(self.frame);
                        final_death_destroy = topple_kill;
                    }
                }
                // Wave 775: under coupled shadow, StructureCollapseUpdate is owned by
                // Optional movement authority advances the animation; DONE never deletes.
                {
                    if !topple_kill && obj.structure_collapse_data.is_some() {
                        let _completed = obj.tick_structure_collapse(self.frame);
                    }
                }
                // StructureTopple lends this Object only for bounded phases;
                // its original FX/crushing callbacks run on the owner below.
            }
            if let Some((name, pos, ori, burned)) = pending_stump {
                self.spawn_topple_stump(&name, pos, ori, burned);
            }
            if let Some(obj) = self.objects.get_mut(&object_id) {
                obj.poll_slow_death_phase_fx(self.frame);
                obj.poll_structure_collapse_phase_fx(self.frame);
            }
            self.sync_helicopter_attach_particle(object_id);
            if !topple_kill {
                if let Err(error) = self.update_owned_structure_topple(object_id) {
                    log::error!("Owned StructureTopple update rejected: {error}");
                }
            } else {
            }
            // C++ FireWeaponWhenDamagedBehavior forceFire (reaction + continuous).
            let has_live_damaged = self
                .objects
                .get(&object_id)
                .is_some_and(|o| !o.temporary_weapon_runtime.damaged.is_empty());
            if has_live_damaged {
                let _ = self
                    .objects
                    .get_mut(&object_id)
                    .and_then(|o| o.take_pending_fire_when_damaged_weapon());
                let damage_events: Vec<_> = self
                    .health_events
                    .snapshot_damage()
                    .into_iter()
                    .filter(|event| event.target == object_id)
                    .collect();
                for event in damage_events {
                    let _ = self.execute_temporary_weapon_on_damage(
                        object_id,
                        event.amount,
                        event.damage_type_ordinal,
                    );
                }
                let _ = self.execute_temporary_weapon_continuous(object_id);
            } else if let Some(wname) = self
                .objects
                .get_mut(&object_id)
                .and_then(|o| o.take_pending_fire_when_damaged_weapon())
            {
                let _ = self.apply_fire_weapon_when_damaged_named(object_id, &wname);
            }
            // C++ TransitionDamageFX + FXListDie → FXList::doFXObj / doFXPos.
            {
                let (pos, yaw, model, scale) = self
                    .objects
                    .get(&object_id)
                    .map(|o| {
                        (
                            o.get_position(),
                            o.get_orientation(),
                            o.thing()
                                .template
                                .model_name_for_conditions(o.model_condition_bits),
                            o.thing().template.asset_scale,
                        )
                    })
                    .unwrap_or((glam::Vec3::ZERO, 0.0, String::new(), 1.0));
                let (transition_evs, death_fx, death_audio, death_audio_stop, death_killer) =
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        let te = o.take_pending_transition_damage_fx();
                        let (df, da) = o.take_pending_death_fx_audio();
                        let stop = o.take_pending_death_audio_stop();
                        let killer = o.last_damage_source;
                        (te, df, da, stop, killer)
                    } else {
                        (Vec::new(), None, None, false, None)
                    };
                for ev in transition_evs {
                    if let Some(a) = ev.audio_name {
                        self.queue_audio_event(
                            AudioEventRequest::new(&a)
                                .with_object(object_id)
                                .with_position(pos)
                                .with_priority(140),
                        );
                    }
                    if let Some(fx) = ev.fx_name {
                        if !self.dispatch_fx_list_at_host_object(&fx, object_id, None) {
                            for sound in crate::game_logic::sound_names_for_fx_list(&fx) {
                                self.queue_audio_event(
                                    AudioEventRequest::new(&sound)
                                        .with_object(object_id)
                                        .with_position(pos)
                                        .with_priority(130),
                                );
                            }
                        }
                    }
                    for fx in ev.extra_fx_names {
                        let _ = self.dispatch_fx_list_at_host_object(&fx, object_id, None);
                    }
                    for ocl in ev.ocl_names {
                        crate::game_logic::host_transition_damage_fx::play_authored_transition_ocl(
                            &ocl,
                            object_id.0,
                            pos,
                        );
                    }
                    if let Some(old) = ev.clear_old_state {
                        if let Some(cfg) = self
                            .objects
                            .get_mut(&object_id)
                            .and_then(|o| o.transition_damage_fx.as_mut())
                        {
                            for id in cfg.take_attached_ids(old) {
                                self.combat_particles.deactivate(id);
                            }
                        }
                    }
                    if !ev.particles.is_empty() {
                        let ids = crate::game_logic::host_transition_damage_fx::spawn_transition_particles_at_pose(
                            &mut self.combat_particles,
                            &ev.particles,
                            pos,
                            yaw,
                            &model,
                            scale,
                            self.frame,
                            object_id,
                        );
                        if let Some(cfg) = self
                            .objects
                            .get_mut(&object_id)
                            .and_then(|o| o.transition_damage_fx.as_mut())
                        {
                            cfg.store_attached_ids(ev.new_state, ids);
                        }
                    }
                }
                if let Some(a) = death_audio {
                    let mut req = AudioEventRequest::new(&a)
                        .with_object(object_id)
                        .with_position(pos)
                        .with_priority(200);
                    if death_audio_stop {
                        req = req.stopping();
                    } else if a.contains("DamagedLoop") || a.contains("DeathLoop") {
                        req = req.looping();
                    }
                    self.queue_audio_event(req);
                }
                if let Some(fx) = death_fx {
                    if !self.dispatch_fx_list_at_host_object(&fx, object_id, death_killer) {
                        let mut sounds = crate::game_logic::sound_names_for_fx_list(&fx);
                        if sounds.is_empty() {
                            sounds = heli_fx_fallback_sounds(&fx);
                        }
                        for sound in sounds {
                            self.queue_audio_event(
                                AudioEventRequest::new(&sound)
                                    .with_object(object_id)
                                    .with_position(pos)
                                    .with_priority(190),
                            );
                        }
                    }
                }
            }
            // C++ BoneFXUpdate::update — leftover authored FXList/OCL/PSys at bone pos.
            {
                let pose = self.objects.get(&object_id).map(|o| {
                    (
                        o.get_position(),
                        o.get_orientation(),
                        o.thing()
                            .template
                            .model_name_for_conditions(o.model_condition_bits),
                        o.thing().template.asset_scale,
                        o.drawable_hidden,
                    )
                });
                let events = if let Some(o) = self.objects.get_mut(&object_id) {
                    if o.bone_fx_damage.is_none() {
                        o.bone_fx_damage = crate::game_logic::host_bone_fx_damage::HostBoneFxDamageData::from_template(
                            &o.template_name,
                        );
                    }
                    if let Some(bfx) = o.bone_fx_damage.as_mut() {
                        bfx.stamp_last_damage_type(o.last_damage_info_type);
                        bfx.tick(self.frame as i32);
                        bfx.drain_pending()
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                };
                if let Some((origin, yaw, model, scale, hidden)) = pose {
                    for ev in events {
                        let leftover_id =
                            crate::game_logic::host_bone_fx_damage::play_bone_fx_event(
                                &ev,
                                object_id.0,
                                origin,
                                yaw,
                                &model,
                                scale,
                                hidden,
                            );
                        if let Some(id) = leftover_id {
                            if let Some(o) = self.objects.get_mut(&object_id) {
                                if let Some(bfx) = o.bone_fx_damage.as_mut() {
                                    bfx.track_particle(id);
                                }
                            }
                        }
                    }
                }
            }
            // C++ CreateObjectDie residual (spawn after death FX).
            self.apply_pending_create_object_die(object_id);
            if lifetime_kill {
                // The receiving kill driver already delivered onDie/retention.
                // Never turn a retained corpse into a second forced destroy.
                continue;
            }
            for a in defector_audio {
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(a.as_str())
                        .with_object(object_id)
                        .with_priority(80),
                );
            }
            if let Some((play, stop, stop_name, pos)) = missile_idle_audio {
                if let Some(name) = play {
                    if !name.is_empty() {
                        self.queue_audio_event(
                            AudioEventRequest::new(&name)
                                .with_object(object_id)
                                .with_position(pos)
                                .with_priority(160)
                                .looping(),
                        );
                    }
                }
                if stop {
                    if let Some(name) = stop_name.filter(|n| !n.is_empty()) {
                        self.queue_audio_event(
                            AudioEventRequest::new(&name)
                                .with_object(object_id)
                                .with_position(pos)
                                .with_priority(160)
                                .stopping(),
                        );
                    }
                }
            }
            if let Some((pos, gaining, has_victim)) = disguise_halfpoint {
                use crate::game_logic::host_bomb_truck_disguise::{
                    BOMB_TRUCK_DISGUISE_FX, BOMB_TRUCK_DISGUISE_REVEAL_FX,
                    BOMB_TRUCK_DISGUISE_REVEALED_FAILURE_AUDIO,
                    BOMB_TRUCK_DISGUISE_REVEALED_SUCCESS_AUDIO, BOMB_TRUCK_DISGUISE_STARTED_AUDIO,
                };
                let (sound, fx) = if gaining {
                    (BOMB_TRUCK_DISGUISE_STARTED_AUDIO, BOMB_TRUCK_DISGUISE_FX)
                } else if has_victim {
                    (
                        BOMB_TRUCK_DISGUISE_REVEALED_SUCCESS_AUDIO,
                        BOMB_TRUCK_DISGUISE_REVEAL_FX,
                    )
                } else {
                    (
                        BOMB_TRUCK_DISGUISE_REVEALED_FAILURE_AUDIO,
                        BOMB_TRUCK_DISGUISE_REVEAL_FX,
                    )
                };
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(sound)
                        .with_object(object_id)
                        .with_position(pos)
                        .with_priority(160),
                );
                if !crate::game_logic::dispatch_fx_list_at_pos(fx, pos) {
                    let _ = self.combat_particles.spawn_named(
                        crate::game_logic::combat_particles::CombatParticleKind::WeaponImpact,
                        fx,
                        pos,
                        self.frame,
                        Some(object_id),
                        None,
                    );
                }
            }
            if poison_kill {
                self.mark_object_for_destruction(object_id, None);
                continue;
            }
            if topple_kill {
                if final_death_destroy {
                    // CPP SlowDeath/JetSlowDeath/HelicopterSlowDeath finish with
                    // destroyObject; they must not re-enter the onDie latch.
                    self.destroy_object(object_id);
                } else {
                    // Topple and HeightDie trigger damage/kill. Preserve the
                    // existing structure completion policy separately.
                    self.mark_object_for_destruction(object_id, None);
                }
                continue;
            }
            // OCL_EjectPilotViaParachute residual sink (elevated pilot → ground).
            self.tick_eject_parachute_residual(object_id);
            // AmericaCrateParachute residual sink (cargo crate freefall → OpenDist → land).
            self.tick_crate_parachute_residual(object_id);
            // C++ AIRappelState: combat-drop rappel dest Z / kill-2 / addToContain.
            self.tick_rappel_into(object_id);
            // PilotFindVehicleUpdate residual: AI idle pilot auto-scan for
            // recrewable unmanned vehicles (ScanRate 1000ms / range 300 / MinHealth 0.5).
            // C++ human players sleep forever — host residual: is_local → skip.
            // Base-center fallback residual when no vehicle found (m_didMoveToBase).
            self.try_pilot_find_vehicle_residual(object_id);
            // AutoFindHealingUpdate residual: AI idle injured infantry → HealPad.
            // C++ AutoFindHealingUpdate.cpp:78-123 any template with the module.
            // ScanRate 1000ms / ScanRange 300 / NeverHeal 0.85 (AlwaysHeal busy path fail-closed).
            self.try_auto_find_healing_residual(object_id);
            // AutoFindRepair residual: AI idle damaged vehicles → RepairPad/WarFactory.
            self.try_auto_find_repair_residual(object_id);
            self.try_auto_resume_construction_residual(object_id);
            // C++ AIIdleState: CAN_BE_REPULSED idle units flee closest repulsor.
            let _ = self.try_idle_repulse(object_id);
            // C++ AIIdleState: checkForCrateToPickup → aiMoveToObject.
            let _ = self.try_idle_crate_pickup(object_id);
            // C++ AIHuntState parent machine: after AttackThenIdle kill, stay in hunt.
            if let Some(obj) = self.objects.get_mut(&object_id) {
                if obj.hunting
                    && matches!(obj.ai_state, AIState::Idle)
                    && obj.target.is_none()
                    && obj.is_alive()
                {
                    obj.set_ai_state(AIState::Patrolling);
                }
            }
            // C++ AIIdleState::onEnter → resetNextMoodCheckTime, before the scan.
            self.apply_pending_idle_mood_reset(object_id);
            // C++ AIIdleState::update: mood scan / attack only — never AI_HUNT wander.
            if let Some(obj) = self.objects.get(&object_id) {
                if obj.temporary_move_frames > 0 {
                    continue;
                }
                let can_attack = obj.can_attack();
                if dense_world
                    && !can_attack
                    && !obj.is_kind_of(KindOf::Structure)
                    && !obj.is_kind_of(KindOf::Worker)
                    && !obj.is_kind_of(KindOf::Infantry)
                    && !obj.is_kind_of(KindOf::Vehicle)
                    && !obj.is_kind_of(KindOf::Aircraft)
                    && obj.target.is_none()
                    && matches!(obj.ai_state, AIState::Idle)
                {
                    continue;
                }
                let position = obj.get_position();
                let team = obj.team;
                let ai_state = obj.ai_state.clone();
                let current_target = obj.target;
                if let Some(command) = self.process_ai_behavior(
                    object_id,
                    ai_state,
                    current_target,
                    position,
                    team,
                    can_attack,
                    self.frame,
                    dt,
                ) {
                    ai_commands.push(command);
                }
            }
        }

        // Second pass: Handle production buildings
        for &object_id in object_ids {
            let (team, is_production_building) = match self.objects.get(&object_id) {
                Some(obj)
                    if obj.is_kind_of(KindOf::Structure)
                        && obj.is_constructed()
                        && obj.is_alive() =>
                {
                    let is_production_building = obj.template_name.contains("Barracks")
                        || obj.template_name.contains("WarFactory")
                        || obj.template_name.contains("ArmsDealer");
                    (obj.team, is_production_building)
                }
                _ => continue,
            };

            if !is_production_building {
                continue;
            }

            // Find which player owns this building.
            let player_id = self
                .players
                .iter()
                .find_map(|(pid, player)| (player.team == team).then_some(*pid));

            let Some(pid) = player_id else {
                continue;
            };
            if self.players.get(&pid).is_some_and(|p| p.is_local) {
                continue;
            }

            // Check if should produce units (every 10 seconds).
            if !self.frame.is_multiple_of(600) {
                continue;
            }

            if let Some(unit_to_produce) =
                AIDecisionSystem::select_production_unit(self, team, game_phase, pid)
            {
                // Queue unit production (in a full implementation)
                log::trace!(
                    "AI Building {} queuing production of {}",
                    object_id,
                    unit_to_produce
                );

                self.enqueue_production(object_id, unit_to_produce);
            }
        }

        // GuardRetaliate victim-death / return residual.
        self.tick_guard_retaliate_states();
        // HijackerUpdate ride residual.
        self.tick_hijacker_updates();
        // C++ UndeadBody + BattleBusSlowDeathBehavior residual.
        self.tick_battle_bus_slow_deaths();
        // C++ ChinookAIUpdate idle auto-land / evac / combat-drop residual.
        self.tick_chinook_ai(1.0 / 30.0);

        // C++ AssaultTransportAIUpdate wounded-retrieve residual.
        self.tick_assault_transport_updates();
        // C++ DeployStyleAIUpdate pack/unpack residual.
        self.tick_deploy_style_updates();
        // C++ CommandButtonHuntUpdate residual.
        self.tick_command_button_hunt_updates();

        // Apply all AI commands (or log-only when GameWorld owns decision apply).

        for command in ai_commands {
            {
                self.apply_ai_command(command);
            }
        }

        // Resolve command-driven support states (guard/repair/docking/garrison) after AI decisions.
        // C++ SlavedUpdate.cpp:91-261 follow / guard / 2x GuardMaxRange recall.
        self.update_slaved_drone_follow(object_ids);
        self.sync_all_spawn_behavior_veterancy();
        // C++ DemoTrapUpdate.cpp:124-130 isEffectivelyDead + DetonateWhenKilled.
        self.update_demo_trap_detonate_when_killed();
        self.update_support_states(object_ids, dt);
    }

    /// C++ SlavedUpdate::update — drones follow/guard/recall their producer master.
    pub(in super::super) fn update_slaved_drone_follow(&mut self, object_ids: &[ObjectId]) {
        for &object in object_ids {
            self.update_owned_slaved_object(object);
        }
    }

    /// C++ DemoTrapUpdate.cpp:124-130 isEffectivelyDead + DetonateWhenKilled.
    pub(in super::super) fn update_demo_trap_detonate_when_killed(&mut self) {
        use crate::game_logic::host_mines::{
            DEMO_TRAP_DETONATE_WHEN_KILLED, HostMineDetonateReason, should_detonate_when_killed,
        };
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                let data = obj.mine_data.as_ref()?;
                if should_detonate_when_killed(
                    data.kind,
                    DEMO_TRAP_DETONATE_WHEN_KILLED,
                    !obj.is_alive(),
                    data.detonated,
                    obj.status.under_construction || obj.status.sold,
                ) {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for id in due {
            let _ = self.detonate_mine_internal(id, HostMineDetonateReason::Killed, None);
        }
    }

    /// C++ HelicopterSlowDeath beginSlowDeath attachParticle + follow object.
    fn sync_helicopter_attach_particle(&mut self, id: ObjectId) {
        let pos = self.objects.get(&id).map(|o| o.get_position());
        let Some(pos) = pos else {
            return;
        };
        let Some(mut h) = self
            .objects
            .get_mut(&id)
            .and_then(|o| o.helicopter_slow_death.take())
        else {
            return;
        };
        if h.pending_attach {
            h.spawn_attach_particle(&mut self.combat_particles, pos, self.frame, id);
        } else if h.is_active() || h.attach_system_id.is_some() {
            h.sync_attach_particle_position(&mut self.combat_particles, pos);
        }
        if let Some(obj) = self.objects.get_mut(&id) {
            obj.helicopter_slow_death = Some(h);
        }
    }

    /// C++ Object::setDisabledUntil orderSlavesDisabledUntil for spawn-weapon sites.
    fn propagate_hacked_to_spawn_slaves(&mut self) {
        let sites: Vec<(ObjectId, u32)> = self
            .objects
            .values()
            .filter(|o| o.status.disabled_hacked && o.is_spawns_are_the_weapons())
            .map(|o| (o.id, o.status.disabled_hacked_until_frame))
            .collect();
        for (site_id, until) in sites {
            let child_ids: Vec<ObjectId> = self
                .objects
                .values()
                .filter(|o| {
                    o.producer_id == Some(site_id)
                        && (!o.status.disabled_hacked
                            || (until > 0 && until > o.status.disabled_hacked_until_frame))
                })
                .map(|o| o.id)
                .collect();
            for child_id in child_ids {
                if let Some(child) = self.objects.get_mut(&child_id) {
                    child.apply_disabled_hacked(until, &mut self.health_events);
                }
                self.admit_host_radar_disabled_edges(child_id);
            }
        }
    }
}

/// Retail FXList.ini Sound nuggets when the FX store is not loaded.
fn heli_fx_fallback_sounds(fx: &str) -> Vec<String> {
    match fx {
        "FX_HelicopterHitGround" => vec!["ComancheCrash".into()],
        "FX_GroundedHelicopterBlowUp" | "FX_HelixHelicopterBlowUpBig" => {
            vec!["CarDie".into()]
        }
        "FX_HelicopterBladeExplosion" => vec!["ComancheSpinExplosion".into()],
        _ => Vec::new(),
    }
}
