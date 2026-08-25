//! Strike damage falloff, due-impact planning, and impact recording.
use super::types::*;
use super::*;
impl HostSpecialPowerStrikeRegistry {
    /// Compute falloff damage for distance from epicenter.
    ///
    /// ScudStorm residual uses retail primary/secondary step damage
    /// (`ScudStormDamageWeapon`): full Primary inside PrimaryRadius, Secondary
    /// out to SecondaryRadius (not linear falloff).
    pub fn damage_at_distance(kind: HostSuperweaponKind, distance: f32) -> f32 {
        Self::damage_at_distance_with_tiers(
            kind,
            distance,
            ScudStormAnthraxTier::Base,
            A10StrikeScienceTier::Level1,
        )
    }

    /// Falloff residual with ScudStorm anthrax-upgrade tier (Secondary 150/200, Primary 500/550).
    pub fn damage_at_distance_with_scud_tier(
        kind: HostSuperweaponKind,
        distance: f32,
        scud_tier: ScudStormAnthraxTier,
    ) -> f32 {
        Self::damage_at_distance_with_tiers(kind, distance, scud_tier, A10StrikeScienceTier::Level1)
    }

    /// Falloff residual with ScudStorm anthrax + A10 FormationSize science tiers.
    ///
    /// A10 residual: OCL jets from the map edge apply missile/vulcan damage.
    /// The delayed circular blob (500 × FormationSize) is not retail.
    pub fn damage_at_distance_with_tiers(
        kind: HostSuperweaponKind,
        distance: f32,
        scud_tier: ScudStormAnthraxTier,
        a10_tier: A10StrikeScienceTier,
    ) -> f32 {
        // Instant one-shot suppressed: multi-blast residual applies Blast6.
        if kind == HostSuperweaponKind::NuclearMissile {
            return 0.0;
        }
        // C++ A10 is CREATE_AT_EDGE_NEAR_SOURCE jets, not a host circle blast.
        if kind == HostSuperweaponKind::A10Strike {
            let _ = (distance, a10_tier);
            return 0.0;
        }
        if kind == HostSuperweaponKind::ScudStorm {
            if distance <= SCUD_STORM_PRIMARY_RADIUS {
                return scud_tier.primary_damage();
            }
            if distance <= SCUD_STORM_SECONDARY_RADIUS {
                return scud_tier.secondary_damage();
            }
            return 0.0;
        }
        let radius = kind.damage_radius();
        let inner = kind.falloff_inner();
        let max = kind.max_damage();
        if distance <= inner {
            max
        } else if distance >= radius {
            0.0
        } else {
            let range = (radius - inner).max(f32::EPSILON);
            let t = (distance - inner) / range;
            max * (1.0 - t).max(0.0)
        }
    }

