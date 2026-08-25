use super::super::super::*;

impl GameLogic {
    pub fn honesty_baikonur_ok(&self) -> bool {
        self.baikonur_launches.honesty_host_path_ok()
    }

    pub fn baikonur_launches(
        &self,
    ) -> &crate::game_logic::host_baikonur_launch::HostBaikonurLaunchRegistry {
        &self.baikonur_launches
    }

    /// C++ BaikonurLaunchPower::doSpecialPower residual — DOOR_1_OPENING on tower.
    pub fn activate_baikonur_launch_door(&mut self, source_id: ObjectId) -> bool {
        use crate::game_logic::host_enum_table_residual::door_1_opening_model_bit;
        let Some(obj) = self.objects.get_mut(&source_id) else {
            return false;
        };
        if obj.is_disabled() {
            return false;
        }
        let bit = door_1_opening_model_bit();
        obj.model_condition_bits |= 1u128 << bit;
        obj.refresh_model_condition_bits();
        self.baikonur_launches.record_launch_door();
        true
    }

    /// C++ BaikonurLaunchPower::doSpecialPowerAtLocation residual —
    /// spawn BaikonurRocketDetonation + NeutronMissileSlowDeath multi-blast.
    pub fn activate_baikonur_detonation(
        &mut self,
        source_id: ObjectId,
        location: glam::Vec3,
    ) -> bool {
        use crate::game_logic::host_baikonur_launch::{
            BAIKONUR_DETONATION_OBJECT, BAIKONUR_NUKE_FX,
        };
        let Some(src) = self.objects.get(&source_id) else {
            return false;
        };
        if src.is_disabled() {
            return false;
        }
        let team = src.team;
        // Ensure detonation template exists residual.
        if !self.templates.contains_key(BAIKONUR_DETONATION_OBJECT) {
            let mut t = crate::game_logic::ThingTemplate::new(BAIKONUR_DETONATION_OBJECT);
            t.set_health(1.0);
            t.add_kind_of(crate::game_logic::KindOf::Immobile);
            self.templates
                .insert(BAIKONUR_DETONATION_OBJECT.to_string(), t);
        }
        let det_id = match self.create_object(BAIKONUR_DETONATION_OBJECT, team, location) {
            Some(id) => id,
            None => return false,
        };
        // Arm Neutron multi-blast residual at detonation (same as nuke impact).
        let _ = self
            .special_power_strikes
            .spawn_neutron_slow_death_field(det_id, team, location, self.frame, 0);
        // Presentation FX residual name on detonation object.
        if let Some(d) = self.objects.get_mut(&det_id) {
            d.pending_death_fx = Some(BAIKONUR_NUKE_FX.to_string());
            // Lifetime 0 residual — mark for quick completion after blasts.
            d.ensure_lifetime_update(self.frame);
        }
        self.baikonur_launches
            .record_detonation(location.x, location.z);
        // Queue audio residual.
        self.queue_audio_event(
            crate::game_logic::AudioEventRequest::new("BaikonurRocketDetonation")
                .with_object(det_id)
                .with_position(location)
                .with_priority(200),
        );
        true
    }

    /// Activate EmpPulse residual: temporarily disable vehicles/structures in radius.
    ///
    /// Matches retail SuperweaponEMPPulse → EMPPulseEffectSpheroid EMPUpdate:
    /// - Radius residual 200 (RadiusCursorRadius / default EffectRadius)
    /// - DisabledDuration 30000 ms → 900 logic frames (DISABLED_EMP)
    /// - Vehicles + faction structures disabled; airborne aircraft killed residual
    ///
    /// Fail-closed: not full OCL bomb / spheroid drawable / spark particle path.
    /// Returns true when the residual activation was recorded (even if 0 targets).
    pub fn activate_emp_pulse(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        // C++ SUPERWEAPON_EMPPulse DeliverPayload residual: cargo plane + bomb first.
        if let Some(cid) = caster_id {
            if self
                .spawn_emp_pulse_flight(cid, location, player_id)
                .is_some()
            {
                return true;
            }
        }
        self.apply_emp_pulse_at(player_id, location, caster_id)
    }

