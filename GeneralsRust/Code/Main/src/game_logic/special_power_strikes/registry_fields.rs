//! Radiation / toxin / orbit / beam / remnant spawn, tick, and prune.
use super::types::*;
use super::*;
impl HostSpecialPowerStrikeRegistry {
    /// Spawn a residual radiation field at `position` (NuclearMissile impact).

    /// C++ NeutronMissileSlowDeathBehavior activation residual at impact.
    pub fn spawn_neutron_slow_death_field(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        use crate::game_logic::host_neutron_missile_slow_death::{
            HostNeutronMissileSlowDeathData, NEUTRON_FX_LIST, NEUTRON_SCORCH_MARK_SIZE,
        };
        let id = self.next_neutron_slow_death_id;
        self.next_neutron_slow_death_id = self.next_neutron_slow_death_id.saturating_add(1).max(1);
        self.neutron_slow_death_fields
            .push(HostNeutronMissileSlowDeathData::begin(spawn_frame));
        self.neutron_slow_death_meta.push(HostNeutronSlowDeathMeta {
            id,
            source_object,
            source_team,
            position,
            parent_strike_id,
            scorch_size: NEUTRON_SCORCH_MARK_SIZE,
            fx_list: NEUTRON_FX_LIST.into(),
        });
        let _ = crate::game_logic::dispatch_fx_list_at_pos(NEUTRON_FX_LIST, position);
        self.neutron_slow_death_spawned_total =
            self.neutron_slow_death_spawned_total.saturating_add(1);
        id
    }

    pub fn neutron_slow_death_field_count(&self) -> usize {
        self.neutron_slow_death_fields.len()
    }

    pub fn neutron_slow_death_spawned_total(&self) -> u32 {
        self.neutron_slow_death_spawned_total
    }

    pub fn neutron_slow_death_meta(&self) -> &[HostNeutronSlowDeathMeta] {
        &self.neutron_slow_death_meta
    }

    pub fn neutron_slow_death_fields(
        &self,
    ) -> &[crate::game_logic::host_neutron_missile_slow_death::HostNeutronMissileSlowDeathData]
    {
        &self.neutron_slow_death_fields
    }

    pub fn neutron_slow_death_next_id(&self) -> u32 {
        self.next_neutron_slow_death_id
    }

    pub fn neutron_slow_death_fields_mut_for_tick(
        &mut self,
    ) -> Vec<crate::game_logic::host_neutron_missile_slow_death::HostNeutronMissileSlowDeathData>
    {
        std::mem::take(&mut self.neutron_slow_death_fields)
    }

    pub fn restore_neutron_slow_death_fields(
        &mut self,
        fields: Vec<
            crate::game_logic::host_neutron_missile_slow_death::HostNeutronMissileSlowDeathData,
        >,
        metas: Vec<HostNeutronSlowDeathMeta>,
    ) {
        self.neutron_slow_death_fields = fields;
        self.neutron_slow_death_meta = metas;
    }

    pub fn restore_neutron_slow_death_persist(
        &mut self,
        next_id: u32,
        spawned_total: u32,
        fields: Vec<
            crate::game_logic::host_neutron_missile_slow_death::HostNeutronMissileSlowDeathData,
        >,
        metas: Vec<HostNeutronSlowDeathMeta>,
    ) {
        self.next_neutron_slow_death_id = next_id;
        self.neutron_slow_death_spawned_total = spawned_total;
        self.neutron_slow_death_fields = fields;
        self.neutron_slow_death_meta = metas;
    }

    pub fn spawn_radiation_field(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        let id = self.next_radiation_id;
        self.next_radiation_id = self.next_radiation_id.saturating_add(1).max(1);
        let field = HostRadiationField {
            id,
            source_object,
            source_team,
            object_id: None,
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(NUKE_RADIATION_DURATION_FRAMES),
            // First tick on spawn frame (retail FireWeaponUpdate residual).
            next_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
            radiation_residual_pack_armed: 1,
            radiation_suspend_fx_applications: 1,
            radiation_fire_fx_applications: 1,
        };
        self.radiation_fields.push(field);
        self.radiation_spawned_this_frame.push(id);
        self.radiation_fields_spawned_total = self.radiation_fields_spawned_total.saturating_add(1);
        // Wave 56: arm parent strike radiation residual pack honesty.
        if parent_strike_id != 0 {
            if let Some(s) = self.strikes.get_mut(&parent_strike_id) {
                s.nuke_radiation_residual_pack_applications = s
                    .nuke_radiation_residual_pack_applications
                    .saturating_add(1);
            }
        }
        id
    }

