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
        // C++ OCLSpecialPower::findOCL: getObject()->getControllingPlayer()->hasScience.
        // Never union same-faction allies or USA-vs-USA opponents.
        let sciences: Vec<String> = self
            .objects
            .get(&source_object)
            .and_then(|obj| self.player_owner_for_host_object(obj))
            .and_then(|pid| self.get_player(pid))
            .map(|p| p.unlocked_sciences.iter().cloned().collect())
            .unwrap_or_default();
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
        // C++ ParticleUplinkCannonUpdate.cpp:260-268: !COMMAND_FIRED_BY_SCRIPT
        // arms m_manualTargetMode so the live beam holds the click.
        if kind == HostSuperweaponKind::ParticleCannon {
            let fired_by_script = self
                .special_power_strikes
                .take_script_fired_special_power(source_object);
            let waypoint_id = self
                .special_power_strikes
                .take_scripted_waypoint_special_power(source_object);
            if let Some(wid) = waypoint_id {
                if let Some(strike) = self.special_power_strikes.get_mut(id) {
                    strike.scripted_waypoint_mode = true;
                    if let Some(terrain) = gamelogic::helpers::TheTerrainLogic::get() {
                        if let Some((next_id, next_pos)) =
                            terrain.scripted_waypoint_initial_override(wid)
                        {
                            strike.next_dest_waypoint_id = next_id;
                            strike.waypoint_override =
                                glam::Vec3::new(next_pos.x, next_pos.z, next_pos.y);
                        } else {
                            strike.next_dest_waypoint_id = wid;
                            strike.waypoint_override = target_position;
                        }
                    } else {
                        strike.next_dest_waypoint_id = wid;
                        strike.waypoint_override = target_position;
                    }
                }
            } else if !fired_by_script {
                if let Some(strike) = self.special_power_strikes.get_mut(id) {
                    strike.manual_beam_hold = true;
                }
            }
        }
        // C++ SpectreGunshipUpdate.cpp:532 PLAYER_HUMAN disables wide auto-acquire.
        // Host residual: local player is human; unmapped owner fail-closed (no wide).
        if kind == HostSuperweaponKind::SpectreGunship {
            let controller_is_ai = self
                .objects
                .get(&source_object)
                .and_then(|obj| self.player_owner_for_host_object(obj))
                .and_then(|pid| self.players.get(&pid))
                .map(|p| !p.is_local)
                .unwrap_or(false);
            if controller_is_ai {
                self.special_power_strikes
                    .note_spectre_ai_controller(source_object);
            }
        }

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
        // (A10 / Daisy / MOAB / Anthrax). Calling execute_ocl here doubled the jets.
        if !matches!(
            kind,
            HostSuperweaponKind::A10Strike
                | HostSuperweaponKind::DaisyCutter
                | HostSuperweaponKind::AnthraxBomb
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
            // C++ one CarpetBombWeapon per drop. Flight leftover owns the blast.
            if self
                .spawn_carpet_bomb_flight(source_object, target_position, tier)
                .is_some()
            {
                if let Some(s) = self.special_power_strikes.get_mut(id) {
                    s.live_carpet_delivery = true;
                }
            }
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
        // C++ OCLSpecialPower::findOCL: first owned UpgradeOCL science wins.
        // SCIENCE_MOAB (Upgrade_AmericaMOAB) → SUPERWEAPON_MOAB on the same
        // SPECIAL_DAISY_CUTTER button. FuelAirBomb is that same template, not a
        // separate MOAB command.
        if kind == HostSuperweaponKind::DaisyCutter {
            use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
            let tmpl =
                crate::game_logic::host_ocl_special_power::special_power_template_for_host_kind(
                    kind.label(),
                )
                .unwrap_or("SuperweaponDaisyCutter");
            let plan = self.plan_ocl_special_power(tmpl, source_object, target_position);
            let tier = plan
                .as_ref()
                .map(|p| DaisyFlightPayloadTier::from_ocl_name(&p.ocl_name))
                .unwrap_or(DaisyFlightPayloadTier::DaisyCutter);
            let _ = self.spawn_daisy_cutter_flight(source_object, target_position, tier);
        }
        // C++ AnthraxBomb DeliverPayload residual (GLAJetCargoPlane + bomb).
        // C++ one FireWeaponWhenDead — flight leftover owns the blast + toxin.
        if kind == HostSuperweaponKind::AnthraxBomb {
            if self
                .spawn_anthrax_bomb_flight(source_object, target_position)
                .is_some()
            {
                if let Some(s) = self.special_power_strikes.get_mut(id) {
                    s.live_anthrax_delivery = true;
                }
            }
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
                    let spawned =
                        self.execute_ocl_fire_weapon(ocl, source_object, primary, target_position);
                    // C++ one NeutronMissileSlowDeath on the flying missile.
                    // Registry delayed blast is only a fallback when spawn fails.
                    if kind == HostSuperweaponKind::NuclearMissile {
                        if let Some(mid) = spawned {
                            let live = self
                                .objects
                                .get(&mid)
                                .and_then(|o| o.neutron_missile_update.as_ref())
                                .is_some_and(|d| !d.is_cruise);
                            if live {
                                if let Some(s) = self.special_power_strikes.get_mut(id) {
                                    s.live_neutron_delivery = true;
                                }
                            }
                        }
                    }
                }
                OclNuggetKind::Attack(ocl) => {
                    // C++ AttackNugget::create — 9 flying missiles own the warhead.
                    if self.execute_ocl_attack(ocl, source_object, target_position)
                        && kind == HostSuperweaponKind::ScudStorm
                    {
                        if let Some(s) = self.special_power_strikes.get_mut(id) {
                            s.live_scud_delivery = true;
                        }
                    }
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
        let source_owner_player_id = self
            .objects
            .get(&source_object)
            .and_then(|obj| obj.owner_player_id);
        self.try_eva_superweapon_launched_owned(source_owner_player_id, source_team, kind);

        // C++ SpecialPowerModule.cpp:513 aboutToDoSpecialPower.
        self.notify_script_engine_special_power_event(source_object, power, true, false);
        // C++ SpecialPowerModule.cpp:454/462 createViewObject (range 250 / 30-40s).
        self.create_special_power_view_object(source_object, target_position, kind);

        // C++ SpecialPowerModule.cpp:622-628 getInitiateAtTargetSound at click.
        // Distinct from source InitiateSound (CommandXlat / hq-yip5e).
        let at_location = kind.retail_initiate_at_location_sound();
        if !at_location.is_empty() {
            self.queue_audio_event(
                AudioEventRequest::new(at_location)
                    .with_position(target_position)
                    .with_priority(180),
            );
        }
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
        // C++ initiateIntentToDoSpecialPower → recordSpecialPowerUsed
        // (ACT_SUPERPOWER). Live fire never ran leftover special_power_module.
        self.record_academy_special_power_used(source_object, power);
        self.drain_puc_loop_audio();
        Some(id)
    }

    /// C++ SpecialPowerModule::initiateIntentToDoSpecialPower →
    /// AcademyStats::recordSpecialPowerUsed. Gate is ACT_SUPERPOWER only.
    pub fn record_academy_special_power_used(
        &mut self,
        source_object: ObjectId,
        power: &crate::command_system::SpecialPowerType,
    ) {
        if !special_power_is_act_superpower(power) {
            return;
        }
        let owner_id = self
            .objects
            .get(&source_object)
            .and_then(|obj| self.player_owner_for_host_object(obj));
        let owner_team = self.objects.get(&source_object).map(|o| o.team);
        if let Some(pid) = owner_id {
            if let Some(player) = self.get_player_mut(pid) {
                player.record_special_power_used();
                return;
            }
        }
        if let Some(team) = owner_team {
            if let Some(player) = self.get_player_mut_by_team(team) {
                player.record_special_power_used();
            }
        }
    }

    /// C++ ParticleUplinkCannonUpdate::setClientStatus — play authored loops
    /// on CHARGING / PREPARING / FIRING (hq-l0dl2).
    fn drain_puc_loop_audio(&mut self) {
        let events = self.special_power_strikes.take_puc_loop_audio_this_frame();
        for (object_id, position, cue) in events {
            // C++ setLogicalStatus binds the loops to the PUC object, not the beam target.
            let pos = self
                .objects
                .get(&object_id)
                .map(|o| o.get_position())
                .unwrap_or(position);
            self.queue_audio_event(
                AudioEventRequest::new(cue)
                    .with_object(object_id)
                    .with_position(pos)
                    .with_priority(150),
            );
        }
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
        self.create_special_power_view_object_at(
            source_object,
            target_position,
            kind.view_object_range(),
            kind.view_object_duration_frames(),
        )
    }

    /// C++ `SpecialPowerModule::createViewObject` with explicit range/duration.
    /// Cluster Mines inherits this via `OCLSpecialPower::doSpecialPowerAtLocation`.
    pub fn create_special_power_view_object_at(
        &mut self,
        source_object: ObjectId,
        target_position: Vec3,
        range: f32,
        duration: u32,
    ) -> bool {
        use crate::game_logic::special_power_strikes::HostViewObjectReveal;
        use crate::game_logic::{KindOf, ThingTemplate};
        use gamelogic::common::Coord3D;

        const VIEW_OBJECT_TEMPLATE: &str = "SpecialPowerViewObject";
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
            self.templates.insert(VIEW_OBJECT_TEMPLATE.to_string(), t);
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
            ANTHRAX_TOXIN_AUDIO, HostSuperweaponKind, NUKE_RADIATION_AUDIO, SPECTRE_ORBIT_AUDIO,
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
                    // C++ Weapon.cpp dealDamage / NeutronMissileSlowDeath: authored
                    // DamageType through Armor.ini. Not DAMAGE_UNRESISTABLE.
                    let destroyed = target.take_damage_from_immediate_typed_death(
                        hit.damage,
                        Some(plan.source_object),
                        plan.kind.authored_damage_type(),
                        plan.kind.authored_death_type(),
                    );

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

            // C++ one SlowDeath on the flying missile — no registry instant blast.
            // C++ A10 is OCL jets + per-missile FX — no DeathExplosion at the click.
            // C++ one FireWeaponWhenDead on AnthraxBomb — flight leftover owns FX.
            let skip_registry_nuke_blast = plan.kind.spawns_radiation()
                && self
                    .special_power_strikes
                    .get(plan.strike_id)
                    .is_some_and(|s| s.live_neutron_delivery);
            let skip_a10_consolidated_blast = plan.kind == HostSuperweaponKind::A10Strike;
            let skip_anthrax_live_blast = plan.kind == HostSuperweaponKind::AnthraxBomb
                && self
                    .special_power_strikes
                    .get(plan.strike_id)
                    .is_some_and(|s| s.live_anthrax_delivery);
            if !skip_registry_nuke_blast && !skip_a10_consolidated_blast && !skip_anthrax_live_blast
            {
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
            }

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
            self.drain_puc_loop_audio();

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
            MC_BIT_BURNED, neutron_blast_can_topple, plan_neutron_frame,
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
            let (hits, place_scorch, done) = plan_neutron_frame(&mut state, frame, epicenter, &xyz);

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
                    // C++ NeutronMissileSlowDeathUpdate.cpp:284-286 DAMAGE_EXPLOSION / DEATH_EXPLODED.
                    let destroyed = obj.take_damage_from_immediate_typed_death(
                        hit.damage,
                        Some(meta.source_object),
                        crate::game_logic::combat::DamageType::Explosive,
                        crate::game_logic::host_usa_pilot::HostDeathType::Exploded,
                    );

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
        use crate::game_logic::host_bridge_behavior::is_bridge_span_template;
        use crate::game_logic::host_topple::{
            HostToppleData, TOPPLE_OPTIONS_NO_BOUNCE, TOPPLE_OPTIONS_NO_FX,
        };
        use crate::game_logic::host_usa_pilot::HostDeathType;
        use crate::game_logic::host_wave_guide::{
            MC_BIT_FLOODED, WAVE_DAMAGE_RADIUS, WAVE_TOPPLE_FORCE, is_wave_guide_template,
            wave_damage_at_distance,
        };

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

        // C++ startMoving TheAudio + splash roll (leftover-play, then queue).
        let mut audio_jobs: Vec<(ObjectId, String, bool, glam::Vec3)> = Vec::new();
        for (gid, gpos, _) in &guides {
            let Some(obj) = self.objects.get_mut(gid) else {
                continue;
            };
            let template = obj.template_name.clone();
            let Some(wg) = obj.wave_guide_data.as_mut() else {
                continue;
            };
            let (looping, splash) =
                crate::game_logic::host_wave_guide::leftover_wave_guide_audio_tick(
                    wg, &template, gid.0, frame,
                );
            if let Some(name) = looping {
                audio_jobs.push((*gid, name, true, *gpos));
            }
            if let Some(name) = splash {
                audio_jobs.push((*gid, name, false, *gpos));
            }
        }
        for (id, name, looping, pos) in audio_jobs {
            let mut req = AudioEventRequest::new(&name)
                .with_object(id)
                .with_position(pos);
            if looping {
                req = req.looping();
            }
            self.queue_audio_event(req);
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
                            Some(gamelogic::terrain::WaveGuide1Bind::MissingWaypoint) | None => {
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

            // C++ WaveGuideUpdate.cpp:411-415 `doWaterMotion` → addWaterVelocity.
            if let Some(tv) = gamelogic::helpers::TheTerrainVisual::get() {
                let facing = self
                    .objects
                    .get(&gid)
                    .map(|o| o.get_orientation())
                    .unwrap_or(gori);
                let vel = crate::game_logic::host_wave_guide::WAVE_WATER_VELOCITY;
                for (wx, wy) in crate::game_logic::host_wave_guide::wave_shape_world_points(
                    gpos.x, gpos.z, facing,
                ) {
                    tv.add_water_velocity(wx, wy, vel, preferred);
                }
            }

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
                        target.take_radiation_field_tick(hit.damage, Some(plan.source_object));
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

    fn apply_pending_special_power_overrides(&mut self) {
        let frame = self.frame;
        let pending: Vec<(ObjectId, Vec3, Option<ObjectId>)> = self
            .objects
            .iter()
            .filter_map(|(id, object)| {
                if !object.is_alive() || object.is_disabled() {
                    return None;
                }
                object
                    .special_power_override_destination
                    .map(|destination| (*id, destination, object.producer_id))
            })
            .collect();
        for (id, destination, producer) in pending {
            self.special_power_strikes
                .apply_source_override_destination(id, destination, frame);
            // C++ SpectreGunshipUpdate owns the dest; live orbit fields are
            // sourced from the command-center caster that spawned the gunship.
            if let Some(producer) = producer {
                if producer != id {
                    self.special_power_strikes
                        .apply_source_override_destination(producer, destination, frame);
                }
            }
        }
    }

    /// C++ `SpectreGunshipUpdate::update` `isEffectivelyDead` / missing-object `cleanUp`.
    /// Live orbit fields are sourced from the command-center caster; resolve the
    /// bound / produced gunship and treat missing or `!is_alive` as shot down.
    fn spectre_orbit_source_gunship_is_dead(&self, source: ObjectId) -> bool {
        if let Some(obj) = self.objects.get(&source) {
            if obj.spectre_gunship_update.is_some() {
                return !obj.is_alive();
            }
            if let Some(gid) = obj
                .spectre_gunship_deployment
                .as_ref()
                .and_then(|d| d.gunship_id)
            {
                return self.objects.get(&gid).is_none_or(|g| !g.is_alive());
            }
        }
        let mut saw_gunship = false;
        for obj in self.objects.values() {
            if obj.producer_id == Some(source) && obj.spectre_gunship_update.is_some() {
                saw_gunship = true;
                if obj.is_alive() {
                    return false;
                }
            }
        }
        saw_gunship
    }

    /// Live gunship world position for leftover `is_fair_distance_from_ship`.
    fn spectre_orbit_gunship_position(&self, source: ObjectId) -> Option<Vec3> {
        if let Some(obj) = self.objects.get(&source) {
            if obj.spectre_gunship_update.is_some() {
                return Some(obj.get_position());
            }
            if let Some(gid) = obj
                .spectre_gunship_deployment
                .as_ref()
                .and_then(|d| d.gunship_id)
            {
                return self
                    .objects
                    .get(&gid)
                    .filter(|g| g.is_alive())
                    .map(|g| g.get_position());
            }
        }
        self.objects.values().find_map(|obj| {
            (obj.producer_id == Some(source)
                && obj.spectre_gunship_update.is_some()
                && obj.is_alive())
            .then(|| obj.get_position())
        })
    }

    /// C++ `PartitionFilterLiveMapEnemies` / leftover `relationship_to`:
    /// real-team ENEMIES only. Disguise is not applied here — C++ uses
    /// `getRelationship` on the real object, then
    /// `PartitionFilterStealthedAndUndetected` exempts `isDisguisedAsEnemy`.
    /// Missing owner ids fall back to faction residual (other playable team).
    fn spectre_orbit_relationship_enemies_ids(
        &self,
        source_id: ObjectId,
        source_team: Team,
        target_id: ObjectId,
        target_team: Team,
    ) -> bool {
        let (src_owner, src_inst) = self
            .objects
            .get(&source_id)
            .map(|o| (o.owner_player_id, o.team_instance_name.clone()))
            .unwrap_or((None, String::new()));
        let (tgt_owner, tgt_inst) = self
            .objects
            .get(&target_id)
            .map(|o| (o.owner_player_id, o.team_instance_name.clone()))
            .unwrap_or((None, String::new()));
        self.spectre_orbit_owners_are_enemies(
            src_owner,
            &src_inst,
            source_team,
            tgt_owner,
            &tgt_inst,
            target_team,
        )
    }

    fn spectre_orbit_owners_are_enemies(
        &self,
        src_owner: Option<u32>,
        src_inst: &str,
        source_team: Team,
        tgt_owner: Option<u32>,
        tgt_inst: &str,
        target_team: Team,
    ) -> bool {
        if src_owner.is_some() && tgt_owner.is_some() {
            return Self::object_relationship_from_owners(
                &self.players,
                src_owner,
                src_inst,
                tgt_owner,
                tgt_inst,
            ) == gamelogic::common::Relationship::Enemies;
        }
        target_team != source_team && target_team != Team::Neutral && source_team != Team::Neutral
    }

    /// C++ `StealthUpdate::getDisguisedPlayerIndex` → `getNthPlayer`.
    /// Live stores `disguise_as_team`; pick that team's controlling player,
    /// not the truck's own owner (same-faction FFA).
    fn spectre_orbit_disguised_player_id(&self, target_id: ObjectId) -> Option<u32> {
        let obj = self.objects.get(&target_id)?;
        let team = obj.disguise_as_team?;
        let owner = obj.owner_player_id;
        let mut first = None;
        let mut copied = None;
        for player in self.players.values() {
            if player.team != team {
                continue;
            }
            if first.is_none() {
                first = Some(player.id);
            }
            if owner.is_none_or(|id| player.id != id) {
                copied = Some(player.id);
            }
        }
        copied.or(first)
    }

    /// Leftover `SpectreGunshipUpdate::is_disguised_as_enemy`.
    /// KINDOF_DISGUISER + OBJECT_STATUS_DISGUISED + gunship relationship to
    /// the disguise (apparent) player's default team is ENEMIES.
    fn spectre_orbit_is_disguised_as_enemy(
        &self,
        source_id: ObjectId,
        source_team: Team,
        target_id: ObjectId,
    ) -> bool {
        let Some(target) = self.objects.get(&target_id) else {
            return false;
        };
        if !target.is_kind_of(KindOf::Disguiser) || !target.status.disguised {
            return false;
        }
        let Some(disguise_team) = target.disguise_as_team else {
            return false;
        };
        let (src_owner, src_inst) = self
            .objects
            .get(&source_id)
            .map(|o| (o.owner_player_id, o.team_instance_name.clone()))
            .unwrap_or((None, String::new()));
        crate::game_logic::special_power_strikes::spectre_orbit_is_disguised_as_enemy(
            true,
            true,
            self.spectre_orbit_owners_are_enemies(
                src_owner,
                &src_inst,
                source_team,
                self.spectre_orbit_disguised_player_id(target_id),
                "",
                disguise_team,
            ),
        )
    }

    /// C++ `PartitionFilterFreeOfFog`: `getShroudedStatus == OBJECTSHROUD_CLEAR`.
    /// No FOW / no grid / no PartitionData → CLEAR (Object.cpp:1786-1788).
    fn spectre_orbit_fog_clear(&self, viewer_player_id: Option<u32>, target_id: ObjectId) -> bool {
        if !self.skirmish_rules.fog_of_war {
            return true;
        }
        let Some(pid) = viewer_player_id else {
            return true;
        };
        let Ok(mgr) = get_shroud_manager().lock() else {
            return true;
        };
        if !mgr.has_shroud_grid() {
            return true;
        }
        match mgr.get_host_object_shroud_status(pid, target_id.0) {
            Some(gamelogic::common::types::ObjectShroudStatus::Clear) => true,
            Some(_) => false,
            None => true,
        }
    }

    /// Live residual of C++ Spectre acquire filters (stealth / fog / neutral / air).
    ///
    /// Stealth gate is leftover `find_target_in_radius`: STEALTHED && !DETECTED
    /// unless `is_disguised_as_enemy`. `is_effectively_stealthed` is wrong —
    /// DISGUISED clears that flag so any disguised Bomb Truck would pass and
    /// then real-team ENEMIES would shoot a friendly-presenting truck.
    fn spectre_orbit_target_allowed_by_id(
        &self,
        source_id: ObjectId,
        source_team: Team,
        source_player_id: Option<u32>,
        target_id: ObjectId,
    ) -> bool {
        let Some((alive, stealthed, detected, is_air, team)) =
            self.objects.get(&target_id).map(|t| {
                (
                    t.is_alive(),
                    t.status.stealthed,
                    t.status.detected,
                    t.is_kind_of(KindOf::Aircraft) || t.status.airborne_target,
                    t.team,
                )
            })
        else {
            return false;
        };
        let stealthed_undetected =
            crate::game_logic::special_power_strikes::spectre_orbit_stealthed_undetected_blocks(
                stealthed,
                detected,
                self.spectre_orbit_is_disguised_as_enemy(source_id, source_team, target_id),
            );
        crate::game_logic::special_power_strikes::spectre_orbit_target_passes_partition_filters(
            alive,
            self.spectre_orbit_relationship_enemies_ids(source_id, source_team, target_id, team),
            stealthed_undetected,
            is_air,
            self.spectre_orbit_fog_clear(source_player_id, target_id),
        )
    }

    #[cfg(test)]
    pub(in super::super) fn test_spectre_orbit_target_allowed_by_id(
        &self,
        source_id: ObjectId,
        source_team: Team,
        source_player_id: Option<u32>,
        target_id: ObjectId,
    ) -> bool {
        self.spectre_orbit_target_allowed_by_id(source_id, source_team, source_player_id, target_id)
    }

    #[cfg(test)]
    pub(in super::super) fn test_spectre_orbit_relationship_enemies_ids(
        &self,
        source_id: ObjectId,
        source_team: Team,
        target_id: ObjectId,
        target_team: Team,
    ) -> bool {
        self.spectre_orbit_relationship_enemies_ids(source_id, source_team, target_id, target_team)
    }

    fn spectre_orbit_source_viewer(
        &self,
        source_object: ObjectId,
        source_team: Team,
    ) -> Option<u32> {
        self.objects
            .get(&source_object)
            .and_then(|o| o.owner_player_id)
            .or_else(|| self.player_id_for_team(source_team))
    }

    /// Snapshot for `plan_due_orbit_ticks`: drop stealth / fog / neutral / air.
    fn spectre_orbit_filtered_positions(&self) -> Vec<(ObjectId, Vec3, Team, bool)> {
        let sources: Vec<(ObjectId, Team, Option<u32>)> = self
            .special_power_strikes
            .orbit_fields()
            .iter()
            .map(|f| {
                (
                    f.source_object,
                    f.source_team,
                    self.spectre_orbit_source_viewer(f.source_object, f.source_team),
                )
            })
            .collect();
        let ids: Vec<ObjectId> = self.objects.keys().copied().collect();
        ids.into_iter()
            .filter_map(|id| {
                let (pos, team, alive) = {
                    let obj = self.objects.get(&id)?;
                    (obj.get_position(), obj.team, obj.is_alive())
                };
                let allowed = if sources.is_empty() {
                    alive
                } else {
                    sources.iter().any(|(sid, steam, pid)| {
                        self.spectre_orbit_target_allowed_by_id(*sid, *steam, *pid, id)
                    })
                };
                allowed.then_some((id, pos, team, alive))
            })
            .collect()
    }

    /// Tick residual Spectre orbit fields spawned at orbit insertion.
    /// Fail-closed vs full SpectreGunshipUpdate gattling-strafe / howitzer projectile.
    pub(in super::super) fn update_spectre_orbit_fields(&mut self) {
        self.apply_pending_special_power_overrides();
        // C++ cease fire on isEffectivelyDead / cleanUp when the gunship is gone.
        // Do this before planning ticks so a dead gunship never lands another volley.
        let frame = self.frame;
        let dead_sources: Vec<ObjectId> = self
            .special_power_strikes
            .orbit_fields()
            .iter()
            .map(|f| f.source_object)
            .filter(|&src| self.spectre_orbit_source_gunship_is_dead(src))
            .collect();
        if !dead_sources.is_empty() {
            crate::game_logic::host_spectre_gunship_update::expire_orbit_fields_on_gunship_dead(
                self.special_power_strikes.orbit_fields_mut(),
                frame,
                |src| dead_sources.contains(&src),
            );
        }
        // Bind live ship position when a gunship exists. Keep the orbit-ring
        // stand-in when the residual host path has no ship object yet — C++
        // acquire always has getObject(); None here would fail-close gattling
        // on every caster-only orbit field.
        let gunship_pos: Vec<(u32, Option<Vec3>)> = self
            .special_power_strikes
            .orbit_fields()
            .iter()
            .map(|f| (f.id, self.spectre_orbit_gunship_position(f.source_object)))
            .collect();
        for (id, pos) in gunship_pos {
            if let Some(pos) = pos {
                if let Some(field) = self
                    .special_power_strikes
                    .orbit_fields_mut()
                    .iter_mut()
                    .find(|f| f.id == id)
                {
                    field.gunship_position = Some(pos);
                }
            }
        }
        self.special_power_strikes
            .advance_orbit_shoot_at(self.frame);

        let object_positions = self.spectre_orbit_filtered_positions();

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
                let source_player =
                    self.spectre_orbit_source_viewer(plan.source_object, plan.source_team);
                if !self.spectre_orbit_target_allowed_by_id(
                    plan.source_object,
                    plan.source_team,
                    source_player,
                    hit.target_id,
                ) {
                    continue;
                }
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
        self.special_power_strikes.advance_orbit_strafe(frame);
        self.spawn_spectre_gattling_strafe_smoke();

        self.special_power_strikes.prune_expired_orbit(frame);
    }

    /// C++ `SpectreGunshipUpdate.cpp:598-643` — leftover `createParticleSystem`
    /// at gattling target ±5 / ground height while firing and PARTIAL_CLEAR.
    fn spawn_spectre_gattling_strafe_smoke(&mut self) {
        use crate::game_logic::combat_particles::CombatParticleKind;
        use crate::game_logic::host_spectre_gunship_update::{
            spawn_spectre_gattling_strafe_smoke as spawn_leftover_strafe_smoke,
            spectre_gattling_strafe_smoke_impact, spectre_gunship_visible_for_strafe_fx,
            spectre_orbit_gattling_is_firing,
        };
        use crate::game_logic::special_power_strikes::SPECTRE_GATTLING_STRAFE_FX;
        use gamelogic::system::shroud_manager::get_shroud_manager;

        let frame = self.frame;
        let local = gamelogic::player::player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .filter(|&idx| idx >= 0)
            .map(|idx| idx as u32)
            .or_else(|| self.local_player_id());

        let shots: Vec<(ObjectId, Vec3)> = self
            .special_power_strikes
            .orbit_fields()
            .iter()
            .filter(|field| !field.is_expired(frame) && spectre_orbit_gattling_is_firing(field))
            .map(|field| (field.source_object, field.gattling_aim()))
            .collect();

        for (source, aim) in shots {
            let shroud = local.and_then(|pid| {
                get_shroud_manager()
                    .lock()
                    .ok()
                    .and_then(|mgr| mgr.get_host_object_shroud_status(pid, source.0))
            });
            if !spectre_gunship_visible_for_strafe_fx(shroud) {
                continue;
            }
            let impact = spectre_gattling_strafe_smoke_impact(aim);
            let _ = spawn_leftover_strafe_smoke(impact);
            let _ = self.combat_particles.spawn_named(
                CombatParticleKind::WeaponImpact,
                SPECTRE_GATTLING_STRAFE_FX,
                impact,
                frame,
                Some(source),
                None,
            );
        }
    }

    /// Tick residual Particle Uplink continuous beam fields after charge residual.
    /// Manual drive + WidthGrow grow/hold/decay + outer-node honesty residual closed.
    /// Intensity schedule (CHARGING/PREPARING/ALMOST_READY/POSTFIRE/PACKING) +
    /// BeamLaunchFX residual closed.
    /// Fail-closed vs full bone-extract lasers / GPU OuterBeamWidth matrix.
    /// Swath + DamagePulseRemnant residual closed.
    pub(in super::super) fn update_particle_beam_fields(&mut self) {
        let frame = self.frame;
        self.apply_pending_special_power_overrides();
        let aborting: std::collections::HashSet<ObjectId> = self
            .objects
            .iter()
            .filter(|(_, obj)| obj.puc_live_beam_abort_disabled())
            .map(|(id, _)| *id)
            .collect();
        self.special_power_strikes
            .abort_beam_fields_on_owner_disable(frame, |id| aborting.contains(&id));
        // Pre-fire intensity schedule + BeamLaunchFX + POSTFIRE/PACKING residual
        // (also advances ScudStorm PreAttack residual frame counter).
        self.special_power_strikes
            .advance_particle_intensity_schedule(frame);
        self.drain_puc_loop_audio();
        // Manual beam driving residual: advance current target toward override
        // before damage / scorch planning (retail update order).
        self.special_power_strikes.advance_manual_beam_drive(frame);

        let object_positions: Vec<(ObjectId, Vec3, Team, bool)> = self
            .objects
            .iter()
            .map(|(id, obj)| (*id, obj.get_position(), obj.team, obj.is_alive()))
            .collect();
        // C++ SwathOfDeath rotates onto building→target, not world +X.
        self.special_power_strikes
            .bind_beam_source_axes(&object_positions);

        let plans = self
            .special_power_strikes
            .plan_due_beam_ticks(frame, &object_positions);

        for plan in plans {
            let mut total_damage = 0.0_f32;
            let mut applications = 0_u32;
            let mut destroyed = 0_u32;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
            let source_template = self
                .objects
                .get(&plan.source_object)
                .map(|o| o.template_name.clone());
            let (damage_type, death_type) =
                crate::game_logic::special_power_strikes::particle_beam_authored_types(
                    source_template.as_deref(),
                );

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    // C++ ParticleUplinkCannonUpdate.cpp:633-634 module DamageType/DeathType.
                    let killed = target.take_damage_from_immediate_typed_death(
                        hit.damage,
                        Some(plan.source_object),
                        damage_type,
                        death_type,
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

        // TotalScorchMarks / GroundHitFX / RevealRange (retail STATUS_FIRING).
        // C++: addScorch + FXList::doFXPos(m_groundHitFX) then do/undoShroudReveal
        // at current target with RevealRange (instant vision pulse, not duration).
        let scorch_events = self
            .special_power_strikes
            .apply_due_beam_scorch_reveals(frame);
        for event in &scorch_events {
            crate::game_logic::special_power_strikes::apply_particle_beam_scorch_and_ground_hit_fx(
                event.position,
                event.scorch_radius,
            );
        }
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
            SPECTRE_HOWITZER_FIRE_SOUND, SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES,
            SPECTRE_HOWITZER_SHELL_MAX_HEALTH, SPECTRE_HOWITZER_SHELL_OBJECT,
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
            // C++ SpectreGunshipUpdate.cpp:585-586 — HowitzerFireSound on the
            // gunship after createAndFireTempWeapon (StrategyCenter_ArtilleryRound).
            let gunship_pos = self
                .objects
                .get(&source)
                .map(|o| o.get_position())
                .unwrap_or(pos);
            self.queue_audio_event(
                AudioEventRequest::new(SPECTRE_HOWITZER_FIRE_SOUND)
                    .with_object(source)
                    .with_position(gunship_pos)
                    .with_priority(150),
            );
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
            let (damage_type, death_type) =
                crate::game_logic::special_power_strikes::particle_remnant_authored_types();

            for hit in &plan.hits {
                if let Some(target) = self.objects.get_mut(&hit.target_id) {
                    if !target.is_alive() {
                        continue;
                    }
                    let killed = target.take_damage_from_immediate_typed_death(
                        hit.damage,
                        Some(plan.source_object),
                        damage_type,
                        death_type,
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

/// C++ AcademyStats::recordSpecialPowerUsed increments only when
/// `getAcademyClassificationType() == ACT_SUPERPOWER`.
fn special_power_is_act_superpower(power: &crate::command_system::SpecialPowerType) -> bool {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::HostSuperweaponKind;
    if HostSuperweaponKind::from_command_power(power).is_some() {
        return true;
    }
    // Retail SuperweaponCIAIntelligence / CommunicationsDownload AcademyClassify.
    if matches!(
        power,
        SpecialPowerType::CiaIntelligence | SpecialPowerType::CommunicationsDownload
    ) {
        return true;
    }
    leftover_special_power_is_act_superpower(power)
}

fn leftover_special_power_is_act_superpower(
    power: &crate::command_system::SpecialPowerType,
) -> bool {
    let Some(name) = leftover_academy_special_power_template_name(power) else {
        return false;
    };
    let rts = game_engine::common::rts::special_power::get_special_power_store();
    if let Some(template) = rts.find_template(name) {
        return template.get_academy_classification_type()
            == game_engine::common::rts::special_power::AcademyClassificationType::Superpower;
    }
    if let Some(store) = gamelogic::object::special_power_template::get_special_power_store() {
        if let Some(template) = store.find_special_power_template(name) {
            return matches!(
                template.get_academy_classification_type(),
                gamelogic::object::special_power_template::AcademyClassificationType::Superweapon
            );
        }
    }
    false
}

fn leftover_academy_special_power_template_name(
    power: &crate::command_system::SpecialPowerType,
) -> Option<&'static str> {
    use crate::command_system::SpecialPowerType;
    Some(match power {
        SpecialPowerType::CiaIntelligence => "SuperweaponCIAIntelligence",
        SpecialPowerType::CommunicationsDownload => "SpecialPowerCommunicationsDownload",
        SpecialPowerType::EmpPulse => "SuperweaponEMPPulse",
        SpecialPowerType::Frenzy | SpecialPowerType::EarlyFrenzy => "SuperweaponFrenzy",
        SpecialPowerType::GpsScrambler | SpecialPowerType::StealthGpsScrambler => {
            "SuperweaponGPSScrambler"
        }
        SpecialPowerType::LeafletDrop | SpecialPowerType::EarlyLeafletDrop => {
            "SuperweaponLeafletDrop"
        }
        SpecialPowerType::Paradrop | SpecialPowerType::InfantryParadrop => {
            "SuperweaponParadropAmerica"
        }
        SpecialPowerType::TankParadrop => "Tank_SuperweaponTankParadrop",
        SpecialPowerType::Ambush | SpecialPowerType::TerrorCell => "SuperweaponRebelAmbush",
        SpecialPowerType::SneakAttack => "SuperweaponSneakAttack",
        SpecialPowerType::ClusterMines | SpecialPowerType::NukeDrop => "SuperweaponClusterMines",
        SpecialPowerType::EmergencyRepair | SpecialPowerType::EarlyEmergencyRepair => {
            "SuperweaponEmergencyRepair"
        }
        SpecialPowerType::SpySatellite => "SpecialPowerSpySatellite",
        SpecialPowerType::SpyDrone => "SpecialPowerSpyDrone",
        SpecialPowerType::RadarScan => "SpecialPowerRadarVanScan",
        SpecialPowerType::CrateDrop => "SuperweaponCrateDrop",
        SpecialPowerType::CashHack => "SuperweaponCashHack",
        SpecialPowerType::CleanupArea => "SpecialAbilityAmbulanceCleanupArea",
        _ => return None,
    })
}
