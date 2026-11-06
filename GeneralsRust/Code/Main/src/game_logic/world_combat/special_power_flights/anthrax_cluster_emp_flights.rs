//! Anthrax, cluster mine and EMP flights plus frenzy markers.

use super::*;

impl GameLogic {
    pub fn update_anthrax_bomb_flights(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_anthrax_bomb_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_anthrax_bomb_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_anthrax_bomb_flight::anthrax_payload_drop_pos;
        use crate::game_logic::special_power_strikes::{
            ANTHRAX_BOMB_IMPACT_DAMAGE, ANTHRAX_BOMB_IMPACT_RADIUS,
        };

        let tids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.anthrax_bomb_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(
            Team,
            Vec3,
            ObjectId,
            crate::game_logic::host_anthrax_bomb_flight::AnthraxBombPayloadTier,
            Vec3,
            Vec3,
        )> = Vec::new();
        let mut leave = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.anthrax_bomb_transport.as_mut() else {
                continue;
            };
            if !data.map_extent_ok() {
                data.map_min = self.world_min;
                data.map_max = self.world_max;
            }
            let should_drop = !data.delivery_complete && data.in_delivery_band(pos);
            if should_drop {
                data.delivery_complete = true;
            }
            let (new_pos, vel, at_exit) = data.tick_transport(pos);
            let target = data.target;
            let tier = data.tier;
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
            o.movement.target_position = None;
            if vel.length_squared() > 1e-6 {
                o.set_orientation_with_partition_owner(
                    vel.z.atan2(vel.x),
                    &mut self.partition_manager,
                );
            }
            if should_drop {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                drops.push((team, target, producer, tier, new_pos, vel));
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        for (team, target, producer, tier, plane_pos, plane_vel) in drops {
            let bomb = tier.bomb();
            let drop_pos = anthrax_payload_drop_pos(plane_pos);
            if let Some(bid) = self.create_object_with_script_effects(
                bomb,
                team,
                drop_pos,
                None,
                presentation_effects,
            ) {
                if let Some(o) = self.objects.get_mut(&bid) {
                    o.producer_id = Some(producer);
                    o.anthrax_bomb_payload = true;
                    o.movement.velocity = Vec3::new(plane_vel.x, -14.0, plane_vel.z);
                    let _ = o.set_smart_bomb_target(target);
                }
                self.anthrax_bomb_flight_reg.record_drop();
            }
        }

        let bombs: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.anthrax_bomb_payload && o.is_alive())
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
                let impact = Vec3::new(pos.x, 0.0, pos.z);
                let _ = self.apply_fuel_air_radius_damage_with_effects(
                    id,
                    producer,
                    team,
                    impact,
                    ANTHRAX_BOMB_IMPACT_DAMAGE,
                    ANTHRAX_BOMB_IMPACT_RADIUS,
                    DamageType::Explosive,
                    presentation_effects,
                );
                // C++ FireOCL OCL_PoisonFieldAnthraxBomb / AnthraxGammaBomb.
                let src = producer.unwrap_or(id);
                let toxin_object = {
                    let bomb_name = self
                        .objects
                        .get(&id)
                        .map(|o| o.template_name.clone())
                        .unwrap_or_default();
                    if bomb_name.to_ascii_lowercase().contains("gamma") {
                        crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_OBJECT_NAME_GAMMA
                    } else {
                        crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_OBJECT_NAME
                    }
                };
                let _ = self.special_power_strikes.spawn_toxin_field_with_params(
                    src,
                    team,
                    impact,
                    self.frame,
                    0,
                    crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_DAMAGE_PER_TICK,
                    crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_RADIUS,
                    crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES,
                    crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_DURATION_FRAMES,
                    toxin_object,
                );
                // C++ AnthraxBomb death audio residuals on the live path:
                // FXListDie DeathFX = FX_AnthraxBomb (WeaponObjects.ini
                // AnthraxBomb ModuleTag_09) and the spawned
                // PoisonFieldAnthraxBomb SoundAmbient = AnthraxPoolAmbientLoop.
                // The registry fallback path queues both via the strike plan;
                // live delivery must not skip them.
                self.queue_audio_event(
                    AudioEventRequest::new(
                        crate::game_logic::special_power_strikes::HostSuperweaponKind::AnthraxBomb
                            .impact_audio(),
                    )
                    .with_position(impact)
                    .with_priority(200),
                );
                self.queue_audio_event(
                    AudioEventRequest::new(
                        crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_AUDIO,
                    )
                    .with_position(impact)
                    .with_priority(150),
                );
                self.anthrax_bomb_flight_reg.record_toxin_field();
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                self.anthrax_bomb_flight_reg.record_detonation();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        Ok(())
    }

