//! Host tick `impl GameLogic` — `ai`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    pub(in super::super) fn update_ai(&mut self, object_ids: &[ObjectId], dt: f32) {
        use crate::ai_decisions::*;
        self.flush_countermeasure_flare_spawns();
        // Wave 806: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_countermeasure_flare_objects();
        }

        let mut ai_commands = Vec::new();
        let current_time = self.frame as f32 * LOGIC_FRAME_TIMESTEP; // Convert frame to seconds
        let game_phase = GamePhase::from_time(current_time);
        // Campaign maps place thousands of decorative props. Skip AI for
        // non-combat, non-structure objects so frame cost stays reasonable.
        let dense_world = object_ids.len() > 400;

        // BattlePlan pack/unpack door residual (AnimationTime 7000ms → 210 frames).
        self.tick_battle_plan_door_residuals();

        // First pass: Dispatch object AI through the existing state machine.
        for &object_id in object_ids {
            // Expire DISABLED_HACKED / DISABLED_EMP / Frenzy residual timers.
            let mut topple_kill = false;
            let mut lifetime_kill = false;
            let mut poison_kill = false;
            let mut defector_audio: Vec<String> = Vec::new();
            if let Some(obj) = self.objects.get_mut(&object_id) {
                // Wave 761: under coupled GameWorld shadow, status timer expire
                // (faerie/repulsor/disable/frenzy/continuous-fire/selection flash)
                // is owned by `tick_status_timer_expirations` + writeback. Host must
                // not dual-expire mid-frame. Eject-invulnerable stays host-only
                // (no GW until_frame field yet).
                let peel_status_timers = crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active();
                if !peel_status_timers {
                    obj.tick_disabled_hacked(self.frame);
                    obj.tick_selection_flash();
                    obj.tick_disabled_emp(self.frame);
                    obj.tick_disabled_paralyzed(self.frame);
                    obj.tick_weapon_bonus_frenzy(self.frame);
                    obj.tick_faerie_fire(self.frame);
                }
                // Wave 762: under coupled shadow, eject-invulnerable expire is
                // owned by GW tick_status_timer_expirations + writeback.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    obj.tick_eject_invulnerable(self.frame);
                }
                // Wave 766: under coupled shadow, ObjectDefectionHelper timer is
                // owned by GW tick_status_timer_expirations + writeback.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
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
                // C++ PoisonedBehavior::update residual (DoT).
                // Wave 769: under coupled shadow, PoisonedBehavior DoT is owned by
                // GW tick_status_timer_expirations + host_poison_dot_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if let Some((dot, death_ty)) = obj.tick_poisoned_behavior(self.frame) {
                        // Apply as UNRESISTABLE so it doesn't re-infect (C++).
                        let killed = obj.take_damage_from_typed_death(
                            dot,
                            None,
                            crate::game_logic::combat::DamageType::Unresistable,
                            death_ty,
                        );
                        if killed {
                            poison_kill = true;
                        }
                    }
                }
                // Wave 778: under coupled shadow, FWWDB continuous is owned by
                // GW tick_status_timer_expirations + host_fwwd_continuous_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if obj.temporary_weapon_runtime.damaged.is_empty() {
                        if let Some(w) = obj.tick_fire_weapon_when_damaged_continuous(self.frame) {
                            if obj.pending_fire_when_damaged_weapon.is_none() {
                                obj.pending_fire_when_damaged_weapon = Some(w);
                            }
                        }
                    }
                }
                // C++ LifetimeUpdate residual.
                // Wave 745: under damage authority, do not zero host HP / stamp
                // destroyed mid-frame (dual with GW HP writeback). Mark-for-destroy
                // owns lethal residual; non-authority path keeps host HP clear.
                // Wave 768: under coupled shadow, LifetimeUpdate expire is owned by
                // GW tick_status_timer_expirations + host_lifetime_expire_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if obj.tick_lifetime_update(self.frame) {
                        lifetime_kill = true;
                        if !crate::gameworld_shadow::gameworld_damage_authority_live() {
                            obj.health.current = 0.0;
                            obj.status.destroyed = true;
                            obj.refresh_model_condition_bits();
                        }
                    }
                }
                // Wave 761: continuous-fire coast + repulsor expire peel under coupled.
                // Wave 761: CF coast + repulsor peel under coupled.
                // Wave 765: subdual heal owned by GW when coupled.
                // Wave 767: fire-sound loop stop owned by GW tick_status_timer_expirations
                // when coupled (non-coupled still via tick_continuous_fire_coast).
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    obj.tick_continuous_fire_coast(self.frame);
                    obj.tick_repulsor_status(self.frame);
                }
                // Wave 763: under coupled shadow, force-reload-when-idle is owned by
                // GW tick_status_timer_expirations + weapon_stats/continuous-fire writeback.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    obj.tick_force_reload_when_idle(self.frame);
                }
                obj.tick_spy_vision_disabled(self.frame);
                if obj.tick_disguise_transition() {
                    self.bomb_truck_disguise.record_transition_halfpoint();
                }
                // Wave 770: under coupled shadow, ToppleUpdate fall is owned by
                // GW tick_status_timer_expirations + host_topple_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    // C++ ToppleUpdate::update residual (trees / crushable props).
                    topple_kill = obj.tick_topple();
                }
                // C++ StructureToppleUpdate::update residual (buildings).
                // C++ HeightDieUpdate residual (bombs/missiles).
                // Wave 771: under coupled shadow, HeightDieUpdate is owned by
                // GW tick_status_timer_expirations + host_height_die_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if !topple_kill && obj.tick_height_die(self.frame, 0.0) {
                        topple_kill = true;
                    }
                }
                // Wave 772: under coupled shadow, JetSlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + host_jet_slow_death_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
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
                    }
                }
                // Wave 773: under coupled shadow, HelicopterSlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + host_heli_slow_death_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
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
                    }
                }
                // Wave 774: under coupled shadow, SlowDeathBehavior is owned by
                // GW tick_status_timer_expirations + host_slow_death_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if !topple_kill
                        && obj
                            .slow_death
                            .as_ref()
                            .map(|s| s.is_active())
                            .unwrap_or(false)
                    {
                        topple_kill = obj.tick_slow_death(self.frame);
                    }
                }
                // Wave 775: under coupled shadow, StructureCollapseUpdate is owned by
                // GW tick_status_timer_expirations + host_structure_collapse_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if !topple_kill && obj.structure_collapse_data.is_some() {
                        topple_kill = obj.tick_structure_collapse(self.frame);
                    }
                }
                // Wave 776: under coupled shadow, StructureToppleUpdate is owned by
                // GW tick_status_timer_expirations + host_structure_topple_kill_log drain.
                if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                    && crate::gameworld_shadow::shadow_coupled_tick_active())
                {
                    if !topple_kill && obj.structure_topple_data.is_some() {
                        topple_kill = obj.tick_structure_topple(self.frame);
                    }
                }
            }
            // Wave 777: under coupled shadow, StructureTopple crush sweep is owned by
            // GW tick_status_timer_expirations + host_structure_topple_crush_log drain.
            if !(crate::gameworld_shadow::gameworld_shadow_enabled()
                && crate::gameworld_shadow::shadow_coupled_tick_active())
            {
                // C++ StructureToppleUpdate::applyCrushingDamage residual.
                if self
                    .objects
                    .get(&object_id)
                    .and_then(|o| o.structure_topple_data.as_ref())
                    .map(|d| d.is_active() || topple_kill)
                    .unwrap_or(false)
                {
                    let samples = self
                        .objects
                        .get_mut(&object_id)
                        .map(|o| o.take_structure_topple_crush_samples())
                        .unwrap_or_default();
                    if !samples.is_empty() {
                        self.apply_structure_topple_crush_samples(object_id, samples);
                    }
                }
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
                let damage_events: Vec<_> = crate::game_logic::host_damage_log::snapshot()
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
                let pos = self
                    .objects
                    .get(&object_id)
                    .map(|o| o.get_position())
                    .unwrap_or(glam::Vec3::ZERO);
                let (transition_evs, death_fx, death_audio) =
                    if let Some(o) = self.objects.get_mut(&object_id) {
                        let te = o.take_pending_transition_damage_fx();
                        let (df, da) = o.take_pending_death_fx_audio();
                        (te, df, da)
                    } else {
                        (Vec::new(), None, None)
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
                        if !crate::game_logic::dispatch_fx_list_at_pos(&fx, pos) {
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
                }
                if let Some(a) = death_audio {
                    self.queue_audio_event(
                        AudioEventRequest::new(&a)
                            .with_object(object_id)
                            .with_position(pos)
                            .with_priority(200),
                    );
                }
                if let Some(fx) = death_fx {
                    if !crate::game_logic::dispatch_fx_list_at_pos(&fx, pos) {
                        for sound in crate::game_logic::sound_names_for_fx_list(&fx) {
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
            // C++ CreateObjectDie residual (spawn after death FX).
            self.apply_pending_create_object_die(object_id);
            if lifetime_kill {
                self.mark_object_for_destruction(object_id, None);
                continue;
            }
            for a in defector_audio {
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(a.as_str())
                        .with_object(object_id)
                        .with_priority(80),
                );
            }
            if poison_kill {
                self.mark_object_for_destruction(object_id, None);
                continue;
            }
            if topple_kill {
                // Completed topple: queue destroy (structure Done bypasses re-topple).
                self.mark_object_for_destruction(object_id, None);
                continue;
            }
            // OCL_EjectPilotViaParachute residual sink (elevated pilot → ground).
            self.tick_eject_parachute_residual(object_id);
            // AmericaCrateParachute residual sink (cargo crate freefall → OpenDist → land).
            self.tick_crate_parachute_residual(object_id);
            // PilotFindVehicleUpdate residual: AI idle pilot auto-scan for
            // recrewable unmanned vehicles (ScanRate 1000ms / range 300 / MinHealth 0.5).
            // C++ human players sleep forever — host residual: is_local → skip.
            // Base-center fallback residual when no vehicle found (m_didMoveToBase).
            self.try_pilot_find_vehicle_residual(object_id);
            // AutoFindHealingUpdate residual: AI idle injured USA infantry → HealPad.
            // Pilot / Ranger / MissileDefender / Pathfinder / ColonelBurton residual.
            // ScanRate 1000ms / ScanRange 300 / NeverHeal 0.85 (AlwaysHeal busy path fail-closed).
            self.try_auto_find_healing_residual(object_id);
            // AutoFindRepair residual: AI idle damaged vehicles → RepairPad/WarFactory.
            self.try_auto_find_repair_residual(object_id);
            self.try_auto_resume_construction_residual(object_id);
            // C++ AIIdleState: CAN_BE_REPULSED idle units flee closest repulsor.
            let _ = self.try_idle_repulse(object_id);
            // C++ AIIdleState: checkForCrateToPickup → aiMoveToObject.
            let _ = self.try_idle_crate_pickup(object_id);
            // C++ AIIdleState::update: mood scan / attack only — never AI_HUNT wander.
            if let Some(obj) = self.objects.get(&object_id) {
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
        let decision_auth = crate::gameworld_shadow::gameworld_ai_decision_authority_live();
        for command in ai_commands {
            if decision_auth {
                // Record only — shadow applies SetAttackTarget/SetMoveTarget/SetAiState.
                match &command {
                    AICommand::AttackTarget {
                        object_id,
                        target_id,
                    } => crate::game_logic::host_ai_decision_log::record_attack(
                        *object_id, *target_id,
                    ),
                    AICommand::StopAttack { object_id } => {
                        crate::game_logic::host_ai_decision_log::record_stop_attack(*object_id)
                    }
                    AICommand::MoveTo {
                        object_id,
                        position,
                    } => crate::game_logic::host_ai_decision_log::record_move_to(
                        *object_id, *position,
                    ),
                    AICommand::SetAIState { object_id, state } => {
                        let ordinal =
                            crate::gameworld_shadow::GameWorldShadow::host_ai_state_ordinal(&state);
                        crate::game_logic::host_ai_decision_log::record_set_state(
                            *object_id, ordinal,
                        );
                    }
                }
            } else {
                self.apply_ai_command(command);
            }
        }

        // Resolve command-driven support states (guard/repair/docking/garrison) after AI decisions.
        self.update_support_states(object_ids, dt);
    }
}