    /// Build radiation damage plans for all fields whose tick frame has arrived.
    ///
    /// Retail `NukeRadiationFieldWeapon` hits ALLIES ENEMIES NEUTRALS NOT_AIRBORNE.
    /// Planner still lists living objects in radius except the source launcher.
    /// Apply-time `take_radiation_field_tick` skips airborne / significantly-above
    /// terrain (Weapon.cpp:1351) and uses DAMAGE_RADIATION so Armor.ini applies.
    pub fn plan_due_radiation_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, crate::game_logic::Team, bool)],
    ) -> Vec<HostRadiationTickPlan> {
        let mut plans = Vec::new();
        for field in &self.radiation_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                let dist = horizontal_distance(pos, field.position);
                if dist <= NUKE_RADIATION_RADIUS {
                    hits.push(HostRadiationDamageHit {
                        target_id: id,
                        damage: NUKE_RADIATION_DAMAGE_PER_TICK,
                        field_id: field.id,
                    });
                }
            }
            plans.push(HostRadiationTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: field.position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record radiation tick results and advance next_tick_frame.
    pub fn record_radiation_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(field) = self.radiation_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.next_tick_frame =
                current_frame.saturating_add(NUKE_RADIATION_TICK_INTERVAL_FRAMES);
            self.radiation_damage_applications_total = self
                .radiation_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Drop expired radiation fields.
    pub fn prune_expired_radiation(&mut self, current_frame: u32) {
        self.radiation_fields
            .retain(|f| !f.is_expired(current_frame));
    }

    /// Spawn a residual toxin field at `position` (AnthraxBomb impact defaults).
    pub fn spawn_toxin_field(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        self.spawn_toxin_field_with_params(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            ANTHRAX_TOXIN_DAMAGE_PER_TICK,
            ANTHRAX_TOXIN_RADIUS,
            ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES,
            ANTHRAX_TOXIN_DURATION_FRAMES,
            ANTHRAX_TOXIN_OBJECT_NAME,
        )
    }

    /// Spawn residual LargePoisonField toxin (ScudStorm OCL_PoisonFieldLarge).
    pub fn spawn_scud_poison_field(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        self.spawn_scud_poison_field_with_tier(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            ScudStormAnthraxTier::Base,
        )
    }

    /// Spawn ScudStorm LargePoison residual with anthrax-upgrade tier stats.
    pub fn spawn_scud_poison_field_with_tier(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
        anthrax_tier: ScudStormAnthraxTier,
    ) -> u32 {
        self.spawn_toxin_field_with_params(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            anthrax_tier.poison_damage_per_tick(),
            SCUD_STORM_POISON_RADIUS,
            SCUD_STORM_POISON_TICK_INTERVAL_FRAMES,
            SCUD_STORM_POISON_DURATION_FRAMES,
            anthrax_tier.poison_object_name(),
        )
    }

    /// Spawn a residual toxin field with explicit weapon residual params.
    pub fn spawn_toxin_field_with_params(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
        damage_per_tick: f32,
        radius: f32,
        tick_interval_frames: u32,
        duration_frames: u32,
        object_template: &str,
    ) -> u32 {
        let id = self.next_toxin_id;
        self.next_toxin_id = self.next_toxin_id.saturating_add(1).max(1);
        let field = HostToxinField {
            id,
            source_object,
            source_team,
            object_id: None,
            object_template: object_template.to_string(),
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(duration_frames),
            // First tick on spawn frame (retail FireWeaponUpdate residual).
            next_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
            toxin_residual_pack_armed: 1,
            toxin_fire_fx_applications: 1,
            toxin_damage_type_applications: 1,
            damage_per_tick,
            radius,
            tick_interval_frames,
        };
        self.toxin_fields.push(field);
        self.toxin_spawned_this_frame.push(id);
        self.toxin_fields_spawned_total = self.toxin_fields_spawned_total.saturating_add(1);
        // Wave 56: arm parent AnthraxBomb residual pack honesty (Scud also uses toxin fields).
        if parent_strike_id != 0 {
            if let Some(s) = self.strikes.get_mut(&parent_strike_id) {
                if s.kind == HostSuperweaponKind::AnthraxBomb {
                    s.anthrax_toxin_residual_pack_applications =
                        s.anthrax_toxin_residual_pack_applications.saturating_add(1);
                }
            }
        }
        id
    }

    /// Build toxin damage plans for all fields whose tick frame has arrived.
    ///
    /// Retail `AnthraxBombPoisonFieldWeapon` hits ALLIES ENEMIES NEUTRALS
    /// NOT_AIRBORNE. Radius splash skips airborne (C++ `WEAPON_DOESNT_AFFECT_AIRBORNE`
    /// / `isSignificantlyAboveTerrain`) unless they are the primary target —
    /// FireWeaponUpdate fires at the field's feet, so aircraft are excluded.
    /// `object_positions`: (id, pos, team, alive, airborne).
    pub fn plan_due_toxin_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, crate::game_logic::Team, bool, bool)],
    ) -> Vec<HostToxinTickPlan> {
        let mut plans = Vec::new();
        for field in &self.toxin_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            let death_type = toxin_field_death_type_for_template(&field.object_template);
            let mut hits = Vec::new();
            for &(id, pos, _team, alive, airborne) in object_positions {
                if !alive || id == field.source_object || airborne {
                    continue;
                }
                let dist = horizontal_distance(pos, field.position);
                if dist <= field.radius {
                    hits.push(HostToxinDamageHit {
                        target_id: id,
                        damage: field.damage_per_tick,
                        field_id: field.id,
                    });
                }
            }
            plans.push(HostToxinTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: field.position,
                hits,
                death_type,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record toxin tick results and advance next_tick_frame.
    pub fn record_toxin_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(field) = self.toxin_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.next_tick_frame = current_frame.saturating_add(field.tick_interval_frames.max(1));
            self.toxin_damage_applications_total = self
                .toxin_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Drop expired toxin fields.
    pub fn prune_expired_toxin(&mut self, current_frame: u32) {
        self.toxin_fields.retain(|f| !f.is_expired(current_frame));
    }

    /// Spawn a residual Spectre orbit field at `position` (orbit insertion).
    /// Uses default Level2 OrbitTime (15s) when no tier is supplied.
    pub fn spawn_orbit_field(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        self.spawn_orbit_field_with_tier(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            SpectreGunshipScienceTier::Level2,
        )
    }

    /// Spawn Spectre orbit field with science-tier OrbitTime residual.
    pub fn spawn_orbit_field_with_tier(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
        spectre_tier: SpectreGunshipScienceTier,
    ) -> u32 {
        let id = self.next_orbit_id;
        self.next_orbit_id = self.next_orbit_id.saturating_add(1).max(1);
        let duration = spectre_tier.orbit_duration_frames();
        let field = HostSpectreOrbitField {
            id,
            source_object,
            source_team,
            position,
            override_destination: position,
            gattling_target_position: position,
            position_to_shoot_at: position,
            ok_to_fire_howitzer_counter: 0,

            spawn_frame,
            expires_frame: spawn_frame.saturating_add(duration),
            // First howitzer residual tick on orbit insertion frame.
            next_tick_frame: spawn_frame,
            // First gattling residual tick on orbit insertion frame.
            next_gattling_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
            howitzer_ticks: 0,
            gattling_ticks: 0,
            gattling_consecutive: 0,
            howitzer_consecutive: 0,
            gattling_fire_level: 0,
            howitzer_fire_level: 0,
            gattling_coast_until_frame: 0,
            howitzer_coast_until_frame: 0,
            gattling_coast_applications: 0,
            howitzer_coast_applications: 0,
            rapid_fire_voice_cues: 0,
            model_condition_mean_sets: 0,
            model_condition_fast_sets: 0,
            model_condition_slow_sets: 0,
            howitzer_shells_spawned: 0,
            howitzer_shell_fire_fx: 0,
            howitzer_shell_detonation_fx: 0,
            howitzer_shell_height_die_delays: 0,
            howitzer_shell_fire_sounds: 0,
            howitzer_shell_dumb_projectile_applications: 0,
            howitzer_shell_physics_mass_applications: 0,
            howitzer_shell_death_detonated_applications: 0,
            howitzer_shell_death_lasered_applications: 0,
            howitzer_shell_death_lasered_ocl_applications: 0,
            howitzer_shell_death_generic_applications: 0,
            howitzer_shell_object_params_applications: 0,
            howitzer_shell_design_params_applications: 0,
            howitzer_shell_only_moving_down_applications: 0,
            howitzer_shell_model_draw_applications: 0,
            howitzer_shell_scale_applications: 0,
            howitzer_shell_shadow_applications: 0,
            howitzer_shell_geometry_applications: 0,
            howitzer_shell_max_health_applications: 0,
            howitzer_shell_loft_flight_applications: 0,
            howitzer_shell_last_loft_height: 0.0,
            howitzer_shell_loft_height_die_applications: 0,
            howitzer_shell_locomotor_template_applications: 0,
            howitzer_shell_damage_fx_applications: 0,
            howitzer_shell_thing_factory_spawn_applications: 0,
            howitzer_gun_aim_params_applications: 0,
            howitzer_gun_fire_params_applications: 0,
            howitzer_gun_anti_params_applications: 0,
            gattling_gun_params_applications: 0,
            gattling_rof_mean_applications: 0,
            gattling_rof_fast_applications: 0,
            // Residual stand-in: gunship on the orbit ring so gattling acquire
            // is not fail-closed before the host binds the live ship position.
            gunship_position: Some(Vec3::new(
                position.x + SPECTRE_GUNSHIP_ORBIT_RADIUS,
                position.y,
                position.z,
            )),
        };
        self.orbit_fields.push(field);
        self.orbit_spawned_this_frame.push(id);
        self.orbit_fields_spawned_total = self.orbit_fields_spawned_total.saturating_add(1);
        id
    }

    /// Build Spectre orbit damage plans for all fields whose tick frame has arrived.
    ///
    /// Wave 13 dual residual:
    /// - Howitzer (`SpectreHowitzerGun`): PrimaryDamage **80** in PrimaryDamageRadius
    ///   **25** around reticle + deterministic RandomOffsetForHowitzer residual.
    /// - Gattling (`SpectreGattlingGun`): PrimaryDamage **90** to nearest living
    ///   enemy in TargetingReticleRadius **25**. Wide AttackAreaRadius **200**
    ///   auto-acquire is AI-only (C++ SpectreGunshipUpdate.cpp:530-556).
    /// Both exclude source launcher and same-team friendlies.
    /// Continuous-fire ROF residual advances on record_orbit_tick_complete.

    /// SpectreHowitzerShell projectile residual honesty is recorded on each
    /// howitzer tick (not full DumbProjectileBehavior Object / HeightDie flight).
    pub fn plan_due_orbit_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, crate::game_logic::Team, bool)],
    ) -> Vec<HostSpectreOrbitTickPlan> {
        let mut plans = Vec::new();
        for field in &self.orbit_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            let howitzer_due = field.is_due_howitzer(current_frame);
            let gattling_due = field.is_due_gattling(current_frame);
            // Accumulate damage per target (howitzer AOE + gattling single-target).
            let mut dmg_map: std::collections::BTreeMap<ObjectId, f32> =
                std::collections::BTreeMap::new();

            if howitzer_due && field.howitzer_follow_ready() {
                let off = spectre_howitzer_offset(field.howitzer_ticks);
                let aim = field.gattling_aim();
                let epicenter = Vec3::new(aim.x + off.x, aim.y, aim.z + off.z);

                for &(id, pos, team, alive) in object_positions {
                    if !alive || id == field.source_object || team == field.source_team {
                        continue;
                    }
                    let dist = horizontal_distance(pos, epicenter);
                    if dist <= SPECTRE_HOWITZER_RADIUS {
                        *dmg_map.entry(id).or_insert(0.0) += SPECTRE_ORBIT_DAMAGE_PER_TICK;
                    }
                }
            }

            if gattling_due {
                // C++: reticle (override) first; wide AttackAreaRadius only if
                // controlling player is not PLAYER_HUMAN.
                let cands: Vec<_> = object_positions
                    .iter()
                    .filter(|&&(id, _, team, alive)| {
                        alive && id != field.source_object && team != field.source_team
                    })
                    .map(|&(id, pos, team, _)| {
                        crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                            id,
                            team,
                            position: pos,
                            is_alive: true,
                            is_neutral: false,
                            under_construction: false,
                            combat_kind: true,
                            effectively_stealthed: false,
                            is_air: false,
                            eject_invulnerable: false,
                        }
                    })
                    .collect();
                let reticle_aim = field.override_aim();
                let reticle_origin = (reticle_aim.x, reticle_aim.z);
                let wide_origin = (field.position.x, field.position.z);
                let fair =
                    |c: &crate::game_logic::host_residual_acquire::ResidualAcquireCandidate| {
                        spectre_is_fair_distance_from_ship(
                            field.gunship_position,
                            c.position,
                            SPECTRE_GUNSHIP_ORBIT_RADIUS,
                        )
                    };
                let reticle =
                    crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                        Some(field.source_object),
                        reticle_origin,
                        cands.iter().copied(),
                        SPECTRE_TARGETING_RETICLE_RADIUS,
                        fair,
                    );
                let picked = reticle.or_else(|| {
                    if self.spectre_wide_auto_acquire_allowed(field.source_object) {
                        crate::game_logic::host_residual_acquire::pick_nearest_residual_target_xz(
                            Some(field.source_object),
                            wide_origin,
                            cands,
                            SPECTRE_ORBIT_RADIUS,
                            fair,
                        )
                    } else {
                        None
                    }
                });

                if let Some((id, _, _)) = picked {
                    *dmg_map.entry(id).or_insert(0.0) += SPECTRE_GATTLING_DAMAGE;
                }
            }

            let hits: Vec<HostSpectreOrbitDamageHit> = dmg_map
                .into_iter()
                .filter(|(_, d)| *d > 0.0)
                .map(|(target_id, damage)| HostSpectreOrbitDamageHit {
                    target_id,
                    damage,
                    field_id: field.id,
                })
                .collect();
            plans.push(HostSpectreOrbitTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: field.position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record Spectre orbit tick results and advance howitzer/gattling timers.
    pub fn record_orbit_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        // Apply ContinuousFireCoast cool-down before arming new shots this frame.
        self.apply_orbit_coast_cooldown(current_frame);
        let mut shell_spawn_evt: Option<(ObjectId, crate::game_logic::Team, Vec3)> = None;
        if let Some(field) = self.orbit_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            // Advance whichever residual streams were due this frame.
            // Continuous-fire residual: consecutive shot counters raise ROF
            // (gattling 200%/300%, howitzer 150%/200%) after ContinuousFireOne/Two.
            // ContinuousFireCoast residual arms spin-down deadline after each shot.
            if current_frame >= field.next_tick_frame {
                let ready = field.howitzer_follow_ready();
                if ready {
                    field.howitzer_consecutive = field.howitzer_consecutive.saturating_add(1);
                }
                let interval = spectre_howitzer_interval_frames(field.howitzer_consecutive).max(1);
                field.next_tick_frame = current_frame.saturating_add(interval);
                if ready {
                    field.howitzer_ticks = field.howitzer_ticks.saturating_add(1);
                    // SpectreHowitzerShell projectile residual + Object spawn request.
                    // Retail: ProjectileObject=SpectreHowitzerShell, FireFX, detonation
                    // FX, FireSound, HeightDie InitialDelay pad-safe loft residual.
                    field.howitzer_shells_spawned = field.howitzer_shells_spawned.saturating_add(1);
                    let off = spectre_howitzer_offset(field.howitzer_ticks.saturating_sub(1));
                    let aim = field.gattling_aim();
                    shell_spawn_evt = Some((
                        field.source_object,
                        field.source_team,
                        Vec3::new(aim.x + off.x, aim.y + 80.0, aim.z + off.z),
                    ));
                    field.howitzer_shell_fire_fx = field.howitzer_shell_fire_fx.saturating_add(1);
                    field.howitzer_shell_detonation_fx =
                        field.howitzer_shell_detonation_fx.saturating_add(1);
                    field.howitzer_shell_height_die_delays =
                        field.howitzer_shell_height_die_delays.saturating_add(1);
                    field.howitzer_shell_fire_sounds =
                        field.howitzer_shell_fire_sounds.saturating_add(1);
                    // DumbProjectileBehavior + Physics mass + InstantDeath + HeightDie
                    // OnlyWhenMovingDown residual honesty (not full W3D shell Object).
                    field.howitzer_shell_dumb_projectile_applications = field
                        .howitzer_shell_dumb_projectile_applications
                        .saturating_add(1);
                    field.howitzer_shell_physics_mass_applications = field
                        .howitzer_shell_physics_mass_applications
                        .saturating_add(1);
                    field.howitzer_shell_death_detonated_applications = field
                        .howitzer_shell_death_detonated_applications
                        .saturating_add(1);
                    field.howitzer_shell_death_lasered_applications = field
                        .howitzer_shell_death_lasered_applications
                        .saturating_add(1);
                    field.howitzer_shell_death_lasered_ocl_applications = field
                        .howitzer_shell_death_lasered_ocl_applications
                        .saturating_add(1);
                    field.howitzer_shell_death_generic_applications = field
                        .howitzer_shell_death_generic_applications
                        .saturating_add(1);
                    field.howitzer_shell_design_params_applications = field
                        .howitzer_shell_design_params_applications
                        .saturating_add(1);
                    field.howitzer_shell_object_params_applications = field
                        .howitzer_shell_object_params_applications
                        .saturating_add(1);
                    // SpectreHowitzerShellLocomotor template + Armor DamageFX residual.
                    field.howitzer_shell_locomotor_template_applications = field
                        .howitzer_shell_locomotor_template_applications
                        .saturating_add(1);
                    field.howitzer_shell_damage_fx_applications = field
                        .howitzer_shell_damage_fx_applications
                        .saturating_add(1);
                    // Wave 74: SpectreHowitzerShell ThingFactory residual spawn
                    // bookkeeping (object pack ledger; not full shell Object).
                    let _shell_spawn = spectre_howitzer_shell_spawn_residual(
                        current_frame,
                        aim + Vec3::new(0.0, 80.0, 0.0),
                    );
                    debug_assert!(honesty_thing_factory_spawn_residual(&_shell_spawn));
                    field.howitzer_shell_thing_factory_spawn_applications = field
                        .howitzer_shell_thing_factory_spawn_applications
                        .saturating_add(1);
                    // SpectreHowitzerGun AcceptableAimDelta / AttackRange residual.
                    field.howitzer_gun_aim_params_applications =
                        field.howitzer_gun_aim_params_applications.saturating_add(1);
                    // SpectreHowitzerGun fire residual (Delay/DamageType/FireFX/Clip).
                    field.howitzer_gun_fire_params_applications = field
                        .howitzer_gun_fire_params_applications
                        .saturating_add(1);
                    // SpectreHowitzerGun anti residual (AntiAir*/ProjectileObject/Coast).
                    field.howitzer_gun_anti_params_applications = field
                        .howitzer_gun_anti_params_applications
                        .saturating_add(1);
                    field.howitzer_shell_only_moving_down_applications = field
                        .howitzer_shell_only_moving_down_applications
                        .saturating_add(1);
                    // W3D ModelDraw / Scale / Shadow / Geometry / MaxHealth residual
                    // (fail-closed vs full ThingFactory Object / live Physics flight).
                    field.howitzer_shell_model_draw_applications = field
                        .howitzer_shell_model_draw_applications
                        .saturating_add(1);
                    field.howitzer_shell_scale_applications =
                        field.howitzer_shell_scale_applications.saturating_add(1);
                    field.howitzer_shell_shadow_applications =
                        field.howitzer_shell_shadow_applications.saturating_add(1);
                    field.howitzer_shell_geometry_applications =
                        field.howitzer_shell_geometry_applications.saturating_add(1);
                    field.howitzer_shell_max_health_applications = field
                        .howitzer_shell_max_health_applications
                        .saturating_add(1);
                    // Shell loft flight residual (pad-safe HeightDie InitialDelay path).
                    let spawn = aim + Vec3::new(0.0, 80.0, 0.0);
                    let target = aim;
                    let loft_frames = SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES + 15;
                    let (loft_pos, _moving_down, height_die) =
                        howitzer_shell_loft_sample(spawn, target, loft_frames);
                    field.howitzer_shell_loft_flight_applications = field
                        .howitzer_shell_loft_flight_applications
                        .saturating_add(1);
                    field.howitzer_shell_last_loft_height = loft_pos.y;
                    if height_die {
                        field.howitzer_shell_loft_height_die_applications = field
                            .howitzer_shell_loft_height_die_applications
                            .saturating_add(1);
                    }
                    field.howitzer_coast_until_frame =
                        spectre_coast_until_after_shot(current_frame, interval);
                    let prev_level = field.howitzer_fire_level;
                    if field.howitzer_consecutive > SPECTRE_HOWITZER_CONTINUOUS_FIRE_TWO {
                        field.howitzer_fire_level = 2;
                    } else if field.howitzer_consecutive > SPECTRE_HOWITZER_CONTINUOUS_FIRE_ONE {
                        field.howitzer_fire_level = field.howitzer_fire_level.max(1);
                    }
                    // VoiceRapidFire residual when entering FAST (FiringTracker::speedUp).
                    if prev_level < 2 && field.howitzer_fire_level == 2 {
                        field.rapid_fire_voice_cues = field.rapid_fire_voice_cues.saturating_add(1);
                    }
                    // MODELCONDITION_CONTINUOUS_FIRE_* residual (FiringTracker::speedUp).
                    if prev_level < 1 && field.howitzer_fire_level >= 1 {
                        field.model_condition_mean_sets =
                            field.model_condition_mean_sets.saturating_add(1);
                    }
                    if prev_level < 2 && field.howitzer_fire_level == 2 {
                        field.model_condition_fast_sets =
                            field.model_condition_fast_sets.saturating_add(1);
                    }
                }
            }
            if current_frame >= field.next_gattling_tick_frame {
                field.gattling_consecutive = field.gattling_consecutive.saturating_add(1);
                let interval = spectre_gattling_interval_frames(field.gattling_consecutive);
                field.next_gattling_tick_frame = current_frame.saturating_add(interval);
                field.gattling_ticks = field.gattling_ticks.saturating_add(1);
                // SpectreGattlingGun anti/fire residual (Anti*/ProjectileObject NONE/Clip).
                field.gattling_gun_params_applications =
                    field.gattling_gun_params_applications.saturating_add(1);
                // ContinuousFire WeaponBonus ROF residual applications: the interval
                // just computed used MEAN (200%) or FAST (300%) when consecutive
                // crosses One/Two thresholds (exclusive `>`).
                if field.gattling_consecutive > SPECTRE_GATTLING_CONTINUOUS_FIRE_TWO {
                    field.gattling_rof_fast_applications =
                        field.gattling_rof_fast_applications.saturating_add(1);
                } else if field.gattling_consecutive > SPECTRE_GATTLING_CONTINUOUS_FIRE_ONE {
                    field.gattling_rof_mean_applications =
                        field.gattling_rof_mean_applications.saturating_add(1);
                }
                field.gattling_coast_until_frame =
                    spectre_coast_until_after_shot(current_frame, interval);
                let prev_level = field.gattling_fire_level;
                if field.gattling_consecutive > SPECTRE_GATTLING_CONTINUOUS_FIRE_TWO {
                    field.gattling_fire_level = 2;
                } else if field.gattling_consecutive > SPECTRE_GATTLING_CONTINUOUS_FIRE_ONE {
                    field.gattling_fire_level = field.gattling_fire_level.max(1);
                }
                // VoiceRapidFire residual when entering FAST (FiringTracker::speedUp).
                if prev_level < 2 && field.gattling_fire_level == 2 {
                    field.rapid_fire_voice_cues = field.rapid_fire_voice_cues.saturating_add(1);
                }
                // MODELCONDITION_CONTINUOUS_FIRE_* residual (FiringTracker::speedUp).
                if prev_level < 1 && field.gattling_fire_level >= 1 {
                    field.model_condition_mean_sets =
                        field.model_condition_mean_sets.saturating_add(1);
                }
                if prev_level < 2 && field.gattling_fire_level == 2 {
                    field.model_condition_fast_sets =
                        field.model_condition_fast_sets.saturating_add(1);
                }
            }
            self.orbit_damage_applications_total = self
                .orbit_damage_applications_total
                .saturating_add(applications);
        }
        if let Some(evt) = shell_spawn_evt {
            self.howitzer_shell_spawns_this_frame.push(evt);
        }
    }

    /// Apply FiringTracker ContinuousFireCoast residual to all orbit fields.
    ///
    /// Retail: after ContinuousFireCoast (2000 ms / 60 frames) without a shot past
    /// the next possible fire frame, coolDown() zeros consecutive shots and clears
    /// MEAN/FAST ROF bonuses. Host residual applies the same spin-down to both
    /// gattling and howitzer streams independently.
    pub fn apply_orbit_coast_cooldown(&mut self, current_frame: u32) {
        for field in &mut self.orbit_fields {
            if let Some((consec, level)) = spectre_coast_spin_down(
                current_frame,
                field.gattling_coast_until_frame,
                field.gattling_fire_level,
                field.gattling_consecutive,
            ) {
                // MODELCONDITION_CONTINUOUS_FIRE_SLOW residual on coolDown.
                if field.gattling_fire_level > 0 {
                    field.model_condition_slow_sets =
                        field.model_condition_slow_sets.saturating_add(1);
                }
                field.gattling_consecutive = consec;
                field.gattling_fire_level = level;
                field.gattling_coast_until_frame = 0;
                field.gattling_coast_applications =
                    field.gattling_coast_applications.saturating_add(1);
            }
            if let Some((consec, level)) = spectre_coast_spin_down(
                current_frame,
                field.howitzer_coast_until_frame,
                field.howitzer_fire_level,
                field.howitzer_consecutive,
            ) {
                if field.howitzer_fire_level > 0 {
                    field.model_condition_slow_sets =
                        field.model_condition_slow_sets.saturating_add(1);
                }
                field.howitzer_consecutive = consec;
                field.howitzer_fire_level = level;
                field.howitzer_coast_until_frame = 0;
                field.howitzer_coast_applications =
                    field.howitzer_coast_applications.saturating_add(1);
            }
        }
    }

    pub fn prune_expired_orbit(&mut self, current_frame: u32) {
        self.apply_orbit_coast_cooldown(current_frame);
        self.orbit_fields.retain(|f| !f.is_expired(current_frame));
    }

    /// Spawn a residual Particle Uplink continuous beam field at `position`.
    /// Direct callers (AI residual / tests) default to SwathOfDeath.
    pub fn spawn_beam_field(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
    ) -> u32 {
        self.spawn_beam_field_with_manual(
            source_object,
            source_team,
            position,
            spawn_frame,
            parent_strike_id,
            false,
        )
    }

    /// Spawn a beam. `manual_target_mode` is C++ `m_manualTargetMode` at fire
    /// (`TRUE` for human click — hold at dest; `FALSE` for script/AI swath).
    pub fn spawn_beam_field_with_manual(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_strike_id: u32,
        manual_target_mode: bool,
    ) -> u32 {
        let id = self.next_beam_id;
        self.next_beam_id = self.next_beam_id.saturating_add(1).max(1);
        let field = HostParticleBeamField {
            id,
            source_object,
            source_team,
            object_id: None,
            connector_object_ids: Vec::new(),
            position,
            source_position: Vec3::ZERO,
            source_axis_set: false,
            spawn_frame,
            // Orbital death after TotalFiringTime + WidthGrow decay tail
            // (retail orbitalDeathFrame = orbitalDecayStart + widthGrowFrames).
            expires_frame: particle_death_frame(spawn_frame),
            // First damage pulse on beam-start frame (retail m_nextDamagePulseFrame = now).
            next_tick_frame: spawn_frame,
            pulses_made: 0,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_strike_id,
            last_swath_position: position,
            max_swath_offset: 0.0,
            swath_applications: 0,
            // First scorch/reveal on beam-start frame (retail m_nextScorchMarkFrame = now).
            next_scorch_frame: spawn_frame,
            scorch_marks_made: 0,
            reveal_applications: 0,
            ground_hit_fx_applications: 0,
            peak_width_scalar: 0.0,
            last_damage_radius: 0.0,
            last_width_scalar: 0.0,
            trough_width_scalar: 1.0,
            decay_samples: 0,
            last_scorch_position: position,
            last_scorch_radius: 0.0,
            // Human fire starts in manual hold (override == current == click).
            // Script/AI residual keeps swath until set_beam_override_destination.
            manual_target_mode,
            override_destination: position,
            current_target_position: position,
            last_driving_click_frame: 0,
            second_last_driving_click_frame: 0,
            last_drive_update_frame: spawn_frame,
            manual_drive_distance_total: 0.0,
            manual_drive_applications: 0,
            fast_drive_applications: 0,
            scripted_waypoint_mode: false,
            next_dest_waypoint_id: 0,
            // STATUS_FIRING client residual: Intense outer nodes + connector
            // lasers + laser-base flare + ground-to-orbit orbital laser.
            // Fail-closed: not full bone extract / drawable ThingFactory lasers.
            outer_node_systems_created: PARTICLE_OUTER_EFFECT_NUM_BONES,
            connector_lasers_created: PARTICLE_OUTER_EFFECT_NUM_BONES,
            laser_base_flare_created: 1,
            ground_to_orbit_laser_created: 1,
            status: ParticleUplinkStatus::Firing,
            outer_intensity: ParticleIntensity::Intense,
            connector_intensity: ParticleIntensity::Intense,
            laser_base_intensity: ParticleIntensity::Intense,
            // First BeamLaunchFX on STATUS_FIRING entry (retail m_nextLaunchFXFrame = 0).
            beam_launch_fx_applications: 1,
            next_launch_fx_frame: spawn_frame.saturating_add(PARTICLE_LAUNCH_FX_INTERVAL_FRAMES),
            postfire_applications: 0,
            packing_applications: 0,
            intensity_transitions: 1, // Idle/Ready → Firing on spawn
            connector_flare_created: 1,
            peak_outer_beam_draw_width: 0.0,
            last_outer_beam_draw_width: 0.0,
            peak_retail_laser_radius: 0.0,
            last_retail_laser_radius: 0.0,
            peak_retail_damage_radius: 0.0,
            last_retail_damage_radius: 0.0,
            // Orbital laser W3DLaserDraw params + Intense connector OuterBeamWidth.
            orbital_laser_draw_params_armed: 1,
            connector_outer_beam_width_armed: 1,
            // Multi-beam NumBeams + TilingScalar residual armed at STATUS_FIRING.
            num_beams_armed: PARTICLE_ORBITAL_LASER_NUM_BEAMS,
            tiling_scalar_armed: 1,
            last_scroll_uv: 0.0,
            peak_abs_scroll_uv: 0.0,
            scroll_uv_samples: 0,
            // Soft-edge color residual armed (Inner/Outer color constants).
            soft_edge_samples: 0,
            peak_soft_edge_outer_width: 0.0,
            last_soft_edge_outer_width: 0.0,
            last_soft_edge_outer_alpha: 0.0,
            last_soft_edge_tile_factor: 0.0,
            soft_edge_color_armed: 1,
            soft_edge_premul_samples: 0,
            last_soft_edge_premul_outer_r: 0.0,
            connector_soft_edge_premul_samples: 0,
            last_connector_soft_edge_premul_outer_r: 0.0,
            orbital_kindof_immobile_armed: 1,
            orbital_segments_armed: PARTICLE_ORBITAL_LASER_SEGMENTS,
            orbital_arc_height_armed: 1,
            // Connector KindOf IMMOBILE + Segments/MaxIntensity/Fade/Tile residual.
            connector_kindof_immobile_armed: 1,
            connector_segments_armed: PARTICLE_CONNECTOR_SEGMENTS,
            connector_max_intensity_fade_armed: 1,
            connector_tile_no_armed: 1,
            // Outer-node bone layout residual (FX01..FX05 ring + connector).
            // Fail-closed: not full W3D bone-world extract.
            outer_node_bone_layout_applications: PARTICLE_OUTER_EFFECT_NUM_BONES,
            last_outer_node_bone_position: particle_outer_node_bone_position(position, 0),
            connector_bone_layout_applications: 1,
            // Intense connector soft-edge + laser segments residual.
            connector_soft_edge_armed: 1,
            peak_connector_soft_edge_outer_width: particle_connector_intense_soft_edge_width(
                PARTICLE_CONNECTOR_INTENSE_NUM_BEAMS.saturating_sub(1),
            ),
            connector_laser_segments_created: PARTICLE_OUTER_EFFECT_NUM_BONES,
            last_connector_segment_start: particle_connector_laser_segment(position, 0).0,
            last_connector_segment_end: particle_connector_laser_segment(position, 0).1,
            // Medium connector soft-edge residual (armed when POSTFIRE intensity hits Medium).
            medium_connector_soft_edge_armed: 0,
            peak_medium_connector_soft_edge_outer_width: 0.0,
            // OrbitalLaser VisionRange / ShroudClearing residual (design params).
            orbital_vision_shroud_armed: 1,
            last_orbital_vision_range: PARTICLE_ORBITAL_LASER_VISION_RANGE,
            last_orbital_shroud_clearing_range: PARTICLE_ORBITAL_LASER_SHROUD_CLEARING_RANGE,
            // KindOf IMMOBILE + Segments=1 + ArcHeight=0 residual armed at STATUS_FIRING.
            // LaserUpdate client residual: initLaser ground-to-orbit + orbit-to-target
            // with WidthGrow sizeDeltaFrames. Fail-closed: not full drawable GPU.
            laser_update_init_applications: 2, // ground-to-orbit + orbit-to-target
            laser_update_dirty: true,
            laser_update_growth_frames: PARTICLE_WIDTH_GROW_FRAMES,
            laser_update_current_width_scalar: 0.0, // widening starts at 0
            laser_update_widening: PARTICLE_WIDTH_GROW_FRAMES > 0,
            laser_update_decaying: false,
            last_laser_update_start: particle_orbit_to_target_laser_segment(position).0,
            last_laser_update_end: particle_orbit_to_target_laser_segment(position).1,
            last_laser_update_drawable_mid: {
                let (s, e) = particle_orbit_to_target_laser_segment(position);
                laser_update_drawable_midpoint(s, e)
            },
            last_laser_update_radius: 0.0,
            // STATUS_FIRING sound residual: GroundAnnihilation + FiringToPack loops.
            // Fail-closed: not full Miles 3D positional loop / stop on POSTFIRE.
            ground_annihilation_audio_applications: 1,
            firing_to_pack_audio_applications: 1,
            // Full sound residual pack names + LaunchFX interval + GroundHitFX.
            sound_residual_pack_armed: 1,
            // ScorchMarkScalar + TotalScorchMarks residual pack armed at spawn.
            scorch_scalar_pack_armed: 1,
            // OuterNodes Light/Medium/Intense + LaserBase + connector name pack.
            outer_node_flare_pack_armed: 1,
            // SlowDeath / InstantDeath residual pack design params armed.
            death_pack_armed: 1,
            start_decay_frame: particle_decay_start_frame(spawn_frame),
        };
        self.beam_fields.push(field);
        self.beam_spawned_this_frame.push(id);
        // C++ STATUS_FIRING → FiringToPackSoundLoop (setClientStatus :1158-1175).
        // GroundAnnihilation is queued separately on beam_spawned_this_frame.
        self.note_puc_loop_audio(source_object, position, PARTICLE_FIRING_TO_PACK_AUDIO);
        self.beam_fields_spawned_total = self.beam_fields_spawned_total.saturating_add(1);
        id
    }

    /// C++ `m_startDecayFrame = now` when the PUC owner is UNDERPOWERED/EMP/SUBDUED/HACKED.
    pub fn abort_beam_fields_on_owner_disable(
        &mut self,
        current_frame: u32,
        owner_aborts: impl Fn(ObjectId) -> bool,
    ) {
        for field in &mut self.beam_fields {
            if owner_aborts(field.source_object) {
                field.begin_abort_decay(current_frame);
            }
        }
    }

    /// Apply `setSpecialPowerOverridableDestination` residual to a live beam.
    ///
    /// C++: sets `m_overrideTargetDestination`, arms `m_manualTargetMode`, and
    /// records double-click frames for ManualFastDrivingSpeed. Host residual
    /// seeds `current_target_position` from the last swath/click epicenter when
    /// first entering manual mode.
    pub fn set_beam_override_destination(
        &mut self,
        field_id: u32,
        destination: Vec3,
        current_frame: u32,
    ) -> bool {
        if let Some(field) = self.beam_fields.iter_mut().find(|f| f.id == field_id) {
            if field.is_expired(current_frame) {
                return false;
            }
            field.second_last_driving_click_frame = field.last_driving_click_frame;
            field.last_driving_click_frame = current_frame;
            field.override_destination = destination;
            if !field.manual_target_mode {
                // Entering manual: seed from last residual epicenter (swath or click).
                field.current_target_position = if field.swath_applications > 0 {
                    field.last_swath_position
                } else {
                    field.position
                };
                field.last_drive_update_frame = current_frame;
            }
            field.manual_target_mode = true;
            true
        } else {
            false
        }
    }

    /// C++ SpectreGunshipUpdate::setSpecialPowerOverridableDestination residual.
    /// Writes the clamped reticle; never drags `field.position` (orbit epicenter).
    pub fn set_orbit_override_destination(
        &mut self,
        field_id: u32,
        destination: Vec3,
        current_frame: u32,
    ) -> bool {
        if let Some(field) = self.orbit_fields.iter_mut().find(|f| f.id == field_id) {
            if field.is_expired(current_frame) {
                return false;
            }
            field.apply_override_destination(destination);
            true
        } else {
            false
        }
    }

    /// C++ howitzer-rate re-eval: `m_positionToShootAt = m_overrideTargetDestination`.
    /// Call once per logic frame before orbit damage planning.
    pub fn advance_orbit_shoot_at(&mut self, current_frame: u32) {
        for field in &mut self.orbit_fields {
            if field.is_expired(current_frame) {
                continue;
            }
            if field.is_due_howitzer(current_frame) {
                field.refresh_position_to_shoot_at();
            }
        }
    }

    /// C++ gattling wind (every orbiting frame while the residual stream is live).
    /// StrafingIncrement toward shoot-at; FollowLag counter resets while steering.
    pub fn advance_orbit_strafe(&mut self, current_frame: u32) {
        for field in &mut self.orbit_fields {
            if field.is_expired(current_frame) {
                continue;
            }
            field.wind_gattling_aim();
        }
    }

    /// Apply a live Object override click to matching PUC beam / Spectre orbit.
    pub fn apply_source_override_destination(
        &mut self,
        source: ObjectId,
        destination: Vec3,
        current_frame: u32,
    ) -> bool {
        let mut applied = false;
        let beam_ids: Vec<u32> = self
            .beam_fields
            .iter()
            .filter(|field| field.source_object == source && !field.is_expired(current_frame))
            .filter(|field| {
                !field.manual_target_mode
                    || (field.override_destination.x - destination.x).abs() > 1e-4
                    || (field.override_destination.z - destination.z).abs() > 1e-4
            })
            .map(|field| field.id)
            .collect();
        for field_id in beam_ids {
            if self.set_beam_override_destination(field_id, destination, current_frame) {
                applied = true;
            }
        }
        let orbit_ids: Vec<u32> = self
            .orbit_fields
            .iter()
            .filter(|field| field.source_object == source && !field.is_expired(current_frame))
            .filter(|field| {
                let current = field.override_aim();
                (current.x - destination.x).abs() > 1e-4 || (current.z - destination.z).abs() > 1e-4
            })
            .map(|field| field.id)
            .collect();
        for field_id in orbit_ids {
            if self.set_orbit_override_destination(field_id, destination, current_frame) {
                applied = true;
            }
        }
        applied
    }

    /// Advance manual beam positions for all fields in manual-target mode.
    ///
    /// C++ update each frame: move `m_currentTargetPosition` toward override at
    /// ManualDrivingSpeed (or Fast) / LOGICFRAMES_PER_SECOND, clamping so the
    /// step never overshoots. Call once per logic frame before damage planning.
    pub fn advance_manual_beam_drive(&mut self, current_frame: u32) {
        for field in &mut self.beam_fields {
            if !(field.manual_target_mode || field.scripted_waypoint_mode)
                || field.is_expired(current_frame)
            {
                continue;
            }
            let last = field.last_drive_update_frame;
            if current_frame <= last {
                continue;
            }
            let frames = current_frame - last;
            // C++ scriptedWaypointMode always uses ManualFastDrivingSpeed.
            let fast = field.scripted_waypoint_mode
                || particle_is_fast_drive(
                    field.last_driving_click_frame,
                    field.second_last_driving_click_frame,
                );
            let max_step = particle_manual_speed_per_frame(fast) * frames as f32;
            let dx = field.override_destination.x - field.current_target_position.x;
            let dz = field.override_destination.z - field.current_target_position.z;
            let dist = (dx * dx + dz * dz).sqrt();
            // C++: when dist < speed, clamp then pick leftover next link.
            // Movement this frame still uses the old dest vector.
            if field.scripted_waypoint_mode && dist < max_step {
                if let Some(terrain) = gamelogic::helpers::TheTerrainLogic::get() {
                    if let Some((next_id, next_pos)) =
                        terrain.random_outgoing_waypoint_link(field.next_dest_waypoint_id)
                    {
                        field.next_dest_waypoint_id = next_id;
                        field.override_destination =
                            glam::Vec3::new(next_pos.x, next_pos.z, next_pos.y);
                    }
                }
            }
            if dist > 1e-4 {
                let step = max_step.min(dist);
                let scale = step / dist;
                field.current_target_position.x += dx * scale;
                field.current_target_position.z += dz * scale;
                field.manual_drive_distance_total += step;
                field.manual_drive_applications = field.manual_drive_applications.saturating_add(1);
                if fast {
                    field.fast_drive_applications = field.fast_drive_applications.saturating_add(1);
                }
            }
            field.last_drive_update_frame = current_frame;
        }
    }

    /// Stamp each beam's cannon world position so SwathOfDeath rotates onto
    /// building→target (C++ `me->getPosition()` / leftover).
    pub fn bind_beam_source_axes(
        &mut self,
        object_positions: &[(ObjectId, Vec3, crate::game_logic::Team, bool)],
    ) {
        for field in &mut self.beam_fields {
            if let Some((_, pos, _, _)) = object_positions
                .iter()
                .find(|(id, _, _, _)| *id == field.source_object)
            {
                let first_bind = !field.source_axis_set;
                field.bind_source_axis(*pos);
                if first_bind {
                    spawn_particle_outer_node_flares(
                        field.source_object,
                        *pos,
                        field.outer_intensity,
                    );
                    play_particle_beam_launch_fx(*pos);
                }
            }
        }
    }

    /// Build Particle Uplink beam pulse plans for all fields whose tick frame
    /// has arrived.
    ///
    /// Retail damages all alive objects in beam radius (DamageRadiusScalar ×
    /// laser radius) at the SwathOfDeath or manual-drive epicenter. C++
    /// `ParticleUplinkCannonUpdate.cpp:636-648` iterates `PartitionFilterAlive`
    /// only — no team filter. Allies, neutrals, and the owner's units take
    /// `damagePerPulse` (35). Host residual damages living objects in
    /// WidthGrow-scaled [`PARTICLE_BEAM_RADIUS`] (**44.2**) around the residual
    /// epicenter, excluding only the source launcher. SwathOfDeath rotates onto
    /// cannon→click when [`bind_beam_source_axes`] has stamped the building.
    /// Fail-closed vs full GPU laser width matrix. WidthGrow
    /// damage-radius residual scales radius 0→full over grow, holds full through
    /// TotalFiringTime, then shrinks full→0 over decay ([`PARTICLE_WIDTH_GROW_FRAMES`]).
    /// Manual driving residual uses override destination when armed.
    /// DamagePulseRemnant trail residual spawns on each completed pulse
    /// ([`spawn_remnant_field`]).
    pub fn plan_due_beam_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, crate::game_logic::Team, bool)],
    ) -> Vec<HostParticleBeamTickPlan> {
        let mut plans = Vec::new();
        for field in &self.beam_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            // SwathOfDeath or manual-drive residual epicenter.
            let epicenter = field.residual_epicenter(field.pulses_made);
            // WidthGrow residual: damage radius ramps with laser width scalar.
            let width_scalar = particle_width_scalar(field.spawn_frame, current_frame);
            let damage_radius = particle_beam_damage_radius(field.spawn_frame, current_frame);
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                let dist = horizontal_distance(pos, epicenter);
                if dist <= damage_radius {
                    hits.push(HostParticleBeamDamageHit {
                        target_id: id,
                        damage: PARTICLE_BEAM_DAMAGE_PER_PULSE,
                        field_id: field.id,
                    });
                }
            }
            plans.push(HostParticleBeamTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: epicenter,
                hits,
                damage_radius,
                width_scalar,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record Particle Uplink beam pulse results and advance next_tick_frame.
    ///
    /// Also spawns a DamagePulseRemnant trail residual at the pulse swath
    /// epicenter (retail ParticleUplinkCannonTrailRemnant).
    pub fn record_beam_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        let mut spawn_remnant: Option<(ObjectId, crate::game_logic::Team, Vec3, u32, u32)> = None;
        if let Some(field) = self.beam_fields.iter_mut().find(|f| f.id == field_id) {
            // Epicenter residual honesty for the pulse that just applied.
            let epicenter = field.residual_epicenter(field.pulses_made);
            if field.manual_target_mode || field.scripted_waypoint_mode {
                // Manual mode: still record last epicenter; swath offset honesty
                // remains 0 (no S-curve while player is driving).
                field.last_swath_position = epicenter;
            } else {
                let offset = particle_swath_offset(field.pulses_made);
                let offset_len = (offset.x * offset.x + offset.z * offset.z).sqrt();
                field.last_swath_position = epicenter;
                if offset_len > field.max_swath_offset {
                    field.max_swath_offset = offset_len;
                }
                if offset_len > 0.01 {
                    field.swath_applications = field.swath_applications.saturating_add(1);
                }
            }

            // WidthGrow grow/hold/decay residual honesty for the pulse that just applied.
            field.sample_width_honesty(current_frame);
            let damage_radius = particle_beam_damage_radius(field.spawn_frame, current_frame);
            field.last_damage_radius = damage_radius;

            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.pulses_made = field.pulses_made.saturating_add(1);
            // Fractional nextFactor scheduling residual (C++ orbital lifetime).
            // Also never schedule in the past relative to current_frame.
            let scheduled = particle_next_pulse_frame(field.spawn_frame, field.pulses_made);
            field.next_tick_frame = scheduled.max(current_frame.saturating_add(1));
            self.beam_damage_applications_total = self
                .beam_damage_applications_total
                .saturating_add(applications);
            // DamagePulseRemnant residual at this pulse's swath epicenter.
            spawn_remnant = Some((
                field.source_object,
                field.source_team,
                epicenter,
                field.id,
                field.parent_strike_id,
            ));
        }
        if let Some((source, team, pos, beam_id, strike_id)) = spawn_remnant {
            self.spawn_remnant_field(source, team, pos, current_frame, beam_id, strike_id);
        }
    }

    /// Spawn a residual DamagePulseRemnant trail field at `position`.
    pub fn spawn_remnant_field(
        &mut self,
        source_object: ObjectId,
        source_team: crate::game_logic::Team,
        position: Vec3,
        spawn_frame: u32,
        parent_beam_id: u32,
        parent_strike_id: u32,
    ) -> u32 {
        let id = self.next_remnant_id;
        self.next_remnant_id = self.next_remnant_id.saturating_add(1).max(1);
        let field = HostParticleRemnantField {
            id,
            source_object,
            source_team,
            object_id: None,
            position,
            spawn_frame,
            expires_frame: spawn_frame.saturating_add(PARTICLE_REMNANT_DURATION_FRAMES),
            // First tick on spawn frame (retail FireWeaponUpdate residual).
            next_tick_frame: spawn_frame,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
            parent_beam_id,
            parent_strike_id,
            // KindOf / ImmortalBody residual armed on spawn.
            remnant_object_params_applications: 1,
            // FireWeaponUpdate + DeletionUpdate residual armed on spawn.
            remnant_fire_deletion_applications: 1,
            // ImmortalBody health-floor residual armed on spawn.
            remnant_immortal_body_applications: 1,
            // Wave 74: TrailRemnant ThingFactory residual spawn bookkeeping.
            remnant_thing_factory_spawn_applications: 1,
        };
        // Wave 74: residual spawn ledger honesty (ImmortalBody/DeletionUpdate closed).
        let _remnant_spawn = trail_remnant_spawn_residual(spawn_frame, position);
        debug_assert!(honesty_thing_factory_spawn_residual(&_remnant_spawn));
        self.remnant_fields.push(field);
        self.remnant_spawned_this_frame.push(id);
        self.remnant_fields_spawned_total = self.remnant_fields_spawned_total.saturating_add(1);
        id
    }

    /// Build remnant trail damage plans for all fields whose tick frame arrived.
    ///
    /// Retail RadiusDamageAffects ALLIES ENEMIES NEUTRALS — host residual damages
    /// all living objects in radius except the source launcher (same as toxin /
    /// poison field residual). Fail-closed vs full Object / ImmortalBody stack.
    pub fn plan_due_remnant_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, crate::game_logic::Team, bool)],
    ) -> Vec<HostParticleRemnantTickPlan> {
        let mut plans = Vec::new();
        for field in &self.remnant_fields {
            if !field.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive) in object_positions {
                if !alive || id == field.source_object {
                    continue;
                }
                let dist = horizontal_distance(pos, field.position);
                if dist <= PARTICLE_REMNANT_RADIUS {
                    hits.push(HostParticleRemnantDamageHit {
                        target_id: id,
                        damage: PARTICLE_REMNANT_DAMAGE_PER_TICK,
                        field_id: field.id,
                    });
                }
            }
            plans.push(HostParticleRemnantTickPlan {
                field_id: field.id,
                source_object: field.source_object,
                source_team: field.source_team,
                position: field.position,
                hits,
            });
        }
        plans.sort_by_key(|p| p.field_id);
        plans
    }

    /// Record remnant trail tick results and advance next_tick_frame.
    pub fn record_remnant_tick_complete(
        &mut self,
        field_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(field) = self.remnant_fields.iter_mut().find(|f| f.id == field_id) {
            field.total_damage_applied += total_damage;
            field.damage_applications += applications;
            field.objects_destroyed += objects_destroyed;
            field.next_tick_frame =
                current_frame.saturating_add(PARTICLE_REMNANT_TICK_INTERVAL_FRAMES.max(1));
            self.remnant_damage_applications_total = self
                .remnant_damage_applications_total
                .saturating_add(applications);
        }
    }

    /// Sample WidthGrow grow/hold/decay honesty for all live beam fields.
    ///
    /// Call each logic frame so decay-tail residual is observed even when no
    /// damage pulses remain (retail LASERSTATUS_DECAYING after TotalFiringTime).
    pub fn sample_beam_width_honesty(&mut self, current_frame: u32) {
        for field in &mut self.beam_fields {
            if !field.is_expired(current_frame) {
                field.sample_width_honesty(current_frame);
            }
        }
    }

    /// Drop expired Particle Uplink beam fields (after WidthGrow decay death).
    pub fn prune_expired_beam(&mut self, current_frame: u32) {
        self.beam_fields.retain(|f| !f.is_expired(current_frame));
    }

    /// Drop expired DamagePulseRemnant trail fields.
    pub fn prune_expired_remnant(&mut self, current_frame: u32) {
        self.remnant_fields.retain(|f| !f.is_expired(current_frame));
    }

    /// CleanupArea residual: remove radiation fields whose epicenter is within
    /// `radius` of `center` (AmbulanceCleanHazardWeapon / HAZARD_CLEANUP residual).
    /// Returns number of fields cleared.
    pub fn clear_radiation_fields_in_radius(&mut self, center: Vec3, radius: f32) -> u32 {
        let before = self.radiation_fields.len();
        self.radiation_fields
            .retain(|f| horizontal_distance(f.position, center) > radius);
        (before.saturating_sub(self.radiation_fields.len())) as u32
    }

    /// CleanupArea residual: remove toxin fields whose epicenter is within
    /// `radius` of `center`. Returns number of fields cleared.
    pub fn clear_toxin_fields_in_radius(&mut self, center: Vec3, radius: f32) -> u32 {
        let before = self.toxin_fields.len();
        self.toxin_fields
            .retain(|f| horizontal_distance(f.position, center) > radius);
        (before.saturating_sub(self.toxin_fields.len())) as u32
    }

    /// Cancel pending strikes owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for strike in self.strikes.values_mut() {
            if strike.source_object == source && strike.phase == HostStrikePhase::Queued {
                strike.phase = HostStrikePhase::Cancelled;
            }
        }
    }
}
