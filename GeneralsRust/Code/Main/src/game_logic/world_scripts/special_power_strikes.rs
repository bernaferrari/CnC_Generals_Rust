//! Host scripts `impl GameLogic` — `special_power_strikes`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! queue/update special power strikes / field effects
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Queue a host residual superweapon strike from DoSpecialPower.
    /// Returns strike id when the power maps to a supported residual kind.
    /// Residual A10 science tier stored on a queued/completed strike.
    /// Residual CarpetBomb faction tier stored on a queued/completed strike.
    pub fn special_power_strike_carpet_tier(
        &self,
        strike_id: u32,
    ) -> Option<crate::game_logic::special_power_strikes::CarpetBombFactionTier> {
        self.special_power_strikes
            .get(strike_id)
            .map(|s| s.carpet_tier)
    }

    pub fn special_power_strike_a10_tier(
        &self,
        strike_id: u32,
    ) -> Option<crate::game_logic::special_power_strikes::A10StrikeScienceTier> {
        self.special_power_strikes
            .get(strike_id)
            .map(|s| s.a10_tier)
    }

    pub fn queue_special_power_strike(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::special_power_strikes::{
            A10StrikeScienceTier, ArtilleryBarrageScienceTier, HostSuperweaponKind,
            ScudStormAnthraxTier, SpectreGunshipScienceTier,
        };
        let kind = HostSuperweaponKind::from_command_power(power)?;
        let source_team = self
            .objects
            .get(&source_object)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let frame = self.frame;
        let sciences: Vec<String> = self
            .players
            .values()
            .filter(|p| p.team == source_team)
            .flat_map(|p| p.unlocked_sciences.iter().cloned())
            .collect();
        // ArtilleryBarrage FormationSize residual from unlocked SCIENCE_ArtilleryBarrage1/2/3.
        let artillery_tier = if kind == HostSuperweaponKind::ArtilleryBarrage {
            ArtilleryBarrageScienceTier::highest_from_sciences(sciences.iter().map(|s| s.as_str()))
        } else {
            ArtilleryBarrageScienceTier::Level1
        };
        // SpectreGunship OrbitTime residual from unlocked SCIENCE_SpectreGunship1/2/3.
        let spectre_tier = if kind == HostSuperweaponKind::SpectreGunship {
            SpectreGunshipScienceTier::highest_from_sciences(sciences.iter().map(|s| s.as_str()))
        } else {
            SpectreGunshipScienceTier::Level2
        };
        // ScudStorm anthrax-upgrade residual from unlocked Anthrax Beta/Gamma.
        let scud_anthrax_tier = if kind == HostSuperweaponKind::ScudStorm {
            ScudStormAnthraxTier::highest_from_upgrades(sciences.iter().map(|s| s.as_str()))
        } else {
            ScudStormAnthraxTier::Base
        };
        // A10 FormationSize residual from unlocked SCIENCE_A10ThunderboltMissileStrike1/2/3.
        let a10_tier = if kind == HostSuperweaponKind::A10Strike {
            A10StrikeScienceTier::highest_from_sciences(sciences.iter().map(|s| s.as_str()))
        } else {
            A10StrikeScienceTier::Level1
        };
        let id = self.special_power_strikes.queue_with_all_tiers(
            kind,
            source_object,
            source_team,
            target_position,
            frame,
            artillery_tier,
            spectre_tier,
            scud_anthrax_tier,
            a10_tier,
        );
        // C++ OCL DeliveryDecal via RadiusDecalUpdate on SCUD Storm host.
        if kind == HostSuperweaponKind::ScudStorm {
            let _ = self.create_delivery_radius_decal(source_object, target_position);
        }
        // C++ SpectreGunshipDeploymentUpdate::initiateIntent residual.
        if kind == HostSuperweaponKind::SpectreGunship {
            let _ = self.initiate_spectre_gunship_deployment(source_object, target_position);
        }
        // C++ OCLSpecialPower::doSpecialPowerAtLocation → ObjectCreationList::create.
        // Dedicated flight spawners already create the DeliverPayload transport
        // (A10 / Daisy / MOAB). Calling execute_ocl here doubled the jets.
        if !matches!(
            kind,
            HostSuperweaponKind::A10Strike | HostSuperweaponKind::DaisyCutter
        ) {
            if let Some(tmpl) =
                crate::game_logic::host_ocl_special_power::special_power_template_for_host_kind(
                    kind.label(),
                )
            {
                let _ = self.execute_ocl_special_power(tmpl, source_object, target_position);
            }
        }
        // C++ CarpetBomb DeliverPayload residual (B52/AirF/China + staggered drops).
        let carpet_flight_tier = if kind == HostSuperweaponKind::CarpetBomb {
            use crate::command_system::SpecialPowerType;
            use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
            Some(
                if matches!(
                    *power,
                    SpecialPowerType::EarlyChinaCarpetBomb | SpecialPowerType::NukeChinaCarpetBomb
                ) {
                    CarpetBombFactionTier::China
                } else if matches!(*power, SpecialPowerType::AirForceCarpetBomb) {
                    CarpetBombFactionTier::AirForce
                } else {
                    CarpetBombFactionTier::highest_from_team_and_sciences(
                        source_team,
                        sciences.iter().map(|s| s.as_str()),
                    )
                },
            )
        } else {
            None
        };
        if let Some(tier) = carpet_flight_tier {
            let _ = self.spawn_carpet_bomb_flight(source_object, target_position, tier);
        }
        // C++ ArtilleryBarrage DeliverPayload residual (cannon + staggered shells).
        if kind == HostSuperweaponKind::ArtilleryBarrage {
            let _ =
                self.spawn_artillery_barrage_flight(source_object, target_position, artillery_tier);
        }
        // C++ A10Thunderbolt DeliverPayload residual (jet + staggered missiles).
        if kind == HostSuperweaponKind::A10Strike {
            let _ = self.spawn_a10_strike_flight(source_object, target_position, a10_tier);
        }
        // C++ DaisyCutter / MOAB DeliverPayload residual (B52 or JetB3 + bomb).
        if kind == HostSuperweaponKind::DaisyCutter {
            use crate::command_system::SpecialPowerType;
            use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
            let tier = match power {
                SpecialPowerType::FuelAirBomb => DaisyFlightPayloadTier::Moab,
                _ => DaisyFlightPayloadTier::DaisyCutter,
            };
            let _ = self.spawn_daisy_cutter_flight(source_object, target_position, tier);
        }
        // C++ AnthraxBomb DeliverPayload residual (GLAJetCargoPlane + bomb).
        if kind == HostSuperweaponKind::AnthraxBomb {
            let _ = self.spawn_anthrax_bomb_flight(source_object, target_position);
        }
        // C++ OCL FireWeaponNugget / AttackNugget residual (Neutron / Cruise / ScudStorm).
        if let Some(nugget) =
            crate::game_logic::host_ocl_fire_weapon_attack::ocl_nugget_for_host_kind(kind.label())
        {
            use crate::game_logic::host_ocl_fire_weapon_attack::OclNuggetKind;
            match nugget {
                OclNuggetKind::FireWeapon(ocl) => {
                    let primary = self
                        .objects
                        .get(&source_object)
                        .map(|o| o.get_position())
                        .unwrap_or(target_position);
                    let _ =
                        self.execute_ocl_fire_weapon(ocl, source_object, primary, target_position);
                }
                OclNuggetKind::Attack(ocl) => {
                    let _ = self.execute_ocl_attack(ocl, source_object, target_position);
                }
            }
        }
        // CarpetBomb faction residual (America / AirForce / China payload matrix).
        if kind == HostSuperweaponKind::CarpetBomb {
            use crate::command_system::SpecialPowerType;
            use crate::game_logic::special_power_strikes::CarpetBombFactionTier;
            let carpet = if matches!(
                *power,
                SpecialPowerType::EarlyChinaCarpetBomb | SpecialPowerType::NukeChinaCarpetBomb
            ) {
                CarpetBombFactionTier::China
            } else if matches!(*power, SpecialPowerType::AirForceCarpetBomb) {
                CarpetBombFactionTier::AirForce
            } else {
                CarpetBombFactionTier::highest_from_team_and_sciences(
                    source_team,
                    sciences.iter().map(|s| s.as_str()),
                )
            };
            let _ =
                self.special_power_strikes
                    .apply_carpet_tier(id, carpet, frame, target_position);
        }

        // C++ SpecialPowerModule SuperweaponLaunched EVA residual.
        self.try_eva_superweapon_launched(source_team, kind);
        // C++ SpecialPowerModule.cpp:513 aboutToDoSpecialPower.
        self.notify_script_engine_special_power_event(source_object, power, true, false);
        // C++ SpecialPowerModule.cpp:454/462 createViewObject (range 250 / 30-40s).
        self.create_special_power_view_object(source_object, target_position, kind);


        // Activation audio residual (observable request path).
        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
        // Launch-site combat particle residual (not full OCL aircraft).
        let _ = self.combat_particles.spawn(
            CombatParticleKind::WeaponMuzzleFlash,
            self.objects
                .get(&source_object)
                .map(|o| o.get_position())
                .unwrap_or(target_position),
            frame,
            Some(source_object),
            None,
        );
        Some(id)
    }

    /// C++ `SpecialPowerModule::createViewObject` — spawn reveal at strike target.
    ///
    /// Retail Superweapon INI ViewObjectRange 250 / ViewObjectDuration 30-40s.
    /// Fail-closed: not full ThingFactory DeletionUpdate object stack.
    pub fn create_special_power_view_object(
        &mut self,
        source_object: ObjectId,
        target_position: Vec3,
        kind: crate::game_logic::special_power_strikes::HostSuperweaponKind,
    ) -> bool {
        use crate::game_logic::special_power_strikes::HostViewObjectReveal;
        use crate::game_logic::{KindOf, ThingTemplate};
        use gamelogic::common::Coord3D;


        const VIEW_OBJECT_TEMPLATE: &str = "SpecialPowerViewObject";
        let range = kind.view_object_range();
        let duration = kind.view_object_duration_frames();
        if range <= 0.0 || duration == 0 {
            return false;
        }

        let (player_id, team) = match self.objects.get(&source_object) {
            Some(obj) => (
                self.player_owner_for_host_object(obj).unwrap_or(0),
                obj.team,
            ),
            None => return false,
        };

        if !self.templates.contains_key(VIEW_OBJECT_TEMPLATE) {
            let mut t = ThingTemplate::new(VIEW_OBJECT_TEMPLATE);
            t.add_kind_of(KindOf::Unattackable)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(VIEW_OBJECT_TEMPLATE.to_string(), t);
        }

        let object_id = self.create_object(VIEW_OBJECT_TEMPLATE, team, target_position);
        if let Some(oid) = object_id {
            if let Some(o) = self.objects.get_mut(&oid) {
                o.shroud_clearing_range = range;
                o.vision_range = range;
                o.note_producer(source_object);
                o.owner_player_id = Some(player_id);
            }
        }

        let world_w = self.world_width.max(1.0);
        let world_h = self.world_height.max(1.0);
        let player_mask = 1u32 << player_id.min(31);
        let center = Coord3D::new(target_position.x, target_position.z, target_position.y);
        let frame = self.frame;
        let fow_reveal_ok = {
            let shroud = get_shroud_manager();
            let mut shroud_mgr = match shroud.lock() {
                Ok(mgr) => mgr,
                Err(_) => {
                    self.special_power_strikes
                        .record_view_object(HostViewObjectReveal {
                            source_object,
                            player_id,
                            position: target_position,
                            range,
                            spawn_frame: frame,
                            expires_frame: frame.saturating_add(duration),
                            object_id,
                            fow_reveal_ok: false,
                        });
                    return object_id.is_some();
                }
            };
            if !shroud_mgr.has_shroud_grid() {
                shroud_mgr.init_shroud_grid(world_w, world_h);
            }
            shroud_mgr.do_shroud_reveal(&center, range, player_mask);
            shroud_mgr.queue_undo_shroud_reveal(&center, range, player_mask, duration, frame);
            let mut visible = shroud_mgr.is_position_visible(player_id.min(31), &center);
            if !visible {
                for bit in 0..32u32 {
                    if (player_mask & (1u32 << bit)) != 0
                        && shroud_mgr.is_position_visible(bit, &center)
                    {
                        visible = true;
                        break;
                    }
                }
            }
            visible
        };

        self.special_power_strikes
            .record_view_object(HostViewObjectReveal {
                source_object,
                player_id,
                position: target_position,
                range,
                spawn_frame: frame,
                expires_frame: frame.saturating_add(duration),
                object_id,
                fow_reveal_ok,
            });
        fow_reveal_ok || object_id.is_some()
    }


    /// Advance pending host superweapon strikes to impact and apply area damage.
    /// NuclearMissile residual also ticks radiation fields after impact.
    /// AnthraxBomb residual also ticks toxin fields after impact.
    /// SpectreGunship residual also ticks orbit fields after orbit insertion.
    /// CarpetBomb residual applies multi-point line damage after approach delay.
    /// ArtilleryBarrage residual applies multi-shell scatter damage after delay.
    /// CruiseMissile residual applies MOAB area damage after loft delay.
    pub fn update_special_power_strikes(&mut self) {
        use crate::game_logic::special_power_strikes::{
            ANTHRAX_TOXIN_AUDIO, NUKE_RADIATION_AUDIO, SPECTRE_ORBIT_AUDIO,
        };

        self.special_power_strikes.clear_frame_events();

        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_impacts(self.frame, &object_positions);

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut objects_hit = 0_u32;
            let mut objects_destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    // BodyModule last_damage_source residual for cash bounty killer
                    // (superweapon blast path — same residual as combat fire).
                    let destroyed =
                        target.take_damage_from_immediate(hit.damage, Some(plan.source_object));
                    total_damage += hit.damage;
                    objects_hit += 1;
                    if destroyed {
                        objects_destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            // Impact feedback residual: explosion particle + audio at epicenter.
            let _ = self.combat_particles.spawn(
                CombatParticleKind::DeathExplosion,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );
            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.impact_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(200),
            );

            self.special_power_strikes.record_impact_wave(
                plan.strike_id,
                total_damage,
                objects_hit,
                objects_destroyed,
                plan.wave_shell_count,
                plan.is_final_wave,
                &plan.epicenters,
            );

            // NuclearMissile residual: radiation field ambient cue on spawn.
            if plan.is_final_wave
                && plan.kind.spawns_radiation()
                && !self
                    .special_power_strikes
                    .radiation_spawned_this_frame()
                    .is_empty()
            {
                self.queue_audio_event(
                    AudioEventRequest::new(NUKE_RADIATION_AUDIO)
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(150),
                );
            }

            // AnthraxBomb final / ScudStorm per-missile residual toxin ambient.
            if plan.kind.spawns_toxin_field()
                && !self
                    .special_power_strikes
                    .toxin_spawned_this_frame()
                    .is_empty()
                && (plan.is_final_wave || plan.kind.spawns_scud_poison_field())
            {
                let cue = if plan.kind.spawns_scud_poison_field() {
                    crate::game_logic::special_power_strikes::SCUD_STORM_POISON_AUDIO
                } else {
                    ANTHRAX_TOXIN_AUDIO
                };
                self.queue_audio_event(
                    AudioEventRequest::new(cue)
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(150),
                );
            }

            // SpectreGunship residual: orbit ambient cue on insertion.
            if plan.is_final_wave
                && plan.kind.spawns_orbit_field()
                && !self
                    .special_power_strikes
                    .orbit_spawned_this_frame()
                    .is_empty()
            {
                self.queue_audio_event(
                    AudioEventRequest::new(SPECTRE_ORBIT_AUDIO)
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(150),
                );
            }

            // ParticleCannon residual: continuous beam annihilation cue on start.
            if plan.is_final_wave
                && plan.kind.spawns_beam_field()
                && !self
                    .special_power_strikes
                    .beam_spawned_this_frame()
                    .is_empty()
            {
                use crate::game_logic::special_power_strikes::PARTICLE_BEAM_AUDIO;
                self.queue_audio_event(
                    AudioEventRequest::new(PARTICLE_BEAM_AUDIO)
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(150),
                );
            }

            log::info!(
                "Host superweapon {} strike {} completed at {:?} (dmg={:.1}, hit={}, killed={})",
                plan.kind.label(),
                plan.strike_id,
                plan.target_position,
                total_damage,
                objects_hit,
                objects_destroyed
            );

            if plan.is_final_wave {
                // C++ SpecialPowerCompletionDie::onDie analog: strike finished.
                if let Some(power) = plan.kind.command_power_for_notify() {
                    self.notify_script_engine_special_power_event(
                        plan.source_object,
                        &power,
                        false,
                        true,
                    );
                }
            }

        }

        // NuclearMissile residual radiation field ticks (after impact blasts).
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_nuclear_radiation_fields();
        }
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_neutron_slow_death_fields();
        }
        self.update_wave_guides();
        // AnthraxBomb residual toxin field ticks (after impact blasts).
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_anthrax_toxin_fields();
        }
        // SpectreGunship residual orbit damage ticks (after insertion).
        // Wave 825: under coupled shadow, zone/field damage sole-ticks after GW writeback.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_spectre_orbit_fields();
        }
        self.spawn_spectre_howitzer_shell_objects_for_new_spawns();
        // Wave 806: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_spectre_howitzer_shell_objects();
        }
        // ParticleCannon residual continuous beam pulses (after charge residual).
        self.update_particle_beam_fields();
        self.spawn_particle_orbital_laser_objects_for_new_beams();
        self.spawn_particle_connector_laser_objects_for_new_beams();
        // Wave 808: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_particle_orbital_laser_objects();
        }
        // Wave 808: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_particle_connector_laser_objects();
        }
        // Particle Uplink DamagePulseRemnant trail residual ticks.
        self.spawn_particle_trail_remnant_objects_for_new_fields();
        // Wave 808: under coupled shadow, lifetime owned by GW expire + logs.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_particle_trail_remnant_objects();
        }
        self.update_particle_remnant_fields();
    }

    /// Tick residual radiation fields spawned by NuclearMissile impacts.
    /// Fail-closed vs full HazardousMaterialArmor / cleanup-hazard objects.

    /// C++ NeutronMissileSlowDeathBehavior multi-blast residual.
    pub(in super::super) fn update_neutron_slow_death_fields(&mut self) {
        use crate::game_logic::host_neutron_missile_slow_death::{
            neutron_blast_can_topple, plan_neutron_frame, MC_BIT_BURNED,
        };

        let n = self.special_power_strikes.neutron_slow_death_field_count();
        if n == 0 {
            return;
        }

        // Snapshot object xyz + ids for 3D falloff planning.
        let objects: Vec<(ObjectId, f32, f32, f32, bool)> = self
            .objects
            .iter()
            .map(|(id, o)| {
                let p = o.get_position();
                (*id, p.x, p.y, p.z, o.is_alive())
            })
            .collect();

        // Access fields via temporary steal pattern.
        let fields = self
            .special_power_strikes
            .neutron_slow_death_fields_mut_for_tick();
        let metas = self
            .special_power_strikes
            .neutron_slow_death_meta()
            .to_vec();

        let frame = self.frame;
        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
        let mut keep_fields = Vec::new();
        let mut keep_metas = Vec::new();

        for (mut state, meta) in fields.into_iter().zip(metas.into_iter()) {
            let epicenter = (meta.position.x, meta.position.y, meta.position.z);
            let xyz: Vec<(f32, f32, f32)> =
                objects.iter().map(|(_, x, y, z, _)| (*x, *y, *z)).collect();
            let (hits, place_scorch, done) =
                plan_neutron_frame(&mut state, frame, epicenter, &xyz);

            // C++ SlowDeath MIDPOINT OCL_NukeRadiationField residual.
            if state.take_radiation_ocl_request(frame) {
                self.special_power_strikes.spawn_radiation_field(
                    meta.source_object,
                    meta.source_team,
                    meta.position,
                    frame,
                    meta.parent_strike_id,
                );
            }

            if place_scorch {
                // Presentation residual: combat particle at epicenter.
                let _ = self.combat_particles.spawn(
                    crate::game_logic::combat_particles::CombatParticleKind::DeathExplosion,
                    meta.position,
                    frame,
                    Some(meta.source_object),
                    None,
                );
            }

            for hit in hits {
                let Some((id, _, _, _, alive)) = objects.get(hit.target_index).copied() else {
                    continue;
                };
                if id == meta.source_object {
                    continue;
                }
                let Some(obj) = self.objects.get_mut(&id) else {
                    continue;
                };
                if hit.set_burned {
                    obj.model_condition_bits |= 1u128 << MC_BIT_BURNED;
                }
                if hit.topple_speed > 0.0 {
                    // C++ Object::topple: any ToppleUpdate that isAbleToBeToppled.
                    let has = obj.topple_data.is_some();
                    let able = obj
                        .topple_data
                        .as_ref()
                        .map(|t| t.is_able_to_be_toppled())
                        .unwrap_or(false);
                    if neutron_blast_can_topple(has, able) {
                        let _ = obj.apply_topple(
                            hit.topple_dx,
                            hit.topple_dz,
                            hit.topple_speed,
                            crate::game_logic::host_topple::TOPPLE_OPTIONS_NO_BOUNCE
                                | crate::game_logic::host_topple::TOPPLE_OPTIONS_NO_FX,
                        );
                    }
                }
                if hit.damage > 0.0 && alive {
                    let destroyed =
                        obj.take_damage_from_immediate(hit.damage, Some(meta.source_object));
                    if destroyed {
                        destroy_ids.push((id, meta.source_team));
                    }
                }
            }

            if !done {
                keep_fields.push(state);
                keep_metas.push(meta);
            }
        }

        self.special_power_strikes
            .restore_neutron_slow_death_fields(keep_fields, keep_metas);

        for (id, team) in destroy_ids {
            self.mark_object_for_destruction(id, Some(team));
        }
    }

    /// C++ WaveGuideUpdate residual — flood wave motion + damage after DamDie.
    pub(in super::super) fn update_wave_guides(&mut self) {
        use crate::game_logic::host_topple::{
            HostToppleData, TOPPLE_OPTIONS_NO_BOUNCE, TOPPLE_OPTIONS_NO_FX,
        };
        use crate::game_logic::host_wave_guide::{
            is_wave_guide_template, wave_damage_at_distance, MC_BIT_FLOODED, WAVE_DAMAGE_RADIUS,
            WAVE_TOPPLE_FORCE,
        };
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use crate::game_logic::host_bridge_behavior::is_bridge_span_template;


        let frame = self.frame;
        // C++ WaveGuideUpdate.cpp:93-101 ctor m_needDisable; update:739-743
        // setDisabled(DISABLED_DEFAULT) on first tick so flood waves stay
        // inert until DamDie::onDie clears the bit.
        for obj in self.objects.values_mut() {
            let is_wg = obj.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                || is_wave_guide_template(&obj.template_name);
            if !is_wg {
                continue;
            }
            if obj.wave_guide_data.is_none() {
                let mut wg = crate::game_logic::host_wave_guide::HostWaveGuideData::default();
                wg.facing = obj.get_orientation();
                obj.wave_guide_data = Some(wg);
                obj.status.disabled_default = true;
            }
        }

        // Collect waveguide ids + poses first.
        let guides: Vec<(ObjectId, glam::Vec3, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.status.disabled_default {
                    return None;
                }
                let is_wg = o.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                    || is_wave_guide_template(&o.template_name);
                if !is_wg {
                    return None;
                }
                let wg = o.wave_guide_data.as_ref()?;
                if !wg.is_moving(frame) {
                    // Still ensure data exists / active clock.
                    return None;
                }
                Some((*id, o.get_position(), o.get_orientation()))
            })
            .collect();

        if guides.is_empty() {
            // Still tick ensure_active for enabled waveguides waiting on delay.
            for obj in self.objects.values_mut() {
                if obj.status.disabled_default {
                    continue;
                }
                let is_wg = obj.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                    || is_wave_guide_template(&obj.template_name);
                if is_wg {
                    if obj.wave_guide_data.is_none() {
                        let mut wg =
                            crate::game_logic::host_wave_guide::HostWaveGuideData::default();
                        wg.facing = obj.get_orientation();
                        wg.ensure_active(frame.max(1));
                        obj.wave_guide_data = Some(wg);
                    } else if let Some(wg) = obj.wave_guide_data.as_mut() {
                        wg.ensure_active(frame.max(1));
                    }
                }
            }
            return;
        }

        let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

        for (gid, gpos, gori) in guides {
            // C++ startMoving: bind WaveGuide1, snap, face the first link.
            let mut apply_ori: Option<f32> = None;
            let mut apply_pos: Option<glam::Vec3> = None;
            if let Some(obj) = self.objects.get_mut(&gid) {
                let cur = obj.get_position();
                if let Some(wg) = obj.wave_guide_data.as_mut() {
                    if !wg.initialized {
                        match gamelogic::terrain::get_terrain_logic()
                            .read()
                            .ok()
                            .map(|tl| tl.bind_wave_guide1())
                        {
                            Some(gamelogic::terrain::WaveGuide1Bind::Follow {
                                first,
                                last,
                                angle,
                            }) => {
                                wg.final_destination = Some((last.x, last.y));
                                wg.facing = angle;
                                apply_ori = Some(angle);
                                apply_pos = Some(glam::Vec3::new(first.x, first.z, first.y));
                            }
                            Some(gamelogic::terrain::WaveGuide1Bind::InvalidPath) => {
                                wg.mark_done();
                            }
                            Some(gamelogic::terrain::WaveGuide1Bind::MissingWaypoint)
                            | None => {
                                wg.final_destination = Some((0.0, 0.0));
                                wg.facing = gori;
                            }
                        }
                    } else {
                        wg.facing = gori;
                    }
                    if let Some((dx, dz)) = wg.motion_delta(frame) {
                        let mut p = apply_pos.unwrap_or(cur);
                        p.x += dx;
                        p.z += dz;
                        apply_pos = Some(p);
                    }
                }
            }
            if let Some(obj) = self.objects.get_mut(&gid) {
                if let Some(angle) = apply_ori {
                    obj.set_orientation(angle);
                }
                if let Some(p) = apply_pos {
                    obj.set_position(p);
                }
                let p = obj.get_position();
                let team = obj.team;
                if let Some(wg) = obj.wave_guide_data.as_mut() {
                    if wg.reached_destination(p.x, p.z) {
                        wg.mark_done();
                        destroy_ids.push((gid, team));
                        continue;
                    }
                    if wg.done {
                        destroy_ids.push((gid, team));
                        continue;
                    }
                }
            }
            let gpos = self
                .objects
                .get(&gid)
                .map(|o| o.get_position())
                .unwrap_or(gpos);

            let preferred = self
                .objects
                .get(&gid)
                .and_then(|o| o.wave_guide_data.as_ref())
                .map(|w| w.preferred_height)
                .unwrap_or(crate::game_logic::host_wave_guide::WAVE_PREFERRED_HEIGHT);

            let victims: Vec<ObjectId> = self
                .objects
                .iter()
                .filter_map(|(id, o)| {
                    if *id == gid {
                        return None;
                    }
                    if o.is_kind_of(crate::game_logic::KindOf::WaveGuide)
                        || is_wave_guide_template(&o.template_name)
                    {
                        return None;
                    }
                    if o.is_kind_of(crate::game_logic::KindOf::BridgeTower)
                        || o.template_name.to_ascii_lowercase().contains("bridgetower")
                    {
                        return None;
                    }
                    if o.status.wet {
                        return None;
                    }
                    if !o.is_alive() {
                        return None;
                    }
                    if o.is_kind_of(crate::game_logic::KindOf::Aircraft)
                        || o.is_kind_of(crate::game_logic::KindOf::Projectile)
                    {
                        return None;
                    }
                    let p = o.get_position();
                    let is_bridge = o.is_kind_of(crate::game_logic::KindOf::Bridge)
                        || is_bridge_span_template(&o.template_name);
                    if p.y > preferred && !is_bridge {
                        return None;
                    }
                    let dx = p.x - gpos.x;
                    let dz = p.z - gpos.z;
                    let dist = (dx * dx + dz * dz).sqrt();
                    if dist <= WAVE_DAMAGE_RADIUS {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();

            for vid in victims {
                let Some(obj) = self.objects.get_mut(&vid) else {
                    continue;
                };
                if obj.status.wet {
                    continue;
                }
                obj.status.wet = true;
                let p = obj.get_position();
                let dx = p.x - gpos.x;
                let dz = p.z - gpos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let dmg = wave_damage_at_distance(dist);
                obj.model_condition_bits |= 1u128 << MC_BIT_FLOODED;
                let name = obj.template_name.to_ascii_lowercase();
                if obj.topple_data.is_none()
                    && (name.contains("tree")
                        || name.contains("shrub")
                        || crate::game_logic::host_topple::is_topple_capable_template(
                            &obj.template_name,
                        ))
                {
                    let mut td = HostToppleData::default();
                    let len = dist.max(0.001);
                    if td.apply_toppling_force(
                        dx / len,
                        dz / len,
                        WAVE_TOPPLE_FORCE,
                        TOPPLE_OPTIONS_NO_BOUNCE | TOPPLE_OPTIONS_NO_FX,
                    ) {
                        obj.topple_data = Some(td);
                    }
                }
                let is_bridge = obj.is_kind_of(crate::game_logic::KindOf::Bridge)
                    || is_bridge_span_template(&obj.template_name);
                if dmg > 0.0 {
                    if let Some(wg) = self
                        .objects
                        .get_mut(&gid)
                        .and_then(|o| o.wave_guide_data.as_mut())
                    {
                        wg.damage_applications = wg.damage_applications.saturating_add(1);
                    }
                    let team = self
                        .objects
                        .get(&gid)
                        .map(|o| o.team)
                        .unwrap_or(Team::Neutral);
                    if let Some(obj) = self.objects.get_mut(&vid) {
                        let destroyed = obj.take_damage_from_immediate_typed_death(
                            dmg,
                            Some(gid),
                            crate::game_logic::combat::DamageType::Water,
                            HostDeathType::Flooded,
                        );
                        if destroyed {
                            destroy_ids.push((vid, team));
                        }
                    }
                    if is_bridge {
                        self.ensure_named_bridge_template("WaterWaveBridge", 1.0);
                        let _ = self.create_object("WaterWaveBridge", Team::Neutral, p);
                        if let Ok(mut tl) = gamelogic::terrain::get_terrain_logic().write() {
                            let loc = gamelogic::common::Coord3D::new(p.x, p.z, p.y);
                            let _ = tl.delete_bridge(&loc);
                        }
                    }
                }
            }
        }


        for (id, team) in destroy_ids {
            self.mark_object_for_destruction(id, Some(team));
        }
    }

    pub(in super::super) fn update_nuclear_radiation_fields(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_radiation_ticks(self.frame, &object_positions);
        let frame = self.frame;

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed =
                        target.take_damage_from_immediate(hit.damage, Some(plan.source_object));
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.special_power_strikes.record_radiation_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        // NukeRadiationFieldWeapon Object residual (spawn + DeletionUpdate lifetime).
        self.spawn_nuke_radiation_field_objects_for_new_fields();
        // Wave 820: under coupled shadow, field-object lifetime owned by GW expire.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_nuke_radiation_field_objects();
        }
        self.special_power_strikes.prune_expired_radiation(frame);
    }

    /// Tick residual toxin fields spawned by AnthraxBomb impacts.
    /// Fail-closed vs full HazardousMaterialArmor / cleanup-hazard / gamma objects.
    pub(in super::super) fn update_anthrax_toxin_fields(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| {
                (
                    *id,
                    obj.get_position(),
                    obj.team,
                    obj.is_alive(),
                    obj.is_kind_of(KindOf::Aircraft) || obj.status.airborne_target,
                )
            })
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_toxin_ticks(self.frame, &object_positions);
        let frame = self.frame;

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed = target.take_damage_from_immediate_typed_death(
                        hit.damage,
                        Some(plan.source_object),
                        crate::game_logic::host_poisoned_behavior::poison_weapon_damage_type(),
                        plan.death_type,
                    );
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.special_power_strikes.record_toxin_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.spawn_anthrax_toxin_field_objects_for_new_fields();
        // Wave 820: under coupled shadow, field-object lifetime owned by GW expire.
        if !(crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active())
        {
            self.update_anthrax_toxin_field_objects();
        }
        self.special_power_strikes.prune_expired_toxin(frame);
    }

    /// Tick residual Spectre orbit fields spawned at orbit insertion.
    /// Fail-closed vs full SpectreGunshipUpdate gattling-strafe / howitzer projectile.
    pub(in super::super) fn update_spectre_orbit_fields(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_orbit_ticks(self.frame, &object_positions);
        let frame = self.frame;

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed =
                        target.take_damage_from_immediate(hit.damage, Some(plan.source_object));
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.special_power_strikes.record_orbit_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.special_power_strikes.prune_expired_orbit(frame);
    }

    /// Tick residual Particle Uplink continuous beam fields after charge residual.
    /// Manual drive + WidthGrow grow/hold/decay + outer-node honesty residual closed.
    /// Intensity schedule (CHARGING/PREPARING/ALMOST_READY/POSTFIRE/PACKING) +
    /// BeamLaunchFX residual closed.
    /// Fail-closed vs full bone-extract lasers / GPU OuterBeamWidth matrix.
    /// Swath + DamagePulseRemnant residual closed.
    pub(in super::super) fn update_particle_beam_fields(&mut self) {
        let frame = self.frame;
        // Pre-fire intensity schedule + BeamLaunchFX + POSTFIRE/PACKING residual
        // (also advances ScudStorm PreAttack residual frame counter).
        self.special_power_strikes
            .advance_particle_intensity_schedule(frame);
        // Manual beam driving residual: advance current target toward override
        // before damage / scorch planning (retail update order).
        self.special_power_strikes.advance_manual_beam_drive(frame);

        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_beam_ticks(frame, &object_positions);

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed =
                        target.take_damage_from_immediate(hit.damage, Some(plan.source_object));
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.special_power_strikes.record_beam_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        // WidthGrow grow/hold/decay honesty sample (even when no damage pulse due).
        // Retail LASERSTATUS_DECAYING after TotalFiringTime shrinks m_currentWidthScalar.
        self.special_power_strikes.sample_beam_width_honesty(frame);

        // TotalScorchMarks / GroundHitFX / RevealRange residual (retail STATUS_FIRING).
        // C++: doShroudReveal + undoShroudReveal at current target with RevealRange
        // each scorch tick (instant "gratuitous vision" pulse, not duration reveal).
        let scorch_events = self
            .special_power_strikes
            .apply_due_beam_scorch_reveals(frame);
        if !scorch_events.is_empty() {
            use crate::game_logic::special_power_strikes::PARTICLE_REVEAL_RANGE;
            use gamelogic::common::Coord3D;
            let world_w = self.world_width.max(1.0);
            let world_h = self.world_height.max(1.0);
            if let Ok(mut shroud_mgr) = get_shroud_manager().lock() {
                if !shroud_mgr.has_shroud_grid() {
                    shroud_mgr.init_shroud_grid(world_w, world_h);
                }
                for event in &scorch_events {
                    let mut player_mask = 0u32;
                    for (&pid, player) in &self.players {
                        if player.team == event.source_team {
                            player_mask |= 1u32 << pid.min(31);
                        }
                    }
                    if player_mask == 0 {
                        // No registered players for team: skip FOW write (honesty
                        // counters already recorded on the beam field).
                        continue;
                    }
                    // Host gameplay plane (x,z) → shroud (x,y).
                    let center = Coord3D::new(event.position.x, event.position.z, event.position.y);
                    let range = if event.reveal_range > 0.0 {
                        event.reveal_range
                    } else {
                        PARTICLE_REVEAL_RANGE
                    };
                    // Retail: do + undo same frame (pulse reveal, not duration FOW).
                    shroud_mgr.do_shroud_reveal(&center, range, player_mask);
                    shroud_mgr.undo_shroud_reveal(&center, range, player_mask);
                }
            }
        }

        self.special_power_strikes.prune_expired_beam(frame);
    }

    /// C++ SpectreHowitzerShell ThingFactory Object residual (orbit howitzer ticks).
    pub fn spawn_spectre_howitzer_shell_objects_for_new_spawns(&mut self) {
        use crate::game_logic::special_power_strikes::{
            SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES, SPECTRE_HOWITZER_SHELL_MAX_HEALTH,
            SPECTRE_HOWITZER_SHELL_OBJECT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending = self
            .special_power_strikes
            .take_howitzer_shell_spawns_this_frame();
        if pending.is_empty() {
            return;
        }
        if !self.templates.contains_key(SPECTRE_HOWITZER_SHELL_OBJECT) {
            let mut t = ThingTemplate::new(SPECTRE_HOWITZER_SHELL_OBJECT);
            t.add_kind_of(KindOf::Projectile)
                .set_health(SPECTRE_HOWITZER_SHELL_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(SPECTRE_HOWITZER_SHELL_OBJECT.to_string(), t);
        }
        let expires = self
            .frame
            .saturating_add(SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES.max(1));
        for (source, team, pos) in pending {
            if let Some(oid) = self.create_object(SPECTRE_HOWITZER_SHELL_OBJECT, team, pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.spectre_howitzer_shell = true;
                    o.note_producer(source);
                    o.spectre_howitzer_shell_expires_frame = Some(expires);
                    o.health.maximum = SPECTRE_HOWITZER_SHELL_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, SPECTRE_HOWITZER_SHELL_MAX_HEALTH);
                    // Fall residual toward ground.
                    o.movement.velocity = Vec3::new(0.0, -14.0, 0.0);
                }
                self.special_power_strikes
                    .record_howitzer_shell_object_spawn();
            }
        }
    }

    pub fn update_spectre_howitzer_shell_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.spectre_howitzer_shell {
                    if let Some(exp) = o.spectre_howitzer_shell_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                    // HeightDie residual: destroy near ground.
                    if o.get_position().y <= 1.0 {
                        return Some(*id);
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.spectre_howitzer_shell = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ PoisonFieldAnthraxBomb / PoisonFieldLarge ThingFactory Object residual.
    pub fn spawn_anthrax_toxin_field_objects_for_new_fields(&mut self) {
        use crate::game_logic::special_power_strikes::{
            ANTHRAX_TOXIN_FIELD_MAX_HEALTH, ANTHRAX_TOXIN_OBJECT_NAME,
            SCUD_POISON_FIELD_MAX_HEALTH, SCUD_POISON_OBJECT_NAME,
            SCUD_POISON_UPGRADED_FIELD_MAX_HEALTH, SCUD_POISON_UPGRADED_OBJECT_NAME,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3, u32, String)> = self
            .special_power_strikes
            .toxin_spawned_this_frame()
            .iter()
            .filter_map(|tid| {
                self.special_power_strikes
                    .toxin_fields()
                    .iter()
                    .find(|f| f.id == *tid && f.object_id.is_none())
                    .map(|f| {
                        (
                            f.id,
                            f.source_object,
                            f.source_team,
                            f.position,
                            f.expires_frame.saturating_sub(f.spawn_frame),
                            f.object_template.clone(),
                        )
                    })
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        for (tid, source, team, pos, lifetime, template) in pending {
            let max_hp = if template == SCUD_POISON_UPGRADED_OBJECT_NAME
                || template == "PoisonFieldUpgradedLarge"
            {
                SCUD_POISON_UPGRADED_FIELD_MAX_HEALTH
            } else if template == SCUD_POISON_OBJECT_NAME {
                SCUD_POISON_FIELD_MAX_HEALTH
            } else {
                ANTHRAX_TOXIN_FIELD_MAX_HEALTH
            };
            let tmpl = if template.is_empty() {
                ANTHRAX_TOXIN_OBJECT_NAME.to_string()
            } else {
                template
            };
            if !self.templates.contains_key(&tmpl) {
                let mut t = ThingTemplate::new(&tmpl);
                t.add_kind_of(KindOf::Immobile)
                    .set_health(max_hp)
                    .set_cost(0, 0);
                self.templates.insert(tmpl.clone(), t);
            }
            let expires = self.frame.saturating_add(lifetime.max(1));
            if let Some(oid) = self.create_object(&tmpl, team, pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.anthrax_toxin_field = true;
                    o.note_producer(source);
                    o.anthrax_toxin_field_expires_frame = Some(expires);
                    o.health.maximum = max_hp;
                    Self::write_object_health_authority_aware(o, max_hp);
                }
                let _ = self.special_power_strikes.bind_toxin_object(tid, oid);
            }
        }
    }

    pub fn update_anthrax_toxin_field_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.anthrax_toxin_field {
                    if let Some(exp) = o.anthrax_toxin_field_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.anthrax_toxin_field = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ NukeRadiationFieldWeapon ThingFactory Object residual.
    pub fn spawn_nuke_radiation_field_objects_for_new_fields(&mut self) {
        use crate::game_logic::special_power_strikes::{
            NUKE_RADIATION_DURATION_FRAMES, NUKE_RADIATION_FIELD_MAX_HEALTH,
            NUKE_RADIATION_OBJECT_NAME,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3)> = self
            .special_power_strikes
            .radiation_spawned_this_frame()
            .iter()
            .filter_map(|rid| {
                self.special_power_strikes
                    .radiation_fields()
                    .iter()
                    .find(|f| f.id == *rid && f.object_id.is_none())
                    .map(|f| (f.id, f.source_object, f.source_team, f.position))
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        if !self.templates.contains_key(NUKE_RADIATION_OBJECT_NAME) {
            let mut t = ThingTemplate::new(NUKE_RADIATION_OBJECT_NAME);
            t.add_kind_of(KindOf::Immobile)
                .set_health(NUKE_RADIATION_FIELD_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(NUKE_RADIATION_OBJECT_NAME.to_string(), t);
        }
        let expires = self.frame.saturating_add(NUKE_RADIATION_DURATION_FRAMES);
        for (rid, source, team, pos) in pending {
            if let Some(oid) = self.create_object(NUKE_RADIATION_OBJECT_NAME, team, pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.nuke_radiation_field = true;
                    o.note_producer(source);
                    o.nuke_radiation_field_expires_frame = Some(expires);
                    o.health.maximum = NUKE_RADIATION_FIELD_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, NUKE_RADIATION_FIELD_MAX_HEALTH);
                }
                let _ = self.special_power_strikes.bind_radiation_object(rid, oid);
            }
        }
    }

    pub fn update_nuke_radiation_field_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.nuke_radiation_field {
                    if let Some(exp) = o.nuke_radiation_field_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.nuke_radiation_field = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ Medium/Intense ConnectorLaser ThingFactory Objects (STATUS_FIRING residual).
    pub fn spawn_particle_connector_laser_objects_for_new_beams(&mut self) {
        use crate::game_logic::special_power_strikes::{
            PARTICLE_CONNECTOR_INTENSE_LASER, PARTICLE_CONNECTOR_LASER_MAX_HEALTH,
            PARTICLE_CONNECTOR_MEDIUM_LASER,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3, u32)> = self
            .special_power_strikes
            .beam_spawned_this_frame()
            .iter()
            .filter_map(|bid| {
                self.special_power_strikes
                    .beam_fields()
                    .iter()
                    .find(|f| f.id == *bid && f.connector_object_ids.is_empty())
                    .map(|f| {
                        (
                            f.id,
                            f.source_object,
                            f.source_team,
                            // Connector residual originates at caster building.
                            self.objects
                                .get(&f.source_object)
                                .map(|o| o.get_position())
                                .unwrap_or(f.position),
                            f.expires_frame.saturating_sub(f.spawn_frame),
                        )
                    })
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        for name in [
            PARTICLE_CONNECTOR_MEDIUM_LASER,
            PARTICLE_CONNECTOR_INTENSE_LASER,
        ] {
            if !self.templates.contains_key(name) {
                let mut t = ThingTemplate::new(name);
                t.add_kind_of(KindOf::Immobile)
                    .set_health(PARTICLE_CONNECTOR_LASER_MAX_HEALTH)
                    .set_cost(0, 0);
                self.templates.insert(name.to_string(), t);
            }
        }
        for (bid, source, team, pos, lifetime) in pending {
            let expires = self.frame.saturating_add(lifetime.max(1));
            // Medium connector slightly above building; intense higher toward orbit.
            let placements = [
                (
                    PARTICLE_CONNECTOR_MEDIUM_LASER,
                    Vec3::new(pos.x, pos.y + 40.0, pos.z),
                ),
                (
                    PARTICLE_CONNECTOR_INTENSE_LASER,
                    Vec3::new(pos.x, pos.y + 120.0, pos.z),
                ),
            ];
            let mut ids = Vec::new();
            for (name, cpos) in placements {
                if let Some(oid) = self.create_object(name, team, cpos) {
                    if let Some(o) = self.objects.get_mut(&oid) {
                        o.particle_connector_laser = true;
                        o.note_producer(source);
                        o.particle_connector_laser_expires_frame = Some(expires);
                        o.health.maximum = PARTICLE_CONNECTOR_LASER_MAX_HEALTH;
                        Self::write_object_health_authority_aware(
                            o,
                            PARTICLE_CONNECTOR_LASER_MAX_HEALTH,
                        );
                    }
                    ids.push(oid);
                }
            }
            if !ids.is_empty() {
                let _ = self.special_power_strikes.bind_connector_objects(bid, &ids);
            }
        }
    }

    pub fn update_particle_connector_laser_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.particle_connector_laser {
                    if let Some(exp) = o.particle_connector_laser_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.particle_connector_laser = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ ParticleUplinkCannon_OrbitalLaser ThingFactory Object residual.
    pub fn spawn_particle_orbital_laser_objects_for_new_beams(&mut self) {
        use crate::game_logic::special_power_strikes::{
            PARTICLE_ORBITAL_LASER_MAX_HEALTH, PARTICLE_ORBITAL_LASER_NAME,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3, u32)> = self
            .special_power_strikes
            .beam_spawned_this_frame()
            .iter()
            .filter_map(|bid| {
                self.special_power_strikes
                    .beam_fields()
                    .iter()
                    .find(|f| f.id == *bid && f.object_id.is_none())
                    .map(|f| {
                        (
                            f.id,
                            f.source_object,
                            f.source_team,
                            f.position,
                            f.expires_frame.saturating_sub(f.spawn_frame),
                        )
                    })
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        if !self.templates.contains_key(PARTICLE_ORBITAL_LASER_NAME) {
            let mut t = ThingTemplate::new(PARTICLE_ORBITAL_LASER_NAME);
            t.add_kind_of(KindOf::Immobile)
                .set_health(PARTICLE_ORBITAL_LASER_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(PARTICLE_ORBITAL_LASER_NAME.to_string(), t);
        }
        for (bid, source, team, pos, lifetime) in pending {
            let expires = self.frame.saturating_add(lifetime.max(1));
            // Place orbital laser residual above target (retail laser origin altitude).
            let laser_pos = Vec3::new(pos.x, pos.y + 500.0, pos.z);
            if let Some(oid) = self.create_object(PARTICLE_ORBITAL_LASER_NAME, team, laser_pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.particle_orbital_laser = true;
                    o.note_producer(source);
                    o.particle_orbital_laser_expires_frame = Some(expires);
                    o.health.maximum = PARTICLE_ORBITAL_LASER_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, PARTICLE_ORBITAL_LASER_MAX_HEALTH);
                }
                let _ = self.special_power_strikes.bind_beam_object(bid, oid);
            }
        }
    }

    pub fn update_particle_orbital_laser_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.particle_orbital_laser {
                    if let Some(exp) = o.particle_orbital_laser_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.particle_orbital_laser = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ ParticleUplinkCannonTrailRemnant ThingFactory Object residual.
    pub fn spawn_particle_trail_remnant_objects_for_new_fields(&mut self) {
        use crate::game_logic::special_power_strikes::{
            PARTICLE_REMNANT_DURATION_FRAMES, PARTICLE_REMNANT_MAX_HEALTH,
            PARTICLE_REMNANT_OBJECT_NAME,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let pending: Vec<(u32, ObjectId, Team, Vec3)> = self
            .special_power_strikes
            .remnant_spawned_this_frame()
            .iter()
            .filter_map(|rid| {
                self.special_power_strikes
                    .remnant_fields()
                    .iter()
                    .find(|f| f.id == *rid && f.object_id.is_none())
                    .map(|f| (f.id, f.source_object, f.source_team, f.position))
            })
            .collect();
        if pending.is_empty() {
            return;
        }
        if !self.templates.contains_key(PARTICLE_REMNANT_OBJECT_NAME) {
            let mut t = ThingTemplate::new(PARTICLE_REMNANT_OBJECT_NAME);
            t.add_kind_of(KindOf::Immobile)
                .set_health(PARTICLE_REMNANT_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates
                .insert(PARTICLE_REMNANT_OBJECT_NAME.to_string(), t);
        }
        let expires = self.frame.saturating_add(PARTICLE_REMNANT_DURATION_FRAMES);
        for (rid, source, team, pos) in pending {
            if let Some(oid) = self.create_object(PARTICLE_REMNANT_OBJECT_NAME, team, pos) {
                if let Some(o) = self.objects.get_mut(&oid) {
                    o.particle_trail_remnant = true;
                    o.note_producer(source);
                    o.particle_trail_remnant_expires_frame = Some(expires);
                    o.health.maximum = PARTICLE_REMNANT_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, PARTICLE_REMNANT_MAX_HEALTH);
                }
                let _ = self.special_power_strikes.bind_remnant_object(rid, oid);
            }
        }
    }

    pub fn update_particle_trail_remnant_objects(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.particle_trail_remnant {
                    if let Some(exp) = o.particle_trail_remnant_expires_frame {
                        if exp <= frame {
                            return Some(*id);
                        }
                    }
                }
                None
            })
            .collect();
        for id in due {
            if let Some(o) = self.objects.get_mut(&id) {
                // Wave 752: under damage authority, do not zero host HP mid-frame
                // (dual with GW HP writeback). Project lethal via damage log + flags.
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.particle_trail_remnant = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// Tick residual DamagePulseRemnant trail fields spawned by Particle Uplink
    /// beam pulses. ParticleUplinkCannonTrailRemnant Object residual closed.
    pub(in super::super) fn update_particle_remnant_fields(&mut self) {
        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();

        let plans = self
            .special_power_strikes
            .plan_due_remnant_ticks(self.frame, &object_positions);
        let frame = self.frame;

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed =
                        target.take_damage_from_immediate(hit.damage, Some(plan.source_object));
                    total_damage += hit.damage;
                    applications += 1;
                    if killed {
                        destroyed += 1;
                        destroy_ids.push((hit.target_id, plan.source_team));
                    }
                }
            }

            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }

            self.special_power_strikes.record_remnant_tick_complete(
                plan.field_id,
                total_damage,
                applications,
                destroyed,
                frame,
            );
        }

        self.special_power_strikes.prune_expired_remnant(frame);
    }
}
