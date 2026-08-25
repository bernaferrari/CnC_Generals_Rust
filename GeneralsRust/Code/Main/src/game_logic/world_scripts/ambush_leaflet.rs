//! Host scripts `impl GameLogic` — `ambush_leaflet`.
//! Child of `world_scripts` (itself a child of `game_logic.rs`).
//! paradrop / ambush / leaflet / sneak attack / unit queries
#![allow(unused_imports, non_snake_case)]
use super::super::*;

impl GameLogic {
    /// Queue a host residual America Paradrop / Airborne mission from DoSpecialPower.
    /// Returns mission id when the power maps to a supported residual kind.
    /// Residual unit_count for a queued/completed host paradrop mission.
    pub fn paradrop_mission_unit_count(&self, mission_id: u32) -> Option<u32> {
        self.host_paradrops.get(mission_id).map(|m| m.unit_count)
    }

    pub fn queue_paradrop(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_paradrop::{HostParadropKind, PARADROP_RESIDUAL_TEMPLATE};
        let kind = HostParadropKind::from_command_power(power)?;
        let (source_team, source_owner_player_id) = {
            let source = self.objects.get(&source_object)?;
            let owner_player_id = if source.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(source)?)
            } else {
                None
            };
            (source.team, owner_player_id)
        };
        let frame = self.frame;

        // Prefer the kind's authored template; Tank falls back to Crusader.
        // Infantry General keeps Infa_AmericaInfantryRanger (not USA Rangers).
        let preferred = kind.unit_template();
        let unit_template = if self.templates.contains_key(preferred) {
            preferred.to_string()
        } else if let Some(fallback) = kind
            .fallback_template()
            .filter(|name| self.templates.contains_key(*name))
        {
            fallback.to_string()
        } else {
            self.ensure_paradrop_kind_template(kind);
            preferred.to_string()
        };

        // C++ findOCL science UpgradeOCL: Tank 1/2/3 Crusaders, USA/Infa 5/10/20.
        let unit_count = {
            let sciences: Vec<&str> = source_owner_player_id
                .and_then(|player_id| self.players.get(&player_id))
                .filter(|player| player.team == source_team)
                .into_iter()
                .flat_map(|player| {
                    player
                        .unlocked_sciences
                        .iter()
                        .map(|science| science.as_str())
                })
                .collect();
            kind.payload_count_from_sciences(sciences)
        };
        let id = self.host_paradrops.queue_with_unit_count_for_owner(
            kind,
            source_object,
            source_team,
            source_owner_player_id,
            target_position,
            frame,
            unit_template,
            unit_count,
        );

        // One DeliverPayload plane (hq-byyds). OCL TransportOnly plus
        // spawn_paradrop_cargo_plane used to create two AmericaJetCargoPlanes.
        let _cargo_id = self.host_deliver_payloads.queue_for_owner(
            crate::game_logic::host_deliver_payload::HostDeliverPayloadKind::AmericaParadrop,
            source_object,
            source_team,
            source_owner_player_id,
            target_position,
            frame,
            String::new(),
        );
        let _ = self.spawn_paradrop_cargo_plane(source_object, target_position);

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
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

    /// Ensure residual infantry template used by America Paradrop drop path.
    pub(in super::super) fn ensure_residual_paradrop_infantry_template(&mut self) {
        use crate::game_logic::host_paradrop::PARADROP_RESIDUAL_TEMPLATE;
        if self.templates.contains_key(PARADROP_RESIDUAL_TEMPLATE) {
            return;
        }
        let mut t = ThingTemplate::new(PARADROP_RESIDUAL_TEMPLATE);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0)
            .set_cost(100, 0);
        self.templates
            .insert(PARADROP_RESIDUAL_TEMPLATE.to_string(), t);
    }

    /// Insert the kind's authored payload template when the store is empty.
    pub(in super::super) fn ensure_paradrop_kind_template(
        &mut self,
        kind: crate::game_logic::host_paradrop::HostParadropKind,
    ) {
        use crate::game_logic::host_paradrop::HostParadropKind;
        let name = kind.unit_template();
        if self.templates.contains_key(name) {
            return;
        }
        let mut t = ThingTemplate::new(name);
        match kind {
            HostParadropKind::TankParadrop => {
                t.add_kind_of(KindOf::Vehicle)
                    .add_kind_of(KindOf::Selectable)
                    .add_kind_of(KindOf::Attackable)
                    .set_health(400.0)
                    .set_cost(800, 0);
            }
            HostParadropKind::AmericaParadrop | HostParadropKind::InfantryParadrop => {
                t.add_kind_of(KindOf::Infantry)
                    .add_kind_of(KindOf::Selectable)
                    .add_kind_of(KindOf::Attackable)
                    .set_health(100.0)
                    .set_cost(100, 0);
            }
        }
        self.templates.insert(name.to_string(), t);
    }
    /// Advance pending host paradrops to drop frame and spawn infantry near target.
    pub fn update_paradrops(&mut self) {
        self.host_paradrops.clear_frame_events();

        let plans = self.host_paradrops.plan_due_drops(self.frame);
        for plan in plans {
            if !self.templates.contains_key(&plan.unit_template) {
                self.ensure_paradrop_kind_template(plan.kind);
            }
            let template_name = if self.templates.contains_key(&plan.unit_template) {
                plan.unit_template.clone()
            } else {
                crate::game_logic::host_paradrop::PARADROP_RESIDUAL_TEMPLATE.to_string()
            };

            let mut spawned: Vec<ObjectId> = Vec::with_capacity(plan.spawn_positions.len());
            for pos in &plan.spawn_positions {
                if let Some(id) = self.create_object_for_owner_or_team(
                    &template_name,
                    plan.source_team,
                    plan.source_owner_player_id,
                    *pos,
                ) {
                    // C++ Paradrop PutInContainer AmericaParachute + ParachuteDirectly
                    // residual: elevated infantry freefall aiming at LZ.
                    if let Some(obj) = self.objects.get_mut(&id) {
                        // Elevate residual if spawn is near ground.
                        let mut p = obj.get_position();
                        if p.y < 80.0 {
                            p.y = 120.0;
                            obj.set_position(p);
                            crate::game_logic::host_ground_height_log::record(id, p.y, false);
                            if crate::gameworld_shadow::gameworld_movement_authority_live() {
                                crate::game_logic::host_move_log::record(id, Some([p.x, p.y, p.z]));
                                obj.record_host_movement();
                            }
                        }
                        obj.apply_eject_parachuting();
                    }
                    if self.set_parachute_override_destination(id, plan.target_position) {
                        self.host_deliver_payloads
                            .record_parachute_directly_override();
                    }
                    spawned.push(id);
                }
            }

            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.drop_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::DeathExplosion,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );

            let spawned_count = spawned.len();
            self.host_paradrops
                .record_drop_complete(plan.mission_id, spawned);

            // Complete matching DeliverPayload cargo residual bookkeeping
            // (AmericaJetCargoPlane honesty; infantry already spawned above).
            let cargo_due = self.host_deliver_payloads.plan_due_drops(self.frame);
            for cargo_plan in cargo_due {
                if cargo_plan.kind
                    == crate::game_logic::host_deliver_payload::HostDeliverPayloadKind::AmericaParadrop
                    && cargo_plan.source_object == plan.source_object
                {
                    self.host_deliver_payloads.record_drop_complete(
                        cargo_plan.mission_id,
                        Vec::new(),
                        0,
                    );
                }
            }

            log::info!(
                "Host paradrop {} mission {} completed at {:?} (spawned={}/{})",
                plan.kind.label(),
                plan.mission_id,
                plan.target_position,
                spawned_count,
                plan.spawn_positions.len()
            );
        }
    }

    /// Queue a host residual GLA Rebel Ambush mission from DoSpecialPower.
    /// Returns mission id when the power maps to a supported residual kind.
    /// Residual unit_count for a queued/completed host ambush mission.
    pub fn ambush_mission_unit_count(&self, mission_id: u32) -> Option<u32> {
        self.host_ambushes.get(mission_id).map(|m| m.unit_count)
    }

    pub fn queue_ambush(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_ambush::{HostAmbushKind, resolve_ambush_ocl_payload};
        let kind = HostAmbushKind::from_command_power(power)?;
        let (source_team, source_owner_player_id) = {
            let source = self.objects.get(&source_object)?;
            let owner_player_id = if source.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(source)?)
            } else {
                None
            };
            (source.team, owner_player_id)
        };
        let frame = self.frame;

        // C++ OCLSpecialPower::findOCL: controlling player's sciences only.
        let sciences: Vec<&str> = source_owner_player_id
            .and_then(|player_id| self.players.get(&player_id))
            .filter(|player| player.team == source_team)
            .into_iter()
            .flat_map(|player| {
                player
                    .unlocked_sciences
                    .iter()
                    .map(|science| science.as_str())
            })
            .collect();
        let payload = resolve_ambush_ocl_payload(sciences);
        self.ensure_ambush_unit_template(&payload.unit_template);
        let unit_template = if self.templates.contains_key(&payload.unit_template) {
            payload.unit_template
        } else {
            self.ensure_residual_ambush_infantry_template();
            crate::game_logic::host_ambush::AMBUSH_RESIDUAL_TEMPLATE.to_string()
        };

        let id = self.host_ambushes.queue_with_unit_count_for_owner(
            kind,
            source_object,
            source_team,
            source_owner_player_id,
            target_position,
            frame,
            unit_template,
            payload.unit_count,
        );

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
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
        // C++ ObjectCreationList::create is synchronous in doSpecialPowerAtLocation.
        self.spawn_due_ambushes();
        Some(id)
    }

    /// Ensure the leftover OCL CreateObject template exists (Chem/Demo/Slth/base).
    pub(in super::super) fn ensure_ambush_unit_template(&mut self, name: &str) {
        if name.is_empty() || self.templates.contains_key(name) {
            return;
        }
        let mut t = ThingTemplate::new(name);
        t.add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(100.0)
            .set_cost(100, 0);
        self.templates.insert(name.to_string(), t);
    }

    /// Ensure residual infantry template used by GLA Ambush spawn path.
    pub(in super::super) fn ensure_residual_ambush_infantry_template(&mut self) {
        use crate::game_logic::host_ambush::AMBUSH_RESIDUAL_TEMPLATE;
        self.ensure_ambush_unit_template(AMBUSH_RESIDUAL_TEMPLATE);
    }

    /// Advance pending host ambushes to spawn frame and create infantry near target.
    pub fn update_ambushes(&mut self) {
        self.host_ambushes.clear_frame_events();

        // C++ FadeIn residual: clear STEALTHED after FadeTime frames.
        let fade_due = self.host_ambushes.take_due_fade_clears(self.frame);
        for id in fade_due {
            if let Some(o) = self.objects.get_mut(&id) {
                if o.ambush_fade_in {
                    o.set_status_stealthed(false);
                    o.ambush_fade_in = false;
                }
            }
        }

        self.spawn_due_ambushes();
    }

    fn spawn_due_ambushes(&mut self) {
        let plans = self.host_ambushes.plan_due_spawns(self.frame);
        for plan in plans {
            if !self.templates.contains_key(&plan.unit_template) {
                self.ensure_residual_ambush_infantry_template();
            }
            let template_name = if self.templates.contains_key(&plan.unit_template) {
                plan.unit_template.clone()
            } else {
                crate::game_logic::host_ambush::AMBUSH_RESIDUAL_TEMPLATE.to_string()
            };

            let mut spawned: Vec<ObjectId> = Vec::with_capacity(plan.spawn_positions.len());
            for pos in &plan.spawn_positions {
                if let Some(id) = self.create_object_for_owner_or_team(
                    &template_name,
                    plan.source_team,
                    plan.source_owner_player_id,
                    *pos,
                ) {
                    // C++ CreateObject FadeIn residual: STEALTHED until FadeTime.
                    if crate::game_logic::host_ambush::AMBUSH_FADE_IN {
                        if let Some(o) = self.objects.get_mut(&id) {
                            o.set_status_stealthed(true);
                            o.ambush_fade_in = true;
                        }
                        self.host_ambushes.schedule_fade_in(id, self.frame);
                    }
                    // C++ DiesOnBadLand residual: drown on water/cliff spawn cell.
                    if crate::game_logic::host_ambush::AMBUSH_DIES_ON_BAD_LAND {
                        let (cliff, water) = self.sample_stun_surface_at(*pos);
                        if water || cliff {
                            if let Some(o) = self.objects.get_mut(&id) {
                                o.cell_is_underwater = water;
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
                                o.ambush_fade_in = false;
                                o.set_status_stealthed(false);
                            }
                            self.host_ambushes.record_dies_on_bad_land_kill();
                            self.mark_object_for_destruction(id, None);
                            // Do not count drowned residual as successful spawn.
                            continue;
                        }
                    }
                    spawned.push(id);
                }
            }

            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.spawn_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::DeathExplosion,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );

            let spawned_count = spawned.len();
            self.host_ambushes
                .record_spawn_complete(plan.mission_id, spawned);

            log::info!(
                "Host ambush {} mission {} completed at {:?} (spawned={}/{})",
                plan.kind.label(),
                plan.mission_id,
                plan.target_position,
                spawned_count,
                plan.spawn_positions.len()
            );
        }
    }

    /// Queue a host residual USA Leaflet Drop mission from DoSpecialPower.
    /// Returns mission id when the power maps to a supported residual kind.
    pub fn queue_leaflet_drop(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        use crate::game_logic::host_leaflet_drop::HostLeafletDropKind;
        let kind = HostLeafletDropKind::from_command_power(power)?;
        let source_team = self
            .objects
            .get(&source_object)
            .map(|o| o.team)
            .unwrap_or(Team::Neutral);
        let frame = self.frame;

        let id =
            self.host_leaflet_drops
                .queue(kind, source_object, source_team, target_position, frame);

        // C++ OCLSpecialPower::doSpecialPowerAtLocation → one DeliverPayload
        // AmericaJetB52. Dedicated spawn_leaflet_b52_flight owns that transport
        // (same as A10/Daisy skip execute_ocl). LeafletDropBehavior disable
        // stays host-owned via host_leaflet_drops.queue.
        let _ = self.spawn_leaflet_b52_flight(source_object, target_position);
        // C++ SpecialPowerModule::triggerSpecialPower createViewObject
        // SuperweaponLeafletDrop ViewObjectRange 250 / Duration 30000ms.
        let _ = self.create_special_power_view_object_at(
            source_object,
            target_position,
            crate::game_logic::host_leaflet_drop::LEAFLET_VIEW_OBJECT_RANGE,
            crate::game_logic::host_leaflet_drop::LEAFLET_VIEW_OBJECT_DURATION_FRAMES,
        );

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
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

    /// Advance leaflet clouds past Delay and pulse leftover disable every frame.
    ///
    /// Matches leftover LeafletDropBehavior::update_simple / do_disable_attack:
    /// - Delay residual 2500 ms → 75 frames (`start_frame`)
    /// - After start, pulse every frame (walk-ins get DISABLED_EMP)
    /// - AffectRadius residual 110
    /// - DisabledDuration 20000 ms → 600 frames (DISABLED_EMP, now+dur overwrite)
    /// - ENEMIES infantry + vehicles only (player relationship, not faction Team)
    ///
    /// Fail-closed: not full OCL B52 / LeafletContainer drawable / LeafletFX path.
    pub fn update_leaflet_drops(&mut self) {
        use crate::game_logic::host_leaflet_drop::{
            in_leaflet_radius_2d, is_legal_leaflet_disable_target,
        };

        self.host_leaflet_drops.clear_frame_events();

        let plans = self.host_leaflet_drops.plan_due_impacts(self.frame);
        for plan in plans {
            let center = (plan.target_position.x, plan.target_position.z);
            // C++ LeafletDropBehavior::doDisableAttack:
            //   if (curVictim->getRelationship(object) != ENEMIES) continue;
            // Player relationship, not faction Team equality (FFA / 2v2).
            let (source_owner, source_inst) =
                if let Some(src) = self.objects.get(&plan.source_object) {
                    (
                        self.player_owner_for_host_object(src),
                        src.team_instance_name.clone(),
                    )
                } else {
                    (
                        self.unique_player_id_for_team(plan.source_team),
                        String::new(),
                    )
                };
            let candidates: Vec<(ObjectId, bool, bool, bool, bool)> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() {
                        return None;
                    }
                    // Residual: never leaflet the caster object itself.
                    if *id == plan.source_object {
                        return None;
                    }
                    let pos = obj.get_position();
                    if !in_leaflet_radius_2d(center, (pos.x, pos.z), plan.radius) {
                        return None;
                    }
                    let is_infantry = obj.is_kind_of(KindOf::Infantry);
                    let is_vehicle = obj.is_kind_of(KindOf::Vehicle);
                    let is_enemy = Self::object_relationship_from_owners(
                        &self.players,
                        self.player_owner_for_host_object(obj),
                        &obj.team_instance_name,
                        source_owner,
                        &source_inst,
                    ) == gamelogic::common::Relationship::Enemies;
                    let under_construction =
                        obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                    Some((*id, is_infantry, is_vehicle, is_enemy, under_construction))
                })
                .collect();

            let mut disables: u32 = 0;
            for (id, is_infantry, is_vehicle, is_enemy, under_construction) in candidates {
                if !is_legal_leaflet_disable_target(
                    is_infantry,
                    is_vehicle,
                    true,
                    is_enemy,
                    under_construction,
                ) {
                    continue;
                }
                let Some(target) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !target.is_alive() {
                    continue;
                }
                target.apply_disabled_emp(plan.disable_until_frame);
                disables = disables.saturating_add(1);
            }

            if plan.initial_impact {
                self.queue_audio_event(
                    AudioEventRequest::new(plan.kind.impact_audio())
                        .with_object(plan.source_object)
                        .with_position(plan.target_position)
                        .with_priority(190),
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::WeaponImpact,
                    plan.target_position,
                    self.frame,
                    Some(plan.source_object),
                    None,
                );
                log::info!(
                    "Host leaflet drop {} mission {} completed at {:?} (disables={})",
                    plan.kind.label(),
                    plan.mission_id,
                    plan.target_position,
                    disables
                );
            }

            self.host_leaflet_drops
                .record_impact_complete(plan.mission_id, disables);
        }
    }

    /// Queue a host residual GLA Sneak Attack mission from DoSpecialPower.
    /// Returns mission id when the power maps to a supported residual kind.
    pub fn queue_sneak_attack(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
    ) -> Option<u32> {
        self.queue_sneak_attack_facing(power, source_object, target_position, 0.0)
    }

    /// Same as [`queue_sneak_attack`] with C++ PlaceEvent placement angle.
    pub fn queue_sneak_attack_facing(
        &mut self,
        power: &crate::command_system::SpecialPowerType,
        source_object: ObjectId,
        target_position: Vec3,
        placement_angle: f32,
    ) -> Option<u32> {
        use crate::game_logic::host_sneak_attack::{
            HostSneakAttackKind, SNEAK_ATTACK_RESIDUAL_TEMPLATE,
        };
        let kind = HostSneakAttackKind::from_command_power(power)?;
        let (source_team, source_owner_player_id) = {
            let source = self.objects.get(&source_object)?;
            let owner_player_id = if source.owner_player_id.is_some() {
                Some(self.player_owner_for_host_object(source)?)
            } else {
                None
            };
            (source.team, owner_player_id)
        };
        let frame = self.frame;

        // Prefer retail tunnel template when loaded; otherwise residual TestSneakTunnel.
        let preferred = kind.tunnel_template();
        let tunnel_template = if self.templates.contains_key(preferred) {
            preferred.to_string()
        } else {
            self.ensure_residual_sneak_tunnel_template();
            SNEAK_ATTACK_RESIDUAL_TEMPLATE.to_string()
        };

        let id = self.host_sneak_attacks.queue_for_owner(
            kind,
            source_object,
            source_team,
            source_owner_player_id,
            target_position,
            frame,
            tunnel_template,
            placement_angle,
        );

        // C++ OCL_CreateSneakAttackTunnelStart residual (Start object, Lifetime 5000ms).
        let _ = self.spawn_sneak_attack_tunnel_start(
            id,
            source_team,
            source_owner_player_id,
            target_position,
            placement_angle,
        );

        // C++ SuperweaponLaunched Sneak Attack EVA residual.
        self.try_eva_special_launched_misc_owned(source_owner_player_id, source_team, "sneak");

        self.queue_audio_event(
            AudioEventRequest::new(kind.activate_audio())
                .with_object(source_object)
                .with_position(target_position)
                .with_priority(180),
        );
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

    /// Ensure residual tunnel structure template used by GLA Sneak Attack spawn path.
    pub(in super::super) fn ensure_residual_sneak_tunnel_template(&mut self) {
        use crate::game_logic::host_sneak_attack::SNEAK_ATTACK_RESIDUAL_TEMPLATE;
        if self.templates.contains_key(SNEAK_ATTACK_RESIDUAL_TEMPLATE) {
            return;
        }
        let mut t = ThingTemplate::new(SNEAK_ATTACK_RESIDUAL_TEMPLATE);
        t.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0)
            .set_cost(0, 0);
        self.templates
            .insert(SNEAK_ATTACK_RESIDUAL_TEMPLATE.to_string(), t);
    }

    /// Advance pending host sneak attacks to spawn frame: create tunnel + shockwave.
    ///
    /// Matches residual SuperweaponSneakAttack → Start Lifetime 5000ms → tunnel:
    /// - Spawn delay residual 150 frames
    /// - Shockwave residual SneakAttackShockwaveWeaponBig (50 dmg / radius 50)
    /// - Tunnel template GLASneakAttackTunnelNetwork or residual TestSneakTunnel
    ///
    /// TunnelStart object residual closed; fail-closed vs full Start animation / TunnelContain.
    pub fn update_sneak_attacks(&mut self) {
        use crate::game_logic::host_sneak_attack::{
            SNEAK_ATTACK_RESIDUAL_TEMPLATE, in_sneak_shockwave_radius_2d,
            is_legal_sneak_shockwave_target,
        };

        self.host_sneak_attacks.clear_frame_events();

        // C++ Start FireWeaponUpdate multi-pulse residual (Small + 2× Big).
        let due_pulses = self.host_sneak_attacks.take_due_shockwaves(self.frame);
        for pulse in due_pulses {
            let center = (pulse.target_position.x, pulse.target_position.z);
            let candidates: Vec<(ObjectId, bool)> = self
                .objects
                .iter()
                .filter_map(|(id, obj)| {
                    if !obj.is_alive() {
                        return None;
                    }
                    let pos = obj.get_position();
                    if !in_sneak_shockwave_radius_2d(center, (pos.x, pos.z), pulse.radius) {
                        return None;
                    }
                    let under_construction =
                        obj.status.under_construction || obj.construction_percent + 0.001 < 1.0;
                    Some((*id, under_construction))
                })
                .collect();

            let mut hits: u32 = 0;
            let mut destroy_ids: Vec<(ObjectId, Team)> = Vec::new();
            for (id, under_construction) in candidates {
                if !is_legal_sneak_shockwave_target(true, under_construction) {
                    continue;
                }
                let Some(target) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !target.is_alive() {
                    continue;
                }
                let destroyed =
                    target.take_damage_from_immediate(pulse.damage, Some(pulse.source_object));
                hits = hits.saturating_add(1);
                if destroyed {
                    destroy_ids.push((id, pulse.source_team));
                }
            }
            for (id, killer_team) in destroy_ids {
                self.mark_object_for_destruction(id, Some(killer_team));
            }
            if hits > 0 || pulse.pulse_index == 0 {
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pulse.target_position,
                    self.frame,
                    Some(pulse.source_object),
                    None,
                );
            }
            self.host_sneak_attacks.record_multi_pulse_apply(hits);
            if let Some(m) = self.host_sneak_attacks.get_mut(pulse.mission_id) {
                m.shockwave_hits = m.shockwave_hits.saturating_add(hits);
                m.shockwave_damage_total += pulse.damage * hits as f32;
            }
        }

        let plans = self.host_sneak_attacks.plan_due_spawns(self.frame);
        for plan in plans {
            if !self.templates.contains_key(&plan.tunnel_template) {
                self.ensure_residual_sneak_tunnel_template();
            }
            let template_name = if self.templates.contains_key(&plan.tunnel_template) {
                plan.tunnel_template.clone()
            } else {
                SNEAK_ATTACK_RESIDUAL_TEMPLATE.to_string()
            };

            // C++ CreateObjectDie on Start → destroy Start, spawn real tunnel.
            if let Some(start_id) = self
                .host_sneak_attacks
                .get(plan.mission_id)
                .and_then(|m| m.tunnel_start_object)
            {
                self.mark_object_for_destruction(start_id, None);
            }

            let tunnel_id = self.create_object_for_owner_or_team(
                &template_name,
                plan.source_team,
                plan.source_owner_player_id,
                plan.target_position,
            );
            if let Some(id) = tunnel_id {
                if let Some(o) = self.objects.get_mut(&id) {
                    o.set_orientation(plan.placement_angle);
                }
            }

            // Shockwave damage is multi-pulse residual (applied above); tunnel spawn
            // only creates the structure + audio residual.
            let shockwave_hits = self
                .host_sneak_attacks
                .get(plan.mission_id)
                .map(|m| m.shockwave_hits)
                .unwrap_or(0);
            let shockwave_damage_total = self
                .host_sneak_attacks
                .get(plan.mission_id)
                .map(|m| m.shockwave_damage_total)
                .unwrap_or(0.0);

            self.queue_audio_event(
                AudioEventRequest::new(plan.kind.spawn_audio())
                    .with_object(plan.source_object)
                    .with_position(plan.target_position)
                    .with_priority(190),
            );
            let _ = self.combat_particles.spawn(
                CombatParticleKind::DeathExplosion,
                plan.target_position,
                self.frame,
                Some(plan.source_object),
                None,
            );

            self.host_sneak_attacks.record_spawn_complete(
                plan.mission_id,
                tunnel_id,
                shockwave_hits,
                shockwave_damage_total,
            );

            log::info!(
                "Host sneak attack {} mission {} completed at {:?} (tunnel={:?}, shock_hits={})",
                plan.kind.label(),
                plan.mission_id,
                plan.target_position,
                tunnel_id,
                shockwave_hits
            );
        }
    }

    pub fn get_frame(&self) -> u32 {
        self.frame
    }

    /// Wave 908: single residual snapshot for host stamp after logic ticks.
    #[inline]
    pub fn sim_timing_snapshot(&self) -> SimTimingSnapshot {
        let d = self.last_fixed_step_diagnostics;
        SimTimingSnapshot {
            frame: self.frame,
            steps_run: d.steps_run,
            budget_hit: d.budget_hit,
            accumulated_time_seconds: d.accumulated_time_seconds,
        }
    }

    /// Apply skirmish match rules from UI configuration.
    pub fn set_skirmish_rules(
        &mut self,
        fog_of_war: bool,
        crates_enabled: bool,
        limit_superweapons: bool,
        allow_tech_buildings: bool,
        game_speed: f32,
    ) {
        self.skirmish_rules = SkirmishRulesState {
            fog_of_war,
            crates_enabled,
            limit_superweapons,
            allow_tech_buildings,
            game_speed: game_speed.clamp(0.1, 4.0),
        };
    }

    /// Read-only skirmish rules snapshot.
    pub fn skirmish_rules(&self) -> &SkirmishRulesState {
        &self.skirmish_rules
    }

    /// C++ `GameLogic::xfer` v10 `m_superweaponRestriction` live cap.
    pub fn set_limit_superweapons(&mut self, limited: bool) {
        self.skirmish_rules.limit_superweapons = limited;
    }

    /// Replace the live CaveSystem from a world snapshot tail.
    pub fn restore_cave_system(&mut self, system: crate::game_logic::HostCaveSystem) {
        self.cave_system = system;
    }

    /// Mutable CaveSystem for snapshot tests / restore helpers.
    pub fn cave_system_residual_mut(&mut self) -> &mut crate::game_logic::HostCaveSystem {
        &mut self.cave_system
    }

    /// Replace the live per-player TunnelTracker pool from a snapshot tail.
    pub fn restore_tunnel_network(
        &mut self,
        network: crate::game_logic::HostTunnelNetworkRegistry,
    ) {
        self.tunnel_network = network;
    }

    /// Mutable tunnel pool for snapshot tests.
    pub fn tunnel_network_residual_mut(
        &mut self,
    ) -> &mut crate::game_logic::HostTunnelNetworkRegistry {
        &mut self.tunnel_network
    }

    pub fn world_dimensions(&self) -> (f32, f32) {
        (self.world_width, self.world_height)
    }

    /// Get the current map name
    pub fn get_current_map_name(&self) -> &str {
        &self.map_name
    }

    /// Get total play time for this game session
    pub fn get_total_play_time(&self) -> f32 {
        self.sim_time_seconds
    }

    /// C++ `Player::getPlayerDifficulty` (Player.cpp:1519-1525) plus
    /// `GameLogic::prepareNewGame` (GameLogicDispatch.cpp:295): after NewGame
    /// the session difficulty is ScriptEngine / MSG_NEW_GAME arg 1, not the
    /// AI-manager default Medium.
    pub fn get_difficulty(&self) -> AIDifficulty {
        crate::game_logic::host_faction_skirmish_residual::live_host_session_difficulty()
            .or_else(|| self.ai_manager.dominant_difficulty())
            .unwrap_or(AIDifficulty::Medium)
    }

    /// True when the skirmish/AI manager owns this player id.
    #[inline]
    pub fn ai_manager_contains_player(&self, player_id: u32) -> bool {
        self.ai_manager.ai_players.contains_key(&player_id)
    }

    /// Check if the game is currently in battle
    pub fn is_in_battle(&self) -> bool {
        // Check if any objects are currently in combat
        self.objects
            .values()
            .any(|obj| obj.status.attacking || obj.ai_state == AIState::Attacking)
    }

    pub fn get_world_dimensions(&self) -> (f32, f32) {
        (self.world_width, self.world_height)
    }

    // Command system compatibility methods

    /// Wave 958: legacy alias — prefer [`Self::host_object`] at authority boundaries.
    #[inline]
    pub fn get_object(&self, id: ObjectId) -> Option<&Object> {
        self.host_object(id)
    }

    /// Wave 958: legacy alias — prefer [`Self::host_object_mut`] at authority boundaries.
    #[inline]
    pub fn get_object_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.host_object_mut(id)
    }

    /// Wave 227: alive probe without exposing `&Object` to engine dual-read paths.
    #[inline]
    pub fn object_is_alive(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_alive())
    }

    /// Wave 230: command-system unit mutation APIs (authority owned by GameLogic).
    #[inline]
    pub fn unit_can_move(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.can_move())
    }

    #[inline]
    pub fn unit_can_attack(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.can_attack())
    }

    /// Wave 244: team probe without exposing `&Object`.
    #[inline]
    pub fn unit_team(&self, id: ObjectId) -> Option<Team> {
        self.objects.get(&id).map(|o| o.team)
    }

    /// Wave 244: alive probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_alive(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_alive())
    }

    /// C++ ActionManager service actions reject either endpoint while it is
    /// contained by another object. Keep this as an authority probe instead
    /// of inferring containment from movement or a template role.
    #[inline]
    pub fn unit_is_contained(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.contained_by.is_some())
    }

    /// Wave 244: worker probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_worker(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_worker())
    }

    /// C++ construction/structure-repair authority is `KINDOF_DOZER`, not
    /// the host's broader legacy worker presentation category.
    #[inline]
    pub fn unit_is_dozer(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.is_kind_of(KindOf::Dozer))
    }

    /// C++ `Object::isAboveTerrain` gate used by aircraft repair requests.
    /// `airborne_target` is the retained authoritative flight state; when it
    /// is unavailable, accept only a sourced terrain sample showing a strictly
    /// elevated position.  Missing terrain/state fails closed.
    #[inline]
    pub fn unit_is_above_terrain(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            o.status.airborne_target
                || (o.ground_height_from_terrain && o.get_position().y > o.ground_height + 0.01)
        })
    }

    /// C++ `KINDOF_HARVESTER` probe used by the Gather authority path.
    /// Keep it distinct from the builder/worker probe above: a Dozer can
    /// construct or repair without being allowed to collect resources.
    #[inline]
    pub fn unit_is_resource_collector(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.is_resource_collector())
    }

    /// Exact target DockUpdate family for command classification.  This is a
    /// template field parsed from Object INI Behavior declarations, rather
    /// than a name/containment heuristic.
    #[inline]
    pub fn unit_dock_kind(&self, id: ObjectId) -> DockKind {
        self.objects
            .get(&id)
            .map(|o| o.thing.template.dock_kind)
            .unwrap_or(DockKind::None)
    }

    /// Supply carried by a collector or remaining at a warehouse, exposed as
    /// a value probe for boot-only RMB classification.
    #[inline]
    pub fn unit_stored_supplies(&self, id: ObjectId) -> u32 {
        self.objects
            .get(&id)
            .map(|o| o.stored_resources.supplies)
            .unwrap_or(0)
    }

    /// Wave 244: repair/dozer probe without exposing `&Object`.
    #[inline]
    pub fn unit_can_repair(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.can_repair())
    }

    /// Wave 244: hero probe without exposing `&Object`.
    /// C++ `Object::isHero` includes contained KINDOF_HERO occupants.
    #[inline]
    pub fn unit_is_hero(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| self.host_object_is_hero(o))
    }

    /// leftover `Object::is_hero` plus live occupant KindOf residual.
    pub(crate) fn host_object_is_hero(&self, o: &crate::game_logic::object::Object) -> bool {
        o.is_hero()
            || o.contained_units().iter().any(|oid| {
                self.objects
                    .get(oid)
                    .is_some_and(|u| u.is_kind_of(KindOf::Hero))
            })
    }

    /// Wave 244: KindOf probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_kind_of(&self, id: ObjectId, kind: KindOf) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_kind_of(kind))
    }

    /// Wave 244: template name probe without exposing `&Object`.
    #[inline]
    pub fn unit_template_name(&self, id: ObjectId) -> Option<String> {
        self.objects.get(&id).map(|o| o.template_name.clone())
    }

    /// Wave 244: existence probe without exposing `&Object`.
    #[inline]
    pub fn unit_exists(&self, id: ObjectId) -> bool {
        self.objects.contains_key(&id)
    }

    /// Wave 244: under-construction status without exposing `&Object`.
    #[inline]
    pub fn unit_under_construction(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.status.under_construction)
    }

    /// Wave 244: damaged/injured probe without exposing `&Object`.
    #[inline]
    pub fn unit_needs_service(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.health.current + 0.01 < o.health.maximum)
    }

    /// Wave 245: dead probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_dead(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.health.current <= 0.0)
    }

    /// Wave 245: sold status without exposing `&Object`.
    #[inline]
    pub fn unit_is_sold(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.status.sold)
    }

    /// Wave 245: resource/harvestable probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_resource_target(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            o.is_kind_of(KindOf::SupplySource)
                || o.is_kind_of(KindOf::Harvestable)
                || o.is_kind_of(KindOf::Resource)
                || o.object_type == ObjectType::Supply
        })
    }

    /// Wave 245: can-contain probe without exposing `&Object`.
    #[inline]
    pub fn unit_can_contain(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.can_contain())
    }

    /// Wave 245: medical facility probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_medical_facility(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.is_kind_of(KindOf::HealPad))
    }

    /// Wave 245: building type probe without exposing `&Object`.
    #[inline]
    pub fn unit_building_type(&self, id: ObjectId) -> Option<crate::game_logic::BuildingType> {
        self.objects
            .get(&id)
            .and_then(|o| o.building_data.as_ref().map(|b| b.building_type))
    }

    /// Wave 245: faction structure probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_faction_structure(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.is_faction_structure())
    }

    /// Wave 245: container has space for one more without exposing `&Object`.
    #[inline]
    pub fn unit_has_capacity_for(&self, id: ObjectId, count: usize) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.has_capacity_for(count))
    }

    /// Wave 245: container contains unit without exposing `&Object`.
    #[inline]
    pub fn unit_contains(&self, container: ObjectId, unit: ObjectId) -> bool {
        self.objects
            .get(&container)
            .is_some_and(|o| o.contained_units().contains(&unit))
    }

    /// Wave 245: container has any occupants without exposing `&Object`.
    #[inline]
    pub fn unit_has_occupants(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| !o.contained_units().is_empty())
    }

    /// True only when the target has retained enough exact ContainModule data
    /// (or an explicitly implemented host role) to validate normal player
    /// Enter.  Missing/unsupported module masks fail closed.
    #[inline]
    pub fn unit_supports_normal_enter(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.supports_normal_enter())
    }

    /// Enter-time `AllowInsideKindOf = INFANTRY` probe without exposing
    /// `&Object`.  This reads parsed/typed contain metadata rather than a
    /// vehicle footprint or template spelling.
    #[inline]
    pub fn unit_enter_infantry_only(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.normal_enter_requires_infantry())
    }

    /// Enter-time `ForbidInsideKindOf = AIRCRAFT` (or an equivalent retained
    /// allow mask) probe without exposing `&Object`.
    #[inline]
    pub fn unit_enter_forbids_aircraft(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.normal_enter_forbids_aircraft())
    }

    /// C++ `Object::getTransportSlotCount` raw authored value.
    #[inline]
    pub fn unit_transport_slot_count(&self, id: ObjectId) -> usize {
        self.objects
            .get(&id)
            .map(|o| o.transport_slot_count())
            .unwrap_or(0)
    }

    /// C++ OpenContain relationship admission for normal Enter.
    #[inline]
    pub fn unit_allows_normal_enter_from_team(&self, id: ObjectId, team: Team) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.allows_normal_enter_from_team(team))
    }

    /// C++ `DISABLED_SUBDUED` closes a container to normal Enter.
    #[inline]
    pub fn unit_is_subdued_disabled(&self, id: ObjectId) -> bool {
        self.objects
            .get(&id)
            .is_some_and(|o| o.is_subdued_disabled())
    }

    /// Wave 245: selectable probe without exposing `&Object`.
    #[inline]
    pub fn unit_is_selectable(&self, id: ObjectId) -> bool {
        self.objects.get(&id).is_some_and(|o| o.is_selectable())
    }

    /// Wave 245: object-type probe without exposing `&Object`.
    #[inline]
    pub fn unit_object_type(&self, id: ObjectId) -> Option<ObjectType> {
        self.objects.get(&id).map(|o| o.object_type)
    }

    /// Wave 245: position probe without exposing `&Object`.
    #[inline]
    pub fn unit_position(&self, id: ObjectId) -> Option<glam::Vec3> {
        self.objects.get(&id).map(|o| o.get_position())
    }

    /// Wave 245: selectable similar-unit ids (select-similar boot residual).
    pub fn selectable_similar_unit_ids(
        &self,
        team: Team,
        template_name: &str,
        object_type: ObjectType,
        match_object_type: bool,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == team
                    && obj.is_selectable()
                    && (obj.template_name == template_name
                        || (match_object_type && obj.object_type == object_type))
                {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Wave 245: selectable unit ids in world XZ bounds (box-select boot residual).
    pub fn selectable_unit_ids_in_bounds(
        &self,
        team: Team,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team != team || !obj.is_selectable() {
                    return None;
                }
                let p = obj.get_position();
                if p.x >= min_x && p.x <= max_x && p.z >= min_z && p.z <= max_z {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Player-owned selectable ids in world XZ bounds.  This is the command
    /// path counterpart to `select_objects`: faction alone is ambiguous in a
    /// same-faction skirmish.
    pub fn selectable_unit_ids_in_bounds_for_player(
        &self,
        player_id: u32,
        min_x: f32,
        max_x: f32,
        min_z: f32,
        max_z: f32,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.owner_player_id != Some(player_id) || !obj.is_selectable() {
                    return None;
                }
                let p = obj.get_position();
                (p.x >= min_x && p.x <= max_x && p.z >= min_z && p.z <= max_z).then_some(id)
            })
            .collect()
    }

    /// Wave 245: selectable unit ids on a team matching a unit-id predicate.
    pub fn selectable_unit_ids_for_team_where(
        &self,
        team: Team,
        mut predicate: impl FnMut(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == team && obj.is_selectable() && predicate(id) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Exact-player variant used by live input selection. The team-based
    /// probe remains for legacy/script callers that truly have no player id.
    pub fn selectable_unit_ids_for_player_where(
        &self,
        player_id: u32,
        mut predicate: impl FnMut(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                (obj.owner_player_id == Some(player_id) && obj.is_selectable() && predicate(id))
                    .then_some(id)
            })
            .collect()
    }

    /// Wave 245: all unit ids on a team matching a unit-id predicate.
    pub fn unit_ids_for_team_where(
        &self,
        team: Team,
        mut predicate: impl FnMut(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                if obj.team == team && predicate(id) {
                    Some(id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Exact-player unit query for hotkey/cycle selection.
    pub fn unit_ids_for_player_where(
        &self,
        player_id: u32,
        mut predicate: impl FnMut(ObjectId) -> bool,
    ) -> Vec<ObjectId> {
        self.objects
            .iter()
            .filter_map(|(&id, obj)| {
                (obj.owner_player_id == Some(player_id) && predicate(id)).then_some(id)
            })
            .collect()
    }
}
