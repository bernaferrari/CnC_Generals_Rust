//! Neutron, Scud, carpet bomb, artillery and A-10 strike flights.

use super::*;

impl GameLogic {
    /// C++ ApplyRandomForceNugget::create residual on primary (dying) object.
    /// C++ NeutronMissileUpdate::update residual.
    pub fn update_neutron_missile_flights(&mut self) {
        self.update_neutron_missile_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_neutron_missile_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        #[cfg(feature = "game_client")]
        let fx_operation_environment = self.observe_object_fx_environment();

        use crate::game_logic::host_neutron_missile_update::{
            NEUTRON_DEFAULT_BOUNDING_SPHERE, NeutronMissileFlightPhase, NeutronMissileWorld,
        };

        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.neutron_missile_update.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let others: Vec<(ObjectId, glam::Vec3)> = self
            .objects
            .registered()
            .filter(|(_, o)| o.is_alive())
            .map(|(id, o)| (*id, o.get_position()))
            .collect();
        let mut destroy = Vec::new();
        let mut intermediate_hits = 0u32;
        for id in ids {
            let (pos, producer, launcher, sphere) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                let data = o.neutron_missile_update.as_ref();
                (
                    o.get_position(),
                    o.producer_id,
                    data.and_then(|d| d.launcher_id),
                    data.map(|d| d.bounding_sphere_radius)
                        .unwrap_or(NEUTRON_DEFAULT_BOUNDING_SPHERE),
                )
            };
            let terrain_y = self.terrain_height_at(pos);
            let colliding_other = others.iter().find_map(|(oid, opos)| {
                if *oid == id {
                    return None;
                }
                if launcher == Some(oid.0) {
                    return None;
                }
                if (*opos - pos).length() <= sphere.max(0.0) {
                    Some(oid.0)
                } else {
                    None
                }
            });
            let (grounded, phase, is_cruise, producer, launch_fx, ignition_fx) = {
                #[cfg(feature = "game_client")]
                let fx_operation_environment = self.observe_object_fx_environment();
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let producer = o.producer_id.or(producer);
                let Some(data) = o.neutron_missile_update.as_mut() else {
                    continue;
                };
                let was_inter = data.reached_intermediate;
                let is_cruise = data.is_cruise;
                let tick = data.tick_world(
                    pos,
                    self.frame,
                    NeutronMissileWorld {
                        terrain_height_y: terrain_y,
                        bounding_sphere_radius: Some(sphere),
                        colliding_other,
                    },
                );
                if !was_inter && data.reached_intermediate {
                    intermediate_hits += 1;
                }
                let grounded = tick.grounded;
                let phase = tick.phase;
                let new_pos = tick.pos;
                let vel = tick.vel;
                let launch_fx = tick.launch_fx;
                let ignition_fx = tick.ignition_fx;
                drop(data);
                o.set_position_with_effects(
                    new_pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
                o.movement.velocity = vel;
                if vel.length_squared() > 1e-6 {
                    let yaw = vel.z.atan2(vel.x);
                    o.set_orientation_with_partition_owner(yaw, &mut self.partition_manager);
                }
                (grounded, phase, is_cruise, producer, launch_fx, ignition_fx)
            };
            if launch_fx || ignition_fx {
                // C++ NeutronMissileUpdate.cpp:226/241 FXList::doFXObj after
                // the tilted transform is set. No invented DeathExplosion.
                if launch_fx {
                    let _ = self.dispatch_fx_list_at_host_object_with_effects(
                        crate::game_logic::host_neutron_missile_update::NEUTRON_LAUNCH_FX,
                        id,
                        None,
                        presentation_effects,
                    );
                }
                if ignition_fx {
                    let _ = self.dispatch_fx_list_at_host_object_with_effects(
                        crate::game_logic::host_neutron_missile_update::NEUTRON_IGNITION_FX,
                        id,
                        None,
                        presentation_effects,
                    );
                }
            }
            if grounded || matches!(phase, NeutronMissileFlightPhase::Dead) {
                // Impact residuals: neutron SlowDeath vs cruise MOAB detonation.
                let (pos, team, producer) = self
                    .objects
                    .get(&id)
                    .map(|o| (o.get_position(), o.team, o.producer_id.or(producer)))
                    .unwrap_or((Vec3::ZERO, Team::Neutral, producer));
                let source = producer.unwrap_or(id);
                // Kill delivery decals on missile + launcher.
                #[cfg(feature = "game_client")]
                let fx_operation_environment = self.observe_object_fx_environment();
                if let Some(o) = self.objects.get_mut(&id) {
                    if let Some(rd) = o.radius_decal_update.as_mut() {
                        rd.kill_radius_decal();
                        self.radius_decal_update_reg.record_kill(false);
                    }
                }
                if let Some(lid) = producer {
                    #[cfg(feature = "game_client")]
                    let fx_operation_environment = self.observe_object_fx_environment();
                    if let Some(o) = self.objects.get_mut(&lid) {
                        if let Some(rd) = o.radius_decal_update.as_mut() {
                            rd.kill_radius_decal();
                            self.radius_decal_update_reg.record_kill(false);
                        }
                    }
                }
                if is_cruise {
                    use crate::game_logic::combat::DamageType;
                    use crate::game_logic::special_power_strikes::{
                        CRUISE_MISSILE_DAMAGE, CRUISE_MISSILE_RADIUS, MOAB_FLAME_DAMAGE,
                    };
                    let _ = self.apply_fuel_air_radius_damage_with_effects(
                        id,
                        producer,
                        team,
                        pos,
                        CRUISE_MISSILE_DAMAGE,
                        CRUISE_MISSILE_RADIUS,
                        DamageType::Explosive,
                        presentation_effects,
                    );
                    let _ = self.apply_fuel_air_radius_damage_with_effects(
                        id,
                        producer,
                        team,
                        pos,
                        MOAB_FLAME_DAMAGE,
                        CRUISE_MISSILE_RADIUS,
                        DamageType::Flame,
                        presentation_effects,
                    );
                } else {
                    {
                        let (fx_arg_0, fx_arg_1, fx_arg_2, fx_arg_3, fx_arg_4) =
                            (source, team, pos, self.frame, 0);
                        #[cfg(feature = "game_client")]
                        {
                            let player = self.local_player_id();
                            let receiver = &mut self.special_power_strikes;
                            Self::with_object_fx_services(
                                &mut self.session_view,
                                self.queued_audio_events.policy().clone(),
                                player,
                                self.world_services.shroud(),
                                presentation_effects,
                                &fx_operation_environment,
                                self.world_services.terrain(),
                                &self.logic_random,
                                |view| {
                                    receiver.spawn_neutron_slow_death_field_with_view(
                                        fx_arg_0, fx_arg_1, fx_arg_2, fx_arg_3, fx_arg_4, view,
                                    )
                                },
                            )
                        }
                        #[cfg(not(feature = "game_client"))]
                        {
                            self.special_power_strikes.spawn_neutron_slow_death_field(
                                fx_arg_0, fx_arg_1, fx_arg_2, fx_arg_3, fx_arg_4,
                            )
                        }
                    };
                    // C++ SlowDeath detonation carries the impact sound (FXList
                    // audio at the blast position) — the registry blob that also
                    // queues it is suppressed for live delivery.
                    self.queue_audio_event(
                        crate::game_logic::AudioEventRequest::new(
                            crate::game_logic::special_power_strikes::HostSuperweaponKind::NuclearMissile
                                .impact_audio())
                        .with_position(pos)
                        .with_priority(200),
                    );
                }
                #[cfg(feature = "game_client")]
                let fx_operation_environment = self.observe_object_fx_environment();
                if let Some(o) = self.objects.get_mut(&id) {
                    o.fire_create_object_die();
                }
                self.fire_owned_fx_list_die_with_effects(id, presentation_effects);
                self.apply_pending_create_object_die_with_effects(id, presentation_effects);
                self.neutron_missile_update_reg.record_ground();
                destroy.push(id);
            }
        }
        for _ in 0..intermediate_hits {
            self.neutron_missile_update_reg.record_intermediate();
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
    }

    /// C++ ScudStormMissile MissileAIUpdate ballistic residual tick.
    pub fn update_scud_storm_missile_flights(&mut self) {
        self.update_scud_storm_missile_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_scud_storm_missile_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            SCUD_STORM_PRIMARY_RADIUS, SCUD_STORM_SECONDARY_RADIUS,
        };

        // ClipSize staggered launches residual.
        self.spawn_due_scud_storm_missiles_with_effects(presentation_effects);

        let ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.scud_storm_missile_flight.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in ids {
            let (pos, ground) = {
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                (o.get_position(), o.ground_height)
            };
            let terrain_y = self.terrain_height_at(pos).unwrap_or(ground);
            let surface = self.ground_or_structure_height_at(pos, terrain_y);
            let structure_h = (surface - terrain_y).max(0.0);
            let (grounded, ignition_fx, target, producer) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let pos = o.get_position();
                let producer = o.producer_id;
                let Some(data) = o.scud_storm_missile_flight.as_mut() else {
                    continue;
                };
                let target = data.target;
                let tick = data.tick_at_surface(pos, self.frame, terrain_y, structure_h);
                let grounded = tick.grounded;
                let ignition_fx = tick.ignition_fx;
                let new_pos = tick.pos;
                let vel = tick.vel;
                drop(data);
                o.set_position_with_effects(
                    new_pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
                o.movement.velocity = vel;
                if vel.length_squared() > 1e-6 {
                    o.set_orientation_with_partition_owner(
                        vel.z.atan2(vel.x),
                        &mut self.partition_manager,
                    );
                }
                (grounded, ignition_fx, target, producer)
            };
            if ignition_fx {
                // C++ MissileAIUpdate.cpp:451-474 doIgnitionState doFXObj.
                // No invented DeathExplosion at ignition.
                let _ = self.dispatch_fx_list_at_host_object_with_effects(
                    crate::game_logic::special_power_strikes::SCUD_STORM_MISSILE_IGNITION_FX,
                    id,
                    None,
                    presentation_effects,
                );
                self.scud_storm_missile_flight_reg.record_ignition();
            } else if !grounded {
                // ScudMissileExhaust residual (trail honesty; sparse spawn).
                if self.frame % 5 == 0 {
                    let p = self
                        .objects
                        .get(&id)
                        .map(|o| o.get_position())
                        .unwrap_or(Vec3::ZERO);
                    let _ = self.combat_particles.spawn(
                        CombatParticleKind::DeathExplosion,
                        p,
                        self.frame,
                        Some(id),
                        None,
                    );
                    self.scud_storm_missile_flight_reg.record_exhaust();
                }
            }
            if grounded {
                let team = self
                    .objects
                    .get(&id)
                    .map(|o| o.team)
                    .unwrap_or(Team::Neutral);
                // Resolve Anthrax Beta/Gamma from producer owner sciences/upgrades.
                use crate::game_logic::special_power_strikes::ScudStormAnthraxTier;
                let anthrax = {
                    let mut names: Vec<String> = Vec::new();
                    if let Some(pid) = producer {
                        if let Some(o) = self.objects.get(&pid) {
                            // C++ getControllingPlayer()->hasScience, not faction union.
                            if let Some(owner) = self.player_owner_for_host_object(o) {
                                if let Some(p) = self.get_player(owner) {
                                    names.extend(p.unlocked_sciences.iter().cloned());
                                }
                            }
                        }
                    }
                    // Also accept Upgrade_ naming via sciences list residual.
                    ScudStormAnthraxTier::highest_from_upgrades(names.iter().map(|s| s.as_str()))
                };
                // ScudStormDamageWeapon primary + secondary residual at scatter impact.
                // Chem_GLAScudStorm residual: primary is anthrax warhead (no HE primary).
                let source = producer.unwrap_or(id);
                let chem_primary = producer
                    .and_then(|pid| self.objects.get(&pid))
                    .map(|o| {
                        crate::game_logic::host_scud_launcher::scud_uses_anthrax_primary(
                            &o.template_name,
                        ) || o.template_name.to_ascii_lowercase().contains("chem_")
                            && o.template_name.to_ascii_lowercase().contains("scudstorm")
                    })
                    .unwrap_or(false);
                if chem_primary {
                    // Toxin primary blast residual (use secondary damage as toxin splash).
                    let _ = self.apply_fuel_air_radius_damage_with_effects(
                        id,
                        producer,
                        team,
                        target,
                        anthrax
                            .secondary_damage()
                            .max(anthrax.primary_damage() * 0.4),
                        SCUD_STORM_SECONDARY_RADIUS,
                        DamageType::Toxin,
                        presentation_effects,
                    );
                } else {
                    let _ = self.apply_fuel_air_radius_damage_with_effects(
                        id,
                        producer,
                        team,
                        target,
                        anthrax.primary_damage(),
                        SCUD_STORM_PRIMARY_RADIUS,
                        DamageType::Explosive,
                        presentation_effects,
                    );
                    let _ = self.apply_fuel_air_radius_damage_with_effects(
                        id,
                        producer,
                        team,
                        target,
                        anthrax.secondary_damage(),
                        SCUD_STORM_SECONDARY_RADIUS,
                        DamageType::Explosive,
                        presentation_effects,
                    );
                }
                // DeathFire OCL poison field residual (OCL_PoisonFieldLarge / upgraded).
                let _ = self
                    .special_power_strikes
                    .spawn_scud_poison_field_with_tier(
                        source, team, target, self.frame, 0, anthrax,
                    );
                self.scud_storm_missile_flight_reg.record_ground();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
    }

    /// C++ SUPERWEAPON_CarpetBomb DeliverPayload residual.
    pub fn spawn_carpet_bomb_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::CarpetBombFactionTier,
    ) -> Option<ObjectId> {
        self.spawn_carpet_bomb_flight_with_effects(
            source_id,
            target,
            tier,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_carpet_bomb_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::CarpetBombFactionTier,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_carpet_bomb_flight::HostCarpetBombFlightData;
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        // C++ CREATE_AT_EDGE_NEAR_SOURCE: TerrainLogic::findClosestEdgePoint(owner).
        let mut edge = self.closest_map_edge_point(source_pos);
        edge.y = 150.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
        let transport = tier.transport();
        if !self.templates.contains_key(transport) {
            let mut t = ThingTemplate::new(transport);
            t.set_health(500.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(transport.to_string(), t);
        }
        let tid = self.create_object_with_script_effects(
            transport,
            team,
            edge,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            let mut data = HostCarpetBombFlightData::start(edge, target, tier);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.carpet_bomb_transport = Some(data);
            o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
        }
        self.carpet_bomb_flight_reg.record_transport();
        self.carpet_bomb_flight_reg.schedule_drops(
            self.frame,
            source_id.0,
            target,
            edge,
            tier,
            tid.0,
        );
        // C++ DeliverPayloadAIUpdate::deliverPayload createRadiusDecal (formation 0).
        let _ = self.create_delivery_radius_decal_with_effects(
            tid,
            target,
            tier.delivery_decal_radius(),
            presentation_effects,
        );
        Some(tid)
    }

    pub fn update_carpet_bomb_flights(&mut self) {
        self.update_carpet_bomb_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_carpet_bomb_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            CARPET_BOMB_DAMAGE, CARPET_BOMB_PAYLOAD_OBJECT, CARPET_BOMB_RADIUS,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        // Move transports. C++ HeadOffMapState + CleanUpState: fly off then destroy.
        let tids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.carpet_bomb_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut leave = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.carpet_bomb_transport.as_mut() else {
                continue;
            };
            let (new_pos, vel, at_exit) = data.tick_transport(pos);
            drop(data);
            o.set_position_with_effects_and_pose_owners(
                new_pos,
                self.frame,
                &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                &self.host_trigger_world,
                &self.team_factory,
            );
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation_with_partition_owner(
                    vel.z.atan2(vel.x),
                    &mut self.partition_manager,
                );
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }

        // C++ payload is contained on the transport. Dead bomber cancels remaining drops.
        let carpet_ids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.carpet_bomb_transport.is_some())
            .map(|(id, _)| *id)
            .collect();
        let mut alive_transports = Vec::new();
        for id in carpet_ids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let alive = o.is_alive();
            if let Some(data) = o.carpet_bomb_transport.as_mut() {
                data.transport_alive = alive;
                if data.transport_alive {
                    alive_transports.push(id.0);
                }
            }
        }
        let due = self
            .carpet_bomb_flight_reg
            .take_due_drops(self.frame, &alive_transports);
        if !due.is_empty() {
            if !self.templates.contains_key(CARPET_BOMB_PAYLOAD_OBJECT) {
                let mut t = ThingTemplate::new(CARPET_BOMB_PAYLOAD_OBJECT);
                t.set_health(100.0).add_kind_of(KindOf::Projectile);
                self.templates
                    .insert(CARPET_BOMB_PAYLOAD_OBJECT.to_string(), t);
            }
            for p in due {
                let team = self
                    .objects
                    .get(&ObjectId(p.source_id))
                    .map(|o| o.team)
                    .unwrap_or(Team::Neutral);
                // Drop from above target residual.
                let drop_pos = Vec3::new(p.target.x, 80.0, p.target.z);
                if let Some(bid) = self.create_object_with_script_effects(
                    CARPET_BOMB_PAYLOAD_OBJECT,
                    team,
                    drop_pos,
                    None,
                    presentation_effects,
                ) {
                    if let Some(o) = self.objects.get_mut(&bid) {
                        o.producer_id = Some(ObjectId(p.source_id));
                        o.carpet_bomb_payload = true;
                        o.movement.velocity = Vec3::new(0.0, -15.0, 0.0);
                        let _ = o.set_smart_bomb_target(p.target);
                    }
                    self.carpet_bomb_flight_reg.record_drop();
                }
            }
        }
        // C++ DeliveringState success → HeadOffMapState (not while payload remains).
        let pending_tids: std::collections::HashSet<u32> = self
            .carpet_bomb_flight_reg
            .pending_drops
            .iter()
            .map(|p| p.transport_id)
            .collect();
        let mark: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.carpet_bomb_transport.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in mark {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            if let Some(data) = o.carpet_bomb_transport.as_mut() {
                if !pending_tids.contains(&id.0) {
                    data.delivery_complete = true;
                }
            }
        }

        // Fall payloads and detonate near ground.
        let bombs: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.carpet_bomb_payload && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in bombs {
            let (pos, producer, team) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let mut p = o.get_position();
                p.y += o.movement.velocity.y;
                o.set_position_with_effects(
                    p,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
                (p, o.producer_id, o.team)
            };
            if pos.y <= 5.0 {
                let (dmg, hits, destroyed) = self.apply_fuel_air_radius_damage_with_effects(
                    id,
                    producer,
                    team,
                    Vec3::new(pos.x, 0.0, pos.z),
                    CARPET_BOMB_DAMAGE,
                    CARPET_BOMB_RADIUS,
                    DamageType::Explosive,
                    presentation_effects,
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                // C++ CarpetBombWeapon HeightDie detonation audio.
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(
                        crate::game_logic::special_power_strikes::HostSuperweaponKind::CarpetBomb
                            .impact_audio(),
                    )
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(200),
                );
                self.carpet_bomb_flight_reg.record_impact();
                let final_wave = self.carpet_bomb_flight_reg.impacts
                    >= self.carpet_bomb_flight_reg.bombs_scheduled;
                // Credit the live payload warhead onto the queued strike
                // (C++ DeliverPayload payload fires its own CarpetBombWeapon).
                self.special_power_strikes.credit_live_flight_impact(
                    crate::game_logic::special_power_strikes::HostSuperweaponKind::CarpetBomb,
                    dmg,
                    hits,
                    destroyed,
                    1,
                    final_wave,
                    Vec3::new(pos.x, 0.0, pos.z),
                );
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
    }

    /// C++ SUPERWEAPON_ArtilleryBarrage DeliverPayload residual.
    /// CREATE_AT_EDGE_FARTHEST_FROM_TARGET (z += 300) + FormationSize 12/24/36.
    pub fn spawn_artillery_barrage_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier,
    ) -> Option<ObjectId> {
        self.spawn_artillery_barrage_flight_with_effects(
            source_id,
            target,
            tier,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_artillery_barrage_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_artillery_barrage_flight::HostArtilleryBarrageFlightData;
        use crate::game_logic::host_ocl_special_power::OCL_CREATE_ABOVE_LOCATION_HEIGHT;
        use crate::game_logic::host_spectre_gunship_deployment::find_farthest_edge_point_residual;
        use crate::game_logic::special_power_strikes::{
            ARTILLERY_BARRAGE_CANNON_MAX_HEALTH, ARTILLERY_BARRAGE_DECAL_RADIUS,
            ARTILLERY_BARRAGE_DELIVERY_DISTANCE, ARTILLERY_BARRAGE_FORMATION_SPACING,
            ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, ARTILLERY_BARRAGE_PREFERRED_HEIGHT,
            ARTILLERY_BARRAGE_TRANSPORT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};
        use gamelogic::object_creation_list::DeliverPayloadNugget;

        let team = self
            .objects
            .get(&source_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        // C++ CREATE_AT_EDGE_FARTHEST_FROM_TARGET + CREATE_ABOVE_LOCATION_HEIGHT.
        let mut edge = find_farthest_edge_point_residual(
            target,
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
            0.0,
        );
        edge.y += OCL_CREATE_ABOVE_LOCATION_HEIGHT;
        // C++ StartAtPreferredHeight overwrites z after deliverPayload.
        let spawn_height = ARTILLERY_BARRAGE_PREFERRED_HEIGHT.max(edge.y);
        // Leftover Coord3D is C++ x/y ground, z height. Host is x/z ground, y height.
        let primary = Vec3::new(edge.x, edge.z, edge.y);
        let secondary = Vec3::new(target.x, target.z, target.y);
        if !self.templates.contains_key(ARTILLERY_BARRAGE_TRANSPORT) {
            let mut t = ThingTemplate::new(ARTILLERY_BARRAGE_TRANSPORT);
            t.set_health(ARTILLERY_BARRAGE_CANNON_MAX_HEALTH)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates
                .insert(ARTILLERY_BARRAGE_TRANSPORT.to_string(), t);
        }
        let formation_size = tier.formation_size().max(1);
        let mut first = None;
        // One DelayDelivery roll per transport, honored by both the transport
        // hold and the shell schedule (ObjectCreationList.cpp:375-378).
        let mut transport_delays: Vec<u32> = Vec::with_capacity(formation_size as usize);
        for i in 0..formation_size {
            // Leftover DeliverPayloadNugget::formation_flight_pose matches C++.
            // WeaponErrorRadius is applied by schedule_drops, not spawn offsets.
            let pose = DeliverPayloadNugget::formation_flight_pose(
                &primary,
                &secondary,
                i as i32,
                formation_size as i32,
                ARTILLERY_BARRAGE_FORMATION_SPACING,
                0.0,
                ARTILLERY_BARRAGE_DELIVERY_DISTANCE,
                None,
            );
            let mut launch = Vec3::new(pose.start_pos.x, pose.start_pos.z, pose.start_pos.y);
            launch.y = spawn_height;
            let mut move_to = Vec3::new(pose.move_to_pos.x, pose.move_to_pos.z, pose.move_to_pos.y);
            move_to.y = spawn_height;
            let delay = if ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES > 0 {
                self.logic_random
                    .next_int(0, ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES as i32)
                    .max(0) as u32
            } else {
                0
            };
            transport_delays.push(delay);
            let tid = self.create_object_with_script_effects(
                ARTILLERY_BARRAGE_TRANSPORT,
                team,
                launch,
                None,
                presentation_effects,
            )?;
            if let Some(o) = self.objects.get_mut(&tid) {
                o.note_producer(source_id);
                let mut data = HostArtilleryBarrageFlightData::start(launch, move_to, tier);
                data.map_min = self.world_min;
                data.map_max = self.world_max;
                data.delay_until_frame = self.frame.saturating_add(delay);
                o.artillery_barrage_transport = Some(data);
                o.set_orientation_with_partition_owner(pose.orient, &mut self.partition_manager);
            }
            self.artillery_barrage_flight_reg.record_transport();
            if first.is_none() {
                first = Some(tid);
                // C++ only formation index 0 gets a DeliveryDecal ring.
                let _ = self.create_delivery_radius_decal_with_effects(
                    tid,
                    target,
                    ARTILLERY_BARRAGE_DECAL_RADIUS,
                    presentation_effects,
                );
            }
        }
        self.artillery_barrage_flight_reg.schedule_drops(
            self.frame,
            source_id.0,
            target,
            tier,
            &transport_delays,
        );
        first
    }

    pub fn update_artillery_barrage_flights(&mut self) {
        self.update_artillery_barrage_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_artillery_barrage_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_RADIUS, ARTILLERY_BARRAGE_SHELL_OBJECT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let tids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.artillery_barrage_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut leave = Vec::new();
        let world_min = self.world_min;
        let world_max = self.world_max;
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.artillery_barrage_transport.as_mut() else {
                continue;
            };
            if !data.map_extent_ok() {
                data.map_min = world_min;
                data.map_max = world_max;
            }
            // C++ setDisabledUntil(DISABLED_DEFAULT, DelayDeliveryMax) — hold until ready.
            if data.is_hold_for_delay(self.frame) {
                continue;
            }
            let should_drop = !data.delivery_complete && data.in_delivery_band(pos);
            if should_drop {
                data.delivery_complete = true;
            }
            let (new_pos, vel, at_exit) = data.tick_transport(pos);
            let _ = data;
            if should_drop {
                o.kill_delivery_radius_decal();
            }
            o.set_position_with_effects_and_pose_owners(
                new_pos,
                self.frame,
                &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                &self.host_trigger_world,
                &self.team_factory,
            );
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation_with_partition_owner(
                    vel.z.atan2(vel.x),
                    &mut self.partition_manager,
                );
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }

        let due = self.artillery_barrage_flight_reg.take_due_drops(self.frame);
        if !due.is_empty() {
            if !self.templates.contains_key(ARTILLERY_BARRAGE_SHELL_OBJECT) {
                let mut t = ThingTemplate::new(ARTILLERY_BARRAGE_SHELL_OBJECT);
                t.set_health(50.0).add_kind_of(KindOf::Projectile);
                self.templates
                    .insert(ARTILLERY_BARRAGE_SHELL_OBJECT.to_string(), t);
            }
            for p in due {
                let team = self
                    .objects
                    .get(&ObjectId(p.source_id))
                    .map(|o| o.team)
                    .unwrap_or(Team::Neutral);
                let drop_pos = Vec3::new(p.target.x, 100.0, p.target.z);
                if let Some(sid) = self.create_object_with_script_effects(
                    ARTILLERY_BARRAGE_SHELL_OBJECT,
                    team,
                    drop_pos,
                    None,
                    presentation_effects,
                ) {
                    if let Some(o) = self.objects.get_mut(&sid) {
                        o.producer_id = Some(ObjectId(p.source_id));
                        o.artillery_barrage_shell = true;
                        o.movement.velocity = Vec3::new(0.0, -18.0, 0.0);
                        let _ = o.set_smart_bomb_target(p.target);
                    }
                    self.artillery_barrage_flight_reg.record_drop();
                }
            }
        }

        let shells: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.artillery_barrage_shell && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in shells {
            let (pos, producer, team) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                let mut p = o.get_position();
                p.y += o.movement.velocity.y;
                o.set_position_with_effects(
                    p,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
                (p, o.producer_id, o.team)
            };
            if pos.y <= 5.0 {
                let _ = self.apply_fuel_air_radius_damage_with_effects(
                    id,
                    producer,
                    team,
                    Vec3::new(pos.x, 0.0, pos.z),
                    ARTILLERY_BARRAGE_DAMAGE,
                    ARTILLERY_BARRAGE_RADIUS,
                    DamageType::Explosive,
                    presentation_effects,
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                self.artillery_barrage_flight_reg.record_impact();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
    }

    /// C++ SUPERWEAPON_A10ThunderboltMissileStrike DeliverPayload residual.
    /// CREATE_AT_EDGE_NEAR_SOURCE + FormationSize 1/2/3 AmericaJetA10Thunderbolt.
    pub fn spawn_a10_strike_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier,
    ) -> Option<ObjectId> {
        self.spawn_a10_strike_flight_for_delivery_with_effects(
            source_id,
            target,
            tier,
            None,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_a10_strike_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.spawn_a10_strike_flight_for_delivery_with_effects(
            source_id,
            target,
            tier,
            None,
            presentation_effects,
        )
    }

    pub(crate) fn spawn_a10_strike_flight_for_strike_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier,
        strike_id: u32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        self.spawn_a10_strike_flight_for_delivery_with_effects(
            source_id,
            target,
            tier,
            Some(strike_id),
            presentation_effects,
        )
    }

    pub(super) fn spawn_a10_strike_flight_for_delivery_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::special_power_strikes::A10StrikeScienceTier,
        strike_id: Option<u32>,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_a10_strike_flight::HostA10StrikeFlightData;
        use crate::game_logic::special_power_strikes::{A10_FORMATIONION_SPACING, A10_TRANSPORT};
        use crate::game_logic::{KindOf, ThingTemplate};

        let source = self
            .objects
            .get(&source_id)
            .filter(|source| source.is_alive())?;
        source.owner_player_id?;
        let source_identity = source.visual_draw_identity_token.clone();
        let source_pos = source.get_position();
        let delivery_id = self
            .a10_strike_flight_reg
            .begin_delivery(source_id, tier, strike_id)?;
        let edge = self.closest_map_edge_point(source_pos);
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
        let exit = crate::game_logic::host_deliver_payload::head_off_map_exit_point_residual(
            edge,
            dx,
            dz,
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
        );
        let dist = (dx * dx + dz * dz).sqrt().max(1.0);
        let px = -dz / dist;
        let pz = dx / dist;
        if self.receiving_module_content.is_none() && !self.templates.contains_key(A10_TRANSPORT) {
            let mut t = ThingTemplate::new(A10_TRANSPORT);
            t.set_health(600.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(A10_TRANSPORT.to_string(), t);
        }
        let jets = tier.formation_size().max(1);
        let half = (jets as f32 - 1.0) * 0.5;
        let mut first = None;
        for j in 0..jets {
            let lat = (j as f32 - half) * A10_FORMATIONION_SPACING;
            let launch = Vec3::new(edge.x + px * lat, 160.0, edge.z + pz * lat);
            // A partial successful formation remains a real partial delivery;
            // the launch_complete bit prevents snapshots during this callback
            // cohort from claiming its factory stack can be resumed.
            let Some(owner) = self
                .objects
                .get(&source_id)
                .filter(|source| {
                    std::sync::Arc::ptr_eq(&source.visual_draw_identity_token, &source_identity)
                })
                .and_then(|source| source.owner_player_id)
            else {
                break;
            };
            let Some(tid) = self.create_object_for_player_with_effects(
                A10_TRANSPORT,
                owner,
                launch,
                None,
                presentation_effects,
            ) else {
                continue;
            };
            if let Some(o) = self.objects.get_mut(&tid) {
                o.note_producer(source_id);
                let mut data = HostA10StrikeFlightData::start_with_exit(launch, target, exit, tier);
                data.map_min = self.world_min;
                data.map_max = self.world_max;
                o.a10_strike_transport = Some(data);
                o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
            }
            let bound = self.a10_strike_flight_reg.bind_transport(delivery_id, tid);
            assert!(bound, "new A10 transport ID must bind exactly once");
            let identity = &self
                .objects
                .get(&tid)
                .expect("newly bound A10 actor")
                .visual_draw_identity_token;
            assert!(
                self.a10_strike_flight_reg
                    .bind_transport_lifetime(delivery_id, tid, identity),
                "new A10 transport lifetime must bind exactly once"
            );
            if first.is_none() {
                first = Some(tid);
                let _ = self.create_delivery_radius_decal_with_effects(
                    tid,
                    target,
                    crate::game_logic::special_power_strikes::A10_DELIVERY_DECAL_RADIUS,
                    presentation_effects,
                );
            }
        }
        let _ = self
            .a10_strike_flight_reg
            .finish_delivery_launch(delivery_id);
        first
    }

    /// C++ TerrainLogic::findClosestEdgePoint residual (world_min/max).
    pub fn closest_map_edge_point(&self, pos: Vec3) -> Vec3 {
        let min = self.world_min;
        let max = self.world_max;
        let dl = (pos.x - min.x).abs();
        let dr = (max.x - pos.x).abs();
        let db = (pos.z - min.z).abs();
        let dt = (max.z - pos.z).abs();
        let m = dl.min(dr).min(db).min(dt);
        if (m - dl).abs() <= f32::EPSILON {
            Vec3::new(min.x, 160.0, pos.z.clamp(min.z, max.z))
        } else if (m - dr).abs() <= f32::EPSILON {
            Vec3::new(max.x, 160.0, pos.z.clamp(min.z, max.z))
        } else if (m - db).abs() <= f32::EPSILON {
            Vec3::new(pos.x.clamp(min.x, max.x), 160.0, min.z)
        } else {
            Vec3::new(pos.x.clamp(min.x, max.x), 160.0, max.z)
        }
    }

    pub fn update_a10_strike_flights(&mut self) {
        self.update_a10_strike_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }
}