    /// Apply EMP disable field residual at location (bomb impact / fail-open path).
    ///
    /// C++ EMPUpdate ctor sets `m_tintEnvPlayFrame = now + StartFadeTime` and
    /// only calls `doDisableAttack` on that exact frame. Spawn the spheroid now;
    /// disable waits StartFadeTime (9 frames).
    pub fn apply_emp_pulse_at(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_emp_pulse::EMP_PULSE_ACTIVATE_AUDIO;

        let frame = self.frame;
        let spheroid_id = if let Some(pid) = caster_id {
            self.spawn_emp_pulse_spheroid(location, pid)
        } else {
            self.objects
                .keys()
                .next()
                .copied()
                .and_then(|pid| self.spawn_emp_pulse_spheroid(location, pid))
        };
        if let Some(sid) = spheroid_id {
            self.emp_pulses
                .begin_spheroid(sid, player_id, location, caster_id, frame);
        }

        self.queue_audio_event(
            AudioEventRequest::new(EMP_PULSE_ACTIVATE_AUDIO)
                .with_position(location)
                .with_priority(180),
        );
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponImpact,
            location,
            frame,
            caster_id,
            None,
        );

        true
    }

    /// C++ EMPUpdate::doDisableAttack residual (FROM_BOUNDINGSPHERE_3D).
    /// `curVictim != object` skips the EMPPulseEffectSpheroid, not the caster.
    /// Pulse spheroid DoesNotAffectMyOwnBuildings=No — own buildings disable.
    pub fn apply_emp_pulse_disable_field_at(
        &mut self,
        player_id: u32,
        location: Vec3,
        caster_id: Option<ObjectId>,
    ) -> bool {
        use crate::game_logic::host_emp_pulse::{
            EMP_PULSE_DISABLED_DURATION_FRAMES, HOST_EMP_PULSE_RADIUS, HostEmpPulse,
            in_emp_pulse_radius_from_bounding_sphere_3d, is_emp_hardened_name,
            is_legal_emp_disable_target, leftover_emp_bounding_sphere_radius,
            should_emp_kill_airborne, should_emp_skip_hardened_airborne,
        };

        let frame = self.frame;
        let until = frame.saturating_add(EMP_PULSE_DISABLED_DURATION_FRAMES);

        let candidates: Vec<(ObjectId, bool, bool, bool, bool, bool, bool, bool)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                // C++ EMPUpdate.cpp:192 — skip the spheroid (`object`), not caster_id.
                if obj.emp_pulse_spheroid {
                    return None;
                }
                let pos = obj.get_position();
                let sphere = leftover_emp_bounding_sphere_radius(
                    obj.thing.geometry.radius,
                    obj.thing.geometry.bounds_min,
                    obj.thing.geometry.bounds_max,
                    obj.selection_radius,
                );
                if !in_emp_pulse_radius_from_bounding_sphere_3d(
                    location,
                    pos,
                    sphere,
                    HOST_EMP_PULSE_RADIUS,
                ) {
                    return None;
                }
                let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                let is_structure = obj.is_kind_of(KindOf::Structure);
                let is_faction_structure = is_structure && obj.is_faction_structure();
                let is_aircraft = obj.is_kind_of(KindOf::Aircraft);
                let is_airborne = obj.status.airborne_target;
                let is_spawns = obj
                    .template_name
                    .to_ascii_lowercase()
                    .contains("spawnsaretheweapons")
                    || obj.template_name.to_ascii_lowercase().contains("stinger");
                let under_construction =
                    obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                let emp_hardened = is_emp_hardened_name(&obj.template_name);
                Some((
                    *id,
                    is_vehicle,
                    is_faction_structure,
                    is_aircraft,
                    is_airborne,
                    is_spawns,
                    under_construction,
                    emp_hardened,
                ))
            })
            .collect();

        let mut disables: u32 = 0;
        let mut airborne_kills: u32 = 0;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();
        let mut spark_ids: Vec<ObjectId> = Vec::new();

        for (
            id,
            is_vehicle,
            is_faction_structure,
            is_aircraft,
            is_airborne,
            is_spawns,
            under_construction,
            emp_hardened,
        ) in candidates
        {
            if should_emp_kill_airborne(is_aircraft, is_airborne, emp_hardened) {
                destroy_ids.push(id);
                airborne_kills = airborne_kills.saturating_add(1);
                continue;
            }
            // C++ EMPUpdate.cpp:240-241 — EMP_HARDENED airborne continue.
            if should_emp_skip_hardened_airborne(is_aircraft, is_airborne, emp_hardened) {
                continue;
            }

            if !is_legal_emp_disable_target(
                is_vehicle,
                is_faction_structure,
                is_spawns,
                true,
                under_construction,
                emp_hardened,
            ) {
                continue;
            }

            let Some(target) = self.objects.get_mut(&id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            target.apply_disabled_emp(until);
            disables = disables.saturating_add(1);
            spark_ids.push(id);
        }

        for id in destroy_ids {
            let killer_team = caster_id
                .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
                .unwrap_or(Team::Neutral);
            self.mark_object_for_destruction(id, Some(killer_team));
        }
        // C++ doDisableAttack EMPSparks on disabled victims (not airborne kills).
        for vid in spark_ids {
            self.spawn_emp_sparks_on_victim(vid, EMP_PULSE_DISABLED_DURATION_FRAMES);
        }

        let pulse_id = self.emp_pulses.alloc_id();
        self.emp_pulses.record_activation(HostEmpPulse {
            id: pulse_id,
            player_id,
            location,
            radius: HOST_EMP_PULSE_RADIUS,
            activate_frame: frame,
            disable_until_frame: until,
            caster_id,
            disables,
            airborne_kills,
        });
        true
    }

    /// Fire leftover EMPUpdate doDisableAttack on StartFadeTime frames.
    pub fn apply_due_emp_pulse_disables(&mut self) {
        use crate::game_logic::host_emp_pulse::EMP_SPHEROID_GEOMETRY_RADIUS;

        let now = self.frame;
        self.emp_pulses.tick_spheroids(now);
        let visual: Vec<(ObjectId, f32)> = self
            .emp_pulses
            .spheroids()
            .iter()
            .map(|s| (s.id, s.current_scale))
            .collect();
        for (id, scale) in visual {
            if let Some(o) = self.objects.get_mut(&id) {
                if o.emp_pulse_spheroid {
                    o.thing.geometry.radius = EMP_SPHEROID_GEOMETRY_RADIUS * scale;
                    o.visual_draw_state_revision = o.visual_draw_state_revision.wrapping_add(1);
                }
            }
        }
        let due = self.emp_pulses.due_disable_spheroids(now);
        for sph in due {
            self.emp_pulses.mark_disable_applied(sph.id);
            let _ =
                self.apply_emp_pulse_disable_field_at(sph.player_id, sph.location, sph.caster_id);
        }
    }

    /// Host China Frenzy ("Rage") residual registry (activate + honesty).
    pub fn frenzies(&self) -> &crate::game_logic::host_frenzy::HostFrenzyRegistry {
        &self.frenzies
    }

    /// Residual honesty: Frenzy activated at least once.
    pub fn honesty_frenzy_activate_ok(&self) -> bool {
        self.frenzies.honesty_activate_ok()
    }

    /// Residual honesty: Frenzy applied attack buff at least once.
    pub fn honesty_frenzy_buff_ok(&self) -> bool {
        self.frenzies.honesty_buff_ok()
    }

    /// Combined host path honesty for Frenzy / Rage residual.
    pub fn honesty_frenzy_ok(&self) -> bool {
        self.frenzies.honesty_host_path_ok()
    }

    /// Host USA Strategy Center battle-plan residual registry (select + honesty).
    pub fn battle_plans(&self) -> &crate::game_logic::host_strategy_center::HostBattlePlanRegistry {
        &self.battle_plans
    }

    /// C++ `Player::xfer` (`Player.cpp:4480-4507`) restores `m_battlePlanBonuses`.
    pub fn restore_battle_plans(
        &mut self,
        registry: crate::game_logic::host_strategy_center::HostBattlePlanRegistry,
    ) {
        self.battle_plans = registry;
    }

    /// Residual honesty: Strategy Center battle plan selected at least once.
    pub fn honesty_battle_plan_select_ok(&self) -> bool {
        self.battle_plans.honesty_select_ok()
    }

    /// Residual honesty: battle plan applied army residual buff at least once.
    pub fn honesty_battle_plan_buff_ok(&self) -> bool {
        self.battle_plans.honesty_buff_ok()
    }

    /// Residual honesty: BattlePlanChangeParalyze residual applied at least once.
    pub fn honesty_battle_plan_paralyze_ok(&self) -> bool {
        self.battle_plans.honesty_paralyze_ok()
    }

    /// Combined host path honesty for Strategy Center battle-plan residual.
    pub fn honesty_battle_plan_ok(&self) -> bool {
        self.battle_plans.honesty_host_path_ok()
    }

    /// Residual honesty: Bombardment turret StrategyCenterGun fired.
    pub fn honesty_battle_plan_turret_fire_ok(&self) -> bool {
        self.battle_plans.honesty_turret_fire_ok()
    }

    /// Residual honesty: StealthDetectorUpdate enabled (SearchAndDestroy residual).
    pub fn honesty_battle_plan_stealth_detector_ok(&self) -> bool {
        self.battle_plans.honesty_stealth_detector_ok()
    }

    /// Residual honesty: pack/unpack door residual started.
    pub fn honesty_battle_plan_door_ok(&self) -> bool {
        self.battle_plans.honesty_door_residual_ok()
    }

    /// Residual honesty: door residual reached ACTIVE / WAITING_TO_CLOSE.
    pub fn honesty_battle_plan_door_active_ok(&self) -> bool {
        self.battle_plans.honesty_door_active_ok()
    }

    /// Residual honesty: delayed setBattlePlan applied after unpack ACTIVE.
    pub fn honesty_battle_plan_delayed_active_ok(&self) -> bool {
        self.battle_plans.honesty_delayed_active_apply_ok()
    }

    /// Residual honesty: setBattlePlan(NONE) pack-clear residual fired.
    pub fn honesty_battle_plan_pack_clear_ok(&self) -> bool {
        self.battle_plans.honesty_pack_clear_ok()
    }

    /// Residual honesty: Bombardment turret recenter residual before pack.
    pub fn honesty_battle_plan_turret_recenter_ok(&self) -> bool {
        self.battle_plans.honesty_turret_recenter_ok()
    }

    /// Residual honesty: Strategy Center turret pitch/yaw left natural (aim residual).
    pub fn honesty_strategy_center_turret_aim_ok(&self) -> bool {
        self.objects.values().any(|o| {
            crate::game_logic::host_strategy_center::is_strategy_center_template(&o.template_name)
                && !crate::game_logic::host_strategy_center::turret_angles_are_natural(
                    o.turret_angle_deg,
                    o.turret_pitch_deg,
                )
        })
    }

    /// Residual honesty: TurretAI idle-scan residual started (Bombardment ACTIVE).
    pub fn honesty_strategy_center_turret_idle_scan_ok(&self) -> bool {
        self.battle_plans.honesty_turret_idle_scan_ok()
    }

    /// Residual honesty: TurretAI HoldTurret residual started (after idle-scan).
    pub fn honesty_strategy_center_turret_hold_ok(&self) -> bool {
        self.battle_plans.honesty_turret_hold_ok()
    }

    /// Residual honesty: TurretAI idle-recenter residual completed (after Hold).
    pub fn honesty_strategy_center_turret_idle_recenter_ok(&self) -> bool {
        self.battle_plans.honesty_turret_idle_recenter_ok()
    }

    /// Tick TurretAI idle mood-target residual for Bombardment ACTIVE Strategy Centers.
    ///
    /// C++ `TurretAI::friend_checkForIdleMoodTarget` residual:
    /// - When idle, acquire nearest legal enemy in StrategyCenterGun range band
    /// - Aim pitch/yaw at target (FirePitch **45**), flag `m_targetWasSetByIdleMood`
    /// - While held: re-aim each frame; clear when dead / OOR / illegal (team/air/UC)
    /// - Mood matrix Sleep → IgnoreAll (no acquire); Passive → WaitForAttack
    ///   (only last_damage_source residual); Normal/Alert/Aggressive → free
    /// - Fire residual ownership: bombardment fire clears mood flag if it engages
    ///   a different target (see `try_strategy_center_bombardment_turret_fire`)
    pub(in crate::game_logic) fn tick_strategy_center_turret_mood_target(&mut self) {
        use crate::game_logic::host_strategy_center::{
            HostAiAttitude, HostBattlePlan, HostBattlePlanTransition, is_strategy_center_template,
            strategy_center_gun_in_range, strategy_center_mood_target_eligible_with_attitude,
            strategy_center_mood_target_enemy_legal_with_vision,
            strategy_center_mood_target_in_vision,
            strategy_center_mood_target_should_clear_with_vision,
            strategy_center_mood_vision_range, strategy_center_turret_aim_at,
        };

        // Bombardment ACTIVE centers.
        let centers: Vec<ObjectId> = self
            .battle_plans
            .door_states()
            .iter()
            .filter(|s| {
                s.status == HostBattlePlanTransition::Active
                    && s.door_plan == Some(HostBattlePlan::Bombardment)
                    && !s.centering_turret
            })
            .map(|s| s.center_id)
            .collect();

        let mut acquires = 0u32;
        let mut clears = 0u32;
        for cid in centers {
            let Some(obj) = self.objects.get(&cid) else {
                continue;
            };
            if !obj.is_alive() || !is_strategy_center_template(&obj.template_name) {
                continue;
            }
            if obj.weapon.is_none() {
                continue;
            }
            let team = obj.team;
            let fire_pos = obj.get_position();
            let has_mood = obj.turret_mood_target;
            let attitude = HostAiAttitude::from_i8(obj.ai_attitude);
            let last_dmg = obj.last_damage_source;
            // Partition / AI vision residual: VisionRange **400**.
            // Bombardment ACTIVE path: S&D sight scalar does not apply (plans
            // are mutually exclusive). Host residual still uses the vision
            // filter helper so reduced-vision / S&D matrix stays host-testable.
            let vision_range = strategy_center_mood_vision_range(false);
            // "Busy" for acquire: only non-mood attacking (pack recenter / explicit
            // non-mood attack). Mood-set Attacking is the hold state, not busy.
            let busy_non_mood = !has_mood
                && (obj.status.attacking
                    || matches!(
                        obj.ai_state,
                        AIState::Attacking | AIState::AttackMoving | AIState::AttackingGround
                    ));

            // Hold / clear / re-aim mood target residual.
            if has_mood {
                let tgt = obj.target;
                let mut clear = tgt.is_none();
                let mut aim_xz: Option<(f32, f32)> = None;
                if let Some(tid) = tgt {
                    if let Some(t) = self.objects.get(&tid) {
                        let tp = t.get_position();
                        let dx = tp.x - fire_pos.x;
                        let dz = tp.z - fire_pos.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        let is_air = t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target;
                        let legal = strategy_center_mood_target_enemy_legal_with_vision(
                            t.is_alive(),
                            t.team == team,
                            t.team == Team::Neutral,
                            t.status.under_construction,
                            is_air,
                            dist,
                            vision_range,
                        );
                        let in_range = strategy_center_gun_in_range(dist);
                        let in_vision = strategy_center_mood_target_in_vision(dist, vision_range);
                        clear = strategy_center_mood_target_should_clear_with_vision(
                            true,
                            t.is_alive(),
                            in_range,
                            in_vision,
                        ) || !legal;
                        if !clear {
                            aim_xz = Some((tp.x, tp.z));
                        }
                    } else {
                        clear = true;
                    }
                }
                if clear {
                    if let Some(o) = self.objects.get_mut(&cid) {
                        o.turret_mood_target = false;
                        o.set_status_attacking(false);
                        o.target = None;
                        if matches!(o.ai_state, AIState::Attacking) {
                            o.set_ai_state(AIState::Idle);
                            if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                                crate::game_logic::host_ai_decision_log::record_set_state(cid, 0);
                            }
                        }
                    }
                    if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                        crate::game_logic::host_ai_decision_log::record_stop_attack(cid);
                    }
                    clears = clears.saturating_add(1);
                } else if let Some((tx, tz)) = aim_xz {
                    // C++ AIM continuous aim residual while mood target held.
                    let (aim_a, aim_p) =
                        strategy_center_turret_aim_at(fire_pos.x, fire_pos.z, tx, tz);
                    if let Some(o) = self.objects.get_mut(&cid) {
                        o.turret_angle_deg = aim_a;
                        o.record_host_turret();
                        o.turret_pitch_deg = aim_p;
                        o.record_host_turret();
                        o.set_ai_state(AIState::Attacking);
                        o.set_status_attacking(true);
                        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                            crate::game_logic::host_ai_decision_log::record_set_state(cid, 2);
                        }
                    }
                }
                continue; // no re-acquire this frame while mood flag set
            }

            // Passive WaitForAttack: only retaliate vs last_damage_source residual.
            let passive_last = last_dmg.is_some();
            if !strategy_center_mood_target_eligible_with_attitude(
                true,
                true,
                busy_non_mood,
                has_mood,
                attitude,
                passive_last,
            ) {
                continue;
            }

            // Find residual mood target: Passive uses last damage source only
            // (C++ getNextMoodTarget Passive branch); else nearest legal enemy.
            // Partition vision residual gates acquire distance.
            let mut best: Option<(ObjectId, f32, f32, f32)> = None; // id, dist, x, z
            if attitude.idle_mood_wait_for_attack() {
                if let Some(tid) = last_dmg {
                    if tid != cid {
                        if let Some(other) = self.objects.get(&tid) {
                            let op = other.get_position();
                            let dx = op.x - fire_pos.x;
                            let dz = op.z - fire_pos.z;
                            let dist = (dx * dx + dz * dz).sqrt();
                            let is_air =
                                other.is_kind_of(KindOf::Aircraft) || other.status.airborne_target;
                            if strategy_center_mood_target_enemy_legal_with_vision(
                                other.is_alive(),
                                other.team == team,
                                other.team == Team::Neutral,
                                other.status.under_construction,
                                is_air,
                                dist,
                                vision_range,
                            ) {
                                best = Some((tid, dist, op.x, op.z));
                            }
                        }
                    }
                }
            } else {
                // Pure residual acquire: nearest legal enemy in mood vision (XZ).
                let candidates: Vec<_> = self
                    .objects
                    .iter()
                    .map(|(&oid, other)| {
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id: oid,
                            team: other.team,
                            position: other.get_position(),
                            is_alive: other.is_alive(),
                            is_neutral: other.team == Team::Neutral,
                            under_construction: other.status.under_construction,
                            combat_kind: true,
                            effectively_stealthed: other.is_effectively_stealthed(),
                            is_air: other.is_kind_of(KindOf::Aircraft)
                                || other.status.airborne_target,
                            eject_invulnerable: other.is_eject_invulnerable(),
                        }
                    })
                    .collect();
                best = crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                    Some(cid),
                    (fire_pos.x, fire_pos.z),
                    candidates,
                    vision_range,
                    |c| {
                        let dist = {
                            let dx = c.position.x - fire_pos.x;
                            let dz = c.position.z - fire_pos.z;
                            (dx * dx + dz * dz).sqrt()
                        };
                        strategy_center_mood_target_enemy_legal_with_vision(
                            c.is_alive,
                            c.team == team,
                            c.is_neutral,
                            c.under_construction,
                            c.is_air,
                            dist,
                            vision_range,
                        )
                    },
                )
                .map(|(id, dist, _)| {
                    let p = self
                        .objects
                        .get(&id)
                        .map(|o| o.get_position())
                        .unwrap_or(fire_pos);
                    (id, dist, p.x, p.z)
                });
            }
            if let Some((tid, _, tx, tz)) = best {
                let (aim_a, aim_p) = strategy_center_turret_aim_at(fire_pos.x, fire_pos.z, tx, tz);
                if let Some(o) = self.objects.get_mut(&cid) {
                    o.set_target(Some(tid));
                    o.turret_mood_target = true;
                    o.turret_angle_deg = aim_a;
                    o.record_host_turret();
                    o.turret_pitch_deg = aim_p;
                    o.record_host_turret();
                    // Mood acquire cancels idle-scan residual.
                    o.turret_idle_scanning = false;
                    o.record_host_turret();
                    o.turret_holding = false;
                    o.record_host_turret();
                    o.turret_hold_until_frame = 0;
                    o.turret_idle_recentering = false;
                    o.set_ai_state(AIState::Attacking);
                    o.set_status_attacking(true);
                }
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_attack(cid, tid);
                    crate::game_logic::host_ai_decision_log::record_set_state(cid, 2);
                }
                acquires = acquires.saturating_add(1);
            }
        }
        for _ in 0..acquires {
            self.battle_plans.record_turret_mood_target_acquire();
        }
        for _ in 0..clears {
            self.battle_plans.record_turret_mood_target_clear();
        }
    }
}
