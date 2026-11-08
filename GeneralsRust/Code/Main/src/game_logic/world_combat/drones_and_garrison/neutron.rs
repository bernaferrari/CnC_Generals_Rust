use super::super::super::*;

impl GameLogic {
    pub fn honesty_neutron_shell_projectile_ok(&self) -> bool {
        self.neutron_shells_spawned > 0
    }

    fn neutron_kill_railed_contained(
        &mut self, container_id: ObjectId, destroy_ids: &mut Vec<ObjectId>,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> u32 {
        let Some(birth) = self.objects.get(&container_id).and_then(|container| {
            (container.thing().template.contain_module.kind == crate::game_logic::ContainModuleKind::RailedTransport)
                .then_some(container.visual_object_generation)
        }) else { return 0 };
        let mut killed = 0u32;
        loop {
            let Some(rider) = self.objects.get(&container_id)
                .filter(|container| container.visual_object_generation == birth)
                .and_then(|container| container.contained_unit_ids().first().copied()) else { break };
            let Some(rider_birth) = self.objects.get(&rider).map(|object| object.visual_object_generation) else { break };
            if !self.railed_kill_contained_with_effects(container_id, rider, effects)
                .expect("rail killAllContained requires its receiving services")
            {
                break;
            }
            if !self.objects.get(&rider).is_some_and(|object| object.visual_object_generation == rider_birth) {
                continue;
            }
            self.apply_owned_kill_with_effects(
                rider, crate::game_logic::combat::DamageType::Unresistable,
                crate::game_logic::host_usa_pilot::HostDeathType::Normal, effects,
            );
            if self.objects.get(&rider).is_some_and(|object| object.visual_object_generation == rider_birth) {
                destroy_ids.push(rider);
            }
            killed = killed.saturating_add(1);
        }
        killed
    }

    /// Apply NeutronBlast residual: kill infantry, unman vehicles, and
    /// `killAllContained` on every in-radius container (including structures).
    /// Returns (infantry_kills, vehicles_unmanned, vehicle_kills).
    pub fn apply_neutron_blast_at(
        &mut self,
        impact: glam::Vec3,
        caster_team: Team,
        caster_id: Option<ObjectId>,
        affect_allies: bool,
    ) -> (u32, u32, u32) {
        self.apply_neutron_blast_at_with_effects(
            impact,
            caster_team,
            caster_id,
            affect_allies,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(crate) fn apply_neutron_blast_at_with_effects(
        &mut self,
        impact: glam::Vec3,
        caster_team: Team,
        caster_id: Option<ObjectId>,
        affect_allies: bool,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> (u32, u32, u32) {
        use crate::game_logic::host_neutron_shell::{
            HOST_NEUTRON_BLAST_RADIUS, NEUTRON_SHELL_AUDIO, NeutronEffect,
            in_neutron_blast_radius_2d, is_legal_neutron_blast_target, neutron_effect_for_target,
        };

        let center = (impact.x, impact.z);
        let radius = HOST_NEUTRON_BLAST_RADIUS;
        let mut infantry_kills = 0u32;
        let mut vehicles_unmanned = 0u32;
        let mut vehicle_kills = 0u32;
        let mut passengers_killed = 0u32;
        let mut destroy_ids: Vec<ObjectId> = Vec::new();
        let mut bomb_detonate_ids: Vec<ObjectId> = Vec::new();
        let mut unmanned_ids: Vec<ObjectId> = Vec::new();

        let candidates: Vec<ObjectId> = self
            .objects
            .registered()
            .filter_map(|(id, obj)| {
                if caster_id == Some(*id) {
                    return None;
                }
                if !obj.is_alive() {
                    return None;
                }
                let pos = obj.get_position();
                if !in_neutron_blast_radius_2d(center, (pos.x, pos.z), radius) {
                    return None;
                }
                let same_team = obj.team == caster_team;
                if !is_legal_neutron_blast_target(
                    obj.is_alive(),
                    obj.is_kind_of(KindOf::Infantry),
                    obj.is_kind_of(KindOf::Vehicle),
                    obj.is_kind_of(KindOf::Aircraft),
                    obj.is_kind_of(KindOf::Structure),
                    obj.is_kind_of(KindOf::Drone),
                    obj.status.airborne_target,
                    false, // AffectAirborne = No for NeutronCannonShell residual
                    same_team,
                    affect_allies,
                ) {
                    return None;
                }
                Some(*id)
            })
            .collect();

        // C++ neutronBlastToObject: if contain → killAllContained, even on
        // structures / transports / drones that are otherwise not unmanned.
        // TunnelContain iterates the shared TunnelTracker pool, not the local door list.
        let contain_pairs: Vec<(ObjectId, Vec<ObjectId>)> = candidates
            .iter()
            .filter_map(|id| {
                let obj = self.objects.get(id)?;
                if obj.thing().template.contain_module.kind == crate::game_logic::ContainModuleKind::RailedTransport {
                    return None;
                }
                let mut occupants = obj.contained_units();
                let is_tunnel = obj.is_tunnel_network_style_container();
                if is_tunnel {
                    for uid in self
                        .tunnel_network
                        .contained_for_player(obj.tunnel_system_key())
                    {
                        if !occupants.contains(&uid) {
                            occupants.push(uid);
                        }
                    }
                }
                if occupants.is_empty() {
                    None
                } else {
                    Some((*id, occupants))
                }
            })
            .collect();

        for id in candidates {
            let Some(obj) = self.objects.get_mut(&id) else {
                continue;
            };
            let effect = neutron_effect_for_target(
                obj.is_kind_of(KindOf::Infantry),
                obj.is_kind_of(KindOf::Vehicle),
                obj.is_kind_of(KindOf::Drone),
                &obj.template_name,
            );
            let birth = obj.visual_object_generation;
            let _ = obj;
            if !matches!(effect, NeutronEffect::KillInfantry) {
                let count = self.neutron_kill_railed_contained(id, &mut destroy_ids, presentation_effects);
                passengers_killed = passengers_killed.saturating_add(count);
                infantry_kills = infantry_kills.saturating_add(count);
            }
            let Some(obj) = self.objects.get_mut(&id)
                .filter(|obj| obj.visual_object_generation == birth) else { continue };
            match effect {
                NeutronEffect::KillInfantry => {
                    // Residual: kill infantry (take full health damage).
                    let receiving_damage_id = obj.id;
                    let receiving_damage_argument_1 = obj.health.current.max(1.0) * 10.0;
                    let _ = self.apply_residual_damage_owned_with_effects(
                        receiving_damage_id,
                        receiving_damage_argument_1,
                        caster_id,
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                            crate::game_logic::combat::DamageType::Unresistable,
                        ),
                        None,
                        presentation_effects,
                    );
                    let obj = self
                        .objects
                        .get_mut(&receiving_damage_id)
                        .expect("bounded residual victim remains installed");
                    if !obj.is_alive() || obj.health.current <= 0.0 {
                        infantry_kills = infantry_kills.saturating_add(1);
                        destroy_ids.push(id);
                    } else {
                        // Force kill residual.
                        let receiving_damage_id = obj.id;
                        let _ = self.apply_residual_damage_owned_with_effects(
                            receiving_damage_id,
                            999_999.0,
                            caster_id,
                            crate::game_logic::combat::DamageType::Unresistable,
                            crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                                crate::game_logic::combat::DamageType::Unresistable,
                            ),
                            None,
                            presentation_effects,
                        );
                        let obj = self
                            .objects
                            .get_mut(&receiving_damage_id)
                            .expect("bounded residual victim remains installed");
                        infantry_kills = infantry_kills.saturating_add(1);
                        destroy_ids.push(id);
                    }
                    let count = self.neutron_kill_railed_contained(id, &mut destroy_ids, presentation_effects);
                    passengers_killed = passengers_killed.saturating_add(count);
                    infantry_kills = infantry_kills.saturating_add(count);
                }
                NeutronEffect::UnmanVehicle => {
                    if obj.is_car_bomb() {
                        // Dead-man trigger residual — detonate after this borrow ends.
                        bomb_detonate_ids.push(id);
                    } else {
                        obj.apply_kill_pilot_unmanned_at_frame(
                            &mut self.health_events,
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                            self.frame,
                        );
                        // C++ NeutonBlastBehavior.cpp:124-127 neutronBlastToObject:
                        //   getAI()->aiIdle(CMD_FROM_AI);
                        //   TheGameLogic->deselectObject(obj, PLAYERMASK_ALL, TRUE);
                        obj.set_ai_state(
                            AIState::Idle,
                            self.frame,
                            &mut crate::game_logic::ObjectAudio::queue(
                                &mut self.queued_audio_events,
                            ),
                        );
                        obj.deselect();
                        // C++ NeutronBlastBehavior: setTeam(neutral) residual.
                        obj.team = Team::Neutral;
                        vehicles_unmanned = vehicles_unmanned.saturating_add(1);
                        unmanned_ids.push(id);
                    }
                }
                NeutronEffect::KillVehicle => {
                    let receiving_damage_target_id = obj.id;
                    let receiving_damage_argument_1 = obj.health.current.max(1.0) * 10.0;
                    let _ = self.apply_residual_damage_owned_with_effects(
                        receiving_damage_target_id,
                        receiving_damage_argument_1,
                        caster_id,
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                            crate::game_logic::combat::DamageType::Unresistable,
                        ),
                        None,
                        presentation_effects,
                    );
                    vehicle_kills = vehicle_kills.saturating_add(1);
                    destroy_ids.push(id);
                }
                NeutronEffect::None => {}
            }
        }

        for (container_id, occupants) in contain_pairs {
            for &occ_id in &occupants {
                self.remove_owned_occupant_with_bonus_callback(container_id, occ_id, presentation_effects);
            }
            for occ_id in occupants {
                if destroy_ids.contains(&occ_id) {
                    continue;
                }
                let Some(occ) = self.objects.get_mut(&occ_id) else {
                    continue;
                };
                if !occ.is_alive() {
                    continue;
                }
                let containment_actor_id = occ.id;
                Self::set_owned_containment_in_parts(
                    &mut self.objects,
                    &mut self.partition_manager,
                    &mut self.ai_groups,
                    &mut self.pathfinding_system,
                    containment_actor_id,
                    None,
                    true,
                );
                let occ = self
                    .objects
                    .get_mut(&containment_actor_id)
                    .expect("containment operation preserves this birth");
                occ.set_ai_state(
                    AIState::Idle,
                    self.frame,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
                let receiving_damage_id = occ.id;
                let receiving_damage_argument_1 = occ.health.current.max(1.0) * 10.0;
                let _ = self.apply_residual_damage_owned_with_effects(
                    receiving_damage_id,
                    receiving_damage_argument_1,
                    caster_id,
                    crate::game_logic::combat::DamageType::Unresistable,
                    crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                        crate::game_logic::combat::DamageType::Unresistable,
                    ),
                    None,
                    presentation_effects,
                );
                let occ = self
                    .objects
                    .get_mut(&receiving_damage_id)
                    .expect("bounded residual victim remains installed");
                if occ.is_alive() && occ.health.current > 0.0 && !occ.status.destroyed {
                    let receiving_damage_target_id = occ.id;
                    let _ = self.apply_residual_damage_owned_with_effects(
                        receiving_damage_target_id,
                        999_999.0,
                        caster_id,
                        crate::game_logic::combat::DamageType::Unresistable,
                        crate::game_logic::host_usa_pilot::HostDeathType::from_host_damage_type(
                            crate::game_logic::combat::DamageType::Unresistable,
                        ),
                        None,
                        presentation_effects,
                    );
                }
                passengers_killed = passengers_killed.saturating_add(1);
                infantry_kills = infantry_kills.saturating_add(1);
                destroy_ids.push(occ_id);
            }
        }

        // C++ deselectObject(PLAYERMASK_ALL): drop unmanned husks from every
        // selection roster so they cannot keep player/AI orders.
        if !unmanned_ids.is_empty() {
            for &id in &unmanned_ids {
                self.admit_local_drawable_deselected(id);
            }
            for player in self.players.values_mut() {
                player
                    .selected_objects
                    .retain(|sid| !unmanned_ids.contains(sid));
            }
        }

        for id in destroy_ids {
            self.mark_object_for_destruction_with_effects(
                id,
                Some(caster_team),
                presentation_effects,
            );
        }
        for id in bomb_detonate_ids {
            let _ = self.maybe_detonate_carbomb_on_unmanned_with_effects(id, presentation_effects);
        }

        self.neutron_shell_residual_blasts = self.neutron_shell_residual_blasts.saturating_add(1);
        self.neutron_shell_residual_infantry_kills = self
            .neutron_shell_residual_infantry_kills
            .saturating_add(infantry_kills);
        self.neutron_shell_residual_vehicles_unmanned = self
            .neutron_shell_residual_vehicles_unmanned
            .saturating_add(vehicles_unmanned);
        let _ = passengers_killed;

        self.queue_audio_event(
            AudioEventRequest::new(NEUTRON_SHELL_AUDIO)
                .with_position(impact)
                .with_priority(155),
        );
        // C++ Weapon.cpp FireFX is fire-time only (shared dispatch owns the muzzle);
        // impact uses ProjectileDetonateFX, never a muzzle flash.

        (infantry_kills, vehicles_unmanned, vehicle_kills)
    }
}