    /// C++ OCL_CreateSneakAttackTunnelStart → GLASneakAttackTunnelNetworkStart residual.
    pub fn spawn_sneak_attack_tunnel_start(
        &mut self,
        mission_id: u32,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        placement_angle: f32,
    ) -> Option<ObjectId> {
        self.spawn_sneak_attack_tunnel_start_with_effects(
            mission_id,
            team,
            owner_player_id,
            position,
            placement_angle,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_sneak_attack_tunnel_start_with_effects(
        &mut self,
        mission_id: u32,
        team: Team,
        owner_player_id: Option<u32>,
        position: Vec3,
        placement_angle: f32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_sneak_attack::SNEAK_ATTACK_TUNNEL_START_TEMPLATE;
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self
            .templates
            .contains_key(SNEAK_ATTACK_TUNNEL_START_TEMPLATE)
        {
            let mut t = ThingTemplate::new(SNEAK_ATTACK_TUNNEL_START_TEMPLATE);
            t.add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(SNEAK_ATTACK_TUNNEL_START_TEMPLATE.to_string(), t);
        }
        let sid = self.create_object_for_owner_or_team_with_effects(
            SNEAK_ATTACK_TUNNEL_START_TEMPLATE,
            team,
            owner_player_id,
            position,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&sid) {
            o.sneak_tunnel_start = true;
            o.set_orientation_with_partition_owner(placement_angle, &mut self.partition_manager);
        }
        self.host_sneak_attacks.record_tunnel_start(mission_id, sid);
        Some(sid)
    }

    /// C++ SUPERWEAPON_ClusterMines ChinaJetCargoPlane + ClusterMinesBomb residual.
    pub fn spawn_cluster_mines_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        self.spawn_cluster_mines_flight_with_effects(
            source_id,
            target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_cluster_mines_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_cluster_mines_flight::{
            CLUSTER_MINES_BOMB_OBJECT, HostClusterMinesFlightData,
        };
        use crate::game_logic::host_mines::CLUSTER_MINES_OCL_TRANSPORT;
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
        if self.receiving_module_content.is_some() {
            // Materialize the receiving definitions before the factory's map skip
            // check; a bare fallback would discard authored gameplay modules.
            if !self.ensure_host_spawn_template(CLUSTER_MINES_OCL_TRANSPORT)
                || !self.ensure_host_spawn_template(CLUSTER_MINES_BOMB_OBJECT)
            {
                return None;
            }
        } else {
            if !self.templates.contains_key(CLUSTER_MINES_OCL_TRANSPORT) {
                let mut t = ThingTemplate::new(CLUSTER_MINES_OCL_TRANSPORT);
                t.set_health(600.0)
                    .add_kind_of(KindOf::Aircraft)
                    .add_kind_of(KindOf::Vehicle);
                self.templates
                    .insert(CLUSTER_MINES_OCL_TRANSPORT.to_string(), t);
            }
            if !self.templates.contains_key(CLUSTER_MINES_BOMB_OBJECT) {
                let mut t = ThingTemplate::new(CLUSTER_MINES_BOMB_OBJECT);
                t.set_health(50.0).add_kind_of(KindOf::Projectile);
                self.templates
                    .insert(CLUSTER_MINES_BOMB_OBJECT.to_string(), t);
            }
        }
        let tid = self.create_object_with_script_effects(
            CLUSTER_MINES_OCL_TRANSPORT,
            team,
            edge,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            let mut data = HostClusterMinesFlightData::start(edge, target);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.cluster_mines_transport = Some(data);
            o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
        }
        self.cluster_mines_flight_reg.record_transport();
        // C++ OCLSpecialPower::doSpecialPowerAtLocation → base createViewObject.
        // Retail SuperweaponClusterMines ViewObjectRange 250 / Duration 30000ms.
        let _ = self.create_special_power_view_object_at_with_effects(
            source_id,
            target,
            crate::game_logic::host_mines::CLUSTER_MINES_VIEW_OBJECT_RANGE,
            crate::game_logic::host_mines::CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES,
            presentation_effects,
        );
        let _ = self.create_delivery_radius_decal_with_effects(
            tid,
            target,
            crate::game_logic::host_mines::CLUSTER_MINES_DELIVERY_DECAL_RADIUS,
            presentation_effects,
        );
        Some(tid)
    }

    pub fn update_cluster_mines_flights(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_cluster_mines_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_cluster_mines_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_cluster_mines_flight::{
            CLUSTER_MINES_BOMB_OBJECT, cluster_mines_payload_drop_pos,
        };

        let tids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.cluster_mines_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(Team, Vec3, ObjectId, Vec3, Vec3)> = Vec::new();
        let mut leave = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.cluster_mines_transport.as_mut() else {
                continue;
            };
            if !data.map_extent_ok() {
                data.map_min = self.world_min;
                data.map_max = self.world_max;
            }
            let should_drop = !data.delivery_complete && data.in_delivery_band(pos);
            if should_drop {
                data.delivery_complete = true;
            }
            let (new_pos, vel, at_exit) = data.tick_transport(pos);
            let target = data.target;
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
            if should_drop {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                drops.push((team, target, producer, pos, vel));
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        for (team, target, producer, plane_pos, plane_vel) in drops {
            use crate::game_logic::host_mines::{
                apply_cluster_mines_drop_variance, cluster_mines_drop_unit_samples,
            };
            let seed = producer.0.wrapping_add(self.frame);
            let (ux, uy) = cluster_mines_drop_unit_samples(seed);
            let target = apply_cluster_mines_drop_variance(target, ux, uy);
            let drop_pos = cluster_mines_payload_drop_pos(plane_pos);
            if let Some(bid) = self.create_object_with_script_effects(
                CLUSTER_MINES_BOMB_OBJECT,
                team,
                drop_pos,
                None,
                presentation_effects,
            ) {
                if let Some(o) = self.objects.get_mut(&bid) {
                    o.producer_id = Some(producer);
                    o.cluster_mines_bomb = true;
                    o.movement.velocity = Vec3::new(plane_vel.x, -14.0, plane_vel.z);
                    let _ = o.set_smart_bomb_target(target);
                }
                self.cluster_mines_flight_reg.record_drop();
            }
        }

        let bombs: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.cluster_mines_bomb && o.is_alive())
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
                let impact = Vec3::new(pos.x, 0.0, pos.z);
                // Bomb already carries DropVariance; SmartBorder around impact only.
                let mines = self.place_cluster_mines_unvaried_with_effects(
                    team,
                    impact,
                    producer,
                    Some(id),
                    presentation_effects,
                );

                self.cluster_mines_flight_reg
                    .record_minefield(mines.len() as u32);
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        Ok(())
    }

    /// C++ SUPERWEAPON_EMPPulse ChinaJetCargoPlane + EMPPulseBomb residual.
    /// C++ OCL_EMPPulseEffectSpheroids CreateObject EMPPulseEffectSpheroid residual.
    pub fn spawn_emp_pulse_spheroid(
        &mut self,
        position: Vec3,
        producer: ObjectId,
    ) -> Option<ObjectId> {
        self.spawn_emp_pulse_spheroid_with_effects(
            position,
            producer,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_emp_pulse_spheroid_with_effects(
        &mut self,
        position: Vec3,
        producer: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_emp_pulse::{
            EMP_PULSE_EFFECT_SPHEROID, EMP_SPHEROID_LIFETIME_FRAMES,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&producer)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        if !self.templates.contains_key(EMP_PULSE_EFFECT_SPHEROID) {
            let mut t = ThingTemplate::new(EMP_PULSE_EFFECT_SPHEROID);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(EMP_PULSE_EFFECT_SPHEROID.to_string(), t);
        }
        let sid = self.create_object_with_script_effects(
            EMP_PULSE_EFFECT_SPHEROID,
            team,
            position,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&sid) {
            o.emp_pulse_spheroid = true;
            o.producer_id = Some(producer);
            o.emp_pulse_spheroid_expires_frame =
                Some(self.frame.saturating_add(EMP_SPHEROID_LIFETIME_FRAMES));
            o.visual_draw_state_revision = o.visual_draw_state_revision.wrapping_add(1);
        }
        self.emp_pulses.record_spheroid_spawn();
        self.emp_pulse_flight_reg.record_spheroid();
        Some(sid)
    }

    /// C++ ProjectileDetonationOCL CreateObject EMPPatriotEffectSpheroid.
    /// Shares EMPUpdate Lifetime expire with EMPPulseEffectSpheroid.
    pub fn spawn_emp_patriot_spheroid(
        &mut self,
        position: Vec3,
        producer: ObjectId,
    ) -> Option<ObjectId> {
        self.spawn_emp_patriot_spheroid_with_effects(
            position,
            producer,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_emp_patriot_spheroid_with_effects(
        &mut self,
        position: Vec3,
        producer: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_base_defense::EMP_PATRIOT_EFFECT_SPHEROID;
        use crate::game_logic::host_emp_pulse::EMP_SPHEROID_LIFETIME_FRAMES;
        use crate::game_logic::{KindOf, ThingTemplate};

        let team = self
            .objects
            .get(&producer)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        if !self.templates.contains_key(EMP_PATRIOT_EFFECT_SPHEROID) {
            let mut t = ThingTemplate::new(EMP_PATRIOT_EFFECT_SPHEROID);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(EMP_PATRIOT_EFFECT_SPHEROID.to_string(), t);
        }
        let sid = self.create_object_with_script_effects(
            EMP_PATRIOT_EFFECT_SPHEROID,
            team,
            position,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&sid) {
            o.emp_pulse_spheroid = true;
            o.producer_id = Some(producer);
            o.emp_pulse_spheroid_expires_frame =
                Some(self.frame.saturating_add(EMP_SPHEROID_LIFETIME_FRAMES));
            o.visual_draw_state_revision = o.visual_draw_state_revision.wrapping_add(1);
        }
        self.supw_patriot_emp_spheroids_spawned =
            self.supw_patriot_emp_spheroids_spawned.saturating_add(1);
        Some(sid)
    }

    /// C++ EMPUpdate::doDisableAttack EMPSparks on each disabled victim drawable.
    pub fn spawn_emp_sparks_on_victim(
        &mut self,
        victim_id: ObjectId,
        disabled_duration_frames: u32,
        particle_owner: &mut dyn crate::game_logic::combat_particles::ParticleBindingAccess,
    ) {
        use crate::game_logic::combat_particles::CombatParticleKind;
        use crate::game_logic::host_emp_pulse::{
            EMP_SPHEROID_DISABLE_FX, leftover_emp_spark_dome_clamp,
            leftover_emp_spark_emitter_count, leftover_emp_spark_initial_delay,
            leftover_emp_spark_lifetime, leftover_emp_spark_z,
        };
        use crate::game_logic::host_hero_abilities::leftover_disable_fx_footprint_area;

        let Some(victim) = self.objects.get(&victim_id) else {
            return;
        };
        let pos = victim.get_position();
        let yaw = victim.get_orientation();
        let geom = (*victim.geometry_info());
        let height = if geom.height > 0.0 {
            geom.height
        } else {
            victim.selection_radius.max(1.0)
        };
        let footprint = leftover_disable_fx_footprint_area(
            geom.authored,
            geom.geom_type as u32,
            geom.major_radius,
            geom.minor_radius,
            victim.selection_radius,
        );
        let count = leftover_emp_spark_emitter_count(footprint, height);
        let lifetime = leftover_emp_spark_lifetime(disabled_duration_frames);
        let frame = self.frame;
        for _ in 0..count {
            let mut offset =
                crate::game_logic::host_hero_abilities::leftover_disable_fx_footprint_offset(
                    &geom,
                    &self.logic_random,
                );
            offset.y = leftover_emp_spark_z(height, &self.logic_random);
            offset = leftover_emp_spark_dome_clamp(offset, height);
            let is_shrouded = self.host_fx_object_is_shrouded(victim_id.0);
            let Some(pid) = self
                .combat_particles
                .attach_named_to_object_local_with_visibility(
                    victim_id,
                    pos,
                    yaw,
                    offset,
                    frame,
                    EMP_SPHEROID_DISABLE_FX,
                    CombatParticleKind::DisableFx,
                    Some(lifetime),
                    is_shrouded,
                    particle_owner,
                )
            else {
                continue;
            };
            if let Some(client_id) = self
                .combat_particles
                .get(pid)
                .and_then(|e| e.client_system_id)
            {
                if let Some(manager) = particle_owner.particle_systems_mut() {
                    if let Some(system) = manager.find_particle_system_mut(client_id) {
                        system.set_initial_delay(leftover_emp_spark_initial_delay(
                            &self.logic_random,
                        ));
                    }
                }
            }
            self.hero_abilities
                .record_leftover_disable_fx_until(pid, frame.saturating_add(lifetime));
            self.supw_patriot_emp_sparks_spawned =
                self.supw_patriot_emp_sparks_spawned.saturating_add(1);
        }
    }

    pub fn update_emp_pulse_spheroids(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_emp_pulse_spheroids_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_emp_pulse_spheroids_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let frame = self.frame;
        self.apply_due_emp_pulse_disables_with_effects(presentation_effects);

        let due: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, o)| {
                if o.emp_pulse_spheroid {
                    if let Some(exp) = o.emp_pulse_spheroid_expires_frame {
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
                {
                    o.internal_change_health(-o.health.current);
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.emp_pulse_spheroid = false;
            }
            self.emp_pulses.remove_spheroid(id);
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        Ok(())
    }

    pub fn spawn_emp_pulse_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        player_id: u32,
    ) -> Option<ObjectId> {
        self.spawn_emp_pulse_flight_with_effects(
            source_id,
            target,
            player_id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_emp_pulse_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        player_id: u32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_emp_pulse::{EMP_PULSE_BOMB_TEMPLATE, EMP_PULSE_OCL_TRANSPORT};
        use crate::game_logic::host_emp_pulse_flight::HostEmpPulseFlightData;
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
        if !self.templates.contains_key(EMP_PULSE_OCL_TRANSPORT) {
            let mut t = ThingTemplate::new(EMP_PULSE_OCL_TRANSPORT);
            t.set_health(600.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates
                .insert(EMP_PULSE_OCL_TRANSPORT.to_string(), t);
        }
        if !self.templates.contains_key(EMP_PULSE_BOMB_TEMPLATE) {
            let mut t = ThingTemplate::new(EMP_PULSE_BOMB_TEMPLATE);
            t.set_health(50.0).add_kind_of(KindOf::Projectile);
            self.templates
                .insert(EMP_PULSE_BOMB_TEMPLATE.to_string(), t);
        }
        let tid = self.create_object_with_script_effects(
            EMP_PULSE_OCL_TRANSPORT,
            team,
            edge,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            let mut data = HostEmpPulseFlightData::start(edge, target, player_id, source_id.0);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.emp_pulse_transport = Some(data);
            o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
        }
        self.emp_pulse_flight_reg.record_transport();
        let _ = self.create_delivery_radius_decal_with_effects(
            tid,
            target,
            crate::game_logic::host_emp_pulse::EMP_PULSE_DELIVERY_DECAL_RADIUS,
            presentation_effects,
        );
        Some(tid)
    }

    pub fn update_emp_pulse_flights(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_emp_pulse_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_emp_pulse_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::host_emp_pulse::EMP_PULSE_BOMB_TEMPLATE;

        let tids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.emp_pulse_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(Team, Vec3, ObjectId, u32, u32)> = Vec::new();
        let mut leave = Vec::new();
        let world_min = self.world_min;
        let world_max = self.world_max;
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.emp_pulse_transport.as_mut() else {
                continue;
            };
            if !data.map_extent_ok() {
                data.map_min = world_min;
                data.map_max = world_max;
            }
            let should_drop = !data.delivery_complete && data.in_delivery_band(pos);
            if should_drop {
                data.delivery_complete = true;
            }
            let (new_pos, vel, at_exit) = data.tick_transport(pos);
            let target = data.target;
            let player_id = data.player_id;
            let caster = data.caster_id;
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
            if should_drop {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                drops.push((team, target, producer, player_id, caster));
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        for (_team, target, producer, player_id, caster) in drops {
            let drop_pos = Vec3::new(target.x, 80.0, target.z);
            if let Some(bid) = self.create_object_with_script_effects(
                EMP_PULSE_BOMB_TEMPLATE,
                _team,
                drop_pos,
                None,
                presentation_effects,
            ) {
                if let Some(o) = self.objects.get_mut(&bid) {
                    o.producer_id = Some(producer);
                    o.emp_pulse_bomb = true;
                    o.movement.velocity = Vec3::new(0.0, -14.0, 0.0);
                    let _ = o.set_smart_bomb_target(target);
                    let _ = (player_id, caster);
                }
                self.emp_pulse_flight_reg.record_drop();
            }
        }

        let bombs: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.emp_pulse_bomb && o.is_alive())
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
                let impact = Vec3::new(pos.x, 0.0, pos.z);
                let player_id = producer
                    .and_then(|pid| self.objects.get(&pid))
                    .and_then(|o| {
                        self.players
                            .iter()
                            .find(|(_, p)| p.team == o.team)
                            .map(|(id, _)| *id)
                    })
                    .unwrap_or(0);
                let _ = self.apply_emp_pulse_at_with_effects(
                    player_id,
                    impact,
                    producer,
                    presentation_effects,
                );
                self.emp_pulse_flight_reg.record_detonation();
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects)?;
        }
        self.apply_due_emp_pulse_disables_with_effects(presentation_effects);
        Ok(())
    }

    /// C++ SUPERWEAPON_Frenzy OCL Frenzy_InvisibleMarker residual.
    pub fn spawn_frenzy_invisible_marker(
        &mut self,
        team: Team,
        position: Vec3,
        level: crate::game_logic::host_frenzy::HostFrenzyLevel,
    ) -> Option<ObjectId> {
        self.spawn_frenzy_invisible_marker_with_effects(
            team,
            position,
            level,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_frenzy_invisible_marker_with_effects(
        &mut self,
        team: Team,
        position: Vec3,
        level: crate::game_logic::host_frenzy::HostFrenzyLevel,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::{KindOf, ThingTemplate};

        let tmpl = level.marker_template();
        if !self.templates.contains_key(tmpl) {
            let mut t = ThingTemplate::new(tmpl);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates.insert(tmpl.to_string(), t);
        }
        let mid = self.create_object_with_script_effects(
            tmpl,
            team,
            position,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&mid) {
            o.frenzy_invisible_marker = true;
        }
        self.frenzies.record_marker_spawn(mid);
        Some(mid)
    }

    pub fn update_frenzy_invisible_markers(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_frenzy_invisible_markers_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_frenzy_invisible_markers_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // Retail DeletionUpdate Min/MaxLifetime = 1ms → 1 frame residual.
        let due = self.frenzies.take_due_marker_deletes();
        for id in due {
            if self
                .objects
                .get(&id)
                .map(|o| o.frenzy_invisible_marker)
                .unwrap_or(false)
            {
                // C++ DeletionUpdate: Destroy (NOT kill). Keep body HP and
                // run the owning instance's direct deletion operation.
                self.destroy_object_with_effects(id, presentation_effects)?;
            }
        }
        Ok(())
    }
}
