//! Host combat `impl GameLogic` — `special_power_flights`.
//! Child of `world_combat` (itself a child of `game_logic.rs`).
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    pub fn update_anthrax_bomb_flights(&mut self) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_anthrax_bomb_flight::anthrax_payload_drop_pos;
        use crate::game_logic::special_power_strikes::{
            ANTHRAX_BOMB_IMPACT_DAMAGE, ANTHRAX_BOMB_IMPACT_RADIUS,
        };

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
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
            o.set_position(new_pos);
            o.movement.velocity = vel;
            o.movement.target_position = None;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
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
            self.mark_object_for_destruction(id, None);
        }
        for (team, target, producer, tier, plane_pos, plane_vel) in drops {
            let bomb = tier.bomb();
            let drop_pos = anthrax_payload_drop_pos(plane_pos);
            if let Some(bid) = self.create_object(bomb, team, drop_pos) {
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
            .iter()
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
                o.set_position(p);
                (p, o.producer_id, o.team)
            };
            if pos.y <= 5.0 {
                let impact = Vec3::new(pos.x, 0.0, pos.z);
                let _ = self.apply_fuel_air_radius_damage(
                    id,
                    producer,
                    team,
                    impact,
                    ANTHRAX_BOMB_IMPACT_DAMAGE,
                    ANTHRAX_BOMB_IMPACT_RADIUS,
                    DamageType::Explosive,
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
            self.mark_object_for_destruction(id, None);
        }
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
        let sid = self.create_object_for_owner_or_team(
            SNEAK_ATTACK_TUNNEL_START_TEMPLATE,
            team,
            owner_player_id,
            position,
        )?;
        if let Some(o) = self.objects.get_mut(&sid) {
            o.sneak_tunnel_start = true;
            o.set_orientation(placement_angle);
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
        let tid = self.create_object(CLUSTER_MINES_OCL_TRANSPORT, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            let mut data = HostClusterMinesFlightData::start(edge, target);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.cluster_mines_transport = Some(data);
            o.set_orientation(dz.atan2(dx));
        }
        self.cluster_mines_flight_reg.record_transport();
        // C++ OCLSpecialPower::doSpecialPowerAtLocation → base createViewObject.
        // Retail SuperweaponClusterMines ViewObjectRange 250 / Duration 30000ms.
        let _ = self.create_special_power_view_object_at(
            source_id,
            target,
            crate::game_logic::host_mines::CLUSTER_MINES_VIEW_OBJECT_RANGE,
            crate::game_logic::host_mines::CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES,
        );
        let _ = self.create_delivery_radius_decal_with_radius(
            tid,
            target,
            crate::game_logic::host_mines::CLUSTER_MINES_DELIVERY_DECAL_RADIUS,
        );
        Some(tid)
    }

    pub fn update_cluster_mines_flights(&mut self) {
        use crate::game_logic::host_cluster_mines_flight::{
            CLUSTER_MINES_BOMB_OBJECT, cluster_mines_payload_drop_pos,
        };

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
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
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
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
            self.mark_object_for_destruction(id, None);
        }
        for (team, target, producer, plane_pos, plane_vel) in drops {
            use crate::game_logic::host_mines::{
                apply_cluster_mines_drop_variance, cluster_mines_drop_unit_samples,
            };
            let seed = producer.0.wrapping_add(self.frame);
            let (ux, uy) = cluster_mines_drop_unit_samples(seed);
            let target = apply_cluster_mines_drop_variance(target, ux, uy);
            let drop_pos = cluster_mines_payload_drop_pos(plane_pos);
            if let Some(bid) = self.create_object(CLUSTER_MINES_BOMB_OBJECT, team, drop_pos) {
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
            .iter()
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
                o.set_position(p);
                (p, o.producer_id, o.team)
            };
            if pos.y <= 5.0 {
                let impact = Vec3::new(pos.x, 0.0, pos.z);
                // Bomb already carries DropVariance; SmartBorder around impact only.
                let mines = self.place_cluster_mines_unvaried(team, impact, producer);

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
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ SUPERWEAPON_EMPPulse ChinaJetCargoPlane + EMPPulseBomb residual.
    /// C++ OCL_EMPPulseEffectSpheroids CreateObject EMPPulseEffectSpheroid residual.
    pub fn spawn_emp_pulse_spheroid(
        &mut self,
        position: Vec3,
        producer: ObjectId,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_emp_pulse::{
            EMP_PULSE_EFFECT_SPHEROID, EMP_SPHEROID_GEOMETRY_RADIUS, EMP_SPHEROID_LIFETIME_FRAMES,
            EMP_SPHEROID_START_SCALE,
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
        let sid = self.create_object(EMP_PULSE_EFFECT_SPHEROID, team, position)?;
        if let Some(o) = self.objects.get_mut(&sid) {
            o.emp_pulse_spheroid = true;
            o.producer_id = Some(producer);
            o.emp_pulse_spheroid_expires_frame =
                Some(self.frame.saturating_add(EMP_SPHEROID_LIFETIME_FRAMES));
            o.thing.geometry.radius = EMP_SPHEROID_GEOMETRY_RADIUS * EMP_SPHEROID_START_SCALE;
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
        use crate::game_logic::host_base_defense::EMP_PATRIOT_EFFECT_SPHEROID;
        use crate::game_logic::host_emp_pulse::{
            EMP_SPHEROID_GEOMETRY_RADIUS, EMP_SPHEROID_LIFETIME_FRAMES, EMP_SPHEROID_START_SCALE,
        };
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
        let sid = self.create_object(EMP_PATRIOT_EFFECT_SPHEROID, team, position)?;
        if let Some(o) = self.objects.get_mut(&sid) {
            o.emp_pulse_spheroid = true;
            o.producer_id = Some(producer);
            o.emp_pulse_spheroid_expires_frame =
                Some(self.frame.saturating_add(EMP_SPHEROID_LIFETIME_FRAMES));
            o.thing.geometry.radius = EMP_SPHEROID_GEOMETRY_RADIUS * EMP_SPHEROID_START_SCALE;
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
        let geom = victim.thing.template.geometry_info;
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
                crate::game_logic::host_hero_abilities::leftover_disable_fx_footprint_offset(&geom);
            offset.y = leftover_emp_spark_z(height);
            offset = leftover_emp_spark_dome_clamp(offset, height);
            let Some(pid) = self.combat_particles.attach_named_to_object_local(
                victim_id,
                pos,
                yaw,
                offset,
                frame,
                EMP_SPHEROID_DISABLE_FX,
                CombatParticleKind::DisableFx,
                Some(lifetime),
            ) else {
                continue;
            };
            if let Some(client_id) = self
                .combat_particles
                .get(pid)
                .and_then(|e| e.client_system_id)
            {
                if let Some(mgr) = gamelogic::helpers::TheParticleSystemManager::get() {
                    mgr.set_initial_delay(client_id, leftover_emp_spark_initial_delay());
                }
            }
            self.hero_abilities
                .record_leftover_disable_fx_until(pid, frame.saturating_add(lifetime));
            self.supw_patriot_emp_sparks_spawned =
                self.supw_patriot_emp_sparks_spawned.saturating_add(1);
        }
    }

    pub fn update_emp_pulse_spheroids(&mut self) {
        let frame = self.frame;
        self.apply_due_emp_pulse_disables();

        let due: Vec<ObjectId> = self
            .objects
            .iter()
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
                if crate::gameworld_shadow::gameworld_damage_authority_live() {
                    let hp = o.health.current.max(1.0);
                    let oid = o.id;
                    crate::game_logic::host_damage_log::record(oid, hp, None, true);
                } else {
                    o.health.current = 0.0;
                }
                o.status.destroyed = true;
                o.status.effectively_dead = true;
                o.emp_pulse_spheroid = false;
            }
            self.emp_pulses.remove_spheroid(id);
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn spawn_emp_pulse_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        player_id: u32,
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
        let tid = self.create_object(EMP_PULSE_OCL_TRANSPORT, team, edge)?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            let mut data = HostEmpPulseFlightData::start(edge, target, player_id, source_id.0);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.emp_pulse_transport = Some(data);
            o.set_orientation(dz.atan2(dx));
        }
        self.emp_pulse_flight_reg.record_transport();
        let _ = self.create_delivery_radius_decal_with_radius(
            tid,
            target,
            crate::game_logic::host_emp_pulse::EMP_PULSE_DELIVERY_DECAL_RADIUS,
        );
        Some(tid)
    }

    pub fn update_emp_pulse_flights(&mut self) {
        use crate::game_logic::host_emp_pulse::EMP_PULSE_BOMB_TEMPLATE;

        let tids: Vec<ObjectId> = self
            .objects
            .iter()
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
            o.set_position(new_pos);
            o.movement.velocity = vel;
            if vel.length_squared() > 1e-6 {
                o.set_orientation(vel.z.atan2(vel.x));
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
            self.mark_object_for_destruction(id, None);
        }
        for (_team, target, producer, player_id, caster) in drops {
            let drop_pos = Vec3::new(target.x, 80.0, target.z);
            if let Some(bid) = self.create_object(EMP_PULSE_BOMB_TEMPLATE, _team, drop_pos) {
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
            .iter()
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
                o.set_position(p);
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
                let _ = self.apply_emp_pulse_at(player_id, impact, producer);
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
            self.mark_object_for_destruction(id, None);
        }
        self.apply_due_emp_pulse_disables();
    }

    /// C++ SUPERWEAPON_Frenzy OCL Frenzy_InvisibleMarker residual.
    pub fn spawn_frenzy_invisible_marker(
        &mut self,
        team: Team,
        position: Vec3,
        level: crate::game_logic::host_frenzy::HostFrenzyLevel,
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
        let mid = self.create_object(tmpl, team, position)?;
        if let Some(o) = self.objects.get_mut(&mid) {
            o.frenzy_invisible_marker = true;
        }
        self.frenzies.record_marker_spawn(mid);
        Some(mid)
    }

    pub fn update_frenzy_invisible_markers(&mut self) {
        // Retail DeletionUpdate Min/MaxLifetime = 1ms → 1 frame residual.
        let due = self.frenzies.take_due_marker_deletes();
        for id in due {
            if self
                .objects
                .get(&id)
                .map(|o| o.frenzy_invisible_marker)
                .unwrap_or(false)
            {
                // Invisible marker has no SlowDeath residual — hard-remove.
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
                }
                self.mark_object_for_destruction(id, None);
            }
        }
    }

    /// C++ OCL GPSScrambler_InvisibleMarker residual.
    pub fn spawn_gps_scrambler_marker(&mut self, team: Team, position: Vec3) -> Option<ObjectId> {
        use crate::game_logic::host_gps_scrambler::GPS_SCRAMBLER_INVISIBLE_MARKER;
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(GPS_SCRAMBLER_INVISIBLE_MARKER) {
            let mut t = ThingTemplate::new(GPS_SCRAMBLER_INVISIBLE_MARKER);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(GPS_SCRAMBLER_INVISIBLE_MARKER.to_string(), t);
        }
        let mid = self.create_object(GPS_SCRAMBLER_INVISIBLE_MARKER, team, position)?;
        if let Some(o) = self.objects.get_mut(&mid) {
            o.gps_scrambler_marker = true;
        }
        self.gps_scramblers.record_marker_spawn();
        Some(mid)
    }

    /// C++ GrantStealthBehavior.cpp:147-150 `TheGameLogic->destroyObject(self)`
    /// after the FinalRadius scan. Invisible marker has no SlowDeath — hard-remove
    /// like Frenzy DeletionUpdate teardown.
    fn destroy_gps_scrambler_marker(&mut self, id: ObjectId) {
        if !self
            .objects
            .get(&id)
            .map(|o| o.gps_scrambler_marker)
            .unwrap_or(false)
        {
            return;
        }
        if let Some(o) = self.objects.get_mut(&id) {
            if crate::gameworld_shadow::gameworld_damage_authority_live() {
                let hp = o.health.current.max(1.0);
                let oid = o.id;
                crate::game_logic::host_damage_log::record(oid, hp, None, true);
            } else {
                o.health.current = 0.0;
            }
            o.status.destroyed = true;
            o.status.effectively_dead = true;
        }
        self.mark_object_for_destruction(id, None);
    }

    /// C++ GrantStealthBehavior radius grow pulse residual (Start 20 → Final 100).
    pub fn update_gps_scrambler_grow(&mut self) {
        use crate::game_logic::host_gps_scrambler::{
            GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL, gps_scrambler_grow_is_final,
            gps_scrambler_scan_radius_after_updates, in_gps_scrambler_radius_2d,
            is_gps_scrambler_disguise_name, is_legal_gps_scrambler_target,
        };

        // Collect grow work without holding registry mut across object mut.
        let mut markers_to_destroy: Vec<ObjectId> = Vec::new();
        let work: Vec<(u32, Vec3, f32, Team, Option<ObjectId>, u32)> = {
            let mut out = Vec::new();
            for a in self.gps_scramblers.growing_missions_mut() {
                if a.grow_index >= GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL {
                    a.growing = false;
                    a.radius =
                        gps_scrambler_scan_radius_after_updates(a.grow_index.saturating_sub(1));
                    if let Some(mid) = a.marker_id.take() {
                        markers_to_destroy.push(mid);
                    }
                    continue;
                }
                let radius = gps_scrambler_scan_radius_after_updates(a.grow_index);
                a.radius = radius;
                a.grow_index = a.grow_index.saturating_add(1);
                let this_is_final = gps_scrambler_grow_is_final(a.grow_index.saturating_sub(1))
                    || a.grow_index >= GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL;
                if this_is_final {
                    a.growing = false;
                    // C++ GrantStealthBehavior.cpp:147-150 after the FinalRadius scan.
                    if let Some(mid) = a.marker_id.take() {
                        markers_to_destroy.push(mid);
                    }
                }
                let team = a
                    .caster_id
                    .and_then(|cid| self.objects.get(&cid).map(|o| o.team))
                    .unwrap_or(Team::GLA);
                out.push((a.id, a.location, radius, team, a.caster_id, a.player_id));
            }
            out
        };

        for (_aid, location, radius, team, caster_id, player_id) in work {
            self.gps_scramblers.record_grow_pulse();
            let center = (location.x, location.z);
            let candidates: Vec<(ObjectId, bool, bool, bool, bool, bool, bool)> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() {
                        return None;
                    }
                    if obj.gps_scrambler_marker {
                        return None;
                    }
                    let pos = obj.get_position();
                    if !in_gps_scrambler_radius_2d(center, (pos.x, pos.z), radius) {
                        return None;
                    }
                    let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                    let is_infantry = obj.is_kind_of(KindOf::Infantry);
                    let is_ally = self.gps_grant_is_ally(player_id, caster_id, team, obj);
                    let under_construction =
                        obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                    let is_disguise = is_gps_scrambler_disguise_name(&obj.template_name);
                    let has_stealth_module = obj.has_gps_stealth_module();
                    Some((
                        *id,
                        is_vehicle,
                        is_infantry,
                        is_ally,
                        under_construction,
                        is_disguise,
                        has_stealth_module,
                    ))
                })
                .collect();

            let mut grants = 0u32;
            for (
                id,
                is_vehicle,
                is_infantry,
                is_ally,
                under_construction,
                is_disguise,
                has_stealth_module,
            ) in candidates
            {
                if !is_legal_gps_scrambler_target(
                    is_vehicle,
                    is_infantry,
                    true,
                    is_ally,
                    under_construction,
                    is_disguise,
                    has_stealth_module,
                ) {
                    continue;
                }
                let Some(target) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !target.is_alive() {
                    continue;
                }
                let was = target.is_effectively_stealthed();
                target.apply_grant_stealth();
                // C++ grantStealthToObject: receiveGrant() then draw->flashAsSelected().
                target.flash_as_selected();
                if !was || target.is_effectively_stealthed() {
                    grants = grants.saturating_add(1);
                }
            }
            if grants > 0 {
                // Bookkeeping on registry grant_count via record if available.
                self.gps_scramblers.grant_count =
                    self.gps_scramblers.grant_count.saturating_add(grants);
            }
            let _ = caster_id;
        }

        // C++ GrantStealthBehavior.cpp:147-150 destroyObject after the final scan.
        for mid in markers_to_destroy {
            self.destroy_gps_scrambler_marker(mid);
        }
        let owned: std::collections::HashSet<ObjectId> = self
            .gps_scramblers
            .growing_missions_mut()
            .filter_map(|a| a.marker_id)
            .collect();
        let orphans: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                (obj.gps_scrambler_marker && obj.is_alive() && !owned.contains(id)).then_some(*id)
            })
            .collect();
        for mid in orphans {
            self.destroy_gps_scrambler_marker(mid);
        }
    }

    /// C++ OCL RepairVehiclesInArea_InvisibleMarker residual (DeletionUpdate 0 = same-frame die).
    pub fn spawn_emergency_repair_marker(
        &mut self,
        team: Team,
        position: Vec3,
        level: crate::game_logic::host_emergency_repair::HostEmergencyRepairLevel,
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
        let mid = self.create_object(tmpl, team, position)?;
        if let Some(o) = self.objects.get_mut(&mid) {
            o.emergency_repair_marker = true;
            // DeletionUpdate Lifetime 0 residual: die immediately after pulse.
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
        }
        self.emergency_repairs.record_marker_spawn();
        // Avoid full destroy pipeline on the one-pulse marker residual.
        Some(mid)
    }

    /// C++ AmericaVehicleSpyDrone DynamicShroudClearingRangeUpdate grow residual.
    /// Expands the spawned scout's shroud-clearing range 0→250; look follows the unit.
    pub fn update_spy_drone_grow(&mut self) {
        use crate::game_logic::host_spy_drone::{
            SPY_DRONE_GROW_UPDATES_TO_FINAL, SPY_DRONE_VISION_RANGE, spy_drone_grow_is_final,
            spy_drone_scan_radius_after_updates,
        };

        let work: Vec<(usize, Option<crate::game_logic::ObjectId>, f32)> = {
            let acts = self.spy_drones.activations_mut();
            let mut out = Vec::new();
            for (i, a) in acts.iter_mut().enumerate() {
                if !a.growing {
                    continue;
                }
                if a.grow_index >= SPY_DRONE_GROW_UPDATES_TO_FINAL {
                    a.growing = false;
                    a.radius = SPY_DRONE_VISION_RANGE;
                    continue;
                }
                let radius = spy_drone_scan_radius_after_updates(a.grow_index);
                a.radius = radius;
                a.grow_index = a.grow_index.saturating_add(1);
                if spy_drone_grow_is_final(a.grow_index.saturating_sub(1)) {
                    a.growing = false;
                    a.radius = SPY_DRONE_VISION_RANGE;
                }
                out.push((i, a.spawned_id, a.radius));
            }
            out
        };

        if work.is_empty() {
            return;
        }

        for (i, spawned_id, radius) in work {
            if let Some(id) = spawned_id {
                if let Some(obj) = self.objects.get_mut(&id) {
                    if obj.is_alive() {
                        obj.shroud_clearing_range = radius;
                        obj.vision_range = SPY_DRONE_VISION_RANGE;
                        let pos = obj.get_position();
                        if let Some(act) = self.spy_drones.activations_mut().get_mut(i) {
                            act.location = pos;
                            act.radius = radius;
                        }
                    }
                }
            }
            self.spy_drones.record_grow_pulse();
        }
    }

    /// C++ OCL_FireWallSegment CreateObject FireWallSegment residual.
    pub fn spawn_firewall_segment_objects(
        &mut self,
        wall_id: u32,
        source_object: ObjectId,
        source_team: Team,
    ) -> u32 {
        use crate::game_logic::host_firewall::{
            FIREWALL_DURATION_FRAMES, FIREWALL_SEGMENT_MAX_HEALTH, HostFireWallRegistry,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let (positions, upgraded): (Vec<Vec3>, bool) = self
            .fire_walls
            .active_walls()
            .iter()
            .find(|w| w.id == wall_id)
            .map(|w| (w.segments.iter().map(|s| s.position).collect(), w.upgraded))
            .unwrap_or_default();
        if positions.is_empty() {
            return 0;
        }
        let template = HostFireWallRegistry::wall_segment_template(upgraded);
        if !self.templates.contains_key(template) {
            let mut t = ThingTemplate::new(template);
            t.add_kind_of(KindOf::Immobile)
                .set_health(FIREWALL_SEGMENT_MAX_HEALTH)
                .set_cost(0, 0);
            self.templates.insert(template.to_string(), t);
        }
        let (dir_x, dir_z) = self
            .fire_walls
            .active_walls()
            .iter()
            .find(|w| w.id == wall_id)
            .map(|w| (w.dir_x, w.dir_z))
            .unwrap_or((1.0, 0.0));
        let expires = self.frame.saturating_add(FIREWALL_DURATION_FRAMES);
        let mut spawned = 0u32;
        for pos in positions {
            if let Some(sid) = self.create_object(template, source_team, pos) {
                if let Some(o) = self.objects.get_mut(&sid) {
                    o.firewall_segment = true;
                    o.note_producer(source_object);
                    o.firewall_segment_expires_frame = Some(expires);
                    o.firewall_segment_wall_id = Some(wall_id);
                    o.firewall_segment_dir = Some([dir_x, dir_z]);
                    o.health.maximum = FIREWALL_SEGMENT_MAX_HEALTH;
                    Self::write_object_health_authority_aware(o, FIREWALL_SEGMENT_MAX_HEALTH);
                }
                spawned = spawned.saturating_add(1);
            }
        }
        self.fire_walls.record_segment_spawns(spawned);
        spawned
    }

    pub fn update_firewall_segment_objects(&mut self) {
        use crate::game_logic::host_firewall::FIREWALL_INCH_PER_FRAME;

        let frame = self.frame;

        // C++ InchForwardLocomotor residual: segments crawl along wall direction.
        // Keep registry damage zones and live segment objects in lockstep.
        self.fire_walls.crawl_segments();
        // Wave 809: under coupled shadow, object crawl/expire owned by GW.
        if crate::gameworld_shadow::gameworld_shadow_enabled()
            && crate::gameworld_shadow::shadow_coupled_tick_active()
        {
            return;
        }
        let crawlers: Vec<(ObjectId, f32, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if !o.firewall_segment || !o.is_alive() {
                    return None;
                }
                let dir = o.firewall_segment_dir.unwrap_or([1.0, 0.0]);
                Some((
                    *id,
                    dir[0] * FIREWALL_INCH_PER_FRAME,
                    dir[1] * FIREWALL_INCH_PER_FRAME,
                ))
            })
            .collect();
        for (id, dx, dz) in crawlers {
            if let Some(o) = self.objects.get_mut(&id) {
                let mut p = o.get_position();
                p.x += dx;
                p.z += dz;
                o.set_position(p);
            }
        }

        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.firewall_segment {
                    if let Some(exp) = o.firewall_segment_expires_frame {
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
                o.firewall_segment = false;
                o.firewall_segment_wall_id = None;
                o.firewall_segment_dir = None;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ OCL SUPERWEAPON_RadarVanScan CreateObject RadarVanPing residual.
    pub fn spawn_radar_van_ping(
        &mut self,
        team: Team,
        position: Vec3,
        caster_id: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_radar_scan::{
            RADAR_SCAN_DURATION_FRAMES, RADAR_SCAN_STEALTH_DETECTION_RANGE,
            RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES, RADAR_VAN_PING_TEMPLATE,
        };
        use crate::game_logic::host_strategy_center::stealth_detector_hold_frames;
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(RADAR_VAN_PING_TEMPLATE) {
            let mut t = ThingTemplate::new(RADAR_VAN_PING_TEMPLATE);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(RADAR_VAN_PING_TEMPLATE.to_string(), t);
        }
        let pid = self.create_object(RADAR_VAN_PING_TEMPLATE, team, position)?;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.radar_van_ping = true;
            o.producer_id = caster_id;
            // C++ StealthDetectorUpdate.cpp:167-282 on RadarVanPing
            // (DetectionRate 500ms, DetectionRange 0 → VisionRange 150).
            o.set_detector_state(
                true,
                RADAR_SCAN_STEALTH_DETECTION_RANGE,
                RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES,
            );
            o.radar_van_ping_expires_frame =
                Some(self.frame.saturating_add(RADAR_SCAN_DURATION_FRAMES));
        }
        let (extra_required, extra_forbidden) = self
            .objects
            .get(&pid)
            .map(|o| {
                crate::game_logic::host_radar_stealth_vision_residual::extra_detect_kindof_for_detector(
                    &o.template_name,
                    o.extra_detect_kindof,
                    o.extra_detect_kindof_not,
                )
            })
            .unwrap_or((0, 0));
        // First DetectionRate scan is immediate (C++ UPDATE_SLEEP_NONE).
        let hold = stealth_detector_hold_frames(RADAR_SCAN_STEALTH_DETECTION_RATE_FRAMES);
        let expires = self.frame.saturating_add(hold);
        let range_sq = RADAR_SCAN_STEALTH_DETECTION_RANGE * RADAR_SCAN_STEALTH_DETECTION_RANGE;
        let stealthed: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(id, o)| {
                *id != &pid
                    && o.is_alive()
                    && o.team != team
                    && (o.status.stealthed || o.status.disguised)
                    && crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                        o.kind_of_cpp_mask(),
                        extra_required,
                        extra_forbidden,
                    )
            })
            .filter(|(_, o)| {
                let p = o.get_position();
                let dx = p.x - position.x;
                let dz = p.z - position.z;
                dx * dx + dz * dz <= range_sq
            })
            .map(|(id, _)| *id)
            .collect();
        let mut garrison_detect_recalc: Vec<ObjectId> = Vec::new();
        for sid in stealthed {
            let (contained_by, already_detected) = self
                .objects
                .get(&sid)
                .map(|o| (o.contained_by, o.status.detected))
                .unwrap_or((None, false));
            if let Some(o) = self.objects.get_mut(&sid) {
                o.mark_detected(expires);
            }
            self.order_idle_enemies_to_attack_on_reveal(sid);
            // C++ StealthDetectorUpdate.cpp:198-282 first reveal (UPDATE_SLEEP_NONE).
            if !already_detected {
                self.fire_stealth_discover_feedback(sid, &[pid]);
            }
            if let Some(cid) = contained_by {
                garrison_detect_recalc.push(cid);
            }
        }
        self.recalc_garrisons_after_occupant_detect_change(&garrison_detect_recalc);
        self.radar_scans.record_ping_spawn();
        Some(pid)
    }

    pub fn update_radar_van_pings(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.radar_van_ping {
                    if let Some(exp) = o.radar_van_ping_expires_frame {
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
                o.radar_van_ping = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    /// C++ OCL SUPERWEAPON_SpySatellite CreateObject SpySatellitePing residual.
    /// SpySatellitePing carries StealthDetectorUpdate (DetectionRate 500ms,
    /// DetectionRange 0 → VisionRange 300) so stealthed units in the scan
    /// are DETECTED while the ping lives.
    pub fn spawn_spy_satellite_ping(
        &mut self,
        team: Team,
        position: Vec3,
        caster_id: Option<ObjectId>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_spy_satellite::{
            SPY_SATELLITE_DURATION_FRAMES, SPY_SATELLITE_PING_TEMPLATE,
            SPY_SATELLITE_STEALTH_DETECTION_RANGE, SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES,
        };
        use crate::game_logic::host_strategy_center::stealth_detector_hold_frames;
        use crate::game_logic::{KindOf, ThingTemplate};

        if !self.templates.contains_key(SPY_SATELLITE_PING_TEMPLATE) {
            let mut t = ThingTemplate::new(SPY_SATELLITE_PING_TEMPLATE);
            t.add_kind_of(KindOf::Immobile)
                .set_health(1.0)
                .set_cost(0, 0);
            self.templates
                .insert(SPY_SATELLITE_PING_TEMPLATE.to_string(), t);
        }
        let owner_player_id = match caster_id.and_then(|caster| self.objects.get(&caster)) {
            Some(caster) if caster.owner_player_id.is_some() => {
                Some(self.player_owner_for_host_object(caster)?)
            }
            Some(_) => None,
            None => None,
        };
        let pid = self.create_object_for_owner_or_team(
            SPY_SATELLITE_PING_TEMPLATE,
            team,
            owner_player_id,
            position,
        )?;
        if let Some(o) = self.objects.get_mut(&pid) {
            o.spy_satellite_ping = true;
            o.producer_id = caster_id;
            o.set_detector_state(
                true,
                SPY_SATELLITE_STEALTH_DETECTION_RANGE,
                SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES,
            );
            // DeletionUpdate lifetime residual tracked via destroy at duration.
            o.spy_satellite_ping_expires_frame =
                Some(self.frame.saturating_add(SPY_SATELLITE_DURATION_FRAMES));
        }
        let (extra_required, extra_forbidden) = self
            .objects
            .get(&pid)
            .map(|o| {
                crate::game_logic::host_radar_stealth_vision_residual::extra_detect_kindof_for_detector(
                    &o.template_name,
                    o.extra_detect_kindof,
                    o.extra_detect_kindof_not,
                )
            })
            .unwrap_or((0, 0));
        // First DetectionRate scan is immediate (C++ UPDATE_SLEEP_NONE).
        let hold = stealth_detector_hold_frames(SPY_SATELLITE_STEALTH_DETECTION_RATE_FRAMES);
        let expires = self.frame.saturating_add(hold);
        let range_sq =
            SPY_SATELLITE_STEALTH_DETECTION_RANGE * SPY_SATELLITE_STEALTH_DETECTION_RANGE;
        let stealthed: Vec<ObjectId> = self
            .objects
            .iter()
            .filter(|(id, o)| {
                *id != &pid
                    && o.is_alive()
                    && o.team != team
                    && (o.status.stealthed || o.status.disguised)
                    && crate::game_logic::host_radar_stealth_vision_residual::detector_accepts_kindof_residual(
                        o.kind_of_cpp_mask(),
                        extra_required,
                        extra_forbidden,
                    )
            })
            .filter(|(_, o)| {
                let p = o.get_position();
                let dx = p.x - position.x;
                let dz = p.z - position.z;
                dx * dx + dz * dz <= range_sq
            })
            .map(|(id, _)| *id)
            .collect();
        let mut garrison_detect_recalc: Vec<ObjectId> = Vec::new();
        for sid in stealthed {
            let contained_by = self.objects.get(&sid).and_then(|o| o.contained_by);
            if let Some(obj) = self.objects.get_mut(&sid) {
                obj.mark_detected(expires);
            }
            self.order_idle_enemies_to_attack_on_reveal(sid);
            if let Some(cid) = contained_by {
                garrison_detect_recalc.push(cid);
            }
        }
        self.recalc_garrisons_after_occupant_detect_change(&garrison_detect_recalc);
        self.spy_satellites.record_ping_spawn();
        Some(pid)
    }

    pub fn update_spy_satellite_pings(&mut self) {
        let frame = self.frame;
        let due: Vec<ObjectId> = self
            .objects
            .iter()
            .filter_map(|(id, o)| {
                if o.spy_satellite_ping {
                    if let Some(exp) = o.spy_satellite_ping_expires_frame {
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
                o.spy_satellite_ping = false;
            }
            self.mark_object_for_destruction(id, None);
        }
    }

    pub fn apply_ocl_random_force(&mut self, object_id: ObjectId) -> bool {
        use crate::game_logic::host_ocl_apply_random_force::{
            apply_random_force_plan_for, calc_random_force, spin_nudge_rad,
        };
        let Some(o) = self.objects.get_mut(&object_id) else {
            return false;
        };
        let plan = match apply_random_force_plan_for(&o.template_name) {
            Some(p) => p,
            None => return false,
        };
        let salt = object_id.0.wrapping_add(self.frame);
        let force = calc_random_force(&plan, salt);
        o.movement.velocity += force * 0.05;
        let spin = spin_nudge_rad(&plan, salt);
        o.set_orientation(o.get_orientation() + spin);
        self.ocl_apply_random_force_reg.record(force);
        true
    }

    pub fn honesty_carpet_bomb_flight_ok(&self) -> bool {
        crate::game_logic::host_carpet_bomb_flight::honesty_carpet_bomb_flight_residual_ok()
    }

    pub fn honesty_artillery_barrage_flight_ok(&self) -> bool {
        crate::game_logic::host_artillery_barrage_flight::honesty_artillery_barrage_flight_residual_ok()
    }

    pub fn honesty_a10_strike_flight_ok(&self) -> bool {
        crate::game_logic::host_a10_strike_flight::honesty_a10_strike_flight_residual_ok()
    }

    pub fn honesty_daisy_cutter_flight_ok(&self) -> bool {
        crate::game_logic::host_daisy_cutter_flight::honesty_daisy_cutter_flight_residual_ok()
    }

    pub fn honesty_anthrax_bomb_flight_ok(&self) -> bool {
        crate::game_logic::host_anthrax_bomb_flight::honesty_anthrax_bomb_flight_residual_ok()
    }

    pub fn honesty_cluster_mines_flight_ok(&self) -> bool {
        crate::game_logic::host_cluster_mines_flight::honesty_cluster_mines_flight_residual_ok()
    }

    pub fn honesty_emp_pulse_flight_ok(&self) -> bool {
        crate::game_logic::host_emp_pulse_flight::honesty_emp_pulse_flight_residual_ok()
    }

    pub fn honesty_scud_storm_missile_flight_ok(&self) -> bool {
        crate::game_logic::host_scud_storm_missile_flight::honesty_scud_storm_missile_flight_residual_ok()
    }

    /// Residual honesty: Neutron shell ScatterRadiusVsInfantry applied at least once.
    pub fn honesty_neutron_shell_scatter_ok(&self) -> bool {
        self.neutron_shell_scatter_applied > 0 || self.neutron_shell_scatter_misses > 0
    }

    pub fn honesty_neutron_missile_update_ok(&self) -> bool {
        crate::game_logic::host_neutron_missile_update::honesty_neutron_missile_update_residual_ok()
    }

    pub fn honesty_ocl_apply_random_force_ok(&self) -> bool {
        crate::game_logic::host_ocl_apply_random_force::honesty_ocl_apply_random_force_residual_ok()
    }

    pub fn honesty_fuel_air_gas_slow_death_ok(&self) -> bool {
        crate::game_logic::host_fuel_air_gas_slow_death::honesty_fuel_air_gas_slow_death_residual_ok(
        )
    }

    pub fn honesty_ocl_fire_weapon_attack_ok(&self) -> bool {
        crate::game_logic::host_ocl_fire_weapon_attack::honesty_ocl_fire_weapon_attack_residual_ok()
    }

    pub fn honesty_ocl_create_debris_ok(&self) -> bool {
        crate::game_logic::host_ocl_create_debris::honesty_ocl_create_debris_residual_ok()
            && (self.ocl_create_debris_reg.plans > 0
                || self.ocl_create_debris_reg.debris_spawned == 0)
    }

    pub fn honesty_ocl_special_power_ok(&self) -> bool {
        self.ocl_special_power_reg.honesty_host_path_ok()
            && crate::game_logic::host_ocl_special_power::honesty_ocl_special_power_residual_ok()
    }

    pub fn tensile_formation_registry(
        &self,
    ) -> &crate::game_logic::host_tensile_formation::HostTensileFormationRegistry {
        &self.tensile_formation_reg
    }

    pub fn honesty_highlander_body_ok(&self) -> bool {
        self.highlander_body_reg.honesty_clamp_ok()
    }

    pub fn honesty_upgrade_die_ok(&self) -> bool {
        self.upgrade_die_reg.honesty_removal_ok()
    }

    pub fn record_highlander_clamp(&mut self) {
        self.highlander_body_reg.record_clamp();
    }

    /// Residual honesty: Battle Bus passenger residual fire.
    pub fn honesty_battle_bus_passenger_fire_ok(&self) -> bool {
        self.battle_bus.honesty_passenger_fire_ok()
    }

    /// Residual honesty: Battle Bus armed-riders weapon-set upgrade.
    pub fn honesty_battle_bus_weapon_set_upgrade_ok(&self) -> bool {
        self.battle_bus.honesty_weapon_set_upgrade_ok()
    }

    /// Residual Tunnel Network honesty: enter count.
    pub fn tunnel_network_residual_enters(&self) -> u32 {
        self.tunnel_network.enters
    }

    /// Residual Tunnel Network honesty: exit count.
    pub fn tunnel_network_residual_exits(&self) -> u32 {
        self.tunnel_network.exits
    }

    /// Residual Tunnel Network honesty: cross-tunnel exit count (enter A, exit B).
    pub fn tunnel_network_residual_cross_exits(&self) -> u32 {
        self.tunnel_network.cross_exits
    }

    /// Residual honesty: tunnel enter → exit path.
    pub fn honesty_tunnel_network_enter_exit_ok(&self) -> bool {
        self.tunnel_network.honesty_enter_exit_ok()
    }

    /// Residual honesty: cross-tunnel exit (enter A, exit B) exercised.
    pub fn honesty_tunnel_network_cross_exit_ok(&self) -> bool {
        self.tunnel_network.honesty_cross_exit_ok()
    }

    /// Residual honesty: any tunnel network residual path.
    pub fn honesty_tunnel_network_ok(&self) -> bool {
        self.tunnel_network.honesty_any_ok()
    }

    /// Residual honesty: TunnelNetworkGun auto-fire residual exercised.
    pub fn honesty_tunnel_network_gun_ok(&self) -> bool {
        self.tunnel_network.honesty_gun_fire_ok()
    }

    /// Residual honesty counter: TunnelNetworkGun residual shots.
    pub fn tunnel_network_residual_gun_fires(&self) -> u32 {
        self.tunnel_network.gun_fires
    }

    /// Residual honesty counter: TunnelNetworkGun residual units hit.
    pub fn tunnel_network_residual_gun_units_hit(&self) -> u32 {
        self.tunnel_network.gun_units_hit
    }

    /// Shared tunnel pool accessors for command residual.
    pub fn tunnel_network_residual(
        &self,
    ) -> &crate::game_logic::host_tunnel_network::HostTunnelNetworkRegistry {
        &self.tunnel_network
    }

    /// Exit one unit from the player's tunnel network at `exit_tunnel`.
    /// Removes local occupant bookkeeping from the entry tunnel if different.
    /// Returns true when the unit was in the shared pool and was released.
    pub fn exit_tunnel_network_unit(&mut self, unit_id: ObjectId, exit_tunnel: ObjectId) -> bool {
        let Some(player_id) = self.tunnel_network.player_holding_unit(unit_id) else {
            return false;
        };
        let entry = self
            .tunnel_network
            .record_exit(player_id, unit_id, exit_tunnel);
        // Remove from entry tunnel local list (and exit tunnel if mirrored).
        if let Some(entry_id) = entry {
            if let Some(container) = self.objects.get_mut(&entry_id) {
                container.remove_occupant(unit_id);
            }
        }
        if entry != Some(exit_tunnel) {
            if let Some(container) = self.objects.get_mut(&exit_tunnel) {
                container.remove_occupant(unit_id);
            }
        }
        // C++ TunnelContain::onRemoving setSafeOcclusionFrame(frame + OcclusionDelay).
        if let Some(unit) = self.objects.get_mut(&unit_id) {
            unit.stamp_safe_occlusion_frame(self.frame);
        }

        true
    }

    /// List all units in a player's shared tunnel pool (for Evacuate residual).
    pub fn tunnel_network_contained_for_player(&self, player_id: u32) -> Vec<ObjectId> {
        self.tunnel_network.contained_for_player(player_id)
    }

    /// C++ CaveContain.cpp:254-336 changeTeamOnAllConnectedCaves.
    pub(in super::super) fn apply_cave_capture_event(
        &mut self,
        index: i32,
        event: crate::game_logic::host_cave_system::CaveCaptureEvent,
    ) {
        use crate::game_logic::host_cave_system::CaveCaptureEvent;
        match event {
            CaveCaptureEvent::None => {}
            CaveCaptureEvent::FirstOccupant(team) => {
                for cid in self.cave_system.cave_ids_for_index(index) {
                    if let Some(cave) = self.objects.get_mut(&cid) {
                        cave.team = team;
                    }
                }
            }
            CaveCaptureEvent::LastEmpty => {
                for cid in self.cave_system.cave_ids_for_index(index) {
                    if let Some(orig) = self.cave_system.original_team_of(cid) {
                        if let Some(cave) = self.objects.get_mut(&cid) {
                            cave.team = orig;
                        }
                    }
                }
            }
        }
    }

    /// C++ ScriptActions.cpp:5063 SET_CAVE_INDEX / CaveContain::tryToSetCaveIndex.
    pub fn try_set_cave_index(&mut self, cave_id: ObjectId, new_index: i32) -> bool {
        if !self
            .objects
            .get(&cave_id)
            .is_some_and(|o| o.is_cave_style_container())
        {
            return false;
        }
        if !self.cave_system.try_set_cave_index(cave_id, new_index) {
            return false;
        }
        if let Some(obj) = self.objects.get_mut(&cave_id) {
            obj.cave_index = new_index;
        }
        true
    }

    /// C++ ScriptActions::doSetCaveIndex named-object lookup.
    pub fn set_named_cave_index(&mut self, cave_name: &str, new_index: i32) -> bool {
        let Some(id) = self.find_object_id_by_name(cave_name) else {
            return false;
        };
        self.try_set_cave_index(id, new_index)
    }

    pub fn exit_cave_unit(&mut self, unit_id: ObjectId, exit_cave: ObjectId) -> bool {
        let Some(index) = self.cave_system.index_holding_unit(unit_id) else {
            return false;
        };
        let (entry, ev) = self.cave_system.record_exit(index, unit_id, exit_cave);
        if let Some(entry_id) = entry {
            if let Some(container) = self.objects.get_mut(&entry_id) {
                container.remove_occupant(unit_id);
            }
        }
        if entry != Some(exit_cave) {
            if let Some(container) = self.objects.get_mut(&exit_cave) {
                container.remove_occupant(unit_id);
            }
        }
        self.apply_cave_capture_event(index, ev);
        true
    }

    pub fn cave_system_residual(&self) -> &crate::game_logic::host_cave_system::HostCaveSystem {
        &self.cave_system
    }

    pub(crate) fn resolve_bridge_span_for_repair(&self, target_id: ObjectId) -> Option<ObjectId> {
        if let Some(sid) = self.bridge_behavior.span_id_for(target_id) {
            return Some(sid);
        }
        if self.bridge_behavior.span(target_id).is_some() {
            return Some(target_id);
        }
        let pos = self.objects.get(&target_id)?.get_position();
        self.objects
            .values()
            .filter(|o| {
                crate::game_logic::host_bridge_behavior::is_bridge_span_template(&o.template_name)
                    || o.is_kind_of(crate::game_logic::KindOf::Bridge)
            })
            .min_by(|a, b| {
                a.get_position()
                    .distance(pos)
                    .partial_cmp(&b.get_position().distance(pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|o| o.id)
    }

    pub(crate) fn spawn_bridge_scaffolding(&mut self, span_id: ObjectId) {
        if !self.bridge_behavior.create_scaffolding(span_id) {
            return;
        }
        let team = self
            .objects
            .get(&span_id)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let center = self
            .bridge_behavior
            .span(span_id)
            .map(|s| (s.from_left + s.from_right + s.to_left + s.to_right) * 0.25)
            .unwrap_or(glam::Vec3::ZERO);
        let tiles = self.bridge_behavior.tiled_scaffold_sites(span_id);
        let mut ids = Vec::new();
        let mut anims = Vec::new();
        for tile in tiles {
            let template = if tile.is_support {
                crate::game_logic::host_bridge_behavior::BRIDGE_SCAFFOLD_SUPPORT_TEMPLATE
            } else {
                crate::game_logic::host_bridge_behavior::BRIDGE_SCAFFOLD_TEMPLATE
            };
            if let Some(sid) = self.create_object(template, team, tile.create_pos) {
                if let Some(obj) = self.objects.get_mut(&sid) {
                    obj.set_orientation(tile.angle);
                }
                ids.push(sid);
                anims.push(
                    crate::game_logic::host_bridge_behavior::HostScaffoldAnim::from_tile(
                        sid, &tile, center,
                    ),
                );
            }
        }
        if let Some(span) = self.bridge_behavior.span_mut(span_id) {
            span.scaffold_ids.extend(ids);
        }
        self.bridge_behavior.bind_scaffold_anims(span_id, anims);
        if let Some(span) = self.bridge_behavior.span(span_id) {
            self.pathfinding_system.grid.stamp_bridge_deck(
                span.from_left,
                span.from_right,
                span.to_left,
                span.to_right,
                true,
            );
        }
    }

    /// C++ `WorkerAIUpdate.cpp:830` / `DozerAIUpdate::removeBridgeScaffolding`.
    pub(crate) fn remove_bridge_scaffolding(&mut self, span_id: ObjectId) {
        let ids = self.bridge_behavior.remove_scaffolding(span_id);
        for sid in ids {
            if let Some(obj) = self.objects.get_mut(&sid) {
                obj.status.destroyed = true;
                obj.health.current = 0.0;
            }
            self.destroy_object(sid);
        }
        let rubble = self.objects.get(&span_id).is_some_and(|o| {
            o.body_damage_state
                == crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                || o.health.current <= 0.0
        });
        if !rubble {
            if let Some(span) = self.bridge_behavior.span(span_id) {
                self.pathfinding_system.grid.stamp_bridge_deck(
                    span.from_left,
                    span.from_right,
                    span.to_left,
                    span.to_right,
                    false,
                );
            }
        }
    }

    fn leftover_bridge_body_state(
        state: crate::game_logic::host_enum_table_residual::HostBodyDamageType,
    ) -> gamelogic::common::BodyDamageType {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        match state {
            HostBodyDamageType::Pristine => gamelogic::common::BodyDamageType::Pristine,
            HostBodyDamageType::Damaged => gamelogic::common::BodyDamageType::Damaged,
            HostBodyDamageType::ReallyDamaged => gamelogic::common::BodyDamageType::ReallyDamaged,
            HostBodyDamageType::Rubble => gamelogic::common::BodyDamageType::Rubble,
        }
    }

    fn host_object_is_bridge_member(obj: &crate::game_logic::object::Object) -> bool {
        obj.is_kind_of(crate::game_logic::KindOf::Bridge)
            || obj.is_kind_of(crate::game_logic::KindOf::BridgeTower)
            || crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                &obj.template_name,
            )
    }

    fn apply_bridge_rubble_kill(&mut self, id: ObjectId) {
        if let Some(obj) = self.objects.get_mut(&id) {
            if obj.status.keep_as_rubble && obj.health.current <= 0.0 {
                return;
            }
            obj.convert_bridge_to_rubble_husk();
        }
    }

    fn apply_pending_bridge_mirrors(&mut self) {
        use crate::game_logic::host_bridge_behavior::HostBridgeMirrorKind;
        loop {
            let events = crate::game_logic::host_bridge_behavior::drain_mirrors();
            if events.is_empty() {
                break;
            }
            for ev in events {
                let source_is_member = ev.source.is_some_and(|sid| {
                    self.objects
                        .get(&sid)
                        .is_some_and(Self::host_object_is_bridge_member)
                });
                if source_is_member {
                    continue;
                }
                let pct = if ev.max_health > 0.0 {
                    ev.amount / ev.max_health
                } else {
                    continue;
                };
                if pct <= 0.0 {
                    continue;
                }
                let targets = self.bridge_behavior.mirror_targets(ev.victim);
                if targets.is_empty() {
                    continue;
                }
                self.bridge_behavior.record_mirror_applied();
                for tid in targets {
                    let Some(max) = self
                        .objects
                        .get(&tid)
                        .map(|o| o.health.maximum.max(o.max_health).max(1.0))
                    else {
                        continue;
                    };
                    let amount = pct * max;
                    match ev.kind {
                        HostBridgeMirrorKind::Damage => {
                            if let Some(obj) = self.objects.get_mut(&tid) {
                                let dtype = crate::game_logic::combat::DamageType::from_store(
                                    gamelogic::damage::DamageType::from_u32(ev.damage_type),
                                );
                                let death =
                                    crate::game_logic::host_usa_pilot::HostDeathType::from_ordinal(
                                        ev.death_type as u8,
                                    );
                                let _ = obj.take_damage_from_typed_death(
                                    amount,
                                    Some(ev.victim),
                                    dtype,
                                    death,
                                );
                            }
                        }
                        HostBridgeMirrorKind::Heal => {
                            if let Some(obj) = self.objects.get_mut(&tid) {
                                obj.revive_from_bridge_rubble();
                                obj.heal(amount);
                            }
                        }
                    }
                }
            }
        }
    }

    fn apply_pending_bridge_death_links(&mut self) {
        let deaths = crate::game_logic::host_bridge_behavior::drain_death_links();
        for victim in deaths {
            let Some(span_id) = self.bridge_behavior.span_id_for(victim) else {
                continue;
            };
            let members = self.bridge_behavior.linked_members(victim);
            self.bridge_behavior.record_death_link_applied();
            if victim == span_id {
                for tid in members {
                    if tid != span_id {
                        self.apply_bridge_rubble_kill(tid);
                    }
                }
            } else {
                self.apply_bridge_rubble_kill(span_id);
                for tid in members {
                    if tid != victim && tid != span_id {
                        self.apply_bridge_rubble_kill(tid);
                    }
                }
            }
        }
    }

    fn bind_bridge_towers_from_terrain(&mut self) {
        let mut binds: Vec<(ObjectId, [ObjectId; 4])> = Vec::new();
        if let Ok(terrain) = gamelogic::terrain::get_terrain_logic().read() {
            terrain.for_each_bridge(|bridge| {
                let info = bridge.get_bridge_info();
                if info.bridge_object_id == 0 {
                    return;
                }
                let span = ObjectId(info.bridge_object_id);
                let towers = [
                    ObjectId(info.tower_object_id[0]),
                    ObjectId(info.tower_object_id[1]),
                    ObjectId(info.tower_object_id[2]),
                    ObjectId(info.tower_object_id[3]),
                ];
                if towers.iter().any(|t| t.0 != 0) {
                    binds.push((span, towers));
                }
            });
        }
        for (span, towers) in binds {
            if self.bridge_behavior.span(span).is_some() {
                self.bridge_behavior.bind_towers(span, towers);
            }
        }
    }

    fn play_bridge_die_fx_at(&mut self, span_id: ObjectId) {
        let pos = self
            .bridge_behavior
            .span(span_id)
            .map(|s| s.random_surface_position(self.frame))
            .or_else(|| self.objects.get(&span_id).map(|o| o.get_position()))
            .unwrap_or(glam::Vec3::ZERO);
        let _ = crate::game_logic::host_fx_list_dispatch::dispatch_fx_list_at_pos(
            crate::game_logic::host_bridge_behavior::BRIDGE_DIE_FX_NAME,
            pos,
        );
        if let Some(obj) = self.objects.get_mut(&span_id) {
            if obj.pending_death_fx.is_none() {
                obj.pending_death_fx =
                    Some(crate::game_logic::host_bridge_behavior::BRIDGE_DIE_FX_NAME.to_string());
            }
            obj.fire_fx_list_die();
        }
    }

    fn play_bridge_die_ocl_at(&mut self, span_id: ObjectId) {
        let pos = self
            .bridge_behavior
            .span(span_id)
            .map(|s| s.random_surface_position(self.frame.wrapping_add(31)))
            .or_else(|| self.objects.get(&span_id).map(|o| o.get_position()))
            .unwrap_or(glam::Vec3::ZERO);
        crate::game_logic::host_transition_damage_fx::play_authored_transition_ocl(
            crate::game_logic::host_bridge_behavior::BRIDGE_DIE_OCL_NAME,
            span_id.0,
            pos,
        );
    }

    fn play_bridge_body_transition(&mut self, span_id: ObjectId, old_state: u8, new_state: u8) {
        let cue = self
            .bridge_behavior
            .body_transition_cues(old_state, new_state);
        if let Some(sound) = cue.sound.as_deref() {
            let pos = self
                .objects
                .get(&span_id)
                .map(|o| o.get_position())
                .unwrap_or(glam::Vec3::ZERO);
            self.queue_audio_event(
                AudioEventRequest::new(sound)
                    .with_object(span_id)
                    .with_position(pos)
                    .with_priority(160),
            );
        }
        for (i, fx) in cue.fx.iter().enumerate() {
            let pos = self
                .bridge_behavior
                .span(span_id)
                .map(|s| s.random_surface_position(self.frame.wrapping_add(i as u32 * 7 + 1)))
                .unwrap_or(glam::Vec3::ZERO);
            let _ = crate::game_logic::host_fx_list_dispatch::dispatch_fx_list_at_pos(fx, pos);
        }
        for (i, ocl) in cue.ocl.iter().enumerate() {
            let pos = self
                .bridge_behavior
                .span(span_id)
                .map(|s| s.random_surface_position(self.frame.wrapping_add(i as u32 * 11 + 3)))
                .unwrap_or(glam::Vec3::ZERO);
            crate::game_logic::host_transition_damage_fx::play_authored_transition_ocl(
                ocl, span_id.0, pos,
            );
        }
    }

    pub(in super::super) fn sync_host_bridge_rubble_and_scaffolds(&mut self) {
        let moved = self.bridge_behavior.tick_scaffolds();
        for (sid, pos) in moved {
            if let Some(obj) = self.objects.get_mut(&sid) {
                obj.set_position(pos);
            }
        }
        self.bind_bridge_towers_from_terrain();
        self.apply_pending_bridge_mirrors();
        self.apply_pending_bridge_death_links();
        let span_ids: Vec<ObjectId> = self
            .objects
            .values()
            .filter(|o| {
                crate::game_logic::host_bridge_behavior::is_bridge_span_template(&o.template_name)
                    || o.is_kind_of(crate::game_logic::KindOf::Bridge)
            })
            .map(|o| o.id)
            .collect();
        for id in span_ids {
            let Some((hp, max, pos, radius, body_state)) = self.objects.get(&id).map(|o| {
                (
                    o.health.current,
                    o.health.maximum,
                    o.get_position(),
                    o.selection_radius.max(20.0),
                    o.body_damage_state,
                )
            }) else {
                continue;
            };
            if self.bridge_behavior.span(id).is_none() {
                self.bridge_behavior.register_span(
                    id,
                    glam::Vec3::new(pos.x - radius, 0.0, pos.z - radius),
                    glam::Vec3::new(pos.x + radius, 0.0, pos.z - radius),
                    glam::Vec3::new(pos.x - radius, 0.0, pos.z + radius),
                    glam::Vec3::new(pos.x + radius, 0.0, pos.z + radius),
                );
            }
            let old_state = self
                .bridge_behavior
                .span(id)
                .map(|s| s.last_body_state)
                .unwrap_or(0);
            if self
                .bridge_behavior
                .note_body_state(id, body_state.ordinal())
            {
                crate::game_logic::host_bridge_behavior::sync_leftover_bridge_body_state(
                    id.0,
                    pos,
                    Self::leftover_bridge_body_state(body_state),
                );
                self.play_bridge_body_transition(id, old_state, body_state.ordinal());
                if matches!(
                    body_state,
                    crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged
                        | crate::game_logic::host_enum_table_residual::HostBodyDamageType::ReallyDamaged
                        | crate::game_logic::host_enum_table_residual::HostBodyDamageType::Pristine
                ) {
                    crate::game_logic::host_radar::host_radar_queue_terrain_refresh();
                }
            }
            let rubble = max <= 0.0
                || hp <= 0.0
                || matches!(
                    body_state,
                    crate::game_logic::host_enum_table_residual::HostBodyDamageType::Rubble
                );
            if rubble {
                let positions: Vec<(ObjectId, glam::Vec3)> = self
                    .objects
                    .iter()
                    .filter(|(oid, o)| {
                        **oid != id
                            && o.is_alive()
                            && !crate::game_logic::host_bridge_behavior::is_bridge_or_tower_template(
                                &o.template_name,
                            )
                    })
                    .map(|(oid, o)| (*oid, o.get_position()))
                    .collect();
                let occupants = self.bridge_behavior.occupants_on_deck(id, &positions);
                crate::game_logic::host_bridge_behavior::sync_leftover_bridge_body_state(
                    id.0,
                    pos,
                    gamelogic::common::BodyDamageType::Rubble,
                );

                if self.bridge_behavior.on_enter_rubble(id, &occupants) {
                    self.bridge_behavior.mark_death(id, self.frame);
                    if let Some(span) = self.bridge_behavior.span(id) {
                        self.pathfinding_system.grid.stamp_bridge_deck(
                            span.from_left,
                            span.from_right,
                            span.to_left,
                            span.to_right,
                            true,
                        );
                    }
                    for uid in occupants {
                        if let Some(unit) = self.objects.get_mut(&uid) {
                            let _ = unit.take_damage_from_typed_death(
                                crate::game_logic::host_bridge_behavior::BRIDGE_SPLAT_DAMAGE,
                                Some(id),
                                crate::game_logic::combat::DamageType::Falling,
                                crate::game_logic::host_usa_pilot::HostDeathType::Splatted,
                            );
                        }
                    }
                    crate::game_logic::host_radar::host_radar_queue_terrain_refresh();
                }
                let due = self.bridge_behavior.take_due_die_fx(id, self.frame);
                for _ in 0..due {
                    self.play_bridge_die_fx_at(id);
                }
                let due_ocl = self.bridge_behavior.take_due_die_ocl(id, self.frame);
                for _ in 0..due_ocl {
                    self.play_bridge_die_ocl_at(id);
                }
            } else {
                self.bridge_behavior.on_leave_rubble(id);
                if !self.bridge_behavior.is_scaffold_present(id) {
                    if let Some(span) = self.bridge_behavior.span(id) {
                        self.pathfinding_system.grid.stamp_bridge_deck(
                            span.from_left,
                            span.from_right,
                            span.to_left,
                            span.to_right,
                            false,
                        );
                    }
                }
            }
        }
    }

    /// Residual Combat Chinook honesty: successful load count.
    pub fn combat_chinook_residual_loads(&self) -> u32 {
        self.combat_chinook.loads
    }

    /// Residual Combat Chinook honesty: successful unload/evacuate count.
    pub fn combat_chinook_residual_unloads(&self) -> u32 {
        self.combat_chinook.unloads
    }

    /// Residual Combat Chinook honesty: passenger fire-from-chinook shots.
    pub fn combat_chinook_residual_passenger_fires(&self) -> u32 {
        self.combat_chinook.passenger_fires
    }

    /// Residual Combat Chinook honesty: armed-riders weapon-set upgrades.
    pub fn combat_chinook_residual_weapon_set_upgrades(&self) -> u32 {
        self.combat_chinook.weapon_set_upgrades
    }

    /// Record a residual Combat Chinook load (tests / host path).
    pub fn record_combat_chinook_residual_load(&mut self) {
        self.combat_chinook.record_load();
    }

    /// Record a residual Combat Chinook unload/evacuate (tests / host path).
    pub fn record_combat_chinook_residual_unload(&mut self) {
        self.combat_chinook.record_unload();
    }

    /// C++ `ChinookAIUpdate::update` residual: auto-land / evac / HeadOffMap / combat-drop height.
    pub fn tick_chinook_ai(&mut self, dt: f32) {
        let step =
            (dt * crate::game_logic::host_combat_chinook::COMBAT_CHINOOK_LOCOMOTOR_SPEED).max(1.0);
        let ids: Vec<_> = self
            .objects
            .iter()
            .filter(|(_, o)| o.chinook_ai.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in ids {
            let wanting = self.objects.values().any(|o| {
                o.target == Some(id) && matches!(o.ai_state, AIState::Entering | AIState::Docking)
            }) || self
                .objects
                .get(&id)
                .is_some_and(|c| c.pending_evacuate_on_stop);
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let p = obj.get_position();
            let contained = obj.contained_units().len() as u32;
            let idle = !obj.status.moving && obj.movement.path.is_empty();
            let Some(ai) = obj.chinook_ai.as_mut() else {
                continue;
            };
            ai.pos = [p.x, p.z, p.y];
            ai.parent_idle = idle;
            ai.wanting_enter_or_exit = wanting;
            ai.contained_count = contained;
            let was_landing = matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
            );
            let was_taking_off = matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::TakingOff
            );
            ai.tick(step);
            let landing_now = matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
            );
            let taking_off_now = matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::TakingOff
            );
            let precise_now = landing_now || taking_off_now;
            let landing_from = glam::Vec3::new(ai.pos[0], ai.pos[2], ai.pos[1]);
            let landing_dest = glam::Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]);
            let apply_landing_dest = landing_now && !was_landing;
            let evac_fly = matches!(
                ai.state,
                crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToAndEvac
                    | crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToAndEvacAndExit
            )
            .then_some(glam::Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]));
            let landed = matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
            );
            let new_pos = glam::Vec3::new(ai.pos[0], ai.pos[2], ai.pos[1]);
            let preferred = if matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landed
                    | crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
            ) {
                0.0
            } else {
                ai.preferred_height
            };
            let destroyed = ai.destroyed;
            let dump_crates =
                crate::game_logic::host_combat_chinook::chinook_flight_dumps_carried_boxes(
                    ai.flight_status,
                );
            drop(ai);

            obj.set_position(new_pos);
            if dump_crates {
                crate::game_logic::host_combat_chinook::lose_all_chinook_object_boxes(obj);
            }

            obj.loco_preferred_height = preferred;
            if let Some(dest) = evac_fly {
                if obj.movement.path.is_empty() && !obj.status.moving {
                    obj.movement.path = vec![new_pos, dest];
                    obj.movement.current_path_index = 1;
                    obj.movement.target_position = Some(dest);
                    obj.set_ai_state(AIState::Moving);
                    obj.status.moving = true;
                    obj.pending_evacuate_on_stop = true;
                }
            }
            if landed {
                // C++ ChinookAIUpdate::chooseLocomotorSet — CHINOOK_LANDED → TAXIING.
                obj.apply_taxiing_locomotor_set();
            } else if obj
                .jet_ai
                .cur_locomotor_set
                .as_deref()
                .is_some_and(|s| s.eq_ignore_ascii_case("SET_TAXIING"))
            {
                obj.apply_airborne_locomotor_set();
            }
            // C++ ChinookTakeoffOrLandingState::onEnter/onExit (ChinookAIUpdate.cpp:223-225, :294-295).
            if precise_now {
                obj.set_precise_z_and_ultra_accurate(true);
            } else if was_landing || was_taking_off {
                obj.set_precise_z_and_ultra_accurate(false);
            }
            if destroyed {
                obj.status.destroyed = true;
            }
            drop(obj);
            if apply_landing_dest {
                let adj = self.pathfinding_system.adjust_landing_destination_for(
                    id.0,
                    &self.objects,
                    landing_from,
                    landing_dest,
                );
                if let Some(obj) = self.objects.get_mut(&id) {
                    if let Some(ai) = obj.chinook_ai.as_mut() {
                        ai.dest = [adj.x, adj.z, adj.y];
                    }
                    obj.movement.target_position = Some(adj);
                    obj.set_precise_z_and_ultra_accurate(true);
                }
            } else if precise_now {
                // C++ setLocomotorGoalPositionExplicit(m_destLoc) each update.
                if let Some(obj) = self.objects.get_mut(&id) {
                    obj.movement.target_position = Some(landing_dest);
                }
            }
        }
    }

    /// Residual honesty: Combat Chinook load → docked → unload path.
    pub fn honesty_combat_chinook_load_unload_ok(&self) -> bool {
        self.combat_chinook.honesty_load_unload_ok()
    }

    /// Residual honesty: Combat Chinook passenger residual fire.
    pub fn honesty_combat_chinook_passenger_fire_ok(&self) -> bool {
        self.combat_chinook.honesty_passenger_fire_ok()
    }

    /// Residual honesty: Combat Chinook armed-riders weapon-set upgrade.
    pub fn honesty_combat_chinook_weapon_set_upgrade_ok(&self) -> bool {
        self.combat_chinook.honesty_weapon_set_upgrade_ok()
    }

    /// C++ TransportContain armed-riders weapon-set residual.
    /// When `armed_riders_upgrade_weapon_set` and any infantry rider has a viable
    /// ranged damage weapon, set WEAPONSET_PLAYER_UPGRADE and bind the passenger
    /// dummy weapon. Clears the flag when no armed riders remain.
    /// Battle Bus binds BattleBusPassengerDummyWeapon; Combat Chinook and
    /// Listening Outpost bind ListeningOutpostUpgradedDummyWeapon.
    pub fn refresh_battle_bus_armed_riders_weapon_set(&mut self, container_id: ObjectId) {
        let Some(container) = self.objects.get(&container_id) else {
            return;
        };
        if !container.armed_riders_upgrade_weapon_set {
            return;
        }
        let is_combat_chinook = container.is_combat_chinook_style_container();
        let is_listening_outpost = container.is_listening_outpost_style_container();
        let is_battle_bus = container.is_battle_bus_style_container();
        let occupant_ids = container.contained_units();
        let was_upgraded = container.weapon_set_player_upgrade;

        let mut any_armed = false;
        for oid in &occupant_ids {
            if let Some(rider) = self.objects.get(oid) {
                let infantry =
                    gamelogic::object::contain::transport_contain_passenger_kind_allowed_to_fire(
                        rider.is_kind_of(KindOf::Infantry),
                    );
                let armed = if is_combat_chinook {
                    crate::game_logic::host_combat_chinook::combat_chinook_rider_has_viable_weapon(
                        rider.weapon.as_ref(),
                        infantry,
                        rider.is_kind_of(KindOf::Vehicle),
                    )
                } else {
                    crate::game_logic::host_battle_bus::rider_has_viable_weapon(
                        rider.weapon.as_ref(),
                        infantry,
                    )
                };
                if armed {
                    any_armed = true;
                    break;
                }
            }
        }

        let mut newly_upgraded = false;
        if let Some(container) = self.objects.get_mut(&container_id) {
            let _ = container.set_weapon_set_flag(0, any_armed);
            if any_armed {
                // Bind residual dummy weapon when primary is empty or still a
                // passenger dummy (PLAYER_UPGRADE weapon set residual).
                let need_dummy = match container.weapon.as_ref() {
                    None => true,
                    Some(w) => {
                        crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(w)
                            || w.damage < 0.01
                    }
                };
                if need_dummy {
                    // Combat Chinook + Listening Outpost share ListeningOutpost dummy.
                    let _ = container.replace_weapon_set_slot(
                        0,
                        Some(if is_combat_chinook || is_listening_outpost {
                            crate::game_logic::host_combat_chinook::listening_outpost_upgraded_dummy_weapon(
                            )
                        } else {
                            crate::game_logic::host_battle_bus::battle_bus_passenger_dummy_weapon()
                        }),
                    );
                }
                newly_upgraded = !was_upgraded;
            } else if was_upgraded {
                // Clear dummy primary when no armed riders remain.
                if container
                    .weapon
                    .as_ref()
                    .map(|w| {
                        crate::game_logic::host_combat_chinook::is_passenger_dummy_weapon(w)
                            || w.damage < 0.01
                    })
                    .unwrap_or(false)
                {
                    let _ = container.replace_weapon_set_slot(0, None);
                }
            }
        }
        if newly_upgraded {
            if is_listening_outpost {
                self.listening_outpost.record_weapon_set_upgrade();
            } else if is_combat_chinook {
                self.combat_chinook.record_weapon_set_upgrade();
            } else if is_battle_bus {
                self.battle_bus.record_weapon_set_upgrade();
            }
        }
    }

    /// Residual Listening Outpost honesty: successful infantry load count.
    pub fn listening_outpost_residual_loads(&self) -> u32 {
        self.listening_outpost.loads
    }

    /// Residual Listening Outpost honesty: successful unload/evacuate count.
    pub fn listening_outpost_residual_unloads(&self) -> u32 {
        self.listening_outpost.unloads
    }

    /// Residual Listening Outpost honesty: passenger fire-from-outpost shots.
    pub fn listening_outpost_residual_passenger_fires(&self) -> u32 {
        self.listening_outpost.passenger_fires
    }

    /// Residual Listening Outpost honesty: armed-riders weapon-set upgrades.
    pub fn listening_outpost_residual_weapon_set_upgrades(&self) -> u32 {
        self.listening_outpost.weapon_set_upgrades
    }

    /// Residual Listening Outpost honesty: detector residual reveals.
    pub fn listening_outpost_residual_detects(&self) -> u32 {
        self.listening_outpost.detects
    }

    /// Residual Listening Outpost honesty: InitialPayload TankHunter docks.
    pub fn listening_outpost_residual_initial_payload_docks(&self) -> u32 {
        self.listening_outpost.initial_payload_docks
    }

    /// Record a residual Listening Outpost load (tests / host path).
    pub fn record_listening_outpost_residual_load(&mut self) {
        self.listening_outpost.record_load();
    }

    /// Record a residual Listening Outpost unload/evacuate (tests / host path).
    pub fn record_listening_outpost_residual_unload(&mut self) {
        self.listening_outpost.record_unload();
    }

    /// Residual honesty: Listening Outpost load → docked → unload path.
    pub fn honesty_listening_outpost_load_unload_ok(&self) -> bool {
        self.listening_outpost.honesty_load_unload_ok()
    }

    /// Residual honesty: Listening Outpost passenger residual fire.
    pub fn honesty_listening_outpost_passenger_fire_ok(&self) -> bool {
        self.listening_outpost.honesty_passenger_fire_ok()
    }

    /// Residual honesty: Listening Outpost armed-riders weapon-set upgrade.
    pub fn honesty_listening_outpost_weapon_set_upgrade_ok(&self) -> bool {
        self.listening_outpost.honesty_weapon_set_upgrade_ok()
    }

    /// Residual honesty: Listening Outpost detector residual revealed a unit.
    pub fn honesty_listening_outpost_detect_ok(&self) -> bool {
        self.listening_outpost.honesty_detect_ok()
    }

    /// Residual honesty: Listening Outpost InitialPayload TankHunter residual.
    pub fn honesty_listening_outpost_initial_payload_ok(&self) -> bool {
        self.listening_outpost.honesty_initial_payload_ok()
    }

    /// Residual honesty: any Listening Outpost residual path.
    pub fn honesty_listening_outpost_ok(&self) -> bool {
        self.listening_outpost.honesty_any_ok()
    }

    /// Residual Troop Crawler honesty accessors.
    pub fn troop_crawler_residual_loads(&self) -> u32 {
        self.troop_crawler.loads
    }
    pub fn troop_crawler_residual_unloads(&self) -> u32 {
        self.troop_crawler.unloads
    }
    pub fn troop_crawler_residual_assault_deploys(&self) -> u32 {
        self.troop_crawler.assault_deploys
    }
    pub fn troop_crawler_residual_detects(&self) -> u32 {
        self.troop_crawler.detects
    }
    pub fn troop_crawler_residual_initial_payloads(&self) -> u32 {
        self.troop_crawler.initial_payloads
    }
    pub fn record_troop_crawler_residual_load(&mut self) {
        self.troop_crawler.record_load();
    }
    pub fn record_troop_crawler_residual_unload(&mut self) {
        self.troop_crawler.record_unload();
    }
    pub fn honesty_troop_crawler_load_unload_ok(&self) -> bool {
        self.troop_crawler.honesty_load_unload_ok()
    }
    pub fn honesty_troop_crawler_wounded_retrieve_ok(&self) -> bool {
        self.troop_crawler.honesty_wounded_retrieve_ok()
    }

    pub fn honesty_troop_crawler_assault_deploy_ok(&self) -> bool {
        self.troop_crawler.honesty_assault_deploy_ok()
    }
    pub fn honesty_troop_crawler_detect_ok(&self) -> bool {
        self.troop_crawler.honesty_detect_ok()
    }
    pub fn honesty_troop_crawler_initial_payload_ok(&self) -> bool {
        self.troop_crawler.honesty_initial_payload_ok()
    }
    pub fn honesty_troop_crawler_ok(&self) -> bool {
        self.troop_crawler.honesty_any_ok()
    }

    /// Dock InitialPayload TankHunter × 2 into a Listening Outpost residual.
    ///
    /// Fail-closed: no dock when TankHunter template is missing or capacity full.
    pub(in super::super) fn apply_listening_outpost_initial_payload(
        &mut self,
        outpost_id: ObjectId,
        team: Team,
        position: Vec3,
    ) {
        use crate::game_logic::host_listening_outpost::{
            LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT, LISTENING_OUTPOST_PAYLOAD_TEMPLATE,
            LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT, preferred_payload_template,
            tank_hunter_missile_weapon,
        };

        // Ensure a payload template is available (retail or host seed).
        if !self
            .templates
            .contains_key(LISTENING_OUTPOST_PAYLOAD_TEMPLATE)
            && !self
                .templates
                .contains_key(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT)
        {
            // Inject residual TankHunter template for host playability residual.
            let mut th = ThingTemplate::new(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT);
            th.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(70.0)
                .set_cost(110, 0);
            self.templates
                .insert(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT.to_string(), th);
        }

        let payload_name = preferred_payload_template(
            self.templates
                .contains_key(LISTENING_OUTPOST_PAYLOAD_TEMPLATE),
            self.templates
                .contains_key(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT),
        );
        let Some(payload_name) = payload_name else {
            return;
        };

        for _ in 0..LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT {
            // Capacity check before spawn (avoid orphan infantry on full residual).
            let has_space = self
                .objects
                .get(&outpost_id)
                .map(|o| o.has_capacity_for(1))
                .unwrap_or(false);
            if !has_space {
                break;
            }

            let Some(hunter_id) = self.create_object(payload_name, team, position) else {
                break;
            };

            // Ensure TankHunter residual missile weapon for armed-riders residual.
            if let Some(hunter) = self.objects.get_mut(&hunter_id) {
                if hunter.weapon.is_none()
                    || hunter
                        .weapon
                        .as_ref()
                        .map(|w| w.damage < 1.0)
                        .unwrap_or(true)
                {
                    hunter.weapon = Some(tank_hunter_missile_weapon());
                }
                hunter.set_contained_by(Some(outpost_id));
                hunter.set_ai_state(AIState::Docked);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(hunter_id, 12);
                }
                hunter.stop_moving();
                hunter.set_status_moving(false);
                hunter.set_status_attacking(false);
                hunter.set_target(None);
                hunter.set_position(position);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(
                        hunter_id,
                        Some([position.x, position.y, position.z]),
                    );
                    hunter.record_host_movement();
                }
            }

            if let Some(outpost) = self.objects.get_mut(&outpost_id) {
                outpost.add_occupant(hunter_id);
            }
            self.listening_outpost.record_initial_payload_dock();
        }

        self.refresh_battle_bus_armed_riders_weapon_set(outpost_id);
    }

    /// Dock InitialPayload Redguard × 8 into a Troop Crawler residual.
    ///
    /// Fail-closed: injects host seed RedGuard template when retail name missing.
    pub(in super::super) fn apply_troop_crawler_initial_payload(
        &mut self,
        crawler_id: ObjectId,
        team: Team,
        position: Vec3,
    ) {
        use crate::game_logic::host_troop_crawler::{
            TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT, TROOP_CRAWLER_PAYLOAD_TEMPLATE,
            TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS, resolve_payload_template_name,
        };
        use crate::game_logic::weapon_bootstrap::REDGUARD_PRIMARY_WEAPON;

        // Ensure a payload template is available (retail or host seed).
        if !self.templates.contains_key(TROOP_CRAWLER_PAYLOAD_TEMPLATE)
            && !self
                .templates
                .contains_key(TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS)
            && !self.templates.contains_key("TestInfantry")
        {
            let mut rg = ThingTemplate::new(TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS);
            rg.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(55.0)
                .set_cost(70, 0)
                .set_primary_weapon_name(REDGUARD_PRIMARY_WEAPON);
            self.templates
                .insert(TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS.to_string(), rg);
        }

        let payload_name = resolve_payload_template_name(|n| self.templates.contains_key(n));
        let Some(payload_name) = payload_name else {
            return;
        };

        for i in 0..TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT {
            let has_space = self
                .objects
                .get(&crawler_id)
                .map(|o| o.has_capacity_for(1))
                .unwrap_or(false);
            if !has_space {
                break;
            }

            let Some(guard_id) = self.create_object(payload_name, team, position) else {
                break;
            };

            if let Some(guard) = self.objects.get_mut(&guard_id) {
                // Ensure residual combat weapon so assault-deploy attack residual works.
                if guard.weapon.is_none() {
                    guard.weapon = Some(Weapon {
                        damage: 15.0,
                        range: 100.0,
                        reload_time: 1.0,
                        last_fire_time: 0.0,
                        ..Weapon::default()
                    });
                }
                guard.set_contained_by(Some(crawler_id));
                guard.set_ai_state(AIState::Docked);
                if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
                    crate::game_logic::host_ai_decision_log::record_set_state(guard_id, 12);
                }
                guard.stop_moving();
                guard.set_status_moving(false);
                guard.set_status_attacking(false);
                guard.set_target(None);
                guard.set_position(position);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    crate::game_logic::host_move_log::record(
                        guard_id,
                        Some([position.x, position.y, position.z]),
                    );
                    guard.record_host_movement();
                }
            }

            if let Some(crawler) = self.objects.get_mut(&crawler_id) {
                crawler.add_occupant(guard_id);
            }
            self.troop_crawler.record_initial_payload();
            // Initial payload docks count as residual loads for honesty bookkeeping.
            let _ = i;
        }
    }

    /// Residual Troop Crawler assault deploy: unload docked infantry and order attack.
    ///
    /// C++ AssaultTransportAIUpdate::beginAssault + update exit/attack residual.
    /// Fail-closed: not wounded-retrieve / multi-exit stagger / heal matrix.
    pub(in super::super) fn apply_troop_crawler_assault_deploy(
        &mut self,
        crawler_id: ObjectId,
        target_id: ObjectId,
    ) -> u32 {
        use crate::game_logic::host_troop_crawler::{
            HostAssaultTransportState, TROOP_CRAWLER_DEPLOY_AUDIO, is_assault_member_wounded,
        };

        let Some(crawler) = self.objects.get(&crawler_id) else {
            return 0;
        };
        if !crawler.is_troop_crawler_style_container() {
            return 0;
        }
        let crawler_pos = crawler.get_position();
        let occupants = crawler.contained_units();
        if occupants.is_empty() {
            // Still record deploy attempt residual (weapon fired DEPLOY pulse).
            self.troop_crawler.record_assault_deploy();
            return 0;
        }

        // C++ update ejects healthy contained members; wounded stay aboard to heal.
        let mut eject_ids: Vec<ObjectId> = Vec::new();
        for occ_id in occupants.iter().copied() {
            let wounded = self
                .objects
                .get(&occ_id)
                .map(|u| is_assault_member_wounded(u.health.current, u.health.maximum))
                .unwrap_or(false);
            if !wounded {
                eject_ids.push(occ_id);
            }
        }

        let mut ordered = 0u32;
        let mut outside_members: Vec<u32> = Vec::new();
        for (i, occ_id) in eject_ids.into_iter().enumerate() {
            // Remove from container.
            if let Some(crawler) = self.objects.get_mut(&crawler_id) {
                crawler.remove_occupant(occ_id);
            }
            // Drop near crawler and order attack.
            let angle = (occ_id.0 as f32 + i as f32 * 1.37).sin().atan2(1.0) + i as f32 * 0.7;
            let offset = Vec3::new(angle.cos(), 0.0, angle.sin()) * 8.0;
            if let Some(unit) = self.objects.get_mut(&occ_id) {
                unit.stop_moving();
                unit.set_position(crawler_pos + offset);
                if crate::gameworld_shadow::gameworld_movement_authority_live() {
                    let p = crawler_pos + offset;
                    crate::game_logic::host_move_log::record(unit.id, Some([p.x, p.y, p.z]));
                    unit.record_host_movement();
                }
                unit.set_contained_by(None);
                unit.set_status_moving(false);
            }
            // GoAggressiveOnExit residual: attack designated target.
            if self.apply_engagement_decision_aware(occ_id, target_id) {
                // The helper has already stored the legal target and attack
                // state.  Do not re-stamp it here: that used to bypass the
                // C++ WeaponSet gate for an incompatible assault passenger.
                if let Some(unit) = self.objects.get_mut(&occ_id) {
                    unit.set_status_attacking(true);
                }
            }
            outside_members.push(occ_id.0);
            ordered = ordered.saturating_add(1);
            self.troop_crawler.record_deploy_attack_order();
            self.troop_crawler.record_unload();
        }

        // Track assault members (outside + still-contained wounded) for retrieve residual.
        // Preserve Attack / AttackMove flags from the player's prior order.
        if let Some(crawler) = self.objects.get_mut(&crawler_id) {
            for occ in crawler.occupants.iter() {
                if !outside_members.contains(&occ.0) {
                    outside_members.push(occ.0);
                }
            }
            let prev = crawler.assault_transport.clone();
            crawler.assault_transport = Some(HostAssaultTransportState::begin_from(
                target_id.0,
                outside_members,
                prev.as_ref(),
            ));
        }

        self.troop_crawler.record_assault_deploy();
        self.queue_audio_event(
            AudioEventRequest::new(TROOP_CRAWLER_DEPLOY_AUDIO)
                .with_object(crawler_id)
                .with_position(crawler_pos)
                .with_priority(140),
        );
        ordered
    }

    #[cfg(test)]
    pub fn apply_troop_crawler_assault_deploy_for_test(
        &mut self,
        crawler_id: ObjectId,
        target_id: ObjectId,
    ) -> u32 {
        self.apply_troop_crawler_assault_deploy(crawler_id, target_id)
    }

    /// Record a residual Overlord BattleBunker enter (tests / host path).
    pub fn record_overlord_bunker_residual_enter(&mut self) {
        self.overlord_bunker_residual_enters =
            self.overlord_bunker_residual_enters.saturating_add(1);
    }

    /// Record a residual Overlord BattleBunker exit/evacuate (tests / host path).
    pub fn record_overlord_bunker_residual_exit(&mut self) {
        self.overlord_bunker_residual_exits = self.overlord_bunker_residual_exits.saturating_add(1);
    }
}