    /// Build impact damage plans for all strikes whose impact frame has arrived.
    /// Does not mutate object health — GameLogic applies hits.
    ///
    /// Multi-strike residuals (CarpetBomb line / ArtilleryBarrage scatter):
    /// - Shells/bombs apply on their DelayDelivery / DropDelay residual frames.
    /// - Each living enemy takes the max damage from any **due this wave**
    ///   epicenter (not a single circular blast at the click point only).
    /// - Jumping past several stagger frames applies all overdue shells/bombs
    ///   in one wave (save/load and host tests).
    pub fn plan_due_impacts(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, crate::game_logic::Team, bool)],
    ) -> Vec<HostStrikeImpactPlan> {
        let mut plans = Vec::new();
        for strike in self.strikes.values() {
            if strike.phase != HostStrikePhase::Queued || current_frame < strike.impact_frame {
                continue;
            }

            let (due_points, wave_shell_count, is_final_wave) = if strike.kind.is_multi_strike() {
                // Prefer once-at-queue residual plan (stored ADC draws); fall back
                // to re-query for older snapshots without ocl_points.
                let all_points = if !strike.ocl_points.is_empty() {
                    strike.ocl_points.clone()
                } else if strike.kind.is_line_multi_strike() {
                    carpet_bomb_points_for_tier(strike.target_position, strike.carpet_tier)
                } else {
                    strike
                        .kind
                        .multi_strike_points_with_tier(
                            strike.target_position,
                            strike.artillery_tier,
                        )
                        .unwrap_or_default()
                };
                let total = all_points.len() as u32;
                if total == 0 || strike.multi_strike_applied >= total {
                    continue;
                }
                let mut due = Vec::new();
                let mut due_count = 0_u32;
                for (i, p) in all_points.iter().enumerate() {
                    let idx = i as u32;
                    if idx < strike.multi_strike_applied {
                        continue;
                    }
                    let shell_frame = if let Some(&f) = strike.ocl_shell_frames.get(i) {
                        f
                    } else if strike.kind.is_scatter_multi_strike() {
                        artillery_shell_impact_frame(strike.activate_frame, idx)
                    } else if strike.kind.is_scud_multi_strike() {
                        scud_missile_impact_frame(strike.activate_frame, idx)
                    } else {
                        carpet_bomb_impact_frame_for_tier(
                            strike.activate_frame,
                            idx,
                            strike.carpet_tier,
                        )
                    };
                    if shell_frame <= current_frame {
                        due.push(*p);
                        due_count = due_count.saturating_add(1);
                    }
                }
                if due_count == 0 {
                    continue;
                }
                let applied_after = strike.multi_strike_applied.saturating_add(due_count);
                let is_final = applied_after >= total;
                (due, due_count, is_final)
            } else {
                (vec![strike.target_position], 1, true)
            };

            // C++ AttackNugget leftover: flying missiles own ScudStormDamageWeapon.
            // C++ one CarpetBombWeapon per drop: falling bombs own the blast.
            // C++ one FireWeaponWhenDead on AnthraxBomb: falling bomb owns 200/100 + toxin.
            // Registry blob is fallback only when leftover did not schedule.
            let mut hits = Vec::new();
            if !(strike.kind.is_scud_multi_strike() && strike.live_scud_delivery)
                && !(strike.kind.is_line_multi_strike() && strike.live_carpet_delivery)
                && !(strike.kind == HostSuperweaponKind::AnthraxBomb
                    && strike.live_anthrax_delivery)
            {
                for &(id, pos, team, alive) in object_positions {
                    if !alive || id == strike.source_object {
                        continue;
                    }
                    // Retail RadiusDamageAffects ALLIES residual (wave 11).
                    // Kinds without ALLIES still exclude same-team friendlies.
                    if team == strike.source_team && !strike.kind.hits_allies() {
                        continue;
                    }
                    let dmg = if strike.kind.is_multi_strike() {
                        // Multi-strike wave: best (nearest) due shell/bomb epicenter.
                        due_points
                            .iter()
                            .map(|epicenter| {
                                Self::damage_at_distance_with_tiers(
                                    strike.kind,
                                    horizontal_distance(pos, *epicenter),
                                    strike.scud_anthrax_tier,
                                    strike.a10_tier,
                                )
                            })
                            .fold(0.0_f32, f32::max)
                    } else {
                        let dist = horizontal_distance(pos, strike.target_position);
                        let primary = Self::damage_at_distance_with_tiers(
                            strike.kind,
                            dist,
                            strike.scud_anthrax_tier,
                            strike.a10_tier,
                        );
                        // MOABFlameWeapon secondary residual (DaisyCutter / CruiseMissile).
                        // Fail-closed: not full SlowDeath MIDPOINT timing / tree burn state.
                        let flame = if strike.kind.spawns_moab_flame() && dist <= MOAB_FLAME_RADIUS
                        {
                            MOAB_FLAME_DAMAGE
                        } else {
                            0.0
                        };
                        primary + flame
                    };
                    if dmg > 0.0 {
                        hits.push(HostStrikeDamageHit {
                            target_id: id,
                            damage: dmg,
                        });
                    }
                }
            }
            // Presentation epicenter: first due point (or strike target).
            let present_pos = due_points
                .first()
                .copied()
                .unwrap_or(strike.target_position);
            plans.push(HostStrikeImpactPlan {
                strike_id: strike.id,
                kind: strike.kind,
                target_position: present_pos,
                source_object: strike.source_object,
                source_team: strike.source_team,
                hits,
                epicenters: due_points,
                wave_shell_count,
                is_final_wave,
            });
        }
        plans.sort_by_key(|p| p.strike_id);
        plans
    }

    /// Record impact results after GameLogic applied damage.
    ///
    /// Multi-strike waves accumulate damage and only complete on `is_final_wave`.
    /// For `NuclearMissile`, also spawns a residual radiation field at the
    /// epicenter (retail `OCL_NukeRadiationField` residual).
    /// For `AnthraxBomb`, also spawns a residual toxin field at the epicenter
    /// (retail `OCL_PoisonFieldAnthraxBomb` residual).
    /// For `SpectreGunship`, also spawns a residual orbit field at the target
    /// (retail `SpectreGunshipUpdate` ORBITING residual).
    /// For `ParticleCannon`, also spawns a residual continuous beam field at
    /// the target (retail `ParticleUplinkCannonUpdate` STATUS_FIRING residual).
    pub fn record_impact_complete(
        &mut self,
        strike_id: u32,
        total_damage: f32,
        objects_hit: u32,
        objects_destroyed: u32,
    ) {
        // Default: treat as final single wave (legacy callers).
        self.record_impact_wave(
            strike_id,
            total_damage,
            objects_hit,
            objects_destroyed,
            1,
            true,
            &[],
        );
    }

    /// Record one multi-strike impact wave (or a one-shot final impact).
    ///
    /// `epicenters` carries this wave's shell/missile impact points so ScudStorm
    /// can spawn per-missile LargePoisonField residual (retail FireOCL each detonation).
    pub fn record_impact_wave(
        &mut self,
        strike_id: u32,
        total_damage: f32,
        objects_hit: u32,
        objects_destroyed: u32,
        wave_shell_count: u32,
        is_final_wave: bool,
        epicenters: &[Vec3],
    ) {
        let mut spawn_radiation: Option<(ObjectId, crate::game_logic::Team, Vec3, u32)> = None;
        let mut spawn_toxin: Option<(ObjectId, crate::game_logic::Team, Vec3, u32)> = None;
        let mut spawn_scud_poison: Vec<(
            ObjectId,
            crate::game_logic::Team,
            Vec3,
            u32,
            ScudStormAnthraxTier,
        )> = Vec::new();
        let mut spawn_orbit: Option<(
            ObjectId,
            crate::game_logic::Team,
            Vec3,
            u32,
            SpectreGunshipScienceTier,
        )> = None;
        let mut spawn_beam: Option<(ObjectId, crate::game_logic::Team, Vec3, u32, bool)> = None;
        if let Some(strike) = self.strikes.get_mut(&strike_id) {
            if strike.phase == HostStrikePhase::Queued {
                strike.total_damage_applied = strike.total_damage_applied + total_damage;
                strike.objects_hit = strike.objects_hit.saturating_add(objects_hit);
                strike.objects_destroyed =
                    strike.objects_destroyed.saturating_add(objects_destroyed);
                strike.multi_strike_applied = strike
                    .multi_strike_applied
                    .saturating_add(wave_shell_count.max(1));
                let shells = wave_shell_count.max(1);
                // Wave 56: CarpetBomb FireFX residual per bomb wave.
                if strike.kind == HostSuperweaponKind::CarpetBomb {
                    strike.carpet_fire_fx_applications =
                        strike.carpet_fire_fx_applications.saturating_add(shells);
                }
                // Wave 56: ArtilleryBarrage detonation FX residual per shell wave.
                if strike.kind == HostSuperweaponKind::ArtilleryBarrage {
                    strike.artillery_fire_fx_applications =
                        strike.artillery_fire_fx_applications.saturating_add(shells);
                }
                // Wave 56: CruiseMissile MOAB primary + flame + FireFX residual.
                if strike.kind == HostSuperweaponKind::CruiseMissile {
                    strike.cruise_moab_weapon_applications =
                        strike.cruise_moab_weapon_applications.saturating_add(1);
                    strike.cruise_moab_flame_applications =
                        strike.cruise_moab_flame_applications.saturating_add(1);
                    strike.cruise_moab_fire_fx_applications =
                        strike.cruise_moab_fire_fx_applications.saturating_add(1);
                    strike.cruise_loft_applications =
                        strike.cruise_loft_applications.saturating_add(1);
                    strike.cruise_height_die_applications =
                        strike.cruise_height_die_applications.saturating_add(1);
                    strike.cruise_projectile_applications =
                        strike.cruise_projectile_applications.saturating_add(1);
                }
                // ScudStorm: per-missile LargePoisonField residual (each detonation).
                if strike.kind.spawns_scud_poison_field() {
                    let source = strike.source_object;
                    let team = strike.source_team;
                    let frame = strike.impact_frame;
                    let anthrax = strike.scud_anthrax_tier;
                    // PreAttack ends on first missile wave; FireFX + detonation residual.
                    if strike.scud_pre_attack_active {
                        strike.scud_pre_attack_active = false;
                    }
                    let shells = wave_shell_count.max(1);
                    strike.scud_fire_fx_applications =
                        strike.scud_fire_fx_applications.saturating_add(shells);
                    strike.scud_detonation_fx_applications = strike
                        .scud_detonation_fx_applications
                        .saturating_add(shells);
                    strike.scud_launch_bone_applications =
                        strike.scud_launch_bone_applications.saturating_add(shells);
                    // ScudStormMissile loft residual (MissileAIUpdate / HeightDie /
                    // IgnitionFX / exhaust / SpecialPowerCompletionDie honesty).
                    strike.scud_missile_loft_applications =
                        strike.scud_missile_loft_applications.saturating_add(shells);
                    strike.scud_ignition_fx_applications =
                        strike.scud_ignition_fx_applications.saturating_add(shells);
                    strike.scud_launch_sound_applications =
                        strike.scud_launch_sound_applications.saturating_add(shells);
                    strike.scud_exhaust_applications =
                        strike.scud_exhaust_applications.saturating_add(shells);
                    strike.scud_height_die_applications =
                        strike.scud_height_die_applications.saturating_add(shells);
                    strike.scud_special_power_completion_applications = strike
                        .scud_special_power_completion_applications
                        .saturating_add(shells);
                    // PreferredHeight spring residual (Locomotor damping path).
                    // Host residual: spawn at PreferredHeight, spring sample, and
                    // loft phase honesty per missile wave. Fail-closed: not full
                    // live MissileAIUpdate physics / ThingFactory Object.
                    strike.scud_spawn_height_applications =
                        strike.scud_spawn_height_applications.saturating_add(shells);
                    strike.scud_preferred_height_spring_applications = strike
                        .scud_preferred_height_spring_applications
                        .saturating_add(shells);
                    // Sample spring from ground (0) over HeightDie InitialDelay
                    // frames toward PreferredHeight (retail loft climb residual).
                    let spring_h = scud_missile_preferred_height_after_frames(
                        0.0,
                        SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES,
                    );
                    strike.scud_last_spring_height = spring_h;
                    // Ballistic flight residual: locomotor path toward first epicenter
                    // (or strike target) with OnlyWhenMovingDown / SnapToGround honesty.
                    let flight_target = epicenters
                        .first()
                        .copied()
                        .unwrap_or(strike.target_position);
                    // Launch residual near building; host uses target - offset as pad.
                    let launch = Vec3::new(
                        flight_target.x - SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING,
                        0.0,
                        flight_target.z,
                    );
                    // Sample enough frames to cover loft→turn→dive→HeightDie residual.
                    let sample_frames = ((SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING
                        + SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING)
                        / scud_missile_speed_per_frame())
                    .ceil() as u32
                        + SCUD_STORM_MISSILE_HEIGHT_DIE_INITIAL_DELAY_FRAMES;
                    let (flight_pos, flight_dist, _dist_to, flight_phase) =
                        scud_missile_ballistic_sample(launch, flight_target, sample_frames);
                    strike.scud_ballistic_flight_applications = strike
                        .scud_ballistic_flight_applications
                        .saturating_add(shells);
                    strike.scud_only_moving_down_applications = strike
                        .scud_only_moving_down_applications
                        .saturating_add(shells);
                    strike.scud_snap_to_ground_applications = strike
                        .scud_snap_to_ground_applications
                        .saturating_add(shells);
                    strike.scud_model_draw_applications =
                        strike.scud_model_draw_applications.saturating_add(shells);
                    // Geometry residual (Cylinder / radius / height / mass / max health).
                    strike.scud_geometry_applications =
                        strike.scud_geometry_applications.saturating_add(shells);
                    // VisionRange / KindOf / Armor / TransportSlot residual.
                    strike.scud_object_params_applications = strike
                        .scud_object_params_applications
                        .saturating_add(shells);
                    // MissileAIUpdate residual (TryToFollow/Fuel/InitialVel/DistTurning/Diving).
                    strike.scud_missile_ai_applications =
                        strike.scud_missile_ai_applications.saturating_add(shells);
                    // FireWeaponWhenDead death-weapon matrix residual.
                    strike.scud_fire_weapon_when_dead_applications = strike
                        .scud_fire_weapon_when_dead_applications
                        .saturating_add(shells);
                    // InitialHealth / EditorSorting / OkToChangeModelColor residual.
                    strike.scud_body_draw_params_applications = strike
                        .scud_body_draw_params_applications
                        .saturating_add(shells);
                    // Locomotor Surfaces/Appearance/AllowAirborne/Braking residual.
                    strike.scud_locomotor_appearance_applications = strike
                        .scud_locomotor_appearance_applications
                        .saturating_add(shells);
                    // DestroyDie + Locomotor template name + Armor DamageFX residual.
                    strike.scud_destroy_die_locomotor_name_applications = strike
                        .scud_destroy_die_locomotor_name_applications
                        .saturating_add(shells);
                    // DeathWeapon FireOCL PoisonField residual.
                    strike.scud_death_fire_ocl_applications = strike
                        .scud_death_fire_ocl_applications
                        .saturating_add(shells);
                    // Locomotor SpeedDamaged/MinSpeed/MaxThrustAngle residual.
                    strike.scud_locomotor_speed_table_applications = strike
                        .scud_locomotor_speed_table_applications
                        .saturating_add(shells);
                    // DeathWeapon Primary/Secondary damage table residual.
                    strike.scud_death_damage_table_applications = strike
                        .scud_death_damage_table_applications
                        .saturating_add(shells);
                    // ScudStormWeapon launch residual (Clip/Scatter/AutoReload/Collides).
                    strike.scud_weapon_launch_applications = strike
                        .scud_weapon_launch_applications
                        .saturating_add(shells);
                    // ScudStormWeapon special residual (unused Primary/Speed/PreAttackType).
                    strike.scud_weapon_special_applications = strike
                        .scud_weapon_special_applications
                        .saturating_add(shells);
                    // MissileAIUpdate defaults residual (IgnitionDelay / Lock / KillSelf).
                    strike.scud_missile_ai_defaults_applications = strike
                        .scud_missile_ai_defaults_applications
                        .saturating_add(shells);
                    // Wave 74: ScudStormMissile ThingFactory residual spawn
                    // bookkeeping on impact (object pack ledger; not full Object).
                    let _scud_spawn = scud_storm_missile_spawn_residual(frame, flight_target);
                    debug_assert!(honesty_thing_factory_spawn_residual(&_scud_spawn));
                    strike.scud_thing_factory_spawn_applications = strike
                        .scud_thing_factory_spawn_applications
                        .saturating_add(shells);
                    strike.scud_last_flight_distance = flight_dist;
                    if flight_dist > strike.scud_peak_flight_distance {
                        strike.scud_peak_flight_distance = flight_dist;
                    }
                    // Pre-snap height residual lives in spring sample; snap sets Y=0.
                    strike.scud_last_flight_height =
                        if flight_phase == ScudMissileLoftPhase::HeightDie {
                            0.0
                        } else {
                            flight_pos.y
                        };
                    // ThrustRoll / ThrustWobble residual honesty (locomotor thrust path).
                    strike.scud_thrust_wobble_applications = strike
                        .scud_thrust_wobble_applications
                        .saturating_add(shells);
                    let wobble = scud_missile_thrust_wobble(sample_frames);
                    strike.scud_last_thrust_wobble = wobble;
                    let abs_w = wobble.abs();
                    if abs_w > strike.scud_peak_abs_thrust_wobble {
                        strike.scud_peak_abs_thrust_wobble = abs_w;
                    }
                    // Peak loft phase residual: prefer ballistic sample, fall back.
                    let phase = if flight_phase.as_u8() >= ScudMissileLoftPhase::HeightDie.as_u8() {
                        flight_phase
                    } else {
                        scud_missile_loft_phase(
                            SCUD_STORM_MISSILE_DISTANCE_BEFORE_TURNING + 1.0,
                            SCUD_STORM_MISSILE_DISTANCE_BEFORE_DIVING * 0.5,
                            SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET * 0.5,
                        )
                    };
                    if phase.as_u8() > strike.scud_loft_phase_peak.as_u8() {
                        strike.scud_loft_phase_peak = phase;
                    }
                    // Live AttackNugget missiles spawn FireOCL poison on HeightDie.
                    if !strike.live_scud_delivery {
                        if epicenters.is_empty() {
                            spawn_scud_poison.push((
                                source,
                                team,
                                strike.target_position,
                                frame,
                                anthrax,
                            ));
                        } else {
                            for p in epicenters {
                                spawn_scud_poison.push((source, team, *p, frame, anthrax));
                            }
                        }
                    }
                }
                if is_final_wave {
                    strike.phase = HostStrikePhase::Completed;
                    self.completed_this_frame.push(strike_id);
                    // Live flying NeutronMissile owns the one SlowDeath + midpoint OCL.
                    if strike.kind.spawns_radiation() && !strike.live_neutron_delivery {
                        spawn_radiation = Some((
                            strike.source_object,
                            strike.source_team,
                            strike.target_position,
                            strike.impact_frame,
                        ));
                    }
                    // AnthraxBomb toxin (not Scud — Scud already spawned per-missile).
                    // Live falling bomb leftover owns OCL_PoisonFieldAnthraxBomb.
                    if strike.kind.spawns_toxin_field()
                        && !strike.kind.spawns_scud_poison_field()
                        && !strike.live_anthrax_delivery
                    {
                        spawn_toxin = Some((
                            strike.source_object,
                            strike.source_team,
                            strike.target_position,
                            strike.impact_frame,
                        ));
                    }
                    if strike.kind.spawns_orbit_field() {
                        spawn_orbit = Some((
                            strike.source_object,
                            strike.source_team,
                            strike.target_position,
                            strike.impact_frame,
                            strike.spectre_tier,
                        ));
                    }
                    if strike.kind.spawns_beam_field() {
                        // READY_TO_FIRE → FIRING residual on beam spawn.
                        apply_particle_charge_status(strike, strike.impact_frame);
                        if strike.particle_status != ParticleUplinkStatus::Firing {
                            // Force FIRING honesty when beam field is about to spawn.
                            let prev = strike.particle_status;
                            strike.particle_status = ParticleUplinkStatus::Firing;
                            if prev != ParticleUplinkStatus::Firing {
                                strike.particle_intensity_transitions =
                                    strike.particle_intensity_transitions.saturating_add(1);
                            }
                            if strike.particle_status_peak.as_u8()
                                < ParticleUplinkStatus::Firing.as_u8()
                            {
                                strike.particle_status_peak = ParticleUplinkStatus::Firing;
                            }
                            strike.particle_model_deployed_sets =
                                strike.particle_model_deployed_sets.saturating_add(1);
                        }
                        spawn_beam = Some((
                            strike.source_object,
                            strike.source_team,
                            strike.target_position,
                            strike.impact_frame,
                            strike.manual_beam_hold,
                        ));
                    }
                }
            }
        }
        if let Some((source, team, pos, impact_frame)) = spawn_radiation {
            self.spawn_radiation_field(source, team, pos, impact_frame, strike_id);
            // Fallback only: live flying missile already owns SlowDeath.
            self.spawn_neutron_slow_death_field(source, team, pos, impact_frame, strike_id);
        }
        if let Some((source, team, pos, impact_frame)) = spawn_toxin {
            self.spawn_toxin_field(source, team, pos, impact_frame, strike_id);
        }
        for (source, team, pos, impact_frame, anthrax) in spawn_scud_poison {
            self.spawn_scud_poison_field_with_tier(
                source,
                team,
                pos,
                impact_frame,
                strike_id,
                anthrax,
            );
        }
        if let Some((source, team, pos, impact_frame, spectre_tier)) = spawn_orbit {
            self.spawn_orbit_field_with_tier(
                source,
                team,
                pos,
                impact_frame,
                strike_id,
                spectre_tier,
            );
        }
        if let Some((source, team, pos, impact_frame, manual_hold)) = spawn_beam {
            let beam_id = self.spawn_beam_field_with_manual(
                source,
                team,
                pos,
                impact_frame,
                strike_id,
                manual_hold,
            );
            if let Some(strike) = self.strikes.get(&strike_id) {
                if strike.scripted_waypoint_mode {
                    if let Some(field) = self.beam_fields.iter_mut().find(|f| f.id == beam_id) {
                        field.scripted_waypoint_mode = true;
                        field.next_dest_waypoint_id = strike.next_dest_waypoint_id;
                        field.override_destination = strike.waypoint_override;
                        field.current_target_position = pos;
                    }
                }
            }
        }
    }
}
