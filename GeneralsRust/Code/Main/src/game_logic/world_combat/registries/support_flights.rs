//! Leaflet, paradrop, daisy cutter and anthrax bomb flights.

use super::*;

impl GameLogic {
    /// C++ SUPERWEAPON_LeafletDrop AmericaJetB52 + LeafletContainer residual.
    pub fn spawn_leaflet_b52_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        self.spawn_leaflet_b52_flight_with_effects(
            source_id,
            target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_leaflet_b52_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_leaflet_drop::{
            LEAFLET_CONTAINER_OBJECT, LEAFLET_DECAL_RADIUS, LEAFLET_TRANSPORT,
        };
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
        if !self.templates.contains_key(LEAFLET_TRANSPORT) {
            let mut t = ThingTemplate::new(LEAFLET_TRANSPORT);
            t.set_health(500.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(LEAFLET_TRANSPORT.to_string(), t);
        }
        // Ensure container template for drop residual.
        if !self.templates.contains_key(LEAFLET_CONTAINER_OBJECT) {
            let mut t = ThingTemplate::new(LEAFLET_CONTAINER_OBJECT);
            t.set_health(100.0).add_kind_of(KindOf::Projectile);
            self.templates
                .insert(LEAFLET_CONTAINER_OBJECT.to_string(), t);
        }
        let tid = self.create_object_with_script_effects(
            LEAFLET_TRANSPORT,
            team,
            edge,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            o.leaflet_transport_target = Some(target);
            o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
        }
        self.host_leaflet_drops.transports_spawned =
            self.host_leaflet_drops.transports_spawned.saturating_add(1);
        let _ = self.create_delivery_radius_decal_with_effects(
            tid,
            target,
            LEAFLET_DECAL_RADIUS,
            presentation_effects,
        );
        Some(tid)
    }

    pub fn update_leaflet_b52_flights(&mut self) {
        self.update_leaflet_b52_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_leaflet_b52_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_deliver_payload::{
            head_off_map_exit_point_residual, is_off_map_residual,
        };
        use crate::game_logic::host_leaflet_drop::{
            LEAFLET_CONTAINER_OBJECT, LEAFLET_DELIVERY_DISTANCE,
        };

        let tids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.leaflet_transport_target.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(ObjectId, Team, Vec3, ObjectId)> = Vec::new();
        let mut leave = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(target) = o.leaflet_transport_target else {
                continue;
            };
            let pos = o.get_position();
            let dest_off_map = is_off_map_residual(
                target,
                self.world_min.x,
                self.world_min.z,
                self.world_max.x,
                self.world_max.z,
            );
            let dx = target.x - pos.x;
            let dz = target.z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 20.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(140.0);
            if dist > 1.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                o.set_position_with_effects_and_pose_owners(
                    new_pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
                o.movement.velocity = new_pos - pos;
                o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
            }
            if !dest_off_map && dist <= LEAFLET_DELIVERY_DISTANCE * 0.5 {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                let exit = head_off_map_exit_point_residual(
                    new_pos,
                    dx,
                    dz,
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                );
                o.leaflet_transport_target = Some(exit);
                o.kill_delivery_radius_decal();
                drops.push((id, team, target, producer));
            }
            if dest_off_map
                && is_off_map_residual(
                    new_pos,
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                )
            {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
        for (_tid, team, target, producer) in drops {
            let drop_pos = Vec3::new(target.x, 80.0, target.z);
            if let Some(cid) = self.create_object_with_script_effects(
                LEAFLET_CONTAINER_OBJECT,
                team,
                drop_pos,
                None,
                presentation_effects,
            ) {
                if let Some(o) = self.objects.get_mut(&cid) {
                    o.producer_id = Some(producer);
                    o.leaflet_container = true;
                    o.movement.velocity = Vec3::new(0.0, -12.0, 0.0);
                    let _ = o.set_smart_bomb_target(target);
                }
                self.host_leaflet_drops.containers_dropped =
                    self.host_leaflet_drops.containers_dropped.saturating_add(1);
            }
        }

        // C++ LeafletDropBehavior::update first tick: create + attachToObject.
        let containers: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.leaflet_container && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        for id in containers {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let first_fx = self.host_leaflet_drops.try_mark_leaflet_fx_started(id.0);
            if first_fx {
                if let Some(fx_name) =
                    crate::game_logic::host_leaflet_drop::leaflet_fx_particle_name(
                        &o.thing().template,
                    )
                {
                    let p = o.get_position();
                    let _ = self.combat_particles.spawn_named(
                        CombatParticleKind::ParticleSysBone,
                        fx_name,
                        p,
                        self.frame,
                        Some(id),
                        None,
                        presentation_effects,
                    );
                }
            }
            let mut p = o.get_position();
            p.y += o.movement.velocity.y;
            o.set_position_with_effects(
                p,
                self.frame,
                &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
            );
            if p.y <= 5.0 {
                p.y = p.y.max(0.0);
                o.set_position_with_effects(
                    p,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                );
                o.movement.velocity = Vec3::ZERO;
                o.leaflet_container = false;
            }
        }
    }

    /// C++ SUPERWEAPON_Paradrop AmericaJetCargoPlane residual.
    pub fn spawn_paradrop_cargo_plane(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        self.spawn_paradrop_cargo_plane_with_effects(
            source_id,
            target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_paradrop_cargo_plane_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_paradrop::{
            PARADROP_DECAL_RADIUS, PARADROP_PARACHUTE_CONTAINER, PARADROP_TRANSPORT,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let (team, source_owner_player_id) = {
            let source = self.objects.get(&source_id)?;
            let owner_player_id = if source.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(source)?)
            } else {
                None
            };
            (source.team, owner_player_id)
        };
        let source_pos = self
            .objects
            .get(&source_id)
            .map(|o| o.get_position())
            .unwrap_or(target);
        // C++ CREATE_AT_EDGE_NEAR_SOURCE: TerrainLogic::findClosestEdgePoint(owner).
        let mut edge = self.closest_map_edge_point(source_pos);
        edge.y = 160.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
        if !self.templates.contains_key(PARADROP_TRANSPORT) {
            let mut t = ThingTemplate::new(PARADROP_TRANSPORT);
            t.set_health(800.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(PARADROP_TRANSPORT.to_string(), t);
        }
        if !self.templates.contains_key(PARADROP_PARACHUTE_CONTAINER) {
            let mut t = ThingTemplate::new(PARADROP_PARACHUTE_CONTAINER);
            t.set_health(50.0).add_kind_of(KindOf::Projectile);
            self.templates
                .insert(PARADROP_PARACHUTE_CONTAINER.to_string(), t);
        }
        let tid = self.create_object_for_owner_or_team_with_effects(
            PARADROP_TRANSPORT,
            team,
            source_owner_player_id,
            edge,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            o.paradrop_transport_target = Some(target);
            o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
        }
        self.host_paradrops.transports_spawned =
            self.host_paradrops.transports_spawned.saturating_add(1);
        let _ = self.create_delivery_radius_decal_with_effects(
            tid,
            target,
            PARADROP_DECAL_RADIUS,
            presentation_effects,
        );
        Some(tid)
    }

    pub fn update_paradrop_cargo_planes(&mut self) {
        self.update_paradrop_cargo_planes_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_paradrop_cargo_planes_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::host_deliver_payload::{
            head_off_map_exit_point_residual, is_off_map_residual,
        };
        use crate::game_logic::host_paradrop::{
            PARADROP_DELIVERY_DISTANCE, PARADROP_PARACHUTE_CONTAINER,
        };

        let tids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.paradrop_transport_target.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(Team, Option<u32>, Vec3, ObjectId)> = Vec::new();
        let mut leave = Vec::new();
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let Some(target) = o.paradrop_transport_target else {
                continue;
            };
            let pos = o.get_position();
            let dest_off_map = is_off_map_residual(
                target,
                self.world_min.x,
                self.world_min.z,
                self.world_max.x,
                self.world_max.z,
            );
            let dx = target.x - pos.x;
            let dz = target.z - pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            let speed = 18.0_f32;
            let mut new_pos = pos;
            new_pos.y = new_pos.y.max(150.0);
            if dist > 1.0 {
                let step = speed.min(dist);
                new_pos.x += dx / dist * step;
                new_pos.z += dz / dist * step;
                o.set_position_with_effects_and_pose_owners(
                    new_pos,
                    self.frame,
                    &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
                    &self.host_trigger_world,
                    &self.team_factory,
                );
                o.movement.velocity = new_pos - pos;
                o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
            }
            if !dest_off_map && dist <= PARADROP_DELIVERY_DISTANCE {
                let team = o.team;
                let owner_player_id = o.owner_player_id;
                let producer = o.producer_id.unwrap_or(id);
                let exit = head_off_map_exit_point_residual(
                    new_pos,
                    dx,
                    dz,
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                );
                o.paradrop_transport_target = Some(exit);
                o.kill_delivery_radius_decal();
                drops.push((team, owner_player_id, target, producer));
            }
            if dest_off_map
                && is_off_map_residual(
                    new_pos,
                    self.world_min.x,
                    self.world_min.z,
                    self.world_max.x,
                    self.world_max.z,
                )
            {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
        for (team, owner_player_id, target, producer) in drops {
            // Drop a residual parachute marker over the LZ (infantry still from host_paradrops).
            let drop_pos = Vec3::new(target.x, 100.0, target.z);
            if let Some(pid) = self.create_object_for_owner_or_team_with_effects(
                PARADROP_PARACHUTE_CONTAINER,
                team,
                owner_player_id,
                drop_pos,
                None,
                presentation_effects,
            ) {
                if let Some(o) = self.objects.get_mut(&pid) {
                    o.producer_id = Some(producer);
                    o.paradrop_parachute = true;
                    o.movement.velocity = Vec3::new(0.0, -8.0, 0.0);
                    let _ = o.set_smart_bomb_target(target);
                    let _ = o.apply_eject_parachuting();
                }
                self.host_paradrops.parachutes_dropped =
                    self.host_paradrops.parachutes_dropped.saturating_add(1);
            }
        }

        let chutes: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.paradrop_parachute && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in chutes {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let mut p = o.get_position();
            p.y += o.movement.velocity.y;
            // Slow parachute residual.
            if o.movement.velocity.y < -2.0 {
                o.movement.velocity.y = -2.5;
            }
            o.set_position_with_effects(
                p,
                self.frame,
                &mut presentation_effects.reborrow_for_object(&mut self.partition_manager),
            );
            if p.y <= 5.0 {
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
    }

    /// C++ SUPERWEAPON_DaisyCutter / SUPERWEAPON_MOAB jet + bomb residual.
    /// `findOCL` UpgradeOCL SCIENCE_MOAB upgrades a Daisy call to B3 + MOAB
    /// (DeliveryDistance / PreOpenDistance 160). Explicit Moab tier is kept.
    pub fn spawn_daisy_cutter_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier,
    ) -> Option<ObjectId> {
        self.spawn_daisy_cutter_flight_with_effects(
            source_id,
            target,
            tier,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_daisy_cutter_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_daisy_cutter_flight::{
            DaisyFlightPayloadTier, HostDaisyCutterFlightData,
        };
        use crate::game_logic::host_ocl_special_power::{find_ocl_name, peel_for_special_power};
        use crate::game_logic::host_replace_object_upgrade::grant_science_for_upgrade;
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
        edge.y = 160.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
        // C++ findOCL: first owned UpgradeOCL science wins. SCIENCE_MOAB is
        // granted by Upgrade_AmericaMOAB (same SPECIAL_DAISY_CUTTER click).
        let tier = {
            let unlocked: Vec<String> = self
                .objects
                .get(&source_id)
                .and_then(|obj| self.player_owner_for_host_object(obj))
                .and_then(|pid| self.get_player(pid))
                .map(|p| {
                    let mut names: Vec<String> = p.unlocked_sciences.iter().cloned().collect();
                    names.extend(p.completed_upgrades.iter().cloned());
                    for u in &p.completed_upgrades {
                        if let Some(sci) = grant_science_for_upgrade(u) {
                            names.push(sci.to_string());
                        }
                    }
                    names
                })
                .unwrap_or_default();
            let from_ocl = peel_for_special_power("SuperweaponDaisyCutter")
                .map(|peel| {
                    let ocl =
                        find_ocl_name(peel, |s| unlocked.iter().any(|u| u.eq_ignore_ascii_case(s)));
                    DaisyFlightPayloadTier::from_ocl_name(ocl)
                })
                .unwrap_or(tier);
            // findOCL SCIENCE_MOAB upgrades Daisy; explicit Moab tier is kept.
            if from_ocl == DaisyFlightPayloadTier::Moab {
                from_ocl
            } else {
                tier
            }
        };
        let transport = tier.transport();
        let bomb = tier.bomb();
        if !self.templates.contains_key(transport) {
            let mut t = ThingTemplate::new(transport);
            t.set_health(500.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(transport.to_string(), t);
        }
        if !self.templates.contains_key(bomb) {
            let mut t = ThingTemplate::new(bomb);
            t.set_health(100.0).add_kind_of(KindOf::Projectile);
            self.templates.insert(bomb.to_string(), t);
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
            let mut data = HostDaisyCutterFlightData::start(edge, target, tier);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.daisy_cutter_transport = Some(data);
            o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
        }
        self.daisy_cutter_flight_reg.record_transport(tier);
        let _ = self.create_delivery_radius_decal_with_effects(
            tid,
            target,
            crate::game_logic::special_power_strikes::DAISY_CUTTER_RADIUS_CURSOR,
            presentation_effects,
        );
        Some(tid)
    }

    pub fn update_daisy_cutter_flights(&mut self) {
        self.update_daisy_cutter_flights_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn update_daisy_cutter_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;

        let tids: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.daisy_cutter_transport.is_some() && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut drops: Vec<(Team, Vec3, ObjectId, DaisyFlightPayloadTier)> = Vec::new();
        let mut leave = Vec::new();
        let world_min = self.world_min;
        let world_max = self.world_max;
        for id in tids {
            let Some(o) = self.objects.get_mut(&id) else {
                continue;
            };
            let pos = o.get_position();
            let Some(data) = o.daisy_cutter_transport.as_mut() else {
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
            if vel.length_squared() > 1e-6 {
                o.set_orientation_with_partition_owner(
                    vel.z.atan2(vel.x),
                    &mut self.partition_manager,
                );
            }
            if should_drop {
                let team = o.team;
                let producer = o.producer_id.unwrap_or(id);
                drops.push((team, target, producer, tier));
            }
            if at_exit {
                leave.push(id);
            }
        }
        for id in leave {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
        for (team, target, producer, tier) in drops {
            let bomb = tier.bomb();
            let drop_pos = Vec3::new(target.x, 90.0, target.z);
            if let Some(bid) = self.create_object_with_script_effects(
                bomb,
                team,
                drop_pos,
                None,
                presentation_effects,
            ) {
                if let Some(o) = self.objects.get_mut(&bid) {
                    o.producer_id = Some(producer);
                    o.daisy_cutter_bomb = true;
                    // Stash tier via MOAB name residual for detonation path.
                    if tier == DaisyFlightPayloadTier::Moab {
                        o.template_name = bomb.to_string();
                    }
                    o.movement.velocity = Vec3::new(0.0, -16.0, 0.0);
                    let _ = o.set_smart_bomb_target(target);
                }
                self.daisy_cutter_flight_reg.record_drop();
            }
        }

        let bombs: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.daisy_cutter_bomb && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in bombs {
            let (pos, producer, team, is_moab) = {
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
                let is_moab = o.template_name == "MOAB" || o.template_name.contains("MOAB");
                (p, o.producer_id, o.team, is_moab)
            };
            if pos.y <= 5.0 {
                let tier = if is_moab {
                    DaisyFlightPayloadTier::Moab
                } else {
                    DaisyFlightPayloadTier::DaisyCutter
                };
                let _ = self.apply_fuel_air_radius_damage_with_effects(
                    id,
                    producer,
                    team,
                    Vec3::new(pos.x, 0.0, pos.z),
                    tier.primary_damage(),
                    tier.primary_radius(),
                    DamageType::Explosive,
                    presentation_effects,
                );
                if let Some(o) = self.objects.get_mut(&id) {
                    o.ensure_fuel_air_gas_slow_death(self.frame);
                    if o.fuel_air_gas_slow_death.is_some() {
                        self.fuel_air_gas_reg.record_install();
                    }
                }
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                self.daisy_cutter_flight_reg.record_detonation();
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
    }

    /// C++ SUPERWEAPON_AnthraxBomb / AnthraxBombGamma GLAJetCargoPlane residual.
    pub fn spawn_anthrax_bomb_flight(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
    ) -> Option<ObjectId> {
        self.spawn_anthrax_bomb_flight_with_effects(
            source_id,
            target,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn spawn_anthrax_bomb_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::host_anthrax_bomb_flight::{
            ANTHRAX_BOMB_GAMMA_OBJECT, ANTHRAX_TRANSPORT, AnthraxBombPayloadTier,
            HostAnthraxBombFlightData,
        };
        use crate::game_logic::host_ocl_special_power::{
            deliver_payload_for_ocl, resolve_anthrax_bomb_ocl,
        };
        use crate::game_logic::{KindOf, ThingTemplate};

        let (team, source_pos, owner_pid, source_template) =
            if let Some(o) = self.objects.get(&source_id) {
                (
                    o.team,
                    o.get_position(),
                    o.owner_player_id,
                    o.template_name.clone(),
                )
            } else {
                (Team::Neutral, target, None, String::new())
            };
        let mut names: Vec<String> = Vec::new();
        if let Some(pid) = owner_pid {
            if let Some(p) = self.players.get(&pid) {
                names.extend(p.unlocked_sciences.iter().cloned());
                names.extend(p.completed_upgrades.iter().cloned());
            }
            if let Some(ident) = self.player_template_identity(pid) {
                names.push(ident.template_name.clone());
            }
        }
        names.push(source_template.clone());
        // C++ Chem_GLACommandCenter default OCL=SUPERWEAPON_AnthraxBombGamma.
        // Leftover findOCL + Chem identity; never hardcode Base.
        let ocl = resolve_anthrax_bomb_ocl(&source_template, names.iter().map(|s| s.as_str()));
        let mut tier = AnthraxBombPayloadTier::from_ocl(ocl);
        if let Some(deliver) = deliver_payload_for_ocl(ocl) {
            tier = AnthraxBombPayloadTier::from_ocl(&deliver.ocl_name);
            if deliver
                .payload
                .eq_ignore_ascii_case(ANTHRAX_BOMB_GAMMA_OBJECT)
            {
                tier = AnthraxBombPayloadTier::Gamma;
            }
        }
        // C++ CREATE_AT_EDGE_NEAR_SOURCE: TerrainLogic::findClosestEdgePoint(owner).
        let mut edge = self.closest_map_edge_point(source_pos);
        edge.y = 150.0;
        let dx = target.x - edge.x;
        let dz = target.z - edge.z;
        if !self.templates.contains_key(ANTHRAX_TRANSPORT) {
            let mut t = ThingTemplate::new(ANTHRAX_TRANSPORT);
            t.set_health(600.0)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(ANTHRAX_TRANSPORT.to_string(), t);
        }
        let bomb = tier.bomb();
        if !self.templates.contains_key(bomb) {
            let mut t = ThingTemplate::new(bomb);
            t.set_health(80.0).add_kind_of(KindOf::Projectile);
            self.templates.insert(bomb.to_string(), t);
        }
        if matches!(tier, AnthraxBombPayloadTier::Gamma)
            && !self.templates.contains_key(ANTHRAX_BOMB_GAMMA_OBJECT)
        {
            let mut t = ThingTemplate::new(ANTHRAX_BOMB_GAMMA_OBJECT);
            t.set_health(80.0).add_kind_of(KindOf::Projectile);
            self.templates
                .insert(ANTHRAX_BOMB_GAMMA_OBJECT.to_string(), t);
        }
        let tid = self.create_object_with_script_effects(
            ANTHRAX_TRANSPORT,
            team,
            edge,
            None,
            presentation_effects,
        )?;
        if let Some(o) = self.objects.get_mut(&tid) {
            o.note_producer(source_id);
            let mut data = HostAnthraxBombFlightData::start(edge, target, tier);
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            o.anthrax_bomb_transport = Some(data);
            o.movement.target_position = None;
            o.set_orientation_with_partition_owner(dz.atan2(dx), &mut self.partition_manager);
        }
        self.anthrax_bomb_flight_reg.record_transport();
        let _ = self.create_delivery_radius_decal_with_effects(
            tid,
            target,
            crate::game_logic::host_anthrax_bomb_flight::ANTHRAX_DELIVERY_DECAL_RADIUS,
            presentation_effects,
        );
        Some(tid)
    }
}
